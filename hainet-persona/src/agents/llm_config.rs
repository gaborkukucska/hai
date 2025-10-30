//! # START OF FILE hainet-persona/src/agents/llm_config.rs
//! LLM Configuration System for HAI-Net Agents
//! 
//! Provides per-agent-type configuration for LLM provider selection,
//! generation parameters, and prompt assembly options.

use serde::{Deserialize, Serialize};
use crate::agents::AgentType;

/// Provider preference strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderPreference {
    /// Prefer local providers (Ollama, vLLM) - default
    LocalFirst,
    /// Use cloud providers as fallback
    CloudFallback,
    /// Hybrid approach (best available)
    Hybrid,
}

impl Default for ProviderPreference {
    fn default() -> Self {
        Self::LocalFirst
    }
}

/// Model size preference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelSize {
    /// 1B parameter models
    #[serde(rename = "1b")]
    OneB,
    /// 4B parameter models (default)
    #[serde(rename = "4b")]
    FourB,
    /// 7B parameter models
    #[serde(rename = "7b")]
    SevenB,
    /// 14B+ parameter models
    #[serde(rename = "14b+")]
    FourteenBPlus,
}

impl Default for ModelSize {
    fn default() -> Self {
        Self::FourB
    }
}

/// Quantization level preference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quantization {
    /// 4-bit quantization (default, good balance)
    #[serde(rename = "q4_0")]
    Q4_0,
    /// 5-bit quantization (better quality)
    #[serde(rename = "q5_0")]
    Q5_0,
    /// 8-bit quantization (high quality)
    #[serde(rename = "q8_0")]
    Q8_0,
    /// Float16 (full quality)
    #[serde(rename = "f16")]
    F16,
}

impl Default for Quantization {
    fn default() -> Self {
        Self::Q4_0
    }
}

/// Complete LLM configuration for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLLMConfig {
    /// Provider selection strategy
    pub provider_preference: ProviderPreference,
    
    /// Preferred model size
    pub model_size_preference: ModelSize,
    
    /// Quantization level
    pub quantization: Quantization,
    
    /// Generation parameters
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: Option<u32>,
    pub repeat_penalty: f32,
    pub max_tokens: u32,
    
    /// System prompt assembly options
    pub include_tool_feedback: bool,
    pub include_syntax_examples: bool,
    pub include_json_schema: bool,
    
    /// Performance tracking
    pub track_metrics: bool,
    pub optimization_enabled: bool,
}

impl AgentLLMConfig {
    /// Configuration for Admin AI agent
    /// - Higher temperature for creative conversation
    /// - Larger token budget for complex responses
    /// - Tool feedback enabled
    pub fn for_admin() -> Self {
        Self {
            provider_preference: ProviderPreference::LocalFirst,
            model_size_preference: ModelSize::FourB,
            quantization: Quantization::Q4_0,
            temperature: 0.7,
            top_p: 0.95,
            top_k: Some(40),
            repeat_penalty: 1.1,
            max_tokens: 4096,
            include_tool_feedback: true,
            include_syntax_examples: true,
            include_json_schema: true,
            track_metrics: true,
            optimization_enabled: true,
        }
    }
    
    /// Configuration for PM Agent
    /// - Lower temperature for structured output
    /// - JSON schema always included
    /// - Syntax examples enabled (task decomposition is structured)
    pub fn for_pm() -> Self {
        Self {
            provider_preference: ProviderPreference::LocalFirst,
            model_size_preference: ModelSize::FourB,
            quantization: Quantization::Q4_0,
            temperature: 0.3,
            top_p: 0.9,
            top_k: Some(40),
            repeat_penalty: 1.1,
            max_tokens: 2048,
            include_tool_feedback: true,
            include_syntax_examples: true,
            include_json_schema: true,
            track_metrics: true,
            optimization_enabled: true,
        }
    }
    
    /// Configuration for Worker Agent
    /// - Very low temperature for deterministic execution
    /// - Tool feedback critical for task execution
    /// - Smaller token budget (focused tasks)
    pub fn for_worker() -> Self {
        Self {
            provider_preference: ProviderPreference::LocalFirst,
            model_size_preference: ModelSize::FourB,
            quantization: Quantization::Q4_0,
            temperature: 0.1,
            top_p: 0.8,
            top_k: Some(40),
            repeat_penalty: 1.15,
            max_tokens: 1024,
            include_tool_feedback: true,
            include_syntax_examples: false,
            include_json_schema: false,
            track_metrics: true,
            optimization_enabled: true,
        }
    }
    
    /// Configuration for Guardian agent
    /// - Low temperature for analytical consistency
    /// - Larger model for better reasoning
    /// - No tool feedback (Guardian is oversight, not execution)
    pub fn for_guardian() -> Self {
        Self {
            provider_preference: ProviderPreference::LocalFirst,
            model_size_preference: ModelSize::SevenB,
            quantization: Quantization::Q4_0,
            temperature: 0.2,
            top_p: 0.9,
            top_k: Some(40),
            repeat_penalty: 1.1,
            max_tokens: 2048,
            include_tool_feedback: false,
            include_syntax_examples: false,
            include_json_schema: true,
            track_metrics: true,
            optimization_enabled: true,
        }
    }
    
    /// Get configuration for specific agent type
    pub fn for_agent_type(agent_type: AgentType) -> Self {
        match agent_type {
            AgentType::User => Self::for_admin(), // Users don't have LLM config, fallback to admin
            AgentType::Admin => Self::for_admin(),
            AgentType::PM => Self::for_pm(),
            AgentType::Worker => Self::for_worker(),
            AgentType::Guardian => Self::for_guardian(),
        }
    }
    
    /// Merge with user overrides from config file
    pub fn merge_with(&mut self, overrides: &AgentLLMConfigOverrides) {
        if let Some(temp) = overrides.temperature {
            self.temperature = temp;
        }
        if let Some(top_p) = overrides.top_p {
            self.top_p = top_p;
        }
        if let Some(top_k) = overrides.top_k {
            self.top_k = Some(top_k);
        }
        if let Some(repeat_penalty) = overrides.repeat_penalty {
            self.repeat_penalty = repeat_penalty;
        }
        if let Some(max_tokens) = overrides.max_tokens {
            self.max_tokens = max_tokens;
        }
        if let Some(provider) = overrides.provider_preference {
            self.provider_preference = provider;
        }
        if let Some(size) = overrides.model_size_preference {
            self.model_size_preference = size;
        }
        if let Some(quant) = overrides.quantization {
            self.quantization = quant;
        }
        if let Some(tool_fb) = overrides.include_tool_feedback {
            self.include_tool_feedback = tool_fb;
        }
        if let Some(syntax_ex) = overrides.include_syntax_examples {
            self.include_syntax_examples = syntax_ex;
        }
        if let Some(json_schema) = overrides.include_json_schema {
            self.include_json_schema = json_schema;
        }
        if let Some(track) = overrides.track_metrics {
            self.track_metrics = track;
        }
        if let Some(opt) = overrides.optimization_enabled {
            self.optimization_enabled = opt;
        }
    }
}

/// User-provided overrides for AgentLLMConfig
/// All fields are optional - only provided values override defaults
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentLLMConfigOverrides {
    pub provider_preference: Option<ProviderPreference>,
    pub model_size_preference: Option<ModelSize>,
    pub quantization: Option<Quantization>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    pub repeat_penalty: Option<f32>,
    pub max_tokens: Option<u32>,
    pub include_tool_feedback: Option<bool>,
    pub include_syntax_examples: Option<bool>,
    pub include_json_schema: Option<bool>,
    pub track_metrics: Option<bool>,
    pub optimization_enabled: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_admin_config_defaults() {
        let config = AgentLLMConfig::for_admin();
        assert_eq!(config.temperature, 0.7);
        assert_eq!(config.max_tokens, 4096);
        assert!(config.include_tool_feedback);
        assert!(config.track_metrics);
    }
    
    #[test]
    fn test_pm_config_defaults() {
        let config = AgentLLMConfig::for_pm();
        assert_eq!(config.temperature, 0.3);
        assert_eq!(config.max_tokens, 2048);
        assert!(config.include_json_schema);
    }
    
    #[test]
    fn test_worker_config_defaults() {
        let config = AgentLLMConfig::for_worker();
        assert_eq!(config.temperature, 0.1);
        assert_eq!(config.max_tokens, 1024);
        assert!(config.include_tool_feedback);
        assert!(!config.include_syntax_examples);
    }
    
    #[test]
    fn test_guardian_config_defaults() {
        let config = AgentLLMConfig::for_guardian();
        assert_eq!(config.temperature, 0.2);
        assert_eq!(config.model_size_preference, ModelSize::SevenB);
        assert!(!config.include_tool_feedback);
    }
    
    #[test]
    fn test_config_merge() {
        let mut config = AgentLLMConfig::for_admin();
        let overrides = AgentLLMConfigOverrides {
            temperature: Some(0.5),
            max_tokens: Some(8192),
            ..Default::default()
        };
        
        config.merge_with(&overrides);
        
        assert_eq!(config.temperature, 0.5);
        assert_eq!(config.max_tokens, 8192);
        // Other values unchanged
        assert_eq!(config.top_p, 0.95);
    }
    
    #[test]
    fn test_for_agent_type() {
        let admin_config = AgentLLMConfig::for_agent_type(AgentType::Admin);
        assert_eq!(admin_config.temperature, 0.7);
        
        let pm_config = AgentLLMConfig::for_agent_type(AgentType::PM);
        assert_eq!(pm_config.temperature, 0.3);
        
        let worker_config = AgentLLMConfig::for_agent_type(AgentType::Worker);
        assert_eq!(worker_config.temperature, 0.1);
    }
}
