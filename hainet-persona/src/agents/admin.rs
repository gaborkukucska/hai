//! # Admin AI Agent
//! 
//! Primary user-facing agent that orchestrates all task execution.
//! Implements the hierarchical agent architecture's top layer.

use anyhow::Result;
use std::sync::Arc;
use super::{Agent, AgentContext, IntentParser, TaskPlanner, AgentStateMachine};
use crate::messaging::{AgentId, Message};
use crate::prompts::{AgentType, AgentState};

/// Admin AI - Primary user interface and orchestrator
pub struct AdminAgent {
    /// Agent identifier
    id: AgentId,
    
    /// Shared context with other agents
    context: Arc<AgentContext>,
    
    /// Intent parser for understanding user requests
    intent_parser: IntentParser,
    
    /// Task planner for breaking down requests
    task_planner: TaskPlanner,
    
    /// State machine managing agent lifecycle
    state_machine: AgentStateMachine,
    
    /// Running flag
    running: bool,
}

impl AdminAgent {
    /// Create new Admin AI agent
    pub fn new(context: Arc<AgentContext>) -> Self {
        let id = AgentId::new(AgentType::Admin, "main-admin".to_string());
        
        Self {
            id,
            context,
            intent_parser: IntentParser::new(),
            task_planner: TaskPlanner::new(),
            state_machine: AgentStateMachine::new(),
            running: false,
        }
    }
    
    /// Process user input and create task plan
    pub async fn process_user_input(&mut self, user_input: String) -> Result<String> {
        // TODO: Full implementation in next iteration
        // 1. Parse intent
        // 2. Create task plan
        // 3. Delegate to PM agents
        // 4. Monitor execution
        // 5. Return results to user
        
        Ok(format!("Admin AI received: {}", user_input))
    }
}

#[async_trait::async_trait]
impl Agent for AdminAgent {
    fn id(&self) -> &AgentId {
        &self.id
    }
    
    async fn process_message(&mut self, _message: Message) -> Result<()> {
        // TODO: Implement message processing
        Ok(())
    }
    
    async fn start(&mut self) -> Result<()> {
        self.running = true;
        self.state_machine.transition(AgentState::Idle, "Agent started".to_string())?;
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<()> {
        self.running = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::MessageBus;
    use crate::prompts::PromptManager;
    use crate::tools::mcp::MCPClientManager;
    use crate::guardian::GuardianSystem;
    
    async fn create_test_context() -> Arc<AgentContext> {
        use tokio::sync::RwLock;
        use std::path::PathBuf;
        
        // Use absolute path to prompts directory
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let prompts_path = PathBuf::from(manifest_dir).join("prompts");
        
        Arc::new(AgentContext::new(
            Arc::new(RwLock::new(MessageBus::new().await.expect("Failed to create MessageBus"))),
            Arc::new(RwLock::new(PromptManager::new(prompts_path).unwrap())),
            Arc::new(RwLock::new(MCPClientManager::new())),
            Arc::new(RwLock::new(GuardianSystem::new(None, None))),
        ))
    }
    
    #[tokio::test]
    async fn test_admin_agent_creation() {
        let context = create_test_context().await;
        let agent = AdminAgent::new(context);
        
        assert_eq!(agent.id().agent_type, AgentType::Admin);
        assert!(!agent.running);
    }
    
    #[tokio::test]
    async fn test_admin_agent_start() {
        let context = create_test_context().await;
        let mut agent = AdminAgent::new(context);
        
        agent.start().await.unwrap();
        assert!(agent.running);
        assert_eq!(agent.state_machine.current_state(), &AgentState::Idle);
    }
}
