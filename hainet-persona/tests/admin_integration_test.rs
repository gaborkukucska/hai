//! # START OF FILE hainet-persona/tests/admin_integration_test.rs

use anyhow::Result;
use hainet_persona::agents::AdminAgent;
use hainet_persona::projects::ProjectManager;
use hainet_persona::messaging::MessageBus;
use hainet_persona::prompts::PromptManager;
use hainet_persona::tools::mcp::MCPClientManager;
use hainet_persona::guardian::GuardianSystem;
use hainet_persona::agents::AgentContext;
use hainet_persona::ai_providers::AIProviderManager;
use hainet_persona::agents::metrics::MetricsCollector;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::path::PathBuf;

async fn create_test_admin_agent() -> Result<AdminAgent> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let prompts_path = PathBuf::from(manifest_dir).join("prompts");

    let ai_provider_manager = Arc::new(AIProviderManager::new().await?);

    let context = Arc::new(AgentContext::new(
        Arc::new(RwLock::new(MessageBus::new().await?)),
        Arc::new(RwLock::new(PromptManager::new(prompts_path)?)),
        Arc::new(RwLock::new(MCPClientManager::new())),
        Arc::new(RwLock::new(GuardianSystem::new(ai_provider_manager.clone(), None))),
    ));

    let project_manager = Arc::new(RwLock::new(
        ProjectManager::new("sqlite::memory:").await?,
    ));

    let ai_provider_manager = Arc::new(AIProviderManager::new().await?);

    // Perform an initial discovery to populate the catalog
    ai_provider_manager.discover_providers().await?;

    let metrics = Arc::new(RwLock::new(
        MetricsCollector::new("sqlite::memory:").await?,
    ));

    Ok(AdminAgent::new(context, project_manager, ai_provider_manager, metrics).await?)
}

#[tokio::test]
async fn test_e2e_complex_intent_project_creation() -> Result<()> {
    let mut admin_agent = create_test_admin_agent().await?;

    // Check if Ollama is running, otherwise skip the test
    let provider_stats = admin_agent.ai_provider_manager.get_stats().await;
    if provider_stats.total_models == 0 {
        println!("Skipping test_e2e_complex_intent_project_creation: No AI providers found. Is Ollama running?");
        return Ok(());
    }

    let user_input = "Create a new website for my portfolio.".to_string();

    let response = admin_agent.process_user_input(user_input).await?;

    assert!(response.contains("I've created a project to handle your request"));

    // Verify that a project was created
    let active_projects = admin_agent.active_project_count();
    assert_eq!(active_projects, 1, "Expected one active project to be created");

    // Further verification could involve querying the project manager, but this confirms the main flow.

    Ok(())
}
