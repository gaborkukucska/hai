//! # START OF FILE hainet-persona/src/agents/pm.rs
//! Project Manager Agent
//! 
//! Manages a single project, coordinating worker agents and ensuring task completion.
//! PM agents follow this state machine:
//! Startup → Planning → Managing → (Idle | Error)

use anyhow::{Result, Context};
use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::messaging::{MessageBus, AgentId};
use crate::prompts::{PromptManager, AgentType, AgentState, PromptContext};
use crate::projects::{ProjectManager, ProjectId, TaskId};
use crate::ai_providers::providers::{GenerationOptions};
use crate::ai_providers::{AIProviderManager, SelectionContext};
use crate::test_utils::JSONValidator;
use super::state::AgentStateMachine;
use super::templates::WorkerTemplate;
use super::pm_intelligence::{
    HistoricalLearner, ProjectComplexity, DecompositionStrategy, 
    ProjectOutcome
};
use super::session_tasks::{SessionTaskList, TaskStatus as SessionTaskStatus};

/// Pending validation task
struct PendingValidation {
    task_id: TaskId,
    started_at: SystemTime,
    handle: JoinHandle<Result<ValidationResponse>>,
}

/// Project Manager Agent
/// 
/// Responsible for:
/// - Breaking down project into detailed tasks using LLM
/// - Creating and managing worker agents
/// - Building dependency graphs for task ordering
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
    
    /// AI provider manager for dynamic model selection
    ai_provider_manager: Arc<AIProviderManager>,
    
    /// Shared MCP client manager (initialized with connected servers)
    mcp_client: Arc<RwLock<crate::tools::mcp::MCPClientManager>>,
    
    /// User settings manager for model preferences
    user_settings: Option<Arc<RwLock<crate::user_settings::UserSettingsManager>>>,
    
    /// Spawned worker agents (task_id -> worker_agent_id)
    workers: HashMap<TaskId, AgentId>,

    /// Active worker channels (TemplateName -> (AgentId, mpsc::Sender<TaskId>))
    active_workers: HashMap<String, (AgentId, mpsc::Sender<TaskId>)>,
    
    /// Task dependency graph
    task_graph: Option<TaskGraph>,
    
    /// Historical learner for strategy selection
    learner: HistoricalLearner,
    
    /// Current project complexity (cached during planning)
    project_complexity: Option<ProjectComplexity>,
    
    /// Selected decomposition strategy
    selected_strategy: Option<DecompositionStrategy>,
    
    /// Project start time for duration tracking
    project_start_time: Option<SystemTime>,
    
    /// Session task list for LLM context (short-term memory)
    session_tasks: SessionTaskList,
    
    /// Pending async validations (task_id -> validation handle)
    pending_validations: HashMap<TaskId, PendingValidation>,
    
    /// Message receiver (kept alive to maintain registration)
    _receiver: Option<mpsc::Receiver<crate::messaging::Message>>,
}

impl PMAgent {
    /// Create new PM agent for a project
    pub fn new(
        project_id: ProjectId,
        message_bus: Arc<RwLock<MessageBus>>,
        prompt_manager: Arc<RwLock<PromptManager>>,
        project_manager: Arc<RwLock<ProjectManager>>,
        ai_provider_manager: Arc<AIProviderManager>,
        mcp_client: Arc<RwLock<crate::tools::mcp::MCPClientManager>>,
        user_settings: Option<Arc<RwLock<crate::user_settings::UserSettingsManager>>>,
    ) -> Self {
        let id = AgentId::new(AgentType::PM, format!("PM-{}", project_id));
        
        Self {
            id,
            project_id,
            state_machine: AgentStateMachine::new(),
            message_bus,
            prompt_manager,
            project_manager,
            ai_provider_manager,
            mcp_client,
            user_settings,

            workers: HashMap::new(),
            active_workers: HashMap::new(),
            task_graph: None,
            learner: HistoricalLearner::new(),
            project_complexity: None,
            selected_strategy: None,
            project_start_time: None,
            session_tasks: SessionTaskList::new(),
            pending_validations: HashMap::new(),
            _receiver: None,
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
    
    /// Helper to update status
    async fn update_status(&self, activity: &str) {
        self.message_bus.write().await.update_agent_status(
            self.id.clone(),
            format!("{:?}", self.state_machine.current_state()),
            activity.to_string()
        ).await;
    }

    /// Start PM agent lifecycle
    /// 
    /// Startup → Idle → Planning → Managing
    pub async fn initialize_and_plan(&mut self) -> Result<()> {
        // Record project start time
        self.project_start_time = Some(SystemTime::now());
        
        // Add initial session task: project analysis
        self.session_tasks.add_task("Analyze project requirements".to_string(), None);
        
        // Transition from Startup to Idle (PM agents must go through Idle first)
        self.state_machine.transition(
            AgentState::Idle,
            "PM initialized, ready to plan".to_string()
        )?;
        
        // Register with MessageBus
        let (receiver, _) = self.message_bus.write().await
            .register_agent(self.id.clone())
            .await
            .context("Failed to register PM agent with MessageBus")?;
        self._receiver = Some(receiver);

        self.state_machine.transition(
            AgentState::Planning,
            "Initializing project planning".to_string()
        )?;
        self.update_status("Planning project").await;
        
        // Mark analysis task as in progress
        let _ = self.session_tasks.start_task("Analyze project requirements");
        
        // Analyze project and create detailed plan
        self.analyze_and_plan().await?;
        
        // Mark analysis as complete
        let _ = self.session_tasks.complete_task("Analyze project requirements");
        
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
        
        tracing::info!("PM analyzing project: {}", project.title);
        
        // Get existing tasks from project
        let existing_tasks = project_manager.get_project_tasks(&self.project_id).await?;
        drop(project_manager);
        
        // Analyze project complexity
        let task_descriptions: Vec<String> = existing_tasks.iter()
            .map(|t| t.description.clone())
            .collect();
        
        let complexity = ProjectComplexity::analyze(&project.overview, &task_descriptions);
        
        tracing::info!(
            "Project complexity: {} (score: {:.2}, tasks: {}, domains: {})",
            complexity.category(),
            complexity.score,
            complexity.task_count,
            complexity.domain_count
        );
        
        // Get strategy recommendation from historical learning
        let strategy = self.learner.recommend_strategy(&complexity);
        
        tracing::info!(
            "Selected decomposition strategy: {:?} (learner has {} outcomes)",
            strategy,
            self.learner.outcome_count()
        );
        
        // Store for later use
        self.project_complexity = Some(complexity);
        self.selected_strategy = Some(strategy);
        
        // Use LLM to decompose tasks into detailed subtasks with strategy guidance
        let detailed_plan = self.generate_detailed_plan_with_strategy(
            &project,
            &existing_tasks,
            strategy
        ).await?;
        
        // Create detailed tasks in database and add to session task list
        for task_detail in &detailed_plan.tasks {
            let project_manager = self.project_manager.write().await;
            project_manager.create_task(
                &self.project_id,
                task_detail.title.clone(),
                task_detail.description.clone(),
            ).await?;
            
            // Add task to session list (truncated title for readability)
            let task_title = if task_detail.title.len() > 50 {
                format!("{}...", &task_detail.title[..47])
            } else {
                task_detail.title.clone()
            };
            self.session_tasks.add_task(task_title, None);
        }
        
        // Build dependency graph
        let all_tasks = {
            let pm = self.project_manager.read().await;
            pm.get_project_tasks(&self.project_id).await?
        };
        
        let num_tasks = detailed_plan.tasks.len();
        let num_deps = detailed_plan.dependencies.len();
        
        self.task_graph = Some(TaskGraph::build(all_tasks, detailed_plan.dependencies)?);
        
        tracing::info!("PM completed planning: {} tasks with {} dependencies", 
                       num_tasks, num_deps);
        
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

            // Process incoming messages
            let mut messages = Vec::new();
        if let Some(receiver) = &mut self._receiver {
            while let Ok(msg) = receiver.try_recv() {
                messages.push(msg);
            }
        }
        
        for msg in messages {
            self.handle_message(msg).await?;
        }
            
            // Check project status from manager
            {
                let pm = self.project_manager.read().await;
                if let Some(project) = pm.get_project(&self.project_id).await? {
                    match project.status {
                        crate::projects::ProjectStatus::Paused => {
                            tracing::info!("PM {} project paused, sleeping...", self.id.name);
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            continue;
                        },
                        crate::projects::ProjectStatus::Cancelled | 
                        crate::projects::ProjectStatus::Failed | 
                        crate::projects::ProjectStatus::Completed => {
                            tracing::info!("PM {} project in terminal state ({:?}), stopping...", self.id.name, project.status);
                            break;
                        },
                        _ => {} // Continue managing
                    }
                } else {
                    tracing::warn!("PM {} project not found, stopping...", self.id.name);
                    break;
                }

                // Log task status distribution for debugging
                let all_tasks = pm.get_project_tasks(&self.project_id).await?;
                let task_status_summary: HashMap<String, usize> = all_tasks.iter()
                    .fold(HashMap::new(), |mut acc, task| {
                        let status_str = format!("{:?}", task.status);
                        *acc.entry(status_str).or_insert(0) += 1;
                        acc
                    });
                
                tracing::info!(
                    "PM {} manage_loop iteration: {:?}",
                    self.id.name,
                    task_status_summary
                );
            }
            
            // Detect and mark stuck tasks
            self.detect_stuck_tasks().await?;
            self.detect_blocked_tasks().await?;
            
            // Get executable tasks (unassigned + dependencies met)
            let executable_tasks = self.get_executable_tasks().await?;
            
            tracing::info!(
                "PM {} found {} executable tasks",
                self.id.name,
                executable_tasks.len()
            );
            
            // Spawn workers and assign tasks
            for task_id in executable_tasks {
                self.assign_task_to_worker(&task_id).await?;
            }
            
            // Poll pending validations (non-blocking)
            self.poll_pending_validations().await?;
            
            // Timeout stale validations (>60s)
            self.timeout_stale_validations().await?;
            
            // Check for completed tasks needing validation
            let tasks_under_review = self.get_tasks_under_review().await?;
            
            tracing::info!(
                "PM {} found {} tasks under review",
                self.id.name,
                tasks_under_review.len()
            );
            
            // Spawn async validations for tasks under review (non-blocking)
            for task_id in tasks_under_review {
                self.spawn_validation(task_id)?;
            }
            
            // Check for tasks needing revision
            let tasks_needing_revision = self.get_tasks_needing_revision().await?;
            
            tracing::info!(
                "PM {} found {} tasks needing revision",
                self.id.name,
                tasks_needing_revision.len()
            );
            
            for task_id in tasks_needing_revision {
                self.handle_revision_task(&task_id).await?;
            }
            
            // Check if all tasks are complete
            if self.is_project_complete().await? {
                // Double check with LLM assessment
                let assessment = self.assess_project_completeness().await?;
                
                if assessment.complete {
                    tracing::info!("PM {} assessment: Project COMPLETE. Reasoning: {}", self.id.name, assessment.reasoning);
                    self.complete_project().await?;
                    break;
                } else {
                    tracing::info!("PM {} assessment: Project INCOMPLETE. Creating {} missing tasks. Reasoning: {}", 
                        self.id.name, assessment.missing_tasks.len(), assessment.reasoning);
                    
                    for task in assessment.missing_tasks {
                        let project_manager = self.project_manager.write().await;
                        project_manager.create_task(
                            &self.project_id,
                            task.title.clone(),
                            task.description,
                        ).await?;
                        
                        // Add to session tasks
                        self.session_tasks.add_task(task.title, None);
                    }
                    // Continue loop to process new tasks
                }
            }
            
            // Sleep for 5s to balance responsiveness and load (reduced from 15s, increased from 500ms)
            tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
        }
        
        Ok(())
    }
    
    /// Get executable tasks (unassigned with dependencies met)
    async fn get_executable_tasks(&self) -> Result<Vec<TaskId>> {
        let project_manager = self.project_manager.read().await;
        let tasks = project_manager.get_project_tasks(&self.project_id).await?;
        
        let unassigned: Vec<TaskId> = tasks.iter()
            .filter(|task| matches!(task.status, crate::projects::TaskStatus::Unassigned))
            .map(|task| task.id.clone())
            .collect();
        
        // Get completed task IDs
        let completed: HashSet<TaskId> = tasks.iter()
            .filter(|task| matches!(task.status, crate::projects::TaskStatus::Complete))
            .map(|task| task.id.clone())
            .collect();
        
        // Filter by dependency graph
        if let Some(graph) = &self.task_graph {
            let executable: Vec<TaskId> = unassigned.into_iter()
                .filter(|task_id| {
                    let can_exec = graph.can_execute(task_id, &completed);
                    if !can_exec {
                        if let Some(deps) = graph.dependencies.get(task_id) {
                            let missing: Vec<_> = deps.iter().filter(|d| !completed.contains(d)).collect();
                            tracing::debug!("Task {} not executable. Missing deps: {:?}", task_id, missing);
                        }
                    }
                    can_exec
                })
                .collect();
            Ok(executable)
        } else {
            // No graph, return all unassigned
            Ok(unassigned)
        }
    }
    
    /// Detect and mark tasks stuck in InProgress state
    /// Tasks stuck for more than 2 minutes are marked as Stuck for manual intervention
    async fn detect_stuck_tasks(&mut self) -> Result<()> {
        const STUCK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120); // 2 minutes
        
        let project_manager = self.project_manager.read().await;
        let tasks = project_manager.get_project_tasks(&self.project_id).await?;
        drop(project_manager);
        
        for task in tasks {
            if task.is_stuck(STUCK_TIMEOUT) {
                tracing::warn!(
                    "PM {} detected stuck task: {} ({}). Marking as Stuck for manual intervention.",
                    self.id.name,
                    task.id,
                    task.title
                );
                
                // Mark task as stuck
                let mut project_manager = self.project_manager.write().await;
                project_manager.mark_task_stuck(
                    &task.id,
                    format!(
                        "Task stuck in InProgress for more than {} seconds. Worker may be queued, generating, stuck, or errored. Manual intervention required.",
                        STUCK_TIMEOUT.as_secs()
                    )
                ).await?;
            }
        }
        
        Ok(())
    }

    /// Detect unassigned tasks that are blocked by failed/stuck dependencies
    async fn detect_blocked_tasks(&mut self) -> Result<()> {
        let project_manager = self.project_manager.read().await;
        let tasks = project_manager.get_project_tasks(&self.project_id).await?;
        drop(project_manager);

        let task_status_map: HashMap<TaskId, crate::projects::TaskStatus> = tasks.iter()
            .map(|t| (t.id.clone(), t.status.clone()))
            .collect();

        for task in tasks.iter().filter(|t| t.status == crate::projects::TaskStatus::Unassigned) {
            for dep_id in &task.dependencies {
                if let Some(dep_status) = task_status_map.get(dep_id) {
                    if matches!(dep_status, crate::projects::TaskStatus::Failed | crate::projects::TaskStatus::Stuck) {
                        tracing::warn!(
                            "PM {} detected blocked task: {} ({}). Dependency {} is {:?}. Marking as Stuck.",
                            self.id.name,
                            task.id,
                            task.title,
                            dep_id,
                            dep_status
                        );
                        
                        let mut project_manager = self.project_manager.write().await;
                        project_manager.mark_task_stuck(
                            &task.id,
                            format!("Dependency {} is {:?}", dep_id, dep_status)
                        ).await?;
                        break; // Mark once and move to next task
                    }
                }
            }
        }
        Ok(())
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
    
    /// Get tasks that need revision (rejected by PM)
    async fn get_tasks_needing_revision(&self) -> Result<Vec<TaskId>> {
        let project_manager = self.project_manager.read().await;
        let tasks = project_manager.get_project_tasks(&self.project_id).await?;
        
        Ok(tasks.into_iter()
            .filter(|task| matches!(task.status, crate::projects::TaskStatus::NeedsRevision))
            .map(|task| task.id)
            .collect())
    }
    
    /// Validate task results submitted by worker
    async fn validate_task(&mut self, task_id: &TaskId) -> Result<()> {
        let task = {
            let pm = self.project_manager.read().await;
            pm.get_task(task_id).await?
        };
        
        // Generate validation prompt
        let prompt = self.generate_validation_prompt(&task)?;
        
        // Select model with user preferences
        let mut selection_context = SelectionContext::for_pm();
        
        // Apply user's preferred model family if available
        if let Some(ref user_settings) = self.user_settings {
            let settings = user_settings.read().await;
            if let Ok(Some(family)) = settings.get_model_preference("PM").await {
                tracing::debug!("PM {} using user-preferred model family: {}", self.id.name, family);
                selection_context.preferred_family = Some(family);
            }
        }
        let selected_model = self.ai_provider_manager
            .select_model_for_agent(selection_context)
            .await
            .context("Failed to select a model for validation")?;
        
        // Call LLM for validation decision
        let options = GenerationOptions {
            temperature: Some(0.3),
            max_tokens: Some(4096),
            ..Default::default()
        };
        
        let client = selected_model.get_client()?;
        // Strip provider prefix from model_id (e.g., "Ollama::model" -> "model")
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };
        
        // Add timeout wrapper to prevent indefinite hanging
        tracing::info!("[DIAGNOSTIC] PM {} calling LLM for validation (model: {})", self.id.name, model_name);
        let llm_timeout = tokio::time::Duration::from_secs(300); // 300s timeout for LLM generation
        let response = tokio::time::timeout(
            llm_timeout,
            client.generate(model_name, &prompt, options)
        )
        .await
        .context(format!("LLM validation timed out after {:?}", llm_timeout))?
        .context("Failed to validate task with LLM")?;
        tracing::debug!(
            target: "llm_messages",
            "[PM VALIDATION RESPONSE] Model: {}, Response ({} chars):\n{}",
            model_name,
            response.text.len(),
            response.text
        );
        
        // Parse validation decision
        let validation = self.parse_validation_response(&response.text)?;
        
        let pm = self.project_manager.read().await;
        
        if validation.approved {
            // Approve task
            pm.approve_task(task_id, validation.feedback).await?;
            
            // Update session task to complete
            let task_title = if task.title.len() > 50 {
                format!("{}...", &task.title[..47])
            } else {
                task.title.clone()
            };
            let _ = self.session_tasks.update_status(&task_title, SessionTaskStatus::Complete);
            
            tracing::info!(
                "PM {} approved task {} - {}",
                self.id.name,
                task_id,
                task.title
            );
        } else if validation.revision_needed && task.can_retry_revision() {
            // Request revision
            pm.request_revision(task_id, validation.feedback.clone()).await?;
            
            tracing::warn!(
                "PM {} requested revision for task {} (attempt {}/{}) - {}",
                self.id.name,
                task_id,
                task.revision_count + 1,
                task.max_revisions,
                validation.feedback
            );
        } else {
            // Reject or max revisions exceeded
            let reason = if task.can_retry_revision() {
                validation.feedback
            } else {
                format!("Max revisions exceeded. Last feedback: {}", validation.feedback)
            };
            
            pm.fail_task(task_id, reason.clone()).await?;
            
            tracing::error!(
                "PM {} failed task {} - {}",
                self.id.name,
                task_id,
                reason
            );
        }
        
        Ok(())
    }
    
    /// Spawn async validation task (non-blocking)
    fn spawn_validation(&mut self, task_id: TaskId) -> Result<()> {
        // Don't spawn if already validating
        if self.pending_validations.contains_key(&task_id) {
            return Ok(());
        }
        
        // Clone necessary data for the spawned task
        let project_manager = self.project_manager.clone();
        let ai_provider_manager = self.ai_provider_manager.clone();
        let id_name = self.id.name.clone();
        let task_id_clone = task_id.clone();
        
        // Spawn validation as separate task
        let handle = tokio::spawn(async move {
            Self::validate_task_async(
                task_id_clone,
                project_manager,
                ai_provider_manager,
                id_name,
            ).await
        });
        
        // Store pending validation
        self.pending_validations.insert(task_id.clone(), PendingValidation {
            task_id: task_id.clone(),
            started_at: SystemTime::now(),
            handle,
        });
        
        tracing::info!("PM {} spawned async validation for task {}", self.id.name, task_id);
        Ok(())
    }
    
    /// Poll pending validations and process completed ones
    async fn poll_pending_validations(&mut self) -> Result<()> {
        let mut completed = Vec::new();
        
        // Check which validations are complete
        for (task_id, pending) in &mut self.pending_validations {
            if pending.handle.is_finished() {
                completed.push(task_id.clone());
            }
        }
        
        // Process completed validations
        for task_id in completed {
            if let Some(pending) = self.pending_validations.remove(&task_id) {
                match pending.handle.await {
                    Ok(Ok(validation_response)) => {
                        self.handle_validation_result(&task_id, validation_response).await?;
                    }
                    Ok(Err(e)) => {
                        tracing::error!(
                            "PM {} validation failed for task {}: {:?}",
                            self.id.name,
                            task_id,
                            e
                        );
                        // Request revision instead of auto-approving
                        self.request_task_revision(
                            &task_id, 
                            format!("Validation failed - please review and resubmit: {}", e)
                        ).await?;
                    }
                    Err(e) => {
                        tracing::error!(
                            "PM {} validation task panicked for task {}: {:?}",
                            self.id.name,
                            task_id,
                            e
                        );
                        // Request revision on panic instead of auto-approving
                        self.request_task_revision(
                            &task_id,
                            format!("Validation system error - please review and resubmit: {}", e)
                        ).await?;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Handle validation result
    async fn handle_validation_result(&mut self, task_id: &TaskId, validation: ValidationResponse) -> Result<()> {
        let task = {
            let pm = self.project_manager.read().await;
            pm.get_task(task_id).await?
        };
        
        let pm = self.project_manager.read().await;
        
        if validation.approved {
            pm.approve_task(task_id, validation.feedback).await?;
            
            let task_title = if task.title.len() > 50 {
                format!("{}...", &task.title[..47])
            } else {
                task.title.clone()
            };
            let _ = self.session_tasks.update_status(&task_title, SessionTaskStatus::Complete);
            
            tracing::info!(
                "PM {} approved task {} - {}",
                self.id.name,
                task_id,
                task.title
            );
        } else if validation.revision_needed && task.can_retry_revision() {
            pm.request_revision(task_id, validation.feedback.clone()).await?;
            
            tracing::warn!(
                "PM {} requested revision for task {} (attempt {}/{}) - {}",
                self.id.name,
                task_id,
                task.revision_count + 1,
                task.max_revisions,
                validation.feedback
            );
        } else {
            let reason = if task.can_retry_revision() {
                validation.feedback
            } else {
                format!("Max revisions exceeded. Last feedback: {}", validation.feedback)
            };
            
            pm.fail_task(task_id, reason.clone()).await?;
            
            tracing::error!(
                "PM {} failed task {} - {}",
                self.id.name,
                task_id,
                reason
            );
        }
        
        Ok(())
    }
    
    /// Timeout stale validations (>60s) and request revision
    async fn timeout_stale_validations(&mut self) -> Result<()> {
        let timeout_duration = std::time::Duration::from_secs(60);
        let now = SystemTime::now();
        let mut timed_out = Vec::new();
        
        for (task_id, pending) in &self.pending_validations {
            if let Ok(elapsed) = now.duration_since(pending.started_at) {
                if elapsed > timeout_duration {
                    timed_out.push(task_id.clone());
                }
            }
        }
        
        for task_id in timed_out {
            if let Some(pending) = self.pending_validations.remove(&task_id) {
                tracing::warn!(
                    "PM {} validation timed out for task {} after 60s, requesting revision",
                    self.id.name,
                    task_id
                );
                
                // Abort the hanging validation task
                pending.handle.abort();
                
                // Request revision instead of auto-approving
                self.request_task_revision(
                    &task_id,
                    "Validation timed out after 60s - please review and resubmit your work".to_string()
                ).await?;
            }
        }
        
        Ok(())
    }
    
    /// Request task revision with feedback
    async fn request_task_revision(&self, task_id: &TaskId, feedback: String) -> Result<()> {
        let pm = self.project_manager.read().await;
        pm.request_revision(task_id, feedback).await?;
        
        tracing::warn!(
            "PM {} requested revision for task {} - validation failed",
            self.id.name,
            task_id
        );
        
        Ok(())
    }
    
    /// Handle revision tasks - reset them for worker to retry
    

    /// Validate task with simple file-based checks (no LLM)
    async fn validate_task_async(
        task_id: TaskId,
        project_manager: Arc<RwLock<ProjectManager>>,
        _ai_provider_manager: Arc<AIProviderManager>,
        pm_name: String,
    ) -> Result<ValidationResponse> {
        let task = {
            let pm = project_manager.read().await;
            pm.get_task(&task_id).await?
        };
        
        tracing::info!("PM {} validating task {} with file-based checks", pm_name, task_id);
        tracing::debug!("PM {} validation context - Deliverables: {:?}", pm_name, task.deliverables);
        
        // Simple validation checks (no LLM needed):
        
        // 1. Check deliverables exist
        if task.deliverables.is_empty() {
            tracing::warn!("PM {} task {} has no deliverables", pm_name, task_id);
            return Ok(ValidationResponse {
                approved: false,
                feedback: "No deliverables submitted. Please complete the task and submit your work.".to_string(),
                revision_needed: true,
            });
        }
        
        // 2. Check deliverables are not empty/trivial
        let total_content: usize = task.deliverables.iter()
            .map(|d| d.len())
            .sum();
        
        if total_content < 50 {
            tracing::warn!("PM {} task {} has minimal deliverables ({} chars)", pm_name, task_id, total_content);
            return Ok(ValidationResponse {
                approved: false,
                feedback: format!("Deliverables are too minimal ({} chars total). Please provide substantial work.", total_content),
                revision_needed: true,
            });
        }
        
        // 3. Check for placeholder/TODO content
        let has_placeholders = task.deliverables.iter()
            .any(|d| {
                let lower = d.to_lowercase();
                lower.contains("todo") || 
                lower.contains("fixme") || 
                lower.contains("placeholder") ||
                lower.contains("not implemented") ||
                lower.contains("coming soon")
            });
        
        if has_placeholders {
            tracing::warn!("PM {} task {} contains placeholder content", pm_name, task_id);
            return Ok(ValidationResponse {
                approved: false,
                feedback: "Deliverables contain TODO/placeholder content. Please complete the implementation.".to_string(),
                revision_needed: true,
            });
        }
        
        // All checks passed - approve
        tracing::info!("PM {} approved task {} ({} deliverables, {} chars)", 
            pm_name, task_id, task.deliverables.len(), total_content);
        tracing::debug!("PM {} approval details - Total Chars: {}", pm_name, total_content);
        
        Ok(ValidationResponse {
            approved: true,
            feedback: format!("Task approved. Deliverables meet requirements ({} items, {} chars total).", 
                task.deliverables.len(), total_content),
            revision_needed: false,
        })
    }
    
    /// Static version of generate_validation_prompt
    fn generate_validation_prompt_static(task: &crate::projects::Task) -> Result<String> {
        Ok(format!(
            r#"You are a Project Manager AI validating worker output.

TASK: {}
DESCRIPTION: {}

WORKER DELIVERABLES:
{}

Evaluate if the deliverables meet the task requirements.

Respond with ONLY this JSON format (no other text):
{{
  "approved": true,
  "feedback": "your feedback here",
  "revision_needed": false
}}

JSON response:"#,
            task.title,
            task.description,
            task.deliverables.join("\n")
        ))
    }
    
    /// Static version of parse_validation_response
    fn parse_validation_response_static(response: &str) -> Result<ValidationResponse> {
        let json_str = if response.contains("```json") {
            response.split("```json").nth(1)
                .and_then(|s| s.split("```").next())
                .unwrap_or(response)
        } else {
            response
        }.trim();
        
        serde_json::from_str(json_str)
            .context(format!("Failed to parse validation response: {}", json_str))
    }
    
    /// Handle a task that needs revision
    async fn handle_revision_task(&mut self, task_id: &TaskId) -> Result<()> {
        let task = {
            let pm = self.project_manager.read().await;
            pm.get_task(task_id).await?
        };
        
        // Check if task can be retried
        if !task.can_retry_revision() {
            tracing::warn!(
                "PM {} task '{}' exceeded max revisions ({}), marking as failed",
                self.id.name,
                task.title,
                task.max_revisions
            );
            
            let failure_reason = format!(
                "Exceeded maximum revision attempts ({}). Last feedback: {}",
                task.max_revisions,
                task.pm_feedback.as_deref().unwrap_or("No feedback")
            );
            
            let pm = self.project_manager.read().await;
            pm.fail_task(task_id, failure_reason).await?;
            return Ok(());
        }
        
        // Reset task for revision
        tracing::info!(
            "PM {} resetting task '{}' for revision (attempt {}/{})",
            self.id.name,
            task.title,
            task.revision_count + 1,
            task.max_revisions
        );
        
        // The task will be reset to InProgress, and the existing worker will retry
        // We don't need to spawn a new worker - the existing one should still be active
        let pm = self.project_manager.read().await;
        pm.reset_task_for_revision(task_id).await?;
        
        Ok(())
    }
    
    /// Generate LLM prompt for validation
    fn generate_validation_prompt(&self, task: &crate::projects::Task) -> Result<String> {
        let deliverables = task.deliverables.join("\n");
        
        Ok(format!(
            "You are a Project Manager reviewing worker task completion.\n\n\
             Task: {}\n\
             Description: {}\n\n\
             Worker Deliverables:\n{}\n\n\
             Review the deliverables and determine:\n\
             1. Are all task requirements met?\n\
             2. Is the quality acceptable?\n\
             3. Are there any issues?\n\n\
             Return ONLY valid JSON:\n\
             {{\n\
               \"approved\": true/false,\n\
               \"feedback\": \"detailed feedback\",\n\
               \"revision_needed\": true/false\n\
             }}\n\n\
             Your response (JSON only):",
            task.title,
            task.description,
            deliverables
        ))
    }
    
    /// Parse LLM validation response
    fn parse_validation_response(&self, response: &str) -> Result<ValidationResponse> {
        // Extract JSON from markdown wrapper
        let json_str = if response.contains("```json") {
            response.split("```json").nth(1)
                .and_then(|s| s.split("```").next())
                .unwrap_or(response)
        } else {
            response
        }.trim();
        
        serde_json::from_str(json_str)
            .context(format!("Failed to parse validation response: {}", json_str))
    }
    
    /// Check if project is complete (all tasks terminal AND LLM assessment passes)
    async fn is_project_complete(&self) -> Result<bool> {
        let project_manager = self.project_manager.read().await;
        let tasks = project_manager.get_project_tasks(&self.project_id).await?;
        drop(project_manager);
        
        // 1. Check if all tasks are terminal
        let all_tasks_terminal = tasks.iter().all(|task| 
            matches!(task.status, crate::projects::TaskStatus::Complete | crate::projects::TaskStatus::Failed | crate::projects::TaskStatus::Stuck)
        );
        
        if !all_tasks_terminal {
            return Ok(false);
        }
        
        // 2. If all tasks terminal, ask LLM to assess completeness
        // This prevents premature completion if key files are missing
        let assessment = self.assess_project_completeness().await?;
        
        if assessment.complete {
            tracing::info!("PM {} assessment: Project COMPLETE. Reasoning: {}", self.id.name, assessment.reasoning);
            Ok(true)
        } else {
            tracing::info!("PM {} assessment: Project INCOMPLETE. Reasoning: {}", self.id.name, assessment.reasoning);
            
            // Create missing tasks
            // Note: We can't easily create tasks here because we need mutable self.
            // So we return false, and let manage_loop handle the assessment.
            // Ideally, manage_loop should call assess_project_completeness directly.
            Ok(false)
        }
    }

    /// List all files in the project sandbox recursively
    async fn list_files_recursive(&self, dir: &Path) -> Result<Vec<String>> {
        let mut files = Vec::new();
        if !dir.exists() {
            return Ok(files);
        }
        
        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                // Recurse
                let sub_files = Box::pin(self.list_files_recursive(&path)).await?;
                files.extend(sub_files);
            } else {
                // Store relative path if possible, or filename
                if let Ok(filename) = entry.file_name().into_string() {
                    // Try to get path relative to sandbox root
                    // For now just use the filename or partial path
                    // Better: pass root and strip prefix
                    files.push(path.to_string_lossy().to_string());
                }
            }
        }
        Ok(files)
    }

    /// Assess project completeness using LLM
    async fn assess_project_completeness(&self) -> Result<ProjectAssessment> {
        let project_manager = self.project_manager.read().await;
        let project = project_manager.get_project(&self.project_id).await?
            .ok_or_else(|| anyhow::anyhow!("Project not found"))?;
        
        let sandbox_path = crate::projects::ProjectManager::get_project_sandbox_path(&project.title);
        drop(project_manager);
        
        // Get file list (relative paths)
        let all_files = self.list_files_recursive(&sandbox_path).await?;
        let relative_files: Vec<String> = all_files.iter()
            .map(|f| f.replace(&sandbox_path.to_string_lossy().to_string(), "").trim_start_matches('/').to_string())
            .collect();
            
        let file_tree = relative_files.join("\n");
        
        let prompt = format!(
            r#"You are a Project Manager AI assessing if a project is complete.
            
PROJECT: {}
OVERVIEW: {}

CURRENT FILE STRUCTURE:
{}

INSTRUCTIONS:
1. Analyze the file structure against the project overview.
2. Determine if the project is functionally complete.
3. IGNORE minor missing files if the core functionality is implemented.
4. If incomplete, list specific missing tasks to finish it.
5. BE REASONABLE. Do not ask for "tests" if the project is a simple script, unless explicitly requested.
6. Do not ask for "docs" if a README exists.

RESPOND WITH VALID JSON ONLY:
{{
  "complete": boolean,
  "missing_tasks": [
    {{ "title": "Fix: ...", "description": "..." }}
  ],
  "reasoning": "..."
}}
"#,
            project.title,
            project.overview,
            file_tree
        );

        // Select model for assessment
        let context = SelectionContext {
            agent_type: crate::prompts::AgentType::PM,
            required_capabilities: vec![crate::ai_providers::catalog::ModelCapability::LogicalReasoning],
            preferred_capabilities: vec![crate::ai_providers::catalog::ModelCapability::TaskPlanning],
            max_latency_ms: 2000,
            min_context_length: 8192, // Need context for file tree
            preferred_model_size: crate::ai_providers::selection::ModelSizePreference::Small,
            requires_math: false,
            requires_coding: false,
            task_type: None,
            preferred_family: None,
            allow_fallback: true,
        };
        
        let selected_model = self.ai_provider_manager.select_model_for_agent(context).await?;
        let client = selected_model.get_client()?;
        
        // Strip provider prefix from model_id (e.g., "Ollama::model" -> "model")
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };
            
        let response = client.generate(model_name, &prompt, GenerationOptions::default()).await?;
        
        // Parse JSON
        let json_str = if response.text.contains("```json") {
            response.text.split("```json").nth(1)
                .and_then(|s| s.split("```").next())
                .unwrap_or(&response.text)
        } else {
            &response.text
        }.trim();
        
        serde_json::from_str(json_str)
            .context(format!("Failed to parse assessment response: {}", json_str))
    }



    /// Handle incoming message
    async fn handle_message(&mut self, message: crate::messaging::Message) -> Result<()> {
        tracing::info!("PM {} received message from {}: {:?}", self.id.name, message.from.name, message.content);
        
        match message.content {
            crate::messaging::MessageContent::TaskResult(result) => {
                tracing::info!("PM {} received task result for task {}", self.id.name, result.task_id);
                // Task completion is handled via DB status updates, but we could log extra info here
            },
            crate::messaging::MessageContent::StatusUpdate(update) => {
                tracing::info!("PM {} received status update from {}: {}", self.id.name, update.agent_id.name, update.message);
            },
            crate::messaging::MessageContent::ErrorReport(error) => {
            tracing::error!("PM {} received error from {}: {}", self.id.name, message.from.name, error.message);
        },
            crate::messaging::MessageContent::Query(query) => {
            tracing::info!("PM {} received query from {}: {}", self.id.name, message.from.name, query);
            // TODO: Implement query handling (e.g., ask Admin/User)
            // For now, we'll just log it.
        },
        crate::messaging::MessageContent::Response(response) => {
            tracing::info!("PM {} received response from {}: {}", self.id.name, message.from.name, response);
            // This would be where we unblock a task waiting for user input
        },
        _ => {
            tracing::debug!("PM {} received unhandled message type", self.id.name);
        }    }
        
        Ok(())
    }
    
    /// Complete project and transition to Idle
    async fn complete_project(&mut self) -> Result<()> {
        // Record project outcome for learning
        if let (Some(complexity), Some(strategy), Some(start_time)) = (
            &self.project_complexity,
            &self.selected_strategy,
            &self.project_start_time,
        ) {
            let duration = start_time.elapsed()
                .map(|d| d.as_secs())
                .unwrap_or(0);
            
            // Count total revisions across all tasks
            let project_manager = self.project_manager.read().await;
            let tasks = project_manager.get_project_tasks(&self.project_id).await?;
            let revision_count: usize = tasks.iter()
                .map(|t| t.revision_count as usize)
                .sum();
            
            drop(project_manager);
            
            self.learner.record_outcome(ProjectOutcome {
                project_id: self.project_id.to_string(),
                strategy: *strategy,
                complexity: complexity.clone(),
                success: true,
                duration_secs: duration,
                revision_count,
                timestamp: SystemTime::now(),
            });
            
            tracing::info!(
                "Recorded project outcome: strategy={:?}, duration={}s, revisions={}",
                strategy,
                duration,
                revision_count
            );
        }
        
        {
        let project_manager = self.project_manager.write().await;
        project_manager.complete_project(&self.project_id).await?;
    }
    
    // Notify Admin
    let tasks = {
        let pm = self.project_manager.read().await;
        pm.get_project_tasks(&self.project_id).await?
    };
    
    let failed_count = tasks.iter().filter(|t| matches!(t.status, crate::projects::TaskStatus::Failed)).count();
    let stuck_count = tasks.iter().filter(|t| matches!(t.status, crate::projects::TaskStatus::Stuck)).count();
    
    let message_content = if failed_count > 0 || stuck_count > 0 {
         format!("Project {} completed with issues: {} failed, {} stuck tasks.", self.project_id, failed_count, stuck_count)
    } else {
         format!("Project {} completed successfully.", self.project_id)
    };

        // Find the registered Admin agent
        let admin_id = {
            let bus = self.message_bus.read().await;
            let active_agents = bus.get_active_agents().await;
            
            active_agents.iter()
                .find(|agent| agent.id.agent_type == crate::messaging::AgentType::Admin)
                .map(|agent| agent.id.clone())
        };
        
        if let Some(admin_id) = admin_id {
            let message = crate::messaging::Message::new(
                self.id.clone(),
                admin_id,
                crate::messaging::MessageContent::StatusUpdate(crate::messaging::StatusUpdate {
                    agent_id: self.id.clone(),
                    state: AgentState::Idle,
                    message: message_content,
                    progress: Some(1.0),
                })
            );
            
            if let Err(e) = self.message_bus.write().await.send_message(message).await {
                tracing::error!("Failed to notify Admin of project completion: {:?}", e);
            } else {
                tracing::info!("PM {} notified Admin of project completion", self.id.name);
            }
        } else {
            tracing::warn!("No Admin agent registered - cannot send project completion notification");
        }


        self.state_machine.transition(
            AgentState::Idle,
            "Project completed successfully".to_string()
        )?;
        
        Ok(())
    }
    
    /// Generate detailed plan using LLM with discovery-based prompting (NEW)
    async fn generate_detailed_plan_with_discovery(
        &self,
        project: &crate::projects::Project,
        existing_tasks: &[crate::projects::Task],
        strategy: DecompositionStrategy,
    ) -> Result<DetailedPlan> {
        // Use modular prompt template from pm/planning.toml
        let strategy_guidance = match strategy {
            DecompositionStrategy::Sequential => 
                "Tasks must complete in order. Each depends on previous.",
            DecompositionStrategy::Parallel => 
                "Tasks are independent. No dependencies.",
            DecompositionStrategy::Hybrid => 
                "Mix sequential and parallel. Some tasks can run together, others must wait.",
        };
        
        let existing_tasks_formatted = existing_tasks.iter()
            .enumerate()
            .map(|(i, t)| format!("{}. {}", i + 1, t.description))
            .collect::<Vec<_>>()
            .join("\n");
        
        // Compact planning prompt using discovery principles
        let planning_prompt = format!(
            r#"You are a PM breaking down a project into tasks.

PROJECT: {}
OVERVIEW: {}

EXISTING TASKS:
{}

STRATEGY: {}

WORKERS AVAILABLE:
- FileWorker (file operations)
- CodeWorker (code analysis, compilation)
- NetworkWorker (API calls, network ops)
- ResearchWorker (documentation, research)

SESSION PROGRESS:
{}

INSTRUCTIONS:
1. Create specific, actionable tasks
2. Assign appropriate worker type
3. Define clear dependencies
4. Keep titles under 60 chars

RESPOND WITH VALID JSON ONLY (no markdown):
{{
  "tasks": [
    {{
      "title": "Task title",
      "description": "Detailed description",
      "worker_type": "FileWorker"
    }}
  ],
  "dependencies": [
    {{
      "task_index": 1,
      "depends_on": [0]
    }}
  ]
}}

CRITICAL: JSON only. No explanations.
"#,
            project.title,
            project.overview,
            existing_tasks_formatted,
            strategy_guidance,
            self.session_tasks.to_prompt_format()
        );
        
        // Select model with user preferences
        let mut selection_context = SelectionContext::for_pm();
        
        // Apply user's preferred model family if available
        if let Some(ref user_settings) = self.user_settings {
            let settings = user_settings.read().await;
            if let Ok(Some(family)) = settings.get_model_preference("PM").await {
                tracing::info!("PM {} using user-preferred model family: {}", self.id.name, family);
                selection_context.preferred_family = Some(family);
            }
        }
        
        let selected_model = self.ai_provider_manager
            .select_model_for_agent(selection_context)
            .await
            .context("Failed to select a model for PM planning")?;
        
        let options = GenerationOptions {
            temperature: Some(0.5),
            max_tokens: Some(4096),
            ..Default::default()
        };
        
        let client = selected_model.get_client()?;
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };
        
        let response = client.generate(model_name, &planning_prompt, options)
            .await
            .context("Failed to generate PM plan")?;
        
        self.parse_detailed_plan(&response.text)
    }
    
    /// Generate detailed plan using LLM with strategy-aware prompting (LEGACY)
    async fn generate_detailed_plan_with_strategy(
        &self,
        project: &crate::projects::Project,
        existing_tasks: &[crate::projects::Task],
        strategy: DecompositionStrategy,
    ) -> Result<DetailedPlan> {
        let mut prompt_manager = self.prompt_manager.write().await;
        let mut prompt_context = PromptContext::default();
        prompt_context.variables.insert("project_title".to_string(), serde_json::json!(project.title));
        prompt_context.variables.insert("project_overview".to_string(), serde_json::json!(project.overview));
        prompt_context.variables.insert("existing_tasks".to_string(), 
            serde_json::json!(existing_tasks.iter().map(|t| &t.description).collect::<Vec<_>>()));
        
        let prompt_agent_id = crate::prompts::types::AgentId::new(
            self.id.agent_type,
            self.id.name.clone()
        );
        
        let system_prompt = prompt_manager.get_prompt(
            &prompt_agent_id,
            AgentState::Planning,
            &prompt_context
        ).await?;
        
        drop(prompt_manager);
        
        // Compact, clear prompt optimized for local small-to-medium LLMs
        let strategy_guidance = match strategy {
            DecompositionStrategy::Sequential => 
                "Tasks must complete in order. Each depends on previous.",
            DecompositionStrategy::Parallel => 
                "Tasks are independent. No dependencies.",
            DecompositionStrategy::Hybrid => 
                "Mix sequential and parallel. Some tasks can run together, others must wait.",
        };
        
        let planning_prompt = format!(
            "Break down project into executable tasks.\n\n\
             PROJECT: {}\n\
             OVERVIEW: {}\n\
             INITIAL TASKS:\n{}\n\n\
             STRATEGY: {}\n\n\
             WORKERS: FileWorker (files), CodeWorker (code), NetworkWorker (APIs), ResearchWorker (docs)\n\n\
             RULES:\n\
             - Specific, actionable tasks\n\
             - Clear titles (max 60 chars)\n\
             - Detailed descriptions\n\
             - List dependencies (0-based indices)\n\n\
             OUTPUT (JSON only):\n\
             {{\n\
               \"tasks\": [{{\"title\": \"...\", \"description\": \"...\", \"worker_type\": \"...\"}}],\n\
               \"dependencies\": [{{\"task_index\": 1, \"depends_on\": [0]}}]\n\
             }}",
            project.title,
            project.overview,
            existing_tasks.iter()
                .enumerate()
                .map(|(i, t)| format!("{}. {}", i + 1, t.description))
                .collect::<Vec<_>>()
                .join("\n"),
            strategy_guidance
        );
        
        // Select the best model for planning using AIProviderManager
        // Select model with user preferences
        let mut selection_context = SelectionContext::for_pm();
        
        // Apply user's preferred model family if available
        if let Some(ref user_settings) = self.user_settings {
            let settings = user_settings.read().await;
            if let Ok(Some(family)) = settings.get_model_preference("PM").await {
                tracing::debug!("PM {} using user-preferred model family: {}", self.id.name, family);
                selection_context.preferred_family = Some(family);
            }
        }
        
        // Load user preference for PM agent if available
        let preferred_family = if let Some(ref user_settings) = self.user_settings {
            let settings = user_settings.read().await;
            match settings.get_model_preference("pm").await {
                Ok(Some(family)) => {
                    tracing::info!("✅ Loaded user preference for PM: family='{}'", family);
                    Some(family)
                },
                Ok(None) => {
                    tracing::warn!("⚠️  No user preference set for PM agent");
                    None
                },
                Err(e) => {
                    tracing::error!("❌ Failed to load user preference for PM: {:?}", e);
                    None
                }
            }
        } else {
            tracing::warn!("⚠️  UserSettingsManager not available in context");
            None
        };
        
        tracing::info!("🎯 Model selection for PM planning: preferred_family={:?}", preferred_family);
        
        let selected_model = self.ai_provider_manager
            .select_model_for_agent_with_preferences(selection_context, preferred_family)
            .await
            .context("Failed to select a model for planning")?;
        
        let options = GenerationOptions {
            temperature: Some(0.7),
            max_tokens: Some(4096),
            system: Some(system_prompt),
            ..Default::default()
        };
        
        let client = selected_model.get_client()?;
        // Strip provider prefix from model_id (e.g., "Ollama::model" -> "model")
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };
        
        // Add timeout wrapper to prevent indefinite hanging
        tracing::info!("[DIAGNOSTIC] PM {} calling LLM for planning (model: {})", self.id.name, model_name);
        let llm_timeout = tokio::time::Duration::from_secs(300); // 300s timeout for LLM generation
        let response = tokio::time::timeout(
            llm_timeout,
            client.generate(model_name, &planning_prompt, options)
        )
        .await
        .context(format!("LLM planning timed out after {:?}", llm_timeout))?
        .context("Failed to generate detailed plan with LLM")?;
        
        tracing::debug!(
            target: "llm_messages",
            "[PM PLANNING RESPONSE] Model: {}, Response ({} chars):\n{}",
            model_name,
            response.text.len(),
            response.text
        );

        self.parse_detailed_plan(&response.text)
    }
    
    /// Parse LLM response into DetailedPlan using multi-strategy JSON parsing
    fn parse_detailed_plan(&self, llm_response: &str) -> Result<DetailedPlan> {
        tracing::debug!("PM parsing detailed plan from LLM response");
        
        // Use multi-strategy JSON parser
        let parse_result = JSONValidator::parse_with_fallbacks(llm_response);
        
        let parsed = match parse_result.value {
            Some(val) => {
                tracing::info!("Successfully parsed PM plan JSON using strategy: {}", parse_result.strategy_used);
                // Log the parsed JSON structure for debugging
                tracing::debug!("Parsed JSON structure: {:?}", val);
                val
            },
            None => {
                tracing::error!("All JSON parsing strategies failed: {}", 
                               parse_result.error.unwrap_or_else(|| "Unknown error".to_string()));
                return Err(anyhow::anyhow!("Failed to parse detailed plan as valid JSON"));
            }
        };
        
        // Log available keys for debugging
        if let Some(obj) = parsed.as_object() {
            tracing::debug!("Available JSON keys: {:?}", obj.keys().collect::<Vec<_>>());
        }
        
        let tasks: Vec<TaskDetail> = parsed["tasks"].as_array()
            .ok_or_else(|| anyhow::anyhow!("Missing 'tasks' array. Parsed JSON: {:?}", parsed))?
            .iter()
            .map(|t| {
                Ok(TaskDetail {
                    title: t["title"].as_str().unwrap_or("Untitled Task").to_string(),
                    description: t["description"].as_str().unwrap_or("No description").to_string(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        
        let dependencies: Vec<TaskDependency> = parsed["dependencies"].as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .filter_map(|d| {
                let task_index = d["task_index"].as_u64()? as usize;
                let depends_on: Vec<usize> = d["depends_on"].as_array()?
                    .iter()
                    .filter_map(|i| i.as_u64().map(|v| v as usize))
                    .filter(|&dep_idx| {
                        if dep_idx == task_index {
                            tracing::warn!("Ignoring self-dependency for task index {}", task_index);
                            false
                        } else {
                            true
                        }
                    })
                    .collect();
                Some(TaskDependency { task_index, depends_on })
            })
            .collect();
        
        tracing::info!("PM parsed detailed plan: {} tasks, {} dependencies (strategy: {})", 
                      tasks.len(), dependencies.len(), parse_result.strategy_used);
        
        Ok(DetailedPlan { tasks, dependencies })
    }
    
    /// Assign task to a worker (reusing existing or spawning new)
    async fn assign_task_to_worker(&mut self, task_id: &TaskId) -> Result<()> {
        // Get task details - scope ends here, releasing lock
        let (task_title, task_description) = {
            let pm = self.project_manager.read().await;
            let tasks = pm.get_project_tasks(&self.project_id).await?;
            let task = tasks.into_iter()
                .find(|t| &t.id == task_id)
                .ok_or_else(|| anyhow::anyhow!("Task not found"))?;
            
            let title = task.title.clone();
            let description = task.description.clone();
            (title, description)
        }; 
        
        // Update session task status to in progress
        let session_task_title = if task_title.len() > 50 {
            format!("{}...", &task_title[..47])
        } else {
            task_title.clone()
        };
        let _ = self.session_tasks.update_status(&session_task_title, SessionTaskStatus::InProgress);
        
        // Select appropriate worker template based on task description
        let template = WorkerTemplate::select_for_task(&task_description);
        let template_name = template.name.clone();
        
        tracing::info!("Assigning task '{}' to worker type '{}'", task_title, template_name);

        // Check if we have an active worker for this template
    if let Some((worker_id, tx)) = self.active_workers.get(&template_name) {
        tracing::info!("Reusing worker {} for task {}", worker_id.name, task_title);
        
        // Mark task as assigned BEFORE sending to channel to prevent duplicate sends
        {
            let project_manager = self.project_manager.write().await;
            project_manager.assign_task(task_id, worker_id.clone()).await?;
        }
        
        // Update tracking
        self.workers.insert(task_id.clone(), worker_id.clone());
        
        // Send task
        if let Err(_) = tx.send(task_id.clone()).await {
            tracing::warn!("Worker channel closed for {}, spawning new one", template_name);
            self.active_workers.remove(&template_name);
            // Recursively retry assignment (will spawn new worker)
            // We need to Box::pin because async recursion
            return Box::pin(self.assign_task_to_worker(task_id)).await;
        }
        
        return Ok(());
    }

        // Spawn new worker if none exists
        tracing::info!("Spawning new {} for task: {}", template_name, task_title);
        
        // Create worker agent with proper template
        let mut worker = super::worker::WorkerAgent::from_template(
            template,
            self.message_bus.clone(),
            self.prompt_manager.clone(),
            self.project_manager.clone(),
            self.mcp_client.clone(),
            self.ai_provider_manager.clone(),
            self.user_settings.clone(),
        );
        
        let worker_id = worker.id().clone();
        
        // Transition worker to Idle state
        worker.state_machine_mut().transition(
            AgentState::Idle,
            "Worker initialized and ready for task loop".to_string()
        )?;
        
        // Create channel for task distribution
        let (tx, rx) = mpsc::channel(100);
        
        // Store worker mapping and channel
        self.workers.insert(task_id.clone(), worker_id.clone());
        self.active_workers.insert(template_name.clone(), (worker_id.clone(), tx.clone()));
        
        // Mark task as assigned BEFORE sending to channel to prevent duplicate sends
        {
            let project_manager = self.project_manager.write().await;
            project_manager.assign_task(task_id, worker_id.clone()).await?;
        }
        
        // Execute worker run loop in background
        let worker_name = worker.id().name.clone();
        let worker_name_clone = worker_name.clone();
        tokio::spawn(async move {
            worker.run(rx).await;
            tracing::info!("Worker {} run loop terminated", worker_name_clone);
        });
        
        // Send the first task
        if let Err(e) = tx.send(task_id.clone()).await {
             tracing::error!("Failed to send initial task to new worker {}: {:?}", worker_name, e);
             return Err(anyhow::anyhow!("Failed to send task to new worker"));
        }
        
        Ok(())
    }
    
    /// Handle error and transition to Error state
    pub fn handle_error(&mut self, error: String) {
        self.state_machine.force_error(error);
    }
    
    /// Get reference to workers (for testing)
    pub fn workers(&self) -> &HashMap<TaskId, AgentId> {
        &self.workers
    }
    
    /// Get reference to task graph (for testing)
    pub fn task_graph(&self) -> Option<&TaskGraph> {
        self.task_graph.as_ref()
    }
}

#[async_trait::async_trait]
impl super::Agent for PMAgent {
    fn id(&self) -> &AgentId {
        &self.id
    }

    async fn start(&mut self) -> Result<()> {
        // This is now the official start method
        // The existing `start` method will be renamed to `initialize_and_plan`
        // and called from here.
        self.initialize_and_plan().await
    }

    async fn stop(&mut self) -> Result<()> {
        tracing::info!("PM agent {} stopping.", self.id.name);
        self.state_machine.transition(
            AgentState::Idle,
            "Agent stopped externally".to_string()
        )?;
        Ok(())
    }

    async fn process_message(&mut self, message: crate::messaging::Message) -> Result<()> {
        // PM agents can receive messages from Admin or Workers
        // This is a placeholder for more complex message handling
        tracing::info!("PM agent {} received message: {:?}", self.id.name, message.content);
        Ok(())
    }
}

/// Detailed task plan from LLM
#[derive(Debug, Clone)]
struct DetailedPlan {
    tasks: Vec<TaskDetail>,
    dependencies: Vec<TaskDependency>,
}

/// Task detail from LLM
#[derive(Debug, Clone)]
struct TaskDetail {
    title: String,
    description: String,
}

/// Task dependency (public for testing)
#[derive(Debug, Clone)]
pub struct TaskDependency {
    pub task_index: usize,
    pub depends_on: Vec<usize>,
}

/// Validation response from LLM
#[derive(Debug, Clone, serde::Deserialize)]
struct ValidationResponse {
    approved: bool,
    feedback: String,
    pub revision_needed: bool,
}

/// Project assessment from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAssessment {
    pub complete: bool,
    pub missing_tasks: Vec<MissingTask>,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingTask {
    pub title: String,
    pub description: String,
}

/// Task dependency graph (DAG)
#[derive(Debug, Clone)]
pub struct TaskGraph {
    /// All tasks in the graph
    pub tasks: HashMap<TaskId, crate::projects::Task>,
    
    /// Dependencies: task_id -> list of task_ids it depends on
    pub dependencies: HashMap<TaskId, Vec<TaskId>>,
}

impl TaskGraph {
    /// Build dependency graph from tasks
    pub fn build(
        tasks: Vec<crate::projects::Task>,
        dependencies: Vec<TaskDependency>,
    ) -> Result<Self> {
        let mut task_map = HashMap::new();
        let mut dep_map = HashMap::new();
        
        // Build task map
        for task in &tasks {
            task_map.insert(task.id.clone(), task.clone());
        }
        
        // Build dependency map
        for dep in dependencies {
            if dep.task_index < tasks.len() {
                let task_id = tasks[dep.task_index].id.clone();
                let depends_on: Vec<TaskId> = dep.depends_on
                    .into_iter()
                    .filter(|&idx| idx < tasks.len())
                    .filter(|&idx| idx != dep.task_index) // Prevent self-dependency
                    .map(|idx| tasks[idx].id.clone())
                    .collect();
                dep_map.insert(task_id, depends_on);
            }
        }
        
        Ok(Self {
            tasks: task_map,
            dependencies: dep_map,
        })
    }
    
    /// Check if a task can be executed (all dependencies met)
    pub fn can_execute(&self, task_id: &TaskId, completed: &HashSet<TaskId>) -> bool {
        if let Some(deps) = self.dependencies.get(task_id) {
            deps.iter().all(|dep_id| completed.contains(dep_id))
        } else {
            // No dependencies, can execute
            true
        }
    }
    
    /// Get topological sort of tasks (execution order)
    pub fn topological_sort(&self) -> Result<Vec<TaskId>> {
        let mut sorted = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_mark = HashSet::new();
        
        for task_id in self.tasks.keys() {
            if !visited.contains(task_id) {
                self.visit(task_id, &mut visited, &mut temp_mark, &mut sorted)?;
            }
        }
        
        sorted.reverse();
        Ok(sorted)
    }
    
    /// DFS visit for topological sort
    fn visit(
        &self,
        task_id: &TaskId,
        visited: &mut HashSet<TaskId>,
        temp_mark: &mut HashSet<TaskId>,
        sorted: &mut Vec<TaskId>,
    ) -> Result<()> {
        if temp_mark.contains(task_id) {
            return Err(anyhow::anyhow!("Circular dependency detected"));
        }
        
        if visited.contains(task_id) {
            return Ok(());
        }
        
        temp_mark.insert(task_id.clone());
        
        if let Some(deps) = self.dependencies.get(task_id) {
            for dep_id in deps {
                self.visit(dep_id, visited, temp_mark, sorted)?;
            }
        }
        
        temp_mark.remove(task_id);
        visited.insert(task_id.clone());
        sorted.push(task_id.clone());
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::Agent;
    
    async fn create_test_pm() -> PMAgent {
        let message_bus = Arc::new(RwLock::new(MessageBus::new().await.unwrap()));
        let prompt_manager = Arc::new(RwLock::new(PromptManager::new("prompts".into()).unwrap()));
        let project_manager = Arc::new(RwLock::new(
            ProjectManager::new("sqlite::memory:").await.unwrap()
        ));
        let ai_provider_manager = Arc::new(AIProviderManager::new().await.unwrap());
        let mcp_client = Arc::new(RwLock::new(crate::tools::mcp::MCPClientManager::new()));
        
        let project_id = ProjectId::new();
        PMAgent::new(project_id, message_bus, prompt_manager, project_manager, ai_provider_manager, mcp_client, None)
    }
    
    #[tokio::test]
    async fn test_pm_creation() {
        let pm = create_test_pm().await;
        assert_eq!(pm.state(), &AgentState::Startup);
    }
    
    async fn check_ollama_gemma3() -> (bool, Vec<String>) {
        let ai_provider_manager = match AIProviderManager::new().await {
            Ok(manager) => manager,
            Err(_) => return (false, vec![]),
        };

        let catalog = ai_provider_manager.get_stats().await;
        if catalog.total_models == 0 {
            return (false, vec![]);
        }

        let all_models = {
            let catalog_lock = ai_provider_manager.catalog.read().await;
            catalog_lock.all_models().into_iter().map(|m| m.clone()).collect::<Vec<_>>()
        };

        let gemma3_models: Vec<String> = all_models.iter()
            .filter(|m| m.provider_type == crate::ai_providers::discovery::ProviderType::Ollama && m.name.contains("gemma3"))
            .map(|m| m.name.clone())
            .collect();

        (!gemma3_models.is_empty(), gemma3_models)
    }

    #[tokio::test]
    async fn test_pm_startup_transition() {
        let (ollama_ok, _) = check_ollama_gemma3().await;
        if !ollama_ok {
            println!("⚠️ Skipping test: Ollama not running");
            return;
        }

        let message_bus = Arc::new(RwLock::new(MessageBus::new().await.unwrap()));
        let prompt_manager = Arc::new(RwLock::new(PromptManager::new("prompts".into()).unwrap()));
        let project_manager = Arc::new(RwLock::new(
            ProjectManager::new("sqlite::memory:").await.unwrap()
        ));

        // Create project first
        let project_id = {
            let pm_mgr = project_manager.write().await;
            pm_mgr.create_project(
                "Test Project".to_string(),
                "Test project for PM agent".to_string(),
                vec!["Task 1".to_string()],
            ).await.unwrap()
        };

        let ai_provider_manager = Arc::new(AIProviderManager::new().await.unwrap());
        let mcp_client = Arc::new(RwLock::new(crate::tools::mcp::MCPClientManager::new()));
        let mut pm = PMAgent::new(project_id, message_bus, prompt_manager, project_manager, ai_provider_manager, mcp_client, None);
        
        pm.start().await.unwrap();
        assert_eq!(pm.state(), &AgentState::Managing);
    }
}
