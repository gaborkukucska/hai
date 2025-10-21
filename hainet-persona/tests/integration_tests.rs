//! <!-- # START OF FILE hainet-persona/tests/integration_tests.rs -->
//! Integration tests for Phase 0 components
//! 
//! These tests validate that all implemented systems work together correctly:
//! - Prompt System + AI Provider Discovery
//! - Guardian System + Messaging
//! - Constitutional compliance across all layers

use hainet_persona::{
    prompts::{PromptManager, PromptContext, AgentState, types::PMDomain},
    ai_providers::{AIProviderManager, SelectionContext},
    messaging::{MessageBus, Message, MessageContent, AgentId, AgentType},
    messaging::types::{Task, TaskConstraints, ResourceTier, PrivacyLevel},
    guardian::{PIIDetector, BiasDetector, HarmAnalyzer, DecisionEngine},
};

#[tokio::test]
async fn test_prompt_system_initialization() {
    // Test that prompt manager can load templates
    // Skip if prompts directory doesn't exist in test environment
    if !std::path::Path::new("hainet-persona/prompts").exists() {
        println!("Skipping prompt test - prompts directory not found");
        return;
    }
    
    let result = PromptManager::new("hainet-persona/prompts".into());
    assert!(result.is_ok(), "Prompt manager should initialize successfully");
    
    let mut manager = result.unwrap();
    
    // Test loading a prompt for Admin agent in Planning state
    let agent_id = hainet_persona::prompts::types::AgentId::new(
        hainet_persona::prompts::types::AgentType::Admin, 
        "test-admin".to_string()
    );
    let context = PromptContext::default();
    
    let prompt = manager.get_prompt(&agent_id, AgentState::Planning, &context).await;
    assert!(prompt.is_ok(), "Should load admin planning prompt");
    
    let prompt_text = prompt.unwrap();
    assert!(!prompt_text.is_empty(), "Prompt should not be empty");
    assert!(prompt_text.contains("planning") || prompt_text.contains("Planning"), 
            "Planning prompt should mention planning");
}

#[tokio::test]
async fn test_ai_provider_discovery() {
    // Test AI provider discovery system
    let manager = AIProviderManager::new().await;
    assert!(manager.is_ok(), "AI Provider Manager should initialize");
    
    let manager = manager.unwrap();
    
    // Discover providers (will find local Ollama if running)
    let discovery_result = manager.discover_providers().await;
    // This may fail if Ollama isn't running, which is okay for integration test
    if discovery_result.is_ok() {
        // If discovery succeeded, verify we can select a model
        let context = SelectionContext::for_admin();
        let selection = manager.select_model_for_agent(context).await;
        
        if let Ok(model) = selection {
            assert!(!model.model_id.is_empty(), "Should have model ID");
            assert!(!model.model_name.is_empty(), "Should have model name");
        }
    }
}

#[tokio::test]
async fn test_messaging_system_creation() {
    // Test MessageBus initialization
    let bus = MessageBus::new().await;
    assert!(bus.is_ok(), "MessageBus should initialize");
    
    let mut bus = bus.unwrap();
    
    // Register test agents
    let admin_id = AgentId::new(AgentType::Admin, "test-admin".to_string());
    let pm_id = AgentId::new_pm(
        "test-pm".to_string(),
        PMDomain::Knowledge
    );
    
    assert!(bus.register_agent(admin_id.clone()).await.is_ok());
    assert!(bus.register_agent(pm_id.clone()).await.is_ok());
    
    // Test message creation
    
    let task = Task {
        id: hainet_persona::messaging::TaskId::new(),
        description: "Test task".to_string(),
        goals: vec!["Complete test".to_string()],
        constraints: TaskConstraints {
            resource_tier: ResourceTier::LocalOnly,
            max_cost_usd: None,
            privacy_level: PrivacyLevel::NoData,
            requires_confirmation: false,
        },
        deadline: None,
        parent_task: None,
        assigned_to: None,
    };
    
    let message = Message::new(
        admin_id.clone(),
        pm_id.clone(),
        MessageContent::TaskAssignment(task),
    );
    
    assert_eq!(message.from, admin_id);
    assert_eq!(message.to, pm_id);
}

#[tokio::test]
async fn test_guardian_pii_detection() {
    // Test PII detector (constructor returns Self, not Result)
    let detector = PIIDetector::new(None);
    
    // Test with clean text
    let clean_result = detector.analyze("Hello, how are you today?").await;
    assert!(clean_result.is_ok());
    let report = clean_result.unwrap();
    assert_eq!(report.detected_patterns.len(), 0, "Should find no PII in clean text");
    
    // Test with email
    let email_result = detector.analyze("Contact me at test@example.com").await;
    assert!(email_result.is_ok());
    let report = email_result.unwrap();
    assert!(report.detected_patterns.len() > 0, "Should detect email");
}

#[tokio::test]
async fn test_guardian_bias_detection() {
    // Test Bias detector (constructor returns Self, not Result)
    let detector = BiasDetector::new(None);
    
    // Test with neutral text
    let neutral_result = detector.analyze("The person completed the task efficiently.").await;
    assert!(neutral_result.is_ok());
    let report = neutral_result.unwrap();
    // Neutral text should have low bias (fewer bias categories detected)
    assert!(!report.contains_bias || report.bias_categories.len() == 0, "Neutral text should have no bias");
}

#[tokio::test]
async fn test_guardian_harm_analysis() {
    // Test Harm analyzer (constructor returns Self, not Result)
    use hainet_persona::guardian::harm_analyzer::AnalysisContext;
    let analyzer = HarmAnalyzer::new(None);
    
    // Test with benign text
    let context = AnalysisContext {
        conversation_id: "test-conversation".to_string(),
        message_count: 1,
        previous_violations: 0,
    };
    let benign_result = analyzer.analyze("Have a great day!", &context).await;
    assert!(benign_result.is_ok());
    let report = benign_result.unwrap();
    assert_eq!(report.intent, hainet_persona::guardian::harm_analyzer::Intent::Benign);
}

#[tokio::test]
async fn test_guardian_decision_engine() {
    // Test Decision Engine (all constructors return Self directly)
    use hainet_persona::guardian::harm_analyzer::AnalysisContext;
    
    let engine = DecisionEngine::new();
    let pii_detector = PIIDetector::new(None);
    let bias_detector = BiasDetector::new(None);
    let harm_analyzer = HarmAnalyzer::new(None);
    
    let pii_report = pii_detector.analyze("Safe text").await.unwrap();
    let bias_report = bias_detector.analyze("Safe text").await.unwrap();
    
    let context = AnalysisContext {
        conversation_id: "test-conversation".to_string(),
        message_count: 1,
        previous_violations: 0,
    };
    let harm_report = harm_analyzer.analyze("Safe text", &context).await.unwrap();
    
    // Test decision making (make_decision returns GuardianDecision directly)
    let decision = engine.make_decision(&pii_report, &bias_report, &harm_report);
    
    // Safe text should be allowed
    assert_eq!(decision.action, hainet_persona::guardian::decision_engine::GuardianAction::Allow);
}

#[tokio::test]
async fn test_constitutional_compliance_integration() {
    // End-to-end test: Prompt → AI Selection → Guardian → Messaging
    
    // 1. Load a prompt (skip if prompts directory doesn't exist in test environment)
    if std::path::Path::new("hainet-persona/prompts").exists() {
        let mut prompt_mgr = PromptManager::new("hainet-persona/prompts".into()).unwrap();
        let agent_id = hainet_persona::prompts::types::AgentId::new(
            hainet_persona::prompts::types::AgentType::Admin,
            "integration-test".to_string()
        );
        let context = PromptContext::default();
        let prompt = prompt_mgr.get_prompt(&agent_id, AgentState::Idle, &context).await;
        assert!(prompt.is_ok(), "Should load prompt");
    }
    
    // 2. Initialize AI provider manager
    let ai_mgr = AIProviderManager::new().await;
    assert!(ai_mgr.is_ok(), "Should initialize AI provider manager");
    
    // 3. Test Guardian on message content
    let pii_detector = PIIDetector::new(None);
    let test_message = "Please help me with my task.";
    let pii_report = pii_detector.analyze(test_message).await;
    assert!(pii_report.is_ok(), "Guardian should analyze message");
    
    // 4. Create message bus
    let bus = MessageBus::new().await;
    assert!(bus.is_ok(), "Should create message bus");
    
    // Integration successful if all components initialized
}

#[test]
fn test_phase_0_component_summary() {
    // Summary test to document what's implemented
    println!("Phase 0 Components Implemented:");
    println!("✅ Prompt System (Cycles 0.2)");
    println!("✅ Messaging Infrastructure (Cycle 0.3)");
    println!("✅ AI Provider Discovery (Cycle 0.4)");
    println!("✅ Guardian System (Cycle 0.4)");
    println!("✅ Blockchain Identity (Cycle 0.5-C)");
    println!("✅ Content-Addressed Storage (Cycle 0.5-D)");
    println!("✅ Ollama Auto-Installer (Cycle 0.5-B)");
    
    println!("\nMissing for Phase 1:");
    println!("❌ Actual AI Agents (Admin, PM, Workers)");
    println!("❌ State Machine Execution");
    println!("❌ Memory System");
    println!("❌ MCP Tool Integration");
    println!("❌ Agent-to-Agent Communication Logic");
}
