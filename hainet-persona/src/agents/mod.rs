//! # HAI-Net Agent System
//! 
//! Multi-agent AI intelligence implementing the hierarchical architecture:
//! - Admin AI: Primary user interface and orchestrator
//! - PM Agents: Specialized coordinators (Communications, Knowledge, System)
//! - Worker Agents: Task executors (Email, Search, Files, etc.)
//!
//! All agents communicate via the MessageBus and are monitored by Constitutional Guardians.

pub mod admin;
pub mod intent;
pub mod planner;
pub mod pm;
pub mod state;
pub mod worker;

// Re-export core agent types
pub use admin::AdminAgent;
pub use intent::{IntentParser, Intent, IntentType};
pub use planner::{TaskPlanner, TaskPlan, TaskStep};
pub use pm::PMAgent;
pub use state::{AgentStateMachine, StateTransition};
pub use worker::WorkerAgent;

use anyhow::Result;
use crate::messaging::{AgentId, MessageBus};
use crate::prompts::PromptManager;
use crate::tools::mcp::MCPClientManager;
use crate::guardian::GuardianSystem;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Base trait for all agent types
#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    /// Get agent's unique identifier
    fn id(&self) -> &AgentId;
    
    /// Process incoming message
    async fn process_message(&mut self, message: crate::messaging::Message) -> Result<()>;
    
    /// Start agent's main loop
    async fn start(&mut self) -> Result<()>;
    
    /// Stop agent gracefully
    async fn stop(&mut self) -> Result<()>;
}

/// Shared agent context available to all agents
pub struct AgentContext {
    /// Message bus for inter-agent communication
    pub message_bus: Arc<RwLock<MessageBus>>,
    
    /// Prompt management system
    pub prompt_manager: Arc<RwLock<PromptManager>>,
    
    /// MCP tool client
    pub mcp_client: Arc<RwLock<MCPClientManager>>,
    
    /// Constitutional Guardian system
    pub guardian: Arc<RwLock<GuardianSystem>>,
}

impl AgentContext {
    pub fn new(
        message_bus: Arc<RwLock<MessageBus>>,
        prompt_manager: Arc<RwLock<PromptManager>>,
        mcp_client: Arc<RwLock<MCPClientManager>>,
        guardian: Arc<RwLock<GuardianSystem>>,
    ) -> Self {
        Self {
            message_bus,
            prompt_manager,
            mcp_client,
            guardian,
        }
    }
}
