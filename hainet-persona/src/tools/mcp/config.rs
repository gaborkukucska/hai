//! # MCP Server Configuration
//!
//! This module handles loading and parsing MCP server configurations from TOML files.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::info;

/// MCP Server Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Display name of the server
    pub name: String,
    
    /// Description of what the server does
    pub description: String,
    
    /// Command to execute to start the server
    pub command: String,
    
    /// Arguments to pass to the command
    #[serde(default)]
    pub args: Vec<String>,
    
    /// Whether this server is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    
    /// Optional working directory for the server process
    #[serde(default)]
    pub working_dir: Option<PathBuf>,
}

fn default_enabled() -> bool {
    true
}

/// Root configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServersConfig {
    /// Map of server ID to server configuration
    pub servers: HashMap<String, ServerConfig>,
}

impl MCPServersConfig {
    /// Load configuration from a TOML file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        info!("Loading MCP server configuration from: {}", path.display());
        
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        
        let config: MCPServersConfig = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
        
        info!("Loaded {} MCP server configurations", config.servers.len());
        Ok(config)
    }
    
    /// Get enabled servers only
    pub fn enabled_servers(&self) -> impl Iterator<Item = (&String, &ServerConfig)> {
        self.servers.iter().filter(|(_, config)| config.enabled)
    }
    
    /// Get a specific server configuration by ID
    pub fn get_server(&self, id: &str) -> Option<&ServerConfig> {
        self.servers.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServerConfig {
            name: "Test Server".to_string(),
            description: "A test server".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "test-server".to_string()],
            enabled: true,
            working_dir: None,
        };
        
        assert_eq!(config.name, "Test Server");
        assert!(config.enabled);
    }
}
