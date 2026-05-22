//! # START OF FILE hainet-portal/src-tauri/src/admin_bridge.rs
//! # Admin AI Bridge
//! 
//! Bridge between Tauri frontend and hainet-persona Admin AI agent.
//! Manages Admin AI lifecycle and provides IPC interface for chat.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

use hainet_persona::agents::{AdminAgent, Agent, AgentContext, MetricsCollector};
use hainet_persona::messaging::MessageBus;
use hainet_persona::prompts::PromptManager;
use hainet_persona::tools::mcp::MCPClientManager;
use hainet_persona::guardian::GuardianSystem;
use hainet_persona::projects::ProjectManager;
use hainet_persona::ai_providers::AIProviderManager;

use crate::stt_handler::{STTHandler, AudioData, TranscriptionResult};

/// Message from user to Admin AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Message ID
    pub id: String,
    /// Message content
    pub content: String,
    /// Sender ("user" or "assistant")
    pub role: String,
    /// Timestamp
    pub timestamp: i64,
    /// Optional file attachments
    #[serde(default)]
    pub attachments: Vec<FileAttachment>,
    /// Optional video source
    #[serde(default)]
    pub video_src: Option<String>,
    /// Optional dynamic UI component
    #[serde(default)]
    pub dynamic_component: Option<Value>,
}

/// File attachment metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAttachment {
    /// File name
    pub name: String,
    /// File path
    pub path: String,
    /// File size in bytes
    pub size: u64,
    /// MIME type
    pub mime_type: String,
}

/// Response from Admin AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Response message
    pub message: ChatMessage,
    /// Current agent state
    pub agent_state: String,
    /// Active project count
    pub active_projects: usize,
}

/// Admin AI Bridge managing agent lifecycle
pub struct AdminBridge {
    /// Admin AI agent
    admin: Arc<RwLock<AdminAgent>>,
    /// Message history
    message_history: Arc<RwLock<Vec<ChatMessage>>>,
    /// STT handler
    stt_handler: Arc<STTHandler>,
}

impl AdminBridge {
    /// Create new Admin AI bridge
    pub async fn new(data_dir: std::path::PathBuf, prompts_path: std::path::PathBuf) -> Result<Self> {
        info!("Initializing Admin AI Bridge...");
        
        info!("Prompts path: {:?}", prompts_path);
        
        // Create project manager with SQLite database
        // Use the centralized data_dir provided by the configuration
        info!("Data directory: {:?}", data_dir);
        
        // Create directories with proper permissions
        std::fs::create_dir_all(&data_dir)?;

        
        // Create user settings manager FIRST (needed by AIProviderManager)
        let settings_db_path = data_dir.join("user_settings.db");
        let user_settings = Arc::new(RwLock::new(
            hainet_persona::UserSettingsManager::new(&format!("sqlite://{}?mode=rwc", settings_db_path.display())).await?
        ));
        
        // Create AIProviderManager with user settings (needed by GuardianSystem)
        let ai_provider_manager = Arc::new(AIProviderManager::new(Some(user_settings.clone())).await?);
        
        // Create shared context
        let message_bus = Arc::new(RwLock::new(MessageBus::new().await?));
        let prompt_manager = Arc::new(RwLock::new(PromptManager::new(prompts_path)?));
        let mcp_client = Arc::new(RwLock::new(MCPClientManager::new()));
        let guardian = Arc::new(RwLock::new(GuardianSystem::new(
            ai_provider_manager.clone(),
            None
        )));
        
        // Initialize MCP servers from default config before creating agents
        {
            let client = mcp_client.write().await;
            match client.start_default_servers().await {
                Ok(results) => {
                    info!("MCP servers initialized successfully");
                    // Log server initialization results
                    for (server_name, result) in &results {
                        match result {
                            Ok(_) => info!("MCP server '{}' started", server_name),
                            Err(e) => warn!("MCP server '{}' failed to start: {:?}", server_name, e),
                        }
                    }
                    // Log available servers for diagnostics
                    let servers = client.list_servers().await;
                    info!("Available MCP servers: {:?}", servers);
                },
                Err(e) => {
                    warn!("Failed to initialize MCP servers: {:?}", e);
                    warn!("Workers will have no tools available");
                }
            }
        }
        
        let db_path = data_dir.join("projects.db");
        info!("Database path: {:?}", db_path);
        
        // SQLite connection string format: sqlite://path/to/db?mode=rwc
        // mode=rwc means: read-write-create (create if doesn't exist)
        let db_connection_string = format!("sqlite://{}?mode=rwc", db_path.display());
        info!("Database connection string: {}", db_connection_string);
        
        let project_manager = Arc::new(RwLock::new(
            ProjectManager::new(&db_connection_string).await?
        ));
        
        // Create metrics collector with database path
        let metrics_db_path = data_dir.join("metrics.db");
        let metrics_collector = Arc::new(RwLock::new(
            MetricsCollector::new(&format!("sqlite://{}?mode=rwc", metrics_db_path.display())).await?
        ));
        
        // Create memory and profile DB paths
        let memory_db_path = data_dir.join("memory.db");
        let profile_db_path = data_dir.join("profile.db");
        
        let memory_db_url = format!("sqlite://{}?mode=rwc", memory_db_path.display());
        let profile_db_url = format!("sqlite://{}?mode=rwc", profile_db_path.display());
        
        let context = Arc::new(AgentContext::new(
            message_bus,
            prompt_manager,
            mcp_client,
            guardian,
        ).with_user_settings(user_settings));
        
        // Create Admin AI agent (ai_provider_manager already created earlier)
        let mut admin = AdminAgent::new(
            context.clone(), 
            project_manager, 
            ai_provider_manager, 
            metrics_collector,
            memory_db_url,
            profile_db_url
        ).await?;
        
        // Start Admin AI
        admin.start().await?;
        
        // Create STT handler
        let stt_handler = Arc::new(STTHandler::new());
        
        info!("Admin AI Bridge initialized successfully");
        
        let bridge = Self {
            admin: Arc::new(RwLock::new(admin)),
            message_history: Arc::new(RwLock::new(Vec::new())),
            stt_handler,
        };

        // Spawn listener for Admin agent messages (e.g. error escalations)
        let admin_clone = bridge.admin.clone();
        // We need to take the receiver out of the admin agent to use it in the loop
        let receiver_opt = {
            let mut admin_write = admin_clone.write().await;
            admin_write.take_receiver()
        };
        
        if let Some(mut receiver) = receiver_opt {
            tokio::spawn(async move {
                info!("Admin agent message listener started");
                while let Some(msg) = receiver.recv().await {
                    let mut admin = admin_clone.write().await;
                    if let Err(e) = admin.handle_message(msg).await {
                        error!("Error handling message in Admin agent: {:?}", e);
                    }
                }
                warn!("Admin agent message listener ended");
            });
        } else {
            warn!("Could not take receiver from Admin agent - message listener not started");
        }

        // Register User agent to receive notifications
        let user_id = hainet_persona::messaging::AgentId::user("user".to_string());
        let (mut user_rx, _) = context.message_bus.write().await.register_agent(user_id.clone()).await?;
        
        info!("Registered User agent: {:?}", user_id);
        
        // Spawn listener for User agent messages
        let history_clone = bridge.message_history.clone();
        tokio::spawn(async move {
            info!("User agent listener started");
            while let Some(msg) = user_rx.recv().await {
                info!("User agent received message from {:?}: {:?}", msg.from, msg.content);
                
                if let hainet_persona::messaging::MessageContent::Response(text) = msg.content {
                    let chat_msg = ChatMessage {
                        id: uuid::Uuid::new_v4().to_string(),
                        content: text,
                        role: "assistant".to_string(), // Display as assistant message
                        timestamp: chrono::Utc::now().timestamp(),
                        attachments: vec![],
                        video_src: None,
                        dynamic_component: None,
                    };
                    
                    history_clone.write().await.push(chat_msg);
                }
            }
            warn!("User agent listener ended");
        });
        
        Ok(bridge)
    }
    
    /// Send message to Admin AI and get response
    pub async fn send_message(&self, content: String, attachments: Vec<FileAttachment>) -> Result<ChatResponse> {
        info!("Processing user message: {}", content);
        
        // Create user message
        let user_message = ChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            content: content.clone(),
            role: "user".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            attachments: attachments.clone(),
            video_src: None,
            dynamic_component: None,
        };
        
        // Store in history
        {
            let mut history = self.message_history.write().await;
            history.push(user_message.clone());
        }
        
        // Process with Admin AI
        let mut admin = self.admin.write().await;
        
        // Build input with attachment info if present
        let input = if attachments.is_empty() {
            content
        } else {
            let attachment_info = attachments.iter()
                .map(|a| format!("- {} ({} bytes)", a.name, a.size))
                .collect::<Vec<_>>()
                .join("\n");
            format!("{}\n\nAttached files:\n{}", content, attachment_info)
        };
        
        let response_text = match admin.process_user_input(input).await {
            Ok(text) => text,
            Err(e) => {
                error!("Admin AI process_user_input failed: {:?}", e);
                error!("Error source chain: {:#}", e);
                return Err(e);
            }
        };
        
        // Get agent state
        let state = format!("{:?}", admin.state());
        let project_count = admin.active_project_count();
        
        drop(admin);
        
        let dynamic_component_json = serde_json::json!({
            "type": "Stack",
            "children": [
                {
                    "type": "Text",
                    "props": { "style": { "fontWeight": "bold" } },
                    "children": ["This is a dynamic component from the backend!"]
                },
                {
                    "type": "Button",
                    "props": { "style": { "backgroundColor": "#007bff", "color": "white", "border": "none", "padding": "10px", "borderRadius": "5px" } },
                    "children": ["Get Agent State"],
                    "action": {
                        "type": "invoke",
                        "payload": {
                            "command": "get_agent_state"
                        }
                    }
                }
            ]
        });

        // Create assistant message
        let assistant_message = ChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            content: response_text,
            role: "assistant".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            attachments: vec![],
            video_src: None,
            dynamic_component: Some(dynamic_component_json),
        };
        
        // Store in history
        {
            let mut history = self.message_history.write().await;
            history.push(assistant_message.clone());
        }
        
        Ok(ChatResponse {
            message: assistant_message,
            agent_state: state,
            active_projects: project_count,
        })
    }
    
    /// Get message history
    pub async fn get_history(&self) -> Result<Vec<ChatMessage>> {
        let history = self.message_history.read().await;
        Ok(history.clone())
    }
    
    /// Clear message history
    pub async fn clear_history(&self) -> Result<()> {
        let mut history = self.message_history.write().await;
        history.clear();
        Ok(())
    }
    
    /// Get current agent state
    pub async fn get_state(&self) -> Result<String> {
        let admin = self.admin.read().await;
        Ok(format!("{:?}", admin.state()))
    }
    
    /// Transcribe audio via Admin AI
    /// 
    /// Flow: Portal audio → STT Handler → Admin AI (TODO: provider discovery) → Portal
    pub async fn transcribe_audio(&self, audio: AudioData) -> Result<TranscriptionResult> {
        info!("Transcribing audio: {} channels, {} Hz, {} format", 
                   audio.channels, audio.sample_rate, audio.format);
        
        // TODO: This currently returns a placeholder error
        // Full implementation requires:
        // 1. Admin AI to detect STT intent
        // 2. Admin AI to spawn/reuse STT Worker
        // 3. Worker to discover STT provider (via ai_providers)
        // 4. Worker to call MCP hainet-stt tool
        // 5. Result flows back to Portal
        
        self.stt_handler.transcribe(audio).await
    }
    /// Get list of active agents
    pub async fn get_active_agents(&self) -> Result<Vec<hainet_persona::messaging::AgentInfo>> {
        let admin = self.admin.read().await;
        let context = admin.context();
        let bus = context.message_bus.read().await;
        Ok(bus.get_active_agents().await)
    }

    pub async fn get_active_projects(&self) -> Result<Vec<hainet_persona::projects::ProjectInfo>> {
        let admin = self.admin.read().await;
        let project_manager = admin.project_manager().read().await;
        Ok(project_manager.get_active_projects_with_tasks().await?)
    }

    // ========== Project Management ==========

    /// Pause a project
    pub async fn pause_project(&self, project_id: String) -> Result<()> {
        let admin = self.admin.read().await;
        let project_manager = admin.project_manager().read().await;
        let pid = hainet_persona::projects::ProjectId::from_string(&project_id)?;
        project_manager.pause_project(&pid).await?;
        Ok(())
    }

    /// Resume a paused project
    pub async fn resume_project(&self, project_id: String) -> Result<()> {
        let admin = self.admin.read().await;
        let project_manager = admin.project_manager().read().await;
        let pid = hainet_persona::projects::ProjectId::from_string(&project_id)?;
        project_manager.resume_project(&pid).await?;
        Ok(())
    }

    /// Stop/cancel a project
    pub async fn stop_project(&self, project_id: String) -> Result<()> {
        let admin = self.admin.read().await;
        let project_manager = admin.project_manager().read().await;
        let pid = hainet_persona::projects::ProjectId::from_string(&project_id)?;
        project_manager.stop_project(&pid).await?;
        Ok(())
    }

    /// Rename a project
    pub async fn rename_project(&self, project_id: String, new_title: String) -> Result<()> {
        let admin = self.admin.read().await;
        let project_manager = admin.project_manager().read().await;
        let pid = hainet_persona::projects::ProjectId::from_string(&project_id)?;
        project_manager.rename_project(&pid, new_title).await?;
        Ok(())
    }

    /// Delete a project
    pub async fn delete_project(&self, project_id: String) -> Result<()> {
        let admin = self.admin.read().await;
        let project_manager = admin.project_manager().read().await;
        let pid = hainet_persona::projects::ProjectId::from_string(&project_id)?;
        project_manager.delete_project(&pid).await?;
        Ok(())
    }

    /// Export a project to a tar.gz file
    pub async fn export_project(&self, project_id: String, export_path: String) -> Result<hainet_persona::projects::ExportMetadata> {
        let admin = self.admin.read().await;
        let project_manager = admin.project_manager().read().await;
        let pid = hainet_persona::projects::ProjectId::from_string(&project_id)?;
        
        let metadata = project_manager.export_project(&pid, std::path::Path::new(&export_path)).await?;
        Ok(metadata)
    }

    /// Import a project from a tar.gz file
    pub async fn import_project(&self, import_path: String) -> Result<hainet_persona::projects::ImportResult> {
        let admin = self.admin.read().await;
        let project_manager = admin.project_manager().read().await;
        
        let result = project_manager.import_project(std::path::Path::new(&import_path)).await?;
        Ok(result)
    }
}

