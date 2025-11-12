//! # HAI-Net Agent System
//! 
//! Multi-agent AI intelligence implementing the hierarchical architecture:
//! - Admin AI: Primary user interface and orchestrator
//! - PM Agents: Specialized coordinators (Communications, Knowledge, System)
//! - Worker Agents: Task executors (Email, Search, Files, etc.)
//!
//! All agents communicate via the MessageBus and are monitored by Constitutional Guardians.

pub mod admin;
pub mod guardian;
pub mod intent;
pub mod planner;
pub mod state;
pub mod pm;
pub mod pm_intelligence;
pub mod worker;
pub mod worker_intelligence;
pub mod templates;
pub mod llm_config;
pub mod metrics;
pub mod session_tasks;

// Re-export core agent types
pub use admin::AdminAgent;
pub use guardian::{
    GuardianAgent, GuardianConfig, GuardianState, Article,
    ConstitutionalChecker, ComplianceContext, AuditReport,
    LearningReport, ComplianceReport,
};
pub use intent::IntentParser;
pub use planner::TaskPlanner;
pub use state::AgentStateMachine;
pub use pm::{PMAgent, TaskGraph, TaskDependency};
pub use pm_intelligence::{
    DecompositionStrategy, ProjectComplexity, ProjectOutcome,
    HistoricalLearner, TaskComplexityAnalyzer, DynamicTaskAdjuster,
};
pub use worker::WorkerAgent;
pub use worker_intelligence::{
    WorkerLearner, TaskOutcome, ExecutionStrategy, ToolSelector,
    ErrorCategory, SuccessMetrics,
};
pub use templates::WorkerTemplate;
pub use session_tasks::{SessionTaskList, SessionTask, TaskStatus as SessionTaskStatus, SessionTaskStats};

// Re-export configuration and metrics types
pub use llm_config::{
    AgentLLMConfig, AgentLLMConfigOverrides,
    ProviderPreference, ModelSize, Quantization,
};
pub use metrics::{
    AgentMetrics, OperationResult, MetricsCollector,
    hash_config,
};

use anyhow::Result;
use crate::messaging::{AgentId, MessageBus};
use crate::prompts::PromptManager;
use crate::tools::mcp::MCPClientManager;
use crate::guardian::GuardianSystem;
use std::sync::Arc;
use tokio::sync::RwLock;

// Re-export AgentType for convenience
pub use crate::prompts::AgentType;

/// Base trait for all agent types
#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    /// Get agent's unique identifier
    fn id(&self) -> &crate::messaging::AgentId;
    
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
