// START OF FILE hainet-persona/src/messaging/types.rs

//! Core message type definitions for HAI-Net agent communication
//!
//! This module defines all message types used in the hierarchical agent communication system.
//! Messages flow through strict hierarchies (User↔Admin↔PM↔Workers) and are monitored by
//! Constitutional Guardians for compliance.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;
use uuid::Uuid;

// Re-export AgentType from prompts module to avoid duplication
pub use crate::prompts::types::{AgentType, AgentState, PMDomain, WorkerType};

/// Unique identifier for a message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(Uuid);

impl MessageId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a task
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(Uuid);

impl TaskId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

/// Agent identifier with hierarchy tracking
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId {
    pub agent_type: AgentType,
    pub name: String,
    pub instance_id: Uuid,
    pub domain: Option<PMDomain>,       // For PM agents
    pub worker_type: Option<WorkerType>, // For Worker agents
}

impl AgentId {
    pub fn new(agent_type: AgentType, name: String) -> Self {
        Self {
            agent_type,
            name,
            instance_id: Uuid::new_v4(),
            domain: None,
            worker_type: None,
        }
    }

    pub fn user(name: String) -> Self {
        Self::new(AgentType::User, name)
    }

    pub fn new_admin(name: String) -> Self {
        Self {
            agent_type: AgentType::Admin,
            name,
            instance_id: Uuid::new_v4(),
            domain: None,
            worker_type: None,
        }
    }

    pub fn new_pm(name: String, domain: PMDomain) -> Self {
        Self {
            agent_type: AgentType::PM,
            name,
            instance_id: Uuid::new_v4(),
            domain: Some(domain),
            worker_type: None,
        }
    }

    pub fn new_worker(name: String, worker_type: WorkerType) -> Self {
        Self {
            agent_type: AgentType::Worker,
            name,
            instance_id: Uuid::new_v4(),
            domain: None,
            worker_type: Some(worker_type),
        }
    }

    /// Check if this agent can send to another agent (hierarchy enforcement)
    pub fn can_send_to(&self, other: &AgentId) -> bool {
        match (&self.agent_type, &other.agent_type) {
            // Admin can send to PMs
            (AgentType::Admin, AgentType::PM) => true,
            // PMs can send to Admin
            (AgentType::PM, AgentType::Admin) => true,
            // PMs can send to other PMs (coordination)
            (AgentType::PM, AgentType::PM) => true,
            // PMs can send to Workers
            (AgentType::PM, AgentType::Worker) => true,
            // Workers can send to their PM
            (AgentType::Worker, AgentType::PM) => true,
            // Workers can send to other Workers (peer collaboration)
            (AgentType::Worker, AgentType::Worker) => true,
            // All other combinations are invalid
            _ => false,
        }
    }
}

/// Priority levels for message routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
    Emergency = 5, // Guardian alerts, system errors
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

/// Channel types for routing validation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChannelType {
    UserToAdmin,
    AdminToUser,
    AdminToPM,
    PMToAdmin,
    PMToPM,
    PMToWorker,
    WorkerToPM,
    WorkerToWorker,
    GuardianMonitoring, // Special channel for Guardian alerts
    Invalid, // Invalid routing
}

impl ChannelType {
    /// Determine channel type from agent IDs
    pub fn from_agents(from: &AgentId, to: &AgentId) -> anyhow::Result<Self> {
        let channel_type = match (&from.agent_type, &to.agent_type) {
            (AgentType::User, AgentType::Admin) => ChannelType::UserToAdmin,
            (AgentType::Admin, AgentType::User) => ChannelType::AdminToUser,
            (AgentType::Admin, AgentType::PM) => ChannelType::AdminToPM,
            (AgentType::PM, AgentType::Admin) => ChannelType::PMToAdmin,
            (AgentType::PM, AgentType::PM) => ChannelType::PMToPM,
            (AgentType::PM, AgentType::Worker) => ChannelType::PMToWorker,
            (AgentType::Worker, AgentType::PM) => ChannelType::WorkerToPM,
            (AgentType::Worker, AgentType::Worker) => ChannelType::WorkerToWorker,
            _ => ChannelType::Invalid,
        };
        Ok(channel_type)
    }
}

/// Task definition for agent work
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub description: String,
    pub goals: Vec<String>,
    pub constraints: TaskConstraints,
    pub deadline: Option<SystemTime>,
    pub parent_task: Option<TaskId>,
    pub assigned_to: Option<AgentId>,
}

/// Constraints for task execution
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskConstraints {
    pub resource_tier: ResourceTier,
    pub max_cost_usd: Option<f64>,
    pub privacy_level: PrivacyLevel,
    pub requires_confirmation: bool,
}

/// Resource tier for task execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceTier {
    LocalOnly,       // Must use only local resources
    PreferLocal,     // Try local first
    MeshAllowed,     // Can use HAI-Net mesh
    ExternalAllowed, // Can use external services
}

/// Privacy level for data handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivacyLevel {
    NoData,       // Only use public APIs, send no personal data
    Anonymous,    // Send data but anonymized
    Pseudonymous, // Use HAI-Net DID, not real identity
    Personal,     // Can use real identity
}

/// Task execution result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: TaskId,
    pub success: bool,
    pub output: serde_json::Value,
    pub error: Option<String>,
    pub metrics: TaskMetrics,
}

/// Metrics for task execution
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskMetrics {
    pub duration_ms: u64,
    pub cost_usd: f64,
    pub resource_tier_used: ResourceTier,
    pub tokens_used: Option<u32>,
}

/// Coordination message between PM agents
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoordinationMsg {
    pub topic: String,
    pub request_type: CoordinationType,
    pub data: serde_json::Value,
}

/// Types of coordination between PMs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CoordinationType {
    ResourceRequest,  // Request resources from another PM's domain
    StatusQuery,      // Query status of another domain
    HandoffRequest,   // Hand off a task to another domain
    SyncRequest,      // Synchronize state
}

/// Status update from any agent
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusUpdate {
    pub agent_id: AgentId,
    pub state: AgentState,
    pub message: String,
    pub progress: Option<f32>, // 0.0 to 1.0
}

/// Alert from Guardian system
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alert {
    pub severity: AlertSeverity,
    pub category: AlertCategory,
    pub message: String,
    pub source_message_id: Option<MessageId>,
    pub recommended_action: String,
}

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Alert categories for constitutional compliance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertCategory {
    PrivacyViolation,    // PII leak, unauthorized data sharing
    BiasDetected,        // Biased or unfair content
    HarmRisk,            // Potential harm to user or others
    SecurityThreat,      // Security vulnerability
    ConstitutionalBreach, // General constitutional violation
}

/// Error report from any agent
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorReport {
    pub error_type: String,
    pub message: String,
    pub stack_trace: Option<String>,
    pub recoverable: bool,
}

/// Message content types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MessageContent {
    /// User input (from human)
    UserInput(String),
    
    /// Task assignment (PM → Worker)
    TaskAssignment(Task),
    
    /// Task result (Worker → PM)
    TaskResult(TaskResult),
    
    /// Coordination between PMs
    PMCoordination(CoordinationMsg),
    
    /// Status update
    StatusUpdate(StatusUpdate),
    
    /// Guardian alert
    GuardianAlert(Alert),
    
    /// Error report
    ErrorReport(ErrorReport),
    
    /// Generic query
    Query(String),
    
    /// Generic response
    Response(String),
}

/// Message context for tracking and monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageContext {
    pub conversation_id: Option<Uuid>,
    pub parent_message_id: Option<MessageId>,
    pub task_chain: Vec<TaskId>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Default for MessageContext {
    fn default() -> Self {
        Self {
            conversation_id: None,
            parent_message_id: None,
            task_chain: Vec::new(),
            metadata: HashMap::new(),
        }
    }
}

/// Message metadata for routing and monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub timestamp: SystemTime,
    pub priority: Priority,
    pub channel_type: ChannelType,
    pub requires_response: bool,
    pub timeout_ms: Option<u64>,
}

impl Default for MessageMetadata {
    fn default() -> Self {
        Self {
            timestamp: SystemTime::now(),
            priority: Priority::Normal,
            channel_type: ChannelType::AdminToUser, // placeholder, will be set based on agents
            requires_response: false,
            timeout_ms: None,
        }
    }
}

/// Complete message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub from: AgentId,
    pub to: AgentId,
    pub content: MessageContent,
    pub metadata: MessageMetadata,
    pub context: MessageContext,
    
    /// Guardian approval status
    pub guardian_approved: bool,
    
    /// Guardian scores (set by Guardian system)
    pub privacy_score: Option<f32>,
    pub bias_score: Option<f32>,
    pub harm_score: Option<f32>,
}

impl Message {
    /// Create a new message
    pub fn new(from: AgentId, to: AgentId, content: MessageContent) -> Self {
        let channel_type = Self::determine_channel_type(&from, &to);
        
        Self {
            id: MessageId::new(),
            from,
            to,
            content,
            metadata: MessageMetadata {
                channel_type,
                ..Default::default()
            },
            context: MessageContext::default(),
            guardian_approved: false,
            privacy_score: None,
            bias_score: None,
            harm_score: None,
        }
    }

    /// Create a message with specific priority
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.metadata.priority = priority;
        self
    }

    /// Create a message that requires a response
    pub fn requires_response(mut self, timeout_ms: u64) -> Self {
        self.metadata.requires_response = true;
        self.metadata.timeout_ms = Some(timeout_ms);
        self
    }

    /// Add parent message tracking
    pub fn with_parent(mut self, parent_id: MessageId) -> Self {
        self.context.parent_message_id = Some(parent_id);
        self
    }

    /// Add conversation tracking
    pub fn with_conversation(mut self, conversation_id: Uuid) -> Self {
        self.context.conversation_id = Some(conversation_id);
        self
    }

    /// Determine channel type based on sender and receiver
    fn determine_channel_type(from: &AgentId, to: &AgentId) -> ChannelType {
        match (&from.agent_type, &to.agent_type) {
            (AgentType::Admin, AgentType::PM) => ChannelType::AdminToPM,
            (AgentType::PM, AgentType::Admin) => ChannelType::PMToAdmin,
            (AgentType::PM, AgentType::PM) => ChannelType::PMToPM,
            (AgentType::PM, AgentType::Worker) => ChannelType::PMToWorker,
            (AgentType::Worker, AgentType::PM) => ChannelType::WorkerToPM,
            (AgentType::Worker, AgentType::Worker) => ChannelType::WorkerToWorker,
            _ => ChannelType::AdminToUser, // fallback
        }
    }

    /// Validate message can be sent based on hierarchy
    pub fn validate_hierarchy(&self) -> bool {
        self.from.can_send_to(&self.to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_id_generation() {
        let id1 = MessageId::new();
        let id2 = MessageId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_agent_hierarchy_validation() {
        let admin = AgentId::new_admin("admin".to_string());
        let pm = AgentId::new_pm("pm_comms".to_string(), PMDomain::Communications);
        let worker = AgentId::new_worker("worker_email".to_string(), WorkerType::Email);

        // Admin can send to PM
        assert!(admin.can_send_to(&pm));
        
        // PM can send to Admin
        assert!(pm.can_send_to(&admin));
        
        // PM can send to Worker
        assert!(pm.can_send_to(&worker));
        
        // Worker can send to PM
        assert!(worker.can_send_to(&pm));
        
        // Admin cannot send directly to Worker
        assert!(!admin.can_send_to(&worker));
        
        // Worker cannot send directly to Admin
        assert!(!worker.can_send_to(&admin));
    }

    #[test]
    fn test_message_creation() {
        let admin = AgentId::new_admin("admin".to_string());
        let pm = AgentId::new_pm("pm_comms".to_string(), PMDomain::Communications);
        
        let msg = Message::new(
            admin.clone(),
            pm.clone(),
            MessageContent::Query("Test query".to_string()),
        );

        assert_eq!(msg.from, admin);
        assert_eq!(msg.to, pm);
        assert_eq!(msg.metadata.channel_type, ChannelType::AdminToPM);
        assert!(!msg.guardian_approved);
    }

    #[test]
    fn test_message_hierarchy_validation() {
        let admin = AgentId::new_admin("admin".to_string());
        let pm = AgentId::new_pm("pm_comms".to_string(), PMDomain::Communications);
        
        let valid_msg = Message::new(
            admin.clone(),
            pm.clone(),
            MessageContent::Query("Test".to_string()),
        );
        assert!(valid_msg.validate_hierarchy());

        let worker = AgentId::new_worker("worker_email".to_string(), WorkerType::Email);
        let invalid_msg = Message::new(
            admin.clone(),
            worker.clone(),
            MessageContent::Query("Test".to_string()),
        );
        assert!(!invalid_msg.validate_hierarchy());
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Emergency > Priority::Critical);
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Normal);
        assert!(Priority::Normal > Priority::Low);
    }

    #[test]
    fn test_message_builder_pattern() {
        let admin = AgentId::new_admin("admin".to_string());
        let pm = AgentId::new_pm("pm_comms".to_string(), PMDomain::Communications);
        
        let msg = Message::new(
            admin,
            pm,
            MessageContent::Query("Test".to_string()),
        )
        .with_priority(Priority::High)
        .requires_response(30000)
        .with_conversation(Uuid::new_v4());

        assert_eq!(msg.metadata.priority, Priority::High);
        assert!(msg.metadata.requires_response);
        assert_eq!(msg.metadata.timeout_ms, Some(30000));
        assert!(msg.context.conversation_id.is_some());
    }
}
