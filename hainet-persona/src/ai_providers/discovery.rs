// START OF FILE hainet-persona/src/ai_providers/discovery.rs

//! AI Provider Discovery System
//!
//! Automatically scans localhost and local network for AI provider APIs including:
//! - Ollama (port 11434)
//! - vLLM (port 8000)
//! - LiteLLM (port 4000)
//! - OpenAI-compatible endpoints
//!
//! Uses port scanning for localhost and mDNS/Zeroconf for LAN discovery.

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

/// Discovered AI provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredProvider {
    pub provider_type: ProviderType,
    pub endpoint: String,
    pub available: bool,
    pub latency_ms: u64,
    pub models: Vec<String>,
}

/// Types of AI providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderType {
    Ollama,
    VLLM,
    LiteLLM,
    OpenAICompatible,
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderType::Ollama => write!(f, "Ollama"),
            ProviderType::VLLM => write!(f, "vLLM"),
            ProviderType::LiteLLM => write!(f, "LiteLLM"),
            ProviderType::OpenAICompatible => write!(f, "OpenAI-Compatible"),
        }
    }
}

/// Provider discovery scanner
pub struct ProviderDiscovery {
    client: Client,
    localhost_ports: Vec<(ProviderType, u16)>,
}

impl ProviderDiscovery {
    /// Create new discovery scanner
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to create HTTP client");

        // Common ports for AI providers
        let localhost_ports = vec![
            (ProviderType::Ollama, 11434),
            (ProviderType::VLLM, 8000),
            (ProviderType::LiteLLM, 4000),
            (ProviderType::OpenAICompatible, 8080),
        ];

        Self {
            client,
            localhost_ports,
        }
    }

    /// Scan all providers (localhost + LAN)
    pub async fn scan_all(&self) -> Result<Vec<DiscoveredProvider>> {
        info!("Starting provider discovery scan");

        let mut providers = Vec::new();

        // Scan localhost
        let localhost_providers = self.scan_localhost().await?;
        providers.extend(localhost_providers);

        // TODO: Scan LAN with mDNS (defer to integration phase)
        // let lan_providers = self.scan_lan().await?;
        // providers.extend(lan_providers);

        info!("Discovery complete: {} providers found", providers.len());

        Ok(providers)
    }

    /// Scan localhost ports for AI providers
    async fn scan_localhost(&self) -> Result<Vec<DiscoveredProvider>> {
        info!("Scanning localhost for AI providers");

        let mut providers = Vec::new();

        for (provider_type, port) in &self.localhost_ports {
            let endpoint = format!("http://localhost:{}", port);
            
            debug!("Checking {} at {}", provider_type, endpoint);

            match self.probe_provider(*provider_type, &endpoint).await {
                Ok(provider) => {
                    if provider.available {
                        info!("✓ Found {} at {} ({} models)", 
                            provider_type, endpoint, provider.models.len());
                        providers.push(provider);
                    }
                }
                Err(e) => {
                    debug!("✗ {} not available at {}: {}", provider_type, endpoint, e);
                }
            }
        }

        Ok(providers)
    }

    /// Probe a specific provider endpoint
    async fn probe_provider(
        &self,
        provider_type: ProviderType,
        endpoint: &str,
    ) -> Result<DiscoveredProvider> {
        let start = std::time::Instant::now();

        let available = match provider_type {
            ProviderType::Ollama => self.probe_ollama(endpoint).await?,
            ProviderType::VLLM => self.probe_vllm(endpoint).await?,
            ProviderType::LiteLLM => self.probe_litellm(endpoint).await?,
            ProviderType::OpenAICompatible => self.probe_openai_compat(endpoint).await?,
        };

        let latency_ms = start.elapsed().as_millis() as u64;

        let models = if available {
            self.fetch_models_from_provider(provider_type, endpoint).await?
        } else {
            Vec::new()
        };

        Ok(DiscoveredProvider {
            provider_type,
            endpoint: endpoint.to_string(),
            available,
            latency_ms,
            models,
        })
    }

    /// Probe Ollama endpoint
    async fn probe_ollama(&self, endpoint: &str) -> Result<bool> {
        let url = format!("{}/api/tags", endpoint);
        
        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Probe vLLM endpoint
    async fn probe_vllm(&self, endpoint: &str) -> Result<bool> {
        let url = format!("{}/v1/models", endpoint);
        
        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Probe LiteLLM endpoint
    async fn probe_litellm(&self, endpoint: &str) -> Result<bool> {
        let url = format!("{}/v1/models", endpoint);
        
        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Probe OpenAI-compatible endpoint
    async fn probe_openai_compat(&self, endpoint: &str) -> Result<bool> {
        let url = format!("{}/v1/models", endpoint);
        
        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Fetch models from discovered provider
    pub async fn fetch_models(&self, provider: &DiscoveredProvider) -> Result<Vec<ModelInfo>> {
        let models = match provider.provider_type {
            ProviderType::Ollama => self.fetch_ollama_models(&provider.endpoint).await?,
            ProviderType::VLLM => self.fetch_vllm_models(&provider.endpoint).await?,
            ProviderType::LiteLLM => self.fetch_litellm_models(&provider.endpoint).await?,
            ProviderType::OpenAICompatible => {
                self.fetch_openai_compat_models(&provider.endpoint).await?
            }
        };

        Ok(models)
    }

    /// Fetch models from provider (internal helper)
    async fn fetch_models_from_provider(
        &self,
        provider_type: ProviderType,
        endpoint: &str,
    ) -> Result<Vec<String>> {
        match provider_type {
            ProviderType::Ollama => {
                let models = self.fetch_ollama_models(endpoint).await?;
                Ok(models.iter().map(|m| m.name.clone()).collect())
            }
            ProviderType::VLLM => {
                let models = self.fetch_vllm_models(endpoint).await?;
                Ok(models.iter().map(|m| m.name.clone()).collect())
            }
            ProviderType::LiteLLM => {
                let models = self.fetch_litellm_models(endpoint).await?;
                Ok(models.iter().map(|m| m.name.clone()).collect())
            }
            ProviderType::OpenAICompatible => {
                let models = self.fetch_openai_compat_models(endpoint).await?;
                Ok(models.iter().map(|m| m.name.clone()).collect())
            }
        }
    }

    /// Fetch models from Ollama
    async fn fetch_ollama_models(&self, endpoint: &str) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/api/tags", endpoint);
        let response = self.client.get(&url).send().await?;
        
        #[derive(Deserialize)]
        struct OllamaResponse {
            models: Vec<OllamaModel>,
        }

        #[derive(Deserialize)]
        struct OllamaModel {
            name: String,
            size: u64,
            #[serde(default)]
            _details: Option<OllamaDetails>,
        }

        #[derive(Deserialize)]
        struct OllamaDetails {
            #[serde(default)]
            _parameter_size: Option<String>,
        }

        let data: OllamaResponse = response.json().await?;

        Ok(data
            .models
            .into_iter()
            .map(|m| {
                let size_gb = m.size as f32 / 1_000_000_000.0;
                ModelInfo {
                    name: m.name,
                    provider_type: ProviderType::Ollama,
                    endpoint: endpoint.to_string(),
                    size_gb,
                    context_length: 4096, // Default, will be refined
                    specialization: ModelSpecialization::General,
                }
            })
            .collect())
    }

    /// Fetch models from vLLM
    async fn fetch_vllm_models(&self, endpoint: &str) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/v1/models", endpoint);
        let response = self.client.get(&url).send().await?;

        #[derive(Deserialize)]
        struct VLLMResponse {
            data: Vec<VLLMModel>,
        }

        #[derive(Deserialize)]
        struct VLLMModel {
            id: String,
        }

        let data: VLLMResponse = response.json().await?;

        Ok(data
            .data
            .into_iter()
            .map(|m| ModelInfo {
                name: m.id,
                provider_type: ProviderType::VLLM,
                endpoint: endpoint.to_string(),
                size_gb: 0.0, // Not provided by vLLM API
                context_length: 4096,
                specialization: ModelSpecialization::General,
            })
            .collect())
    }

    /// Fetch models from LiteLLM
    async fn fetch_litellm_models(&self, endpoint: &str) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/v1/models", endpoint);
        let response = self.client.get(&url).send().await?;

        #[derive(Deserialize)]
        struct LiteLLMResponse {
            data: Vec<LiteLLMModel>,
        }

        #[derive(Deserialize)]
        struct LiteLLMModel {
            id: String,
        }

        let data: LiteLLMResponse = response.json().await?;

        Ok(data
            .data
            .into_iter()
            .map(|m| ModelInfo {
                name: m.id,
                provider_type: ProviderType::LiteLLM,
                endpoint: endpoint.to_string(),
                size_gb: 0.0,
                context_length: 4096,
                specialization: ModelSpecialization::General,
            })
            .collect())
    }

    /// Fetch models from OpenAI-compatible endpoint
    async fn fetch_openai_compat_models(&self, endpoint: &str) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/v1/models", endpoint);
        let response = self.client.get(&url).send().await?;

        #[derive(Deserialize)]
        struct OpenAIResponse {
            data: Vec<OpenAIModel>,
        }

        #[derive(Deserialize)]
        struct OpenAIModel {
            id: String,
        }

        let data: OpenAIResponse = response.json().await?;

        Ok(data
            .data
            .into_iter()
            .map(|m| ModelInfo {
                name: m.id,
                provider_type: ProviderType::OpenAICompatible,
                endpoint: endpoint.to_string(),
                size_gb: 0.0,
                context_length: 4096,
                specialization: ModelSpecialization::General,
            })
            .collect())
    }
}

/// Model information from discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub provider_type: ProviderType,
    pub endpoint: String,
    pub size_gb: f32,
    pub context_length: usize,
    pub specialization: ModelSpecialization,
}

/// Model specialization types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelSpecialization {
    General,
    Code,
    Math,
    Safety,
    Reasoning,
    Creative,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_discovery_creation() {
        let discovery = ProviderDiscovery::new();
        assert_eq!(discovery.localhost_ports.len(), 4);
    }

    #[tokio::test]
    async fn test_provider_type_display() {
        assert_eq!(ProviderType::Ollama.to_string(), "Ollama");
        assert_eq!(ProviderType::VLLM.to_string(), "vLLM");
    }
}
