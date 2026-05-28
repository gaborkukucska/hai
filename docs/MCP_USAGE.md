# MCP (Model Context Protocol) Usage Guide

## Overview

HAI-Net now has full support for the Model Context Protocol (MCP), enabling AI agents to interact with external tool servers. The implementation uses the official `rmcp` SDK for complete protocol compliance.

## Architecture

- **MCP Client** (`MCPClientManager`): Connects to and manages MCP servers
- **MCP Server** (`hainet-files`): Provides file operations as MCP tools
- **Configuration** (`mcp-servers.toml`): Defines available servers

## Quick Start

### 1. Basic Usage

```rust
use hainet_persona::tools::mcp::MCPClientManager;
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create the client manager
    let mcp = MCPClientManager::new();
    
    // Start all enabled servers from config
    let results = mcp.start_default_servers().await?;
    
    // Check which servers started successfully
    for (server_id, result) in results {
        match result {
            Ok(_) => println!("✓ Started: {}", server_id),
            Err(e) => println!("✗ Failed: {} - {}", server_id, e),
        }
    }
    
    // Call a tool
    let result = mcp.call_tool(
        "filesystem",
        "read_file", 
        json!({
            "path": "/home/tom/hai/README.md"
        })
    ).await?;
    
    println!("Result: {}", result);
    
    Ok(())
}
```

### 2. Available Default Servers

The following MCP servers are configured in `mcp-servers.toml`:

#### Filesystem (Enabled by default)
```toml
[servers.filesystem]
name = "Filesystem"
description = "File operations on allowed directories"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/home/tom/hai", "/home/tom/Documents", "/home/tom/Desktop"]
enabled = true
```

**Tools:**
- `read_file` - Read file contents
- `write_file` - Write to files
- `list_directory` - List directory contents
- `create_directory` - Create directories
- `move_file` - Move/rename files
- `search_files` - Search for files

#### Context7 (Enabled by default)
```toml
[servers.context7]
name = "Context7"
description = "Up-to-date library documentation"
command = "npx"
args = ["-y", "@upstash/context7-mcp"]
enabled = true
```

**Requires environment variables:**
- `UPSTASH_VECTOR_REST_URL`
- `UPSTASH_VECTOR_REST_TOKEN`

**Tools:**
- `resolve-library-id` - Find library documentation
- `get-library-docs` - Fetch library documentation

#### Sequential Thinking (Enabled by default)
```toml
[servers.sequential-thinking]
name = "Sequential Thinking"
description = "Structured problem-solving through chain of thought"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-sequential-thinking"]
enabled = true
```

**Tools:**
- `sequentialthinking` - Step-by-step problem solving

#### HAI-Net Files (Enabled by default)
```toml
[servers.hainet-files]
name = "HAI-Net Files"
description = "Local HAI-Net file server with content-addressed storage"
command = "cargo"
args = ["run", "--package", "hainet-files", "--release"]
enabled = true
working_dir = "/home/tom/hai"
```

**Tools:**
- `hainet_file_read` - Read with CAS deduplication
- `hainet_file_write` - Write with CAS storage
- `hainet_file_list` - List directory
- `hainet_file_metadata` - Get file metadata

#### GitHub (Disabled by default)
```toml
[servers.github]
name = "GitHub"
description = "GitHub repository operations"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
enabled = false
```

**Requires:** `GITHUB_PERSONAL_ACCESS_TOKEN` environment variable

## API Reference

### MCPClientManager

#### Connection Management

```rust
// Start a specific server
let mut cmd = std::process::Command::new("npx");
cmd.arg("-y").arg("@modelcontextprotocol/server-filesystem");
mcp.start_server("my-server", cmd).await?;

// Start from configuration file
let results = mcp.start_from_config("path/to/config.toml").await?;

// Start default servers
let results = mcp.start_default_servers().await?;

// Check connection status
if mcp.is_connected("filesystem").await {
    println!("Connected!");
}

// List active servers
let servers = mcp.list_servers().await;

// Shutdown specific server
mcp.shutdown_server("filesystem").await?;

// Shutdown all servers
mcp.shutdown_all().await?;
```

#### Tool Operations

```rust
// List available tools
let tools = mcp.list_tools("filesystem").await?;
for tool in tools {
    println!("Tool: {} - {}", tool.name, tool.description.unwrap_or_default());
}

// Call a tool
let result = mcp.call_tool(
    "filesystem",
    "read_file",
    json!({ "path": "/path/to/file" })
).await?;
```

#### Resource Operations

```rust
// List resources
let resources = mcp.list_resources("my-server").await?;

// Read a resource
let content = mcp.read_resource("my-server", "file:///path/to/resource").await?;
```

#### Prompt Operations

```rust
// List prompts
let prompts = mcp.list_prompts("my-server").await?;

// Get a prompt
let prompt = mcp.get_prompt(
    "my-server",
    "prompt-name",
    Some(json!({ "arg": "value" }))
).await?;
```

## Configuration

### Adding a New Server

Edit `hainet-persona/mcp-servers.toml`:

```toml
[servers.my-custom-server]
name = "My Custom Server"
description = "What it does"
command = "node"
args = ["path/to/server.js"]
enabled = true
working_dir = "/optional/working/dir"  # Optional
```

### Environment Variables

Some servers require environment variables. Set them before starting:

```bash
export GITHUB_PERSONAL_ACCESS_TOKEN="ghp_..."
export UPSTASH_VECTOR_REST_URL="https://..."
export UPSTASH_VECTOR_REST_TOKEN="..."
```

## Examples

### Example 1: File Operations

```rust
use hainet_persona::tools::mcp::MCPClientManager;
use serde_json::json;

async fn file_operations() -> anyhow::Result<()> {
    let mcp = MCPClientManager::new();
    mcp.start_default_servers().await?;
    
    // Read a file
    let content = mcp.call_tool(
        "filesystem",
        "read_file",
        json!({ "path": "/home/tom/hai/README.md" })
    ).await?;
    
    println!("File content: {}", content);
    
    // List directory
    let files = mcp.call_tool(
        "filesystem",
        "list_directory",
        json!({ "path": "/home/tom/hai" })
    ).await?;
    
    println!("Files: {}", files);
    
    Ok(())
}
```

### Example 2: Library Documentation

```rust
async fn get_docs() -> anyhow::Result<()> {
    let mcp = MCPClientManager::new();
    mcp.start_default_servers().await?;
    
    // Find library
    let lib_id = mcp.call_tool(
        "context7",
        "resolve-library-id",
        json!({ "libraryName": "tokio" })
    ).await?;
    
    // Get documentation
    let docs = mcp.call_tool(
        "context7",
        "get-library-docs",
        json!({
            "context7CompatibleLibraryID": lib_id,
            "topic": "async runtime"
        })
    ).await?;
    
    println!("Documentation: {}", docs);
    
    Ok(())
}
```

### Example 3: Sequential Thinking

```rust
async fn solve_problem() -> anyhow::Result<()> {
    let mcp = MCPClientManager::new();
    mcp.start_default_servers().await?;
    
    let result = mcp.call_tool(
        "sequential-thinking",
        "sequentialthinking",
        json!({
            "thought": "Let me break down this problem...",
            "nextThoughtNeeded": true,
            "thoughtNumber": 1,
            "totalThoughts": 5
        })
    ).await?;
    
    println!("Thinking result: {}", result);
    
    Ok(())
}
```

## Troubleshooting

### Server Won't Start

1. Check if the command is available:
   ```bash
   which npx
   which node
   which cargo
   ```

2. Test the command manually:
   ```bash
   npx -y @modelcontextprotocol/server-filesystem /home/tom/hai
   ```

3. Check environment variables are set

### Connection Issues

- Ensure stdio is properly configured
- Check server logs (stderr is inherited)
- Verify the server supports MCP protocol

### Tool Call Failures

- Use `list_tools()` to verify tool names
- Check argument format matches tool schema
- Ensure server is still running with `is_connected()`

## Further Reading

- [MCP Specification](https://modelcontextprotocol.io)
- [Official MCP Servers](https://github.com/modelcontextprotocol/servers)
- [rmcp SDK Documentation](https://docs.rs/rmcp)
