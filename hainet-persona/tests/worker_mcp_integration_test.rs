//! Worker Agent MCP Integration Tests
//! 
//! Tests the full workflow:
//! 1. Create project with tasks
//! 2. Start hainet-files MCP server
//! 3. Assign task to Worker agent
//! 4. Worker discovers tools and executes task
//! 5. Verify task completion

use anyhow::Result;
use hainet_persona::agents::WorkerAgent;
use hainet_persona::prompts::{AgentState, PromptManager, WorkerType};
use hainet_persona::messaging::MessageBus;
use hainet_persona::projects::ProjectManager;
use hainet_persona::tools::mcp::MCPClientManager;
use std::sync::Arc;
use std::process::Command;
use tokio::sync::RwLock;
use std::path::PathBuf;
use std::env;

/// Helper to create a test worker with all dependencies
async fn create_test_worker(worker_type: WorkerType) -> Result<WorkerAgent> {
    let message_bus = Arc::new(RwLock::new(MessageBus::new().await?));
    let prompt_manager = Arc::new(PromptManager::new("prompts".into())?);
    let project_manager = Arc::new(RwLock::new(
        ProjectManager::new("sqlite::memory:").await?
    ));
    let mcp_client = Arc::new(RwLock::new(MCPClientManager::new()));
    
    Ok(WorkerAgent::new(
        worker_type,
        message_bus,
        prompt_manager,
        project_manager,
        mcp_client,
    ))
}

/// Helper to start hainet-files MCP server
async fn start_mcp_server(mcp_client: &Arc<RwLock<MCPClientManager>>) -> Result<()> {
    // Find the hainet-files server binary
    let mut server_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    server_path.push("../target/release/hainet-files");
    
    // Create command to start the server
    let mut command = Command::new(server_path);
    
    // Start the server
    let mut client = mcp_client.write().await;
    client.start_server("hainet-files", command).await?;
    
    Ok(())
}

#[tokio::test]
async fn test_worker_tool_discovery() -> Result<()> {
    let worker = create_test_worker(WorkerType::Files).await?;
    
    // Start MCP server
    start_mcp_server(worker.mcp_client()).await?;
    
    // Discover tools
    let tools = worker.discover_tools().await?;
    
    // Should have 4 tools from hainet-files
    assert!(!tools.is_empty(), "Should discover tools from MCP server");
    assert!(tools.iter().any(|t| t.contains("hainet_file_read")));
    assert!(tools.iter().any(|t| t.contains("hainet_file_write")));
    assert!(tools.iter().any(|t| t.contains("hainet_file_list")));
    assert!(tools.iter().any(|t| t.contains("hainet_file_metadata")));
    
    Ok(())
}

#[tokio::test]
async fn test_worker_file_read_task() -> Result<()> {
    let mut worker = create_test_worker(WorkerType::Files).await?;
    
    // Start MCP server
    start_mcp_server(worker.mcp_client()).await?;
    
    // Create a test file
    let test_file = "/tmp/worker_test_read.txt";
    std::fs::write(test_file, "Worker test content")?;
    
    // Create project with a read task
    let task_id = {
        let pm = worker.project_manager().write().await;
        let project_id = pm.create_project(
            "File Read Test".to_string(),
            "Test worker reading a file".to_string(),
            vec![format!("read {}", test_file)],
        ).await?;
        
        let tasks = pm.get_project_tasks(&project_id).await?;
        tasks[0].id.clone()
    };
    
    // Transition worker to Idle state
    worker.state_machine_mut().transition(AgentState::Idle, "Ready".to_string())?;
    
    // Assign task to worker
    worker.assign_task(task_id).await?;
    
    // Execute task
    let result = worker.execute_task().await;
    assert!(result.is_ok(), "Worker should execute read task successfully: {:?}", result.err());
    
    // Worker should be in Reporting state
    assert_eq!(worker.state(), &AgentState::Reporting);
    
    // Cleanup
    std::fs::remove_file(test_file).ok();
    
    Ok(())
}

#[tokio::test]
async fn test_worker_file_write_task() -> Result<()> {
    let mut worker = create_test_worker(WorkerType::Files).await?;
    
    // Start MCP server
    start_mcp_server(worker.mcp_client()).await?;
    
    let test_file = "/tmp/worker_test_write.txt";
    
    // Create project with a write task
    let task_id = {
        let pm = worker.project_manager().write().await;
        let project_id = pm.create_project(
            "File Write Test".to_string(),
            "Test worker writing a file".to_string(),
            vec![format!("write {}", test_file)],
        ).await?;
        
        let tasks = pm.get_project_tasks(&project_id).await?;
        tasks[0].id.clone()
    };
    
    // Transition worker to Idle state
    worker.state_machine_mut().transition(AgentState::Idle, "Ready".to_string())?;
    
    // Assign and execute task
    worker.assign_task(task_id).await?;
    worker.execute_task().await?;
    
    // Verify file was created
    assert!(std::path::Path::new(test_file).exists(), "File should be created");
    
    // Cleanup
    std::fs::remove_file(test_file).ok();
    
    Ok(())
}

#[tokio::test]
async fn test_worker_directory_list_task() -> Result<()> {
    let mut worker = create_test_worker(WorkerType::Files).await?;
    
    // Start MCP server
    start_mcp_server(worker.mcp_client()).await?;
    
    // Create project with a list task
    let task_id = {
        let pm = worker.project_manager().write().await;
        let project_id = pm.create_project(
            "Directory List Test".to_string(),
            "Test worker listing directory".to_string(),
            vec!["list /tmp".to_string()],
        ).await?;
        
        let tasks = pm.get_project_tasks(&project_id).await?;
        tasks[0].id.clone()
    };
    
    // Transition worker to Idle state
    worker.state_machine_mut().transition(AgentState::Idle, "Ready".to_string())?;
    
    // Assign and execute task
    worker.assign_task(task_id).await?;
    worker.execute_task().await?;
    
    // Should complete successfully
    assert_eq!(worker.state(), &AgentState::Reporting);
    
    Ok(())
}

#[tokio::test]
async fn test_worker_state_transitions() -> Result<()> {
    let mut worker = create_test_worker(WorkerType::Files).await?;
    
    // Start MCP server
    start_mcp_server(worker.mcp_client()).await?;
    
    // Initial state should be Startup
    assert_eq!(worker.state(), &AgentState::Startup);
    
    // Create a simple task
    let task_id = {
        let pm = worker.project_manager().write().await;
        let project_id = pm.create_project(
            "State Test".to_string(),
            "Test state transitions".to_string(),
            vec!["read /tmp/test.txt".to_string()],
        ).await?;
        
        let tasks = pm.get_project_tasks(&project_id).await?;
        tasks[0].id.clone()
    };
    
    // Transition to Idle
    worker.state_machine_mut().transition(AgentState::Idle, "Ready".to_string())?;
    assert_eq!(worker.state(), &AgentState::Idle);
    
    // Assign task
    worker.assign_task(task_id).await?;
    
    // Execute (should go through Planning → Working → Reporting)
    worker.execute_task().await?;
    
    // Should end up in Reporting state
    assert_eq!(worker.state(), &AgentState::Reporting);
    
    Ok(())
}

#[tokio::test]
async fn test_worker_error_handling() -> Result<()> {
    let mut worker = create_test_worker(WorkerType::Files).await?;
    
    // Don't start MCP server - this should cause errors
    
    // Create a task
    let task_id = {
        let pm = worker.project_manager().write().await;
        let project_id = pm.create_project(
            "Error Test".to_string(),
            "Test error handling".to_string(),
            vec!["read /nonexistent/file.txt".to_string()],
        ).await?;
        
        let tasks = pm.get_project_tasks(&project_id).await?;
        tasks[0].id.clone()
    };
    
    // Transition to Idle
    worker.state_machine_mut().transition(AgentState::Idle, "Ready".to_string())?;
    
    // Assign task
    worker.assign_task(task_id).await?;
    
    // Execute should fail because MCP server is not running
    let result = worker.execute_task().await;
    assert!(result.is_err(), "Should fail without MCP server");
    
    Ok(())
}

#[tokio::test]
async fn test_multiple_workers_parallel() -> Result<()> {
    let mut worker1 = create_test_worker(WorkerType::Files).await?;
    let mut worker2 = create_test_worker(WorkerType::Files).await?;
    
    // Start MCP server (shared)
    start_mcp_server(worker1.mcp_client()).await?;
    start_mcp_server(worker2.mcp_client()).await?;
    
    // Create test files
    std::fs::write("/tmp/worker1_test.txt", "Worker 1")?;
    std::fs::write("/tmp/worker2_test.txt", "Worker 2")?;
    
    // Create tasks for each worker
    let task_id1 = {
        let pm = worker1.project_manager().write().await;
        let project_id = pm.create_project(
            "Parallel Test 1".to_string(),
            "Worker 1 task".to_string(),
            vec!["read /tmp/worker1_test.txt".to_string()],
        ).await?;
        
        let tasks = pm.get_project_tasks(&project_id).await?;
        tasks[0].id.clone()
    };
    
    let task_id2 = {
        let pm = worker2.project_manager().write().await;
        let project_id = pm.create_project(
            "Parallel Test 2".to_string(),
            "Worker 2 task".to_string(),
            vec!["read /tmp/worker2_test.txt".to_string()],
        ).await?;
        
        let tasks = pm.get_project_tasks(&project_id).await?;
        tasks[0].id.clone()
    };
    
    // Prepare workers
    worker1.state_machine_mut().transition(AgentState::Idle, "Ready".to_string())?;
    worker2.state_machine_mut().transition(AgentState::Idle, "Ready".to_string())?;
    
    worker1.assign_task(task_id1).await?;
    worker2.assign_task(task_id2).await?;
    
    // Execute in parallel
    let (result1, result2) = tokio::join!(
        worker1.execute_task(),
        worker2.execute_task()
    );
    
    assert!(result1.is_ok(), "Worker 1 should complete");
    assert!(result2.is_ok(), "Worker 2 should complete");
    
    // Cleanup
    std::fs::remove_file("/tmp/worker1_test.txt").ok();
    std::fs::remove_file("/tmp/worker2_test.txt").ok();
    
    Ok(())
}
