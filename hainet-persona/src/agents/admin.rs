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
use super::failover::{FailoverHandler, ModelEndpoint};
use super::loop_detector;
use super::session_tasks::SessionTaskList;
use crate::config::HaiNetConfig;
use crate::messaging::{AgentId, Message};
use crate::prompts::{AgentType, AgentState};
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
    
    /// Take the message receiver (can only be done once)
    pub fn take_receiver(&mut self) -> Option<tokio::sync::mpsc::Receiver<crate::messaging::Message>> {
        self.receiver.take()
    }
    
    /// Handle incoming message from other agents
    pub async fn handle_message(&mut self, msg: crate::messaging::Message) -> Result<()> {
        match msg.content {
            crate::messaging::MessageContent::ErrorReport(report) => {
                tracing::warn!("Admin received error report from {}: {}", msg.from, report.message);
                
                // Format user-friendly notification
                let notification = format!(
                    "⚠️ **Issue Reported**\n\nI received an error from **{}**:\n\n> {}\n\nI'm looking into it. You may need to check the project status.",
                    msg.from.name,
                    report.message
                );
                
                // Send to User agent so it appears in chat
                let user_id = crate::messaging::AgentId::user("user".to_string());
                let response_msg = crate::messaging::Message::new(
                    self.id.clone(),
                    user_id,
                    crate::messaging::MessageContent::Response(notification)
                );
                
                self.context.message_bus.write().await.send_message(response_msg).await?;
            },
            _ => {
                tracing::debug!("Admin received unhandled message type from {}", msg.from);
            }
        }
        Ok(())
    }

    /// Main entry point for user interaction
    pub async fn process_user_input(&mut self, user_input: String, session_id: &str) -> Result<String> {
        tracing::info!("DEBUG: Admin process_user_input called with: {}, session_id: {}", user_input, session_id);

        if self.is_tool_execution_request(&user_input) {
            tracing::info!("DEBUG: Detected tool execution request");
            return self.handle_tool_execution_request(&user_input).await;
        }

        // 3. Parse Intent
        tracing::info!("DEBUG: Parsing intent...");
        self.update_status("Parsing user intent").await;
        let intent = self.intent_parser.parse(&user_input).await?;
        tracing::info!("Parsed intent: {:?} (confidence: {})", intent.intent_type, intent.confidence);

        // 4. Handle initial startup state
        // If still in Startup state, transition to Conversation first
        if *self.state_machine.current_state() == AgentState::Startup {
            tracing::warn!("Admin AI still in Startup state, transitioning to Conversation");
            self.state_machine.transition(
                AgentState::Conversation,
                "Auto-transition from Startup on first message".to_string()
            )?;
            self.update_status("Transitioned to Conversation").await;
        }
        
        // 5. Add user request to session tasks
        let request_title = if user_input.len() > 50 {
            format!("{}...", &user_input[..47])
        } else {
            user_input.clone()
        };
        self.session_tasks.add_task(request_title.clone(), None);
        let _ = self.session_tasks.start_task(&request_title);

        // 6. Route to appropriate handler based on intent complexity
        let is_complex = self.is_complex_intent(&intent, &user_input)?;
        tracing::info!("DEBUG: Intent is complex: {}", is_complex);
        
        let result = if is_complex {
            tracing::info!("DEBUG: Calling handle_complex_intent");
            self.handle_complex_intent(&user_input, &intent).await
        } else {
            tracing::info!("DEBUG: Calling handle_simple_intent");
            self.handle_simple_intent(&user_input, &intent, session_id).await
        };
        
        // Save to memory if successful
        if let Ok(ref response) = result {
            let entry = crate::agents::admin::memory::ConversationEntry {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: session_id.to_string(),
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
        let mut system_prompt = self.get_te_prompt("admin_ai_planning_prompt")
            .unwrap_or_else(|| "You are Admin AI. Plan the project.".to_string());
            
        let tools_list_str = "['request_state', 'project_management', 'manage_team', 'send_message']";
        let date = chrono::Utc::now().to_rfc3339();
        
        system_prompt = system_prompt.replace("{agent_id}", &self.id.name);
        system_prompt = system_prompt.replace("{personality_instructions}", "You are the central Admin AI of HAI-Net.");
        system_prompt = system_prompt.replace("{session_name}", "admin_session");
        system_prompt = system_prompt.replace("{current_time_utc}", &date);
        system_prompt = system_prompt.replace("{tool_instructions}", tools_list_str);
        system_prompt = system_prompt.replace("{address_book}", "pm, workers");
        
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
        
        let mut failover_handler = FailoverHandler::new();
        let initial_endpoint = ModelEndpoint {
            provider: "local".to_string(),
            model: selected_model.model_id.clone(),
            api_key_id: None,
        };
        failover_handler.add_endpoint(initial_endpoint.clone());
        failover_handler.set_active(initial_endpoint.clone());

        let client = selected_model.get_client()?;
        // Strip provider prefix from model_id (e.g., "Ollama::model" -> "model")
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };
        
        let mut response_text = String::new();
        let mut success = false;
        
        for _ in 0..3 {
            // Add timeout wrapper to prevent indefinite hanging
            tracing::info!("[DIAGNOSTIC] Admin {} calling LLM for planning (model: {})", self.id.name, model_name);
            let llm_timeout = tokio::time::Duration::from_secs(300); // 300s timeout for LLM generation
            match tokio::time::timeout(
                llm_timeout,
                client.generate(model_name, &planning_prompt, options.clone())
            ).await {
                Ok(Ok(response)) => {
                    if loop_detector::check_output_limit(&response.text) {
                        tracing::warn!("Admin {} LLM output truncated due to character limit", self.id.name);
                        failover_handler.report_transient_failure(&initial_endpoint, "output_limit_exceeded");
                        continue;
                    }
                    if let Some(pattern_len) = loop_detector::detect_autoregressive_loop(&response.text) {
                        tracing::warn!("Admin {} LLM stuck in autoregressive loop (pattern len {})", self.id.name, pattern_len);
                        failover_handler.report_transient_failure(&initial_endpoint, "autoregressive_loop");
                        continue;
                    }
                    response_text = response.text;
                    success = true;
                    break;
                }
                Ok(Err(e)) => {
                    failover_handler.report_transient_failure(&initial_endpoint, &e.to_string());
                }
                Err(_) => {
                    failover_handler.report_transient_failure(&initial_endpoint, "timeout");
                }
            }
        }
        
        if !success {
            return Err(anyhow::anyhow!("Failed to generate project plan with LLM after failover retries"));
        }
        
        // Parse JSON response
        let plan = self.parse_project_plan(&response_text)?;
        
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
        
        // Spawn the PM agent's full autonomous lifecycle in the background.
        // initialize_and_plan() now runs the complete TrippleEffect autonomous cycle
        // (Startup → Planning → Managing → Auditing → Standby) — no separate manage_loop needed.
        tokio::spawn(async move {
            if let Err(e) = pm_agent.initialize_and_plan().await {
                tracing::error!("PMAgent {} autonomous cycle failed: {:?}", pm_agent.id().name, e);
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
        session_id: &str,
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
        let result = self.generate_conversational_response(user_input, intent, session_id).await;

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
        session_id: &str,
    ) -> Result<String> {
        tracing::info!("DEBUG: generate_conversational_response started");
        // Load conversation prompt
        // Retrieve context
        let history = self.memory.get_recent_context(session_id, 10).await.unwrap_or_default();
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
        
        let mut system_prompt = self.get_te_prompt("admin_ai_conversation_prompt")
            .unwrap_or_else(|| "You are Admin AI. Respond conversationally.".to_string());
        
        let tools_list_str = "['request_state', 'project_management', 'manage_team', 'send_message']";
        let date = chrono::Utc::now().to_rfc3339();
        
        system_prompt = system_prompt.replace("{agent_id}", &self.id.name);
        system_prompt = system_prompt.replace("{personality_instructions}", "You are the central Admin AI of HAI-Net.");
        system_prompt = system_prompt.replace("{session_name}", "admin_session");
        system_prompt = system_prompt.replace("{current_time_utc}", &date);
        system_prompt = system_prompt.replace("{tool_instructions}", tools_list_str);
        system_prompt = system_prompt.replace("{address_book}", "pm, workers");
        
        // Append user_input and memory context
        system_prompt.push_str("\n\n--- CURRENT CONVERSATION CONTEXT ---\n");
        system_prompt.push_str(&memory_context);
        
        tracing::info!("DEBUG: Rendered system_prompt from TE YAML:\n{}", system_prompt);
        
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
        
        tracing::info!("[DIAGNOSTIC] Admin {} calling LLM for conversation (model: {})", self.id.name, model_name);
        
        let llm_timeout = tokio::time::Duration::from_secs(300);
        let response = match tokio::time::timeout(
            llm_timeout,
            client.generate(model_name, user_input, options)
        ).await {
            Ok(Ok(res)) => res,
            Ok(Err(e)) => anyhow::bail!("Failed to generate conversational response: {}", e),
            Err(_) => anyhow::bail!("Timeout waiting for LLM conversational response"),
        };
        
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
    
    /// Load a prompt template from TrippleEffect's prompts.yaml by key name.
    /// Replaces {admin_standard_framework_instructions} automatically.
    fn get_te_prompt(&self, prompt_name: &str) -> Option<String> {
        let extract = |name: &str| -> Option<String> {
            super::prompt_loader::get_prompt(name)
        };

        let mut prompt = extract(prompt_name)?;
        
        if prompt.contains("{admin_standard_framework_instructions}") {
            if let Some(instructions) = extract("admin_standard_framework_instructions") {
                prompt = prompt.replace("{admin_standard_framework_instructions}", &instructions);
            }
        }
        
        Some(prompt)
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
            match project_manager.get_project(project_id).await {
                Ok(Some(project)) => {
                    if project.status.is_terminal() {
                        tracing::info!("Project {} completed with status: {:?}", project_id, project.status);
                        completed_projects.push(project_id.clone());
                    }
                }
                Ok(None) => {
                    // Project was deleted from the database
                    tracing::info!("Project {} no longer exists in database, removing from monitor", project_id);
                    completed_projects.push(project_id.clone());
                }
                Err(e) => {
                    tracing::warn!("Error checking project {} status: {:?}", project_id, e);
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
    
    // ========== Tool Execution Capabilities ==========
    
    /// Discover all available tools from MCP servers
    async fn discover_available_tools(&mut self) -> Result<Vec<String>> {
        let mcp_client = self.context.mcp_client.read().await;
        let servers = mcp_client.list_servers().await;
        
        let mut all_tools = Vec::new();
        for server in &servers {
            if let Ok(tools) = mcp_client.list_tools(server).await {
                for tool in tools {
                    all_tools.push(format!("{}::{}", server, tool.name));
                }
            }
        }
        
        tracing::info!("Admin discovered {} tools across {} servers", all_tools.len(), servers.len());
        Ok(all_tools)
    }
    
    /// Load metadata for selected tools
    async fn load_tool_metadata(&mut self, tool_names: &[String]) -> Result<std::collections::HashMap<String, String>> {
        use std::collections::HashMap;
        
        let mut metadata_map = HashMap::new();
        let mcp_client = self.context.mcp_client.read().await;
        
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
                    tracing::warn!("Failed to load metadata for {}: {}", tool_identifier, e);
                }
            }
        }
        
        Ok(metadata_map)
    }
    
    /// Format tool information based on requested flags
    /// Supports progressive disclosure: compact by default, detailed on demand
    fn format_tool_info(full_metadata: &str, flags: &str) -> String {
        // If --all flag is present, return full metadata (backward compatible)
        if flags.contains("--all") {
            return full_metadata.to_string();
        }
        
        // Try to parse metadata as JSON
        let meta: serde_json::Value = match serde_json::from_str(full_metadata) {
            Ok(v) => v,
            Err(_) => {
                // If not JSON, return as-is with flag info
                return format!("{}\n\nAvailable flags: --params, --examples, --errors, --all", full_metadata);
            }
        };
        
        let mut output = String::new();
        
        // Always include tool name and description
        if let Some(name) = meta.get("name") {
            output.push_str(&format!("Tool: {}\n", name.as_str().unwrap_or("")));
        }
        if let Some(desc) = meta.get("description") {
            output.push_str(&format!("Description: {}\n", desc.as_str().unwrap_or("")));
        }
        
        // If no flags specified, show compact info with available flags
        if flags.is_empty() {
            output.push_str("\nAvailable flags:\n");
            output.push_str("  --params   : Show parameter schema\n");
            output.push_str("  --examples : Show usage examples\n");
            output.push_str("  --errors   : Show common errors\n");
            output.push_str("  --all      : Show full metadata\n");
            output.push_str("\nUsage: admin::get_tool_info({\"tool_name\": \"...\", \"flags\": \"--params\"})\n");
            return output;
        }
        
        output.push_str("\n");
        
        // Show requested sections
        if flags.contains("--params") {
            if let Some(schema) = meta.get("inputSchema") {
                output.push_str("=== PARAMETERS ===\n");
                if let Some(props) = schema.get("properties") {
                    if let Some(obj) = props.as_object() {
                        for (key, value) in obj {
                            let type_str = value.get("type")
                                .and_then(|t| t.as_str())
                                .unwrap_or("any");
                            let desc = value.get("description")
                                .and_then(|d| d.as_str())
                                .unwrap_or("");
                            let required = schema.get("required")
                                .and_then(|r| r.as_array())
                                .map(|arr| arr.iter().any(|v| v.as_str() == Some(key)))
                                .unwrap_or(false);
                            
                            output.push_str(&format!("  {} ({}){}: {}\n", 
                                key, 
                                type_str,
                                if required { " [REQUIRED]" } else { "" },
                                desc
                            ));
                        }
                    }
                }
                output.push_str("\n");
            }
        }
        
        if flags.contains("--examples") {
            if let Some(examples) = meta.get("examples") {
                output.push_str("=== EXAMPLES ===\n");
                output.push_str(&format!("{}\n\n", examples.as_str().unwrap_or("No examples available")));
            }
        }
        
        if flags.contains("--errors") {
            if let Some(errors) = meta.get("commonErrors") {
                output.push_str("=== COMMON ERRORS ===\n");
                output.push_str(&format!("{}\n\n", errors.as_str().unwrap_or("No error documentation available")));
            }
        }
        
        output
    }

    
    /// Execute a single tool step (with admin::get_tool_info handler)
    async fn execute_tool_step(
        &mut self,
        step: &super::worker_discovery::DiscoveryExecutionStep,
        tool_metadata: &std::collections::HashMap<String, String>
    ) -> Result<String> {
        // Special handling for admin::get_tool_info (just-in-time tool discovery)
        if step.tool == "admin::get_tool_info" {
            let tool_name = step.params.get("tool_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing tool_name parameter for admin::get_tool_info"))?;
            
            let flags = step.params.get("flags")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            
            if let Some(metadata) = tool_metadata.get(tool_name) {
                tracing::debug!("Admin providing tool info for: {} (flags: '{}')", tool_name, flags);
                let info = Self::format_tool_info(metadata, flags);
                return Ok(info);
            } else {
                return Err(anyhow::anyhow!("Tool not found: {}. Available tools: {:?}", 
                    tool_name, 
                    tool_metadata.keys().collect::<Vec<_>>()
                ));
            }
        }
        
        // Parse tool name: "server::tool_name"
        let parts: Vec<&str> = step.tool.split("::").collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid tool format: {}", step.tool));
        }
        
        let (server, tool) = (parts[0], parts[1]);
        
        // Execute MCP tool
        let mcp_client = self.context.mcp_client.read().await;
        let result = mcp_client.call_tool(server, tool, step.params.clone()).await?;
        
        Ok(result.to_string())
    }
    
    /// Generate minimal execution plan for tool-based request
    async fn generate_tool_execution_plan(
        &mut self,
        user_request: &str,
        available_tools: &[String]
    ) -> Result<super::worker_discovery::DiscoveryExecutionPlan> {
        use super::worker_discovery::{format_tool_list, parse_execution_plan};
        use crate::ai_providers::SelectionContext;
        use crate::ai_providers::providers::GenerationOptions;
        
        let tool_list = format_tool_list(available_tools);
        let example_tool = available_tools.first().map(|s| s.as_str()).unwrap_or("server::tool");
        
        let prompt = format!(
            r#"USER REQUEST: {}

AVAILABLE TOOLS:
{}
- admin::get_tool_info

RULES:
1. Call admin::get_tool_info({{"tool_name": "{}"}}) to discover tool details
2. Use the information provided (flags like --params, --examples) to learn how to use the tool
3. Use FULL tool names from the list above

Return JSON with steps array:
{{"steps": [{{"step_number": 1, "tool": "admin::get_tool_info", "params": {{"tool_name": "..."}}, "description": "...", "depends_on": []}}]}}"#,
            user_request,
            tool_list,
            example_tool
        );
        
        // Use AI to generate plan
        let options = GenerationOptions {
            temperature: Some(0.2),
            max_tokens: Some(4096),
            ..Default::default()
        };
        
        let selection_context = SelectionContext::for_admin();
        let selected_model = self.ai_provider_manager
            .select_model_for_agent(selection_context)
            .await?;
        
        let client = selected_model.get_client()?;
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };
        
        let response = client.generate(model_name, &prompt, options).await?;
        
        // Parse using worker's existing parser
        parse_execution_plan(&response.text)
    }
    
    /// Check if user request is a direct tool execution request
    fn is_tool_execution_request(&self, user_input: &str) -> bool {
        // Simple heuristics for now:
        // - Contains tool-related keywords
        // - Short, direct commands (< 30 words)
        // - Not asking for project creation
        
        let keywords = [
            "search", "find", "read", "write", "list", "create file", 
            "delete", "show", "get", "fetch", "check"
        ];
        let word_count = user_input.split_whitespace().count();
        
        let has_keyword = keywords.iter().any(|k| user_input.to_lowercase().contains(k));
        let is_short = word_count < 30;
        let not_project_request = !user_input.to_lowercase().contains("project") 
            && !user_input.to_lowercase().contains("build me")
            && !user_input.to_lowercase().contains("create an app");
        
        has_keyword && is_short && not_project_request
    }
    
    /// Handle direct tool execution request
    async fn handle_tool_execution_request(&mut self, user_input: &str) -> Result<String> {
        tracing::info!("Admin handling tool execution request: {}", user_input);
        
        // 1. Discover available tools
        let available_tools = self.discover_available_tools().await?;
        
        // 2. Generate execution plan
        let plan = self.generate_tool_execution_plan(user_input, &available_tools).await?;
        
        // 3. Load metadata for selected tools (excluding admin::get_tool_info)
        let tool_names: Vec<String> = plan.steps.iter()
            .map(|s| s.tool.clone())
            .filter(|t| t != "admin::get_tool_info")
            .collect();
        let tool_metadata = self.load_tool_metadata(&tool_names).await?;
        
        // 4. Execute plan
        let mut results = Vec::new();
        for (idx, step) in plan.steps.iter().enumerate() {
            tracing::info!("Admin executing step {}/{}: {}", idx + 1, plan.steps.len(), step.description);
            
            match self.execute_tool_step(step, &tool_metadata).await {
                Ok(result) => {
                    tracing::debug!("Step {} result: {}", idx + 1, &result[..result.len().min(200)]);
                    results.push(format!("Step {}: {}\nResult: {}", idx + 1, step.description, result));
                }
                Err(e) => {
                    let error_msg = format!("Step {} failed: {}", idx + 1, e);
                    tracing::error!("{}", error_msg);
                    return Err(anyhow::anyhow!("Tool execution failed at step {}: {}", idx + 1, e));
                }
            }
        }
        
        // 5. Format results for user
        Ok(format!("✅ Executed {} steps successfully:\n\n{}", results.len(), results.join("\n\n")))
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
                    
                    match &message.content {
                        crate::messaging::MessageContent::StatusUpdate(status) => {
                            tracing::info!("Admin received status update from {}: {}", message.from.name, status.message);
                            
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
                        },
                        crate::messaging::MessageContent::Query(query) => {
                            tracing::info!("Admin received query from {}: {}", message.from.name, query);
                            // Forward query to user
                            let user_msg = format!("❓ QUESTION from {}: {}", message.from.name, query);
                            let user_id = crate::messaging::AgentId::user("user".to_string());
                            let response = crate::messaging::Message::new(
                                admin_id.clone(),
                                user_id,
                                crate::messaging::MessageContent::Response(user_msg)
                            );
                            
                            if let Err(e) = message_bus.write().await.send_message(response).await {
                                tracing::error!("Failed to forward query to user: {:?}", e);
                            }
                        },
                        crate::messaging::MessageContent::ErrorReport(error) => {
                            tracing::error!("Admin received error from {}: {}", message.from.name, error.message);
                            // Forward error to user
                            let user_msg = format!("⚠️ ERROR from {}: {}", message.from.name, error.message);
                            let user_id = crate::messaging::AgentId::user("user".to_string());
                            let response = crate::messaging::Message::new(
                                admin_id.clone(),
                                user_id,
                                crate::messaging::MessageContent::Response(user_msg)
                            );
                            
                            if let Err(e) = message_bus.write().await.send_message(response).await {
                                tracing::error!("Failed to forward error to user: {:?}", e);
                            }
                        },
                        _ => {}
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
        let ai_provider_manager = Arc::new(AIProviderManager::new(None, "standalone".to_string()).await.unwrap());
        
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
        let ai_provider_manager = Arc::new(AIProviderManager::new(None, "standalone".to_string()).await.unwrap());
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
