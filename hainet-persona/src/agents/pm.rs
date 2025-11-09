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

use crate::messaging::{MessageBus, AgentId};
use crate::prompts::{PromptManager, AgentType, AgentState, PromptContext};
use crate::projects::{ProjectManager, ProjectId, TaskId};
use crate::ai_providers::providers::{GenerationOptions};
use crate::ai_providers::{AIProviderManager, SelectionContext};
use crate::test_utils::JSONValidator;
use super::state::AgentStateMachine;
use super::templates::WorkerTemplate;
use super::llm_config::AgentLLMConfig;
use super::pm_intelligence::{
    HistoricalLearner, ProjectComplexity, DecompositionStrategy, 
    ProjectOutcome
};

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
    
    /// LLM configuration for this PM agent
    llm_config: AgentLLMConfig,
    
    /// Spawned worker agents (task_id -> worker_agent_id)
    workers: HashMap<TaskId, AgentId>,
    
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
}

impl PMAgent {
    /// Create new PM agent for a project
    pub fn new(
        project_id: ProjectId,
        message_bus: Arc<RwLock<MessageBus>>,
        prompt_manager: Arc<RwLock<PromptManager>>,
        project_manager: Arc<RwLock<ProjectManager>>,
        ai_provider_manager: Arc<AIProviderManager>,
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
            llm_config: AgentLLMConfig::for_pm(),
            workers: HashMap::new(),
            task_graph: None,
            learner: HistoricalLearner::new(),
            project_complexity: None,
            selected_strategy: None,
            project_start_time: None,
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
    /// Startup → Idle → Planning → Managing
    pub async fn initialize_and_plan(&mut self) -> Result<()> {
        // Record project start time
        self.project_start_time = Some(SystemTime::now());
        
        // Transition from Startup to Idle (PM agents must go through Idle first)
        self.state_machine.transition(
            AgentState::Idle,
            "PM initialized, ready to plan".to_string()
        )?;
        
        // Then transition to Planning
        self.state_machine.transition(
            AgentState::Planning,
            "PM starting project analysis".to_string()
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
        
        // Create detailed tasks in database
        for task_detail in &detailed_plan.tasks {
            let project_manager = self.project_manager.write().await;
            project_manager.create_task(
                &self.project_id,
                task_detail.title.clone(),
                task_detail.description.clone(),
            ).await?;
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
            
            // Get executable tasks (unassigned + dependencies met)
            let executable_tasks = self.get_executable_tasks().await?;
            
            // Spawn workers and assign tasks
            for task_id in executable_tasks {
                self.spawn_worker_for_task(&task_id).await?;
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
            Ok(unassigned.into_iter()
                .filter(|task_id| graph.can_execute(task_id, &completed))
                .collect())
        } else {
            // No graph, return all unassigned
            Ok(unassigned)
        }
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
        let task = {
            let pm = self.project_manager.read().await;
            pm.get_task(task_id).await?
        };
        
        // Generate validation prompt
        let prompt = self.generate_validation_prompt(&task)?;
        
        // Select the best model for validation using AIProviderManager
        let selection_context = SelectionContext::for_pm();
        let selected_model = self.ai_provider_manager
            .select_model_for_agent(selection_context)
            .await
            .context("Failed to select a model for validation")?;
        
        // Call LLM for validation decision
        let options = GenerationOptions {
            temperature: Some(0.3),
            max_tokens: Some(300),
            ..Default::default()
        };
        
        let client = selected_model.get_client()?;
        // Strip provider prefix from model_id (e.g., "Ollama::model" -> "model")
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };
        let response = client.generate(
            model_name,
            &prompt,
            options
        ).await.context("Failed to validate task with LLM")?;
        
        // Parse validation decision
        let validation = self.parse_validation_response(&response.text)?;
        
        let pm = self.project_manager.read().await;
        
        if validation.approved {
            // Approve task
            pm.approve_task(task_id, validation.feedback).await?;
            
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
        
        let project_manager = self.project_manager.write().await;
        project_manager.complete_project(&self.project_id).await?;
        
        self.state_machine.transition(
            AgentState::Idle,
            "Project completed successfully".to_string()
        )?;
        
        Ok(())
    }
    
    /// Generate detailed plan using LLM with strategy-aware prompting (optimized for local LLMs)
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
        let selection_context = SelectionContext::for_pm();
        let selected_model = self.ai_provider_manager
            .select_model_for_agent(selection_context)
            .await
            .context("Failed to select a model for planning")?;
        
        let options = GenerationOptions {
            temperature: Some(0.7),
            max_tokens: Some(2048),
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
        let response = client.generate(
            model_name,
            &planning_prompt,
            options
        ).await.context("Failed to generate detailed plan with LLM")?;
        
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
                    worker_type: t["worker_type"].as_str().unwrap_or("FileWorker").to_string(),
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
                    .collect();
                Some(TaskDependency { task_index, depends_on })
            })
            .collect();
        
        tracing::info!("PM parsed detailed plan: {} tasks, {} dependencies (strategy: {})", 
                      tasks.len(), dependencies.len(), parse_result.strategy_used);
        
        Ok(DetailedPlan { tasks, dependencies })
    }
    
    /// Spawn worker for a specific task
    async fn spawn_worker_for_task(&mut self, task_id: &TaskId) -> Result<()> {
        // Get task details
        let task = {
            let pm = self.project_manager.read().await;
            let tasks = pm.get_project_tasks(&self.project_id).await?;
            tasks.into_iter()
                .find(|t| &t.id == task_id)
                .ok_or_else(|| anyhow::anyhow!("Task not found"))?
        };
        
        // Select appropriate worker template based on task description
        let template = WorkerTemplate::select_for_task(&task.description);
        
        tracing::info!("Spawning {} for task: {}", template.name, task.title);
        
        // Create worker agent with proper template
        // Workers expect Arc<PromptManager>, so we need to create a new instance
        // Since PromptManager can't be cloned, we'll use the new() constructor instead
        use std::path::PathBuf;
        
        // Find workspace root by looking for hainet-persona/prompts
        // This works whether we're running from hainet-portal or hainet-persona
        let workspace_root = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        let prompts_path = workspace_root.join("hainet-persona").join("prompts");
        
        let prompt_manager_for_worker = Arc::new(crate::prompts::PromptManager::new(prompts_path)?);
        
        let mut worker = super::worker::WorkerAgent::from_template(
            template,
            self.message_bus.clone(),
            prompt_manager_for_worker,
            self.project_manager.clone(),
            Arc::new(RwLock::new(crate::tools::mcp::MCPClientManager::new())),
            self.ai_provider_manager.clone(),
        );
        
        let worker_id = worker.id().clone();
        
        // Store worker mapping
        self.workers.insert(task_id.clone(), worker_id.clone());
        
        // Transition worker to Idle state
        worker.state_machine_mut().transition(
            AgentState::Idle,
            "Worker initialized and ready for task".to_string()
        )?;
        
        // Assign task to worker
        worker.assign_task(task_id.clone()).await?;
        
        // Execute worker in background
        let task_id_for_logging = task_id.clone();
        tokio::spawn(async move {
            tracing::info!("Worker {} starting execution for task {}", worker.id().name, task_id_for_logging);
            
            // Execute task
            if let Err(e) = worker.execute_task().await {
                tracing::error!("Worker {} task execution failed: {:?}", worker.id().name, e);
                return;
            }
            
            tracing::info!("Worker {} completed task execution, awaiting PM validation", worker.id().name);
        });
        
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
        // Add any cleanup logic here
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
    worker_type: String,
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
    revision_needed: bool,
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
        
        let project_id = ProjectId::new();
        PMAgent::new(project_id, message_bus, prompt_manager, project_manager, ai_provider_manager)
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
        let mut pm = PMAgent::new(project_id, message_bus, prompt_manager, project_manager, ai_provider_manager);
        
        pm.start().await.unwrap();
        assert_eq!(pm.state(), &AgentState::Managing);
    }
}
