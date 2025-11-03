//! # START OF FILE hainet-persona/src/agents/pm.rs
//! Project Manager Agent
//! 
//! Manages a single project, coordinating worker agents and ensuring task completion.
//! PM agents follow this state machine:
//! Startup → Planning → Managing → (Idle | Error)

use anyhow::{Result, Context};
use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;

use crate::messaging::{MessageBus, AgentId};
use crate::prompts::{PromptManager, AgentType, AgentState, PromptContext};
use crate::projects::{ProjectManager, ProjectId, TaskId};
use crate::ai_providers::providers::{OllamaClient, ProviderClient, GenerationOptions};
use crate::test_utils::JSONValidator;
use super::state::AgentStateMachine;
use super::templates::WorkerTemplate;
use super::llm_config::AgentLLMConfig;

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
    
    /// Ollama client for LLM-powered task decomposition
    ollama_client: OllamaClient,
    
    /// LLM configuration for this PM agent
    llm_config: AgentLLMConfig,
    
    /// Spawned worker agents (task_id -> worker_agent_id)
    workers: HashMap<TaskId, AgentId>,
    
    /// Task dependency graph
    task_graph: Option<TaskGraph>,
}

impl PMAgent {
    /// Create new PM agent for a project
    pub fn new(
        project_id: ProjectId,
        message_bus: Arc<RwLock<MessageBus>>,
        prompt_manager: Arc<RwLock<PromptManager>>,
        project_manager: Arc<RwLock<ProjectManager>>,
    ) -> Self {
        let id = AgentId::new(AgentType::PM, format!("PM-{}", project_id));
        
        Self {
            id,
            project_id,
            state_machine: AgentStateMachine::new(),
            message_bus,
            prompt_manager,
            project_manager,
            ollama_client: OllamaClient::localhost(),
            llm_config: AgentLLMConfig::for_pm(),
            workers: HashMap::new(),
            task_graph: None,
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
        
        // Use LLM to decompose tasks into detailed subtasks
        let detailed_plan = self.generate_detailed_plan(&project, &existing_tasks).await?;
        
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
        
        // Call LLM for validation decision
        let options = GenerationOptions {
            temperature: Some(0.3),
            max_tokens: Some(300),
            ..Default::default()
        };
        
        // Use gemma3:7b for fast validation (prefer gemma3 over llama3.2)
        let model = self.select_model_for_validation();
        
        let response = self.ollama_client.generate(
            &model,
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
        let project_manager = self.project_manager.write().await;
        project_manager.complete_project(&self.project_id).await?;
        
        self.state_machine.transition(
            AgentState::Idle,
            "Project completed successfully".to_string()
        )?;
        
        Ok(())
    }
    
    /// Generate detailed plan using LLM with enhanced prompting for gemma3
    async fn generate_detailed_plan(
        &self,
        project: &crate::projects::Project,
        existing_tasks: &[crate::projects::Task],
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
        
        // Enhanced planning prompt optimized for gemma3's structured reasoning
        let planning_prompt = format!(
            "You are a Project Manager breaking down a software project into executable tasks.\n\n\
             PROJECT DETAILS:\n\
             Title: {}\n\
             Overview: {}\n\n\
             HIGH-LEVEL TASKS (from Admin AI):\n{}\n\n\
             YOUR JOB:\n\
             Transform these high-level tasks into detailed, executable subtasks that Worker agents can complete.\n\n\
             WORKER TYPES AVAILABLE:\n\
             - FileWorker: Create/edit/delete files, manage directories\n\
             - CodeWorker: Write code, refactor, implement features\n\
             - NetworkWorker: API calls, web scraping, external data\n\
             - ResearchWorker: Documentation, analysis, planning\n\n\
             REQUIREMENTS:\n\
             1. Each subtask must be specific and actionable\n\
             2. Task titles: max 60 chars, clear and descriptive\n\
             3. Descriptions: detailed enough for Worker to execute without clarification\n\
             4. Dependencies: list task indices (0-based) that must complete first\n\
             5. Break complex tasks into 3-5 smaller steps\n\
             6. Logical execution order (setup → implementation → testing)\n\n\
             OUTPUT FORMAT (JSON only, no markdown):\n\
             {{\n\
               \"tasks\": [\n\
                 {{\n\
                   \"title\": \"Create project structure\",\n\
                   \"description\": \"Create index.html, style.css, script.js files in root directory\",\n\
                   \"worker_type\": \"FileWorker\"\n\
                 }},\n\
                 {{\n\
                   \"title\": \"Implement game logic\",\n\
                   \"description\": \"Write JavaScript code for snake movement, collision detection, and score tracking\",\n\
                   \"worker_type\": \"CodeWorker\"\n\
                 }}\n\
               ],\n\
               \"dependencies\": [\n\
                 {{\"task_index\": 1, \"depends_on\": [0]}}\n\
               ]\n\
             }}\n\n\
             Generate your task breakdown now (JSON only):",
            project.title,
            project.overview,
            existing_tasks.iter()
                .enumerate()
                .map(|(i, t)| format!("{}. {}", i + 1, t.description))
                .collect::<Vec<_>>()
                .join("\n")
        );
        
        let options = GenerationOptions {
            temperature: Some(0.7),
            max_tokens: Some(2048),
            system: Some(system_prompt),
            ..Default::default()
        };
        
        // Use gemma3:9b for complex task decomposition (PM's primary intelligence task)
        let model = self.select_model_for_planning();
        
        let response = self.ollama_client.generate(
            &model,
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
                val
            },
            None => {
                tracing::error!("All JSON parsing strategies failed: {}", 
                               parse_result.error.unwrap_or_else(|| "Unknown error".to_string()));
                return Err(anyhow::anyhow!("Failed to parse detailed plan as valid JSON"));
            }
        };
        
        let tasks: Vec<TaskDetail> = parsed["tasks"].as_array()
            .ok_or_else(|| anyhow::anyhow!("Missing 'tasks' array"))?
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
        
        // Create worker agent (simplified - in production this would be a full WorkerAgent)
        let worker_id = AgentId::new(
            AgentType::Worker,
            format!("{}-{}", template.name, task_id)
        );
        
        // Store worker mapping
        self.workers.insert(task_id.clone(), worker_id.clone());
        
        // Assign task to worker
        let pm = self.project_manager.write().await;
        pm.assign_task(task_id, worker_id).await?;
        
        // TODO: Actually spawn WorkerAgent and start execution
        // This will be completed in Session 5
        
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
    
    /// Select model for task planning (complex decomposition)
    /// Prefers gemma3:9b for better structured reasoning
    fn select_model_for_planning(&self) -> String {
        // Prefer gemma3 based on model size from config
        match self.llm_config.model_size_preference {
            super::llm_config::ModelSize::SevenB | super::llm_config::ModelSize::FourteenBPlus => {
                "gemma3:9b".to_string()
            },
            _ => {
                // Fall back to 7b for smaller configurations
                "gemma3:7b".to_string()
            }
        }
    }
    
    /// Select model for task validation (faster checks)
    /// Uses gemma3:7b for speed while maintaining quality
    fn select_model_for_validation(&self) -> String {
        "gemma3:7b".to_string()
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
        
        let project_id = ProjectId::new();
        PMAgent::new(project_id, message_bus, prompt_manager, project_manager)
    }
    
    #[tokio::test]
    async fn test_pm_creation() {
        let pm = create_test_pm().await;
        assert_eq!(pm.state(), &AgentState::Startup);
    }
    
    #[tokio::test]
    async fn test_pm_startup_transition() {
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

        let mut pm = PMAgent::new(project_id, message_bus, prompt_manager, project_manager);
        
        pm.start().await.unwrap();
        assert_eq!(pm.state(), &AgentState::Managing);
    }
}
