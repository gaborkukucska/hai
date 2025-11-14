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
    pub async fn new() -> Result<Self> {
        log::info!("Initializing Admin AI Bridge...");
        
        // Determine prompts path - try multiple strategies
        let prompts_path = if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            // Running via cargo run - use project structure
            let project_root = std::path::PathBuf::from(&manifest_dir)
                .parent()
                .and_then(|p| p.parent())
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_path_buf();
            project_root.join("hainet-persona").join("prompts")
        } else {
            // Running as binary - use current directory
            std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("hainet-persona")
                .join("prompts")
        };
        
        log::info!("Prompts path: {:?}", prompts_path);
        
        // Create AIProviderManager first (needed by GuardianSystem)
        let ai_provider_manager = Arc::new(AIProviderManager::new().await?);
        
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
            let mut client = mcp_client.write().await;
            match client.start_default_servers().await {
                Ok(results) => {
                    log::info!("MCP servers initialized successfully");
                    // Log server initialization results
                    for (server_name, result) in &results {
                        match result {
                            Ok(_) => log::info!("MCP server '{}' started", server_name),
                            Err(e) => log::warn!("MCP server '{}' failed to start: {:?}", server_name, e),
                        }
                    }
                    // Log available servers for diagnostics
                    let servers = client.list_servers().await;
                    log::info!("Available MCP servers: {:?}", servers);
                },
                Err(e) => {
                    log::warn!("Failed to initialize MCP servers: {:?}", e);
                    log::warn!("Workers will have no tools available");
                }
            }
        }
        
        // Create project manager with SQLite database
        // Use a more reliable path in the user's home directory
        let home_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
        let hainet_dir = home_dir.join(".hainet");
        let data_dir = hainet_dir.join("data");
        
        // Create directories with proper permissions
        std::fs::create_dir_all(&data_dir)?;
        
        let db_path = data_dir.join("projects.db");
        log::info!("Database path: {:?}", db_path);
        
        // SQLite connection string format: sqlite://path/to/db?mode=rwc
        // mode=rwc means: read-write-create (create if doesn't exist)
        let db_connection_string = format!("sqlite://{}?mode=rwc", db_path.display());
        log::info!("Database connection string: {}", db_connection_string);
        
        let project_manager = Arc::new(RwLock::new(
            ProjectManager::new(&db_connection_string).await?
        ));
        
        // Create metrics collector with database path
        let metrics_db_path = data_dir.join("metrics.db");
        let metrics_collector = Arc::new(RwLock::new(
            MetricsCollector::new(&format!("sqlite://{}?mode=rwc", metrics_db_path.display())).await?
        ));
        
        // Create user settings manager with database path
        let settings_db_path = data_dir.join("user_settings.db");
        let user_settings = Arc::new(RwLock::new(
            hainet_persona::UserSettingsManager::new(&format!("sqlite://{}?mode=rwc", settings_db_path.display())).await?
        ));
        
        let context = Arc::new(AgentContext::new(
            message_bus,
            prompt_manager,
            mcp_client,
            guardian,
        ).with_user_settings(user_settings));
        
        // Create Admin AI agent (ai_provider_manager already created earlier)
        let mut admin = AdminAgent::new(context, project_manager, ai_provider_manager, metrics_collector).await?;
        
        // Start Admin AI
        admin.start().await?;
        
        // Create STT handler
        let stt_handler = Arc::new(STTHandler::new());
        
        log::info!("Admin AI Bridge initialized successfully");
        
        Ok(Self {
            admin: Arc::new(RwLock::new(admin)),
            message_history: Arc::new(RwLock::new(Vec::new())),
            stt_handler,
        })
    }
    
    /// Send message to Admin AI and get response
    pub async fn send_message(&self, content: String, attachments: Vec<FileAttachment>) -> Result<ChatResponse> {
        log::info!("Processing user message: {}", content);
        
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
                log::error!("Admin AI process_user_input failed: {:?}", e);
                log::error!("Error source chain: {:#}", e);
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
        log::info!("Transcribing audio: {} channels, {} Hz, {} format", 
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
}
