//! # MCP Integration Test
//!
//! Tests end-to-end MCP client-server communication using the hainet-files server.

use anyhow::Result;
use hainet_persona::tools::mcp::client::MCPClientManager;
use serde_json::json;
use std::process::Command;
use tempfile::TempDir;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_mcp_client_server_communication() -> Result<()> {
    // Create a temporary directory for test files
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "Hello from MCP test!")?;

    // Create MCP client manager
    let client = MCPClientManager::new();

    // Build command to start hainet-files server
    let mut cmd = Command::new("cargo");
    let project_root = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    cmd.arg("run")
        .arg("--package")
        .arg("hainet-files")
        .current_dir(project_root);

    // Start the server
    client.start_server("test-files", cmd).await?;

    // Give the server time to initialize
    sleep(Duration::from_millis(500)).await;

    // Test 1: List tools
    let tools = client.list_tools("test-files").await?;
    assert!(tools.len() >= 4, "Expected at least 4 tools");
    
    let tool_names: Vec<String> = tools.iter()
        .map(|t| t.name.to_string())
        .collect();
    
    assert!(tool_names.contains(&"hainet_file_read".to_string()));
    assert!(tool_names.contains(&"hainet_file_write".to_string()));
    assert!(tool_names.contains(&"hainet_file_list".to_string()));
    assert!(tool_names.contains(&"hainet_file_metadata".to_string()));

    // Test 2: Read file
    let result = client
        .call_tool(
            "test-files",
            "hainet_file_read",
            json!({
                "path": test_file.to_str().unwrap()
            }),
        )
        .await?;

    println!("Read result: {:?}", result);
    
    // Verify the result contains our test content
    let result_str = result.to_string();
    assert!(result_str.contains("Hello from MCP test!"));

    // Test 3: Write file
    let new_file = temp_dir.path().join("new_test.txt");
    let write_result = client
        .call_tool(
            "test-files",
            "hainet_file_write",
            json!({
                "path": new_file.to_str().unwrap(),
                "content": "New content from MCP!"
            }),
        )
        .await?;

    println!("Write result: {:?}", write_result);
    
    // Verify file was created
    assert!(new_file.exists());
    let content = std::fs::read_to_string(&new_file)?;
    assert_eq!(content, "New content from MCP!");

    // Test 4: List directory
    let list_result = client
        .call_tool(
            "test-files",
            "hainet_file_list",
            json!({
                "path": temp_dir.path().to_str().unwrap()
            }),
        )
        .await?;

    println!("List result: {:?}", list_result);
    
    let list_str = list_result.to_string();
    assert!(list_str.contains("test.txt"));
    assert!(list_str.contains("new_test.txt"));

    // Test 5: Get metadata
    let metadata_result = client
        .call_tool(
            "test-files",
            "hainet_file_metadata",
            json!({
                "path": test_file.to_str().unwrap()
            }),
        )
        .await?;

    println!("Metadata result: {:?}", metadata_result);
    
    let metadata_str = metadata_result.to_string();
    assert!(metadata_str.contains("is_file"));
    assert!(metadata_str.contains("size"));

    // Cleanup: shutdown server
    client.shutdown_server("test-files").await?;

    println!("✅ All MCP integration tests passed!");
    
    Ok(())
}

#[tokio::test]
async fn test_mcp_config_loading() -> Result<()> {
    let client = MCPClientManager::new();
    
    // This should gracefully handle missing config
    let results = client.start_default_servers().await?;
    
    // Since we may not have all servers available, just verify it doesn't crash
    println!("Started {} servers from default config", results.len());
    
    // Shutdown any that started
    client.shutdown_all().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_mcp_client_error_handling() -> Result<()> {
    let client = MCPClientManager::new();
    
    // Test 1: Calling tool on non-existent server
    let result = client.call_tool("nonexistent", "some_tool", json!({})).await;
    assert!(result.is_err());
    
    // Test 2: Listing tools from non-existent server
    let result = client.list_tools("nonexistent").await;
    assert!(result.is_err());
    
    // Test 3: Shutdown non-existent server
    let result = client.shutdown_server("nonexistent").await;
    assert!(result.is_err());
    
    println!("✅ Error handling tests passed!");
    
    Ok(())
}

#[tokio::test] 
async fn test_mcp_multiple_servers() -> Result<()> {
    let client = MCPClientManager::new();
    
    // Start same server twice with different names (should both work)
    let project_root = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let mut cmd1 = Command::new("cargo");
    cmd1.arg("run").arg("--package").arg("hainet-files")
        .current_dir(project_root.clone());
    
    let mut cmd2 = Command::new("cargo");
    cmd2.arg("run").arg("--package").arg("hainet-files")
        .current_dir(project_root);
    
    client.start_server("files-1", cmd1).await?;
    client.start_server("files-2", cmd2).await?;
    
    sleep(Duration::from_millis(500)).await;
    
    // Verify both are connected
    assert!(client.is_connected("files-1").await);
    assert!(client.is_connected("files-2").await);
    
    // List servers
    let servers = client.list_servers().await;
    assert_eq!(servers.len(), 2);
    
    // Shutdown all
    client.shutdown_all().await?;
    
    // Verify all disconnected
    assert!(!client.is_connected("files-1").await);
    assert!(!client.is_connected("files-2").await);
    
    println!("✅ Multiple server tests passed!");
    
    Ok(())
}
