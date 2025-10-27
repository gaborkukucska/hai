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

use super::{Agent, AgentContext, IntentParser, TaskPlanner, AgentStateMachine};
use super::pm::PMAgent;
use crate::messaging::{AgentId, Message};
use crate::prompts::{AgentType, AgentState, PromptContext};
use crate::projects::{ProjectManager, ProjectId};
use crate::ai_providers::providers::{OllamaClient, ProviderClient, GenerationOptions};

/// Threshold for detecting complex intents that require projects
const COMPLEX_INTENT_THRESHOLD: f64 = 0.7;

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
    
    /// Ollama client for LLM-powered planning
    ollama_client: OllamaClient,
    
    /// Active projects being monitored
    active_projects: HashMap<ProjectId, AgentId>,
    
    /// Running flag
    running: bool,
}

impl AdminAgent {
    /// Create new Admin AI agent
    pub async fn new(
        context: Arc<AgentContext>,
        project_manager: Arc<RwLock<ProjectManager>>,
    ) -> Result<Self> {
        let id = AgentId::new(AgentType::Admin, "main-admin".to_string());
        
        Ok(Self {
            id,
            context,
            intent_parser: IntentParser::new(),
            task_planner: TaskPlanner::new(),
            state_machine: AgentStateMachine::new(),
            project_manager,
            ollama_client: OllamaClient::localhost(),
            active_projects: HashMap::new(),
            running: false,
        })
    }
    
    /// Process user input - main entry point for user interaction
    pub async fn process_user_input(&mut self, user_input: String) -> Result<String> {
        // Parse user intent
        let intent = self.intent_parser.parse(&user_input).await?;
        
        tracing::info!("Parsed intent: {:?} (confidence: {})", intent.intent_type, intent.confidence);
        
        // If still in Startup state, transition to Conversation first
        if *self.state_machine.current_state() == AgentState::Startup {
            tracing::warn!("Admin AI still in Startup state, transitioning to Conversation");
            self.state_machine.transition(
                AgentState::Conversation,
                "Auto-transition from Startup on first message".to_string()
            )?;
        }
        
        // Detect if this is a complex/multi-step intent
        if self.is_complex_intent(&intent, &user_input)? {
            // Transition to Planning state from any valid state
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
            
            // Generate project plan using LLM
            let project_plan = self.generate_project_plan(&user_input, &intent).await?;
            
            // Create project
            let project_id = self.create_project(
                project_plan.title.clone(),
                project_plan.overview.clone(),
                project_plan.initial_tasks.clone(),
            ).await?;
            
            // Spawn PM agent
            let pm_id = self.spawn_pm_agent(&project_id, &project_plan).await?;
            
            // Track project
            self.active_projects.insert(project_id.clone(), pm_id.clone());
            
            // Transition to Monitoring state
            self.state_machine.transition(
                AgentState::Monitoring,
                format!("Project {} created, PM agent {} spawned", project_id, pm_id.name)
            )?;
            
            Ok(format!(
                "I've created a project to handle your request:\n\n\
                 **{}**\n\n\
                 {}\n\n\
                 I'll work on this in the background and keep you updated on progress. \
                 Feel free to ask me anything else in the meantime!",
                project_plan.title,
                project_plan.overview
            ))
        } else {
            // Simple intent - handle directly in Conversation state
            if *self.state_machine.current_state() != AgentState::Conversation {
                self.state_machine.transition(
                    AgentState::Conversation,
                    "Simple intent, conversational response".to_string()
                )?;
            }
            
            self.handle_simple_intent(&user_input, &intent).await
        }
    }
    
    /// Detect if intent requires a project (complex/multi-step)
    fn is_complex_intent(&self, intent: &super::intent::Intent, user_input: &str) -> Result<bool> {
        // Project keywords
        let project_keywords = [
            "build", "create", "develop", "make", "implement",
            "design", "write", "generate", "setup", "configure",
            "install", "deploy", "construct", "architect"
        ];
        
        let input_lower = user_input.to_lowercase();
        let has_project_keyword = project_keywords.iter()
            .any(|kw| input_lower.contains(kw));
        
        // Multi-step indicators
        let multi_step_indicators = [
            "and", "then", "also", "plus", "with",
            "step", "phase", "stage", "first", "next"
        ];
        
        let has_multi_step = multi_step_indicators.iter()
            .any(|ind| input_lower.contains(ind));
        
        // Complex if has project keyword OR (high confidence AND multiple steps)
        Ok(has_project_keyword || (intent.confidence >= COMPLEX_INTENT_THRESHOLD && has_multi_step))
    }
    
    /// Generate project plan using LLM
    async fn generate_project_plan(
        &self,
        user_input: &str,
        intent: &super::intent::Intent,
    ) -> Result<ProjectPlan> {
        // Load planning prompt
        let mut prompt_manager = self.context.prompt_manager.write().await;
        let mut prompt_context = PromptContext::default();
        prompt_context.current_request = Some(user_input.to_string());
        prompt_context.task_analysis = Some(format!("{:?}", intent.intent_type));
        prompt_context.variables.insert("intent_type".to_string(), serde_json::json!(format!("{:?}", intent.intent_type)));
        prompt_context.variables.insert("entities".to_string(), serde_json::json!(format!("{:?}", intent.entities)));
        
        // Create prompts::types::AgentId from messaging::types::AgentId
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
        
        // Create planning prompt
        let planning_prompt = format!(
            "User Request: {}\n\n\
             Detected Intent: {:?}\n\
             Confidence: {:.2}\n\
             Entities: {:?}\n\n\
             Please create a detailed project plan.\n\n\
             CRITICAL: You MUST respond with ONLY valid JSON. No markdown, no explanations.\n\n\
             Required fields:\n\
             1. \"title\": Clear project title (max 60 chars)\n\
             2. \"overview\": Project overview (2-3 sentences)\n\
             3. \"tasks\": Array of 3-7 task descriptions as STRINGS ONLY\n\n\
             IMPORTANT: The \"tasks\" field MUST be a simple string array, NOT objects.\n\n\
             Example format:\n\
             {{\n  \
               \"title\": \"Todo App Development\",\n  \
               \"overview\": \"Create a modern todo application using React and TypeScript.\",\n  \
               \"tasks\": [\n    \
                 \"Set up React project with TypeScript\",\n    \
                 \"Design UI components for todo list\",\n    \
                 \"Implement CRUD operations\",\n    \
                 \"Add local storage persistence\",\n    \
                 \"Write unit tests\"\n  \
               ]\n\
             }}\n\n\
             Your response (JSON only, no other text):",
            user_input,
            intent.intent_type,
            intent.confidence,
            intent.entities
        );
        
        // Generate with Ollama
        let options = GenerationOptions {
            temperature: Some(0.7),
            max_tokens: Some(1024),
            system: Some(system_prompt),
            ..Default::default()
        };
        
        let response = self.ollama_client.generate(
            "llama3.2:latest", // Default model
            &planning_prompt,
            options
        ).await.context("Failed to generate project plan with LLM")?;
        
        // Parse JSON response
        let plan = self.parse_project_plan(&response.text)?;
        
        Ok(plan)
    }
    
    /// Parse LLM response into ProjectPlan
    fn parse_project_plan(&self, llm_response: &str) -> Result<ProjectPlan> {
        // Log the raw response for debugging
        tracing::debug!("LLM response for project plan: {}", llm_response);
        
        // Try to extract JSON from response (LLM might wrap it in markdown)
        let json_str = if let Some(start) = llm_response.find('{') {
            if let Some(end) = llm_response.rfind('}') {
                &llm_response[start..=end]
            } else {
                llm_response
            }
        } else {
            llm_response
        };
        
        tracing::debug!("Extracted JSON string: {}", json_str);
        
        // Try parsing with better error handling
        let parsed: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(val) => val,
            Err(e) => {
                // Log detailed parse error
                tracing::error!("JSON parse error: {:?}", e);
                tracing::error!("Failed at position: {}", e.line());
                tracing::error!("JSON string bytes: {:?}", json_str.as_bytes());
                
                // Try to repair common issues
                let mut repaired = json_str
                    .replace("\n", " ")           // Remove newlines
                    .replace("\r", "")            // Remove carriage returns
                    .trim()                        // Remove leading/trailing whitespace
                    .to_string();
                
                // Check if JSON is missing closing brace (common LLM error)
                let open_braces = repaired.chars().filter(|c| *c == '{').count();
                let close_braces = repaired.chars().filter(|c| *c == '}').count();
                
                if open_braces > close_braces {
                    tracing::warn!("JSON has {} open braces but only {} close braces, adding missing close braces", 
                                   open_braces, close_braces);
                    for _ in 0..(open_braces - close_braces) {
                        repaired.push('}');
                    }
                }
                
                tracing::debug!("Attempting to parse repaired JSON: {}", repaired);
                
                serde_json::from_str(&repaired)
                    .context(format!("Failed to parse LLM response as JSON after repair. Original error: {:?}. Response was: {}", e, json_str))?
            }
        };
        
        let title = parsed["title"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'title' in plan. Parsed JSON: {:?}", parsed))?
            .to_string();
        
        let overview = parsed["overview"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'overview' in plan. Parsed JSON: {:?}", parsed))?
            .to_string();
        
        // Parse tasks - handle both string array and object array formats
        let tasks: Vec<String> = parsed["tasks"].as_array()
            .ok_or_else(|| anyhow::anyhow!("Missing 'tasks' array in plan. Parsed JSON: {:?}", parsed))?
            .iter()
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
        
        tracing::info!("Successfully parsed project plan: title='{}', {} tasks", title, tasks.len());
        
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
        plan: &ProjectPlan,
    ) -> Result<AgentId> {
        // Create PM agent
        let mut pm_agent = PMAgent::new(
            project_id.clone(),
            self.context.message_bus.clone(),
            self.context.prompt_manager.clone(),
            self.project_manager.clone(),
        );
        
        // Assign PM to project
        {
            let project_manager = self.project_manager.write().await;
            project_manager.assign_pm(project_id, pm_agent.id().clone()).await?;
        }
        
        // Start PM agent (transitions to Planning → Managing)
        pm_agent.start().await?;
        
        // In a real implementation, PM agent would run in a separate task
        // For now, we'll store the PM agent reference (simplified)
        let pm_id = pm_agent.id().clone();
        
        tracing::info!("Spawned PM agent {} for project {}", pm_id.name, project_id);
        
        // TODO: Store PM agent instance for lifecycle management
        // This would involve spawning a tokio task to run pm_agent.manage_loop()
        
        Ok(pm_id)
    }
    
    /// Handle simple intent conversationally
    async fn handle_simple_intent(
        &self,
        user_input: &str,
        intent: &super::intent::Intent,
    ) -> Result<String> {
        // Load conversation prompt
        let mut prompt_manager = self.context.prompt_manager.write().await;
        let mut prompt_context = PromptContext::default();
        prompt_context.current_request = Some(user_input.to_string());
        
        // Create prompts::types::AgentId from messaging::types::AgentId
        let prompt_agent_id = crate::prompts::types::AgentId::new(
            self.id.agent_type,
            self.id.name.clone()
        );
        
        let system_prompt = prompt_manager.get_prompt(
            &prompt_agent_id,
            AgentState::Conversation,
            &prompt_context
        ).await?;
        
        drop(prompt_manager);
        
        // Generate conversational response
        let options = GenerationOptions {
            temperature: Some(0.8),
            max_tokens: Some(512),
            system: Some(system_prompt),
            ..Default::default()
        };
        
        let response = self.ollama_client.generate(
            "llama3.2:latest",
            user_input,
            options
        ).await.context("Failed to generate conversational response")?;
        
        Ok(response.text)
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
    fn id(&self) -> &AgentId {
        &self.id
    }
    
    async fn process_message(&mut self, message: Message) -> Result<()> {
        // Process incoming messages from PM agents
        tracing::debug!("Admin received message from {}: {:?}", message.from.name, message.content);
        
        // TODO: Handle project status updates, completion notifications, etc.
        
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
        
        Arc::new(AgentContext::new(
            Arc::new(RwLock::new(MessageBus::new().await.expect("Failed to create MessageBus"))),
            Arc::new(RwLock::new(PromptManager::new(prompts_path).unwrap())),
            Arc::new(RwLock::new(MCPClientManager::new())),
            Arc::new(RwLock::new(GuardianSystem::new(None, None))),
        ))
    }
    
    async fn create_test_admin() -> AdminAgent {
        let context = create_test_context().await;
        let project_manager = Arc::new(RwLock::new(
            ProjectManager::new("sqlite::memory:").await.unwrap()
        ));
        
        AdminAgent::new(context, project_manager).await.unwrap()
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
        // Conversation -> Planning is valid
        agent.state_machine.transition(
            AgentState::Planning,
            "Testing".to_string()
        ).unwrap();
        assert_eq!(agent.state(), &AgentState::Planning);
        
        // Planning -> Conversation is valid (going back)
        agent.state_machine.transition(
            AgentState::Conversation,
            "Testing".to_string()
        ).unwrap();
        assert_eq!(agent.state(), &AgentState::Conversation);
    }
}
