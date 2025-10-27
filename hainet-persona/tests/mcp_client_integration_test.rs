//! # MCP Client Integration Tests
//!
//! Tests the full MCP client workflow using the hainet-files server.

use hainet_persona::tools::mcp::client::MCPClientManager;
use serde_json::json;
use std::process::Command;
use tempfile::TempDir;
use tokio::time::{sleep, Duration};

/// Helper to start hainet-files server
fn create_files_server_command() -> Command {
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--package")
        .arg("hainet-files")
        .arg("--")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit());
    cmd
}

#[tokio::test]
async fn test_client_start_and_list_tools() {
    let manager = MCPClientManager::new();
    let cmd = create_files_server_command();
    
    // Start the server
    manager.start_server("test-files", cmd).await.expect("Failed to start server");
    
    // Give server time to initialize
    sleep(Duration::from_millis(100)).await;
    
    // List tools
    let tools = manager.list_tools("test-files").await.expect("Failed to list tools");
    
    assert_eq!(tools.len(), 4, "Expected 4 tools from hainet-files");
    
    let tool_names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(tool_names.contains(&"hainet_file_read"));
    assert!(tool_names.contains(&"hainet_file_write"));
    assert!(tool_names.contains(&"hainet_file_list"));
    assert!(tool_names.contains(&"hainet_file_metadata"));
    
    // Shutdown
    manager.shutdown_server("test-files").await.expect("Failed to shutdown");
}

#[tokio::test]
async fn test_client_file_write_and_read() {
    let manager = MCPClientManager::new();
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = temp_dir.path().join("client_test.txt");
    
    let cmd = create_files_server_command();
    manager.start_server("test-files", cmd).await.expect("Failed to start server");
    sleep(Duration::from_millis(100)).await;
    
    // Write file
    let write_args = json!({
        "path": test_file.to_string_lossy().to_string(),
        "content": "Hello from MCP client!"
    });
    
    let write_result = manager.call_tool("test-files", "hainet_file_write", write_args)
        .await
        .expect("Failed to write file");
    
    println!("Write result: {}", write_result);
    
    // Read file back
    let read_args = json!({
        "path": test_file.to_string_lossy().to_string()
    });
    
    let read_result = manager.call_tool("test-files", "hainet_file_read", read_args)
        .await
        .expect("Failed to read file");
    
    println!("Read result: {}", read_result);
    
    // Verify content
    let read_obj = read_result.as_object().expect("Expected object response");
    let content = read_obj.get("content").and_then(|v| v.as_str()).expect("Missing content");
    assert_eq!(content, "Hello from MCP client!");
    
    manager.shutdown_server("test-files").await.expect("Failed to shutdown");
}

#[tokio::test]
async fn test_client_file_list() {
    let manager = MCPClientManager::new();
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Create some test files
    std::fs::write(temp_dir.path().join("file1.txt"), "test1").expect("Failed to create file1");
    std::fs::write(temp_dir.path().join("file2.txt"), "test2").expect("Failed to create file2");
    std::fs::write(temp_dir.path().join("file3.txt"), "test3").expect("Failed to create file3");
    
    let cmd = create_files_server_command();
    manager.start_server("test-files", cmd).await.expect("Failed to start server");
    sleep(Duration::from_millis(100)).await;
    
    // List files
    let list_args = json!({
        "path": temp_dir.path().to_string_lossy().to_string()
    });
    
    let list_result = manager.call_tool("test-files", "hainet_file_list", list_args)
        .await
        .expect("Failed to list files");
    
    println!("List result: {}", list_result);
    
    let list_obj = list_result.as_object().expect("Expected object response");
    let count = list_obj.get("count").and_then(|v| v.as_u64()).expect("Missing count");
    assert_eq!(count, 3, "Expected 3 files");
    
    manager.shutdown_server("test-files").await.expect("Failed to shutdown");
}

#[tokio::test]
async fn test_client_file_metadata() {
    let manager = MCPClientManager::new();
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = temp_dir.path().join("metadata_test.txt");
    
    std::fs::write(&test_file, "metadata test content").expect("Failed to create file");
    
    let cmd = create_files_server_command();
    manager.start_server("test-files", cmd).await.expect("Failed to start server");
    sleep(Duration::from_millis(100)).await;
    
    // Get metadata
    let metadata_args = json!({
        "path": test_file.to_string_lossy().to_string()
    });
    
    let metadata_result = manager.call_tool("test-files", "hainet_file_metadata", metadata_args)
        .await
        .expect("Failed to get metadata");
    
    println!("Metadata result: {}", metadata_result);
    
    let metadata_obj = metadata_result.as_object().expect("Expected object response");
    let is_file = metadata_obj.get("is_file").and_then(|v| v.as_bool()).expect("Missing is_file");
    let size = metadata_obj.get("size").and_then(|v| v.as_u64()).expect("Missing size");
    
    assert!(is_file, "Expected file, not directory");
    assert_eq!(size, 21, "Expected size 21 bytes");
    
    manager.shutdown_server("test-files").await.expect("Failed to shutdown");
}

#[tokio::test]
async fn test_client_multiple_servers() {
    let manager = MCPClientManager::new();
    
    // Start two server instances
    let cmd1 = create_files_server_command();
    let cmd2 = create_files_server_command();
    
    manager.start_server("files-1", cmd1).await.expect("Failed to start server 1");
    manager.start_server("files-2", cmd2).await.expect("Failed to start server 2");
    sleep(Duration::from_millis(100)).await;
    
    // Verify both are connected
    assert!(manager.is_connected("files-1").await);
    assert!(manager.is_connected("files-2").await);
    
    // List servers
    let servers = manager.list_servers().await;
    assert_eq!(servers.len(), 2);
    assert!(servers.contains(&"files-1".to_string()));
    assert!(servers.contains(&"files-2".to_string()));
    
    // List tools from both
    let tools1 = manager.list_tools("files-1").await.expect("Failed to list from server 1");
    let tools2 = manager.list_tools("files-2").await.expect("Failed to list from server 2");
    
    assert_eq!(tools1.len(), 4);
    assert_eq!(tools2.len(), 4);
    
    // Shutdown all
    manager.shutdown_all().await.expect("Failed to shutdown all");
    
    assert!(!manager.is_connected("files-1").await);
    assert!(!manager.is_connected("files-2").await);
}

#[tokio::test]
async fn test_client_error_handling_unknown_tool() {
    let manager = MCPClientManager::new();
    let cmd = create_files_server_command();
    
    manager.start_server("test-files", cmd).await.expect("Failed to start server");
    sleep(Duration::from_millis(100)).await;
    
    // Call non-existent tool
    let result = manager.call_tool("test-files", "hainet_file_nonexistent", json!({})).await;
    
    assert!(result.is_err(), "Expected error for unknown tool");
    
    manager.shutdown_server("test-files").await.expect("Failed to shutdown");
}

#[tokio::test]
async fn test_client_error_handling_unknown_server() {
    let manager = MCPClientManager::new();
    
    // Call tool on non-existent server
    let result = manager.call_tool("nonexistent-server", "some_tool", json!({})).await;
    
    assert!(result.is_err(), "Expected error for unknown server");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("not found"), "Error should mention server not found");
}

#[tokio::test]
async fn test_client_shutdown_specific_server() {
    let manager = MCPClientManager::new();
    
    let cmd1 = create_files_server_command();
    let cmd2 = create_files_server_command();
    
    manager.start_server("files-1", cmd1).await.expect("Failed to start server 1");
    manager.start_server("files-2", cmd2).await.expect("Failed to start server 2");
    sleep(Duration::from_millis(100)).await;
    
    assert_eq!(manager.list_servers().await.len(), 2);
    
    // Shutdown only server 1
    manager.shutdown_server("files-1").await.expect("Failed to shutdown server 1");
    
    assert!(!manager.is_connected("files-1").await);
    assert!(manager.is_connected("files-2").await);
    assert_eq!(manager.list_servers().await.len(), 1);
    
    manager.shutdown_all().await.expect("Failed to shutdown all");
}
