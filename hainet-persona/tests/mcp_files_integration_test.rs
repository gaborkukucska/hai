//! # MCP Files Integration Tests
//!
//! End-to-end tests for MCP client ↔ hainet-files server integration.
//! Tests all file operations with content-addressed storage.

use anyhow::Result;
use hainet_persona::tools::mcp::MCPClientManager;
use serde_json::json;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Helper to get the hainet-files binary path
fn get_hainet_files_binary() -> PathBuf {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = PathBuf::from(manifest_dir).parent().unwrap().to_path_buf();
    
    // Try debug build first, then release
    let debug_bin = workspace_root
        .join("target")
        .join("debug")
        .join("hainet-files");
    
    if debug_bin.exists() {
        debug_bin
    } else {
        workspace_root
            .join("target")
            .join("release")
            .join("hainet-files")
    }
}

/// Set up MCP client with hainet-files server
async fn setup_mcp_client() -> Result<(MCPClientManager, TempDir)> {
    // Create temp directory for test files
    let temp_dir = TempDir::new()?;
    
    // Create MCP client
    let client = MCPClientManager::new();
    
    // Build command to start hainet-files server
    let binary_path = get_hainet_files_binary();
    let mut cmd = Command::new(&binary_path);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::inherit());
    
    // Start the server
    client.start_server("hainet-files", cmd).await?;
    
    // Give server a moment to initialize
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    Ok((client, temp_dir))
}

#[tokio::test]
async fn test_mcp_server_startup() -> Result<()> {
    let (client, _temp_dir) = setup_mcp_client().await?;
    
    // Verify server is connected
    assert!(client.is_connected("hainet-files").await);
    
    // List servers
    let servers = client.list_servers().await;
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0], "hainet-files");
    
    // Shutdown
    client.shutdown_server("hainet-files").await?;
    assert!(!client.is_connected("hainet-files").await);
    
    Ok(())
}

#[tokio::test]
async fn test_list_tools() -> Result<()> {
    let (client, _temp_dir) = setup_mcp_client().await?;
    
    // List available tools
    let tools = client.list_tools("hainet-files").await?;
    
    // Verify all 4 tools are available
    assert_eq!(tools.len(), 4);
    
    let tool_names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    assert!(tool_names.contains(&"hainet_file_read".to_string()));
    assert!(tool_names.contains(&"hainet_file_write".to_string()));
    assert!(tool_names.contains(&"hainet_file_list".to_string()));
    assert!(tool_names.contains(&"hainet_file_metadata".to_string()));
    
    // Verify tool schemas
    for tool in &tools {
        assert!(tool.description.is_some());
        assert!(!tool.input_schema.is_empty());
    }
    
    client.shutdown_server("hainet-files").await?;
    Ok(())
}

#[tokio::test]
async fn test_file_write_and_read() -> Result<()> {
    let (client, temp_dir) = setup_mcp_client().await?;
    
    let test_file = temp_dir.path().join("test.txt");
    let test_content = "Hello HAI-Net! This is a test file.";
    
    // Write file
    let write_result = client
        .call_tool(
            "hainet-files",
            "hainet_file_write",
            json!({
                "path": test_file.to_str().unwrap(),
                "content": test_content
            }),
        )
        .await?;
    
    // Parse write result
    assert!(write_result["success"].as_bool().unwrap());
    assert_eq!(write_result["path"].as_str().unwrap(), test_file.to_str().unwrap());
    assert!(write_result["hash"].as_str().is_some());
    assert_eq!(write_result["size"].as_u64().unwrap(), test_content.len() as u64);
    
    let write_hash = write_result["hash"].as_str().unwrap();
    
    // Verify file was actually created
    assert!(test_file.exists());
    let file_content = fs::read_to_string(&test_file)?;
    assert_eq!(file_content, test_content);
    
    // Read file back
    let read_result = client
        .call_tool(
            "hainet-files",
            "hainet_file_read",
            json!({
                "path": test_file.to_str().unwrap()
            }),
        )
        .await?;
    
    // Verify read result
    assert_eq!(read_result["content"].as_str().unwrap(), test_content);
    assert_eq!(read_result["hash"].as_str().unwrap(), write_hash);
    assert_eq!(read_result["size"].as_u64().unwrap(), test_content.len() as u64);
    
    client.shutdown_server("hainet-files").await?;
    Ok(())
}

#[tokio::test]
async fn test_file_list() -> Result<()> {
    let (client, temp_dir) = setup_mcp_client().await?;
    
    // Create some test files
    let file1 = temp_dir.path().join("file1.txt");
    let file2 = temp_dir.path().join("file2.txt");
    let file3 = temp_dir.path().join("file3.txt");
    
    fs::write(&file1, "content1")?;
    fs::write(&file2, "content2")?;
    fs::write(&file3, "content3")?;
    
    // List directory
    let list_result = client
        .call_tool(
            "hainet-files",
            "hainet_file_list",
            json!({
                "path": temp_dir.path().to_str().unwrap()
            }),
        )
        .await?;
    
    // Verify list result
    assert_eq!(list_result["path"].as_str().unwrap(), temp_dir.path().to_str().unwrap());
    assert_eq!(list_result["count"].as_u64().unwrap(), 3);
    
    let entries = list_result["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 3);
    
    let entry_names: Vec<&str> = entries.iter().filter_map(|e| e.as_str()).collect();
    assert!(entry_names.contains(&"file1.txt"));
    assert!(entry_names.contains(&"file2.txt"));
    assert!(entry_names.contains(&"file3.txt"));
    
    client.shutdown_server("hainet-files").await?;
    Ok(())
}

#[tokio::test]
async fn test_file_metadata() -> Result<()> {
    let (client, temp_dir) = setup_mcp_client().await?;
    
    let test_file = temp_dir.path().join("metadata_test.txt");
    let test_content = "Metadata test content";
    fs::write(&test_file, test_content)?;
    
    // Get metadata
    let metadata_result = client
        .call_tool(
            "hainet-files",
            "hainet_file_metadata",
            json!({
                "path": test_file.to_str().unwrap()
            }),
        )
        .await?;
    
    // Verify metadata
    assert_eq!(metadata_result["path"].as_str().unwrap(), test_file.to_str().unwrap());
    assert_eq!(metadata_result["size"].as_u64().unwrap(), test_content.len() as u64);
    assert_eq!(metadata_result["is_file"].as_bool().unwrap(), true);
    assert_eq!(metadata_result["is_dir"].as_bool().unwrap(), false);
    assert!(metadata_result["readonly"].as_bool().is_some());
    
    client.shutdown_server("hainet-files").await?;
    Ok(())
}

#[tokio::test]
async fn test_content_addressed_storage_deduplication() -> Result<()> {
    let (client, temp_dir) = setup_mcp_client().await?;
    
    let file1 = temp_dir.path().join("duplicate1.txt");
    let file2 = temp_dir.path().join("duplicate2.txt");
    let identical_content = "This content is identical in both files";
    
    // Write same content to two different files
    let write_result1 = client
        .call_tool(
            "hainet-files",
            "hainet_file_write",
            json!({
                "path": file1.to_str().unwrap(),
                "content": identical_content
            }),
        )
        .await?;
    
    let write_result2 = client
        .call_tool(
            "hainet-files",
            "hainet_file_write",
            json!({
                "path": file2.to_str().unwrap(),
                "content": identical_content
            }),
        )
        .await?;
    
    // Both files should have the same BLAKE3 hash (content-addressed storage)
    let hash1 = write_result1["hash"].as_str().unwrap();
    let hash2 = write_result2["hash"].as_str().unwrap();
    
    assert_eq!(hash1, hash2, "Identical content should produce identical BLAKE3 hashes");
    
    client.shutdown_server("hainet-files").await?;
    Ok(())
}

#[tokio::test]
async fn test_error_handling_file_not_found() -> Result<()> {
    let (client, temp_dir) = setup_mcp_client().await?;
    
    let nonexistent_file = temp_dir.path().join("does_not_exist.txt");
    
    // Try to read non-existent file
    let read_result = client
        .call_tool(
            "hainet-files",
            "hainet_file_read",
            json!({
                "path": nonexistent_file.to_str().unwrap()
            }),
        )
        .await;
    
    // Should return error
    assert!(read_result.is_err());
    let err_msg = read_result.unwrap_err().to_string();
    assert!(err_msg.contains("Failed to call tool") || err_msg.contains("File read error"));
    
    client.shutdown_server("hainet-files").await?;
    Ok(())
}

#[tokio::test]
async fn test_error_handling_invalid_tool() -> Result<()> {
    let (client, _temp_dir) = setup_mcp_client().await?;
    
    // Try to call non-existent tool
    let result = client
        .call_tool(
            "hainet-files",
            "hainet_file_nonexistent",
            json!({}),
        )
        .await;
    
    // Should return error
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Unknown tool") || err_msg.contains("METHOD_NOT_FOUND"));
    
    client.shutdown_server("hainet-files").await?;
    Ok(())
}

#[tokio::test]
async fn test_error_handling_missing_parameters() -> Result<()> {
    let (client, _temp_dir) = setup_mcp_client().await?;
    
    // Try to call tool with missing required parameter
    let result = client
        .call_tool(
            "hainet-files",
            "hainet_file_read",
            json!({}), // Missing 'path' parameter
        )
        .await;
    
    // Should return error
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Missing 'path' parameter") || err_msg.contains("INVALID_PARAMS"));
    
    client.shutdown_server("hainet-files").await?;
    Ok(())
}

#[tokio::test]
async fn test_multiple_operations_sequence() -> Result<()> {
    let (client, temp_dir) = setup_mcp_client().await?;
    
    let test_file = temp_dir.path().join("sequence_test.txt");
    
    // 1. Write initial content
    let write1 = client
        .call_tool(
            "hainet-files",
            "hainet_file_write",
            json!({
                "path": test_file.to_str().unwrap(),
                "content": "Initial content"
            }),
        )
        .await?;
    
    let hash1 = write1["hash"].as_str().unwrap();
    
    // 2. Read it back
    let read1 = client
        .call_tool(
            "hainet-files",
            "hainet_file_read",
            json!({
                "path": test_file.to_str().unwrap()
            }),
        )
        .await?;
    
    assert_eq!(read1["content"].as_str().unwrap(), "Initial content");
    assert_eq!(read1["hash"].as_str().unwrap(), hash1);
    
    // 3. Get metadata
    let metadata = client
        .call_tool(
            "hainet-files",
            "hainet_file_metadata",
            json!({
                "path": test_file.to_str().unwrap()
            }),
        )
        .await?;
    
    assert_eq!(metadata["size"].as_u64().unwrap(), "Initial content".len() as u64);
    
    // 4. Overwrite with new content
    let write2 = client
        .call_tool(
            "hainet-files",
            "hainet_file_write",
            json!({
                "path": test_file.to_str().unwrap(),
                "content": "Updated content with different length"
            }),
        )
        .await?;
    
    let hash2 = write2["hash"].as_str().unwrap();
    assert_ne!(hash1, hash2, "Different content should have different hashes");
    
    // 5. Read updated content
    let read2 = client
        .call_tool(
            "hainet-files",
            "hainet_file_read",
            json!({
                "path": test_file.to_str().unwrap()
            }),
        )
        .await?;
    
    assert_eq!(read2["content"].as_str().unwrap(), "Updated content with different length");
    assert_eq!(read2["hash"].as_str().unwrap(), hash2);
    
    client.shutdown_server("hainet-files").await?;
    Ok(())
}
