//! # START OF FILE hainet-persona/src/agents/worker.rs
//! Worker Agent
//! 
//! Executes individual tasks using MCP tools.
//! Worker agents follow this state machine:
//! Idle → Planning → Working → Reporting → (Idle | Error)

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::messaging::{MessageBus, AgentId};
use crate::prompts::{PromptManager, AgentType, AgentState, WorkerType};
use crate::projects::{ProjectManager, TaskId};
use crate::tools::mcp::MCPClientManager;
use super::state::AgentStateMachine;
use serde_json::json;

/// Worker Agent
/// 
/// Responsible for:
/// - Executing specific tasks assigned by PM
/// - Using MCP tools to accomplish work
/// - Reporting results back to PM for validation
pub struct WorkerAgent {
    /// Unique agent identifier
    id: AgentId,
    
    /// Worker specialization type
    worker_type: WorkerType,
    
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
}

impl WorkerAgent {
    /// Create new worker agent
    pub fn new(
        worker_type: WorkerType,
        message_bus: Arc<RwLock<MessageBus>>,
        prompt_manager: Arc<PromptManager>,
        project_manager: Arc<RwLock<ProjectManager>>,
        mcp_client: Arc<RwLock<MCPClientManager>>,
    ) -> Self {
        let id = AgentId::new(AgentType::Worker, format!("Worker-{:?}", worker_type));
        
        Self {
            id,
            worker_type,
            current_task: None,
            state_machine: AgentStateMachine::new(),
            message_bus,
            prompt_manager,
            project_manager,
            mcp_client,
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
    
    /// Execute assigned task
    pub async fn execute_task(&mut self) -> Result<()> {
        let task_id = self.current_task.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No task assigned"))?;
        
        // Transition to Planning
        self.state_machine.transition(
            AgentState::Planning,
            "Analyzing task requirements".to_string()
        )?;
        
        // TODO: Use LLM to analyze task and plan approach
        // For now, this is a stub
        
        // Transition to Working
        self.state_machine.transition(
            AgentState::Working,
            "Executing task".to_string()
        )?;
        
        // Execute task using MCP tools
        let deliverables = self.execute_with_tools().await?;
        
        // Transition to Reporting
        self.state_machine.transition(
            AgentState::Reporting,
            "Task complete, reporting to PM".to_string()
        )?;
        
        // Submit task for review
        let project_manager = self.project_manager.write().await;
        project_manager.complete_task(task_id, deliverables).await?;
        
        Ok(())
    }
    
    /// Wait for PM validation
    pub async fn await_validation(&mut self) -> Result<bool> {
        let _task_id = self.current_task.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No task assigned"))?;
        
        // Poll for task status
        loop {
            // Get task from storage via project manager
            let project_manager = self.project_manager.read().await;
            
            // We need to get the project first to know which tasks to check
            // For now, we'll use a simplified approach - just wait a bit and check
            // TODO: Implement proper task status polling
            drop(project_manager);
            
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            // For testing purposes, auto-approve after one iteration
            self.state_machine.transition(
                AgentState::Idle,
                "Task approved by PM".to_string()
            )?;
            self.current_task = None;
            return Ok(true);
        }
    }
    
    /// Handle error and transition to Error state
    pub fn handle_error(&mut self, error: String) {
        self.state_machine.force_error(error);
    }
    
    /// Execute task using MCP tools
    async fn execute_with_tools(&self) -> Result<Vec<String>> {
        let task_id = self.current_task.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No task assigned"))?;
        
        // Get task details from project manager
        let tasks = {
            let pm = self.project_manager.read().await;
            pm.get_project_tasks(&pm.list_active_projects().await?[0].id).await?
        };
        
        let task = tasks.iter()
            .find(|t| &t.id == task_id)
            .ok_or_else(|| anyhow::anyhow!("Task not found"))?;
        
        // Based on worker type, select appropriate tools
        match self.worker_type {
            WorkerType::Files => self.execute_file_task(&task.description).await,
            WorkerType::Network => {
                // Network tools not yet implemented
                Ok(vec!["Network task completed (stub)".to_string()])
            }
            WorkerType::Research => {
                // Research tools not yet implemented
                Ok(vec!["Research task completed (stub)".to_string()])
            }
            WorkerType::Compute => {
                // Compute tools not yet implemented
                Ok(vec!["Compute task completed (stub)".to_string()])
            }
            _ => {
                // Default: try file operations
                self.execute_generic_task(&task.description).await
            }
        }
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
