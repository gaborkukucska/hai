use anyhow::Result;
use hainet_persona::agents::{AdminAgent, Agent, AgentContext};
use hainet_persona::projects::ProjectManager;
use hainet_persona::messaging::{MessageBus, AgentId, AgentType, Message, MessageContent, TaskResult, ErrorReport, StatusUpdate};
use hainet_persona::prompts::{PromptManager, AgentState};
use hainet_persona::tools::mcp::MCPClientManager;
use hainet_persona::guardian::GuardianSystem;
use hainet_persona::ai_providers::AIProviderManager;
use hainet_persona::agents::metrics::MetricsCollector;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::path::PathBuf;

async fn create_test_context() -> Result<Arc<AgentContext>> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let prompts_path = PathBuf::from(manifest_dir).join("prompts");
    let ai_provider_manager = Arc::new(AIProviderManager::new().await?);

    Ok(Arc::new(AgentContext::new(
        Arc::new(RwLock::new(MessageBus::new().await?)),
        Arc::new(RwLock::new(PromptManager::new(prompts_path)?)),
        Arc::new(RwLock::new(MCPClientManager::new())),
        Arc::new(RwLock::new(GuardianSystem::new(ai_provider_manager, None))),
    )))
}

async fn create_test_admin(context: Arc<AgentContext>) -> Result<AdminAgent> {
    let project_manager = Arc::new(RwLock::new(
        ProjectManager::new("sqlite::memory:").await?,
    ));
    let ai_provider_manager = Arc::new(AIProviderManager::new().await?);
    let metrics = Arc::new(RwLock::new(
        MetricsCollector::new("sqlite::memory:").await?,
    ));

    AdminAgent::new(
        context, 
        project_manager, 
        ai_provider_manager, 
        metrics,
        "sqlite::memory:".to_string(),
        "sqlite::memory:".to_string()
    ).await
}

#[tokio::test]
async fn test_messaging_flow() -> Result<()> {
    // 1. Setup Context
    println!("Setup Context");
    let context = create_test_context().await?;
    
    // 2. Setup Admin Agent
    println!("Setup Admin");
    let mut admin = create_test_admin(context.clone()).await?;
    println!("Starting Admin");
    admin.start().await?; // Spawns message loop
    println!("Admin Started");
    
    // 3. Simulate a PM Agent (we'll just register a receiver to act as PM)
    let pm_id = AgentId::new(AgentType::PM, "PM-TestProject".to_string());
    let (mut pm_receiver, _) = context.message_bus.write().await
        .register_agent(pm_id.clone())
        .await?;
        
    // 4. Simulate a Worker Agent (we'll just register a receiver to act as Worker)
    let worker_id = AgentId::new(AgentType::Worker, "Worker-1".to_string());
    let (_worker_receiver, _) = context.message_bus.write().await
        .register_agent(worker_id.clone())
        .await?;
        
    // 5. Simulate User (to receive forwarded messages from Admin)
    let user_id = AgentId::user("user".to_string());
    let (mut user_receiver, _) = context.message_bus.write().await
        .register_agent(user_id.clone())
        .await?;

    // TEST CASE 1: Worker sends TaskResult to PM (simulated)
    println!("Starting TEST CASE 1");
    // In real flow, Worker sends to PM. PM updates DB. 
    // Here we just verify PM receives it.
    let task_result_msg = Message::new(
        worker_id.clone(),
        pm_id.clone(),
        MessageContent::TaskResult(TaskResult {
            task_id: "task-1".to_string(),
            success: true,
            output: serde_json::Value::String("Task done".to_string()),
            error: None,
            metrics: hainet_persona::messaging::TaskMetrics {
                duration_ms: 100,
                cost_usd: 0.0,
                resource_tier_used: hainet_persona::messaging::ResourceTier::LocalOnly,
                tokens_used: None,
            },
        })
    );
    
    context.message_bus.write().await.send_message(task_result_msg).await?;
    
    // Verify PM received it
    let received = tokio::time::timeout(tokio::time::Duration::from_secs(1), pm_receiver.recv()).await
        .expect("PM should receive message")
        .expect("Message channel closed");
        
    if let MessageContent::TaskResult(result) = received.content {
        assert_eq!(result.task_id, "task-1");
        assert_eq!(result.output, serde_json::Value::String("Task done".to_string()));
    } else {
        panic!("PM received wrong message type");
    }
    
    // TEST CASE 2: PM sends Project Completion StatusUpdate to Admin
    println!("Starting TEST CASE 2");
    let completion_msg = Message::new(
        pm_id.clone(),
        admin.id().clone(),
        MessageContent::StatusUpdate(StatusUpdate {
            agent_id: pm_id.clone(),
            state: AgentState::Idle,
            message: "Project completed successfully".to_string(),
            progress: Some(1.0),
        })
    );
    
    context.message_bus.write().await.send_message(completion_msg).await?;
    
    // Verify Admin forwarded to User
    let received_user = tokio::time::timeout(tokio::time::Duration::from_secs(1), user_receiver.recv()).await
        .expect("User should receive forwarded message")
        .expect("Message channel closed");
        
    if let MessageContent::Response(text) = received_user.content {
        assert!(text.contains("PROJECT UPDATE"));
        assert!(text.contains("Project completed successfully"));
    } else {
        panic!("User received wrong message type from Admin");
    }
    
    // TEST CASE 3: PM sends Query to Admin (simulated)
    println!("Starting TEST CASE 3");
    let query_msg = Message::new(
        pm_id.clone(),
        admin.id().clone(),
        MessageContent::Query("Do we need unit tests?".to_string())
    );
    
    context.message_bus.write().await.send_message(query_msg).await?;
    
    // Verify Admin forwarded to User
    let received_user_query = tokio::time::timeout(tokio::time::Duration::from_secs(1), user_receiver.recv()).await
        .expect("User should receive forwarded query")
        .expect("Message channel closed");
        
    if let MessageContent::Response(text) = received_user_query.content {
        assert!(text.contains("QUESTION"));
        assert!(text.contains("Do we need unit tests?"));
    } else {
        panic!("User received wrong message type for query");
    }
    
    // TEST CASE 4: Admin sends Response to PM (simulated user reply)
    println!("Starting TEST CASE 4");
    // Note: In real app, User -> Admin -> PM. 
    // Admin logic to forward User response to PM is not fully implemented yet (Admin processes user input but doesn't track which PM asked).
    // But we can verify that IF Admin sends a Response, PM (simulated) receives it.
    
    let response_msg = Message::new(
        admin.id().clone(),
        pm_id.clone(),
        MessageContent::Response("Yes, absolutely.".to_string())
    );
    
    context.message_bus.write().await.send_message(response_msg).await?;
    
    // Verify PM received it
    let received_pm_response = tokio::time::timeout(tokio::time::Duration::from_secs(1), pm_receiver.recv()).await
        .expect("PM should receive response")
        .expect("Message channel closed");
        
    if let MessageContent::Response(text) = received_pm_response.content {
        assert_eq!(text, "Yes, absolutely.");
    } else {
        panic!("PM received wrong message type for response");
    }

    Ok(())
}
