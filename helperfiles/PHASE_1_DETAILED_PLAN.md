//! # START OF FILE helperfiles/PHASE_1_DETAILED_PLAN.md
# Phase 1: Project-Based Agentic System - Detailed Implementation Plan

**Version:** 1.0  
**Date:** 2025-10-22  
**Status:** Ready to Implement  
**Architecture Reference:** `PROJECT_BASED_AGENTIC_SYSTEM.md`

---

## User-Confirmed Architectural Decisions

Based on feedback received 2025-10-22:

1. **Agent Lifecycle:** Agents hibernate after project completion, deleted only when project is deleted
2. **LLM Integration:** Option C (Both) - Direct calls for simple tasks, MCP for complex reasoning
3. **Project Storage:** Option B - SQLite database for persistent across restarts
4. **Worker Specializations:** Default worker templates with PM-customizable system prompts

---

## Implementation Phases

### **Phase 1.1: Project Management Infrastructure** (~60K tokens, 2 sessions)

**Goal:** Create project entity system with SQLite persistence, support multi-project parallel execution

#### Files to Create

**1. `hainet-persona/src/projects/mod.rs`** (~100 LOC)
```rust
// Module exports and high-level ProjectManager initialization
pub mod project;
pub mod task;
pub mod milestone;
pub mod manager;
pub mod storage;

pub use project::{Project, ProjectId, ProjectStatus};
pub use task::{Task, TaskId, TaskStatus};
pub use milestone::{Milestone, MilestoneId, MilestoneStatus};
pub use manager::ProjectManager;
```

**2. `hainet-persona/src/projects/project.rs`** (~350 LOC)
```rust
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectId(Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub title: String,
    pub overview: String,
    pub status: ProjectStatus,
    
    // Agents (hibernate when project completes)
    pub pm_agent_id: Option<AgentId>,
    pub worker_agent_ids: Vec<AgentId>,
    
    // Tasks & Milestones
    pub milestones: Vec<MilestoneId>,
    pub tasks: Vec<TaskId>,
    
    // Lifecycle
    pub created_at: SystemTime,
    pub started_at: Option<SystemTime>,
    pub completed_at: Option<SystemTime>,
    pub deleted_at: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectStatus {
    Created,      // Admin created, PM not assigned
    Active,       // PM managing, workers executing
    Paused,       // User paused
    Completed,    // Finished, agents hibernated
    Failed,       // Error state
    Cancelled,    // User cancelled
}

impl Project {
    pub fn new(title: String, overview: String) -> Self;
    pub fn assign_pm(&mut self, pm_id: AgentId);
    pub fn add_worker(&mut self, worker_id: AgentId);
    pub fn pause(&mut self);
    pub fn resume(&mut self);
    pub fn complete(&mut self);
    pub fn fail(&mut self, reason: String);
}
```

**3. `hainet-persona/src/projects/task.rs`** (~300 LOC)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub project_id: ProjectId,
    pub title: String,
    pub description: String,
    
    // Assignment
    pub assigned_worker: Option<AgentId>,
    pub dependencies: Vec<TaskId>,
    
    // Status
    pub status: TaskStatus,
    pub deliverables: Vec<String>,
    pub validation_notes: Option<String>,
    
    // Lifecycle
    pub created_at: SystemTime,
    pub assigned_at: Option<SystemTime>,
    pub started_at: Option<SystemTime>,
    pub completed_at: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Unassigned,
    Assigned,
    InProgress,
    Blocked,
    UnderReview,  // PM validating
    Complete,
    Failed,
}

impl Task {
    pub fn assign_to(&mut self, worker_id: AgentId);
    pub fn start(&mut self);
    pub fn block(&mut self, reason: String);
    pub fn submit_for_review(&mut self, deliverables: Vec<String>);
    pub fn approve(&mut self, notes: String);
    pub fn reject(&mut self, reason: String);
}
```

**4. `hainet-persona/src/projects/milestone.rs`** (~250 LOC)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub id: MilestoneId,
    pub project_id: ProjectId,
    pub title: String,
    pub description: String,
    pub deadline: Option<SystemTime>,
    pub task_ids: Vec<TaskId>,
    pub status: MilestoneStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MilestoneStatus {
    NotStarted,
    InProgress,
    Complete,
    Delayed,
}

impl Milestone {
    pub fn progress(&self, tasks: &[Task]) -> f64;  // % complete
    pub fn is_complete(&self, tasks: &[Task]) -> bool;
}
```

**5. `hainet-persona/src/projects/storage.rs`** (~400 LOC)
```rust
use sqlx::{SqlitePool, Row};

pub struct ProjectStorage {
    pool: SqlitePool,
}

impl ProjectStorage {
    pub async fn new(db_path: &str) -> Result<Self>;
    pub async fn create_tables(&self) -> Result<()>;
    
    // Project CRUD
    pub async fn create_project(&self, project: &Project) -> Result<()>;
    pub async fn get_project(&self, id: ProjectId) -> Result<Option<Project>>;
    pub async fn update_project(&self, project: &Project) -> Result<()>;
    pub async fn delete_project(&self, id: ProjectId) -> Result<()>;
    pub async fn list_active_projects(&self) -> Result<Vec<Project>>;
    
    // Task CRUD
    pub async fn create_task(&self, task: &Task) -> Result<()>;
    pub async fn get_task(&self, id: TaskId) -> Result<Option<Task>>;
    pub async fn update_task(&self, task: &Task) -> Result<()>;
    pub async fn list_project_tasks(&self, project_id: ProjectId) -> Result<Vec<Task>>;
    
    // Milestone CRUD
    pub async fn create_milestone(&self, milestone: &Milestone) -> Result<()>;
    pub async fn get_milestone(&self, id: MilestoneId) -> Result<Option<Milestone>>;
    pub async fn update_milestone(&self, milestone: &Milestone) -> Result<()>;
    pub async fn list_project_milestones(&self, project_id: ProjectId) -> Result<Vec<Milestone>>;
}

// SQL Schema
const CREATE_PROJECTS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    overview TEXT NOT NULL,
    status TEXT NOT NULL,
    pm_agent_id TEXT,
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER,
    deleted_at INTEGER
);
"#;

const CREATE_TASKS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    assigned_worker TEXT,
    status TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);
"#;
```

**6. `hainet-persona/src/projects/manager.rs`** (~450 LOC)
```rust
use tokio::sync::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ProjectManager {
    storage: Arc<ProjectStorage>,
    active_projects: Arc<RwLock<HashMap<ProjectId, Project>>>,
    hibernated_agents: Arc<RwLock<HashMap<AgentId, HibernatedAgent>>>,
}

struct HibernatedAgent {
    agent_id: AgentId,
    project_id: ProjectId,
    agent_type: AgentType,
    system_prompt: String,
    hibernated_at: SystemTime,
}

impl ProjectManager {
    pub async fn new(db_path: &str) -> Result<Self>;
    
    // Project lifecycle
    pub async fn create_project(&self, title: String, overview: String, initial_tasks: Vec<String>) -> Result<ProjectId>;
    pub async fn assign_pm(&self, project_id: ProjectId, pm_id: AgentId) -> Result<()>;
    pub async fn complete_project(&self, project_id: ProjectId) -> Result<()>;
    pub async fn delete_project(&self, project_id: ProjectId) -> Result<()>;
    
    // Agent hibernation
    pub async fn hibernate_agent(&self, agent_id: AgentId, project_id: ProjectId, system_prompt: String) -> Result<()>;
    pub async fn wake_agent(&self, agent_id: AgentId) -> Result<String>;  // Returns system prompt
    pub async fn cleanup_hibernated_agents(&self, project_id: ProjectId) -> Result<()>;
    
    // Task management
    pub async fn create_task(&self, project_id: ProjectId, task: Task) -> Result<TaskId>;
    pub async fn assign_task(&self, task_id: TaskId, worker_id: AgentId) -> Result<()>;
    pub async fn complete_task(&self, task_id: TaskId, deliverables: Vec<String>) -> Result<()>;
    
    // Queries
    pub async fn get_project(&self, id: ProjectId) -> Result<Option<Project>>;
    pub async fn list_active_projects(&self) -> Result<Vec<Project>>;
    pub async fn get_project_tasks(&self, project_id: ProjectId) -> Result<Vec<Task>>;
}
```

#### Dependencies to Add

```toml
# hainet-persona/Cargo.toml
[dependencies]
sqlx = { version = "0.7", features = ["runtime-tokio-native-tls", "sqlite", "chrono"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
```

#### Tests

1. Project creation and lifecycle transitions
2. Task assignment and completion
3. Milestone tracking
4. SQLite persistence (save/load)
5. Multi-project parallel management
6. Agent hibernation and wake

**Expected:** 20+ tests

---

### **Phase 1.2: Enhanced Agent State Machines** (~40K tokens, 1-2 sessions)

**Goal:** Add new states (Conversation, Monitoring, Planning) and PM/Worker agent types

#### Files to Modify/Create

**1. Modify `hainet-persona/src/agents/state.rs`** (+150 LOC)
```rust
// Add new states
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    Startup,
    Conversation,  // NEW: Admin AI conversing with user
    Planning,      // NEW: Admin AI creating project plan
    Monitoring,    // NEW: Admin AI monitoring active projects
    Managing,      // NEW: PM agent managing project
    Working,       // Existing: Worker executing task
    Idle,
    Error,
}

// Update state machine transitions
impl AgentStateMachine {
    pub fn can_transition_to(&self, new_state: &AgentState) -> bool {
        match (&self.current_state, new_state) {
            // Admin AI transitions
            (AgentState::Startup, AgentState::Conversation) => true,
            (AgentState::Conversation, AgentState::Planning) => true,
            (AgentState::Planning, AgentState::Monitoring) => true,
            (AgentState::Monitoring, AgentState::Conversation) => true,
            
            // PM transitions
            (AgentState::Startup, AgentState::Planning) => true,
            (AgentState::Planning, AgentState::Managing) => true,
            
            // Worker transitions
            (AgentState::Idle, AgentState::Working) => true,
            (AgentState::Working, AgentState::Idle) => true,
            
            // ... existing transitions
        }
    }
}
```

**2. Create `hainet-persona/src/agents/pm.rs`** (~400 LOC)
```rust
/// PM Agent - Project Manager
pub struct PMAgent {
    id: AgentId,
    project_id: ProjectId,
    state_machine: AgentStateMachine,
    context: Arc<AgentContext>,
    
    // PM-specific
    worker_templates: Vec<WorkerTemplate>,
}

impl PMAgent {
    pub fn new(project_id: ProjectId, context: Arc<AgentContext>) -> Self;
    
    // Startup phase
    pub async fn analyze_initial_tasks(&mut self, tasks: Vec<String>) -> Result<()>;
    
    // Planning phase
    pub async fn break_down_tasks(&mut self) -> Result<Vec<Task>>;
    pub async fn create_milestones(&mut self) -> Result<Vec<Milestone>>;
    pub async fn design_worker_team(&mut self) -> Result<Vec<WorkerRequest>>;
    
    // Managing phase
    pub async fn assign_tasks(&mut self) -> Result<()>;
    pub async fn validate_deliverable(&mut self, task_id: TaskId, deliverable: String) -> Result<bool>;
    pub async fn handle_worker_message(&mut self, message: Message) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct WorkerRequest {
    pub specialization: WorkerType,
    pub custom_prompt_addition: Option<String>,
    pub initial_task: TaskId,
}
```

**3. Create `hainet-persona/src/agents/worker.rs`** (~350 LOC)
```rust
/// Worker Agent - Task executor
pub struct WorkerAgent {
    id: AgentId,
    worker_type: WorkerType,
    project_id: ProjectId,
    pm_id: AgentId,
    state_machine: AgentStateMachine,
    context: Arc<AgentContext>,
    
    // Current task
    current_task: Option<TaskId>,
}

impl WorkerAgent {
    pub fn new(
        worker_type: WorkerType,
        project_id: ProjectId,
        pm_id: AgentId,
        custom_prompt: Option<String>,
        context: Arc<AgentContext>
    ) -> Self;
    
    pub async fn execute_task(&mut self, task: Task) -> Result<Vec<String>>;  // Returns deliverables
    pub async fn report_progress(&self) -> Result<()>;
    pub async fn report_completion(&self, deliverables: Vec<String>) -> Result<()>;
}
```

**4. Create `hainet-persona/src/agents/templates.rs`** (~300 LOC)
```rust
/// Default worker templates that PMs can customize
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerTemplate {
    pub worker_type: WorkerType,
    pub default_system_prompt: String,
    pub required_tools: Vec<String>,  // MCP tools
    pub specializations: Vec<String>,
}

pub fn get_default_templates() -> Vec<WorkerTemplate> {
    vec![
        WorkerTemplate {
            worker_type: WorkerType::FileWorker,
            default_system_prompt: "You are a file operations specialist...".to_string(),
            required_tools: vec!["hainet_file_read", "hainet_file_write", "hainet_file_list"],
            specializations: vec!["file_management", "data_processing"],
        },
        WorkerTemplate {
            worker_type: WorkerType::NetworkWorker,
            default_system_prompt: "You are a network operations specialist...".to_string(),
            required_tools: vec!["hainet_http_get", "hainet_http_post"],
            specializations: vec!["api_integration", "web_scraping"],
        },
        WorkerTemplate {
            worker_type: WorkerType::CodeWorker,
            default_system_prompt: "You are a code generation specialist...".to_string(),
            required_tools: vec!["hainet_file_write", "hainet_execute_command"],
            specializations: vec!["code_generation", "refactoring", "testing"],
        },
        // ... more templates
    ]
}

impl WorkerTemplate {
    pub fn customize(&self, custom_addition: &str) -> String {
        format!("{}\n\nAdditional Instructions:\n{}", self.default_system_prompt, custom_addition)
    }
}
```

#### Tests

1. Admin AI state transitions (Startup → Conversation → Planning → Monitoring)
2. PM state transitions (Startup → Planning → Managing)
3. Worker state transitions (Idle → Working → Idle)
4. Worker template customization
5. Agent hibernation and restoration

**Expected:** 15+ tests

---

### **Phase 1.3: Admin AI Planning & PM Creation** (~50K tokens, 2 sessions)

**Goal:** Admin AI detects complex intents, creates projects, spawns PM agents

#### Implementation

**1. Modify `hainet-persona/src/agents/admin.rs`** (+500 LOC)

```rust
impl AdminAgent {
    /// Analyze user input and determine appropriate action
    pub async fn process_user_input(&mut self, user_input: String) -> Result<String> {
        // Parse intent
        let intent = self.intent_parser.parse(&user_input).await?;
        
        match (&self.state_machine.current_state(), &intent.intent_type) {
            // In Conversation state, check if we should transition to Planning
            (AgentState::Conversation, IntentType::Task) if self.is_complex_intent(&intent) => {
                self.transition_to_planning(intent).await
            },
            
            // Simple conversation
            (AgentState::Conversation, _) => {
                self.respond_conversationally(&user_input).await
            },
            
            // ... other state/intent combinations
        }
    }
    
    fn is_complex_intent(&self, intent: &Intent) -> bool {
        // Heuristics: multi-step, requires planning, mentions project
        intent.normalized_text.split_whitespace().count() > 10 ||
        intent.suggested_domain.is_some() ||
        intent.normalized_text.contains("build") ||
        intent.normalized_text.contains("create") ||
        intent.normalized_text.contains("project")
    }
    
    async fn transition_to_planning(&mut self, intent: Intent) -> Result<String> {
        self.state_machine.transition(AgentState::Planning, "Complex intent detected".to_string())?;
        
        // Use LLM to decompose intent into project plan
        let plan = self.create_project_plan(&intent).await?;
        
        // Create project
        let project_id = self.context.project_manager
            .write().await
            .create_project(plan.title.clone(), plan.overview.clone(), plan.initial_tasks)
            .await?;
        
        // Create PM agent
        let pm_id = self.create_pm_agent(project_id, &plan).await?;
        
        // Assign PM to project
        self.context.project_manager
            .write().await
            .assign_pm(project_id, pm_id)
            .await?;
        
        // Transition to Monitoring
        self.state_machine.transition(AgentState::Monitoring, "Project created".to_string())?;
        
        Ok(format!("I've created project '{}' and assigned a PM. The team will start working on it.", plan.title))
    }
    
    async fn create_project_plan(&self, intent: &Intent) -> Result<ProjectPlan> {
        // Use AI provider (direct call for simple decomposition)
        let provider_manager = self.context.ai_provider_manager.read().await;
        let model = provider_manager.select_model_for_agent(SelectionContext::for_admin()).await?;
        
        let prompt = format!(
            "Decompose this user request into a project plan:\n\n\
             User Request: {}\n\n\
             Output JSON with:\n\
             - title: short project title\n\
             - overview: detailed description\n\
             - initial_tasks: array of 3-5 high-level tasks",
            intent.original_text
        );
        
        let response = provider_manager.call_llm(&model, &prompt).await?;
        let plan: ProjectPlan = serde_json::from_str(&response)?;
        
        Ok(plan)
    }
    
    async fn create_pm_agent(&self, project_id: ProjectId, plan: &ProjectPlan) -> Result<AgentId> {
        let pm_prompt = format!(
            "You are a Project Manager for the '{}' project.\n\
             Goal: {}\n\
             Initial Tasks: {:?}",
            plan.title, plan.overview, plan.initial_tasks
        );
        
        let pm_agent = PMAgent::new_with_prompt(project_id, pm_prompt, self.context.clone());
        let pm_id = pm_agent.id().clone();
        
        // Store PM agent (to be managed by framework)
        // This will be handled by AgentRegistry in later phase
        
        Ok(pm_id)
    }
}

#[derive(Debug, Deserialize)]
struct ProjectPlan {
    title: String,
    overview: String,
    initial_tasks: Vec<String>,
}
```

**2. Add LLM Integration Helper** (~200 LOC)

```rust
// hainet-persona/src/ai_providers/mod.rs

impl AIProviderManager {
    /// Simple LLM call for Admin AI (direct, not via MCP)
    pub async fn call_llm(&self, model: &SelectedModel, prompt: &str) -> Result<String> {
        match &model.provider_type {
            ProviderType::Ollama => {
                let client = OllamaClient::new(&model.inference_url);
                client.generate(&model.model_name, prompt, None).await
            },
            // ... other providers
        }
    }
}
```

#### Tests

1. Complex intent detection
2. Project plan generation (mocked LLM)
3. PM agent creation
4. State transition Admin: Conversation → Planning → Monitoring
5. Multiple parallel projects

**Expected:** 12+ tests

---

## Estimated Totals

**Phase 1.1:** 60K tokens, ~1,900 LOC, 20+ tests  
**Phase 1.2:** 40K tokens, ~1,200 LOC, 15+ tests  
**Phase 1.3:** 50K tokens, ~700 LOC, 12+ tests  

**Total Phase 1:** ~150K tokens, ~3,800 LOC, 47+ tests, 5-6 development sessions

---

## Dependencies Summary

```toml
[dependencies]
sqlx = { version = "0.7", features = ["runtime-tokio-native-tls", "sqlite", "chrono"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
```

---

## Next Steps

1. ✅ Create this detailed plan
2. Update PROJECT_PLAN.md with Phase 1 breakdown
3. Begin Phase 1.1 implementation

---

**Last Updated:** 2025-10-22  
**Ready to Implement:** Yes

//! # END OF FILE helperfiles/PHASE_1_DETAILED_PLAN.md
