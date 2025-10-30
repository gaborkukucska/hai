//! # HAI-Net Configuration System
//! 
//! Centralized configuration for all framework defaults.
//! 
//! ## Usage
//! ```rust
//! let config = HaiNetConfig::load_or_default();
//! let model = config.default_models.admin_model;
//! ```

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use crate::agents::llm_config::{AgentLLMConfig, AgentLLMConfigOverrides};
use crate::prompts::AgentType;

/// Main HAI-Net configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HaiNetConfig {
    /// Default models for different agents
    pub default_models: ModelDefaults,
    
    /// LLM generation settings
    pub generation: GenerationDefaults,
    
    /// Retry and reliability settings
    pub reliability: ReliabilityDefaults,
    
    /// Paths and directories
    pub paths: PathDefaults,
    
    /// AI agent configuration (new system)
    #[serde(default)]
    pub ai: AIConfig,
}

impl Default for HaiNetConfig {
    fn default() -> Self {
        Self {
            default_models: ModelDefaults::default(),
            generation: GenerationDefaults::default(),
            reliability: ReliabilityDefaults::default(),
            paths: PathDefaults::default(),
            ai: AIConfig::default(),
        }
    }
}

impl HaiNetConfig {
    /// Load config from file, or use defaults if file doesn't exist
    pub fn load_or_default() -> Self {
        Self::load_from_path(&Self::default_config_path())
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to load config, using defaults: {:?}", e);
                Self::default()
            })
    }
    
    /// Load config from specific path
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .context(format!("Failed to read config from {:?}", path))?;
        
        toml::from_str(&contents)
            .context("Failed to parse config TOML")
    }
    
    /// Save config to file
    pub fn save(&self, path: &Path) -> Result<()> {
        let contents = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;
        
        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        std::fs::write(path, contents)
            .context(format!("Failed to write config to {:?}", path))
    }
    
    /// Save config to default location
    pub fn save_default(&self) -> Result<()> {
        self.save(&Self::default_config_path())
    }
    
    /// Default config file path: ~/.hainet/config.toml
    pub fn default_config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".hainet")
            .join("config.toml")
    }
    
    /// Load from hainet.toml in project root
    pub fn load_from_project_root() -> Result<Self> {
        let project_root = std::env::current_dir()?;
        let config_path = project_root.join("hainet.toml");
        Self::load_from_path(&config_path)
    }
    
    /// Get LLM config for specific agent type with user overrides applied
    pub fn get_agent_llm_config(&self, agent_type: AgentType) -> AgentLLMConfig {
        let mut config = AgentLLMConfig::for_agent_type(agent_type);
        
        // Apply global defaults if provided
        if let Some(ref defaults) = self.ai.defaults {
            config.merge_with(defaults);
        }
        
        // Apply agent-specific overrides
        let overrides = match agent_type {
            AgentType::User => &None, // Users don't have LLM config
            AgentType::Admin => &self.ai.admin,
            AgentType::PM => &self.ai.pm,
            AgentType::Worker => &self.ai.worker,
            AgentType::Guardian => &self.ai.guardian,
        };
        
        if let Some(ref overrides) = overrides {
            config.merge_with(overrides);
        }
        
        config
    }
}

/// AI agent configuration section
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AIConfig {
    /// Global AI defaults (applied to all agents)
    pub defaults: Option<AgentLLMConfigOverrides>,
    
    /// Admin AI specific overrides
    pub admin: Option<AgentLLMConfigOverrides>,
    
    /// PM Agent specific overrides
    pub pm: Option<AgentLLMConfigOverrides>,
    
    /// Worker Agent specific overrides
    pub worker: Option<AgentLLMConfigOverrides>,
    
    /// Guardian Agent specific overrides
    pub guardian: Option<AgentLLMConfigOverrides>,
}

/// Default models for different agent types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDefaults {
    /// Model for Admin AI agent
    /// 
    /// Recommended: gemma3:4b-it-q4_K_M (fast, good instruction following)
    /// Alternative: llama3.2:latest, qwen2.5:7b-instruct
    #[serde(default = "ModelDefaults::default_admin_model")]
    pub admin_model: String,
    
    /// Model for PM agents
    /// 
    /// Recommended: gemma3:4b-it-q4_K_M (task planning)
    #[serde(default = "ModelDefaults::default_pm_model")]
    pub pm_model: String,
    
    /// Model for Worker agents
    /// 
    /// Recommended: gemma3:4b-it-q4_K_M (fast execution)
    #[serde(default = "ModelDefaults::default_worker_model")]
    pub worker_model: String,
    
    /// Model for Guardian system
    /// 
    /// Recommended: gemma3:4b-it-q4_K_M (safety analysis)
    #[serde(default = "ModelDefaults::default_guardian_model")]
    pub guardian_model: String,
    
    /// Fallback models (in order of preference)
    /// 
    /// If primary model fails, try these in order
    #[serde(default = "ModelDefaults::default_fallback_models")]
    pub fallback_models: Vec<String>,
}

impl Default for ModelDefaults {
    fn default() -> Self {
        Self {
            admin_model: Self::default_admin_model(),
            pm_model: Self::default_pm_model(),
            worker_model: Self::default_worker_model(),
            guardian_model: Self::default_guardian_model(),
            fallback_models: Self::default_fallback_models(),
        }
    }
}

impl ModelDefaults {
    fn default_admin_model() -> String {
        "gemma3:4b-it-q4_K_M".to_string()
    }
    
    fn default_pm_model() -> String {
        "gemma3:4b-it-q4_K_M".to_string()
    }
    
    fn default_worker_model() -> String {
        "gemma3:4b-it-q4_K_M".to_string()
    }
    
    fn default_guardian_model() -> String {
        "gemma3:4b-it-q4_K_M".to_string()
    }
    
    fn default_fallback_models() -> Vec<String> {
        vec![
            "gemma3:4b-it-q4_K_M".to_string(),
            "qwen2.5:7b-instruct".to_string(),
            "llama3.2:latest".to_string(),
        ]
    }
}

/// LLM generation defaults
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationDefaults {
    /// Temperature for creative tasks (conversation)
    #[serde(default = "GenerationDefaults::default_creative_temperature")]
    pub creative_temperature: f32,
    
    /// Temperature for structured tasks (planning, JSON)
    #[serde(default = "GenerationDefaults::default_structured_temperature")]
    pub structured_temperature: f32,
    
    /// Max tokens for generation
    #[serde(default = "GenerationDefaults::default_max_tokens")]
    pub max_tokens: usize,
    
    /// Top-p sampling value
    #[serde(default = "GenerationDefaults::default_top_p")]
    pub top_p: f32,
}

impl Default for GenerationDefaults {
    fn default() -> Self {
        Self {
            creative_temperature: Self::default_creative_temperature(),
            structured_temperature: Self::default_structured_temperature(),
            max_tokens: Self::default_max_tokens(),
            top_p: Self::default_top_p(),
        }
    }
}

impl GenerationDefaults {
    fn default_creative_temperature() -> f32 {
        0.8
    }
    
    fn default_structured_temperature() -> f32 {
        0.2
    }
    
    fn default_max_tokens() -> usize {
        1024
    }
    
    fn default_top_p() -> f32 {
        0.9
    }
}

/// Retry and reliability defaults
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReliabilityDefaults {
    /// Maximum number of LLM retries on format errors
    #[serde(default = "ReliabilityDefaults::default_max_llm_retries")]
    pub max_llm_retries: usize,
    
    /// Backoff multiplier for retry delays (ms)
    #[serde(default = "ReliabilityDefaults::default_retry_backoff_ms")]
    pub retry_backoff_ms: u64,
    
    /// Enable progressive prompt simplification
    #[serde(default = "ReliabilityDefaults::default_progressive_prompts")]
    pub progressive_prompts: bool,
    
    /// Enable automatic model fallback on failures
    #[serde(default = "ReliabilityDefaults::default_auto_fallback")]
    pub auto_fallback: bool,
}

impl Default for ReliabilityDefaults {
    fn default() -> Self {
        Self {
            max_llm_retries: Self::default_max_llm_retries(),
            retry_backoff_ms: Self::default_retry_backoff_ms(),
            progressive_prompts: Self::default_progressive_prompts(),
            auto_fallback: Self::default_auto_fallback(),
        }
    }
}

impl ReliabilityDefaults {
    fn default_max_llm_retries() -> usize {
        3
    }
    
    fn default_retry_backoff_ms() -> u64 {
        500
    }
    
    fn default_progressive_prompts() -> bool {
        true
    }
    
    fn default_auto_fallback() -> bool {
        true
    }
}

/// Path defaults
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathDefaults {
    /// Base data directory
    #[serde(default = "PathDefaults::default_data_dir")]
    pub data_dir: PathBuf,
    
    /// Prompts directory
    #[serde(default = "PathDefaults::default_prompts_dir")]
    pub prompts_dir: PathBuf,
    
    /// Database path
    #[serde(default = "PathDefaults::default_database_path")]
    pub database_path: PathBuf,
}

impl Default for PathDefaults {
    fn default() -> Self {
        Self {
            data_dir: Self::default_data_dir(),
            prompts_dir: Self::default_prompts_dir(),
            database_path: Self::default_database_path(),
        }
    }
}

impl PathDefaults {
    fn default_data_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".hainet")
            .join("data")
    }
    
    fn default_prompts_dir() -> PathBuf {
        PathBuf::from("hainet-persona/prompts")
    }
    
    fn default_database_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".hainet")
            .join("data")
            .join("hainet.db")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = HaiNetConfig::default();
        assert_eq!(config.default_models.admin_model, "gemma3:4b-it-q4_K_M");
        assert_eq!(config.reliability.max_llm_retries, 3);
    }
    
    #[test]
    fn test_serialize_deserialize() {
        let config = HaiNetConfig::default();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: HaiNetConfig = toml::from_str(&serialized).unwrap();
        
        assert_eq!(config.default_models.admin_model, deserialized.default_models.admin_model);
    }
    
    #[test]
    fn test_config_path() {
        let path = HaiNetConfig::default_config_path();
        assert!(path.to_string_lossy().contains(".hainet"));
        assert!(path.to_string_lossy().contains("config.toml"));
    }
    
    #[test]
    fn test_get_agent_llm_config() {
        let config = HaiNetConfig::default();
        
        let admin_config = config.get_agent_llm_config(AgentType::Admin);
        assert_eq!(admin_config.temperature, 0.7);
        
        let pm_config = config.get_agent_llm_config(AgentType::PM);
        assert_eq!(pm_config.temperature, 0.3);
        
        let worker_config = config.get_agent_llm_config(AgentType::Worker);
        assert_eq!(worker_config.temperature, 0.1);
    }
}
