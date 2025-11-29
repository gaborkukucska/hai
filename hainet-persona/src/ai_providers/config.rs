//! Load Balancing Configuration Loader
//! 
//! Loads Ollama endpoint configuration from ollama-endpoints.toml

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use super::request_queue::LoadBalancingStrategy;

/// Complete configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    #[serde(default)]
    pub load_balancing: LoadBalancingConfig,
    #[serde(default)]
    pub endpoints: HashMap<String, EndpointConfig>,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        let mut endpoints = HashMap::new();
        endpoints.insert(
            "primary".to_string(),
            EndpointConfig {
                url: "http://localhost:11434".to_string(),
                max_concurrent: 3,
            },
        );
        
        Self {
            load_balancing: LoadBalancingConfig::default(),
            endpoints,
        }
    }
}

/// Load balancing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancingConfig {
    #[serde(default = "default_strategy")]
    pub strategy: String,
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,
    #[serde(default = "default_health_check_interval_secs")]
    pub health_check_interval_secs: u64,
}

fn default_strategy() -> String {
    "LeastLoaded".to_string()
}

fn default_request_timeout_secs() -> u64 {
    120
}

fn default_health_check_interval_secs() -> u64 {
    30
}

impl Default for LoadBalancingConfig {
    fn default() -> Self {
        Self {
            strategy: default_strategy(),
            request_timeout_secs: default_request_timeout_secs(),
            health_check_interval_secs: default_health_check_interval_secs(),
        }
    }
}

/// Individual endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointConfig {
    pub url: String,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
}

fn default_max_concurrent() -> usize {
    3
}

impl OllamaConfig {
    /// Load configuration from TOML file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(&path)
            .context(format!(
                "Failed to read configuration file: {}",
                path.as_ref().display()
            ))?;
        
        let config: OllamaConfig = toml::from_str(&content)
            .context("Failed to parse TOML configuration")?;
        
        tracing::info!(
            "Loaded Ollama configuration from {} ({} endpoints)",
            path.as_ref().display(),
            config.endpoints.len()
        );
        
        Ok(config)
    }
    
    /// Load configuration with fallback to defaults
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        if !path.as_ref().exists() {
            tracing::info!(
                "Configuration file {} not found. Using defaults.",
                path.as_ref().display()
            );
            return Self::default();
        }

        match Self::load_from_file(&path) {
            Ok(config) => config,
            Err(e) => {
                tracing::warn!(
                    "Failed to load configuration from {}: {:?}. Using defaults.",
                    path.as_ref().display(),
                    e
                );
                Self::default()
            }
        }
    }
    
    /// Parse load balancing strategy from string
    pub fn parse_strategy(&self) -> LoadBalancingStrategy {
        match self.load_balancing.strategy.as_str() {
            "RoundRobin" => LoadBalancingStrategy::RoundRobin,
            "ModelAffinity" => LoadBalancingStrategy::ModelAffinity,
            _ => LoadBalancingStrategy::LeastLoaded, // default
        }
    }
    
    /// Get request timeout as Duration
    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.load_balancing.request_timeout_secs)
    }
    
    /// Get health check interval as Duration
    pub fn health_check_interval(&self) -> Duration {
        Duration::from_secs(self.load_balancing.health_check_interval_secs)
    }
    
    /// Get primary endpoint URL
    pub fn primary_endpoint(&self) -> String {
        self.endpoints
            .get("primary")
            .map(|e| e.url.clone())
            .unwrap_or_else(|| "http://localhost:11434".to_string())
    }
    
    /// Get additional endpoints (non-primary)
    pub fn additional_endpoints(&self) -> Vec<String> {
        self.endpoints
            .iter()
            .filter(|(name, _)| *name != "primary")
            .map(|(_, config)| config.url.clone())
            .collect()
    }
    
    /// Get endpoint-specific max concurrent overrides
    pub fn endpoint_overrides(&self) -> HashMap<String, usize> {
        self.endpoints
            .iter()
            .map(|(_, config)| (config.url.clone(), config.max_concurrent))
            .collect()
    }
    
    /// Get default max concurrent requests
    pub fn default_max_concurrent(&self) -> usize {
        self.endpoints
            .get("primary")
            .map(|e| e.max_concurrent)
            .unwrap_or(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = OllamaConfig::default();
        assert_eq!(config.parse_strategy(), LoadBalancingStrategy::LeastLoaded);
        assert_eq!(config.request_timeout().as_secs(), 120);
        assert_eq!(config.primary_endpoint(), "http://localhost:11434");
        assert_eq!(config.additional_endpoints().len(), 0);
    }
    
    #[test]
    fn test_parse_strategy() {
        let mut config = OllamaConfig::default();
        
        config.load_balancing.strategy = "LeastLoaded".to_string();
        assert_eq!(config.parse_strategy(), LoadBalancingStrategy::LeastLoaded);
        
        config.load_balancing.strategy = "RoundRobin".to_string();
        assert_eq!(config.parse_strategy(), LoadBalancingStrategy::RoundRobin);
        
        config.load_balancing.strategy = "ModelAffinity".to_string();
        assert_eq!(config.parse_strategy(), LoadBalancingStrategy::ModelAffinity);
        
        config.load_balancing.strategy = "Invalid".to_string();
        assert_eq!(config.parse_strategy(), LoadBalancingStrategy::LeastLoaded);
    }
}
