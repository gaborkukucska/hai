//! <!-- # START OF FILE hainet-core/src/config.rs -->
//! Centralized configuration loading for HAI-Net daemons.
//!
//! Supports three loading modes:
//! 1. System installation: `/etc/hainet/hainet.toml`
//! 2. Development mode: `./hainet.toml` (workspace root)
//! 3. Built-in defaults (fallback)
//!
//! Also provides port availability checking for flexible port assignment.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::net::TcpListener;
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Main HAI-Net configuration, deserialized from hainet.toml
#[derive(Debug, Clone, Deserialize)]
pub struct HainetConfig {
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub logs: LogsConfig,
}

/// Network configuration section
#[derive(Debug, Clone, Deserialize)]
pub struct NetworkConfig {
    /// Role: "master", "slave", "standalone"
    #[serde(default = "default_role")]
    pub role: String,
    /// Master node IP (only for slaves)
    pub master_ip: Option<String>,
    /// Primary service port
    #[serde(default = "default_port")]
    pub port: u16,
}

/// Storage configuration section
#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    /// Data directory
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
}

/// Logging configuration section
#[derive(Debug, Clone, Deserialize)]
pub struct LogsConfig {
    /// Log directory
    #[serde(default = "default_log_dir")]
    pub log_dir: String,
    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

// Default value functions for serde
fn default_role() -> String { "standalone".to_string() }
fn default_port() -> u16 { 8080 }
fn default_data_dir() -> String { "/var/lib/hainet/data".to_string() }
fn default_log_dir() -> String { "/var/log/hainet".to_string() }
fn default_log_level() -> String { "info".to_string() }

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            role: default_role(),
            master_ip: None,
            port: default_port(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
        }
    }
}

impl Default for LogsConfig {
    fn default() -> Self {
        Self {
            log_dir: default_log_dir(),
            log_level: default_log_level(),
        }
    }
}

impl Default for HainetConfig {
    fn default() -> Self {
        Self {
            network: NetworkConfig::default(),
            storage: StorageConfig::default(),
            logs: LogsConfig::default(),
        }
    }
}

impl HainetConfig {
    /// Load configuration with automatic source detection.
    ///
    /// Search order:
    /// 1. `/etc/hainet/hainet.toml` (system installation)
    /// 2. `./hainet.toml` (development / workspace root)
    /// 3. Built-in defaults
    pub fn load() -> Self {
        let candidates = vec![
            PathBuf::from("/etc/hainet/hainet.toml"),
            PathBuf::from("hainet.toml"),
        ];

        for path in &candidates {
            if path.exists() {
                debug!("Trying config file: {}", path.display());
                match Self::load_from_file(path) {
                    Ok(config) => {
                        info!("📄 Loaded config from: {}", path.display());
                        return config;
                    }
                    Err(e) => {
                        warn!("⚠  Failed to parse {}: {}", path.display(), e);
                    }
                }
            } else {
                debug!("Config not found at: {}", path.display());
            }
        }

        info!("📄 Using default configuration (no config file found)");
        Self::default()
    }

    /// Load configuration from a specific file path.
    fn load_from_file(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        let config: HainetConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
        Ok(config)
    }

    /// Get a display-friendly role string with emoji
    pub fn role_display(&self) -> String {
        match self.network.role.as_str() {
            "master" => "👑 Master".to_string(),
            "slave" => "⚙  Slave".to_string(),
            "standalone" => "🔹 Standalone".to_string(),
            other => format!("❓ {}", other),
        }
    }

    /// Returns the effective log directory, ensuring it exists.
    pub fn effective_log_dir(&self) -> PathBuf {
        let dir = PathBuf::from(&self.logs.log_dir);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            // If we can't create the configured dir, fall back
            let fallback = std::env::temp_dir().join("hainet-logs");
            warn!("⚠  Cannot create log dir {}: {}. Using {}", dir.display(), e, fallback.display());
            let _ = std::fs::create_dir_all(&fallback);
            return fallback;
        }
        dir
    }
}

/// Check if a TCP port is available for binding on localhost.
pub fn is_port_available(port: u16) -> bool {
    TcpListener::bind(("0.0.0.0", port)).is_ok()
}

/// Find an available TCP port starting from the preferred port.
///
/// If the preferred port is in use, tries incrementing by 1 up to 100 times.
/// Returns the first available port, or None if no port is found.
pub fn find_available_port(preferred: u16) -> Option<u16> {
    for offset in 0..100 {
        let port = preferred + offset;
        if is_port_available(port) {
            if offset > 0 {
                info!("🔌 Port {} in use, using {} instead", preferred, port);
            }
            return Some(port);
        }
    }
    warn!("⚠  No available port found near {}", preferred);
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = HainetConfig::default();
        assert_eq!(config.network.role, "standalone");
        assert_eq!(config.network.port, 8080);
        assert!(config.network.master_ip.is_none());
    }

    #[test]
    fn test_parse_master_config() {
        let toml_str = r#"
[network]
role = "master"
port = 8080

[storage]
data_dir = "/var/lib/hainet/data"

[logs]
log_dir = "/var/log/hainet"
log_level = "info"
"#;
        let config: HainetConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.network.role, "master");
        assert_eq!(config.network.port, 8080);
    }

    #[test]
    fn test_parse_slave_config() {
        let toml_str = r#"
[network]
role = "slave"
master_ip = "10.208.118.178"
port = 8080
"#;
        let config: HainetConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.network.role, "slave");
        assert_eq!(config.network.master_ip.unwrap(), "10.208.118.178");
    }

    #[test]
    fn test_role_display() {
        let mut config = HainetConfig::default();
        config.network.role = "master".to_string();
        assert_eq!(config.role_display(), "👑 Master");

        config.network.role = "slave".to_string();
        assert_eq!(config.role_display(), "⚙  Slave");
    }

    #[test]
    fn test_find_available_port() {
        // Port 0 always works in tests but we'll test the logic
        let port = find_available_port(49152);
        assert!(port.is_some());
    }
}
