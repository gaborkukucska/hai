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
use super::state::AgentStateMachine;

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
}

impl WorkerAgent {
    /// Create new worker agent
    pub fn new(
        worker_type: WorkerType,
        message_bus: Arc<RwLock<MessageBus>>,
        prompt_manager: Arc<PromptManager>,
        project_manager: Arc<RwLock<ProjectManager>>,
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
        let mut project_manager = self.project_manager.write().await;
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
        
        // TODO: Execute task using MCP tools
        // This will be implemented when MCP client is ready
        let deliverables = vec!["Task completed".to_string()];
        
        // Transition to Reporting
        self.state_machine.transition(
            AgentState::Reporting,
            "Task complete, reporting to PM".to_string()
        )?;
        
        // Submit task for review
        let mut project_manager = self.project_manager.write().await;
        project_manager.complete_task(task_id, deliverables).await?;
        
        Ok(())
    }
    
    /// Wait for PM validation
    pub async fn await_validation(&mut self) -> Result<bool> {
        let task_id = self.current_task.as_ref()
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
}

#[cfg(test)]
mod tests {
    use super::*;
    
    async fn create_test_worker() -> WorkerAgent {
        let message_bus = Arc::new(RwLock::new(MessageBus::new().await.unwrap()));
        let prompt_manager = Arc::new(PromptManager::new("hainet-persona/prompts".into()).unwrap());
        let project_manager = Arc::new(RwLock::new(
            ProjectManager::new("sqlite::memory:").await.unwrap()
        ));
        
        WorkerAgent::new(WorkerType::Files, message_bus, prompt_manager, project_manager)
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
        
        // Create a test task
        let task_id = TaskId::new();
        
        // Assignment should succeed
        let result = worker.assign_task(task_id).await;
        assert!(result.is_ok());
        assert!(worker.current_task.is_some());
    }
}
