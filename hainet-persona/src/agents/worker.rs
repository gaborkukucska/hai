//! # START OF FILE hainet-persona/src/agents/worker.rs
//! Worker Agent
//! 
//! Executes individual tasks using MCP tools with learning capabilities.
//! Worker agents follow this state machine:
//! Idle → Planning → Working → Reporting → (Idle | Error)

use anyhow::{Result, Context};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::messaging::{MessageBus, AgentId};
use crate::prompts::{PromptManager, AgentType, AgentState, WorkerType};
use crate::projects::{ProjectManager, TaskId};
use crate::tools::mcp::MCPClientManager;
use crate::ai_providers::{AIProviderManager, SelectionContext, providers::GenerationOptions};
use super::state::AgentStateMachine;
use super::templates::WorkerTemplate;
use super::worker_intelligence::{WorkerLearner, ExecutionStrategy, ToolSelector, ErrorCategory, TaskOutcome};
use super::session_tasks::SessionTaskList;
use super::worker_discovery::{
    DiscoveryExecutionPlan, DiscoveryExecutionStep,
    parse_tool_selection, parse_execution_plan, format_tool_list, format_tool_metadata,
};
use serde_json::json;
use std::time::SystemTime;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubTask {
    id: usize,
    description: String,
    goal: String,
    verification_criteria: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VerificationResult {
    success: bool,
    issues: Vec<String>,
    feedback: String,
}

/// Worker Agent
/// 
/// Responsible for:
/// - Executing specific tasks assigned by PM using LLM-powered planning
/// - Using MCP tools to accomplish work
/// - Reporting results back to PM for validation
/// - Learning from task outcomes to improve performance over time
pub struct WorkerAgent {
    /// Unique agent identifier
    id: AgentId,
    
    /// Worker specialization type
    worker_type: WorkerType,
    
    /// Worker template with capabilities and system prompt
    template: WorkerTemplate,
    
    /// Current task being executed
    current_task: Option<TaskId>,
    
    /// State machine
    state_machine: AgentStateMachine,
    
    /// Message bus for communication
    message_bus: Arc<RwLock<MessageBus>>,
    
    /// Prompt manager for generating prompts
    prompt_manager: Arc<RwLock<PromptManager>>,
    
    /// Project manager for task updates
    project_manager: Arc<RwLock<ProjectManager>>,
    
    /// MCP client for tool access
    mcp_client: Arc<RwLock<MCPClientManager>>,
    
    /// AI provider manager for dynamic model selection
    ai_provider_manager: Arc<AIProviderManager>,
    
    /// User settings manager for model preferences
    user_settings: Option<Arc<RwLock<crate::user_settings::UserSettingsManager>>>,

    /// Maximum retry attempts for failed operations (deprecated - use execution_strategy.max_retries)
    max_retries: usize,
    
    /// Worker intelligence - historical learning
    learner: WorkerLearner,
    
    /// Adaptive execution configuration
    execution_strategy: ExecutionStrategy,
    
    /// Intelligent tool selector
    tool_selector: ToolSelector,
    
    /// Enable self-correction (default: true)
    self_correction_enabled: bool,
    
    /// Session task list - tracks progress within session
    session_tasks: SessionTaskList,
    
    /// Message receiver (kept alive to maintain registration)
    _receiver: Option<tokio::sync::mpsc::Receiver<crate::messaging::Message>>,
    
    /// Current project name (for file operation sandboxing)
    /// Workers are sandboxed to /sandbox/projects/{project_name}/
    /// None means no sandboxing (used by Admin agents)
    current_project_name: Option<String>,
}

impl WorkerAgent {
    /// Create new worker agent with template
    pub fn new(
        worker_type: WorkerType,
        message_bus: Arc<RwLock<MessageBus>>,
        prompt_manager: Arc<RwLock<PromptManager>>,
        project_manager: Arc<RwLock<ProjectManager>>,
        mcp_client: Arc<RwLock<MCPClientManager>>,
        ai_provider_manager: Arc<AIProviderManager>,
    ) -> Self {
        let id = AgentId::new(AgentType::Worker, format!("Worker-{:?}", worker_type));
        
        // Select appropriate template based on worker type
        let template = match worker_type {
            WorkerType::Files => WorkerTemplate::file_worker(),
            WorkerType::Network => WorkerTemplate::network_worker(),
            WorkerType::Research => WorkerTemplate::research_worker(),
            WorkerType::Code => WorkerTemplate::code_worker(),
            WorkerType::Compute => WorkerTemplate::file_worker(), // Default to file worker
            _ => WorkerTemplate::file_worker(),
        };
        
        // Create default fallback tool order based on template
        let fallback_tools = template.mcp_servers.iter()
            .flat_map(|server| {
                match server.as_str() {
                    "hainet-files" => vec![
                        format!("{}::file_read", server),
                        format!("{}::file_write", server),
                        format!("{}::file_list", server),
                    ],
                    "hainet-system" => vec![
                        format!("{}::system_status", server),
                        format!("{}::list_services", server),
                    ],
                    "hainet-dev" => vec![
                        format!("{}::git_status", server),
                        format!("{}::cargo_build", server),
                    ],
                    _ => vec![],
                }
            })
            .collect();
        
        Self {
            id,
            worker_type,
            template,
            current_task: None,
            state_machine: AgentStateMachine::new(),
            message_bus,
            prompt_manager,
            project_manager,
            mcp_client,
            ai_provider_manager,
            user_settings: None, // Workers don't have user settings in new() - use from_template() instead
            max_retries: 3, // Kept for backward compatibility
            learner: WorkerLearner::new(), // Default 100 outcome capacity
            execution_strategy: ExecutionStrategy::default(), // 5s timeout, 3 retries, 1.5x backoff
            tool_selector: ToolSelector::new(fallback_tools),
            self_correction_enabled: true,
            session_tasks: SessionTaskList::new(),
            _receiver: None,
            current_project_name: None, // No project until task is assigned
        }
    }
    
    /// Create worker from template
    pub fn from_template(
        template: WorkerTemplate,
        message_bus: Arc<RwLock<MessageBus>>,
        prompt_manager: Arc<RwLock<PromptManager>>,
        project_manager: Arc<RwLock<ProjectManager>>,
        mcp_client: Arc<RwLock<MCPClientManager>>,
        ai_provider_manager: Arc<AIProviderManager>,
        user_settings: Option<Arc<RwLock<crate::user_settings::UserSettingsManager>>>,
    ) -> Self {
        let id = AgentId::new(AgentType::Worker, template.name.clone());
        
        // Create default fallback tool order based on template
        let fallback_tools = template.mcp_servers.iter()
            .flat_map(|server| {
                match server.as_str() {
                    "hainet-files" => vec![
                        format!("{}::file_read", server),
                        format!("{}::file_write", server),
                        format!("{}::file_list", server),
                    ],
                    "hainet-system" => vec![
                        format!("{}::system_status", server),
                        format!("{}::list_services", server),
                    ],
                    "hainet-dev" => vec![
                        format!("{}::git_status", server),
                        format!("{}::cargo_build", server),
                    ],
                    _ => vec![],
                }
            })
            .collect();
        
        Self {
            id,
            worker_type: WorkerType::Files, // Default, overridden by template
            template,
            current_task: None,
            state_machine: AgentStateMachine::new(),
            message_bus,
            prompt_manager,
            project_manager,
            mcp_client,
            ai_provider_manager,
            user_settings, // Accept user_settings from PM
            max_retries: 3, // Kept for backward compatibility
            learner: WorkerLearner::new(),
            execution_strategy: ExecutionStrategy::default(),
            tool_selector: ToolSelector::new(fallback_tools),
            self_correction_enabled: true,
            session_tasks: SessionTaskList::new(),
            _receiver: None,
            current_project_name: None, // No project until task is assigned
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
        
        // NOTE: Task is already marked as Assigned in DB by PM before being sent to channel
        // We only need to set up worker-side state here
        
        // Get task details and add to session task list
        let task = self.get_task_details(&task_id).await?;
        self.session_tasks.add_task(
            task.title.clone(), 
            Some(task.description.clone())
        );
        
        // Extract project name for file operation sandboxing
        // Workers are sandboxed to /sandbox/projects/{project_name}/
        let project_title = {
            let pm = self.project_manager.read().await;
            let project = pm.get_project(&task.project_id).await?
                .ok_or_else(|| anyhow::anyhow!("Project not found: {}", task.project_id))?;
            project.title.clone()
        };
        
        self.current_project_name = Some(project_title.clone());
        
        tracing::info!(
            "Worker {} assigned to project '{}' (sandboxed file access)",
            self.id.name,
            project_title
        );
        
        tracing::debug!(
            "Worker {} added task to session: {}",
            self.id.name,
            task.title
        );
        
        Ok(())
    }
    
    /// Execute assigned task with discovery-based tool loading (NEW)
    /// 
    /// This method uses modular prompts and lazy-loads tool metadata to avoid
    /// overwhelming small LLMs with excessive context.
    pub async fn execute_task_with_discovery(&mut self) -> Result<()> {
        let task_id = self.current_task.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No task assigned"))?
            .clone();
        
        // Get task details
        let task = self.get_task_details(&task_id).await?;
        
        // Mark task as in progress in session
        self.session_tasks.start_task(&task.title)
            .unwrap_or_else(|e| tracing::warn!("Failed to update session task: {}", e));
        
        // Transition to Planning
        self.state_machine.transition(
            AgentState::Planning,
            "Discovery-based planning".to_string()
        )?;
        self.update_status(&format!("Planning task {}", task_id)).await;
        
        // Execute with discovery-based approach
        let _start_time = SystemTime::now();
        let result = self.execute_with_discovery(&task).await;
        
        match result {
            Ok(deliverables) => {
                // Mark task as complete in session
                self.session_tasks.complete_task(&task.title)
                    .unwrap_or_else(|e| tracing::warn!("Failed to complete session task: {}", e));
                
                // Transition to Reporting
                self.state_machine.transition(
                    AgentState::Reporting,
                    "Task complete, reporting to PM".to_string()
                )?;
                self.update_status(&format!("Reporting task {}", task_id)).await;
                
                // Submit task for review
                let project_manager = self.project_manager.write().await;
                project_manager.complete_task(&task_id, deliverables.clone()).await?;
                
                tracing::info!("Worker {} completed task: {}", self.id.name, task.title);
                
                // Send TaskResult message to PM with retry
                let task_result = crate::messaging::MessageContent::TaskResult(crate::messaging::TaskResult {
                    task_id: task_id.to_string(),
                    success: true,
                    output: serde_json::Value::String(deliverables.join("\n")),
                    error: None,
                    metrics: crate::messaging::TaskMetrics {
                        duration_ms: 0,
                        cost_usd: 0.0,
                        resource_tier_used: crate::messaging::ResourceTier::LocalOnly,
                        tokens_used: None,
                    },
                });
                
                if let Err(e) = self.send_message_to_pm_with_retry(&task.project_id, task_result, 4).await {
                    tracing::warn!("Worker {} failed to send completion message to PM after retries: {:?}", self.id.name, e);
                }
                
                Ok(())
            }
            Err(e) => {
                // Mark task as failed in session
                self.session_tasks.fail_task(&task.title)
                    .unwrap_or_else(|err| tracing::warn!("Failed to fail session task: {}", err));
                
                // Properly fail the task in project manager to prevent it from being stuck
                tracing::error!(
                    "Worker {} failing task {} due to execution error: {}",
                    self.id.name,
                    task_id,
                    e
                );
                
                let project_manager = self.project_manager.write().await;
                if let Err(fail_err) = project_manager.fail_task(&task_id, format!("Worker execution error: {}", e)).await {
                    tracing::error!(
                        "Worker {} failed to mark task {} as failed: {}",
                        self.id.name,
                        task_id,
                        fail_err
                    );
                }
                
                // Send ErrorReport message to PM with retry
                let error_report = crate::messaging::MessageContent::ErrorReport(crate::messaging::ErrorReport {
                    error_type: "ExecutionError".to_string(),
                    message: e.to_string(),
                    stack_trace: None,
                    recoverable: false,
                });
                
                if let Err(send_err) = self.send_message_to_pm_with_retry(&task.project_id, error_report, 4).await {
                    tracing::warn!("Worker {} failed to send error message to PM after retries: {:?}", self.id.name, send_err);
                }
                
                Err(e)
            }
        }
    }
    
    /// Execute assigned task with LLM-powered planning and learning (LEGACY)
    /// 
    /// This is the original monolithic approach - kept for backward compatibility.
    /// Use execute_task_with_discovery() for new code.
    #[deprecated(note = "Use execute_task_with_discovery instead")]
    pub async fn execute_task(&mut self) -> Result<()> {
        let task_id = self.current_task.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No task assigned"))?
            .clone();
        
        // Get task details
        let task = self.get_task_details(&task_id).await?;
        
        // Mark task as in progress in session
        self.session_tasks.start_task(&task.title)
            .unwrap_or_else(|e| tracing::warn!("Failed to update session task: {}", e));
        
        // Adjust execution strategy based on task title (proxy for task type) and history
        self.execution_strategy.adjust_for_task(&task.title, &mut self.learner);
        tracing::info!(
            "Worker {} adaptive strategy for '{}': timeout={}ms, retries={}",
            self.id.name,
            task.title,
            self.execution_strategy.base_timeout_ms,
            self.execution_strategy.max_retries
        );
        
        // Transition to Planning
        self.state_machine.transition(
            AgentState::Planning,
            "Analyzing task requirements with learning".to_string()
        )?;
        self.update_status(&format!("Planning task {}", task_id)).await;
        
        // Use LLM to plan with intelligent tool selection
        self.update_status(&format!("Planning task {}", task_id)).await;
        
        // Plan execution
        let execution_plan = match self.plan_task_execution_with_learning(&task).await {
            Ok(plan) => plan,
            Err(e) => {
                tracing::error!("Worker {} planning failed for task {}: {}", self.id.name, task_id, e);
                
                // Fail the task so it doesn't get stuck in InProgress
                let project_manager = self.project_manager.write().await;
                if let Err(fail_err) = project_manager.fail_task(&task_id, format!("Planning failed: {}", e)).await {
                    tracing::error!("Failed to mark task as failed after planning error: {}", fail_err);
                }
                
                return Err(e);
            }
        };
        
        tracing::info!("Worker {} planned {} steps for task: {}", 
                       self.id.name, execution_plan.steps.len(), task.title);
        
        // Transition to Working state
        self.state_machine.transition(
            AgentState::Working,
            format!("Executing task {}", task_id)
        )?;
        self.update_status(&format!("Executing task {}", task_id)).await;
        
        // Execute with learning and self-correction
        let start_time = SystemTime::now();
        let result = self.execute_with_learning(&execution_plan, &task).await;
        
        match result {
            Ok(deliverables) => {
                // Mark task as complete in session
                self.session_tasks.complete_task(&task.title)
                    .unwrap_or_else(|e| tracing::warn!("Failed to complete session task: {}", e));
                
                // Record success outcome
                self.record_success_outcome(&task, start_time, &execution_plan);
                
                // Transition to Reporting
                self.state_machine.transition(
                    AgentState::Reporting,
                    "Task complete, reporting to PM".to_string()
                )?;
                
                tracing::info!("Worker {} submitting task {} for review with {} deliverables", self.id.name, task_id, deliverables.len());

                // Submit task for review
                let project_manager = self.project_manager.write().await;
                project_manager.complete_task(&task_id, deliverables.clone()).await?;
                
                tracing::info!("Worker {} completed task: {} and updated status to UnderReview", self.id.name, task.title);
                
                // Send TaskResult message to PM with retry
                let task_result = crate::messaging::MessageContent::TaskResult(crate::messaging::TaskResult {
                    task_id: task_id.to_string(),
                    success: true,
                    output: serde_json::Value::String(deliverables.join("\n")),
                    error: None,
                    metrics: crate::messaging::TaskMetrics {
                        duration_ms: 0,
                        cost_usd: 0.0,
                        resource_tier_used: crate::messaging::ResourceTier::LocalOnly,
                        tokens_used: None,
                    },
                });
                
                if let Err(e) = self.send_message_to_pm_with_retry(&task.project_id, task_result, 4).await {
                    tracing::warn!("Worker {} failed to send completion message to PM after retries: {:?}", self.id.name, e);
                }
                
                Ok(())
            }
            Err(e) => {
                // Mark task as failed in session
                self.session_tasks.fail_task(&task.title)
                    .unwrap_or_else(|err| tracing::warn!("Failed to fail session task: {}", err));
                
                // Record failure outcome
                self.record_failure_outcome(&task, start_time, &execution_plan, &e);
                
                // Properly fail the task in project manager to prevent it from being stuck
                tracing::error!(
                    "Worker {} failing task {} due to execution error: {}",
                    self.id.name,
                    task_id,
                    e
                );
                
                let project_manager = self.project_manager.write().await;
                if let Err(fail_err) = project_manager.fail_task(&task_id, format!("Worker execution error: {}", e)).await {
                    tracing::error!(
                        "Worker {} failed to mark task {} as failed: {}",
                        self.id.name,
                        task_id,
                        fail_err
                    );
                }
                
                // Send ErrorReport message to PM with retry
                let error_report = crate::messaging::MessageContent::ErrorReport(crate::messaging::ErrorReport {
                    error_type: "ExecutionError".to_string(),
                    message: e.to_string(),
                    stack_trace: None,
                    recoverable: false,
                });
                
                if let Err(send_err) = self.send_message_to_pm_with_retry(&task.project_id, error_report, 4).await {
                    tracing::warn!("Worker {} failed to send error message to PM after retries: {:?}", self.id.name, send_err);
                }
                
                Err(e)
            }
        }
    }
    
    /// Send a message to PM with retry logic and dynamic PM instance lookup
    /// This handles the case where PM instance IDs may change
    async fn send_message_to_pm_with_retry(
        &self,
        project_id: &crate::projects::ProjectId,
        content: crate::messaging::MessageContent,
        max_retries: u32,
    ) -> Result<()> {
        let pm_name_pattern = format!("PM-{}", project_id);
        
        for attempt in 1..=max_retries {
            // Query message bus for current PM instance
            let pm_id = {
                let bus = self.message_bus.read().await;
                bus.find_agent_by_name(crate::prompts::types::AgentType::PM, &pm_name_pattern).await
            };
            
            match pm_id {
                Some(pm_agent_id) => {
                    tracing::debug!(
                        "Worker {} found PM instance for project {}: {:?}",
                        self.id.name,
                        project_id,
                        pm_agent_id
                    );
                    
                    let message = crate::messaging::Message::new(
                        self.id.clone(),
                        pm_agent_id,
                        content.clone(),
                    );
                    
                    match self.message_bus.write().await.send_message(message).await {
                        Ok(()) => {
                            tracing::info!(
                                "Worker {} successfully sent message to PM (attempt {})",
                                self.id.name,
                                attempt
                            );
                            return Ok(());
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Worker {} failed to send message to PM (attempt {}): {:?}",
                                self.id.name,
                                attempt,
                                e
                            );
                            
                            if attempt < max_retries {
                                // Exponential backoff: 100ms, 200ms, 400ms, 800ms
                                let delay_ms = 100 * (2_u64.pow(attempt - 1));
                                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                            }
                        }
                    }
                }
                None => {
                    tracing::warn!(
                        "Worker {} could not find PM for project {} (attempt {})",
                        self.id.name,
                        project_id,
                        attempt
                    );
                    
                    if attempt < max_retries {
                        // Wait before retrying PM lookup
                        let delay_ms = 100 * (2_u64.pow(attempt - 1));
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }
        
        Err(anyhow::anyhow!(
            "Failed to send message to PM after {} attempts",
            max_retries
        ))
    }
    
    /// Wait for PM validation with real task polling
    pub async fn await_validation(&mut self) -> Result<bool> {
        let task_id = self.current_task.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No task assigned"))?
            .clone();
        
        tracing::info!(
            "Worker {} awaiting PM validation for task {}",
            self.id.name,
            task_id
        );
        
        // Poll every 100ms for status changes
        let poll_interval = tokio::time::Duration::from_millis(100);
        let max_wait = tokio::time::Duration::from_secs(60); // 1 minute timeout
        let start = tokio::time::Instant::now();
        
        loop {
            if start.elapsed() > max_wait {
                return Err(anyhow::anyhow!(
                    "Validation timeout after {}s", 
                    max_wait.as_secs()
                ));
            }
            
            // Get current task status
            let task_status = {
                let pm = self.project_manager.read().await;
                pm.get_task_status(&task_id).await?
            };
            
            match task_status {
                crate::projects::TaskStatus::Complete => {
                    tracing::info!(
                        "Worker {} task {} approved by PM",
                        self.id.name,
                        task_id
                    );
                    
                    // Transition to Idle
                    self.state_machine.transition(
                        AgentState::Idle,
                        "Task approved by PM".to_string()
                    )?;
                    self.current_task = None;
                    return Ok(true);
                }
                
                crate::projects::TaskStatus::NeedsRevision => {
                    tracing::warn!(
                        "Worker {} task {} needs revision",
                        self.id.name,
                        task_id
                    );
                    
                    return self.handle_revision_request(&task_id).await;
                }
                
                crate::projects::TaskStatus::Failed => {
                    let task = {
                        let pm = self.project_manager.read().await;
                        pm.get_task(&task_id).await?
                    };
                    
                    return Err(anyhow::anyhow!(
                        "Task failed"
                    ));
                }
                
                crate::projects::TaskStatus::UnderReview => {
                    // Still waiting for PM validation
                    tokio::time::sleep(poll_interval).await;
                    continue;
                }
                
                crate::projects::TaskStatus::InProgress => {
                    // Check if this is a revision reset by PM
                    let task = {
                        let pm = self.project_manager.read().await;
                        pm.get_task(&task_id).await?
                    };
                    
                    if let Some(feedback) = task.pm_feedback {
                        tracing::info!("Worker {} detected PM reset for revision: {}", self.id.name, feedback);
                        
                        // Transition to Planning
                        self.state_machine.transition(
                            AgentState::Planning,
                            format!("Revision requested: {}", feedback)
                        )?;
                        
                        // Re-execute task
                        self.execute_task().await?;
                        
                        // Recurse to wait for validation of the new attempt
                        // This ensures we don't return until the task is finally approved or failed
                        return Box::pin(self.await_validation()).await;
                    }
                    
                    // Task is still in progress but no feedback? 
                    // This might be a race where PM reset it but we haven't seen feedback yet, or a crash recovery.
                    tracing::warn!(
                        "Worker {} found task {} still in InProgress during validation wait. Feedback: {:?}. Waiting for status change...",
                        self.id.name,
                        task_id,
                        task.pm_feedback
                    );
                    tokio::time::sleep(poll_interval).await;
                    continue;
                }
                
                crate::projects::TaskStatus::Stuck => {
                    tracing::error!(
                        "Worker {} task {} marked as Stuck by PM. Manual intervention required.",
                        self.id.name,
                        task_id
                    );
                    
                    self.state_machine.transition(
                        AgentState::Idle,
                        "Task marked as stuck".to_string()
                    )?;
                    self.current_task = None;
                    
                    return Err(anyhow::anyhow!("Task marked as stuck by PM"));
                }
                
                _ => {
                    tracing::warn!(
                        "Worker {} encountered unexpected task status {:?} for task {}",
                        self.id.name,
                        task_status,
                        task_id
                    );
                    tokio::time::sleep(poll_interval).await;
                    continue;
                }
            }
        }
    }
    
    /// Handle revision request from PM
    fn handle_revision_request<'a>(&'a mut self, task_id: &'a TaskId) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool>> + Send + 'a>> {
        Box::pin(async move {
            let task = {
                let pm = self.project_manager.read().await;
                pm.get_task(task_id).await?
            };
            
            tracing::info!(
                "Worker {} handling revision request (attempt {}/{})",
                self.id.name,
                task.revision_count,
                task.max_revisions
            );
            
            if !task.can_retry_revision() {
                return Err(anyhow::anyhow!(
                    "Max revisions ({}) exceeded", 
                    task.max_revisions
                ));
            }
            
            // Get PM feedback
        let feedback = task.pm_feedback.clone()
            .unwrap_or_else(|| "No specific feedback provided".to_string());
        
        tracing::info!(
            "Worker {} revision feedback: {}",
            self.id.name,
            feedback
        );
        
        // Transition back to Planning to retry with feedback in context
        self.state_machine.transition(
            AgentState::Planning,
            format!("Revision requested: {}", feedback)
        )?;
        
        // Mark as InProgress to indicate we are working on it
        {
            let pm = self.project_manager.write().await;
            // We use start_task to set it to InProgress
            // This might fail if it's already InProgress, but that's fine (idempotent-ish check)
            if let Err(e) = pm.start_task(task_id).await {
                tracing::debug!("Worker {} could not set task to InProgress (might already be): {}", self.id.name, e);
            }
        }
        
        // Re-execute task with PM feedback
        self.execute_task().await?;
        
        // Wait for validation again (use Box::pin to avoid infinite size)
        self.await_validation().await
    })
    }
    
    /// Handle error and transition to Error state
    pub fn handle_error(&mut self, error: String) {
        self.state_machine.force_error(error);
    }
    
    /// Get task details from project manager
    async fn get_task_details(&self, task_id: &TaskId) -> Result<crate::projects::Task> {
        let pm = self.project_manager.read().await;
        let projects = pm.list_active_projects().await?;
        
        for project in projects {
            let tasks = pm.get_project_tasks(&project.id).await?;
            if let Some(task) = tasks.iter().find(|t| &t.id == task_id) {
                return Ok(task.clone());
            }
        }
        
        Err(anyhow::anyhow!("Task not found: {}", task_id))
    }
    
    /// Plan task execution with intelligent tool selection
    async fn plan_task_execution_with_learning(&mut self, task: &crate::projects::Task) -> Result<ExecutionPlan> {
        // Discover available tools
        let available_tools = self.discover_tools().await?;
        
        // Select best tool based on task title (proxy for task type)
        let recommended_tool = self.tool_selector.select_best_tool(&task.title, &available_tools);
        
        tracing::info!(
            "Worker {} recommended tool for '{}': {}",
            self.id.name,
            task.title,
            recommended_tool
        );
        
        // Generate planning prompt with tool recommendation and available servers
        let planning_prompt = match self.generate_planning_prompt_with_servers(&task.description).await {
            Ok(prompt) => format!(
                "{}\n\nRECOMMENDED TOOL (based on history): {}\nConsider using this tool if applicable.",
                prompt,
                recommended_tool
            ),
            Err(e) => {
                tracing::warn!("Failed to generate enhanced prompt with servers: {}. Falling back to basic prompt.", e);
                format!(
                    "{}\n\nRECOMMENDED TOOL (based on history): {}\nConsider using this tool if applicable.",
                    self.generate_planning_prompt(&task.description),
                    recommended_tool
                )
            }
        };
        
        let options = GenerationOptions {
            temperature: Some(0.1),
            max_tokens: Some(4096),
            system: Some(self.template.system_prompt.clone()),
            ..Default::default()
        };

        // Load user preference for Worker agent if available
        let preferred_family = if let Some(ref user_settings) = self.user_settings {
            let settings = user_settings.read().await;
            match settings.get_model_preference("worker").await {
                Ok(Some(family)) => {
                    tracing::info!("✅ Loaded user preference for Worker: family='{}'", family);
                    Some(family)
                },
                Ok(None) => {
                    tracing::debug!("No user preference set for Worker agent");
                    None
                },
                Err(e) => {
                    tracing::error!("Failed to load user preference for Worker: {:?}", e);
                    None
                }
            }
        } else {
            None
        };
        
        let selection_context = SelectionContext::for_worker();
        let selected_model = self
            .ai_provider_manager
            .select_model_for_agent_with_preferences(selection_context, preferred_family)
            .await
            .context("Failed to select a model for planning")?;

        let client = selected_model.get_client()?;
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };
        
        // Add timeout wrapper to prevent indefinite hanging
        // Use execution_strategy timeout (default 120s for complex tasks)
        tracing::info!("[DIAGNOSTIC] Worker {} calling LLM for planning (model: {})", self.id.name, model_name);
        let llm_timeout = tokio::time::Duration::from_millis(
            (self.execution_strategy.base_timeout_ms * 2) as u64 // 2x base timeout for LLM calls
        );
        let response = tokio::time::timeout(
            llm_timeout,
            client.generate(model_name, &planning_prompt, options)
        )
        .await
        .context(format!("LLM generation timed out after {:?}", llm_timeout))?
        .context("Failed to generate execution plan with LLM")?;
        
        tracing::debug!(
            target: "llm_messages",
            "[WORKER PLANNING RESPONSE] Model: {}, Response ({} chars):\n{}",
            model_name,
            response.text.len(),
            response.text
        );

        self.parse_execution_plan(&response.text).await
    }
    
    /// Execute plan with adaptive retry and self-correction
    async fn execute_with_learning(&mut self, plan: &ExecutionPlan, task: &crate::projects::Task) -> Result<Vec<String>> {
        let mut deliverables = Vec::new();
        
        for (idx, step) in plan.steps.iter().enumerate() {
            tracing::info!(
                "Worker {} executing step {}/{}: {}",
                self.id.name,
                idx + 1,
                plan.steps.len(),
                step.description
            );
            
            let step_start = SystemTime::now();
            let mut retry_count = 0u32;
            
            let result = loop {
                retry_count += 1;
                
                match self.execute_step(step).await {
                    Ok(result) => {
                        // Record successful step
                        let duration_ms = step_start.elapsed()
                            .unwrap_or_default()
                            .as_millis() as u64;
                        
                        let outcome = TaskOutcome {
                            task_type: task.title.clone(),
                            tool_used: step.tool.clone(),
                            success: true,
                            duration_ms,
                            retry_count: retry_count.saturating_sub(1),
                            error_category: None,
                            timestamp: SystemTime::now(),
                        };
                        
                        self.learner.record_outcome(outcome.clone());
                        self.tool_selector.record_outcome(outcome);
                        
                        break result;
                    }
                    Err(error) => {
                        // Self-correction check
                        if self.self_correction_enabled {
                            let error_category = ErrorCategory::classify(&error.to_string());
                            
                            tracing::warn!(
                                "Worker {} step failed (attempt {}): {:?} - {}",
                                self.id.name,
                                retry_count,
                                error_category,
                                error
                            );
                            
                            match error_category {
                                ErrorCategory::Transient => {
                                    // Retry with adaptive backoff
                                    if retry_count <= self.execution_strategy.max_retries {
                                        let delay_ms = self.execution_strategy.retry_delay_ms(retry_count);
                                        tracing::info!("Retrying in {}ms (transient error)", delay_ms);
                                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                                        continue;
                                    } else {
                                        // Max retries exceeded
                                        self.record_step_failure(task, step, retry_count, error_category);
                                        return Err(anyhow::anyhow!(
                                            "Step {} failed after {} retries: {}",
                                            idx + 1,
                                            self.execution_strategy.max_retries,
                                            error
                                        ));
                                    }
                                }
                                ErrorCategory::Permanent => {
                                    // No retry, request help from PM
                                    tracing::error!("Permanent error detected, requesting PM help");
                                    self.record_step_failure(task, step, retry_count, error_category);
                                    return Err(anyhow::anyhow!(
                                        "Permanent error (requesting PM help): {}",
                                        error
                                    ));
                                }
                                ErrorCategory::Unknown => {
                                    // Retry once, then request help
                                    if retry_count == 1 {
                                        tracing::info!("Retrying unknown error once");
                                        let delay_ms = self.execution_strategy.retry_delay_ms(retry_count);
                                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                                        continue;
                                    } else {
                                        tracing::error!("Unknown error persists, requesting PM help");
                                        self.record_step_failure(task, step, retry_count, error_category);
                                        return Err(error);
                                    }
                                }
                            }
                        } else {
                            // Self-correction disabled, use old retry logic
                            if retry_count <= self.execution_strategy.max_retries {
                                let delay_ms = 500 * retry_count as u64;
                                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                                continue;
                            } else {
                                return Err(error);
                            }
                        }
                    }
                }
            };
            
            deliverables.push(format!("Step {}: {} - {}", idx + 1, step.description, result));
        }
        
        Ok(deliverables)
    }
    
    /// Record success outcome for learning
    fn record_success_outcome(&mut self, task: &crate::projects::Task, start_time: SystemTime, plan: &ExecutionPlan) {
        let duration_ms = start_time.elapsed()
            .unwrap_or_default()
            .as_millis() as u64;
        
        // Record aggregate outcome for each tool used
        for step in &plan.steps {
            let outcome = TaskOutcome {
                task_type: task.title.clone(),
                tool_used: step.tool.clone(),
                success: true,
                duration_ms,
                retry_count: 0,
                error_category: None,
                timestamp: SystemTime::now(),
            };
            
            self.tool_selector.record_outcome(outcome);
        }
        
        tracing::info!(
            "Worker {} recorded success: task='{}', duration={}ms, tools={}",
            self.id.name,
            task.title,
            duration_ms,
            plan.steps.len()
        );
    }
    
    /// Record failure outcome for learning
    fn record_failure_outcome(&mut self, task: &crate::projects::Task, start_time: SystemTime, plan: &ExecutionPlan, error: &anyhow::Error) {
        let duration_ms = start_time.elapsed()
            .unwrap_or_default()
            .as_millis() as u64;
        let error_category = ErrorCategory::classify(&error.to_string());
        
        for step in &plan.steps {
            let outcome = TaskOutcome {
                task_type: task.title.clone(),
                tool_used: step.tool.clone(),
                success: false,
                duration_ms,
                retry_count: self.execution_strategy.max_retries,
                error_category: Some(error_category),
                timestamp: SystemTime::now(),
            };
            
            self.tool_selector.record_outcome(outcome);
        }
        
        tracing::warn!(
            "Worker {} recorded failure: task='{}', error={:?}",
            self.id.name,
            task.title,
            error_category
        );
    }
    
    /// Record step failure for learning
    fn record_step_failure(&mut self, task: &crate::projects::Task, step: &ExecutionStep, retry_count: u32, category: ErrorCategory) {
        let outcome = TaskOutcome {
            task_type: task.title.clone(),
            tool_used: step.tool.clone(),
            success: false,
            duration_ms: 0,
            retry_count,
            error_category: Some(category),
            timestamp: SystemTime::now(),
        };
        
        self.learner.record_outcome(outcome);
    }
    
    /// Plan task execution using LLM (original method for backward compatibility)
    #[deprecated(note = "Use generate_execution_plan_discovery instead")]
    async fn plan_task_execution(&self, task_description: &str) -> Result<ExecutionPlan> {
        let planning_prompt = self.generate_planning_prompt(task_description);
        
        let options = GenerationOptions {
            temperature: Some(0.1),
            max_tokens: Some(4096),
            system: Some(self.template.system_prompt.clone()),
            ..Default::default()
        };

        // Load user preference for Worker agent if available
        let preferred_family = if let Some(ref user_settings) = self.user_settings {
            let settings = user_settings.read().await;
            match settings.get_model_preference("worker").await {
                Ok(Some(family)) => {
                    tracing::debug!("Loaded user preference for Worker: family='{}'", family);
                    Some(family)
                },
                Ok(None) => None,
                Err(e) => {
                    tracing::error!("Failed to load user preference for Worker: {:?}", e);
                    None
                }
            }
        } else {
            None
        };
        
        let selection_context = SelectionContext::for_worker();
        let selected_model = self
            .ai_provider_manager
            .select_model_for_agent_with_preferences(selection_context, preferred_family)
            .await
            .context("Failed to select a model for planning")?;
        
        let client = selected_model.get_client()?;
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };

        tracing::debug!("Worker {} sending planning prompt to LLM: {}", self.id.name, planning_prompt);
        let response = client.generate(
            model_name,
            &planning_prompt,
            options
        ).await.context("Failed to generate execution plan with LLM")?;
        
        tracing::debug!("Worker {} received raw plan from LLM: {}", self.id.name, response.text);
        self.parse_execution_plan(&response.text).await
    }
    
    /// Generate structured planning prompt for LLM
    async fn generate_planning_prompt_with_servers(&self, task_description: &str) -> Result<String> {
        // Discover available servers at planning time
        let available_tools = self.discover_tools().await?;
        let mut servers_set = std::collections::HashSet::new();
        for tool in &available_tools {
            if let Some(server) = tool.split("::").next() {
                servers_set.insert(server.to_string());
            }
        }
        let available_servers: Vec<String> = servers_set.into_iter().collect();
        
        Ok(format!(
            r#"You are a Worker AI agent executing a task. Analyze this task and create a structured execution plan.

TASK: {}
YOUR ROLE: {}
YOUR CAPABILITIES: {:?}

**CRITICAL - AVAILABLE MCP SERVERS:**
The following MCP servers are currently available:
{}

**YOU MUST ONLY USE TOOLS FROM THESE SERVERS. DO NOT use tools from any other servers (e.g., 'server', 'http', 'network').**

AVAILABLE MCP TOOLS:
{}

INSTRUCTIONS:
1. Break the task into concrete, executable steps.
2. Each step must use ONE MCP tool with specific parameters.
3. **TOOL FORMAT:** Tools must be in format `server_name::tool_name` where server_name is one of the available servers listed above.
4. Steps can have dependencies on previous steps.
5. Be specific with file paths, parameters, and expected outputs.
6. **IMPORTANT - Tool Selection:**
   - Use `hainet-files::directory_create` to create directories
   - Use `hainet-files::file_write` to create/write files
   - Use `hainet-files::file_read` to read existing files
   - Use `filesystem::read_file` to read files from the filesystem
   - Use `filesystem::write_file` to write files
   - Do not attempt to read files that do not exist yet
   - When setting up a new project, create directories FIRST using directory_create, then create files using file_write
   - **DO NOT** use tools from non-existent servers like 'server::http_requests' or 'server::file_write'

RESPOND WITH VALID JSON ONLY (no markdown, no explanations):
{{
  "steps": [
    {{
      "step_number": 1,
      "tool": "hainet-files::file_write",
      "params": {{ "path": "src/main.rs", "content": "// Project entry point" }},
      "description": "Create the main source file for the new project.",
      "depends_on": []
    }},
    {{
      "step_number": 2,
      "tool": "hainet-files::file_read",
      "params": {{ "path": "src/main.rs" }},
      "description": "Read the newly created file to verify its content.",
      "depends_on": [1]
    }}
  ]
}}

CRITICAL: Respond with ONLY the JSON object above. No markdown code blocks, no explanations.
CRITICAL: Only use tools from available servers: {}
"#,
            task_description,
            self.template.name,
            self.template.capabilities,
            available_servers.join(", "),
            self.format_available_tools(),
            available_servers.join(", ")
        ))
    }
    
    /// Generate structured planning prompt for LLM (backward compatible version)
    fn generate_planning_prompt(&self, task_description: &str) -> String {
        format!(
            r#"You are a Worker AI agent executing a task. Analyze this task and create a structured execution plan.

TASK: {}
YOUR ROLE: {}
YOUR CAPABILITIES: {:?}

AVAILABLE MCP TOOLS:
{}

INSTRUCTIONS:
1. Break the task into concrete, executable steps.
2. Each step must use ONE MCP tool with specific parameters.
3. Steps can have dependencies on previous steps.
4. Be specific with file paths, parameters, and expected outputs.
5. **IMPORTANT - Tool Selection:**
   - Use `hainet-files::directory_create` to create directories
   - Use `hainet-files::file_write` to create/write files
   - Use `hainet-files::file_read` to read existing files
   - Do not attempt to read files that do not exist yet
   - When setting up a new project, create directories FIRST using directory_create, then create files using file_write

RESPOND WITH VALID JSON ONLY (no markdown, no explanations):
{{
  "steps": [
    {{
      "step_number": 1,
      "tool": "hainet-files::file_write",
      "params": {{ "path": "src/main.rs", "content": "// Project entry point" }},
      "description": "Create the main source file for the new project.",
      "depends_on": []
    }},
    {{
      "step_number": 2,
      "tool": "hainet-files::file_read",
      "params": {{ "path": "src/main.rs" }},
      "description": "Read the newly created file to verify its content.",
      "depends_on": [1]
    }}
  ]
}}

CRITICAL: Respond with ONLY the JSON object above. No markdown code blocks, no explanations.
"#,
            task_description,
            self.template.name,
            self.template.capabilities,
            self.format_available_tools()
        )
    }
    
    /// Format available MCP tools for prompt
    fn format_available_tools(&self) -> String {
        let mut tools_desc = String::new();
        
        for server in &self.template.mcp_servers {
            tools_desc.push_str(&format!("\n{} server:\n", server));
            
            match server.as_str() {
                "hainet-files" => {
                    tools_desc.push_str("  - file_read(path) - Read file contents\n");
                    tools_desc.push_str("  - file_write(path, content) - Write to file\n");
                    tools_desc.push_str("  - file_list(path) - List directory contents\n");
                    tools_desc.push_str("  - file_metadata(path) - Get file metadata\n");
                    tools_desc.push_str("  - directory_create(path) - Create a directory\n");
                }
                "hainet-system" => {
                    tools_desc.push_str("  - system_status() - Get CPU, RAM, disk usage\n");
                    tools_desc.push_str("  - list_services() - List running services\n");
                    tools_desc.push_str("  - check_health() - Run health checks\n");
                }
                "hainet-dev" => {
                    tools_desc.push_str("  - git_status(repo_path) - Get git status\n");
                    tools_desc.push_str("  - git_diff(repo_path, file_path) - View changes\n");
                    tools_desc.push_str("  - cargo_build(package, release) - Build Rust project\n");
                    tools_desc.push_str("  - cargo_test(package, filter) - Run tests\n");
                    tools_desc.push_str("  - code_search(pattern, path) - Search codebase\n");
                }
                _ => {}
            }
        }
        
        tools_desc
    }
    
    /// Parse LLM response into ExecutionPlan (enhanced with multi-strategy parsing and validation)
    async fn parse_execution_plan(&self, llm_response: &str) -> Result<ExecutionPlan> {
        // Get available servers for validation
        let available_tools = self.discover_tools().await?;
        let mut available_servers = std::collections::HashSet::new();
        for tool in &available_tools {
            if let Some(server) = tool.split("::").next() {
                available_servers.insert(server.to_string());
            }
        }
        
        // Try to parse the execution plan with multiple strategies
        let parse_result: Result<Vec<ExecutionStep>> = (|| {
            // Strategy 1: Direct JSON extraction (simple case)
            let json_str = self.extract_json_from_response(llm_response);
            
            // Strategy 2: Try direct parse
            let parsed = match serde_json::from_str::<serde_json::Value>(&json_str) {
                Ok(value) => value,
                Err(_) => {
                    // Strategy 3: Try markdown extraction
                    if let Ok(value) = self.extract_from_markdown(llm_response) {
                        value
                    } else {
                        // Strategy 4: Try repair (braces/brackets)
                        self.repair_and_parse(&json_str)?
                    }
                }
            };
            
            let steps: Vec<ExecutionStep> = parsed["steps"].as_array()
                .ok_or_else(|| anyhow::anyhow!("Missing 'steps' array in execution plan"))?
                .iter()
                .enumerate()
                .filter_map(|(idx, s)| {
                    let _step_number = s["step_number"].as_u64().unwrap_or((idx + 1) as u64) as usize;
                    let tool = s["tool"].as_str()?.to_string();
                    let params = s["params"].clone();
                    let description = s["description"].as_str().unwrap_or("").to_string();
                    
                    // Validate tool format (server::tool_name)
                    if !tool.contains("::") {
                        tracing::warn!("Invalid tool format (missing ::): {}", tool);
                        return None;
                    }
                    
                    // Validate server exists
                    if let Some(server_name) = tool.split("::").next() {
                        if !available_servers.contains(server_name) {
                            tracing::warn!(
                                "Tool '{}' references non-existent server '{}'. Available servers: {:?}",
                                tool,
                                server_name,
                                available_servers
                            );
                            return None;
                        }
                    }
                    
                    Some(ExecutionStep {
                        tool,
                        params,
                        description,
                    })
                })
                .collect();
            
            Ok(steps)
        })();
        
        // Check if parsing succeeded and has valid steps
        let steps = match parse_result {
            Ok(steps) if !steps.is_empty() => steps,
            Ok(_) => {
                // Parsing succeeded but no valid steps found
                tracing::warn!(
                    "No valid execution steps found in plan. Response snippet: {}. Attempting fallback to simple plan.",
                    &llm_response[..llm_response.len().min(200)]
                );
                return self.create_fallback_plan(llm_response).await;
            }
            Err(e) => {
                // Parsing failed completely
                tracing::warn!(
                    "Failed to parse execution plan: {}. Response snippet: {}. Attempting fallback to simple plan.",
                    e,
                    &llm_response[..llm_response.len().min(200)]
                );
                return self.create_fallback_plan(llm_response).await;
            }
        };
        
        // Validate the plan before returning
        self.validate_execution_plan(&steps)?;
        
        tracing::debug!("Parsed {} execution steps from LLM response", steps.len());
        for (i, step) in steps.iter().enumerate() {
            tracing::debug!("Step {}: {} (Tool: {})", i + 1, step.description, step.tool);
        }
        Ok(ExecutionPlan { steps })
    }
    
    /// Validate execution plan for common issues
    fn validate_execution_plan(&self, steps: &[ExecutionStep]) -> Result<()> {
        if steps.is_empty() {
            return Err(anyhow::anyhow!("Execution plan has no steps"));
        }
        
        // Check for reasonable number of steps (not too many)
        if steps.len() > 50 {
            tracing::warn!("Execution plan has {} steps, which seems excessive", steps.len());
        }
        
        // Validate each step has required fields
        for (idx, step) in steps.iter().enumerate() {
            if step.tool.is_empty() {
                return Err(anyhow::anyhow!("Step {} has empty tool name", idx + 1));
            }
            if step.description.is_empty() {
                tracing::warn!("Step {} has empty description", idx + 1);
            }
        }
        
        Ok(())
    }
    
    /// Create a fallback execution plan when parsing fails
    /// This prevents permanent task failure by creating a simple plan
    async fn create_fallback_plan(&self, _llm_response: &str) -> Result<ExecutionPlan> {
        tracing::warn!("Creating fallback execution plan due to parsing failure");
        
        // Discover available tools to find a safe default
        let available_tools = self.discover_tools().await?;
        
        // Try to find a file listing tool as a safe default action
        let default_tool = available_tools.iter()
            .find(|t| t.contains("file_list") || t.contains("list"))
            .or_else(|| available_tools.first())
            .ok_or_else(|| anyhow::anyhow!("No tools available for fallback plan"))?;
        
        tracing::info!("Fallback plan using tool: {}", default_tool);
        
        // Create a simple single-step plan
        let fallback_step = ExecutionStep {
            tool: default_tool.clone(),
            params: serde_json::json!({ "path": "." }),
            description: "Fallback: List current directory to understand project structure".to_string(),
        };
        
        Ok(ExecutionPlan {
            steps: vec![fallback_step],
        })
    }
    
    /// Parse LLM response into ExecutionPlan (backward compatible version without validation)
    fn parse_execution_plan_legacy(&self, llm_response: &str) -> Result<ExecutionPlan> {
        // Strategy 1: Direct JSON extraction (simple case)
        let json_str = self.extract_json_from_response(llm_response);
        
        // Strategy 2: Try direct parse
        let parsed = match serde_json::from_str::<serde_json::Value>(&json_str) {
            Ok(value) => value,
            Err(_) => {
                // Strategy 3: Try markdown extraction
                if let Ok(value) = self.extract_from_markdown(llm_response) {
                    value
                } else {
                    // Strategy 4: Try repair (braces/brackets)
                    self.repair_and_parse(&json_str)?
                }
            }
        };
        
        let steps: Vec<ExecutionStep> = parsed["steps"].as_array()
            .ok_or_else(|| anyhow::anyhow!("Missing 'steps' array in execution plan"))?
            .iter()
            .enumerate()
            .filter_map(|(idx, s)| {
                let _step_number = s["step_number"].as_u64().unwrap_or((idx + 1) as u64) as usize;
                let tool = s["tool"].as_str()?.to_string();
                let params = s["params"].clone();
                let description = s["description"].as_str().unwrap_or("").to_string();
                
                // Validate tool format (server::tool_name)
                if !tool.contains("::") {
                    tracing::warn!("Invalid tool format (missing ::): {}", tool);
                    return None;
                }
                
                Some(ExecutionStep {
                    tool,
                    params,
                    description,
                })
            })
            .collect();
        
        if steps.is_empty() {
            return Err(anyhow::anyhow!(
                "No valid execution steps found in plan. Response: {}",
                &llm_response[..llm_response.len().min(200)]
            ));
        }
        
        tracing::debug!("Parsed {} execution steps from LLM response", steps.len());
        for (i, step) in steps.iter().enumerate() {
            tracing::debug!("Step {}: {} (Tool: {})", i + 1, step.description, step.tool);
        }
        Ok(ExecutionPlan { steps })
    }
    
    /// Extract JSON from LLM response (handles braces)
    fn extract_json_from_response(&self, response: &str) -> String {
        if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                return response[start..=end].to_string();
            }
        }
        response.to_string()
    }
    
    /// Extract JSON from markdown code blocks
    fn extract_from_markdown(&self, text: &str) -> Result<serde_json::Value> {
        let markers = ["```json\n", "```\n", "```"];
        
        for marker in markers.iter() {
            if let Some(start_idx) = text.find(marker) {
                let json_start = start_idx + marker.len();
                
                if let Some(end_idx) = text[json_start..].find("```") {
                    let json_text = &text[json_start..json_start + end_idx];
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_text.trim()) {
                        return Ok(value);
                    }
                }
            }
        }
        
        Err(anyhow::anyhow!("No valid JSON found in markdown blocks"))
    }
    
    /// Repair common JSON issues (missing braces/brackets)
    fn repair_and_parse(&self, text: &str) -> Result<serde_json::Value> {
        let mut repaired = text.trim().to_string();
        
        // Count braces and brackets
        let open_braces = repaired.matches('{').count();
        let close_braces = repaired.matches('}').count();
        let open_brackets = repaired.matches('[').count();
        let close_brackets = repaired.matches(']').count();
        
        // Add missing closing braces
        if open_braces > close_braces {
            for _ in 0..(open_braces - close_braces) {
                repaired.push('}');
            }
        }
        
        // Add missing closing brackets
        if open_brackets > close_brackets {
            for _ in 0..(open_brackets - close_brackets) {
                repaired.push(']');
            }
        }
        
        serde_json::from_str::<serde_json::Value>(&repaired)
            .context("Failed to parse after JSON repair")
    }
    
    /// Execute plan with retry logic (original method for backward compatibility)
    async fn execute_with_retries(&self, plan: &ExecutionPlan) -> Result<Vec<String>> {
        let mut deliverables = Vec::new();
        
        for (idx, step) in plan.steps.iter().enumerate() {
            tracing::info!("Worker {} executing step {}/{}: {}", 
                           self.id.name, idx + 1, plan.steps.len(), step.description);
            
            let mut attempts = 0;
            let result = loop {
                attempts += 1;
                
                match self.execute_step(step).await {
                    Ok(result) => break result,
                    Err(e) if attempts < self.max_retries => {
                        tracing::warn!("Worker {} step failed (attempt {}/{}): {}",
                                       self.id.name, attempts, self.max_retries, e);
                        tokio::time::sleep(tokio::time::Duration::from_millis(500 * attempts as u64)).await;
                        continue;
                    }
                    Err(e) => {
                        return Err(anyhow::anyhow!(
                            "Step {} failed after {} attempts: {}", 
                            idx + 1, self.max_retries, e
                        ));
                    }
                }
            };
            
            deliverables.push(format!("Step {}: {} - {}", idx + 1, step.description, result));
        }
        
        Ok(deliverables)
    }
    
    /// Execute single step with MCP tool
    async fn execute_step(&self, step: &ExecutionStep) -> Result<String> {
        // Parse tool name: "server::tool_name"
        let parts: Vec<&str> = step.tool.split("::").collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid tool format: {}", step.tool));
        }
        
        let (server, tool) = (parts[0], parts[1]);
        
        // Add project_name to params for sandboxing (Session 52)
        let mut params = step.params.clone();
        if let Some(ref project_name) = self.current_project_name {
            if let Some(obj) = params.as_object_mut() {
                obj.insert("project_name".to_string(), serde_json::Value::String(project_name.clone()));
            }
        }
        
        let mcp_client = self.mcp_client.read().await;
        let result = mcp_client.call_tool(server, tool, params).await?;
        
        Ok(result.to_string())
    }
    
    /// Execute file-related task using hainet-files MCP server
    async fn execute_file_task(&self, task_description: &str) -> Result<Vec<String>> {
        let mcp_client = self.mcp_client.read().await;
        
        // Simple task parsing - in production this would use LLM
        // For now, support basic file operations
        if task_description.contains("read") || task_description.contains("get") {
            let path = self.extract_path_from_task(task_description);
            
            let result = mcp_client.call_tool(
                "hainet-files",
                "hainet_file_read",
                json!({ "path": path })
            ).await?;
            
            Ok(vec![format!("Read file: {}", result)])
        } else if task_description.contains("write") || task_description.contains("create") {
            let path = self.extract_path_from_task(task_description);
            let content = "Generated content"; // Would be LLM-generated
            
            let result = mcp_client.call_tool(
                "hainet-files",
                "hainet_file_write",
                json!({ "path": path, "content": content })
            ).await?;
            
            Ok(vec![format!("Wrote file: {}", result)])
        } else if task_description.contains("list") {
            let path = self.extract_path_from_task(task_description);
            
            let result = mcp_client.call_tool(
                "hainet-files",
                "hainet_file_list",
                json!({ "path": path })
            ).await?;
            
            Ok(vec![format!("Listed directory: {}", result)])
        } else {
            Ok(vec!["File operation completed".to_string()])
        }
    }
    
    /// Execute generic task - tries to auto-detect task type
    async fn execute_generic_task(&self, task_description: &str) -> Result<Vec<String>> {
        // Try file operations first
        if task_description.contains("file") || task_description.contains("directory") {
            return self.execute_file_task(task_description).await;
        }
        
        // Default fallback
        Ok(vec!["Generic task completed".to_string()])
    }
    
    /// Extract file path from task description (simplified NLP)
    fn extract_path_from_task(&self, task_description: &str) -> String {
        // Very simple path extraction - would use LLM in production
        if let Some(start) = task_description.find("/") {
            let remaining = &task_description[start..];
            if let Some(end) = remaining.find(" ") {
                remaining[..end].to_string()
            } else {
                remaining.to_string()
            }
        } else {
            "/tmp/test.txt".to_string()
        }
    }
    
    /// Discover available tools from connected MCP servers (with timeout)
    pub async fn discover_tools(&self) -> Result<Vec<String>> {
        tracing::info!("[DIAGNOSTIC] Worker {} starting tool discovery", self.id.name);
        
        // Wrap discovery in timeout to prevent hanging
        let discovery_timeout = tokio::time::Duration::from_secs(10);
        
        match tokio::time::timeout(discovery_timeout, self.discover_tools_internal()).await {
            Ok(result) => {
                tracing::info!("[DIAGNOSTIC] Worker {} tool discovery completed", self.id.name);
                result
            }
            Err(_) => {
                let error_msg = format!(
                    "Tool discovery timed out after {:?}. MCP servers may not be responding.",
                    discovery_timeout
                );
                tracing::error!("[DIAGNOSTIC] Worker {} {}", self.id.name, error_msg);
                Err(anyhow::anyhow!(error_msg))
            }
        }
    }
    
    /// Internal tool discovery implementation (without timeout wrapper)
    async fn discover_tools_internal(&self) -> Result<Vec<String>> {
        let mcp_client = self.mcp_client.read().await;
        
        tracing::info!("[DIAGNOSTIC] Worker {} calling list_servers()", self.id.name);
        let servers = mcp_client.list_servers().await;
        tracing::info!("[DIAGNOSTIC] Worker {} found {} servers: {:?}", self.id.name, servers.len(), servers);
        
        // SAFETY: Filter out the 'filesystem' server to prevent workers from writing outside project sandboxes
        // The 'filesystem' server has broad access without project-specific sandboxing
        // Workers should ONLY use 'hainet-files' which properly sandboxes to project directories
        let safe_servers: Vec<String> = servers.into_iter()
            .filter(|server| {
                if server == "filesystem" {
                    tracing::warn!(
                        "Worker {} blocked from using 'filesystem' server (security: no project sandboxing). Use 'hainet-files' instead.",
                        self.id.name
                    );
                    false
                } else {
                    true
                }
            })
            .collect();
        
        tracing::info!("[DIAGNOSTIC] Worker {} using {} safe servers (filtered out unsafe servers)", self.id.name, safe_servers.len());
        
        let mut all_tools = Vec::new();
        
        for server_name in safe_servers {
            tracing::debug!("Worker {} listing tools for server: {}", self.id.name, server_name);
            let tools = mcp_client.list_tools(&server_name).await?;
            tracing::debug!("Worker {} found {} tools in {}", self.id.name, tools.len(), server_name);
            
            for tool in tools {
                all_tools.push(format!("{}::{}", server_name, tool.name));
            }
        }
        
        tracing::info!("[DIAGNOSTIC] Worker {} discovered {} total tools", self.id.name, all_tools.len());
        
        Ok(all_tools)
    }
    
    /// Get reference to mcp_client (for testing)
    pub fn mcp_client(&self) -> &Arc<RwLock<MCPClientManager>> {
        &self.mcp_client
    }
    
    /// Get mutable reference to state machine (for testing)
    pub fn state_machine_mut(&mut self) -> &mut AgentStateMachine {
        &mut self.state_machine
    }
    
    /// Get reference to project manager (for testing)
    pub fn project_manager(&self) -> &Arc<RwLock<ProjectManager>> {
        &self.project_manager
    }
    
    /// Get reference to template (for testing)
    pub fn template(&self) -> &WorkerTemplate {
        &self.template
    }
    
    /// Get reference to learner (for testing and monitoring)
    pub fn learner(&self) -> &WorkerLearner {
        &self.learner
    }
    
    /// Get mutable reference to learner (for testing)
    pub fn learner_mut(&mut self) -> &mut WorkerLearner {
        &mut self.learner
    }
    
    /// Get reference to execution strategy (for monitoring)
    pub fn execution_strategy(&self) -> &ExecutionStrategy {
        &self.execution_strategy
    }
    
    /// Get reference to tool selector (for monitoring)
    pub fn tool_selector(&self) -> &ToolSelector {
        &self.tool_selector
    }
    
    /// Set self-correction enabled/disabled
    pub fn set_self_correction(&mut self, enabled: bool) {
        self.self_correction_enabled = enabled;
    }
    
    /// Get reference to session tasks (for monitoring)
    pub fn session_tasks(&self) -> &SessionTaskList {
        &self.session_tasks
    }
    
    /// Get mutable reference to session tasks (for testing)
    pub fn session_tasks_mut(&mut self) -> &mut SessionTaskList {
        &mut self.session_tasks
    }
    
    // ========================================================================
    // DISCOVERY-BASED EXECUTION METHODS (NEW)
    // ========================================================================
    
    /// Execute task using discovery-based approach
    /// Execute task using discovery-based approach with sub-task decomposition
    async fn execute_with_discovery(&mut self, task: &crate::projects::Task) -> Result<Vec<String>> {
        // Step 1: Discover available tools (names only)
        let available_tools = self.discover_tools().await?;
        tracing::info!(
            "Worker {} discovered {} available tools",
            self.id.name,
            available_tools.len()
        );

        // Step 2: Decompose into sub-tasks
        let subtasks = self.generate_subtasks(task).await?;
        tracing::info!("Worker {} decomposed task into {} sub-tasks", self.id.name, subtasks.len());
        
        let mut all_deliverables = Vec::new();
        
        for subtask in subtasks {
            tracing::info!("Worker {} starting sub-task {}: {}", self.id.name, subtask.id, subtask.description);
            
            // Execute sub-task loop (Plan -> Execute -> Verify -> Refine)
            let subtask_deliverables = self.execute_subtask_loop(task, &subtask, &available_tools).await?;
            all_deliverables.extend(subtask_deliverables);
        }
        
        Ok(all_deliverables)
    }

    /// Execute a single sub-task with verification loop
    async fn execute_subtask_loop(
        &mut self,
        main_task: &crate::projects::Task,
        subtask: &SubTask,
        available_tools: &[String],
    ) -> Result<Vec<String>> {
        let max_retries = 3;
        let mut attempts = 0;
        let mut feedback = String::new();
        
        loop {
            attempts += 1;
            if attempts > max_retries {
                return Err(anyhow::anyhow!("Max retries reached for sub-task {}", subtask.id));
            }

            // 1. Tool Selection (focused on sub-task)
            let tool_selection = self.identify_needed_tools_for_subtask(main_task, subtask, available_tools, &feedback).await?;
            
            // 2. Load Metadata
            let tool_metadata = self.load_tool_metadata(&tool_selection.needed_tools).await?;
            
            // 3. Plan Execution
            let execution_plan = self.generate_plan_for_subtask(main_task, subtask, &tool_metadata, &feedback).await?;
            
            // 4. Execute Plan
            let deliverables = match self.execute_discovery_plan(&execution_plan, main_task).await {
                Ok(d) => d,
                Err(e) => {
                    feedback = format!("Execution error: {}", e);
                    continue;
                }
            };
            
            // 5. Verify
            let verification = self.verify_execution(main_task, subtask, &deliverables).await?;
            
            if verification.success {
                tracing::info!("Worker {} verified sub-task {}", self.id.name, subtask.id);
                return Ok(deliverables);
            } else {
                tracing::warn!("Worker {} verification failed for sub-task {}: {}", self.id.name, subtask.id, verification.feedback);
                feedback = format!("Verification failed: {}. Issues: {:?}", verification.feedback, verification.issues);
            }
        }
    }

    async fn generate_subtasks(&self, task: &crate::projects::Task) -> Result<Vec<SubTask>> {
        let prompt = format!(
            r#"You are a Worker AI decomposing a complex task.
TASK: {}
DESCRIPTION: {}

INSTRUCTIONS:
1. Break this task into 1-5 logical sub-tasks.
2. Each sub-task must be atomic and verifiable.
3. Define clear verification criteria for each (e.g., "File X exists and contains Y").

RESPOND WITH JSON ONLY:
{{
  "subtasks": [
    {{
      "id": 1,
      "description": "Create project structure",
      "goal": "Initialize directories and empty files",
      "verification_criteria": "Directory /src exists, main.rs exists"
    }}
  ]
}}
"#,
            task.title,
            task.description
        );

        let options = GenerationOptions {
            temperature: Some(0.3),
            max_tokens: Some(2048),
            system: Some(self.template.system_prompt.clone()),
            ..Default::default()
        };
        
        let selection_context = SelectionContext::for_worker();
        let selected_model = self.ai_provider_manager.select_model_for_agent(selection_context).await?;
        let client = selected_model.get_client()?;
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };
        
        let response = client.generate(model_name, &prompt, options).await?;
        
        #[derive(Deserialize)]
        struct Response { subtasks: Vec<SubTask> }
        
        let json_str = super::worker_discovery::extract_json(&response.text)
            .ok_or_else(|| anyhow::anyhow!("Failed to extract JSON from response"))?;
        
        let res: Response = serde_json::from_str(&json_str)?;
        Ok(res.subtasks)
    }

    async fn identify_needed_tools_for_subtask(
        &self,
        main_task: &crate::projects::Task,
        subtask: &SubTask,
        available_tools: &[String],
        feedback: &str,
    ) -> Result<super::worker_discovery::ToolSelectionRequest> {
        let tool_list = format_tool_list(available_tools);
        let prompt = format!(
            r#"You are selecting tools for a sub-task.
MAIN TASK: {}
SUB-TASK: {}
GOAL: {}
FEEDBACK FROM PREVIOUS ATTEMPT: {}

AVAILABLE TOOLS:
{}

INSTRUCTIONS:
1. Select tools needed for this SPECIFIC sub-task.
2. Use file_search/file_edit for modifications if files exist.
3. Use directory_create/file_write for new content.

RESPOND WITH JSON ONLY:
{{
  "needed_tools": ["hainet-files::file_search", "hainet-files::file_edit"],
  "reasoning": "Need to find TODOs and replace them"
}}
"#,
            main_task.title,
            subtask.description,
            subtask.goal,
            feedback,
            tool_list
        );

        // Reuse existing selection logic/model
        let options = GenerationOptions {
            temperature: Some(0.3),
            max_tokens: Some(1024),
            system: Some(self.template.system_prompt.clone()),
            ..Default::default()
        };
        
        let selection_context = SelectionContext::for_worker();
        let selected_model = self.ai_provider_manager.select_model_for_agent(selection_context).await?;
        let client = selected_model.get_client()?;
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };
        
        let response = client.generate(model_name, &prompt, options).await?;
        parse_tool_selection(&response.text)
    }

    async fn generate_plan_for_subtask(
        &self,
        main_task: &crate::projects::Task,
        subtask: &SubTask,
        tool_metadata: &HashMap<String, String>,
        feedback: &str,
    ) -> Result<DiscoveryExecutionPlan> {
        let formatted_metadata = format_tool_metadata(tool_metadata);
        let prompt = format!(
            r#"You are planning execution for a sub-task.
MAIN TASK: {}
SUB-TASK: {}
GOAL: {}
VERIFICATION CRITERIA: {}
FEEDBACK: {}

TOOLS:
{}

CRITICAL RULES:
1. NO PLACEHOLDERS (e.g. // TODO). Write complete code.
2. Use file_edit to modify existing files.
3. Verify your own work if possible (e.g. read back file).

RESPOND WITH JSON ONLY (steps array).
"#,
            main_task.title,
            subtask.description,
            subtask.goal,
            subtask.verification_criteria,
            feedback,
            formatted_metadata
        );

        // Reuse existing planning logic/model
        let options = GenerationOptions {
            temperature: Some(0.2),
            max_tokens: Some(4096),
            system: Some(self.template.system_prompt.clone()),
            ..Default::default()
        };
        
        let selection_context = SelectionContext::for_worker();
        let selected_model = self.ai_provider_manager.select_model_for_agent(selection_context).await?;
        let client = selected_model.get_client()?;
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };
        
        let response = client.generate(model_name, &prompt, options).await?;
        parse_execution_plan(&response.text)
    }

    async fn verify_execution(
        &self,
        main_task: &crate::projects::Task,
        subtask: &SubTask,
        deliverables: &[String],
    ) -> Result<VerificationResult> {
        let prompt = format!(
            r#"You are verifying a sub-task execution.
SUB-TASK: {}
GOAL: {}
CRITERIA: {}

DELIVERABLES/LOGS:
{:?}

INSTRUCTIONS:
1. Check if the goal was met.
2. Check for placeholders (// TODO, etc) - FAIL if found.
3. Check for errors in logs.

RESPOND WITH JSON ONLY:
{{
  "success": true/false,
  "issues": ["issue 1", "issue 2"],
  "feedback": "Detailed feedback for refinement"
}}
"#,
            subtask.description,
            subtask.goal,
            subtask.verification_criteria,
            deliverables
        );

        let options = GenerationOptions {
            temperature: Some(0.1),
            max_tokens: Some(1024),
            system: Some(self.template.system_prompt.clone()),
            ..Default::default()
        };
        
        let selection_context = SelectionContext::for_worker();
        let selected_model = self.ai_provider_manager.select_model_for_agent(selection_context).await?;
        let client = selected_model.get_client()?;
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };
        
        let response = client.generate(model_name, &prompt, options).await?;
        
        let json_str = super::worker_discovery::extract_json(&response.text)
            .ok_or_else(|| anyhow::anyhow!("Failed to extract JSON from response"))?;
        
        serde_json::from_str(&json_str).context("Failed to parse verification result")
    }
    
    /// Ask LLM which tools it needs (Phase 1: Tool Selection)
    async fn identify_needed_tools_discovery(
        &self,
        task: &crate::projects::Task,
        available_tools: &[String],
    ) -> Result<super::worker_discovery::ToolSelectionRequest> {
        // Format minimal tool list
        let tool_list = format_tool_list(available_tools);
        
        // Generate planning prompt using TOML template
        let planning_prompt = format!(
            r#"You are a Worker AI agent planning task execution.

TASK: {}
YOUR ROLE: {}
CAPABILITIES: {:?}

SESSION PROGRESS:
{}

AVAILABLE TOOLS (use EXACT format shown):
{}

CRITICAL RULES FOR TOOL SELECTION:
1. Tool identifiers MUST use format: server::tool (e.g., 'hainet-files::file_write')
2. ONLY select tools from the AVAILABLE TOOLS list above
3. Do NOT invent tool names or use generic names like 'server::tool' or 'server::file_write'
4. Do NOT use tools that are not in the list above

COMMON TOOLS FOR FILE OPERATIONS:
- hainet-files::directory_create (create directories - NOT 'create-directory')
- hainet-files::file_write (write/create files)
- hainet-files::file_read (read existing files)
- hainet-files::file_list (list directory contents)
- hainet-files::file_metadata (get file information)

EXAMPLE VALID SELECTIONS:
- ["hainet-files::directory_create", "hainet-files::file_write"]
- ["sequential-thinking::sequentialthinking", "hainet-files::file_read"]
- ["context7::search-library", "hainet-files::file_write"]

INSTRUCTIONS:
1. Identify which tools you need from the AVAILABLE TOOLS list
2. Use the EXACT tool identifier format shown (server::tool)
3. Provide brief reasoning for your selection

RESPOND WITH VALID JSON ONLY (no markdown, no explanations):
{{
  "needed_tools": ["hainet-files::directory_create", "hainet-files::file_write"],
  "reasoning": "Need to create project directory structure and write initial files"
}}

CRITICAL: Respond with ONLY the JSON object. No markdown, no code blocks, no explanations.
"#,
            task.description,
            self.template.name,
            self.template.capabilities,
            self.session_tasks.to_prompt_format(),
            tool_list
        );
        
        let options = GenerationOptions {
            temperature: Some(0.3),
            max_tokens: Some(2048),
            system: Some(self.template.system_prompt.clone()),
            ..Default::default()
        };
        
        // Load user preference for Worker agent if available
        let preferred_family = if let Some(ref user_settings) = self.user_settings {
            let settings = user_settings.read().await;
            match settings.get_model_preference("worker").await {
                Ok(Some(family)) => {
                    tracing::debug!("Loaded user preference for Worker (tool selection): family='{}'", family);
                    Some(family)
                },
                Ok(None) => None,
                Err(e) => {
                    tracing::error!("Failed to load user preference for Worker: {:?}", e);
                    None
                }
            }
        } else {
            None
        };
        
        let selection_context = SelectionContext::for_worker();
        let selected_model = self
            .ai_provider_manager
            .select_model_for_agent_with_preferences(selection_context, preferred_family)
            .await
            .context("Failed to select model for tool selection")?;
        
        let client = selected_model.get_client()?;
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };
        
        let response = client
            .generate(model_name, &planning_prompt, options)
            .await
            .context("Failed to get tool selection from LLM")?;
        
        parse_tool_selection(&response.text)
            .context("Failed to parse tool selection response")
    }
    
    /// Load metadata for selected tools (Phase 2: Lazy Loading)
    async fn load_tool_metadata(&self, tool_names: &[String]) -> Result<HashMap<String, String>> {
        let mut metadata_map = HashMap::new();
        
        let mcp_client = self.mcp_client.read().await;
        
        for tool_identifier in tool_names {
            match mcp_client.get_tool_metadata(tool_identifier).await {
                Ok(metadata) => {
                    let formatted = format!(
                        "{}\n{}\n\nParameters:\n{}",
                        metadata.full_name(),
                        metadata.description,
                        metadata.parameter_docs
                    );
                    metadata_map.insert(tool_identifier.clone(), formatted);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to load metadata for {}: {}",
                        tool_identifier,
                        e
                    );
                }
            }
        }
        
        Ok(metadata_map)
    }
    
    /// Generate execution plan with loaded metadata (Phase 3: Focused Planning)
    async fn generate_execution_plan_discovery(
        &self,
        task: &crate::projects::Task,
        tool_metadata: &HashMap<String, String>,
    ) -> Result<DiscoveryExecutionPlan> {
        let formatted_metadata = format_tool_metadata(tool_metadata);
        
        let execution_prompt = format!(
            r#"You are executing a task step-by-step.

TASK: {}
YOUR ROLE: {}

SESSION PROGRESS:
{}

TOOLS YOU REQUESTED (with details):
{}

PREVIOUS STEP RESULTS:
No previous results yet

CRITICAL RULES FOR EXECUTION PLAN:
1. Tool identifiers MUST match the exact format from TOOLS YOU REQUESTED above
2. Use hainet-files::directory_create (NOT 'create-directory' or 'server::directory_create')
3. Use hainet-files::file_write (NOT 'file-write' or 'server::file_write')
4. All file paths are relative to the project sandbox
5. Create directories BEFORE writing files to them

COMMON PATTERNS:
- Create directory: {{"tool": "hainet-files::directory_create", "params": {{"path": "/src"}}}}
- Write file: {{"tool": "hainet-files::file_write", "params": {{"path": "/src/main.rs", "content": "..."}}}}
- Read file: {{"tool": "hainet-files::file_read", "params": {{"path": "/src/main.rs"}}}}
- List directory: {{"tool": "hainet-files::file_list", "params": {{"path": "/"}}}}

INSTRUCTIONS:
1. Create concrete, executable steps using the tools above
2. Each step uses ONE tool with specific parameters
3. Be precise with parameters (paths, values, etc.)
4. Steps can depend on previous step outputs
5. When creating a new project, create directories FIRST, then files

RESPOND WITH VALID JSON ONLY (no markdown, no explanations):
{{
  "steps": [
    {{
      "step_number": 1,
      "tool": "hainet-files::directory_create",
      "params": {{"path": "/src"}},
      "description": "Create source directory for the project",
      "depends_on": []
    }},
    {{
      "step_number": 2,
      "tool": "hainet-files::file_write",
      "params": {{"path": "/src/main.js", "content": "// Main entry point\nconsole.log('Hello World');"}},
      "description": "Create main JavaScript file",
      "depends_on": [1]
    }}
  ]
}}

CRITICAL: Respond with ONLY the JSON object. No markdown, no code blocks, no explanations.
"#,
            task.description,
            self.template.name,
            self.session_tasks.to_prompt_format(),
            formatted_metadata
        );
        
        let options = GenerationOptions {
            temperature: Some(0.1),
            max_tokens: Some(4096),
            system: Some(self.template.system_prompt.clone()),
            ..Default::default()
        };
        
        // Load user preference for Worker agent if available
        let preferred_family = if let Some(ref user_settings) = self.user_settings {
            let settings = user_settings.read().await;
            match settings.get_model_preference("worker").await {
                Ok(Some(family)) => {
                    tracing::debug!("Loaded user preference for Worker (execution planning): family='{}'", family);
                    Some(family)
                },
                Ok(None) => None,
                Err(e) => {
                    tracing::error!("Failed to load user preference for Worker: {:?}", e);
                    None
                }
            }
        } else {
            None
        };
        
        let selection_context = SelectionContext::for_worker();
        let selected_model = self
            .ai_provider_manager
            .select_model_for_agent_with_preferences(selection_context, preferred_family)
            .await
            .context("Failed to select model for execution planning")?;
        
        let client = selected_model.get_client()?;
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };
        
        // Add timeout wrapper to prevent indefinite hanging
        // Use execution_strategy timeout (default 120s for complex tasks)
        tracing::info!("[DIAGNOSTIC] Worker {} calling LLM for execution planning (model: {})", self.id.name, model_name);
        let llm_timeout = tokio::time::Duration::from_millis(
            (self.execution_strategy.base_timeout_ms * 2) as u64 // 2x base timeout for LLM calls
        );
        let response = tokio::time::timeout(
            llm_timeout,
            client.generate(model_name, &execution_prompt, options)
        )
        .await
        .context(format!("LLM generation timed out after {:?}", llm_timeout))?
        .context("Failed to generate execution plan")?;
        
        tracing::debug!(
            target: "llm_messages",
            "[WORKER DISCOVERY PLANNING RESPONSE] Model: {}, Response ({} chars):\n{}",
            model_name,
            response.text.len(),
            response.text
        );

        parse_execution_plan(&response.text)
            .context("Failed to parse execution plan")
    }
    
    /// Generate a new execution plan based on failure feedback
    async fn generate_replanning_discovery(
        &self,
        task: &crate::projects::Task,
        tool_metadata: &HashMap<String, String>,
        error: &str,
        failed_plan: &DiscoveryExecutionPlan,
    ) -> Result<DiscoveryExecutionPlan> {
        let formatted_metadata = format_tool_metadata(tool_metadata);
        let failed_steps_json = serde_json::to_string_pretty(&failed_plan.steps).unwrap_or_default();
        
        let replanning_prompt = format!(
            r#"You are a Worker AI agent. Your previous execution plan failed.
Analyze the error and create a NEW, CORRECTED plan.

TASK: {}
YOUR ROLE: {}

PREVIOUS PLAN (FAILED):
{}

ERROR ENCOUNTERED:
{}

AVAILABLE TOOLS:
{}

INSTRUCTIONS:
1. Analyze why the previous plan failed (e.g., missing directory, wrong parameters).
2. Create a new plan that fixes the issue.
3. If a directory was missing, add a step to create it first.
4. Respond with the complete new plan.

RESPOND WITH VALID JSON ONLY (no markdown):
{{
  "steps": [
    {{
      "step_number": 1,
      "tool": "server::tool_name",
      "params": {{"param1": "value1"}},
      "description": "Corrected step description",
      "depends_on": []
    }}
  ]
}}

CRITICAL: Respond with ONLY the JSON object. No markdown, no explanations.
"#,
            task.description,
            self.template.name,
            failed_steps_json,
            error,
            formatted_metadata
        );

        let options = GenerationOptions {
            temperature: Some(0.1),
            max_tokens: Some(4096),
            system: Some(self.template.system_prompt.clone()),
            ..Default::default()
        };
        
        // Load user preference for Worker agent if available
        let preferred_family = if let Some(ref user_settings) = self.user_settings {
            let settings = user_settings.read().await;
            match settings.get_model_preference("worker").await {
                Ok(Some(family)) => {
                    tracing::debug!("Loaded user preference for Worker (replanning): family='{}'", family);
                    Some(family)
                },
                Ok(None) => None,
                Err(e) => {
                    tracing::error!("Failed to load user preference for Worker: {:?}", e);
                    None
                }
            }
        } else {
            None
        };
        
        let selection_context = SelectionContext::for_worker();
        let selected_model = self
            .ai_provider_manager
            .select_model_for_agent_with_preferences(selection_context, preferred_family)
            .await
            .context("Failed to select model for replanning")?;
        
        let client = selected_model.get_client()?;
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };
        
        tracing::info!("[DIAGNOSTIC] Worker {} calling LLM for replanning (model: {})", self.id.name, model_name);
        
        let llm_timeout = tokio::time::Duration::from_millis(
            (self.execution_strategy.base_timeout_ms * 2) as u64
        );
        
        let response = tokio::time::timeout(
            llm_timeout,
            client.generate(model_name, &replanning_prompt, options)
        )
        .await
        .context(format!("LLM generation timed out after {:?}", llm_timeout))?
        .context("Failed to generate corrected execution plan")?;
        
        tracing::debug!(
            target: "llm_messages",
            "[WORKER REPLANNING RESPONSE] Model: {}, Response ({} chars):\n{}",
            model_name,
            response.text.len(),
            response.text
        );

        parse_execution_plan(&response.text)
            .context("Failed to parse corrected execution plan")
    }

    /// Execute discovery-based plan (Phase 4: Execution with Feedback)
    async fn execute_discovery_plan(
        &mut self,
        plan: &DiscoveryExecutionPlan,
        task: &crate::projects::Task,
    ) -> Result<Vec<String>> {
        let mut deliverables = Vec::new();
        
        for (idx, step) in plan.steps.iter().enumerate() {
            tracing::info!(
                "Worker {} executing discovery step {}/{}: {}",
                self.id.name,
                idx + 1,
                plan.steps.len(),
                step.description
            );
            
            let step_start = SystemTime::now();
            let mut retry_count = 0u32;
            
            let result = loop {
                retry_count += 1;
                
                match self.execute_discovery_step(step).await {
                    Ok(result) => {
                        // Record successful step
                        let duration_ms = step_start.elapsed()
                            .unwrap_or_default()
                            .as_millis() as u64;
                        
                        let outcome = TaskOutcome {
                            task_type: task.title.clone(),
                            tool_used: step.tool.clone(),
                            success: true,
                            duration_ms,
                            retry_count: retry_count.saturating_sub(1),
                            error_category: None,
                            timestamp: SystemTime::now(),
                        };
                        
                        self.learner.record_outcome(outcome.clone());
                        self.tool_selector.record_outcome(outcome);
                        
                        break result;
                    }
                    Err(error) => {
                        // Self-correction with adaptive retry
                        if self.self_correction_enabled {
                            let error_category = ErrorCategory::classify(&error.to_string());
                            
                            tracing::warn!(
                                "Worker {} discovery step failed (attempt {}): {:?} - {}",
                                self.id.name,
                                retry_count,
                                error_category,
                                error
                            );
                            
                            match error_category {
                                ErrorCategory::Transient => {
                                    if retry_count <= self.execution_strategy.max_retries {
                                        let delay_ms = self.execution_strategy.retry_delay_ms(retry_count);
                                        tracing::info!("Retrying in {}ms (transient error)", delay_ms);
                                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                                        continue;
                                    } else {
                                        self.record_step_failure(task, &convert_to_legacy_step(step), retry_count, error_category);
                                        return Err(anyhow::anyhow!(
                                            "Discovery step {} failed after {} retries: {}",
                                            idx + 1,
                                            self.execution_strategy.max_retries,
                                            error
                                        ));
                                    }
                                }
                                ErrorCategory::Permanent => {
                                    tracing::error!("Permanent error detected, requesting PM help");
                                    self.record_step_failure(task, &convert_to_legacy_step(step), retry_count, error_category);
                                    return Err(anyhow::anyhow!(
                                        "Permanent error (requesting PM help): {}",
                                        error
                                    ));
                                }
                                ErrorCategory::Unknown => {
                                    if retry_count == 1 {
                                        tracing::info!("Retrying unknown error once");
                                        let delay_ms = self.execution_strategy.retry_delay_ms(retry_count);
                                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                                        continue;
                                    } else {
                                        tracing::error!("Unknown error persists, requesting PM help");
                                        self.record_step_failure(task, &convert_to_legacy_step(step), retry_count, error_category);
                                        return Err(error);
                                    }
                                }
                            }
                        } else {
                            // Self-correction disabled
                            if retry_count <= self.execution_strategy.max_retries {
                                let delay_ms = 500 * retry_count as u64;
                                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                                continue;
                            } else {
                                return Err(error);
                            }
                        }
                    }
                }
            };
            
            deliverables.push(format!(
                "Step {}: {} - {}",
                idx + 1,
                step.description,
                result
            ));
        }
        
        Ok(deliverables)
    }
    
    /// Execute single discovery step
    async fn execute_discovery_step(&self, step: &DiscoveryExecutionStep) -> Result<String> {
        // Parse tool name: "server::tool_name"
        let parts: Vec<&str> = step.tool.split("::").collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid tool format: {}", step.tool));
        }
        
        let (server, tool) = (parts[0], parts[1]);
        
        // Add project_name to params for sandboxing (Session 52)
        let mut params = step.params.clone();
        if let Some(ref project_name) = self.current_project_name {
            if let Some(obj) = params.as_object_mut() {
                obj.insert("project_name".to_string(), serde_json::Value::String(project_name.clone()));
            }
        }
        
        let mcp_client = self.mcp_client.read().await;
        let result = mcp_client.call_tool(server, tool, params).await?;
        
        Ok(result.to_string())
    }
    /// Run worker loop processing tasks from channel
    pub async fn run(mut self, mut task_rx: tokio::sync::mpsc::Receiver<TaskId>) {
        tracing::info!("Worker {} started run loop", self.id.name);

        // Register with MessageBus and store the receiver to keep registration alive
        let (receiver, _) = self.message_bus.write().await
            .register_agent(self.id.clone())
            .await
            .context("Failed to register Worker agent with MessageBus")
            .expect("Worker failed to register with MessageBus"); // Panic if registration fails, as it's critical
        self._receiver = Some(receiver);
        
        while let Some(task_id) = task_rx.recv().await {
            tracing::info!("Worker {} received task {}", self.id.name, task_id);
            
            // Task is already marked as Assigned in DB by PM
            // But we still need to call assign_task to set up worker-side state
            if let Err(e) = self.assign_task(task_id.clone()).await {
                tracing::error!("Worker {} failed to assign task: {:?}", self.id.name, e);
                continue;
            }

            // Mark task as started (InProgress)
            {
                let project_manager = self.project_manager.write().await;
                if let Err(e) = project_manager.start_task(&task_id).await {
                    tracing::error!("Worker {} failed to start task: {:?}", self.id.name, e);
                    continue;
                }
            }
            
            // Execute task
            // We use execute_task_with_discovery() which includes the full lifecycle (Planning -> Working -> Reporting)
            // and uses dynamic tool discovery instead of rigid hardcoded prompts
            if let Err(e) = self.execute_task_with_discovery().await {
                 tracing::error!("Worker {} failed to execute task: {:?}", self.id.name, e);
                 // We continue to validation even on error, as the task might be marked as Failed
                 // and we need to clear the state or handle revision
            }
            
            // Wait for validation
            match self.await_validation().await {
                Ok(_) => tracing::info!("Worker {} task validated, ready for next", self.id.name),
                Err(e) => tracing::error!("Worker {} validation failed/timed out: {:?}", self.id.name, e),
            }
            
            // Ensure we are back in Idle state before next task
            if !matches!(self.state_machine.current_state(), AgentState::Idle) {
                tracing::warn!("Worker {} not in Idle state after validation, forcing transition", self.id.name);
                let _ = self.state_machine.transition(
                    AgentState::Idle, 
                    "Forcing Idle state for next task".to_string()
                );
                self.current_task = None;
            }
        }
        
        tracing::info!("Worker {} run loop ended", self.id.name);
    }
}

#[async_trait::async_trait]
impl super::Agent for WorkerAgent {
    fn id(&self) -> &AgentId {
        &self.id
    }

    async fn start(&mut self) -> Result<()> {
        // Placeholder start logic
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        // Placeholder stop logic
        Ok(())
    }

    async fn process_message(&mut self, _message: crate::messaging::Message) -> Result<()> {
        // Placeholder message processing logic
        Ok(())
    }
}


/// Execution plan generated by LLM
#[derive(Debug, Clone)]
struct ExecutionPlan {
    steps: Vec<ExecutionStep>,
}

/// Single execution step
#[derive(Debug, Clone)]
struct ExecutionStep {
    tool: String,  // Format: "server::tool_name"
    params: serde_json::Value,
    description: String,
}

/// Convert DiscoveryExecutionStep to ExecutionStep (for backward compatibility)
fn convert_to_legacy_step(discovery_step: &DiscoveryExecutionStep) -> ExecutionStep {
    ExecutionStep {
        tool: discovery_step.tool.clone(),
        params: discovery_step.params.clone(),
        description: discovery_step.description.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    async fn create_test_worker() -> WorkerAgent {
        let message_bus = Arc::new(RwLock::new(MessageBus::new().await.unwrap()));
        let prompt_manager = Arc::new(RwLock::new(PromptManager::new("prompts".into()).unwrap()));
        let project_manager = Arc::new(RwLock::new(
            ProjectManager::new("sqlite::memory:").await.unwrap()
        ));
        let mcp_client = Arc::new(RwLock::new(MCPClientManager::new()));
        let ai_provider_manager = Arc::new(AIProviderManager::new().await.unwrap());
        
        WorkerAgent::new(
            WorkerType::Files, 
            message_bus, 
            prompt_manager, 
            project_manager,
            mcp_client,
            ai_provider_manager
        )
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
        
        // Create a project and a task
        let task_id = {
            let pm_mgr = worker.project_manager.write().await;
            let project_id = pm_mgr.create_project(
                "Test Project".to_string(),
                "Test project for worker".to_string(),
                vec!["Task 1".to_string()],
            ).await.unwrap();
            let tasks = pm_mgr.get_project_tasks(&project_id).await.unwrap();
            tasks[0].id.clone()
        };
        
        // Assignment should succeed
        let result = worker.assign_task(task_id).await;
        assert!(result.is_ok());
        assert!(worker.current_task.is_some());
    }
}