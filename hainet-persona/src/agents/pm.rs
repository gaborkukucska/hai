//! # START OF FILE hainet-persona/src/agents/pm.rs
//! Project Manager Agent
//! 
//! Manages a single project, coordinating worker agents and ensuring task completion.
//! PM agents follow this state machine:
//! Startup → Planning → Managing → (Idle | Error)

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::messaging::{MessageBus, AgentId};
use crate::prompts::{PromptManager, AgentType, AgentState};
use crate::projects::{ProjectManager, ProjectId, TaskId};
use super::state::AgentStateMachine;

/// Project Manager Agent
/// 
/// Responsible for:
/// - Breaking down project into detailed tasks
/// - Creating and managing worker agents
/// - Validating worker outputs
/// - Reporting progress to Admin AI
pub struct PMAgent {
    /// Unique agent identifier
    id: AgentId,
    
    /// Project this PM is managing
    project_id: ProjectId,
    
    /// State machine
    state_machine: AgentStateMachine,
    
    /// Message bus for communication
    message_bus: Arc<RwLock<MessageBus>>,
    
    /// Prompt manager for generating prompts
    prompt_manager: Arc<RwLock<PromptManager>>,
    
    /// Project manager for data persistence
    project_manager: Arc<RwLock<ProjectManager>>,
}

impl PMAgent {
    /// Create new PM agent for a project
    pub fn new(
        project_id: ProjectId,
        message_bus: Arc<RwLock<MessageBus>>,
        prompt_manager: Arc<RwLock<PromptManager>>,
        project_manager: Arc<RwLock<ProjectManager>>,
    ) -> Self {
        let id = AgentId::new(AgentType::PM, format!("PM-{}", project_id));
        
        Self {
            id,
            project_id,
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
    
    /// Start PM agent lifecycle
    /// 
    /// Startup → Planning → Managing
    pub async fn start(&mut self) -> Result<()> {
        // Transition from Startup to Planning
        self.state_machine.transition(
            AgentState::Planning,
            "PM initialized, analyzing project".to_string()
        )?;
        
        // Analyze project and create detailed plan
        self.analyze_and_plan().await?;
        
        // Transition to Managing
        self.state_machine.transition(
            AgentState::Managing,
            "Plan complete, starting execution".to_string()
        )?;
        
        Ok(())
    }
    
    /// Analyze project requirements and create detailed execution plan
    async fn analyze_and_plan(&mut self) -> Result<()> {
        let project_manager = self.project_manager.read().await;
        let project = project_manager.get_project(&self.project_id).await?
            .ok_or_else(|| anyhow::anyhow!("Project not found"))?;
        
        // TODO: Use LLM to analyze project and break down into detailed tasks
        // For now, this is a stub that will be enhanced in Phase 1.3
        
        // Create initial milestones if none exist
        if project.milestone_ids.is_empty() {
            drop(project_manager); // Release read lock
            let mut project_manager = self.project_manager.write().await;
            
            project_manager.create_milestone(
                &self.project_id,
                "Initial Milestone".to_string(),
                "Complete initial project tasks".to_string(),
                None,
            ).await?;
        }
        
        Ok(())
    }
    
    /// Main managing loop
    /// 
    /// Assigns tasks to workers, monitors progress, validates results
    pub async fn manage_loop(&mut self) -> Result<()> {
        loop {
            // Check if we're still in Managing state
            if !matches!(self.state_machine.current_state(), AgentState::Managing) {
                break;
            }
            
            // Get unassigned tasks
            let unassigned_tasks = self.get_unassigned_tasks().await?;
            
            // Assign tasks to available workers
            for task_id in unassigned_tasks {
                // TODO: Find available worker and assign task
                // This will be implemented when we have worker agents
                let _ = task_id;
            }
            
            // Check for completed tasks needing validation
            let tasks_under_review = self.get_tasks_under_review().await?;
            
            for task_id in tasks_under_review {
                self.validate_task(&task_id).await?;
            }
            
            // Check if all tasks are complete
            if self.is_project_complete().await? {
                self.complete_project().await?;
                break;
            }
            
            // Sleep briefly before next iteration
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        Ok(())
    }
    
    /// Get unassigned tasks from project
    async fn get_unassigned_tasks(&self) -> Result<Vec<TaskId>> {
        let project_manager = self.project_manager.read().await;
        let tasks = project_manager.get_project_tasks(&self.project_id).await?;
        
        Ok(tasks.into_iter()
            .filter(|task| matches!(task.status, crate::projects::TaskStatus::Unassigned))
            .map(|task| task.id)
            .collect())
    }
    
    /// Get tasks under review (submitted by workers)
    async fn get_tasks_under_review(&self) -> Result<Vec<TaskId>> {
        let project_manager = self.project_manager.read().await;
        let tasks = project_manager.get_project_tasks(&self.project_id).await?;
        
        Ok(tasks.into_iter()
            .filter(|task| matches!(task.status, crate::projects::TaskStatus::UnderReview))
            .map(|task| task.id)
            .collect())
    }
    
    /// Validate task results submitted by worker
    async fn validate_task(&self, task_id: &TaskId) -> Result<()> {
        // TODO: Implement actual validation logic using LLM
        // For now, auto-approve tasks
        
        let project_manager = self.project_manager.read().await;
        project_manager.approve_task(
            task_id,
            "Validated by PM".to_string()
        ).await?;
        
        Ok(())
    }
    
    /// Check if project is complete
    async fn is_project_complete(&self) -> Result<bool> {
        let project_manager = self.project_manager.read().await;
        let tasks = project_manager.get_project_tasks(&self.project_id).await?;
        
        Ok(tasks.iter().all(|task| 
            matches!(task.status, crate::projects::TaskStatus::Complete)
        ))
    }
    
    /// Complete project and transition to Idle
    async fn complete_project(&mut self) -> Result<()> {
        let mut project_manager = self.project_manager.write().await;
        project_manager.complete_project(&self.project_id).await?;
        
        self.state_machine.transition(
            AgentState::Idle,
            "Project completed successfully".to_string()
        )?;
        
        Ok(())
    }
    
    /// Handle error and transition to Error state
    pub fn handle_error(&mut self, error: String) {
        self.state_machine.force_error(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    async fn create_test_pm() -> PMAgent {
        let message_bus = Arc::new(RwLock::new(MessageBus::new().await.unwrap()));
        let prompt_manager = Arc::new(RwLock::new(PromptManager::new("hainet-persona/prompts".into()).unwrap()));
        let project_manager = Arc::new(RwLock::new(
            ProjectManager::new("sqlite::memory:").await.unwrap()
        ));
        
        let project_id = ProjectId::new();
        PMAgent::new(project_id, message_bus, prompt_manager, project_manager)
    }
    
    #[tokio::test]
    async fn test_pm_creation() {
        let pm = create_test_pm().await;
        assert_eq!(pm.state(), &AgentState::Startup);
    }
    
    #[tokio::test]
    async fn test_pm_startup_transition() {
        let mut pm = create_test_pm().await;
        
        // Create project first
        {
            let pm_mgr = pm.project_manager.write().await;
            pm_mgr.create_project(
                "Test Project".to_string(),
                "Test project for PM agent".to_string(),
                vec!["Task 1".to_string()],
            ).await.unwrap();
        }
        
        pm.start().await.unwrap();
        assert_eq!(pm.state(), &AgentState::Managing);
    }
}
