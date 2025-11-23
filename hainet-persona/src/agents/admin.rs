//! # START OF FILE hainet-persona/src/agents/admin.rs
//! # Admin AI Agent
//! 
//! Primary user-facing agent that orchestrates all task execution.
//! Implements the hierarchical agent architecture's top layer.
//! 
//! ## State Machine
//! Startup → Conversation → Planning → Monitoring → Conversation
//! 
//! ## Responsibilities
//! - Detect user intents (simple vs complex)
//! - Generate project plans using LLM
//! - Spawn PM agents dynamically
//! - Monitor multiple parallel projects
//! - Remain available for user conversation

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, trace};

use super::{Agent, AgentContext, IntentParser, TaskPlanner, AgentStateMachine};
use super::pm::PMAgent;
use super::llm_config::AgentLLMConfig;
use super::metrics::{MetricsCollector, OperationResult};
use super::session_tasks::SessionTaskList;
use crate::config::HaiNetConfig;
use crate::messaging::{AgentId, Message};
use crate::prompts::{AgentType, AgentState, PromptContext};
use crate::projects::{ProjectManager, ProjectId};
use crate::ai_providers::{AIProviderManager, SelectionContext};
use crate::ai_providers::providers::{GenerationOptions};
use crate::test_utils::JSONValidator;
use std::time::Instant;

pub mod memory;
pub mod profile;

use self::memory::ConversationStore;
use self::profile::UserProfile;

/// Threshold for detecting complex intents that require projects
const COMPLEX_INTENT_THRESHOLD: f64 = 0.7;

/// Maximum number of retries for LLM format validation failures
const MAX_LLM_RETRIES: usize = 3;

/// Admin AI - Primary user interface and orchestrator
pub struct AdminAgent {
    /// Agent identifier
    id: AgentId,
    
    /// Shared context with other agents
    context: Arc<AgentContext>,
    
    /// Intent parser for understanding user requests
    intent_parser: IntentParser,
    
    /// Task planner for breaking down requests
    task_planner: TaskPlanner,
    
    /// State machine managing agent lifecycle
    state_machine: AgentStateMachine,
    
    /// Project manager for creating and tracking projects
    project_manager: Arc<RwLock<ProjectManager>>,
    
    /// AI provider manager for dynamic model selection
    pub ai_provider_manager: Arc<AIProviderManager>,
    
    /// Active projects being monitored
    active_projects: HashMap<ProjectId, AgentId>,
    
    /// Running flag
    running: bool,
    
    /// Configuration
    config: HaiNetConfig,
    
    /// LLM configuration for Admin agent
    llm_config: AgentLLMConfig,
    
    /// Metrics collector
    metrics: Arc<RwLock<MetricsCollector>>,
    
    /// Session task list for LLM context (short-term memory)
    /// Session task list for LLM context (short-term memory)
    session_tasks: SessionTaskList,
    
    /// Message receiver for processing system events
    receiver: Option<tokio::sync::mpsc::Receiver<Message>>,
    
    /// Persistent conversation memory
    memory: Arc<ConversationStore>,
    
    /// User profile and preferences
    profile: Arc<UserProfile>,
}

impl AdminAgent {
    /// Create new Admin AI agent
    pub async fn new(
        context: Arc<AgentContext>,
        project_manager: Arc<RwLock<ProjectManager>>,
        ai_provider_manager: Arc<AIProviderManager>,
        metrics: Arc<RwLock<MetricsCollector>>,
        memory_db_url: String,
        profile_db_url: String,
    ) -> Result<Self> {
        let id = AgentId::new(AgentType::Admin, "main-admin".to_string());
        let config = HaiNetConfig::load_or_default();
        
        // Get Admin-specific LLM configuration
        let llm_config = config.get_agent_llm_config(AgentType::Admin);
        
        tracing::info!("Admin AI LLM config: temp={}, max_tokens={}, provider_pref={:?}", 
                      llm_config.temperature, llm_config.max_tokens, llm_config.provider_preference);
        
        // Register with MessageBus
        let (_receiver, _) = context.message_bus.write().await
            .register_agent(id.clone())
            .await
            .context("Failed to register Admin agent with MessageBus")?;

        // Initialize memory and profile
        let memory = Arc::new(ConversationStore::new(&memory_db_url).await?);
        let profile = Arc::new(UserProfile::new(&profile_db_url).await?);

        Ok(Self {
            id: id.clone(),
            context,
            intent_parser: IntentParser::new(),
            task_planner: TaskPlanner::new(),
            state_machine: AgentStateMachine::new(),
            project_manager,
            ai_provider_manager,
            active_projects: HashMap::new(),
            running: false,
            config,
            llm_config,
            metrics,
            session_tasks: SessionTaskList::new(),
            receiver: Some(_receiver),
            memory,
            profile,
        })
    }

    /// Get the agent context
    pub fn context(&self) -> &Arc<AgentContext> {
        &self.context
    }

    /// Get the project manager
    pub fn project_manager(&self) -> &Arc<RwLock<ProjectManager>> {
        &self.project_manager
    }

    /// Helper to update status
    async fn update_status(&self, activity: &str) {
        self.context.message_bus.write().await.update_agent_status(
            self.id.clone(),
            format!("{:?}", self.state_machine.current_state()),
            activity.to_string()
        ).await;
    }

    // ========== Project Management Commands ==========

    /// Detect if the user input is a project management command
    fn is_project_management_command(&self, user_input: &str) -> Option<String> {
        let input_lower = user_input.to_lowercase();
        
        // Check for implicit project references ("it", "PM", "development", "work")
        let has_implicit_project_ref = input_lower.contains(" it ") 
            || input_lower.contains(" pm ") 
            || input_lower.contains("development") 
            || input_lower.contains("working on");
        
        let has_explicit_project_ref = input_lower.contains("project");
        let has_project_ref = has_explicit_project_ref || has_implicit_project_ref;
        
        // Delete keywords
        if (input_lower.contains("delete") || input_lower.contains("remove") || input_lower.contains("cancel")) 
            && (has_project_ref || input_lower.contains("all")) {
            return Some("delete".to_string());
        }
        
        // Pause keywords
        if (input_lower.contains("pause") || input_lower.contains("hold") || input_lower.contains("stop working on")) 
            && has_project_ref {
            return Some("pause".to_string());
        }
        
        // Resume keywords - enhanced to catch "get the PM to keep working", "continue development", etc.
        if ((input_lower.contains("resume") || input_lower.contains("continue") || input_lower.contains("restart"))
            || (input_lower.contains("keep") && input_lower.contains("working"))
            || (input_lower.contains("get") && input_lower.contains("pm") && (input_lower.contains("working") || input_lower.contains("going"))))
            && has_project_ref {
            return Some("resume".to_string());
        }
        
        // Status/Progress keywords - "how is the project going", "what's the status", etc.
        if ((input_lower.contains("how") && input_lower.contains("going"))
            || (input_lower.contains("what") && (input_lower.contains("status") || input_lower.contains("progress")))
            || input_lower.contains("check on"))
            && has_project_ref {
            return Some("status".to_string());
        }
        
        // Stop keywords
        if (input_lower.contains("stop") || input_lower.contains("terminate") || input_lower.contains("end")) 
            && has_project_ref 
            && !input_lower.contains("working on") {
            return Some("stop".to_string());
        }
        
        // Rename keywords
        if (input_lower.contains("rename") || input_lower.contains("change name")) 
            && has_project_ref {
            return Some("rename".to_string());
        }

        // Export keywords
        if (input_lower.contains("export") || input_lower.contains("backup") || input_lower.contains("archive")) 
            && has_project_ref {
            return Some("export".to_string());
        }

        // Import keywords
        if (input_lower.contains("import") || input_lower.contains("restore") || input_lower.contains("load")) 
            && has_project_ref {
            return Some("import".to_string());
        }
        
        // List keywords
        if (input_lower.contains("list") || input_lower.contains("show") || input_lower.contains("what")) 
            && has_project_ref {
            return Some("list".to_string());
        }
        
        None
    }

    /// Handle project management commands
    async fn handle_project_management_command(&mut self, user_input: &str, command_type: &str) -> Result<String> {
        tracing::info!("Handling project management command: {}", command_type);
        
        match command_type {
            "delete" => {
                if user_input.to_lowercase().contains("all") {
                    self.delete_all_projects().await
                } else {
                    // Try to extract project ID or title from input
                    // For now, just inform user they need to specify
                    Ok("To delete a specific project, please use the UI menu next to the project in the Active Projects list. To delete all projects, say 'delete all projects'.".to_string())
                }
            },
            "pause" => {
                self.pause_most_recent_project().await
            },
            "resume" => {
                self.resume_most_recent_project().await
            },
            "status" => {
                self.get_most_recent_project_status().await
            },
            "stop" => {
                self.pause_most_recent_project().await // Stop is same as pause for now
            },
            "rename" => {
                Ok("To rename a project, please use the UI menu next to the project in the Active Projects list.".to_string())
            },
            "export" => {
                Ok("To export a project, please use the 'Export' option in the project menu in the Active Projects list.".to_string())
            },
            "import" => {
                Ok("To import a project, please use the 'Import' button in the Active Projects header.".to_string())
            },
            "list" => {
                self.list_active_projects_to_user().await
            },
            _ => {
                Ok("I didn't understand that project management command. You can ask me to list, delete, pause, resume, or stop projects.".to_string())
            }
        }
    }

    /// Delete all active projects
    async fn delete_all_projects(&self) -> Result<String> {
        let count = self.project_manager.read().await
            .delete_all_active_projects()
            .await?;
        
        Ok(format!("✅ Successfully deleted {} project(s). All associated PM and worker agents will be cleaned up.", count))
    }

    /// List all active projects for the user
    async fn list_active_projects_to_user(&self) -> Result<String> {
        let projects = self.project_manager.read().await
            .list_active_projects()
            .await?;
        
        if projects.is_empty() {
            return Ok("You don't have any active projects at the moment. Say something like 'build me a todo app' to create one!".to_string());
        }
        
        let mut response = format!("📋 **Active Projects ({}):**\n\n", projects.len());
        for (idx, project) in projects.iter().enumerate() {
            response.push_str(&format!(
                "{}. **{}** ({})\n   {}\n   {} tasks\n\n",
                idx + 1,
                project.title,
                project.status,
                project.overview.chars().take(100).collect::<String>(),
                project.task_ids.len()
            ));
        }
        
        response.push_str("💡 You can manage these projects using the menu in the Active Projects sidebar.");
        
        Ok(response)
    }
    
    /// Pause the most recent active project
    async fn pause_most_recent_project(&self) -> Result<String> {
        let projects = self.project_manager.read().await
            .list_active_projects()
            .await?;
        
        if projects.is_empty() {
            return Ok("You don't have any active projects to pause.".to_string());
        }
        
        // Get the most recent project (last in the list)
        let project = projects.last().unwrap();
        let project_id = project.id.clone();
        let project_title = project.title.clone();
        
        // Pause the project
        self.project_manager.read().await
            .pause_project(&project_id)
            .await?;
        
        Ok(format!("✅ Paused project: **{}**\n\nThe PM and workers will stop working on this project. You can resume it anytime by asking me to continue or resume the project.", project_title))
    }
    
    /// Resume the most recent paused project
    async fn resume_most_recent_project(&self) -> Result<String> {
        let projects = self.project_manager.read().await
            .list_active_projects()
            .await?;
        
        if projects.is_empty() {
            return Ok("You don't have any projects to resume. Would you like to create a new project?".to_string());
        }
        
        // Get the most recent project (last in the list)
        let project = projects.last().unwrap();
        let project_id = project.id.clone();
        let project_title = project.title.clone();
        
        // Resume the project
        self.project_manager.read().await
            .resume_project(&project_id)
            .await?;
        
        Ok(format!("✅ Resumed project: **{}**\n\nThe PM will continue managing this project and coordinating with workers to complete the remaining tasks.", project_title))
    }
    
    /// Get status of the most recent project
    async fn get_most_recent_project_status(&self) -> Result<String> {
        let projects = self.project_manager.read().await
            .list_active_projects()
            .await?;
        
        if projects.is_empty() {
            return Ok("You don't have any active projects at the moment.".to_string());
        }
        
        // Get the most recent project (last in the list)
        let project = projects.last().unwrap();
        
        // Get detailed task status
        let pm = self.project_manager.read().await;
        let tasks = pm.get_project_tasks(&project.id).await?;
        
        let total_tasks = tasks.len();
        let completed_tasks = tasks.iter().filter(|t| matches!(t.status, crate::projects::TaskStatus::Complete)).count();
        let in_progress_tasks = tasks.iter().filter(|t| matches!(t.status, crate::projects::TaskStatus::Assigned | crate::projects::TaskStatus::InProgress)).count();
        let unassigned_tasks = tasks.iter().filter(|t| matches!(t.status, crate::projects::TaskStatus::Unassigned)).count();
        let needs_revision_tasks = tasks.iter().filter(|t| matches!(t.status, crate::projects::TaskStatus::NeedsRevision)).count();
        
        let mut response = format!(
            "📊 **Project Status: {}**\n\n**Overview:** {}\n\n**Progress:**\n- Total Tasks: {}\n- ✅ Completed: {}\n- 🔄 In Progress: {}\n- ⏳ Unassigned: {}\n",
            project.title,
            project.overview.chars().take(150).collect::<String>(),
            total_tasks,
            completed_tasks,
            in_progress_tasks,
            unassigned_tasks
        );
        
        if needs_revision_tasks > 0 {
            response.push_str(&format!("- ⚠️ Needs Revision: {}\n", needs_revision_tasks));
        }
        
        response.push_str(&format!("\n**Overall Status:** {}", project.status));
        
        Ok(response)
    }
    

    /// Main entry point for user interaction
    pub async fn process_user_input(&mut self, user_input: String) -> Result<String> {
        tracing::info!("DEBUG: Admin process_user_input called with: {}", user_input);
        
        // 1. Check for project management commands FIRST
        if let Some(command_type) = self.is_project_management_command(&user_input) {
            tracing::info!("DEBUG: Detected project management command: {:?}", command_type);
            return self.handle_project_management_command(&user_input, &command_type).await;
        }

        // 2. Parse Intent
        tracing::info!("DEBUG: Parsing intent...");
        self.update_status("Parsing user intent").await;
        let intent = self.intent_parser.parse(&user_input).await?;
        tracing::info!("Parsed intent: {:?} (confidence: {})", intent.intent_type, intent.confidence);

        // 3. Handle initial startup state
        // If still in Startup state, transition to Conversation first
        if *self.state_machine.current_state() == AgentState::Startup {
            tracing::warn!("Admin AI still in Startup state, transitioning to Conversation");
            self.state_machine.transition(
                AgentState::Conversation,
                "Auto-transition from Startup on first message".to_string()
            )?;
            self.update_status("Transitioned to Conversation").await;
        }
        
        // 4. Add user request to session tasks
        let request_title = if user_input.len() > 50 {
            format!("{}...", &user_input[..47])
        } else {
            user_input.clone()
        };
        self.session_tasks.add_task(request_title.clone(), None);
        let _ = self.session_tasks.start_task(&request_title);

        // 5. Route to appropriate handler based on intent complexity
        let is_complex = self.is_complex_intent(&intent, &user_input)?;
        tracing::info!("DEBUG: Intent is complex: {}", is_complex);
        
        let result = if is_complex {
            tracing::info!("DEBUG: Calling handle_complex_intent");
            self.handle_complex_intent(&user_input, &intent).await
        } else {
            tracing::info!("DEBUG: Calling handle_simple_intent");
            self.handle_simple_intent(&user_input, &intent).await
        };
        
        // Save to memory if successful
        if let Ok(ref response) = result {
            let entry = crate::agents::admin::memory::ConversationEntry {
                id: uuid::Uuid::new_v4().to_string(),
                user_message: user_input.clone(),
                admin_response: response.clone(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
                context_snapshot: None,
                sentiment: None,
                intent: Some(format!("{:?}", intent.intent_type)),
            };
            
            if let Err(e) = self.memory.add_entry(entry).await {
                tracing::warn!("Failed to save conversation entry: {:?}", e);
            }
        }
        
        // 6. Update session task status based on result
        match &result {
            Ok(_) => {
                let _ = self.session_tasks.complete_task(&request_title);
            },
            Err(_) => {
                let _ = self.session_tasks.fail_task(&request_title);
            }
        }
        
        result
    }

    /// Handles complex intents by creating and managing a project.
    async fn handle_complex_intent(&mut self, user_input: &str, intent: &super::intent::Intent) -> Result<String> {
        let start_time = Instant::now();

        // 1. Transition to Planning state
        self.transition_to_planning(intent)?;
        
        // Add planning task to session
        self.session_tasks.add_task("Generate project plan".to_string(), None);
        let _ = self.session_tasks.start_task("Generate project plan");

        // 2. Generate project plan using LLM
        let project_plan = self.generate_project_plan(user_input, intent).await?;
        
        // Mark planning complete
        let _ = self.session_tasks.complete_task("Generate project plan");

        // 3. Create project in the database
        let project_title_short = format!("Create project: {}", &project_plan.title[..project_plan.title.len().min(30)]);
        self.session_tasks.add_task(project_title_short.clone(), None);
        let _ = self.session_tasks.start_task(&project_title_short);
        
        let project_id = self.create_project(
            project_plan.title.clone(),
            project_plan.overview.clone(),
            project_plan.initial_tasks.clone(),
        ).await?;

        // 4. Spawn a PM agent to manage the project
        self.session_tasks.add_task("Spawn PM agent".to_string(), None);
        let _ = self.session_tasks.start_task("Spawn PM agent");
        
        let pm_id = self.spawn_pm_agent(&project_id, &project_plan).await?;
        
        let _ = self.session_tasks.complete_task("Spawn PM agent");

        // 5. Track the new project
        self.active_projects.insert(project_id.clone(), pm_id.clone());
        
        // Mark project creation complete
        let _ = self.session_tasks.complete_task(&project_title_short);

        // 6. Transition to Monitoring state
        self.state_machine.transition(
            AgentState::Monitoring,
            format!("Project {} created, PM agent {} spawned", project_id, pm_id.name)
        )?;

        // 7. Record metrics for successful planning
        let response_time = start_time.elapsed();
        let metrics = self.metrics.read().await;
        let input_tokens = (user_input.len() / 4) as u32;
        let output_tokens = ((project_plan.title.len() + project_plan.overview.len()) / 4) as u32;

        metrics.record_operation(OperationResult {
            agent_type: AgentType::Admin,
            agent_id: self.id.clone(),
            config_hash: super::metrics::hash_config(&self.llm_config),
            operation_type: "complex_intent_planning".to_string(),
            success: true,
            response_time,
            tokens_used: Some(input_tokens + output_tokens),
            ..Default::default()
        }).await?;

        // 8. Formulate and return user-facing response
        Ok(format!(
            "I've created a project to handle your request:\n\n**{}**\n\n{}\n\nI'll work on this in the background and keep you updated on progress. Feel free to ask me anything else in the meantime!",
            project_plan.title,
            project_plan.overview
        ))
    }

    /// Transitions the agent to the Planning state, handling any necessary intermediate steps.
    fn transition_to_planning(&mut self, intent: &super::intent::Intent) -> Result<()> {
        let current_state = self.state_machine.current_state().clone();

        // Allow Planning transition from Conversation, Idle, or Monitoring states
        if current_state == AgentState::Conversation
            || current_state == AgentState::Idle
            || current_state == AgentState::Monitoring {
            self.state_machine.transition(
                AgentState::Planning,
                format!("Complex intent detected: {:?}", intent.intent_type)
            )?;
        } else {
            // Force transition to Conversation first, then to Planning
            tracing::warn!("Admin in unexpected state {:?}, transitioning to Conversation first", current_state);
            self.state_machine.transition(
                AgentState::Conversation,
                "Resetting to Conversation state".to_string()
            )?;
            self.state_machine.transition(
                AgentState::Planning,
                format!("Complex intent detected: {:?}", intent.intent_type)
            )?;
        }
        Ok(())
    }
    
    /// Detect if intent requires a project (complex/multi-step)
    fn is_complex_intent(&self, intent: &super::intent::Intent, user_input: &str) -> Result<bool> {
        // Project keywords (strong indicators of project creation)
        let project_keywords = [
            "build", "create", "develop", "make", "implement",
            "design", "write", "generate", "setup", "configure",
            "install", "deploy", "construct", "architect", "code",
            "program", "add", "fix"
        ];
        
        let input_lower = user_input.to_lowercase();
        let has_project_keyword = project_keywords.iter()
            .any(|kw| input_lower.contains(kw));
        
        // Domain-specific keywords (games, apps, websites, tools)
        // These almost always indicate project creation
        let domain_keywords = [
            "game", "app", "application", "website", "site",
            "tool", "script", "system", "service", "api",
            "bot", "plugin", "extension", "component"
        ];
        
        let has_domain_keyword = domain_keywords.iter()
            .any(|kw| input_lower.contains(kw));
        
        // Multi-step indicators
        let multi_step_indicators = [
            "and", "then", "also", "plus", "with",
            "step", "phase", "stage", "first", "next"
        ];
        
        let has_multi_step = multi_step_indicators.iter()
            .any(|ind| input_lower.contains(ind));
        
        // Complex intent decision tree:
        // 1. If has project keyword + domain keyword → ALWAYS complex (e.g., "build a game")
        // 2. If has project keyword → complex (regardless of confidence)
        // 3. If has domain keyword → complex (even without action verb, implies creation)
        // 4. If Task intent with high confidence + multi-step → complex
        Ok(
            (has_project_keyword && has_domain_keyword) ||
            has_project_keyword ||
            has_domain_keyword ||
            (intent.intent_type == super::intent::IntentType::Task && 
             intent.confidence >= COMPLEX_INTENT_THRESHOLD && 
             has_multi_step)
        )
    }
    
    /// Generate project plan using LLM with retry logic and format validation
    async fn generate_project_plan(
        &self,
        user_input: &str,
        intent: &super::intent::Intent,
    ) -> Result<ProjectPlan> {
        // Try up to MAX_LLM_RETRIES times
        for attempt in 1..=MAX_LLM_RETRIES {
            tracing::info!("Generating project plan (attempt {}/{})", attempt, MAX_LLM_RETRIES);
            
            // Generate plan using progressively simpler prompts
            match self.generate_plan_attempt(user_input, intent, attempt).await {
                Ok(plan) => {
                    // Validate the plan structure
                    if self.validate_project_plan(&plan).is_ok() {
                        tracing::info!("Successfully generated valid project plan on attempt {}", attempt);
                        return Ok(plan);
                    } else {
                        tracing::warn!("Plan validation failed on attempt {}, retrying...", attempt);
                        if attempt == MAX_LLM_RETRIES {
                            return Err(anyhow::anyhow!("Failed to generate valid project plan after {} attempts", MAX_LLM_RETRIES));
                        }
                        continue;
                    }
                },
                Err(e) => {
                    tracing::warn!("Plan generation failed on attempt {}: {:?}", attempt, e);
                    if attempt == MAX_LLM_RETRIES {
                        return Err(e).context(format!("Failed to generate project plan after {} attempts", MAX_LLM_RETRIES));
                    }
                    // Small delay before retry
                    tokio::time::sleep(tokio::time::Duration::from_millis(500 * attempt as u64)).await;
                    continue;
                }
            }
        }
        
        Err(anyhow::anyhow!("Failed to generate project plan after {} attempts", MAX_LLM_RETRIES))
    }
    
    /// Generate a single plan attempt with attempt-specific prompt
    async fn generate_plan_attempt(
        &self,
        user_input: &str,
        intent: &super::intent::Intent,
        attempt: usize,
    ) -> Result<ProjectPlan> {
        // Load planning prompt
        let mut prompt_manager = self.context.prompt_manager.write().await;
        let mut prompt_context = PromptContext::default();
        prompt_context.current_request = Some(user_input.to_string());
        prompt_context.task_analysis = Some(format!("{:?}", intent.intent_type));
        prompt_context.variables.insert("intent_type".to_string(), serde_json::json!(format!("{:?}", intent.intent_type)));
        prompt_context.variables.insert("entities".to_string(), serde_json::json!(format!("{:?}", intent.entities)));
        
        let system_prompt = prompt_manager.get_prompt(
            &self.id,
            AgentState::Planning,
            &prompt_context
        ).await?;
        
        drop(prompt_manager);

        // Select the best model for planning
        let selection_context = SelectionContext::for_admin();
        
        // Load user preference for Admin agent if available
        let preferred_family = if let Some(ref user_settings) = self.context.user_settings {
            let settings = user_settings.read().await;
            match settings.get_model_preference("admin").await {
                Ok(Some(family)) => {
                    tracing::info!("✅ Loaded user preference for Admin: family='{}'", family);
                    Some(family)
                },
                Ok(None) => {
                    tracing::warn!("⚠️  No user preference set for Admin agent");
                    None
                },
                Err(e) => {
                    tracing::error!("❌ Failed to load user preference for Admin: {:?}", e);
                    None
                }
            }
        } else {
            tracing::warn!("⚠️  UserSettingsManager not available in context");
            None
        };
        
        tracing::info!("🎯 Model selection for Admin planning: preferred_family={:?}", preferred_family);
        
        let selected_model = self.ai_provider_manager
            .select_model_for_agent_with_preferences(selection_context, preferred_family)
            .await
            .context("Failed to select a model for planning")?;
        
        // Create progressively simpler prompts based on attempt number
        let planning_prompt = self.create_planning_prompt(user_input, attempt);
        
        // Use LLM config with adjustment for planning (lower temp for structured output)
        let planning_temp = match attempt {
            1 => self.llm_config.temperature * 0.5,  // Lower temperature for first attempt
            2 => self.llm_config.temperature * 0.3,  // Even lower on retry
            _ => 0.1,  // Minimal creativity on final attempts
        };
        
        let options = GenerationOptions {
            temperature: Some(planning_temp),
            max_tokens: Some(self.llm_config.max_tokens.min(2048) as usize), // Cap at 2048 for planning
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
        ).await.context("Failed to generate project plan with LLM")?;
        
        // Log the full response for debugging
        debug!(
            target: "llm_messages",
            "[ADMIN PLANNING RESPONSE] Model: {}, Response ({} chars):\n{}",
            model_name,
            response.text.len(),
            response.text
        );
        
        // Parse JSON response
        let plan = self.parse_project_plan(&response.text)?;
        
        // Log parsed plan result
        trace!(
            target: "llm_messages",
            "[ADMIN PLANNING RESULT] Parsed successfully: title='{}', tasks={}, overview_len={}",
            plan.title,
            plan.initial_tasks.len(),
            plan.overview.len()
        );
        
        Ok(plan)
    }
    
    /// Create planning prompt with progressive simplification
    fn create_planning_prompt(&self, user_input: &str, attempt: usize) -> String {
        match attempt {
            1 => {
                // Attempt 1: Full structured prompt with JSON schema
                format!(
                    "User Request: {}\n\n\
                     YOUR RESPONSE MUST MATCH THIS JSON SCHEMA:\n\n\
                     {{\n\
                       \"$schema\": \"http://json-schema.org/draft-07/schema#\",\n\
                       \"type\": \"object\",\n\
                       \"required\": [\"title\", \"overview\", \"tasks\"],\n\
                       \"properties\": {{\n\
                         \"title\": {{\n\
                           \"type\": \"string\",\n\
                           \"minLength\": 10,\n\
                           \"maxLength\": 60\n\
                         }},\n\
                         \"overview\": {{\n\
                           \"type\": \"string\",\n\
                           \"minLength\": 20\n\
                         }},\n\
                         \"tasks\": {{\n\
                           \"type\": \"array\",\n\
                           \"items\": {{\"type\": \"string\"}},\n\
                           \"minItems\": 3,\n\
                           \"maxItems\": 7\n\
                         }}\n\
                       }}\n\
                     }}\n\n\
                     VALIDATION CHECKLIST:\n\
                     [ ] Response starts with {{ and ends with }}\n\
                     [ ] \"title\" is 10-60 characters\n\
                     [ ] \"overview\" is 20+ characters\n\
                     [ ] \"tasks\" is array of 3-7 strings\n\
                     [ ] NO markdown (no ```json)\n\
                     [ ] NO extra text\n\n\
                     Your JSON:",
                    user_input
                )
            },
            2 => {
                // Attempt 2: Simplified format-focused prompt
                format!(
                    "User Request: {}\n\n\
                     CREATE JSON IN THIS EXACT FORMAT:\n\
                     {{\n\
                       \"title\": \"<project name>\",\n\
                       \"overview\": \"<description>\",\n\
                       \"tasks\": [\"<task 1>\", \"<task 2>\", \"<task 3>\"]\n\
                     }}\n\n\
                     RULES:\n\
                     1. ONLY JSON (no markdown, no text)\n\
                     2. Start with {{ end with }}\n\
                     3. tasks = array of strings\n\
                     4. 3-7 tasks\n\n\
                     JSON:",
                    user_input
                )
            },
            _ => {
                // Attempt 3+: Minimal template-fill prompt
                format!(
                    "Fill this JSON template for: {}\n\n\
                     {{\n\
                       \"title\": \"___\",\n\
                       \"overview\": \"___\",\n\
                       \"tasks\": [\"___\", \"___\", \"___\"]\n\
                     }}",
                    user_input
                )
            }
        }
    }
    
    /// Validate project plan structure
    fn validate_project_plan(&self, plan: &ProjectPlan) -> Result<()> {
        // Validate title
        if plan.title.len() < 10 || plan.title.len() > 60 {
            return Err(anyhow::anyhow!("Title must be 10-60 characters, got {}", plan.title.len()));
        }
        
        // Validate overview
        if plan.overview.len() < 20 {
            return Err(anyhow::anyhow!("Overview must be at least 20 characters, got {}", plan.overview.len()));
        }
        
        // Validate tasks
        if plan.initial_tasks.len() < 3 || plan.initial_tasks.len() > 7 {
            return Err(anyhow::anyhow!("Must have 3-7 tasks, got {}", plan.initial_tasks.len()));
        }
        
        // Check each task is non-empty
        for (i, task) in plan.initial_tasks.iter().enumerate() {
            if task.trim().is_empty() {
                return Err(anyhow::anyhow!("Task {} is empty", i + 1));
            }
        }
        
        Ok(())
    }
    
    /// Parse LLM response into ProjectPlan using multi-strategy JSON parsing
    fn parse_project_plan(&self, llm_response: &str) -> Result<ProjectPlan> {
        // Log the raw response for debugging
        tracing::debug!("LLM response for project plan: {}", llm_response);
        
        // Use multi-strategy JSON parser
        let parse_result = JSONValidator::parse_with_fallbacks(llm_response);
        
        let parsed = match parse_result.value {
            Some(val) => {
                tracing::info!("Successfully parsed JSON using strategy: {}", parse_result.strategy_used);
                val
            },
            None => {
                tracing::error!("All JSON parsing strategies failed: {}", 
                               parse_result.error.unwrap_or_else(|| "Unknown error".to_string()));
                return Err(anyhow::anyhow!("Failed to parse LLM response as valid JSON"));
            }
        };
        
        // Extract fields, supporting both new schema (plan_title/plan_overview/plan_task_list) 
        // and old schema (title/overview/tasks)
        let title = parsed.get("plan_title")
            .or_else(|| parsed.get("title"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'plan_title' or 'title' in plan. Parsed JSON: {:?}", parsed))?
            .to_string();
        
        let overview = parsed.get("plan_overview")
            .or_else(|| parsed.get("overview"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'plan_overview' or 'overview' in plan. Parsed JSON: {:?}", parsed))?
            .to_string();
        
        // Parse tasks - handle both string array and object array formats
        let tasks_array = parsed.get("plan_task_list")
            .or_else(|| parsed.get("tasks"))
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Missing 'plan_task_list' or 'tasks' array in plan. Parsed JSON: {:?}", parsed))?;
        
        let tasks: Vec<String> = tasks_array.iter()
            .filter_map(|t| {
                // Try as string first
                if let Some(s) = t.as_str() {
                    Some(s.to_string())
                } 
                // Try as object with "description" field
                else if let Some(obj) = t.as_object() {
                    obj.get("description")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                } 
                else {
                    None
                }
            })
            .collect();
        
        if tasks.is_empty() {
            return Err(anyhow::anyhow!("No valid tasks found in plan. Parsed JSON: {:?}", parsed));
        }
        
        tracing::info!("Successfully parsed project plan: title='{}', {} tasks (strategy: {})", 
                      title, tasks.len(), parse_result.strategy_used);
        
        Ok(ProjectPlan {
            title,
            overview,
            initial_tasks: tasks,
        })
    }
    
    /// Create a new project
    async fn create_project(
        &self,
        title: String,
        overview: String,
        initial_tasks: Vec<String>,
    ) -> Result<ProjectId> {
        let project_manager = self.project_manager.write().await;
        
        let project_id = project_manager.create_project(
            title,
            overview,
            initial_tasks
        ).await?;
        
        tracing::info!("Created project: {}", project_id);
        
        Ok(project_id)
    }
    
    /// Spawn a new PM agent for a project
    async fn spawn_pm_agent(
        &self,
        project_id: &ProjectId,
        _plan: &ProjectPlan,
    ) -> Result<AgentId> {
        // Create PM agent - pass shared MCP client and user settings from context
        let mut pm_agent = PMAgent::new(
            project_id.clone(),
            self.context.message_bus.clone(),
            self.context.prompt_manager.clone(),
            self.project_manager.clone(),
            self.ai_provider_manager.clone(),
            self.context.mcp_client.clone(), // Pass shared MCP client
            self.context.user_settings.clone(), // Pass user settings for model preferences
        );
        let pm_id = pm_agent.id().clone();
        
        // Assign PM to project
        {
            let project_manager = self.project_manager.write().await;
            project_manager.assign_pm(project_id, pm_agent.id().clone()).await?;
        }
        
        // Start PM agent (transitions to Planning → Managing)
        pm_agent.initialize_and_plan().await?;
        
        // Spawn the PM agent to run in the background
        tokio::spawn(async move {
            if let Err(e) = pm_agent.manage_loop().await {
                tracing::error!("PMAgent {} manage_loop failed: {:?}", pm_agent.id().name, e);
            }
        });
        
        tracing::info!("Spawned PM agent {} for project {}", pm_id.name, project_id);
        
        Ok(pm_id)
    }
    
    /// Handles simple intents conversationally.
    async fn handle_simple_intent(
        &mut self,
        user_input: &str,
        intent: &super::intent::Intent,
    ) -> Result<String> {
        tracing::info!("DEBUG: handle_simple_intent started");
        let start_time = Instant::now();

        // 1. Ensure Conversation state
        if *self.state_machine.current_state() != AgentState::Conversation {
            self.state_machine.transition(
                AgentState::Conversation,
                "Simple intent, conversational response".to_string()
            )?;
        }

        // 2. Generate conversational response
        let result = self.generate_conversational_response(user_input, intent).await;

        // 3. Record metrics
        let response_time = start_time.elapsed();
        let metrics = self.metrics.read().await;

        match &result {
            Ok(response) => {
                let input_tokens = (user_input.len() / 4) as u32;
                let output_tokens = (response.len() / 4) as u32;
                metrics.record_operation(OperationResult {
                    agent_type: AgentType::Admin,
                    agent_id: self.id.clone(),
                    config_hash: super::metrics::hash_config(&self.llm_config),
                    operation_type: "simple_intent_response".to_string(),
                    success: true,
                    response_time,
                    tokens_used: Some(input_tokens + output_tokens),
                    ..Default::default()
                }).await?;
            },
            Err(e) => {
                metrics.record_operation(OperationResult {
                    agent_type: AgentType::Admin,
                    agent_id: self.id.clone(),
                    config_hash: super::metrics::hash_config(&self.llm_config),
                    operation_type: "simple_intent_response".to_string(),
                    success: false,
                    response_time,
                    error_message: Some(e.to_string()),
                    ..Default::default()
                }).await?;
            }
        }

        result
    }

    /// Generates a conversational response for a simple intent.
    async fn generate_conversational_response(
        &self,
        user_input: &str,
        _intent: &super::intent::Intent,
    ) -> Result<String> {
        tracing::info!("DEBUG: generate_conversational_response started");
        // Load conversation prompt
        let mut prompt_manager = self.context.prompt_manager.write().await;
        let mut prompt_context = PromptContext::default();
        
        // Set current_state so the renderer can find the correct state prompt
        prompt_context.variables.insert("current_state".to_string(), serde_json::Value::String("conversation".to_string()));
        
        // Retrieve context
        let history = self.memory.get_recent_context(10).await.unwrap_or_default();
        let goals = self.profile.get_goals().await.unwrap_or_default();
        let preferences = self.profile.get_preferences().await.unwrap_or_default();
        
        let mut memory_context = String::new();
        
        if !goals.is_empty() {
            memory_context.push_str("USER GOALS:\n");
            for goal in goals {
                memory_context.push_str(&format!("- {}\n", goal));
            }
            memory_context.push('\n');
        }

        if !preferences.is_empty() {
            memory_context.push_str("USER PREFERENCES:\n");
            for (k, v) in preferences {
                memory_context.push_str(&format!("- {}: {}\n", k, v));
            }
            memory_context.push('\n');
        }
        
        if !history.is_empty() {
            memory_context.push_str("RECENT CONVERSATION:\n");
            for entry in history {
                let user_msg = if entry.user_message.chars().count() > 200 {
                    let truncated: String = entry.user_message.chars().take(197).collect();
                    format!("{}...", truncated)
                } else {
                    entry.user_message.clone()
                };
                let admin_msg = if entry.admin_response.chars().count() > 200 {
                    let truncated: String = entry.admin_response.chars().take(197).collect();
                    format!("{}...", truncated)
                } else {
                    entry.admin_response.clone()
                };
                memory_context.push_str(&format!("User: {}\nAdmin: {}\n", user_msg, admin_msg));
            }
            memory_context.push('\n');
        }

        // Add project context
        let project_context = self.get_project_context().await;
        memory_context.push_str("\n");
        memory_context.push_str(&project_context);
        
        // Combine with user input
        let full_request = if !memory_context.is_empty() {
            format!("CONTEXT:\n{}\n\nUSER REQUEST:\n{}", memory_context, user_input)
        } else {
            user_input.to_string()
        };
        
        tracing::info!("DEBUG: Admin conversation full_request:\n{}", full_request);
        
        // Set prompt variables - use user_input for current_request since memory_context is injected separately
        prompt_context.current_request = Some(user_input.to_string());
        prompt_context.variables.insert("user_input".to_string(), serde_json::Value::String(user_input.to_string()));
        prompt_context.variables.insert("memory_context".to_string(), serde_json::Value::String(memory_context));
        
        // Add system status variables
        prompt_context.variables.insert("hub_status".to_string(), serde_json::Value::String("Online".to_string()));
        prompt_context.variables.insert("device_count".to_string(), serde_json::json!(1)); // TODO: Get real device count
        prompt_context.variables.insert("mesh_status".to_string(), serde_json::Value::String("Active".to_string()));
        prompt_context.variables.insert("count".to_string(), serde_json::json!(self.active_project_count()));
        
        let system_prompt = prompt_manager.get_prompt(
            &self.id,
            AgentState::Conversation,
            &prompt_context
        ).await?;
        
        tracing::info!("DEBUG: Rendered system_prompt:\n{}", system_prompt);
        
        drop(prompt_manager);

        // Select the best model for conversation
        let selection_context = SelectionContext::for_admin();
        
        let preferred_family = if let Some(ref user_settings) = self.context.user_settings {
            let settings = user_settings.read().await;
            settings.get_model_preference("admin").await.ok().flatten()
        } else {
            None
        };
        
        let selected_model = self.ai_provider_manager
            .select_model_for_agent_with_preferences(selection_context, preferred_family)
            .await
            .context("Failed to select a model for conversation")?;
        
        // Generate conversational response using LLM config
        let options = GenerationOptions {
            temperature: Some(self.llm_config.temperature),
            max_tokens: Some(self.llm_config.max_tokens.min(512) as usize),
            system: Some(system_prompt),
            ..Default::default()
        };
        
        let client = selected_model.get_client()?;
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };
        
        let response = client.generate(
            model_name,
            user_input,
            options
        ).await.context("Failed to generate conversational response")?;
        
        debug!(
            target: "llm_messages",
            "[ADMIN CONVERSATION RESPONSE] Model: {}, Response ({} chars):\n{}",
            model_name,
            response.text.len(),
            response.text
        );
        
        Ok(response.text)
    }
    
    /// Get context about recent projects (active and completed)
    async fn get_project_context(&self) -> String {
        let project_manager = self.project_manager.read().await;
        // Use get_recent_projects to include completed ones
        match project_manager.get_recent_projects(5).await {
            Ok(projects) => {
                tracing::info!("DEBUG: Admin retrieved {} recent projects for context", projects.len());
                if projects.is_empty() {
                    return "No projects found.".to_string();
                }
                
                let mut summary = String::from("PROJECT CONTEXT:\n");
                // Sort by created_at desc (already done by SQL query)
                for project in projects.iter().take(5) {
                    summary.push_str(&format!("- {} ({}): {}\n", project.title, project.status, project.overview));
                }
                tracing::info!("DEBUG: Project context summary:\n{}", summary);
                summary
            },
            Err(e) => {
                tracing::error!("Failed to retrieve project context: {}", e);
                "Failed to retrieve project context.".to_string()
            }
        }
    }
    
    /// Monitor active projects for completion/failures
    pub async fn monitor_projects(&mut self) -> Result<()> {
        if self.active_projects.is_empty() {
            // No active projects, transition to Conversation
            if *self.state_machine.current_state() == AgentState::Monitoring {
                self.state_machine.transition(
                    AgentState::Conversation,
                    "No active projects, returning to conversation".to_string()
                )?;
            }
            return Ok(());
        }
        
        let project_manager = self.project_manager.read().await;
        
        // Check each active project
        let mut completed_projects = Vec::new();
        
        for (project_id, _pm_id) in &self.active_projects {
            if let Some(project) = project_manager.get_project(project_id).await? {
                if project.status.is_terminal() {
                    tracing::info!("Project {} completed with status: {:?}", project_id, project.status);
                    completed_projects.push(project_id.clone());
                }
            }
        }
        
        // Remove completed projects
        for project_id in completed_projects {
            self.active_projects.remove(&project_id);
        }
        
        Ok(())
    }
    
    /// Get current state
    pub fn state(&self) -> &AgentState {
        self.state_machine.current_state()
    }
    
    /// Get number of active projects
    pub fn active_project_count(&self) -> usize {
        self.active_projects.len()
    }
}

/// Project plan generated by LLM
#[derive(Debug, Clone)]
struct ProjectPlan {
    title: String,
    overview: String,
    initial_tasks: Vec<String>,
}

#[async_trait::async_trait]
impl Agent for AdminAgent {
    fn id(&self) -> &crate::messaging::AgentId {
        &self.id
    }
    
    async fn process_message(&mut self, message: Message) -> Result<()> {
        // Process incoming messages from PM agents
        tracing::debug!("Admin received message from {}: {:?}", message.from.name, message.content);
        
        if let crate::messaging::MessageContent::StatusUpdate(status) = message.content {
            // Check if this is a completion update (Idle state + 100% progress)
            if status.state == AgentState::Idle && status.progress == Some(1.0) {
                let user_msg = format!("🔔 PROJECT UPDATE from {}: {}", message.from.name, status.message);
                
                // Send notification to User
                // The UI should be listening for messages directed to "user"
                let user_id = crate::messaging::AgentId::user("user".to_string());
                let response = crate::messaging::Message::new(
                    self.id.clone(),
                    user_id,
                    crate::messaging::MessageContent::Response(user_msg)
                );
                
                if let Err(e) = self.context.message_bus.write().await.send_message(response).await {
                    tracing::error!("Failed to forward project update to user: {:?}", e);
                }
            }
        }
        
        Ok(())
    }
    
    async fn start(&mut self) -> Result<()> {
        self.running = true;
        
        // Transition directly from Startup to Conversation (valid for Admin AI)
        self.state_machine.transition(
            AgentState::Conversation,
            "Admin AI started, ready for user interaction".to_string()
        )?;
        
        tracing::info!("Admin AI started in Conversation state");
        
        // Spawn message processing loop
        if let Some(mut receiver) = self.receiver.take() {
            let message_bus = self.context.message_bus.clone();
            let admin_id = self.id.clone();
            
            tokio::spawn(async move {
                tracing::info!("Admin message processing loop started");
                
                while let Some(message) = receiver.recv().await {
                    tracing::debug!("Admin received message from {}: {:?}", message.from.name, message.content);
                    
                    // Handle StatusUpdate messages from PM agents
                    if let crate::messaging::MessageContent::StatusUpdate(status) = &message.content {
                        // Check if this is a completion update (Idle state + 100% progress)
                        if status.state == AgentState::Idle && status.progress == Some(1.0) {
                            let user_msg = format!("🔔 PROJECT UPDATE from {}: {}", message.from.name, status.message);
                            
                            // Send notification to User
                            let user_id = crate::messaging::AgentId::user("user".to_string());
                            let response = crate::messaging::Message::new(
                                admin_id.clone(),
                                user_id,
                                crate::messaging::MessageContent::Response(user_msg)
                            );
                            
                            if let Err(e) = message_bus.write().await.send_message(response).await {
                                tracing::error!("Failed to forward project update to user: {:?}", e);
                            } else {
                                tracing::info!("Admin forwarded project completion to user");
                            }
                        }
                    }
                }
                
                tracing::info!("Admin message processing loop ended");
            });
        } else {
            tracing::warn!("Admin started without message receiver - will not process system events");
        }
        
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<()> {
        self.running = false;
        
        // TODO: Gracefully stop all PM agents
        
        tracing::info!("Admin AI stopped");
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::MessageBus;
    use crate::prompts::PromptManager;
    use crate::tools::mcp::MCPClientManager;
    use crate::guardian::GuardianSystem;
    
    async fn create_test_context() -> Arc<AgentContext> {
        use tokio::sync::RwLock;
        use std::path::PathBuf;
        
        // Use absolute path to prompts directory
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let prompts_path = PathBuf::from(manifest_dir).join("prompts");
        let ai_provider_manager = Arc::new(AIProviderManager::new().await.unwrap());
        
        Arc::new(AgentContext::new(
            Arc::new(RwLock::new(MessageBus::new().await.expect("Failed to create MessageBus"))),
            Arc::new(RwLock::new(PromptManager::new(prompts_path).unwrap())),
            Arc::new(RwLock::new(MCPClientManager::new())),
            Arc::new(RwLock::new(GuardianSystem::new(ai_provider_manager, None))),
        ))
    }
    
    async fn create_test_admin() -> AdminAgent {
        let context = create_test_context().await;
        let project_manager = Arc::new(RwLock::new(
            ProjectManager::new("sqlite::memory:").await.unwrap()
        ));
        let ai_provider_manager = Arc::new(AIProviderManager::new().await.unwrap());
        let metrics = Arc::new(RwLock::new(
            MetricsCollector::new("sqlite::memory:").await.unwrap()
        ));
        
        AdminAgent::new(
            context, 
            project_manager, 
            ai_provider_manager, 
            metrics,
            "sqlite::memory:".to_string(),
            "sqlite::memory:".to_string()
        ).await.unwrap()
    }
    
    #[tokio::test]
    async fn test_admin_agent_creation() {
        let agent = create_test_admin().await;
        
        assert_eq!(agent.id().agent_type, AgentType::Admin);
        assert!(!agent.running);
        assert_eq!(agent.active_project_count(), 0);
    }
    
    #[tokio::test]
    async fn test_admin_agent_start() {
        let mut agent = create_test_admin().await;
        
        agent.start().await.unwrap();
        assert!(agent.running);
        assert_eq!(agent.state(), &AgentState::Conversation);
    }
    
    #[tokio::test]
    async fn test_complex_intent_detection() {
        let agent = create_test_admin().await;
        let intent_parser = IntentParser::new();
        
        // Complex intent
        let intent = intent_parser.parse("Build me a todo app with React").await.unwrap();
        let is_complex = agent.is_complex_intent(&intent, "Build me a todo app with React").unwrap();
        assert!(is_complex, "Should detect 'build' as complex intent");
        
        // Simple intent
        let intent2 = intent_parser.parse("What time is it?").await.unwrap();
        let is_simple = !agent.is_complex_intent(&intent2, "What time is it?").unwrap();
        assert!(is_simple, "Should detect question as simple intent");
    }
    
    #[tokio::test]
    async fn test_parse_project_plan() {
        let agent = create_test_admin().await;
        
        let json_response = r#"{
            "title": "Todo App Development",
            "overview": "Create a modern todo application using React",
            "tasks": ["Setup React project", "Design UI", "Implement features", "Add tests"]
        }"#;
        
        let plan = agent.parse_project_plan(json_response).unwrap();
        
        assert_eq!(plan.title, "Todo App Development");
        assert!(plan.overview.contains("React"));
        assert_eq!(plan.initial_tasks.len(), 4);
    }
    
    #[tokio::test]
    async fn test_parse_project_plan_with_markdown() {
        let agent = create_test_admin().await;
        
        let markdown_response = r#"
        Here's the project plan:
        
        ```json
        {
            "title": "Website Builder",
            "overview": "Build a static website generator",
            "tasks": ["Research tools", "Create templates", "Write generator"]
        }
        ```
        "#;
        
        let plan = agent.parse_project_plan(markdown_response).unwrap();
        
        assert_eq!(plan.title, "Website Builder");
        assert_eq!(plan.initial_tasks.len(), 3);
    }
    
    #[tokio::test]
    async fn test_state_transitions() {
        let mut agent = create_test_admin().await;
        
        agent.start().await.unwrap();
        assert_eq!(agent.state(), &AgentState::Conversation);
        
        // Test valid state transitions for Admin AI
        let intent = agent.intent_parser.parse("Build me an app").await.unwrap();
        agent.transition_to_planning(&intent).unwrap();
        assert_eq!(agent.state(), &AgentState::Planning);
        
        // Planning -> Conversation is valid (going back)
        agent.state_machine.transition(
            AgentState::Conversation,
            "Testing".to_string()
        ).unwrap();
        assert_eq!(agent.state(), &AgentState::Conversation);
    }
}
