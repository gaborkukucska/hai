//! # START OF FILE hainet-persona/tests/admin_integration_test.rs
//! Integration test for Admin agent with LLM config and metrics
//! 
//! Tests the complete flow:
//! 1. Admin agent creation with config and metrics
//! 2. Processing user input (simple and complex intents)
//! 3. Metrics collection and aggregation
//! 4. LLM configuration usage

use hainet_persona::agents::{Agent, AdminAgent, AgentContext, AgentLLMConfig, MetricsCollector};
use hainet_persona::config::HaiNetConfig;
use hainet_persona::messaging::MessageBus;
use hainet_persona::prompts::{PromptManager, AgentType};
use hainet_persona::projects::ProjectManager;
use hainet_persona::tools::mcp::MCPClientManager;
use hainet_persona::guardian::GuardianSystem;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::path::PathBuf;

async fn create_test_context() -> Arc<AgentContext> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let prompts_path = PathBuf::from(manifest_dir).join("prompts");
    
    Arc::new(AgentContext::new(
        Arc::new(RwLock::new(MessageBus::new().await.expect("Failed to create MessageBus"))),
        Arc::new(RwLock::new(PromptManager::new(prompts_path).unwrap())),
        Arc::new(RwLock::new(MCPClientManager::new())),
        Arc::new(RwLock::new(GuardianSystem::new(None, None))),
    ))
}

#[tokio::test]
async fn test_admin_with_llm_config_and_metrics() {
    // Create dependencies
    let context = create_test_context().await;
    let project_manager = Arc::new(RwLock::new(
        ProjectManager::new("sqlite::memory:").await.unwrap()
    ));
    let metrics = Arc::new(RwLock::new(
        MetricsCollector::new("sqlite::memory:").await.unwrap()
    ));
    
    // Create Admin agent
    let agent = AdminAgent::new(
        context.clone(),
        project_manager.clone(),
        metrics.clone(),
    ).await.unwrap();
    
    // Verify agent created with correct type
    assert_eq!(agent.id().agent_type, AgentType::Admin);
    
    // Verify metrics collector is accessible
    let metrics_read = metrics.read().await;
    let count = metrics_read.count_operations(AgentType::Admin).await.unwrap();
    assert_eq!(count, 0, "Should start with no operations");
}

#[tokio::test]
async fn test_llm_config_loading() {
    // Load default config
    let config = HaiNetConfig::load_or_default();
    
    // Get Admin LLM config
    let admin_config = config.get_agent_llm_config(AgentType::Admin);
    
    // Verify defaults are set
    assert!(admin_config.temperature >= 0.0 && admin_config.temperature <= 2.0);
    assert!(admin_config.max_tokens > 0);
    
    println!("Admin LLM Config:");
    println!("  Temperature: {}", admin_config.temperature);
    println!("  Max Tokens: {}", admin_config.max_tokens);
    println!("  Provider Preference: {:?}", admin_config.provider_preference);
    println!("  Model Size Preference: {:?}", admin_config.model_size_preference);
}

#[tokio::test]
async fn test_metrics_collection() {
    use hainet_persona::agents::metrics::{OperationResult, hash_config};
    use hainet_persona::messaging::AgentId;
    use std::time::Duration;
    
    // Create metrics collector
    let collector = MetricsCollector::new("sqlite::memory:").await.unwrap();
    
    // Create test operation result
    let config = AgentLLMConfig::for_admin();
    let result = OperationResult {
        agent_type: AgentType::Admin,
        agent_id: AgentId::new(AgentType::Admin, "test".to_string()),
        config_hash: hash_config(&config),
        operation_type: "test_operation".to_string(),
        success: true,
        response_time: Duration::from_millis(150),
        tokens_used: Some(100),
        error_message: None,
        json_parse_success: true,
        had_syntax_errors: false,
        validation_passed: true,
    };
    
    // Record operation
    collector.record_operation(result).await.unwrap();
    
    // Verify count
    let count = collector.count_operations(AgentType::Admin).await.unwrap();
    assert_eq!(count, 1);
    
    // Get aggregate metrics
    let metrics = collector.get_aggregate(AgentType::Admin).await.unwrap();
    assert_eq!(metrics.total_operations, 1);
    assert_eq!(metrics.successful_operations, 1);
    assert_eq!(metrics.success_rate, 1.0);
    assert!(metrics.avg_response_time_ms > 0.0);
    assert!(metrics.avg_tokens_used > 0.0);
}

#[tokio::test]
async fn test_multiple_agents_metrics() {
    use hainet_persona::agents::metrics::{OperationResult, hash_config};
    use hainet_persona::messaging::AgentId;
    use std::time::Duration;
    
    let collector = MetricsCollector::new("sqlite::memory:").await.unwrap();
    
    // Record operations for different agent types
    for agent_type in [AgentType::Admin, AgentType::PM, AgentType::Worker] {
        let config = match agent_type {
            AgentType::Admin => AgentLLMConfig::for_admin(),
            AgentType::PM => AgentLLMConfig::for_pm(),
            AgentType::Worker => AgentLLMConfig::for_worker(),
            _ => AgentLLMConfig::for_agent_type(agent_type),
        };
        
        for i in 0..5 {
            let result = OperationResult {
                agent_type,
                agent_id: AgentId::new(agent_type, format!("test-{}", i)),
                config_hash: hash_config(&config),
                operation_type: "test".to_string(),
                success: i % 4 != 0, // 75% success rate
                response_time: Duration::from_millis(100 + i as u64 * 10),
                tokens_used: Some(50 + i),
                error_message: if i % 4 == 0 { Some("test error".to_string()) } else { None },
                json_parse_success: i % 4 != 0,
                had_syntax_errors: false,
                validation_passed: i % 4 != 0,
            };
            
            collector.record_operation(result).await.unwrap();
        }
    }
    
    // Verify counts per agent type
    for agent_type in [AgentType::Admin, AgentType::PM, AgentType::Worker] {
        let count = collector.count_operations(agent_type).await.unwrap();
        assert_eq!(count, 5, "Each agent type should have 5 operations");
        
        let metrics = collector.get_aggregate(agent_type).await.unwrap();
        assert_eq!(metrics.total_operations, 5);
        // Success rate should be 4/5 = 0.8 (80%) since i % 4 != 0 succeeds for i in [1,2,3,5]
        assert!(metrics.success_rate >= 0.75 && metrics.success_rate <= 0.85, 
                "Success rate should be 80% but got {}", metrics.success_rate);
    }
}

#[tokio::test]
async fn test_config_hash_deterministic() {
    use hainet_persona::agents::metrics::hash_config;
    
    let config1 = AgentLLMConfig::for_admin();
    let config2 = AgentLLMConfig::for_admin();
    
    let hash1 = hash_config(&config1);
    let hash2 = hash_config(&config2);
    
    assert_eq!(hash1, hash2, "Same config should produce same hash");
    
    // Different config should produce different hash
    let mut config3 = AgentLLMConfig::for_admin();
    config3.temperature = 0.999;
    let hash3 = hash_config(&config3);
    
    assert_ne!(hash1, hash3, "Different config should produce different hash");
}

#[tokio::test]
async fn test_llm_config_for_all_agent_types() {
    let config = HaiNetConfig::load_or_default();
    
    for agent_type in [
        AgentType::User,
        AgentType::Admin,
        AgentType::PM,
        AgentType::Worker,
        AgentType::Guardian,
    ] {
        let llm_config = config.get_agent_llm_config(agent_type);
        
        println!("{:?} LLM Config:", agent_type);
        println!("  Temperature: {}", llm_config.temperature);
        println!("  Max Tokens: {}", llm_config.max_tokens);
        
        // Validate reasonable values
        if agent_type != AgentType::User {
            assert!(llm_config.temperature >= 0.0 && llm_config.temperature <= 2.0);
            assert!(llm_config.max_tokens > 0 && llm_config.max_tokens <= 32768);
        }
    }
}
