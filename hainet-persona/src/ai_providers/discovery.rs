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

    /// Scan all providers (localhost + all accessible subnets)
    pub async fn scan_all(&self) -> Result<Vec<DiscoveredProvider>> {
        info!("Starting provider discovery scan");

        let mut providers = Vec::new();

        // Scan localhost
        let localhost_providers = self.scan_localhost().await?;
        providers.extend(localhost_providers);

        // Automatically discover and scan all accessible subnets
        info!("Auto-discovering accessible subnets...");
        let subnets = self.discover_all_subnets();
        
        for subnet in subnets {
            info!("Discovered subnet: {}", subnet);
            if let Ok(subnet_providers) = self.scan_subnet(&subnet).await {
                providers.extend(subnet_providers);
            }
        }

        // Also support manual override via environment variable
        // Format: HAINET_EXTRA_SUBNETS="192.168.2.0/24,10.0.0.0/24"
        if let Ok(extra_subnets) = std::env::var("HAINET_EXTRA_SUBNETS") {
            for subnet in extra_subnets.split(',') {
                let subnet = subnet.trim();
                if !subnet.is_empty() {
                    info!("Scanning extra subnet (manual override): {}", subnet);
                    if let Ok(subnet_providers) = self.scan_subnet(subnet).await {
                        providers.extend(subnet_providers);
                    }
                }
            }
        }

        info!("Discovery complete: {} providers found", providers.len());

        Ok(providers)
    }

    /// Discover all accessible subnets on all network interfaces
    fn discover_all_subnets(&self) -> Vec<String> {
        let mut subnets = Vec::new();
        
        // Use the `if-addrs` crate functionality or manual approach
        // For simplicity, we'll use a manual approach with system commands
        
        #[cfg(target_os = "linux")]
        {
            // On Linux, parse `ip addr` output
            if let Ok(output) = std::process::Command::new("ip")
                .args(&["addr", "show"])
                .output()
            {
                if let Ok(stdout) = String::from_utf8(output.stdout) {
                    subnets.extend(self.parse_ip_addr_output(&stdout));
                }
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            // Fallback: try to get network interfaces manually
            // Use local IP derivation as fallback
            if let Ok(local_ip) = self.get_local_ip() {
                let subnet = self.derive_subnet(&local_ip);
                subnets.push(subnet);
            }
        }
        
        // Deduplicate subnets
        subnets.sort();
        subnets.dedup();
        
        // Filter out loopback
        subnets.retain(|s| !s.starts_with("127."));
        
        subnets
    }
    
    /// Parse `ip addr` output to extract subnets (Linux)
    #[cfg(target_os = "linux")]
    fn parse_ip_addr_output(&self, output: &str) -> Vec<String> {
        let mut subnets = Vec::new();
        
        for line in output.lines() {
            let line = line.trim();
            
            // Look for lines like: "inet 192.168.1.100/24 brd ..."
            if line.starts_with("inet ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let addr_with_prefix = parts[1];
                    
                    // Parse "192.168.1.100/24"
                    if let Some((ip_str, prefix_str)) = addr_with_prefix.split_once('/') {
                        // Parse IP octets
                        let octets: Vec<&str> = ip_str.split('.').collect();
                        if octets.len() == 4 {
                            // Only handle /24 subnets for now
                            if prefix_str == "24" {
                                let subnet = format!("{}.{}.{}.0/24", octets[0], octets[1], octets[2]);
                                subnets.push(subnet);
                            }
                            // Handle /16 subnets
                            else if prefix_str == "16" {
                                let subnet = format!("{}.{}.0.0/16", octets[0], octets[1]);
                                subnets.push(subnet);
                            }
                        }
                    }
                }
            }
        }
        
        subnets
    }

    /// Scan a specific subnet for AI providers
    async fn scan_subnet(&self, subnet: &str) -> Result<Vec<DiscoveredProvider>> {
        info!("Scanning subnet {} for AI providers (common ports)", subnet);

        // Parse subnet (e.g., "192.168.1.0/24")
        let parts: Vec<&str> = subnet.split('/').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid subnet format: {}", subnet));
        }

        let base_ip_parts: Vec<&str> = parts[0].split('.').collect();
        if base_ip_parts.len() != 4 {
            return Err(anyhow::anyhow!("Invalid IP format: {}", parts[0]));
        }

        let prefix = format!("{}.{}.{}", base_ip_parts[0], base_ip_parts[1], base_ip_parts[2]);

        // Scan common provider ports on each IP in subnet
        let scan_ips: Vec<u8> = (1..=254).collect();

        // Build all probe tasks and execute them concurrently
        let mut handles = Vec::new();
        for last_octet in scan_ips {
            let ip = format!("{}.{}", prefix, last_octet);
            for (provider_type, port) in &self.localhost_ports {
                let endpoint = format!("http://{}:{}", ip, port);
                let client = self.client.clone();
                let pt = *provider_type;
                
                handles.push(tokio::spawn(async move {
                    match tokio::time::timeout(
                        Duration::from_millis(500),
                        probe_provider_static(&client, pt, &endpoint)
                    ).await {
                        Ok(Ok(provider)) if provider.available => Some(provider),
                        _ => None,
                    }
                }));
            }
        }

        // Await all probes concurrently
        let results = futures::future::join_all(handles).await;
        let mut providers = Vec::new();
        for result in results {
            if let Ok(Some(provider)) = result {
                info!("✓ Found {} at {} ({} models)", 
                    provider.provider_type, provider.endpoint, provider.models.len());
                providers.push(provider);
            }
        }

        Ok(providers)
    }

    /// Get local IP address (excluding loopback)
    fn get_local_ip(&self) -> Result<String> {
        use std::net::UdpSocket;
        
        // Connect to a public DNS server to determine local IP
        // This doesn't actually send data, just binds to local interface
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.connect("8.8.8.8:80")?;
        let local_addr = socket.local_addr()?;
        
        Ok(local_addr.ip().to_string())
    }

    /// Derive /24 subnet from IP address
    fn derive_subnet(&self, ip: &str) -> String {
        let parts: Vec<&str> = ip.split('.').collect();
        if parts.len() == 4 {
            format!("{}.{}.{}.0/24", parts[0], parts[1], parts[2])
        } else {
            "192.168.1.0/24".to_string() // Fallback
        }
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
            .filter(|m| {
                // Filter out embedding models - they don't support generate API
                let name_lower = m.name.to_lowercase();
                !name_lower.contains("embed") && 
                !name_lower.contains("bge-") &&
                !name_lower.contains("nomic-embed")
            })
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
/// Free-standing probe function for use in spawned tasks (cannot capture &self)
async fn probe_provider_static(
    client: &Client,
    provider_type: ProviderType,
    endpoint: &str,
) -> Result<DiscoveredProvider> {
    let start = std::time::Instant::now();

    let probe_url = match provider_type {
        ProviderType::Ollama => format!("{}/api/tags", endpoint),
        _ => format!("{}/v1/models", endpoint),
    };

    let available = match client.get(&probe_url).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                false
            } else {
                // Validate content-type is JSON to avoid false positives from SPA servers
                // (e.g., other hainet-core nodes that serve HTML for unknown paths)
                let content_type = response.headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_lowercase();
                content_type.contains("application/json")
            }
        },
        Err(_) => false,
    };

    let latency_ms = start.elapsed().as_millis() as u64;

    let models = if available {
        match provider_type {
            ProviderType::Ollama => {
                #[derive(Deserialize)]
                struct OllamaResp { models: Vec<OllamaM> }
                #[derive(Deserialize)]
                struct OllamaM { name: String }
                
                let url = format!("{}/api/tags", endpoint);
                match client.get(&url).send().await {
                    Ok(resp) => match resp.json::<OllamaResp>().await {
                        Ok(data) => data.models.into_iter()
                            .filter(|m| {
                                let n = m.name.to_lowercase();
                                !n.contains("embed") && !n.contains("bge-") && !n.contains("nomic-embed")
                            })
                            .map(|m| m.name)
                            .collect(),
                        Err(_) => vec![],
                    },
                    Err(_) => vec![],
                }
            },
            _ => vec![],
        }
    } else {
        vec![]
    };

    Ok(DiscoveredProvider {
        provider_type,
        endpoint: endpoint.to_string(),
        available,
        latency_ms,
        models,
    })
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
