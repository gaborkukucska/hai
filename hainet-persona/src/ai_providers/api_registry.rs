//! API Registry for Multi-Ollama Load Balancing
//! 
//! Manages discovery, health monitoring, and load tracking for multiple
//! Ollama API endpoints to enable distributed request processing.

use anyhow::{Result, Context};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};
use tokio::sync::{RwLock, Semaphore};
use serde::{Deserialize, Serialize};

/// Health status of an Ollama endpoint
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Endpoint responding normally
    Healthy,
    /// Elevated latency or some errors
    Degraded,
    /// Not responding
    Unhealthy,
}

/// Individual Ollama API endpoint
pub struct OllamaEndpoint {
    /// Endpoint URL (e.g., "http://localhost:11434")
    pub url: String,
    
    /// Models available on this endpoint
    pub available_models: RwLock<Vec<String>>,
    
    /// Current health status
    pub health_status: RwLock<HealthStatus>,
    
    /// Number of active requests
    pub current_load: Arc<AtomicUsize>,
    
    /// Maximum concurrent requests allowed
    pub max_concurrent: usize,
    
    /// Semaphore for concurrency control
    pub semaphore: Arc<Semaphore>,
    
    /// Last health check timestamp
    pub last_health_check: RwLock<SystemTime>,
    
    /// Total requests processed
    pub request_count: AtomicU64,
    
    /// Failed requests
    pub failure_count: AtomicU64,
    
    /// HTTP client
    pub(crate) client: reqwest::Client,
}

impl OllamaEndpoint {
    /// Create new Ollama endpoint
    pub fn new(url: String, max_concurrent: usize) -> Self {
        Self {
            url,
            available_models: RwLock::new(Vec::new()),
            health_status: RwLock::new(HealthStatus::Healthy),
            current_load: Arc::new(AtomicUsize::new(0)),
            max_concurrent,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            last_health_check: RwLock::new(SystemTime::now()),
            request_count: AtomicU64::new(0),
            failure_count: AtomicU64::new(0),
            client: reqwest::Client::new(),
        }
    }
    
    /// Check endpoint health and update available models
    pub async fn check_health(&self) -> HealthStatus {
        let start = std::time::Instant::now();
        
        // Try to list models from endpoint
        match self.list_models().await {
            Ok(models) => {
                let latency_ms = start.elapsed().as_millis();
                
                // Update available models
                *self.available_models.write().await = models;
                *self.last_health_check.write().await = SystemTime::now();
                
                // Determine health status based on latency
                let status = if latency_ms < 1000 {
                    HealthStatus::Healthy
                } else if latency_ms < 3000 {
                    HealthStatus::Degraded
                } else {
                    HealthStatus::Degraded
                };
                
                *self.health_status.write().await = status;
                
                tracing::debug!(
                    "Health check for {} completed in {}ms: {:?}",
                    self.url,
                    latency_ms,
                    status
                );
                
                status
            }
            Err(e) => {
                tracing::warn!(
                    "Health check failed for {}: {:?}",
                    self.url,
                    e
                );
                
                *self.health_status.write().await = HealthStatus::Unhealthy;
                *self.last_health_check.write().await = SystemTime::now();
                
                HealthStatus::Unhealthy
            }
        }
    }
    
    /// List available models from this endpoint
    pub async fn list_models(&self) -> Result<Vec<String>> {
        let response = self.client
            .get(format!("{}/api/tags", self.url))
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .context(format!("Failed to connect to {}", self.url))?;
        
        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Ollama API returned error: {}",
                response.status()
            ));
        }
        
        let body: serde_json::Value = response.json().await?;
        
        let models = body["models"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?
            .iter()
            .filter_map(|m| m["name"].as_str().map(String::from))
            .collect();
        
        Ok(models)
    }
    
    /// Check if this endpoint has a specific model
    pub async fn has_model(&self, model_name: &str) -> bool {
        let models = self.available_models.read().await;
        
        // Check exact match or variant match (e.g., "gemma3:4b" matches "gemma3:4b-it-q4_K_M")
        models.iter().any(|m| {
            m == model_name || m.starts_with(&format!("{}:", model_name.split(':').next().unwrap_or(model_name)))
        })
    }
    
    /// Get current load (active requests)
    pub fn get_load(&self) -> usize {
        self.current_load.load(Ordering::Relaxed)
    }
    
    /// Get statistics
    pub fn get_stats(&self) -> EndpointStats {
        EndpointStats {
            url: self.url.clone(),
            current_load: self.current_load.load(Ordering::Relaxed),
            total_requests: self.request_count.load(Ordering::Relaxed),
            failed_requests: self.failure_count.load(Ordering::Relaxed),
        }
    }
}

/// Endpoint statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointStats {
    pub url: String,
    pub current_load: usize,
    pub total_requests: u64,
    pub failed_requests: u64,
}

/// Registry of all Ollama API endpoints
pub struct ApiRegistry {
    /// List of endpoints
    endpoints: Arc<RwLock<Vec<Arc<OllamaEndpoint>>>>,
    
    /// Health check interval
    health_check_interval: Duration,
}

impl ApiRegistry {
    /// Create new API registry
    pub async fn new(
        primary_endpoint: String,
        additional_endpoints: Vec<String>,
        endpoint_overrides: std::collections::HashMap<String, usize>,
        default_max_concurrent: usize,
    ) -> Result<Self> {
        let mut endpoints = Vec::new();
        
        // Add primary endpoint
        let max_concurrent = endpoint_overrides
            .get(&primary_endpoint)
            .copied()
            .unwrap_or(default_max_concurrent);
        
        let primary = Arc::new(OllamaEndpoint::new(
            primary_endpoint.clone(),
            max_concurrent,
        ));
        
        // Initial health check for primary
        primary.check_health().await;
        
        endpoints.push(primary);
        
        // Add additional endpoints
        for url in additional_endpoints {
            let max_concurrent = endpoint_overrides
                .get(&url)
                .copied()
                .unwrap_or(default_max_concurrent);
            
            let endpoint = Arc::new(OllamaEndpoint::new(url.clone(), max_concurrent));
            
            // Initial health check
            endpoint.check_health().await;
            
            endpoints.push(endpoint);
        }
        
        tracing::info!(
            "API Registry initialized with {} endpoints",
            endpoints.len()
        );
        
        Ok(Self {
            endpoints: Arc::new(RwLock::new(endpoints)),
            health_check_interval: Duration::from_secs(30),
        })
    }
    
    /// Get endpoints that have a specific model
    pub async fn endpoints_with_model(&self, model_name: &str) -> Vec<Arc<OllamaEndpoint>> {
        let endpoints = self.endpoints.read().await;
        
        let mut matching = Vec::new();
        
        for endpoint in endpoints.iter() {
            // Only include healthy or degraded endpoints
            let health = *endpoint.health_status.read().await;
            if health == HealthStatus::Unhealthy {
                continue;
            }
            
            if endpoint.has_model(model_name).await {
                matching.push(endpoint.clone());
            }
        }
        
        matching
    }
    
    /// Get healthiest endpoint
    pub async fn healthiest_endpoint(&self) -> Option<Arc<OllamaEndpoint>> {
        let endpoints = self.endpoints.read().await;
        
        endpoints
            .iter()
            .filter(|e| {
                // Only consider healthy endpoints
                matches!(
                    *e.health_status.blocking_read(),
                    HealthStatus::Healthy | HealthStatus::Degraded
                )
            })
            .min_by_key(|e| e.get_load())
            .cloned()
    }
    
    /// Start background health monitoring
    pub async fn start_health_monitoring(self: Arc<Self>) {
        let interval = self.health_check_interval;
        
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            
            loop {
                ticker.tick().await;
                
                let endpoints = self.endpoints.read().await;
                
                for endpoint in endpoints.iter() {
                    endpoint.check_health().await;
                }
                
                tracing::debug!("Completed health check for {} endpoints", endpoints.len());
            }
        });
    }
    
    /// Get registry statistics
    pub async fn get_stats(&self) -> RegistryStats {
        let endpoints = self.endpoints.read().await;
        
        let endpoint_stats: Vec<EndpointStats> = endpoints
            .iter()
            .map(|e| e.get_stats())
            .collect();
        
        let total_requests: u64 = endpoint_stats.iter().map(|s| s.total_requests).sum();
        let total_failures: u64 = endpoint_stats.iter().map(|s| s.failed_requests).sum();
        
        RegistryStats {
            total_endpoints: endpoints.len(),
            healthy_endpoints: endpoints
                .iter()
                .filter(|e| *e.health_status.blocking_read() == HealthStatus::Healthy)
                .count(),
            total_requests,
            total_failures,
            endpoint_stats,
        }
    }
}

/// Registry statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryStats {
    pub total_endpoints: usize,
    pub healthy_endpoints: usize,
    pub total_requests: u64,
    pub total_failures: u64,
    pub endpoint_stats: Vec<EndpointStats>,
}
