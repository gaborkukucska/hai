// START OF FILE hainet-persona/src/ai_providers/providers/ollama.rs

//! Ollama HTTP client implementation
//!
//! Ollama is a popular local LLM inference server that runs on localhost:11434.
//! This client implements the Ollama REST API for model listing and inference.
//!
//! API Documentation: https://github.com/ollama/ollama/blob/main/docs/api.md

use super::{GenerationOptions, GenerationResponse, ModelInfo, ProviderClient, ProviderType};
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{debug, warn};

/// Ollama client for local inference
#[derive(Clone)]
pub struct OllamaClient {
    base_url: String,
    client: Client,
}

impl OllamaClient {
    /// Create new Ollama client
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: Client::new(),
        }
    }

    /// Create client with default localhost endpoint
    pub fn localhost() -> Self {
        Self::new("http://localhost:11434".to_string())
    }

    /// Get endpoint URL
    pub fn endpoint(&self) -> &str {
        &self.base_url
    }
}

#[async_trait::async_trait]
impl ProviderClient for OllamaClient {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Ollama
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/api/tags", self.base_url);
        debug!("Fetching Ollama models from {}", url);

        let response = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .context("Failed to connect to Ollama")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Ollama API returned error: {}",
                response.status()
            );
        }

        let tags_response: TagsResponse = response
            .json()
            .await
            .context("Failed to parse Ollama response")?;

        let models: Vec<_> = tags_response
            .models
            .into_iter()
            .map(|model| {
                let metadata = serde_json::json!({
                    "digest": model.digest,
                    "modified_at": model.modified_at,
                    "format": model.details.format,
                    "parameter_size": model.details.parameter_size,
                    "quantization_level": model.details.quantization_level,
                });

                ModelInfo {
                    name: model.name,
                    provider_type: ProviderType::Ollama,
                    endpoint: self.base_url.clone(),
                    size_bytes: Some(model.size),
                    context_length: None, // Ollama doesn't expose this in /api/tags
                    family: Some(model.details.family),
                    metadata,
                }
            })
            .collect();

        debug!("Found {} Ollama models", models.len());
        Ok(models)
    }

    async fn generate(
        &self,
        model: &str,
        prompt: &str,
        options: GenerationOptions,
    ) -> Result<GenerationResponse> {
        let url = format!("{}/api/generate", self.base_url);
        debug!("Generating with Ollama model: {}", model);

        let start = Instant::now();

        let request = OllamaGenerateRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            system: options.system,
            stream: false,
            options: Some(OllamaOptions {
                temperature: options.temperature,
                num_predict: options.max_tokens.map(|t| t as i32),
                top_p: options.top_p,
                stop: options.stop,
            }),
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .timeout(std::time::Duration::from_secs(120)) // Longer timeout for inference
            .send()
            .await
            .context("Failed to send generation request to Ollama")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Ollama generation failed with status {}: {}",
                status,
                error_text
            );
        }

        let generate_response: OllamaGenerateResponse = response
            .json()
            .await
            .context("Failed to parse Ollama generation response")?;

        let latency_ms = start.elapsed().as_millis() as u64;

        debug!(
            "Generation complete in {}ms ({} tokens)",
            latency_ms,
            generate_response.eval_count.unwrap_or(0)
        );

        Ok(GenerationResponse {
            text: generate_response.response,
            model: model.to_string(),
            tokens_generated: generate_response.eval_count,
            latency_ms,
            finish_reason: Some(if generate_response.done {
                "stop".to_string()
            } else {
                "length".to_string()
            }),
        })
    }

    async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/api/tags", self.base_url);

        match self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(response) => Ok(response.status().is_success()),
            Err(e) => {
                warn!("Ollama health check failed: {}", e);
                Ok(false)
            }
        }
    }
}

// Ollama API Types

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
    modified_at: String,
    size: u64,
    digest: String,
    details: OllamaModelDetails,
}

#[derive(Debug, Deserialize)]
struct OllamaModelDetails {
    format: String,
    family: String,
    parameter_size: String,
    quantization_level: String,
}

#[derive(Debug, Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    response: String,
    done: bool,
    #[serde(default)]
    eval_count: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_client_creation() {
        let client = OllamaClient::localhost();
        assert_eq!(client.endpoint(), "http://localhost:11434");
    }

    #[test]
    fn test_ollama_client_custom_endpoint() {
        let client = OllamaClient::new("http://192.168.1.100:11434".to_string());
        assert_eq!(client.endpoint(), "http://192.168.1.100:11434");
    }

    #[test]
    fn test_provider_type() {
        let client = OllamaClient::localhost();
        assert_eq!(client.provider_type(), ProviderType::Ollama);
    }

    #[tokio::test]
    async fn test_generation_options_default() {
        let options = GenerationOptions::default();
        assert_eq!(options.temperature, Some(0.7));
        assert_eq!(options.max_tokens, Some(512));
        assert_eq!(options.top_p, Some(0.9));
    }
}
