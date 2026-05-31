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
use super::failover::{FailoverHandler, ModelEndpoint};
use super::loop_detector;
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
    
    /// Failover handler for model/endpoint tracking (ported from TE)
    failover_handler: FailoverHandler,
    
    /// Context manager for history bounds (ported from TE)
    context_manager: super::context_manager::ContextManager,
    
    /// Message receiver (kept alive to maintain registration)
    _receiver: Option<mpsc::Receiver<crate::messaging::Message>>,
    
    /// TrippleEffect state name (finer-grained than AgentState, used for prompt selection)
    te_state_name: String,
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
            failover_handler: FailoverHandler::new(),
            context_manager: super::context_manager::ContextManager::new(8192),
            _receiver: None,
            te_state_name: String::new(),
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

    /// Helper to generate text using the AI provider manager with failover, loop detection, and context limits.
    /// (Ported from TrippleEffect's cycle_engine pattern)
    async fn generate_llm_response(
        &mut self,
        prompt: &str,
        options: Option<GenerationOptions>,
        selection_context: SelectionContext,
        preferred_family: Option<String>,
        timeout_ms: u64,
        task_name: &str,
    ) -> Result<String> {
        let max_attempts = 3;
        let mut last_error = anyhow::anyhow!("Unknown error");
        
        // Context Management: Truncate prompt if it exceeds safety bounds
        let estimated_tokens = prompt.len() / 4;
        let final_prompt = if estimated_tokens > 8000 {
            tracing::warn!("PM prompt exceeds token estimate ({} tokens). Truncating.", estimated_tokens);
            let keep_len = (8000 * 4) / 2;
            if prompt.len() > keep_len * 2 {
                format!("{}... [TRUNCATED DUE TO CONTEXT LIMIT] ...{}", &prompt[..keep_len], &prompt[prompt.len()-keep_len..])
            } else {
                prompt.to_string()
            }
        } else {
            prompt.to_string()
        };
        
        for attempt in 1..=max_attempts {
            let selected_model = self.ai_provider_manager.select_model_for_agent_with_preferences(
                selection_context.clone(), 
                preferred_family.clone()
            ).await?;
            
            let client = selected_model.get_client()?;
            let model_name = if selected_model.model_id.contains("::") {
                selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
            } else {
                &selected_model.model_id
            };
            
            tracing::info!("[DIAGNOSTIC] PM {} calling LLM for {} (model: {}, attempt: {})", self.id.name, task_name, model_name, attempt);
            let llm_timeout = tokio::time::Duration::from_millis(timeout_ms);
            
            match tokio::time::timeout(
                llm_timeout,
                client.generate(model_name, &final_prompt, options.clone().unwrap_or_default())
            ).await {
                Ok(Ok(response)) => {
                    if let Some(pattern_len) = loop_detector::detect_autoregressive_loop(&response.text) {
                        tracing::warn!("Autoregressive loop detected in PM {} output (pattern len: {})", task_name, pattern_len);
                        return Ok(format!("{}\n[Framework Watchdog Intervention]: You have entered an autoregressive loop. Please stop repeating yourself and re-evaluate.", response.text));
                    }
                    return Ok(response.text);
                },
                Ok(Err(e)) => {
                    tracing::warn!("PM LLM generation error on attempt {}: {}", attempt, e);
                    let endpoint = ModelEndpoint {
                        provider: selected_model.provider_type.to_string(),
                        model: selected_model.model_id.clone(),
                        api_key_id: None,
                    };
                    let err_str = e.to_string().to_lowercase();
                    if err_str.contains("401") || err_str.contains("403") || err_str.contains("429") {
                        self.failover_handler.report_key_failure(&endpoint, &err_str);
                    } else {
                        self.failover_handler.report_transient_failure(&endpoint, &err_str);
                    }
                    last_error = e;
                },
                Err(e) => {
                    tracing::warn!("PM LLM generation timed out on attempt {}: {}", attempt, e);
                    last_error = anyhow::anyhow!("Timeout: {}", e);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(1000 * attempt as u64)).await;
        }
        
        Err(last_error).context(format!("PM failed to generate response after {} attempts for {}", max_attempts, task_name))
    }

    /// Start PM agent lifecycle
    /// 
    /// Startup → Idle → Planning → Managing
    pub async fn initialize_and_plan(&mut self) -> Result<()> {
        // Record project start time
        self.project_start_time = Some(SystemTime::now());
        
        // Register with MessageBus
        let (receiver, _) = self.message_bus.write().await
            .register_agent(self.id.clone())
            .await
            .context("Failed to register PM agent with MessageBus")?;
        self._receiver = Some(receiver);

        tracing::info!("PM {} registered with MessageBus, entering autonomous cycle", self.id.name);
        
        // The autonomous cycle starts in Startup and drives all transitions itself
        // (startup -> planning/build_team -> activate_workers -> manage -> audit -> standby)
        self.execute_autonomous_cycle().await
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
        let mut cycle_count = 0;
        const MAX_PM_CYCLES: u32 = 1000;
        loop {
            cycle_count += 1;
            if cycle_count > MAX_PM_CYCLES {
                tracing::error!("PM {} exceeded maximum cycle count ({}) - forcing loop termination", self.id.name, MAX_PM_CYCLES);
                break;
            }
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
            
            // Handle stuck tasks - attempt recovery or permanent failure
            self.handle_stuck_tasks().await?;
            
            // PRIORITY 1: Poll pending validations (non-blocking)
            self.poll_pending_validations().await?;
            
            // PRIORITY 2: Timeout stale validations (>60s)
            self.timeout_stale_validations().await?;
            
            // PRIORITY 3: Check for completed tasks needing validation
            let tasks_under_review = self.get_tasks_under_review().await?;
            
            if !tasks_under_review.is_empty() {
                let _ = self.state_machine.transition(
                    AgentState::Reviewing, 
                    "Reviewing worker reports".to_string()
                );
                self.update_status("Reviewing worker reports").await;
                
                tracing::info!(
                    "PM {} found {} tasks under review",
                    self.id.name,
                    tasks_under_review.len()
                );
                
                // Spawn async validations for tasks under review (non-blocking)
                for task_id in tasks_under_review {
                    self.spawn_validation(task_id)?;
                }
                
                let _ = self.state_machine.transition(
                    AgentState::Managing, 
                    "Finished spawning validations".to_string()
                );
                self.update_status("Managing project execution").await;
            }
            
            // PRIORITY 4: Check for tasks needing revision
            let tasks_needing_revision = self.get_tasks_needing_revision().await?;
            
            tracing::info!(
                "PM {} found {} tasks needing revision",
                self.id.name,
                tasks_needing_revision.len()
            );
            
            for task_id in tasks_needing_revision {
                self.handle_revision_task(&task_id).await?;
            }
            
            // PRIORITY 5: Get executable tasks (unassigned + dependencies met)
            let executable_tasks = self.get_executable_tasks().await?;
            
            tracing::info!(
                "PM {} found {} executable tasks",
                self.id.name,
                executable_tasks.len()
            );
            
            // Spawn workers and assign tasks
            for task_id in executable_tasks {
                self.assign_task_to_worker(&task_id, None).await?;
            }
            
            // Check if all tasks are complete
            if self.is_project_complete().await? {
                let _ = self.state_machine.transition(
                    AgentState::Auditing, 
                    "Auditing project completeness".to_string()
                );
                self.update_status("Auditing final project deliverables").await;
                
                // Double check with LLM assessment
                let assessment = self.assess_project_completeness().await?;
                
                if assessment.complete {
                    let _ = self.state_machine.transition(
                        AgentState::Idle, 
                        "Project complete".to_string()
                    );
                    tracing::info!("PM {} assessment: Project COMPLETE. Reasoning: {}", self.id.name, assessment.reasoning);
                    self.complete_project().await?;
                    break;
                } else {
                    let _ = self.state_machine.transition(
                        AgentState::Managing, 
                        "Project incomplete, missing tasks".to_string()
                    );
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
                let project_manager = self.project_manager.write().await;
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
                        
                        let project_manager = self.project_manager.write().await;
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
    
    /// Handle stuck tasks - attempt recovery or permanent failure
    /// This method runs after detect_stuck_tasks() to process tasks that have been marked as Stuck
    async fn handle_stuck_tasks(&mut self) -> Result<()> {
        let project_manager = self.project_manager.read().await;
        let tasks = project_manager.get_project_tasks(&self.project_id).await?;
        drop(project_manager);
        
        for task in tasks {
            if task.status != crate::projects::TaskStatus::Stuck {
                continue;
            }
            
            // Check if task can be retried
            if task.can_retry_stuck() {
                tracing::info!(
                    "PM {} attempting to recover stuck task: {} ({}) - retry {}/{}",
                    self.id.name,
                    task.id,
                    task.title,
                    task.stuck_retry_count + 1,
                    task.max_stuck_retries
                );
                
                // Reset task to Unassigned for reassignment
                let project_manager = self.project_manager.write().await;
                project_manager.reset_stuck_task(&task.id).await?;
                
                tracing::info!(
                    "PM {} reset stuck task {} to Unassigned for retry",
                    self.id.name,
                    task.id
                );
            } else {
                // Max retries exceeded - permanently fail the task
                tracing::error!(
                    "PM {} permanently failing stuck task: {} ({}) - max retries ({}) exceeded",
                    self.id.name,
                    task.id,
                    task.title,
                    task.max_stuck_retries
                );
                
                let failure_reason = format!(
                    "Task failed after {} stuck retry attempts. Last failure: {}",
                    task.max_stuck_retries,
                    task.failure_reason.as_ref().unwrap_or(&"Unknown".to_string())
                );
                
                let project_manager = self.project_manager.write().await;
                project_manager.fail_task(&task.id, failure_reason.clone()).await?;
                
                // ESCALATION: Notify Admin of permanent failure
                let error_report = crate::messaging::ErrorReport {
                    error_type: "TaskPermanentFailure".to_string(),
                    message: format!("Task '{}' ({}) permanently failed: {}", task.title, task.id, failure_reason),
                    stack_trace: None,
                    recoverable: false,
                };
                
                let msg = crate::messaging::Message::new(
                    self.id.clone(),
                    crate::messaging::AgentId::new_admin("main-admin".to_string()),
                    crate::messaging::MessageContent::ErrorReport(error_report)
                ).with_priority(crate::messaging::Priority::High);
                
                if let Err(e) = self.message_bus.write().await.send_message(msg).await {
                    tracing::error!("Failed to send error report to Admin: {:?}", e);
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
        // Call LLM for validation decision using centralized failover helper
        let options = GenerationOptions {
            temperature: Some(0.3),
            max_tokens: Some(4096),
            ..Default::default()
        };

        let response_text = self.generate_llm_response(
            &prompt,
            Some(options),
            selection_context,
            None,
            300_000, // 300s timeout for validation
            "validation"
        ).await.context("Failed to validate task with LLM")?;
        
        tracing::debug!(
            target: "llm_messages",
            "[PM VALIDATION RESPONSE] Response ({} chars):\n{}",
            response_text.len(),
            response_text
        );
        
        // Parse validation decision
        let validation = self.parse_validation_response(&response_text)?;
        
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
            r#"Review worker output.
TASK: {}
DESCRIPTION: {}

WORKER DELIVERABLES:
{}

Evaluate if deliverables meet requirements.

Return JSON:
{{
  "approved": true,
  "feedback": "your feedback here",
  "revision_needed": false
}}"#,
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
            "Review worker task completion.
Task: {}
Description: {}

Worker Deliverables:
{}

Evaluate requirements, quality, and issues.

Return JSON:
{{
  \"approved\": true/false,
  \"feedback\": \"detailed feedback\",
  \"revision_needed\": true/false
}}",
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
    async fn is_project_complete(&mut self) -> Result<bool> {
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
    async fn assess_project_completeness(&mut self) -> Result<ProjectAssessment> {
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
        
        let response_text = self.generate_llm_response(
            &prompt,
            None,
            context,
            None,
            120_000,
            "project assessment"
        ).await?;
        
        // Parse JSON
        let json_str = if response_text.contains("```json") {
            response_text.split("```json").nth(1)
                .and_then(|s| s.split("```").next())
                .unwrap_or(&response_text)
        } else {
            &response_text
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
        &mut self,
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
        
        let mut selection_context = SelectionContext::for_pm();
        let mut preferred_family_opt = None;
        
        // Apply user's preferred model family if available
        if let Some(ref user_settings) = self.user_settings {
            let settings = user_settings.read().await;
            if let Ok(Some(family)) = settings.get_model_preference("PM").await {
                tracing::info!("PM {} using user-preferred model family: {}", self.id.name, family);
                selection_context.preferred_family = Some(family.clone());
                preferred_family_opt = Some(family);
            }
        }
        
        let options = GenerationOptions {
            temperature: Some(0.5),
            max_tokens: Some(4096),
            ..Default::default()
        };
        
        let response_text = self.generate_llm_response(
            &planning_prompt,
            Some(options),
            selection_context,
            preferred_family_opt,
            120_000,
            "dynamic plan creation"
        ).await.context("Failed to generate PM plan")?;
        
        self.parse_detailed_plan(&response_text)
    }
    
    /// Generate detailed plan using LLM with strategy-aware prompting (LEGACY)
    async fn generate_detailed_plan_with_strategy(
        &mut self,
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
             Return JSON:\n\
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
        
        let options = GenerationOptions {
            temperature: Some(0.7),
            max_tokens: Some(4096),
            system: Some(system_prompt),
            ..Default::default()
        };
        
        let response_text = self.generate_llm_response(
            &planning_prompt,
            Some(options),
            selection_context,
            preferred_family,
            300_000, // 300s timeout for planning
            "detailed plan generation"
        ).await.context("Failed to generate detailed plan with LLM")?;
        
        tracing::debug!(
            target: "llm_messages",
            "[PM PLANNING RESPONSE] Response ({} chars):\n{}",
            response_text.len(),
            response_text
        );

        self.parse_detailed_plan(&response_text)
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
    async fn assign_task_to_worker(&mut self, task_id: &TaskId, preferred_template: Option<&str>) -> Result<()> {
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
        
        // Select appropriate worker template based on task description or explicit preference
        let template = if let Some(pref) = preferred_template {
            let templates = WorkerTemplate::all_templates();
            if let Some(t) = templates.into_iter().find(|t| t.name.to_lowercase() == pref.to_lowercase()) {
                t
            } else {
                WorkerTemplate::select_for_task(&task_description)
            }
        } else {
            WorkerTemplate::select_for_task(&task_description)
        };
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
            return Box::pin(self.assign_task_to_worker(task_id, preferred_template)).await;
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

    // =========================================================================
    // TrippleEffect Continuous Cycle Engine (PM)
    // =========================================================================

    /// Load a prompt template from TrippleEffect's prompts.yaml by key name.
    /// Load a prompt from the TrippleEffect prompts.yaml (via shared prompt_loader).
    fn get_te_prompt(&self, prompt_name: &str) -> Option<String> {
        super::prompt_loader::get_prompt(prompt_name)
    }

    /// Extract JSON from a raw LLM response (handles markdown fences and bare braces)
    fn extract_json_from_response(&self, response: &str) -> String {
        // Strip <think>...</think> blocks before extraction to prevent
        // brace-matching from grabbing JSON fragments inside reasoning.
        let cleaned = Self::strip_think_blocks(response);
        let response = cleaned.as_str();

        // Try markdown code block first
        let markers = ["```json\n", "```\n"];
        for marker in markers.iter() {
            if let Some(start_idx) = response.find(marker) {
                let json_start = start_idx + marker.len();
                if let Some(end_idx) = response[json_start..].find("```") {
                    let json_text = &response[json_start..json_start + end_idx];
                    return json_text.trim().to_string();
                }
            }
        }
        // Fall back to brace extraction
        if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                return response[start..=end].to_string();
            }
        }
        response.to_string()
    }

    /// Remove all `<think>...</think>` blocks from the LLM response so that
    /// any JSON-like content inside reasoning doesn't confuse brace extraction.
    fn strip_think_blocks(input: &str) -> String {
        let mut result = String::with_capacity(input.len());
        let mut remaining = input;
        while let Some(start) = remaining.find("<think>") {
            result.push_str(&remaining[..start]);
            if let Some(end) = remaining[start..].find("</think>") {
                remaining = &remaining[start + end + "</think>".len()..];
            } else {
                // Unclosed <think> — discard everything after it
                return result;
            }
        }
        result.push_str(remaining);
        result
    }

    /// Build a summary of current project tasks for injection into prompts.
    async fn build_task_status_summary(&self) -> String {
        let project_manager = self.project_manager.read().await;
        match project_manager.get_project_tasks(&self.project_id).await {
            Ok(tasks) => {
                let mut summary = String::new();
                for task in &tasks {
                    summary.push_str(&format!(
                        "- [{}] {} (ID: {}, assigned: {:?})\n",
                        format!("{:?}", task.status),
                        task.title,
                        task.id,
                        task.assigned_worker.as_ref().map(|a| a.name.clone()).unwrap_or_else(|| "unassigned".to_string())
                    ));
                }
                if summary.is_empty() {
                    "No tasks yet.".to_string()
                } else {
                    summary
                }
            }
            Err(e) => format!("Error fetching tasks: {}", e),
        }
    }

    /// Build a summary of active workers for injection into prompts.
    fn build_worker_summary(&self) -> String {
        if self.active_workers.is_empty() {
            return "No active workers.".to_string();
        }
        let mut summary = String::new();
        for (template_name, (agent_id, _)) in &self.active_workers {
            summary.push_str(&format!("- {} (type: {})\n", agent_id.name, template_name));
        }
        summary
    }

    /// TrippleEffect Continuous Cycle Engine for PM agents.
    ///
    /// Replaces the legacy procedural `manage_loop` with a prompt-driven,
    /// state-machine autonomous loop. The PM decides its own actions via LLM
    /// calls, using `request_state` to transition and native tool interception
    /// for `project_management`, `manage_team`, and `send_message`.
    pub async fn execute_autonomous_cycle(&mut self) -> Result<()> {
        tracing::info!("PM {} starting TrippleEffect autonomous cycle for project {}", self.id.name, self.project_id);

        let mut loop_count = 0;
        let max_loops = 200; // PM cycles can be long-lived

        let mut watchdog = crate::agents::cycle_engine::WatchdogState::new(AgentState::Startup);
        let mut duplicate_tracker = crate::agents::cycle_engine::DuplicateToolTracker::new();
        let mut local_context: Vec<String> = Vec::new();

        // Seed initial context with project overview
        let project_overview = {
            let pm = self.project_manager.read().await;
            match pm.get_project(&self.project_id).await? {
                Some(p) => format!("Project: {}\nOverview: {}", p.title, p.overview),
                None => return Err(anyhow::anyhow!("Project {} not found", self.project_id)),
            }
        };

        local_context.push(format!(
            "System/User: BEGIN PROJECT MANAGEMENT.\n{}\n\nYou are the Project Manager. Analyze the project and begin the startup workflow.",
            project_overview
        ));

        // Start in Startup state — the LLM will drive transitions from here
        self.state_machine.transition(AgentState::Startup, "Autonomous cycle started".to_string())?;
        self.te_state_name = "pm_startup".to_string();

        while loop_count < max_loops {
            loop_count += 1;

            let current_state = self.state_machine.current_state().clone();

            // Terminal states — exit the loop
            if current_state == AgentState::Idle {
                tracing::info!("PM {} reached Idle state, cycle complete.", self.id.name);
                break;
            }
            if current_state == AgentState::Error {
                tracing::error!("PM {} in Error state, aborting cycle.", self.id.name);
                break;
            }

            // Process any incoming messages from workers/admin
            if let Some(receiver) = &mut self._receiver {
                let mut messages = Vec::new();
                while let Ok(msg) = receiver.try_recv() {
                    messages.push(msg);
                }
                for msg in messages {
                    let from = msg.from.name.clone();
                    let content_str = format!("{:?}", msg.content);
                    local_context.push(format!("System: Message received from {}: {}", from, content_str));
                    self.handle_message(msg).await?;
                }
            }

            // Check project status — bail on terminal project states
            {
                let pm = self.project_manager.read().await;
                if let Some(project) = pm.get_project(&self.project_id).await? {
                    match project.status {
                        crate::projects::ProjectStatus::Cancelled |
                        crate::projects::ProjectStatus::Failed |
                        crate::projects::ProjectStatus::Completed => {
                            tracing::info!("PM {} project in terminal state ({:?}), exiting cycle.", self.id.name, project.status);
                            break;
                        },
                        crate::projects::ProjectStatus::Paused => {
                            tracing::info!("PM {} project paused, sleeping...", self.id.name);
                            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                            continue;
                        },
                        _ => {}
                    }
                } else {
                    tracing::warn!("PM {} project not found, exiting cycle.", self.id.name);
                    break;
                }
            }

            // Map TrippleEffect state name to prompt name
            // This uses the fine-grained TE state string instead of the coarse AgentState
            // so that sub-states like pm_activate_workers get their own dedicated prompt.
            let prompt_name = match self.te_state_name.as_str() {
                "pm_startup"          => "pm_startup_prompt",
                "pm_build_team_tasks" => "pm_build_team_tasks_prompt",
                "pm_activate_workers" => "pm_activate_workers_prompt",
                "pm_work"             => "pm_work_prompt",
                "pm_manage"           => "pm_manage_prompt",
                "pm_manage_team"      => "pm_manage_prompt",
                "pm_team_status"      => "pm_manage_prompt",
                "pm_report_check"     => "pm_report_check_prompt",
                "pm_audit"            => "pm_audit_prompt",
                "pm_standby"          => "pm_standby_prompt",
                _                     => "pm_manage_prompt",
            };
            tracing::debug!("PM {} te_state='{}' -> prompt='{}'", self.id.name, self.te_state_name, prompt_name);

            // Load TE prompt
            let mut system_prompt = self.get_te_prompt(prompt_name)
                .unwrap_or_else(|| format!("You are a Project Manager AI. Current state: {:?}. Manage the project.", current_state));

            // Watchdog intervention
            if watchdog.check(current_state.clone()) {
                system_prompt.push_str("\n\n");
                system_prompt.push_str(&watchdog.intervention_message(crate::agents::cycle_engine::AgentType::PM));
            }

            // Template variable substitution
            let framework_instructions = self.get_te_prompt("pm_standard_framework_instructions")
                .unwrap_or_default();
            let task_summary = self.build_task_status_summary().await;
            let worker_summary = self.build_worker_summary();
            let project_name = {
                let pm = self.project_manager.read().await;
                pm.get_project(&self.project_id).await?
                    .map(|p| p.title.clone())
                    .unwrap_or_else(|| self.project_id.to_string())
            };

            system_prompt = system_prompt.replace("{pm_standard_framework_instructions}", &framework_instructions);
            system_prompt = system_prompt.replace("{agent_id}", &self.id.name);
            system_prompt = system_prompt.replace("{project_name}", &project_name);
            system_prompt = system_prompt.replace("{task_description}", &project_overview);
            system_prompt = system_prompt.replace("{team_wip_updates}", &format!("Tasks:\n{}\nWorkers:\n{}", task_summary, worker_summary));
            system_prompt = system_prompt.replace("{current_time_utc}", &chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string());
            // Clean up any remaining unresolved template vars
            system_prompt = system_prompt.replace("{team_id}", &format!("team_{}", project_name));
            system_prompt = system_prompt.replace("{session_name}", "main");
            system_prompt = system_prompt.replace("{address_book}", &format!("Admin AI: admin_ai\nWorkers: {}", worker_summary));
            system_prompt = system_prompt.replace("{pm_provider}", "auto");
            system_prompt = system_prompt.replace("{pm_model}", "auto-selected");
            if current_state == AgentState::Startup {
                system_prompt = system_prompt.replace("{tool_instructions}", "");
            } else {
                let tool_instructions = format!(
                    "Available tools: request_state, project_management, manage_team, send_message, framework::tool_information\n\n\
                    Native Tool Minimal Schemas:\n\
                    - request_state: {{\"state\": \"<pm_state>\"}}\n\
                    - project_management: {{\"action\": \"<list_tasks|add_task|modify_task>\", ...}}\n\
                    - manage_team: {{\"action\": \"<list_agents|add_agent>\", ...}}\n\
                    - send_message: {{\"target_agent_id\": \"<agent_id>\", \"message_content\": \"<message>\"}}\n\n\
                    Use the 'framework::tool_information' tool to request the FULL JSON schema and detailed description of ANY tool (including native tools like project_management) if you need more details."
                );
                system_prompt = system_prompt.replace("{tool_instructions}", &tool_instructions);
            }

            // Append JSON output instruction only if not in Startup state
            // (Startup state has its own specific JSON schema defined in the prompt)
            if current_state != AgentState::Startup {
                system_prompt.push_str("\n\nCRITICAL INSTRUCTION: You MUST respond with exactly ONE JSON object representing your action. Example: {\"tool\": \"request_state\", \"params\": {\"state\": \"pm_manage\"}}");
            }

            let options = GenerationOptions {
                system: Some(system_prompt),
                temperature: Some(0.4),
                ..Default::default()
            };

            let context_history = local_context.join("\n\n");
            let user_prompt = format!("Current Context:\n{}\n\nWhat is your NEXT ACTION?", context_history);

            let preferred_family = if let Some(prefs) = &self.user_settings {
                prefs.read().await.get_model_preference("PM").await.unwrap_or(None)
            } else {
                None
            };

            // Call LLM
            let response_text = match self.generate_llm_response(
                &user_prompt,
                Some(options),
                SelectionContext::for_pm(),
                preferred_family,
                120_000,
                &format!("pm_cycle_{}", loop_count),
            ).await {
                Ok(text) => text,
                Err(e) => {
                    tracing::error!("PM {} LLM generation failed: {}", self.id.name, e);
                    local_context.push(format!("System: Error generating response: {}", e));
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                    continue;
                }
            };

            // Parse LLM JSON action
            tracing::debug!("PM {} raw LLM response (cycle {}): {}", self.id.name, loop_count, response_text);
            let json_str = self.extract_json_from_response(&response_text);
            tracing::debug!("PM {} extracted JSON (cycle {}): {}", self.id.name, loop_count, json_str);
            let action: serde_json::Value = match serde_json::from_str(&json_str) {
                Ok(json) => json,
                Err(_) => {
                    local_context.push("System: Error: Failed to parse output as JSON. Please try again and ensure you output exactly ONE JSON object.".to_string());
                    tracing::warn!("PM {} failed to parse JSON from LLM response (cycle {})", self.id.name, loop_count);
                    continue;
                }
            };

            let tool_name = action.get("tool").and_then(|t| t.as_str()).unwrap_or("").to_string();
            let params = action.get("params").cloned().unwrap_or(serde_json::Value::Object(Default::default()));

            tracing::info!("PM {} cycle {} action: tool={} params={}", self.id.name, loop_count, tool_name, params);
            local_context.push(format!("Assistant: Executing tool: {} with params: {}", tool_name, params));

            // =====================================================
            // PM Startup Interceptor
            // =====================================================
            if current_state == AgentState::Startup && action.get("tasks").is_some() && action.get("roles").is_some() {
                tracing::info!("PM {} intercepted kickoff plan JSON", self.id.name);
                
                let mut created_tasks_count = 0;
                if let Some(tasks) = action.get("tasks").and_then(|t| t.as_array()) {
                    let project_manager = self.project_manager.write().await;
                    for task_val in tasks {
                        let title = task_val.get("title").or(task_val.get("task_name")).or(task_val.get("name")).or(task_val.get("description")).or(task_val.get("id")).and_then(|t| t.as_str()).unwrap_or("Untitled Task").to_string();
                        let description = task_val.get("description").and_then(|d| d.as_str()).unwrap_or(&title).to_string();
                        
                        match project_manager.create_task(&self.project_id, title.clone(), description).await {
                            Ok(_task_id) => {
                                self.session_tasks.add_task(title.clone(), None);
                                created_tasks_count += 1;
                            }
                            Err(e) => tracing::error!("Failed to auto-create kickoff task: {}", e),
                        }
                    }
                }
                
                let summary = format!(
                    "[Framework System Message] **MASTER KICKOFF PLAN SUMMARY**\nRoles to create: {:?}\nTasks created: {}", 
                    action.get("roles").unwrap_or(&serde_json::Value::Null), 
                    created_tasks_count
                );
                local_context.push(format!("System: {}", summary));
                
                if let Err(e) = self.state_machine.transition(AgentState::Planning, "Auto-transition after kickoff plan intercept".to_string()) {
                    local_context.push(format!("System: Error: State transition failed: {}", e));
                } else {
                    self.te_state_name = "pm_build_team_tasks".to_string();
                    duplicate_tracker.reset();
                    self.update_status("Transitioned to Planning (pm_build_team_tasks)").await;
                    tracing::info!("PM {} te_state_name set to '{}' after kickoff intercept", self.id.name, self.te_state_name);
                    local_context.push("System: State successfully changed to pm_build_team_tasks. You must now create worker agents using the manage_team tool.".to_string());
                }
                continue;
            }

            // Duplicate detection
            let sig = crate::agents::cycle_engine::ToolCallSignature {
                tool_name: tool_name.clone(),
                args_hash: params.to_string(),
            };
            if duplicate_tracker.record_call(sig) {
                local_context.push("System: Error: You are repeating the same action. Please try a different approach or transition state.".to_string());
                continue;
            }

            // =====================================================
            // Native Tool Interception
            // =====================================================

            // 1. framework::tool_information
            if tool_name == "framework::tool_information" || tool_name == "tool_information" {
                if let Some(tools_array) = params.get("tools").and_then(|t| t.as_array()) {
                    let mut tool_schemas = Vec::new();
                    let client = self.mcp_client.read().await;
                    for tool_val in tools_array {
                        if let Some(tool_name_req) = tool_val.as_str() {
                            if let Ok(meta) = client.get_tool_metadata(tool_name_req).await {
                                tool_schemas.push(serde_json::to_string(&meta).unwrap_or_default());
                            } else {
                                match tool_name_req {
                                    "request_state" => tool_schemas.push(r#"{"name":"request_state","description":"Drive the PM state machine","inputSchema":{"type":"object","properties":{"state":{"type":"string","enum":["pm_startup","pm_build_team_tasks","pm_activate_workers","pm_manage","pm_report_check","pm_audit","pm_standby"]}},"required":["state"]}}"#.to_string()),
                                    "project_management" => tool_schemas.push(r#"{"name":"project_management","description":"CRUD operations on project tasks","inputSchema":{"type":"object","properties":{"action":{"type":"string","enum":["list_tasks","add_task","modify_task"]},"title":{"type":"string","description":"Task title (for add_task)"},"description":{"type":"string","description":"Task description"},"task_id":{"type":"string","description":"Required for modify_task"},"assignee_agent_id":{"type":"string","description":"Used with modify_task to assign task"}},"required":["action"]}}"#.to_string()),
                                    "manage_team" => tool_schemas.push(r#"{"name":"manage_team","description":"Worker lifecycle management","inputSchema":{"type":"object","properties":{"action":{"type":"string","enum":["list_agents","add_agent"]},"role":{"type":"string","description":"Required for add_agent"},"persona":{"type":"string","description":"Optional persona name for add_agent"}},"required":["action"]}}"#.to_string()),
                                    "send_message" => tool_schemas.push(r#"{"name":"send_message","description":"Inter-agent communication","inputSchema":{"type":"object","properties":{"target_agent_id":{"type":"string"},"message_content":{"type":"string"}},"required":["target_agent_id","message_content"]}}"#.to_string()),
                                    _ => tool_schemas.push(format!("Error: Could not find schema for tool '{}'. Note: Native tools like project_management, manage_team, and send_message are now documented via this tool.", tool_name_req))
                                }
                            }
                        }
                    }
                    local_context.push(format!("System: [tool_information result]\n{}", tool_schemas.join("\n\n")));
                } else {
                    local_context.push("System: Error: tool_information requires a 'tools' parameter containing an array of tool names. Example: {\"tool\": \"framework::tool_information\", \"params\": {\"tools\": [\"hainet-files::file_read\"]}}".to_string());
                }
                continue;
            }

            // 2. request_state — drive the PM state machine
            if tool_name == "request_state" || tool_name == "framework::request_state" {
                if let Some(new_state_str) = params.get("state").and_then(|s| s.as_str()) {
                    let new_state = match new_state_str {
                        "pm_startup"          => AgentState::Startup,
                        "pm_build_team_tasks" => AgentState::Planning,
                        "pm_activate_workers" => AgentState::Managing,
                        "pm_work"             => AgentState::Managing,
                        "pm_manage"           => AgentState::Managing,
                        "pm_manage_team"      => AgentState::Managing,
                        "pm_team_status"      => AgentState::Managing,
                        "pm_report_check"     => AgentState::Reviewing,
                        "pm_audit"            => AgentState::Auditing,
                        "pm_standby" => {
                            // Check for unfinished tasks before allowing standby
                            let summary = self.build_task_status_summary().await;
                            if summary.contains("todo") || summary.contains("pending") || summary.contains("UnderReview") || summary.contains("Failed") {
                                local_context.push("System: BLOCKED — cannot standby with unfinished tasks. Use project_management list_tasks to review and assign remaining tasks.".to_string());
                                continue;
                            }
                            AgentState::Idle
                        },
                        _ => {
                            local_context.push(format!("System: Error: Invalid PM state requested '{}'. Valid states: pm_startup, pm_build_team_tasks, pm_activate_workers, pm_manage, pm_report_check, pm_audit, pm_standby", new_state_str));
                            continue;
                        }
                    };

                    if self.te_state_name == new_state_str {
                        local_context.push(format!("System: Info: You are already in the '{}' state. No transition needed. Please execute a different action.", new_state_str));
                        continue;
                    }

                    if let Err(e) = self.state_machine.transition(new_state.clone(), format!("LLM requested state change to {}", new_state_str)) {
                        local_context.push(format!("System: Error: State transition failed: {}", e));
                    } else {
                        // Store the fine-grained TE state name for prompt selection
                        self.te_state_name = new_state_str.to_string();
                        duplicate_tracker.reset();
                        self.update_status(&format!("Transitioned to {:?} ({})", new_state, new_state_str)).await;
                        local_context.push(format!("System: State successfully changed to {}", new_state_str));
                    }
                } else {
                    local_context.push("System: Error: request_state tool requires 'state' parameter.".to_string());
                }
                continue;
            }

            // 2. project_management — CRUD on tasks
            if tool_name == "project_management" {
                let pm_action = params.get("action").and_then(|a| a.as_str()).unwrap_or("");
                match pm_action {
                    "list_tasks" => {
                        let summary = self.build_task_status_summary().await;
                        local_context.push(format!("System: [list_tasks result]\n{}", summary));
                    }
                    "add_task" => {
                        let title = params.get("title").or(params.get("task_name")).or(params.get("name")).or(params.get("description"))
                            .and_then(|t| t.as_str()).unwrap_or("Untitled Task").to_string();
                        let description = params.get("description")
                            .and_then(|d| d.as_str()).unwrap_or(&title).to_string();
                        let assignee = params.get("assignee").or(params.get("assignee_agent_id")).and_then(|a| a.as_str());
                        
                        let task_id_opt = {
                            let project_manager = self.project_manager.write().await;
                            match project_manager.create_task(&self.project_id, title.clone(), description).await {
                                Ok(task_id) => {
                                    self.session_tasks.add_task(title.clone(), None);
                                    Some(task_id)
                                }
                                Err(e) => {
                                    local_context.push(format!("System: Error creating task: {}", e));
                                    None
                                }
                            }
                        };
                        
                        if let Some(task_id) = task_id_opt {
                            if assignee.is_some() {
                                match self.assign_task_to_worker(&task_id, assignee).await {
                                    Ok(()) => local_context.push(format!("System: Task created and assigned successfully. ID: {}, Title: {}", task_id, title)),
                                    Err(e) => local_context.push(format!("System: Task created, but error assigning task: {}", e)),
                                }
                            } else {
                                local_context.push(format!("System: Task created successfully. ID: {}, Title: {}", task_id, title));
                            }
                        }
                    }
                    "modify_task" => {
                        let task_id_str = params.get("task_id").and_then(|t| t.as_str()).unwrap_or("");
                        let assignee = params.get("assignee_agent_id").and_then(|a| a.as_str());
                        
                        if let Some(assignee_id) = assignee {
                            // This is a task assignment — find matching task and assign
                            let project_manager = self.project_manager.read().await;
                            let tasks = project_manager.get_project_tasks(&self.project_id).await?;
                            drop(project_manager);
                            
                            if let Some(task) = tasks.iter().find(|t| t.id.to_string().contains(task_id_str) || t.title.contains(task_id_str)) {
                                let real_task_id = task.id.clone();
                                match self.assign_task_to_worker(&real_task_id, assignee).await {
                                    Ok(()) => {
                                        local_context.push(format!("System: Task '{}' assigned to worker successfully.", task.title));
                                    }
                                    Err(e) => {
                                        local_context.push(format!("System: Error assigning task: {}", e));
                                    }
                                }
                            } else {
                                local_context.push(format!("System: Error: Task '{}' not found in project.", task_id_str));
                            }
                        } else {
                            // Other modifications (tags, status, etc.)
                            local_context.push(format!("System: modify_task acknowledged for task '{}'. Changes noted.", task_id_str));
                        }
                    }
                    _ => {
                        local_context.push(format!("System: Unknown project_management action: '{}'", pm_action));
                    }
                }
                continue;
            }

            // 3. manage_team — worker lifecycle management
            // NOTE: The LLM does NOT need to specify model/provider — the system handles
            // model selection automatically via AIProviderManager and user preferences.
            if tool_name == "manage_team" {
                // Infer action from params: if 'role' is present, treat as add_agent
                let team_action = params.get("action").and_then(|a| a.as_str())
                    .unwrap_or_else(|| {
                        if params.get("role").is_some() {
                            "add_agent"
                        } else {
                            "list_agents"
                        }
                    });
                match team_action {
                    "list_agents" | "list" | "status" => {
                        let summary = self.build_worker_summary();
                        local_context.push(format!("System: [list_agents result]\n{}", summary));
                    }
                    "add_agent" | "add" | "create" | "spawn" => {
                        let role = params.get("role").and_then(|r| r.as_str()).unwrap_or("worker");
                        let persona = params.get("persona").and_then(|p| p.as_str()).unwrap_or(role);
                        
                        // Select template based on persona/role — model/provider are auto-selected
                        let template = WorkerTemplate::select_for_task(persona);
                        let template_name = template.name.clone();
                        
                        // Check if we already have a worker of this type
                        if self.active_workers.contains_key(&template_name) {
                            local_context.push(format!("System: Worker of type '{}' already exists and is ready. If you need to create more workers for different roles, call manage_team again. If your entire team is complete, use request_state to transition to pm_activate_workers.", template_name));
                        } else {
                            // Create and spawn the worker
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
                            
                            worker.state_machine_mut().transition(
                                AgentState::Idle,
                                "Worker initialized by PM autonomous cycle".to_string()
                            )?;
                            
                            let (tx, rx) = mpsc::channel(100);
                            self.active_workers.insert(template_name.clone(), (worker_id.clone(), tx));
                            
                            let worker_name = worker.id().name.clone();
                            tokio::spawn(async move {
                                worker.run(rx).await;
                                tracing::info!("Worker {} run loop terminated", worker_name);
                            });
                            
                            local_context.push(format!(
                                "System: Worker agent created successfully. ID: {}, Type: {}, Role: {}. Model selection is automatic. If you need to create more workers for the project, call manage_team again. If your entire team is complete, use request_state to transition to pm_activate_workers.",
                                worker_id.name, template_name, role
                            ));
                        }
                    }
                    _ => {
                        local_context.push(format!("System: Unknown manage_team action: '{}'. Valid actions: add_agent, list_agents", team_action));
                    }
                }
                continue;
            }

            // 4. send_message — inter-agent communication
            if tool_name == "send_message" {
                let target = params.get("target_agent_id").and_then(|t| t.as_str()).unwrap_or("");
                let content = params.get("message_content").or(params.get("message"))
                    .and_then(|c| c.as_str()).unwrap_or("");

                if target.contains("admin") {
                    // Send to admin
                    let admin_id = {
                        let bus = self.message_bus.read().await;
                        let active_agents = bus.get_active_agents().await;
                        active_agents.iter()
                            .find(|a| a.id.agent_type == crate::messaging::AgentType::Admin)
                            .map(|a| a.id.clone())
                    };

                    if let Some(admin_id) = admin_id {
                        let msg = crate::messaging::Message::new(
                            self.id.clone(),
                            admin_id,
                            crate::messaging::MessageContent::StatusUpdate(crate::messaging::StatusUpdate {
                                agent_id: self.id.clone(),
                                state: self.state_machine.current_state().clone(),
                                message: content.to_string(),
                                progress: None,
                            })
                        );
                        if let Err(e) = self.message_bus.write().await.send_message(msg).await {
                            local_context.push(format!("System: Error sending message to Admin: {}", e));
                        } else {
                            local_context.push(format!("System: Message sent to Admin AI: {}", content));
                        }
                    } else {
                        local_context.push("System: Warning: No Admin agent registered.".to_string());
                    }
                } else {
                    // Send to a worker
                    let worker_id_opt = self.active_workers.values()
                        .find(|(id, _)| id.name.contains(target))
                        .map(|(id, _)| id.clone());
                    
                    if let Some(worker_id) = worker_id_opt {
                        let msg = crate::messaging::Message::new(
                            self.id.clone(),
                            worker_id,
                            crate::messaging::MessageContent::Query(content.to_string())
                        );
                        if let Err(e) = self.message_bus.write().await.send_message(msg).await {
                            local_context.push(format!("System: Error sending message to {}: {}", target, e));
                        } else {
                            local_context.push(format!("System: Message sent to {}: {}", target, content));
                        }
                    } else {
                        local_context.push(format!("System: Error: Agent '{}' not found in active workers.", target));
                    }
                }
                continue;
            }

            // Unknown tool — inform LLM
            local_context.push(format!(
                "System: Warning: Unknown tool '{}'. Available tools: request_state, project_management, manage_team, send_message",
                tool_name
            ));

            // Throttle to prevent spin-looping
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        if loop_count >= max_loops {
            tracing::error!("PM {} exceeded maximum cycle count ({}) - forcing termination", self.id.name, max_loops);
        }

        tracing::info!("PM {} autonomous cycle finished after {} iterations", self.id.name, loop_count);
        Ok(())
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
        let ai_provider_manager = Arc::new(AIProviderManager::new(None, "standalone".to_string()).await.unwrap());
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
        let ai_provider_manager = match AIProviderManager::new(None, "standalone".to_string()).await {
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

        let ai_provider_manager = Arc::new(AIProviderManager::new(None, "standalone".to_string()).await.unwrap());
        let mcp_client = Arc::new(RwLock::new(crate::tools::mcp::MCPClientManager::new()));
        let mut pm = PMAgent::new(project_id, message_bus, prompt_manager, project_manager, ai_provider_manager, mcp_client, None);
        
        pm.start().await.unwrap();
        assert_eq!(pm.state(), &AgentState::Managing);
    }
}
