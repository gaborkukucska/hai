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
use crate::prompts::{PromptManager, AgentType, AgentState, WorkerType};
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
    
    /// Plan task execution using LLM (enhanced with better prompting)
    async fn plan_task_execution(&self, task_description: &str) -> Result<ExecutionPlan> {
        let planning_prompt = self.generate_planning_prompt(task_description);
        
        let options = GenerationOptions {
            temperature: Some(0.1), // More deterministic for execution planning
            max_tokens: Some(2048),
            system: Some(self.template.system_prompt.clone()),
            ..Default::default()
        };
        
        // Use gemma3:7b for worker task planning (as per Session 1)
        let response = self.ollama_client.generate(
            "gemma3:7b",
            &planning_prompt,
            options
        ).await.context("Failed to generate execution plan with LLM")?;
        
        self.parse_execution_plan(&response.text)
    }
    
    /// Generate structured planning prompt for LLM
    fn generate_planning_prompt(&self, task_description: &str) -> String {
        format!(
            r#"You are a Worker AI agent executing a task. Analyze this task and create a structured execution plan.

TASK: {}
YOUR ROLE: {}
YOUR CAPABILITIES: {:?}

AVAILABLE MCP TOOLS:
{}

INSTRUCTIONS:
1. Break the task into concrete, executable steps
2. Each step must use ONE MCP tool with specific parameters
3. Steps can have dependencies on previous steps
4. Be specific with file paths, parameters, and expected outputs

RESPOND WITH VALID JSON ONLY (no markdown, no explanations):
{{
  "steps": [
    {{
      "step_number": 1,
      "tool": "hainet-files::file_read",
      "params": {{ "path": "/path/to/file" }},
      "description": "Read configuration file",
      "depends_on": []
    }},
    {{
      "step_number": 2,
      "tool": "hainet-files::file_write",
      "params": {{ "path": "/output/file", "content": "result" }},
      "description": "Write processed output",
      "depends_on": [1]
    }}
  ]
}}

CRITICAL: Respond with ONLY the JSON object above. No markdown code blocks, no explanations.
"#,
            task_description,
            self.template.name,
            self.template.capabilities,
            self.format_available_tools()
        )
    }
    
    /// Format available MCP tools for prompt
    fn format_available_tools(&self) -> String {
        let mut tools_desc = String::new();
        
        for server in &self.template.mcp_servers {
            tools_desc.push_str(&format!("\n{} server:\n", server));
            
            match server.as_str() {
                "hainet-files" => {
                    tools_desc.push_str("  - file_read(path) - Read file contents\n");
                    tools_desc.push_str("  - file_write(path, content) - Write to file\n");
                    tools_desc.push_str("  - file_list(path) - List directory contents\n");
                    tools_desc.push_str("  - file_metadata(path) - Get file metadata\n");
                }
                "hainet-system" => {
                    tools_desc.push_str("  - system_status() - Get CPU, RAM, disk usage\n");
                    tools_desc.push_str("  - list_services() - List running services\n");
                    tools_desc.push_str("  - check_health() - Run health checks\n");
                }
                "hainet-dev" => {
                    tools_desc.push_str("  - git_status(repo_path) - Get git status\n");
                    tools_desc.push_str("  - git_diff(repo_path, file_path) - View changes\n");
                    tools_desc.push_str("  - cargo_build(package, release) - Build Rust project\n");
                    tools_desc.push_str("  - cargo_test(package, filter) - Run tests\n");
                    tools_desc.push_str("  - code_search(pattern, path) - Search codebase\n");
                }
                _ => {}
            }
        }
        
        tools_desc
    }
    
    /// Parse LLM response into ExecutionPlan (enhanced with multi-strategy parsing)
    fn parse_execution_plan(&self, llm_response: &str) -> Result<ExecutionPlan> {
        // Strategy 1: Direct JSON extraction (simple case)
        let json_str = self.extract_json_from_response(llm_response);
        
        // Strategy 2: Try direct parse
        let parsed = match serde_json::from_str::<serde_json::Value>(&json_str) {
            Ok(value) => value,
            Err(_) => {
                // Strategy 3: Try markdown extraction
                if let Ok(value) = self.extract_from_markdown(llm_response) {
                    value
                } else {
                    // Strategy 4: Try repair (braces/brackets)
                    self.repair_and_parse(&json_str)?
                }
            }
        };
        
        let steps: Vec<ExecutionStep> = parsed["steps"].as_array()
            .ok_or_else(|| anyhow::anyhow!("Missing 'steps' array in execution plan"))?
            .iter()
            .enumerate()
            .filter_map(|(idx, s)| {
                let _step_number = s["step_number"].as_u64().unwrap_or((idx + 1) as u64) as usize;
                let tool = s["tool"].as_str()?.to_string();
                let params = s["params"].clone();
                let description = s["description"].as_str().unwrap_or("").to_string();
                
                // Validate tool format (server::tool_name)
                if !tool.contains("::") {
                    tracing::warn!("Invalid tool format (missing ::): {}", tool);
                    return None;
                }
                
                Some(ExecutionStep {
                    tool,
                    params,
                    description,
                })
            })
            .collect();
        
        if steps.is_empty() {
            return Err(anyhow::anyhow!(
                "No valid execution steps found in plan. Response: {}",
                &llm_response[..llm_response.len().min(200)]
            ));
        }
        
        tracing::debug!("Parsed {} execution steps from LLM response", steps.len());
        Ok(ExecutionPlan { steps })
    }
    
    /// Extract JSON from LLM response (handles braces)
    fn extract_json_from_response(&self, response: &str) -> String {
        if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                return response[start..=end].to_string();
            }
        }
        response.to_string()
    }
    
    /// Extract JSON from markdown code blocks
    fn extract_from_markdown(&self, text: &str) -> Result<serde_json::Value> {
        let markers = ["```json\n", "```\n", "```"];
        
        for marker in markers.iter() {
            if let Some(start_idx) = text.find(marker) {
                let json_start = start_idx + marker.len();
                
                if let Some(end_idx) = text[json_start..].find("```") {
                    let json_text = &text[json_start..json_start + end_idx];
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_text.trim()) {
                        return Ok(value);
                    }
                }
            }
        }
        
        Err(anyhow::anyhow!("No valid JSON found in markdown blocks"))
    }
    
    /// Repair common JSON issues (missing braces/brackets)
    fn repair_and_parse(&self, text: &str) -> Result<serde_json::Value> {
        let mut repaired = text.trim().to_string();
        
        // Count braces and brackets
        let open_braces = repaired.matches('{').count();
        let close_braces = repaired.matches('}').count();
        let open_brackets = repaired.matches('[').count();
        let close_brackets = repaired.matches(']').count();
        
        // Add missing closing braces
        if open_braces > close_braces {
            for _ in 0..(open_braces - close_braces) {
                repaired.push('}');
            }
        }
        
        // Add missing closing brackets
        if open_brackets > close_brackets {
            for _ in 0..(open_brackets - close_brackets) {
                repaired.push(']');
            }
        }
        
        serde_json::from_str::<serde_json::Value>(&repaired)
            .context("Failed to parse after JSON repair")
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
