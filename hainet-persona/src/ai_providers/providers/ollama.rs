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
use serde_json;
use std::time::Instant;
use tracing::{debug, warn};

/// Ollama client for local inference
#[derive(Clone)]
pub struct OllamaClient {
    base_url: String,
    client: Client,
    state_requested_ctx: usize,
    hardware_max_ctx: usize,
}

impl OllamaClient {
    pub fn new(base_url: String, state_requested_ctx: usize, hardware_max_ctx: usize) -> Self {
        // Configure HTTP client with generous timeouts for LLM generation
        let client = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .build()
            .expect("Failed to build HTTP client");
        
        Self {
            base_url,
            client,
            state_requested_ctx,
            hardware_max_ctx,
        }
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

        // Strip "Ollama::" prefix if present, as the API expects just the model name
        let model_name = if model.starts_with("Ollama::") {
            &model["Ollama::".len()..]
        } else {
            model
        };

        // Determine safe context window dynamically
        let target_ctx = options.num_ctx.unwrap_or(self.state_requested_ctx);
        let safe_ctx = target_ctx.min(self.hardware_max_ctx);
        
        debug!("Ollama generate: num_ctx resolved to {} (target: {}, hardware limit: {})", safe_ctx, target_ctx, self.hardware_max_ctx);

        let request = OllamaRequest {
            model: model_name.to_string(),
            prompt: prompt.to_string(),
            system: options.system,
            stream: false,
            options: Some(OllamaOptions {
                temperature: options.temperature,
                num_predict: options.max_tokens.map(|t| t as i32),
                top_p: options.top_p,
                stop: options.stop,
                num_ctx: Some(safe_ctx as i32),
            }),
            keep_alive: Some("10m".to_string()),
        };

        // Log the full request body for debugging
        if let Ok(request_body) = serde_json::to_string_pretty(&request) {
            debug!(
                target: "llm_messages",
                "[OLLAMA REQUEST] Sending request to {}:\n{}",
                url,
                request_body
            );
        } else {
            warn!("Failed to serialize Ollama request for logging");
        }

        let response = self
            .client
            .post(&url)
            .json(&request)
            .timeout(std::time::Duration::from_secs(300))
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

        let response_text = response.text().await.context("Failed to read Ollama response text")?;

        // Log the full response body for debugging
        debug!(
            target: "llm_messages",
            "[OLLAMA RESPONSE] Raw response from {}:\n{}",
            url,
            response_text
        );

        let generate_response: OllamaResponse = serde_json::from_str(&response_text)
            .context("Failed to parse Ollama generation response from text")?;


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

/// Ollama generation request (exposed for request queue)
#[derive(Debug, Clone, Serialize)]
pub struct OllamaRequest {
    pub model: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<OllamaOptions>,
    /// Keep model loaded in memory for this duration (e.g., "5m", "10m", "1h")
    /// This prevents frequent reloading when multiple requests use the same model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_alive: Option<String>,
}

/// Ollama generation options (exposed for request queue)
#[derive(Debug, Clone, Serialize)]
pub struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_predict: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_ctx: Option<i32>,
}

/// Ollama generation response (exposed for request queue)
#[derive(Debug, Clone, Deserialize)]
pub struct OllamaResponse {
    pub response: String,
    pub done: bool,
    #[serde(default)]
    pub eval_count: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_client_creation() {
        let client = OllamaClient::new("http://localhost:11434".to_string());
        assert_eq!(client.endpoint(), "http://localhost:11434");
    }

    #[test]
    fn test_ollama_client_custom_endpoint() {
        let client = OllamaClient::new("http://192.168.1.100:11434".to_string());
        assert_eq!(client.endpoint(), "http://192.168.1.100:11434");
    }

    #[test]
    fn test_provider_type() {
        let client = OllamaClient::new("http://localhost:11434".to_string());
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
