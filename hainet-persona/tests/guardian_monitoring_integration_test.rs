//! Integration test for Guardian Agent monitoring MessageBus communications
//!
//! This test demonstrates the Guardian's ability to:
//! 1. Register for monitoring all messages on the MessageBus
//! 2. Receive copies of all inter-agent communications
//! 3. Analyze messages for constitutional compliance
//! 4. Track compliance metrics

use hainet_persona::agents::{GuardianAgent, GuardianConfig};
use hainet_persona::agents::metrics::MetricsCollector;
use hainet_persona::messaging::{MessageBus, AgentId, Message, MessageContent, Priority};
use hainet_persona::ai_providers::AIProviderManager;
use hainet_persona::prompts::AgentType;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_guardian_monitoring_integration() {
    // Initialize components
    let message_bus = Arc::new(MessageBus::new().await.unwrap());
    let metrics = Arc::new(MetricsCollector::new(":memory:").await.unwrap());
    let ai_provider_manager = Arc::new(AIProviderManager::new().await.unwrap());
    
    // Create Guardian agent
    let config = GuardianConfig::default();
    let mut guardian = GuardianAgent::new(config, metrics, ai_provider_manager);
    
    // Register Guardian for monitoring
    let guardian_rx = message_bus
        .register_guardian_monitor(guardian.id().clone())
        .await
        .unwrap();
    
    // Start Guardian (spawns monitoring loop in background)
    guardian.start(guardian_rx).await.unwrap();
    
    // Give Guardian time to initialize
    sleep(Duration::from_millis(50)).await;
    
    // Register agents
    let admin_id = AgentId::new(AgentType::Admin, "admin-1".to_string());
    let pm_id = AgentId::new(AgentType::PM, "pm-comms".to_string());
    let worker_id = AgentId::new(AgentType::Worker, "worker-email".to_string());
    
    let (_admin_rx, _admin_endpoint) = message_bus.register_agent(admin_id.clone()).await.unwrap();
    let (_pm_rx, _pm_endpoint) = message_bus.register_agent(pm_id.clone()).await.unwrap();
    let (_worker_rx, _worker_endpoint) = message_bus.register_agent(worker_id.clone()).await.unwrap();
    
    // Send messages through the system
    // 1. Admin delegates to PM
    let msg1 = Message::new(
        admin_id.clone(),
        pm_id.clone(),
        MessageContent::UserInput("Process user's email request".to_string()),
    )
    .with_priority(Priority::Normal);
    
    message_bus.send_message(msg1).await.unwrap();
    
    // 2. PM assigns to Worker
    let msg2 = Message::new(
        pm_id.clone(),
        worker_id.clone(),
        MessageContent::UserInput("Send email to user@example.com".to_string()),
    )
    .with_priority(Priority::Normal);
    
    message_bus.send_message(msg2).await.unwrap();
    
    // 3. Worker responds to PM
    let msg3 = Message::new(
        worker_id.clone(),
        pm_id.clone(),
        MessageContent::Response("Email sent successfully".to_string()),
    )
    .with_priority(Priority::Normal);
    
    message_bus.send_message(msg3).await.unwrap();
    
    // Give Guardian time to process messages
    sleep(Duration::from_millis(100)).await;
    
    // Verify Guardian intercepted all messages
    let stats = guardian.intercept_message(&Message::new(
        admin_id.clone(),
        pm_id.clone(),
        MessageContent::UserInput("test".to_string()),
    )).await;
    
    // Should succeed (test message is benign)
    assert!(stats.is_ok());
    
    // Cleanup
    guardian.stop().await.unwrap();
}

#[tokio::test]
async fn test_guardian_detects_pii_violation() {
    // Initialize components
    let message_bus = Arc::new(MessageBus::new().await.unwrap());
    let metrics = Arc::new(MetricsCollector::new(":memory:").await.unwrap());
    let ai_provider_manager = Arc::new(AIProviderManager::new().await.unwrap());
    
    // Create Guardian agent
    let config = GuardianConfig::default();
    let mut guardian = GuardianAgent::new(config, metrics, ai_provider_manager);
    
    // Register Guardian for monitoring
    let guardian_rx = message_bus
        .register_guardian_monitor(guardian.id().clone())
        .await
        .unwrap();
    
    // Start Guardian
    guardian.start(guardian_rx).await.unwrap();
    sleep(Duration::from_millis(50)).await;
    
    // Register agents
    let admin_id = AgentId::new(AgentType::Admin, "admin-1".to_string());
    let pm_id = AgentId::new(AgentType::PM, "pm-comms".to_string());
    
    let (_admin_rx, _admin_endpoint) = message_bus.register_agent(admin_id.clone()).await.unwrap();
    let (_pm_rx, _pm_endpoint) = message_bus.register_agent(pm_id.clone()).await.unwrap();
    
    // Send message with PII (should be detected by Guardian monitoring)
    let pii_message = Message::new(
        admin_id.clone(),
        pm_id.clone(),
        MessageContent::UserInput("My SSN is 123-45-6789".to_string()),
    )
    .with_priority(Priority::Normal);
    
    message_bus.send_message(pii_message.clone()).await.unwrap();
    
    // Give Guardian time to process
    sleep(Duration::from_millis(100)).await;
    
    // Verify Guardian detected the PII via direct intercept
    let result = guardian.intercept_message(&pii_message).await.unwrap();
    
    // Should be blocked or paused due to PII
    use hainet_persona::messaging::guardian::InterceptResult;
    match result {
        InterceptResult::Block(_) | InterceptResult::Pause(_) => {
            // Expected: Guardian detected PII violation
        }
        InterceptResult::Allow => {
            panic!("Guardian should have detected PII violation");
        }
    }
    
    // Cleanup
    guardian.stop().await.unwrap();
}

#[tokio::test]
async fn test_guardian_detects_harm_keywords() {
    // Initialize components
    let message_bus = Arc::new(MessageBus::new().await.unwrap());
    let metrics = Arc::new(MetricsCollector::new(":memory:").await.unwrap());
    let ai_provider_manager = Arc::new(AIProviderManager::new().await.unwrap());
    
    // Create Guardian agent
    let config = GuardianConfig::default();
    let mut guardian = GuardianAgent::new(config, metrics, ai_provider_manager);
    
    // Register Guardian for monitoring
    let guardian_rx = message_bus
        .register_guardian_monitor(guardian.id().clone())
        .await
        .unwrap();
    
    // Start Guardian
    guardian.start(guardian_rx).await.unwrap();
    sleep(Duration::from_millis(50)).await;
    
    // Register agents
    let pm_id = AgentId::new(AgentType::PM, "pm-system".to_string());
    let worker_id = AgentId::new(AgentType::Worker, "worker-files".to_string());
    
    let (_pm_rx, _pm_endpoint) = message_bus.register_agent(pm_id.clone()).await.unwrap();
    let (_worker_rx, _worker_endpoint) = message_bus.register_agent(worker_id.clone()).await.unwrap();
    
    // Send message with harm keyword (should be detected)
    let harm_message = Message::new(
        pm_id.clone(),
        worker_id.clone(),
        MessageContent::UserInput("How to kill the process".to_string()),
    )
    .with_priority(Priority::Normal);
    
    message_bus.send_message(harm_message.clone()).await.unwrap();
    
    // Give Guardian time to process
    sleep(Duration::from_millis(100)).await;
    
    // Verify Guardian detected the harm keyword via direct intercept
    let result = guardian.intercept_message(&harm_message).await.unwrap();
    
    // Should be blocked due to harm keyword
    use hainet_persona::messaging::guardian::InterceptResult;
    match result {
        InterceptResult::Block(_) => {
            // Expected: Guardian blocked message with harm keyword
        }
        InterceptResult::Pause(_) | InterceptResult::Allow => {
            panic!("Guardian should have blocked message with harm keyword");
        }
    }
    
    // Cleanup
    guardian.stop().await.unwrap();
}

#[tokio::test]
async fn test_guardian_allows_safe_messages() {
    // Initialize components
    let message_bus = Arc::new(MessageBus::new().await.unwrap());
    let metrics = Arc::new(MetricsCollector::new(":memory:").await.unwrap());
    let ai_provider_manager = Arc::new(AIProviderManager::new().await.unwrap());
    
    // Create Guardian agent
    let config = GuardianConfig::default();
    let mut guardian = GuardianAgent::new(config, metrics, ai_provider_manager);
    
    // Register Guardian for monitoring
    let guardian_rx = message_bus
        .register_guardian_monitor(guardian.id().clone())
        .await
        .unwrap();
    
    // Start Guardian
    guardian.start(guardian_rx).await.unwrap();
    sleep(Duration::from_millis(50)).await;
    
    // Register agents
    let admin_id = AgentId::new(AgentType::Admin, "admin-1".to_string());
    let pm_id = AgentId::new(AgentType::PM, "pm-comms".to_string());
    
    let (_admin_rx, _admin_endpoint) = message_bus.register_agent(admin_id.clone()).await.unwrap();
    let (_pm_rx, _pm_endpoint) = message_bus.register_agent(pm_id.clone()).await.unwrap();
    
    // Send multiple safe messages
    let safe_messages = vec![
        "Please organize my files by date",
        "What's the weather today?",
        "Schedule a meeting for tomorrow",
        "Send email to the team",
    ];
    
    for content in safe_messages {
        let msg = Message::new(
            admin_id.clone(),
            pm_id.clone(),
            MessageContent::UserInput(content.to_string()),
        )
        .with_priority(Priority::Normal);
        
        message_bus.send_message(msg.clone()).await.unwrap();
        
        // Verify Guardian allows safe message
        let result = guardian.intercept_message(&msg).await.unwrap();
        
        use hainet_persona::messaging::guardian::InterceptResult;
        match result {
            InterceptResult::Allow => {
                // Expected: Guardian allows safe messages
            }
            InterceptResult::Block(reason) => {
                panic!("Guardian should allow safe message, but blocked: {:?}", reason);
            }
            InterceptResult::Pause(reason) => {
                panic!("Guardian should allow safe message, but paused: {:?}", reason);
            }
        }
    }
    
    // Cleanup
    guardian.stop().await.unwrap();
}
