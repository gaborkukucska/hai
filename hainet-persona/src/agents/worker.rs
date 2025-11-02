//! # START OF FILE hainet-persona/src/agents/worker.rs
//! Worker Agent
//! 
//! Executes individual tasks using MCP tools.
//! Worker agents follow this state machine:
//! Idle → Planning → Working → Reporting → (Idle | Error)

use anyhow::{Result, Context};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::messaging::{MessageBus, AgentId};
use crate::prompts::{PromptManager, AgentType, AgentState, WorkerType, PromptContext};
use crate::projects::{ProjectManager, TaskId};
use crate::tools::mcp::MCPClientManager;
use crate::ai_providers::providers::{OllamaClient, ProviderClient, GenerationOptions};
use super::state::AgentStateMachine;
use super::templates::WorkerTemplate;
use serde_json::json;

/// Worker Agent
/// 
/// Responsible for:
/// - Executing specific tasks assigned by PM using LLM-powered planning
/// - Using MCP tools to accomplish work
/// - Reporting results back to PM for validation
pub struct WorkerAgent {
    /// Unique agent identifier
    id: AgentId,
    
    /// Worker specialization type
    worker_type: WorkerType,
    
    /// Worker template with capabilities and system prompt
    template: WorkerTemplate,
    
    /// Current task being executed
    current_task: Option<TaskId>,
    
    /// State machine
    state_machine: AgentStateMachine,
    
    /// Message bus for communication
    message_bus: Arc<RwLock<MessageBus>>,
    
    /// Prompt manager for generating prompts
    prompt_manager: Arc<PromptManager>,
    
    /// Project manager for task updates
    project_manager: Arc<RwLock<ProjectManager>>,
    
    /// MCP client for tool access
    mcp_client: Arc<RwLock<MCPClientManager>>,
    
    /// Ollama client for LLM-powered task analysis
    ollama_client: OllamaClient,
    
    /// Maximum retry attempts for failed operations
    max_retries: usize,
}

impl WorkerAgent {
    /// Create new worker agent with template
    pub fn new(
        worker_type: WorkerType,
        message_bus: Arc<RwLock<MessageBus>>,
        prompt_manager: Arc<PromptManager>,
        project_manager: Arc<RwLock<ProjectManager>>,
        mcp_client: Arc<RwLock<MCPClientManager>>,
    ) -> Self {
        let id = AgentId::new(AgentType::Worker, format!("Worker-{:?}", worker_type));
        
        // Select appropriate template based on worker type
        let template = match worker_type {
            WorkerType::Files => WorkerTemplate::file_worker(),
            WorkerType::Network => WorkerTemplate::network_worker(),
            WorkerType::Research => WorkerTemplate::research_worker(),
            WorkerType::Compute => WorkerTemplate::file_worker(), // Default to file worker
            _ => WorkerTemplate::file_worker(),
        };
        
        Self {
            id,
            worker_type,
            template,
            current_task: None,
            state_machine: AgentStateMachine::new(),
            message_bus,
            prompt_manager,
            project_manager,
            mcp_client,
            ollama_client: OllamaClient::localhost(),
            max_retries: 3,
        }
    }
    
    /// Create worker from template
    pub fn from_template(
        template: WorkerTemplate,
        message_bus: Arc<RwLock<MessageBus>>,
        prompt_manager: Arc<PromptManager>,
        project_manager: Arc<RwLock<ProjectManager>>,
        mcp_client: Arc<RwLock<MCPClientManager>>,
    ) -> Self {
        let id = AgentId::new(AgentType::Worker, template.name.clone());
        
        Self {
            id,
            worker_type: WorkerType::Files, // Default, overridden by template
            template,
            current_task: None,
            state_machine: AgentStateMachine::new(),
            message_bus,
            prompt_manager,
            project_manager,
            mcp_client,
            ollama_client: OllamaClient::localhost(),
            max_retries: 3,
        }
    }
    
    /// Get agent ID
    pub fn id(&self) -> &AgentId {
        &self.id
    }
    
    /// Get current state
    pub fn state(&self) -> &AgentState {
        self.state_machine.current_state()
    }
    
    /// Get worker type
    pub fn worker_type(&self) -> &WorkerType {
        &self.worker_type
    }
    
    /// Assign task to worker
    pub async fn assign_task(&mut self, task_id: TaskId) -> Result<()> {
        // Must be in Idle state to accept tasks
        if !matches!(self.state_machine.current_state(), AgentState::Idle) {
            return Err(anyhow::anyhow!("Worker not in Idle state, cannot assign task"));
        }
        
        self.current_task = Some(task_id.clone());
        
        // Update task status to Assigned
        let project_manager = self.project_manager.write().await;
        project_manager.assign_task(&task_id, self.id.clone()).await?;
        
        Ok(())
    }
    
    /// Execute assigned task with LLM-powered planning
    pub async fn execute_task(&mut self) -> Result<()> {
        let task_id = self.current_task.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No task assigned"))?;
        
        // Transition to Planning
        self.state_machine.transition(
            AgentState::Planning,
            "Analyzing task requirements".to_string()
        )?;
        
        // Get task details
        let task = self.get_task_details(task_id).await?;
        
        // Use LLM to analyze task and plan tool execution
        let execution_plan = self.plan_task_execution(&task.description).await?;
        
        tracing::info!("Worker {} planned {} steps for task: {}", 
                       self.id.name, execution_plan.steps.len(), task.title);
        
        // Transition to Working
        self.state_machine.transition(
            AgentState::Working,
            "Executing task".to_string()
        )?;
        
        // Execute task using MCP tools with retry logic
        let deliverables = self.execute_with_retries(&execution_plan).await?;
        
        // Transition to Reporting
        self.state_machine.transition(
            AgentState::Reporting,
            "Task complete, reporting to PM".to_string()
        )?;
        
        // Submit task for review
        let project_manager = self.project_manager.write().await;
        project_manager.complete_task(task_id, deliverables).await?;
        
        tracing::info!("Worker {} completed task: {}", self.id.name, task.title);
        
        Ok(())
    }
    
    /// Wait for PM validation with real task polling
    pub async fn await_validation(&mut self) -> Result<bool> {
        let task_id = self.current_task.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No task assigned"))?
            .clone();
        
        tracing::info!(
            "Worker {} awaiting PM validation for task {}",
            self.id.name,
            task_id
        );
        
        // Poll every 100ms for status changes
        let poll_interval = tokio::time::Duration::from_millis(100);
        let max_wait = tokio::time::Duration::from_secs(60); // 1 minute timeout
        let start = tokio::time::Instant::now();
        
        loop {
            if start.elapsed() > max_wait {
                return Err(anyhow::anyhow!(
                    "Validation timeout after {}s", 
                    max_wait.as_secs()
                ));
            }
            
            // Get current task status
            let task_status = {
                let pm = self.project_manager.read().await;
                pm.get_task_status(&task_id).await?
            };
            
            match task_status {
                crate::projects::TaskStatus::Complete => {
                    tracing::info!(
                        "Worker {} task {} approved by PM",
                        self.id.name,
                        task_id
                    );
                    
                    // Transition to Idle
                    self.state_machine.transition(
                        AgentState::Idle,
                        "Task approved by PM".to_string()
                    )?;
                    self.current_task = None;
                    return Ok(true);
                }
                
                crate::projects::TaskStatus::NeedsRevision => {
                    tracing::warn!(
                        "Worker {} task {} needs revision",
                        self.id.name,
                        task_id
                    );
                    
                    return self.handle_revision_request(&task_id).await;
                }
                
                crate::projects::TaskStatus::Failed => {
                    let task = {
                        let pm = self.project_manager.read().await;
                        pm.get_task(&task_id).await?
                    };
                    
                    return Err(anyhow::anyhow!(
                        "Task failed: {}", 
                        task.failure_reason.as_deref().unwrap_or("Unknown reason")
                    ));
                }
                
                crate::projects::TaskStatus::UnderReview => {
                    // Still waiting for PM validation
                    tokio::time::sleep(poll_interval).await;
                    continue;
                }
                
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unexpected task status: {:?}", 
                        task_status
                    ));
                }
            }
        }
    }
    
    /// Handle revision request from PM
    fn handle_revision_request<'a>(&'a mut self, task_id: &'a TaskId) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool>> + 'a>> {
        Box::pin(async move {
            let task = {
                let pm = self.project_manager.read().await;
                pm.get_task(task_id).await?
            };
            
            tracing::info!(
                "Worker {} handling revision request (attempt {}/{})",
                self.id.name,
                task.revision_count,
                task.max_revisions
            );
            
            if !task.can_retry_revision() {
                return Err(anyhow::anyhow!(
                    "Max revisions ({}) exceeded", 
                    task.max_revisions
                ));
            }
            
            // Get PM feedback
            let feedback = task.pm_feedback.clone()
                .unwrap_or_else(|| "No specific feedback provided".to_string());
            
            tracing::info!(
                "Worker {} revision feedback: {}",
                self.id.name,
                feedback
            );
            
            // Reset task for revision
            {
                let pm = self.project_manager.write().await;
                let mut task = pm.get_task(task_id).await?;
                task.reset_for_revision()?;
                pm.request_revision(task_id, feedback.clone()).await?;
            }
            
            // Transition back to Planning to retry with feedback in context
            self.state_machine.transition(
                AgentState::Planning,
                format!("Revision requested: {}", feedback)
            )?;
            
            // Re-execute task with PM feedback
            self.execute_task().await?;
            
            // Wait for validation again (use Box::pin to avoid infinite size)
            self.await_validation().await
        })
    }
    
    /// Handle error and transition to Error state
    pub fn handle_error(&mut self, error: String) {
        self.state_machine.force_error(error);
    }
    
    /// Get task details from project manager
    async fn get_task_details(&self, task_id: &TaskId) -> Result<crate::projects::Task> {
        let pm = self.project_manager.read().await;
        let projects = pm.list_active_projects().await?;
        
        for project in projects {
            let tasks = pm.get_project_tasks(&project.id).await?;
            if let Some(task) = tasks.iter().find(|t| &t.id == task_id) {
                return Ok(task.clone());
            }
        }
        
        Err(anyhow::anyhow!("Task not found: {}", task_id))
    }
    
    /// Plan task execution using LLM
    async fn plan_task_execution(&self, task_description: &str) -> Result<ExecutionPlan> {
        // Use template's system prompt directly
        let system_prompt = self.template.system_prompt.clone();
        
        let planning_prompt = format!(
            "Task: {}\\n\\nYou are a {} worker agent.\\n\\n\
             Your capabilities: {:?}\\n\
             Available MCP servers: {:?}\\n\\n\
             Break this task into specific tool execution steps.\\n\\n\
             Return JSON format:\\n\
             {{\\n\
               \\\"steps\\\": [\\n\
                 {{\\\"tool\\\": \\\"server::tool_name\\\", \\\"params\\\": {{...}}, \\\"description\\\": \\\"what this does\\\"}}\\n\
               ]\\n\
             }}\\n\\n\
             Your response (JSON only):",
            task_description,
            self.template.name,
            self.template.capabilities,
            self.template.mcp_servers
        );
        
        let options = GenerationOptions {
            temperature: Some(0.3), // Lower temperature for more deterministic planning
            max_tokens: Some(1024),
            system: Some(system_prompt),
            ..Default::default()
        };
        
        let response = self.ollama_client.generate(
            "llama3.2:latest",
            &planning_prompt,
            options
        ).await.context("Failed to generate execution plan")?;
        
        self.parse_execution_plan(&response.text)
    }
    
    /// Parse LLM response into ExecutionPlan
    fn parse_execution_plan(&self, llm_response: &str) -> Result<ExecutionPlan> {
        let json_str = if let Some(start) = llm_response.find('{') {
            if let Some(end) = llm_response.rfind('}') {
                &llm_response[start..=end]
            } else {
                llm_response
            }
        } else {
            llm_response
        };
        
        let parsed: serde_json::Value = serde_json::from_str(json_str)
            .context(format!("Failed to parse execution plan JSON: {}", json_str))?;
        
        let steps: Vec<ExecutionStep> = parsed["steps"].as_array()
            .ok_or_else(|| anyhow::anyhow!("Missing 'steps' array"))?
            .iter()
            .filter_map(|s| {
                Some(ExecutionStep {
                    tool: s["tool"].as_str()?.to_string(),
                    params: s["params"].clone(),
                    description: s["description"].as_str().unwrap_or("").to_string(),
                })
            })
            .collect();
        
        if steps.is_empty() {
            return Err(anyhow::anyhow!("No valid execution steps found in plan"));
        }
        
        Ok(ExecutionPlan { steps })
    }
    
    /// Execute plan with retry logic
    async fn execute_with_retries(&self, plan: &ExecutionPlan) -> Result<Vec<String>> {
        let mut deliverables = Vec::new();
        
        for (idx, step) in plan.steps.iter().enumerate() {
            tracing::info!("Worker {} executing step {}/{}: {}", 
                           self.id.name, idx + 1, plan.steps.len(), step.description);
            
            let mut attempts = 0;
            let result = loop {
                attempts += 1;
                
                match self.execute_step(step).await {
                    Ok(result) => break result,
                    Err(e) if attempts < self.max_retries => {
                        tracing::warn!("Worker {} step failed (attempt {}/{}): {}",
                                       self.id.name, attempts, self.max_retries, e);
                        tokio::time::sleep(tokio::time::Duration::from_millis(500 * attempts as u64)).await;
                        continue;
                    }
                    Err(e) => {
                        return Err(anyhow::anyhow!(
                            "Step {} failed after {} attempts: {}", 
                            idx + 1, self.max_retries, e
                        ));
                    }
                }
            };
            
            deliverables.push(format!("Step {}: {} - {}", idx + 1, step.description, result));
        }
        
        Ok(deliverables)
    }
    
    /// Execute single step with MCP tool
    async fn execute_step(&self, step: &ExecutionStep) -> Result<String> {
        // Parse tool name: "server::tool_name"
        let parts: Vec<&str> = step.tool.split("::").collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid tool format: {}", step.tool));
        }
        
        let (server, tool) = (parts[0], parts[1]);
        
        let mcp_client = self.mcp_client.read().await;
        let result = mcp_client.call_tool(server, tool, step.params.clone()).await?;
        
        Ok(result.to_string())
    }
    
    /// Execute file-related task using hainet-files MCP server
    async fn execute_file_task(&self, task_description: &str) -> Result<Vec<String>> {
        let mcp_client = self.mcp_client.read().await;
        
        // Simple task parsing - in production this would use LLM
        // For now, support basic file operations
        if task_description.contains("read") || task_description.contains("get") {
            // Extract file path from task description
            // This is a simplified version - real implementation would use NLP
            let path = self.extract_path_from_task(task_description);
            
            let result = mcp_client.call_tool(
                "hainet-files",
                "hainet_file_read",
                json!({ "path": path })
            ).await?;
            
            Ok(vec![format!("Read file: {}", result)])
        } else if task_description.contains("write") || task_description.contains("create") {
            let path = self.extract_path_from_task(task_description);
            let content = "Generated content"; // Would be LLM-generated
            
            let result = mcp_client.call_tool(
                "hainet-files",
                "hainet_file_write",
                json!({ "path": path, "content": content })
            ).await?;
            
            Ok(vec![format!("Wrote file: {}", result)])
        } else if task_description.contains("list") {
            let path = self.extract_path_from_task(task_description);
            
            let result = mcp_client.call_tool(
                "hainet-files",
                "hainet_file_list",
                json!({ "path": path })
            ).await?;
            
            Ok(vec![format!("Listed directory: {}", result)])
        } else {
            // Default: assume read operation
            Ok(vec!["File operation completed".to_string()])
        }
    }
    
    /// Execute generic task - tries to auto-detect task type
    async fn execute_generic_task(&self, task_description: &str) -> Result<Vec<String>> {
        // Try file operations first
        if task_description.contains("file") || task_description.contains("directory") {
            return self.execute_file_task(task_description).await;
        }
        
        // Default fallback
        Ok(vec!["Generic task completed".to_string()])
    }
    
    /// Extract file path from task description (simplified NLP)
    fn extract_path_from_task(&self, task_description: &str) -> String {
        // Very simple path extraction - would use LLM in production
        // Look for common path patterns
        if let Some(start) = task_description.find("/") {
            // Find end of path (space or end of string)
            let remaining = &task_description[start..];
            if let Some(end) = remaining.find(" ") {
                remaining[..end].to_string()
            } else {
                remaining.to_string()
            }
        } else {
            // Default path for testing
            "/tmp/test.txt".to_string()
        }
    }
    
    /// Discover available tools from connected MCP servers
    pub async fn discover_tools(&self) -> Result<Vec<String>> {
        let mcp_client = self.mcp_client.read().await;
        let servers = mcp_client.list_servers().await;
        
        let mut all_tools = Vec::new();
        
        for server_name in servers {
            let tools = mcp_client.list_tools(&server_name).await?;
            for tool in tools {
                all_tools.push(format!("{}::{}", server_name, tool.name));
            }
        }
        
        Ok(all_tools)
    }
    
    /// Get reference to mcp_client (for testing)
    pub fn mcp_client(&self) -> &Arc<RwLock<MCPClientManager>> {
        &self.mcp_client
    }
    
    /// Get mutable reference to state machine (for testing)
    pub fn state_machine_mut(&mut self) -> &mut AgentStateMachine {
        &mut self.state_machine
    }
    
    /// Get reference to project manager (for testing)
    pub fn project_manager(&self) -> &Arc<RwLock<ProjectManager>> {
        &self.project_manager
    }
    
    /// Get reference to template (for testing)
    pub fn template(&self) -> &WorkerTemplate {
        &self.template
    }
}

#[async_trait::async_trait]
impl super::Agent for WorkerAgent {
    fn id(&self) -> &AgentId {
        &self.id
    }

    async fn start(&mut self) -> Result<()> {
        // Placeholder start logic
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        // Placeholder stop logic
        Ok(())
    }

    async fn process_message(&mut self, _message: crate::messaging::Message) -> Result<()> {
        // Placeholder message processing logic
        Ok(())
    }
}

/// Execution plan generated by LLM
#[derive(Debug, Clone)]
struct ExecutionPlan {
    steps: Vec<ExecutionStep>,
}

/// Single execution step
#[derive(Debug, Clone)]
struct ExecutionStep {
    tool: String,  // Format: "server::tool_name"
    params: serde_json::Value,
    description: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    async fn create_test_worker() -> WorkerAgent {
        let message_bus = Arc::new(RwLock::new(MessageBus::new().await.unwrap()));
        let prompt_manager = Arc::new(PromptManager::new("prompts".into()).unwrap());
        let project_manager = Arc::new(RwLock::new(
            ProjectManager::new("sqlite::memory:").await.unwrap()
        ));
        let mcp_client = Arc::new(RwLock::new(MCPClientManager::new()));
        
        WorkerAgent::new(
            WorkerType::Files, 
            message_bus, 
            prompt_manager, 
            project_manager,
            mcp_client
        )
    }
    
    #[tokio::test]
    async fn test_worker_creation() {
        let worker = create_test_worker().await;
        assert_eq!(worker.state(), &AgentState::Startup);
        assert_eq!(worker.worker_type(), &WorkerType::Files);
    }
    
    #[tokio::test]
    async fn test_worker_assign_task() {
        let mut worker = create_test_worker().await;
        
        // Transition to Idle first
        worker.state_machine.transition(AgentState::Idle, "Init".to_string()).unwrap();
        
        // Create a project and a task
        let task_id = {
            let pm_mgr = worker.project_manager.write().await;
            let project_id = pm_mgr.create_project(
                "Test Project".to_string(),
                "Test project for worker".to_string(),
                vec!["Task 1".to_string()],
            ).await.unwrap();
            let tasks = pm_mgr.get_project_tasks(&project_id).await.unwrap();
            tasks[0].id.clone()
        };
        
        // Assignment should succeed
        let result = worker.assign_task(task_id).await;
        assert!(result.is_ok());
        assert!(worker.current_task.is_some());
    }
}
