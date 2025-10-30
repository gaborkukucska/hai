// START OF FILE hainet-persona/src/prompts/types.rs

//! Type definitions for the prompt management system

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Agent type hierarchy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentType {
    User,     // Human user (sovereign)
    Admin,    // Top-level orchestrator
    PM,       // Project Manager for a domain
    Worker,   // Specialized task executor
    Guardian, // Constitutional oversight and optimization
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentType::User => write!(f, "User"),
            AgentType::Admin => write!(f, "Admin"),
            AgentType::PM => write!(f, "PM"),
            AgentType::Worker => write!(f, "Worker"),
            AgentType::Guardian => write!(f, "Guardian"),
        }
    }
}

/// Agent states (state machine)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentState {
    Startup,      // Initializing
    Idle,         // Ready, waiting for work (Worker agents)
    Conversation, // Admin AI casual interaction with user
    Planning,     // Figuring out how to do something
    Monitoring,   // Admin AI monitoring active projects
    Managing,     // PM agent managing project execution
    Working,      // Actively executing tasks (Worker agents)
    Reporting,    // Worker reporting results to PM
    Error,        // Something went wrong
}

/// Domains for PM agents
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PMDomain {
    Communications, // Email, chat, calls
    Knowledge,      // Learning, research, memory
    System,         // Hub operations, resources
}

/// Specific worker types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkerType {
    // Communications workers
    Email,
    Chat,
    Call,
    Social,

    // Knowledge workers
    Search,
    Research,
    Tutor,
    Memory,

    // System workers
    Files,
    Network,
    Compute,
    Monitor,
}

/// Agent identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId {
    pub agent_type: AgentType,
    pub name: String,
    pub instance_id: Uuid,
}

impl AgentId {
    pub fn new(agent_type: AgentType, name: String) -> Self {
        Self {
            agent_type,
            name,
            instance_id: Uuid::new_v4(),
        }
    }
}

/// Prompt template loaded from TOML
#[derive(Debug, Clone, Deserialize)]
pub struct PromptTemplate {
    pub metadata: PromptMetadata,
    pub base_prompt: Option<BasePrompt>,
    pub states: Option<HashMap<String, StatePrompt>>,
    pub injection_points: Option<HashMap<String, String>>,
    pub personality: Option<PersonalityConfig>,
}

/// Metadata about a prompt template
#[derive(Debug, Clone, Deserialize)]
pub struct PromptMetadata {
    pub version: String,
    pub description: String,
    pub agent_type: Option<String>,
    pub state: Option<String>,
    pub constitutional_compliance: Option<bool>,
    pub model_requirements: Option<ModelRequirements>,
}

/// Base prompt configuration
#[derive(Debug, Clone, Deserialize)]
pub struct BasePrompt {
    pub system: String,
}

/// State-specific prompt
#[derive(Debug, Clone, Deserialize)]
pub struct StatePrompt {
    pub prompt: String,
}

/// Model requirements
#[derive(Debug, Clone, Deserialize)]
pub struct ModelRequirements {
    pub min_params: Option<String>,
    pub preferred_params: Option<String>,
    pub context_length: Option<u32>,
}

/// Personality configuration
#[derive(Debug, Clone, Deserialize)]
pub struct PersonalityConfig {
    pub communication_style: Option<String>,
    pub response_length: Option<String>,
    pub technical_level: Option<String>,
    pub proactivity: Option<String>,
}

/// Context for prompt rendering
#[derive(Debug, Clone, Serialize)]
pub struct PromptContext {
    // User context
    pub user_name: String,
    pub user_did: String,
    pub persona_name: String,

    // System context
    pub hub_name: String,
    pub device_count: u32,
    pub mesh_status: String,
    pub external_enabled: bool,

    // Task context
    pub current_request: Option<String>,
    pub current_task: Option<String>,
    pub task_analysis: Option<String>,
    
    // Agent context
    pub active_agents: Vec<String>,
    pub progress_updates: Vec<String>,
    
    // Constitutional context
    pub constitutional_compliance_prompt: String,
    pub guardian_status: String,

    // Dynamic variables
    pub variables: HashMap<String, serde_json::Value>,
}

impl Default for PromptContext {
    fn default() -> Self {
        Self {
            user_name: "User".to_string(),
            user_did: "did:hai:unknown".to_string(),
            persona_name: "HAI-Assistant".to_string(),
            hub_name: "Local Hub".to_string(),
            device_count: 1,
            mesh_status: "Offline".to_string(),
            external_enabled: false,
            current_request: None,
            current_task: None,
            task_analysis: None,
            active_agents: Vec::new(),
            progress_updates: Vec::new(),
            constitutional_compliance_prompt: "Constitutional compliance active.".to_string(),
            guardian_status: "Active".to_string(),
            variables: HashMap::new(),
        }
    }
}

/// Cache key for prompt caching
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PromptCacheKey {
    pub agent_id: AgentId,
    pub state: AgentState,
    pub context_hash: u64,
}

impl PromptCacheKey {
    pub fn new(agent_id: &AgentId, state: AgentState, context: &PromptContext) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        context.user_name.hash(&mut hasher);
        context.current_request.hash(&mut hasher);
        context.current_task.hash(&mut hasher);
        
        Self {
            agent_id: agent_id.clone(),
            state,
            context_hash: hasher.finish(),
        }
    }
}

/// Validation report for prompt templates
#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub total_templates: usize,
    pub valid_templates: usize,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

/// Validation error
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub file_path: String,
    pub error_type: String,
    pub message: String,
}

/// Validation warning
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub file_path: String,
    pub warning_type: String,
    pub message: String,
}

/// Priority levels for agent messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
    Emergency = 5,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}
