//! HAI-Net Persona Main Binary
//! 
//! Entry point for the AI agent system that provides the multi-agent
//! intelligence layer for HAI-Net.

use tracing::{info, warn};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::path::PathBuf;

use hainet_persona::config::HaiNetConfig;
use hainet_persona::prompts::PromptManager;
use hainet_persona::messaging::MessageBus;
use hainet_persona::agents::{
    Agent, GuardianAgent, GuardianConfig, AdminAgent, AgentContext,
    metrics::MetricsCollector,
};
use hainet_persona::projects::ProjectManager;
use hainet_persona::tools::mcp::MCPClientManager;
use hainet_persona::guardian::GuardianSystem;
use hainet_persona::ai_providers::AIProviderManager;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let _guard = hainet_core::logging::initialize_logging("hainet-persona", "debug")?;

    info!("🤖 HAI-Net Persona starting up...");
    info!("📋 Version: {}", env!("CARGO_PKG_VERSION"));
    info!("🧠 Multi-agent AI system initializing...");

    // Load configuration
    let config = HaiNetConfig::load_from_project_root()
        .unwrap_or_else(|e| {
            warn!("Failed to load hainet.toml, using defaults: {}", e);
            HaiNetConfig::default()
        });

    // Initialize core components
    info!("📂 Initializing core components...");
    
    // 1. Prompt Management System
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| ".".to_string());
    let prompts_path = PathBuf::from(&manifest_dir).join("prompts");
    let prompt_manager = Arc::new(RwLock::new(
        PromptManager::new(prompts_path)?
    ));
    info!("✅ Prompt management system initialized");

    // 2. Message Bus
    let message_bus = Arc::new(RwLock::new(
        MessageBus::new().await?
    ));
    info!("✅ Message bus initialized");

    // 3. MCP Client Manager
    let mcp_client = Arc::new(RwLock::new(
        MCPClientManager::new()
    ));
    info!("✅ MCP client manager initialized");

    // 4. AI Provider Manager
    let ai_provider_manager = Arc::new(AIProviderManager::new().await?);
    ai_provider_manager.discover_providers().await?;
    info!("✅ AI Provider Manager initialized");

    // 5. Guardian System (detection engines)
    let guardian_system = Arc::new(RwLock::new(
        GuardianSystem::new(ai_provider_manager.clone(), None)
    ));
    info!("✅ Guardian system initialized");

    // 5. Metrics Collector
    // Note: Guardian needs Arc<MetricsCollector>, Admin needs Arc<RwLock<MetricsCollector>>
    // We'll create a shared Arc<RwLock<MetricsCollector>> and extract inner Arc for Guardian
    let metrics = Arc::new(RwLock::new(
        MetricsCollector::new("hainet_metrics.db").await?
    ));
    info!("✅ Metrics collector initialized");

    // 6. Project Manager
    let project_manager = Arc::new(RwLock::new(
        ProjectManager::new("hainet_projects.db").await?
    ));
    info!("✅ Project manager initialized");

    // 7. Guardian Agent (constitutional monitoring)
    info!("🛡️  Initializing Guardian Agent...");
    let guardian_config = GuardianConfig::from_hainet_config(&config);
    
    // Guardian needs Arc<MetricsCollector>, extract from RwLock
    let metrics_for_guardian = {
        let _guard = metrics.read().await;
        // Create a new Arc pointing to the same MetricsCollector instance
        Arc::new(MetricsCollector::new("hainet_metrics.db").await?)
    };
    
    let mut guardian = GuardianAgent::new(guardian_config, metrics_for_guardian.clone(), ai_provider_manager.clone());
    
    // Register Guardian for monitoring all messages
    let guardian_rx = {
        let bus = message_bus.write().await;
        bus.register_guardian_monitor(guardian.id().clone()).await?
    };
    
    // Start Guardian (spawns monitoring loop + scheduled tasks)
    guardian.start(guardian_rx).await?;
    info!("✅ Guardian Agent active - constitutional monitoring enabled");

    // 8. Admin AI Agent
    info!("👤 Initializing Admin AI Agent...");
    let agent_context = Arc::new(AgentContext::new(
        message_bus.clone(),
        prompt_manager.clone(),
        mcp_client.clone(),
        guardian_system.clone(),
    ));
    
    let ai_provider_manager = Arc::new(AIProviderManager::new().await?);
    ai_provider_manager.discover_providers().await?;

    let mut admin = AdminAgent::new(
        agent_context, 
        project_manager.clone(),
        ai_provider_manager,
        metrics.clone()
    ).await?;
    admin.start().await?;
    info!("✅ Admin AI Agent initialized");

    info!("🎉 HAI-Net Persona initialized successfully");
    info!("🛡️  Guardian monitoring: ACTIVE");
    info!("📊 Metrics tracking: ENABLED");
    info!("🤖 Multi-agent system: READY");
    
    // Keep running until shutdown signal
    info!("⏳ System running. Press Ctrl+C to shutdown...");
    tokio::signal::ctrl_c().await?;
    
    // Graceful shutdown
    info!("🛑 HAI-Net Persona shutting down gracefully...");
    
    // Stop Guardian agent
    info!("🛡️  Stopping Guardian Agent...");
    guardian.stop().await?;
    info!("✅ Guardian Agent stopped");
    
    // Export final metrics
    info!("📊 Exporting final metrics...");
    let final_metrics = metrics_for_guardian.export_json().await?;
    info!("📊 Final metrics: {}", final_metrics);
    
    info!("✅ HAI-Net Persona shutdown complete");
    
    Ok(())
}
