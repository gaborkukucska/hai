// START OF FILE hainet-persona/src/ai_providers/providers/mod.rs

//! Provider-specific client implementations
//!
//! This module contains HTTP clients for different AI provider APIs:
//! - Ollama: Local LLM server (localhost:11434)
//! - vLLM: High-performance inference server
//! - LiteLLM: Unified proxy for multiple providers
//! - OpenAI-compatible: Generic OpenAI API implementation

pub mod ollama;

pub use ollama::OllamaClient;

use serde::{Deserialize, Serialize};
use std::fmt;

/// Type of AI provider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderType {
    Ollama,
    VLlm,
    LiteLlm,
    OpenAiCompat,
}

impl fmt::Display for ProviderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderType::Ollama => write!(f, "Ollama"),
            ProviderType::VLlm => write!(f, "vLLM"),
            ProviderType::LiteLlm => write!(f, "LiteLLM"),
            ProviderType::OpenAiCompat => write!(f, "OpenAI-Compatible"),
        }
    }
}

/// Basic model information from provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub provider_type: ProviderType,
    pub endpoint: String,
    pub size_bytes: Option<u64>,
    pub context_length: Option<usize>,
    pub family: Option<String>,
    pub metadata: serde_json::Value,
}

/// Trait for provider-specific clients
#[async_trait::async_trait]
pub trait ProviderClient: Send + Sync {
    /// Get provider type
    fn provider_type(&self) -> ProviderType;
    
    /// List available models
    async fn list_models(&self) -> anyhow::Result<Vec<ModelInfo>>;
    
    /// Generate completion (non-streaming)
    async fn generate(
        &self,
        model: &str,
        prompt: &str,
        options: GenerationOptions,
    ) -> anyhow::Result<GenerationResponse>;
    
    /// Check if provider is available
    async fn health_check(&self) -> anyhow::Result<bool>;
}

/// Options for text generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationOptions {
    pub temperature: Option<f32>,
    pub max_tokens: Option<usize>,
    pub top_p: Option<f32>,
    pub stop: Option<Vec<String>>,
    pub system: Option<String>,
    pub num_ctx: Option<usize>,
}

impl Default for GenerationOptions {
    fn default() -> Self {
        Self {
            temperature: Some(0.7),
            max_tokens: Some(512),
            top_p: Some(0.9),
            stop: None,
            system: None,
            num_ctx: None,
        }
    }
}

/// Response from text generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResponse {
    pub text: String,
    pub model: String,
    pub tokens_generated: Option<usize>,
    pub latency_ms: u64,
    pub finish_reason: Option<String>,
}
