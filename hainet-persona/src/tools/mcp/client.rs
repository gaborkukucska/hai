//! # MCP Client - Official SDK Implementation
//!
//! This module implements the MCP client using the official `rmcp` SDK.
//! It provides a complete, protocol-compliant client for interacting with MCP servers.

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

// Re-export rmcp types for convenience
pub use rmcp::model::*;
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::{ClientHandler, RoleClient, ServiceExt};

use super::config::{MCPServersConfig, ServerConfig};

// Type alias to simplify the RunningService type
type RunningClient = rmcp::service::RunningService<RoleClient, MinimalClientHandler>;

/// Tool metadata for discovery-based loading
///
/// Provides structured access to tool information for LLM consumption.
/// Designed to be loaded lazily when the LLM needs specific tool details.
#[derive(Debug, Clone)]
pub struct ToolMetadata {
    /// Tool name (e.g., "file_write")
    pub name: String,
    
    /// Server name (e.g., "hainet-files")
    pub server: String,
    
    /// Human-readable description of what the tool does
    pub description: String,
    
    /// JSON schema for tool parameters
    pub input_schema: Value,
    
    /// Formatted parameter documentation for LLM prompts
    pub parameter_docs: String,
}

impl ToolMetadata {
    /// Create metadata from an rmcp Tool and server name
    fn from_tool(tool: &Tool, server_name: &str) -> Self {
        // Convert Arc<Map> to Value for processing
        let schema_value = Value::Object(tool.input_schema.as_ref().clone());
        let parameter_docs = Self::format_parameters(&schema_value);
        
        Self {
            name: tool.name.to_string(),
            server: server_name.to_string(),
            description: tool.description.clone().map(|s| s.to_string()).unwrap_or_default(),
            input_schema: schema_value,
            parameter_docs,
        }
    }
    
    /// Format JSON schema into human-readable parameter documentation
    fn format_parameters(schema: &Value) -> String {
        let mut docs = String::new();
        
        // Extract properties from schema
        if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
            let required = schema
                .get("required")
                .and_then(|r| r.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            
            for (param_name, param_info) in properties {
                let param_type = param_info
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("any");
                
                let param_desc = param_info
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");
                
                let required_marker = if required.contains(&param_name.as_str()) {
                    " (required)"
                } else {
                    " (optional)"
                };
                
                docs.push_str(&format!(
                    "- {}: {} {}\n  {}\n",
                    param_name, param_type, required_marker, param_desc
                ));
            }
        }
        
        docs
    }
    
    /// Get a concise summary for tool listing (name + brief description)
    pub fn summary(&self) -> String {
        let desc = if self.description.len() > 80 {
            format!("{}...", &self.description[..77])
        } else {
            self.description.clone()
        };
        
        format!("{}::{} - {}", self.server, self.name, desc)
    }
    
    /// Get full tool identifier (server::tool_name)
    pub fn full_name(&self) -> String {
        format!("{}::{}", self.server, self.name)
    }
}

/// Minimal client handler for MCP connections
#[derive(Clone)]
struct MinimalClientHandler;

impl ClientHandler for MinimalClientHandler {
    fn create_message(
        &self,
        _params: CreateMessageRequestParam,
        _context: rmcp::service::RequestContext<RoleClient>,
    ) -> impl std::future::Future<Output = Result<CreateMessageResult, rmcp::model::ErrorData>> + Send + '_ {
        async move {
            // Default implementation - could be extended for sampling support
            Err(rmcp::model::ErrorData {
                code: rmcp::model::ErrorCode::METHOD_NOT_FOUND,
                message: "Sampling not implemented".into(),
                data: None,
            })
        }
    }
}

/// Server connection state
struct ServerConnection {
    peer: rmcp::Peer<RoleClient>,
    // Keep the RunningService alive - it manages the service task lifecycle
    _running: RunningClient,
}

/// MCP Client Manager
///
/// Manages connections to multiple MCP servers using the official rmcp SDK.
/// Provides a high-level interface for tool calling, resource access, and prompt retrieval.
pub struct MCPClientManager {
    /// Active MCP clients by server name
    clients: Arc<RwLock<HashMap<String, ServerConnection>>>,
}

impl MCPClientManager {
    /// Create a new MCP client manager
    pub fn new() -> Self {
        info!("Initializing MCP Client Manager (rmcp SDK)");
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start an MCP server and connect to it
    ///
    /// # Arguments
    /// * `name` - Unique identifier for this server
    /// * `command` - Command to spawn the server process
    pub async fn start_server(&self, name: &str, command: StdCommand) -> Result<()> {
        info!("Starting MCP server: {}", name);

        // Check if server already exists
        {
            let clients = self.clients.read().await;
            if clients.contains_key(name) {
                return Err(anyhow!("Server '{}' is already running", name));
            }
        }

        // Convert std::process::Command to tokio::process::Command
        let tokio_cmd = tokio::process::Command::from(command);

        // Create transport from child process
        let transport = TokioChildProcess::new(tokio_cmd)
            .with_context(|| format!("Failed to create transport for '{}'", name))?;

        // Start the client service
        let handler = MinimalClientHandler;
        let running = handler.serve(transport)
            .await
            .with_context(|| format!("Failed to initialize connection to server '{}'", name))?;

        let peer = running.peer().clone();
        
        info!("Successfully connected to MCP server: {}", name);

        // Store connection (keep RunningService alive to maintain the service task)
        let mut clients = self.clients.write().await;
        clients.insert(
            name.to_string(),
            ServerConnection { 
                peer,
                _running: running,
            },
        );

        Ok(())
    }

    /// Call a tool on a specific server
    ///
    /// # Arguments
    /// * `server_name` - Name of the server to call
    /// * `tool_name` - Name of the tool to invoke
    /// * `arguments` - Tool arguments as JSON
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: Value,
    ) -> Result<Value> {
        debug!("Calling tool '{}' on server '{}'", tool_name, server_name);

        let clients = self.clients.read().await;
        let connection = clients
            .get(server_name)
            .ok_or_else(|| anyhow!("Server '{}' not found", server_name))?;

        // Convert JSON Value to Map for rmcp
        let args = if let Value::Object(map) = arguments {
            Some(map)
        } else if arguments.is_null() {
            None
        } else {
            return Err(anyhow!("Tool arguments must be a JSON object or null"));
        };

        // Call the tool
        let result = connection
            .peer
            .call_tool(CallToolRequestParam {
                name: Cow::Owned(tool_name.to_string()),
                arguments: args,
            })
            .await
            .map_err(|e| anyhow!("Failed to call tool '{}' on '{}': {:?}", tool_name, server_name, e))?;

        // Extract result from content
        if let Some(content_item) = result.content.first() {
            // Access the inner RawContent via the value field
            match &**content_item {
                RawContent::Text(text_content) => {
                    // Try to parse as JSON, otherwise return as string
                    if let Ok(json) = serde_json::from_str::<Value>(&text_content.text) {
                        Ok(json)
                    } else {
                        Ok(Value::String(text_content.text.clone()))
                    }
                }
                RawContent::Image(_) => Ok(Value::String("[Image response]".to_string())),
                RawContent::Resource(_) => Ok(Value::String("[Resource response]".to_string())),
                RawContent::Audio(_) => Ok(Value::String("[Audio response]".to_string())),
                RawContent::ResourceLink(_) => Ok(Value::String("[Resource link response]".to_string())),
            }
        } else {
            Ok(Value::Null)
        }
    }

    /// List all available tools from a server
    pub async fn list_tools(&self, server_name: &str) -> Result<Vec<Tool>> {
        debug!("Listing tools from server '{}'", server_name);

        let clients = self.clients.read().await;
        let connection = clients
            .get(server_name)
            .ok_or_else(|| anyhow!("Server '{}' not found", server_name))?;

        let result = connection
            .peer
            .list_all_tools()
            .await
            .map_err(|e| anyhow!("Failed to list tools from '{}': {:?}", server_name, e))?;

        Ok(result)
    }
    
    /// Get metadata for a specific tool (discovery-based loading)
    ///
    /// Returns structured tool information for LLM consumption.
    /// Tool identifier format: "server_name::tool_name"
    ///
    /// # Arguments
    /// * `tool_identifier` - Full tool name (e.g., "hainet-files::file_write")
    ///
    /// # Example
    /// ```
    /// let metadata = client.get_tool_metadata("hainet-files::file_write").await?;
    /// println!("Tool: {}", metadata.summary());
    /// println!("Parameters:\n{}", metadata.parameter_docs);
    /// ```
    pub async fn get_tool_metadata(&self, tool_identifier: &str) -> Result<ToolMetadata> {
        // Parse tool identifier (server::tool format)
        let parts: Vec<&str> = tool_identifier.split("::").collect();
        if parts.len() != 2 {
            return Err(anyhow!(
                "Invalid tool identifier '{}'. Expected format: 'server::tool'",
                tool_identifier
            ));
        }
        
        let server_name = parts[0];
        let tool_name = parts[1];
        
        debug!(
            "Getting metadata for tool '{}' from server '{}'",
            tool_name, server_name
        );
        
        // List all tools from the server
        let tools = self.list_tools(server_name).await?;
        
        // Find the specific tool
        let tool = tools
            .iter()
            .find(|t| t.name == tool_name)
            .ok_or_else(|| {
                anyhow!(
                    "Tool '{}' not found on server '{}'. Available tools: {}",
                    tool_name,
                    server_name,
                    tools.iter().map(|t| t.name.as_ref()).collect::<Vec<_>>().join(", ")
                )
            })?;
        
        // Convert to metadata
        Ok(ToolMetadata::from_tool(tool, server_name))
    }
    
    /// List all available tools with metadata (for discovery)
    ///
    /// Returns concise summaries of all tools from all connected servers.
    /// Useful for initial tool discovery phase.
    pub async fn list_all_tool_summaries(&self) -> Result<Vec<String>> {
        let mut summaries = Vec::new();
        
        let server_names = self.list_servers().await;
        
        for server_name in server_names {
            match self.list_tools(&server_name).await {
                Ok(tools) => {
                    for tool in tools {
                        let metadata = ToolMetadata::from_tool(&tool, &server_name);
                        summaries.push(metadata.summary());
                    }
                }
                Err(e) => {
                    warn!("Failed to list tools from '{}': {}", server_name, e);
                }
            }
        }
        
        Ok(summaries)
    }

    /// List resources from a server
    pub async fn list_resources(&self, server_name: &str) -> Result<Vec<Resource>> {
        debug!("Listing resources from server '{}'", server_name);

        let clients = self.clients.read().await;
        let connection = clients
            .get(server_name)
            .ok_or_else(|| anyhow!("Server '{}' not found", server_name))?;

        let result = connection
            .peer
            .list_all_resources()
            .await
            .map_err(|e| anyhow!("Failed to list resources from '{}': {:?}", server_name, e))?;

        Ok(result)
    }

    /// Read a resource from a server
    pub async fn read_resource(&self, server_name: &str, uri: &str) -> Result<String> {
        debug!("Reading resource '{}' from server '{}'", uri, server_name);

        let clients = self.clients.read().await;
        let connection = clients
            .get(server_name)
            .ok_or_else(|| anyhow!("Server '{}' not found", server_name))?;

        let result = connection
            .peer
            .read_resource(ReadResourceRequestParam {
                uri: uri.to_string(),
            })
            .await
            .map_err(|e| anyhow!("Failed to read resource '{}' from '{}': {:?}", uri, server_name, e))?;

        // Extract content from first item
        if let Some(content_item) = result.contents.first() {
            match content_item {
                ResourceContents::TextResourceContents { text, .. } => Ok(text.clone()),
                ResourceContents::BlobResourceContents { blob, .. } => {
                    Ok(format!("[Binary blob: {}]", blob))
                }
            }
        } else {
            Ok(String::new())
        }
    }

    /// List prompts from a server
    pub async fn list_prompts(&self, server_name: &str) -> Result<Vec<Prompt>> {
        debug!("Listing prompts from server '{}'", server_name);

        let clients = self.clients.read().await;
        let connection = clients
            .get(server_name)
            .ok_or_else(|| anyhow!("Server '{}' not found", server_name))?;

        let result = connection
            .peer
            .list_all_prompts()
            .await
            .map_err(|e| anyhow!("Failed to list prompts from '{}': {:?}", server_name, e))?;

        Ok(result)
    }

    /// Get a prompt from a server
    pub async fn get_prompt(
        &self,
        server_name: &str,
        name: &str,
        arguments: Option<Value>,
    ) -> Result<GetPromptResult> {
        debug!("Getting prompt '{}' from server '{}'", name, server_name);

        let clients = self.clients.read().await;
        let connection = clients
            .get(server_name)
            .ok_or_else(|| anyhow!("Server '{}' not found", server_name))?;

        let args = arguments.and_then(|v| {
            if let Value::Object(map) = v {
                Some(map)
            } else {
                None
            }
        });

        let result = connection
            .peer
            .get_prompt(GetPromptRequestParam {
                name: name.to_string(),
                arguments: args,
            })
            .await
            .map_err(|e| anyhow!("Failed to get prompt '{}' from '{}': {:?}", name, server_name, e))?;

        Ok(result)
    }

    /// Shutdown a specific server
    pub async fn shutdown_server(&self, server_name: &str) -> Result<()> {
        info!("Shutting down MCP server: {}", server_name);

        let mut clients = self.clients.write().await;
        if let Some(_connection) = clients.remove(server_name) {
            info!("Server '{}' shut down successfully", server_name);
            Ok(())
        } else {
            Err(anyhow!("Server '{}' not found", server_name))
        }
    }

    /// Shutdown all servers
    pub async fn shutdown_all(&self) -> Result<()> {
        info!("Shutting down all MCP servers");

        let mut clients = self.clients.write().await;
        let count = clients.len();
        clients.clear();

        info!("Shut down {} MCP server(s)", count);
        Ok(())
    }

    /// Get list of connected server names
    pub async fn list_servers(&self) -> Vec<String> {
        let clients = self.clients.read().await;
        clients.keys().cloned().collect()
    }

    /// Check if a server is connected
    pub async fn is_connected(&self, server_name: &str) -> bool {
        let clients = self.clients.read().await;
        clients.contains_key(server_name)
    }

    /// Load and start servers from a configuration file
    pub async fn start_from_config<P: AsRef<Path>>(&self, config_path: P) -> Result<Vec<(String, Result<()>)>> {
        let config = MCPServersConfig::load_from_file(config_path)?;
        
        let mut results = Vec::new();
        
        for (server_id, server_config) in config.enabled_servers() {
            info!(
                "Starting MCP server '{}' ({})", 
                server_config.name, 
                server_config.description
            );
            
            let result = self.start_server_from_config(server_id, server_config).await;
            results.push((server_id.clone(), result));
        }
        
        Ok(results)
    }

    /// Start a single server from a ServerConfig
    pub async fn start_server_from_config(
        &self,
        server_id: &str,
        config: &ServerConfig,
    ) -> Result<()> {
        let mut cmd = StdCommand::new(&config.command);
        cmd.args(&config.args);
        
        // Set working directory if specified
        if let Some(working_dir) = &config.working_dir {
            cmd.current_dir(working_dir);
        }
        
        // Set environment variables if specified
        for (key, value) in &config.env {
            cmd.env(key, value);
            debug!("Setting env var for '{}': {}={}", server_id, key, value);
        }
        
        // Set up stdio
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::inherit());
        
        self.start_server(server_id, cmd).await
    }

    /// Start all enabled servers from the default configuration
    pub async fn start_default_servers(&self) -> Result<Vec<(String, Result<()>)>> {
        // Try multiple strategies to find the config file
        let config_path = Self::find_config_file();

        if !config_path.exists() {
            warn!(
                "MCP server configuration not found at: {}",
                config_path.display()
            );
            warn!("Tried paths: current_dir/hainet-persona/mcp-servers.toml, home/.hainet/mcp-servers.toml, /home/tom/hai/hainet-persona/mcp-servers.toml");
            return Ok(Vec::new());
        }

        info!("Using MCP config from: {}", config_path.display());
        self.start_from_config(config_path).await
    }

    /// Find MCP server configuration file using multiple strategies
    fn find_config_file() -> PathBuf {
        // Strategy 1: Current directory (for running via cargo run from project root)
        let current_dir_path = std::env::current_dir()
            .unwrap_or_default()
            .join("hainet-persona")
            .join("mcp-servers.toml");
        if current_dir_path.exists() {
            return current_dir_path;
        }

        // Strategy 2: Home directory ~/.hainet/mcp-servers.toml (for installed binaries)
        if let Some(home) = dirs::home_dir() {
            let home_path = home.join(".hainet").join("mcp-servers.toml");
            if home_path.exists() {
                return home_path;
            }
        }

        // Strategy 3: Hardcoded project path (fallback for development)
        let project_path = PathBuf::from("/home/tom/hai/hainet-persona/mcp-servers.toml");
        if project_path.exists() {
            return project_path;
        }

        // Strategy 4: Environment variable override
        if let Ok(custom_path) = std::env::var("HAINET_MCP_CONFIG") {
            let env_path = PathBuf::from(custom_path);
            if env_path.exists() {
                return env_path;
            }
        }

        // Default to current_dir strategy for error reporting
        current_dir_path
    }
}

impl Default for MCPClientManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for MCPClientManager {
    fn drop(&mut self) {
        debug!("MCPClientManager dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_manager_creation() {
        let manager = MCPClientManager::new();
        assert_eq!(manager.list_servers().await.len(), 0);
    }

    #[tokio::test]
    async fn test_server_connection_check() {
        let manager = MCPClientManager::new();
        assert!(!manager.is_connected("test-server").await);
    }
}
