//! Request Queue for Load-Balanced Ollama Requests
//! 
//! Implements intelligent request routing, load balancing, and automatic
//! failover across multiple Ollama API endpoints.

use anyhow::{Result, Context};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::{RwLock, SemaphorePermit};
use serde::{Deserialize, Serialize};

use super::api_registry::{ApiRegistry, OllamaEndpoint, HealthStatus};
use super::providers::ollama::{OllamaRequest, OllamaResponse};

/// Load balancing strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadBalancingStrategy {
    /// Simple round-robin rotation across endpoints
    RoundRobin,
    /// Route to endpoint with fewest active requests
    LeastLoaded,
    /// Prefer endpoint with model already loaded (keep_alive optimization)
    ModelAffinity,
}

impl Default for LoadBalancingStrategy {
    fn default() -> Self {
        LoadBalancingStrategy::LeastLoaded
    }
}

/// Request queue metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueueMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub failover_count: u64,
    pub average_latency_ms: f64,
}

/// Ollama request queue with load balancing
pub struct OllamaRequestQueue {
    /// API registry
    registry: Arc<ApiRegistry>,
    
    /// Load balancing strategy
    strategy: LoadBalancingStrategy,
    
    /// Request timeout
    request_timeout: Duration,
    
    /// Queue metrics
    metrics: Arc<RwLock<QueueMetrics>>,
    
    /// Round-robin counter
    round_robin_counter: AtomicUsize,
}

impl OllamaRequestQueue {
    /// Create new request queue
    pub fn new(
        registry: Arc<ApiRegistry>,
        strategy: LoadBalancingStrategy,
        request_timeout: Duration,
    ) -> Self {
        Self {
            registry,
            strategy,
            request_timeout,
            metrics: Arc::new(RwLock::new(QueueMetrics::default())),
            round_robin_counter: AtomicUsize::new(0),
        }
    }
    
    /// Route request to best available endpoint
    pub async fn route_request(
        &self,
        model_name: &str,
        request: OllamaRequest,
    ) -> Result<OllamaResponse> {
        // Step 1: Find endpoints that have this model
        let candidates = self.registry.endpoints_with_model(model_name).await;
        
        if candidates.is_empty() {
            return Err(anyhow::anyhow!(
                "No endpoints available with model: {}",
                model_name
            ));
        }
        
        // Step 2: Select endpoint using configured strategy
        let endpoint = self.select_endpoint(&candidates)?;
        
        tracing::debug!(
            "Routing request for model {} to endpoint {} (load: {}/{}, strategy: {:?})",
            model_name,
            endpoint.url,
            endpoint.current_load.load(Ordering::Relaxed),
            endpoint.max_concurrent,
            self.strategy
        );
        
        // Step 3: Acquire execution slot (blocks if at max capacity)
        let _slot = endpoint.acquire_slot().await?;
        
        // Step 4: Execute request with timeout
        let start = std::time::Instant::now();
        let result = tokio::time::timeout(
            self.request_timeout,
            endpoint.execute_request(request.clone())
        ).await;
        
        let latency_ms = start.elapsed().as_millis() as u64;
        
        // Step 5: Handle result (success or failover)
        match result {
            Ok(Ok(response)) => {
                self.record_success(latency_ms).await;
                endpoint.request_count.fetch_add(1, Ordering::Relaxed);
                Ok(response)
            }
            Ok(Err(e)) => {
                tracing::warn!(
                    "Request failed on {}: {:?}, attempting failover",
                    endpoint.url,
                    e
                );
                
                // Mark endpoint as degraded
                *endpoint.health_status.write().await = HealthStatus::Degraded;
                endpoint.failure_count.fetch_add(1, Ordering::Relaxed);
                
                // Try next candidate
                self.failover_to_next_endpoint(&candidates, &endpoint.url, request).await
            }
            Err(_) => {
                tracing::warn!(
                    "Request timed out on {} after {:?}",
                    endpoint.url,
                    self.request_timeout
                );
                
                // Mark endpoint as degraded
                *endpoint.health_status.write().await = HealthStatus::Degraded;
                endpoint.failure_count.fetch_add(1, Ordering::Relaxed);
                
                // Try next candidate
                self.failover_to_next_endpoint(&candidates, &endpoint.url, request).await
            }
        }
    }
    
    /// Select endpoint based on load balancing strategy
    fn select_endpoint<'a>(
        &self,
        candidates: &'a [Arc<OllamaEndpoint>],
    ) -> Result<&'a Arc<OllamaEndpoint>> {
        match self.strategy {
            LoadBalancingStrategy::LeastLoaded => {
                candidates
                    .iter()
                    .min_by_key(|e| e.current_load.load(Ordering::Relaxed))
                    .ok_or_else(|| anyhow::anyhow!("No candidates available"))
            }
            LoadBalancingStrategy::RoundRobin => {
                let idx = self.round_robin_counter
                    .fetch_add(1, Ordering::Relaxed) % candidates.len();
                Ok(&candidates[idx])
            }
            LoadBalancingStrategy::ModelAffinity => {
                // For now, use least loaded (model affinity would require tracking last request per model)
                // TODO: Implement true model affinity with per-model-per-endpoint tracking
                candidates
                    .iter()
                    .min_by_key(|e| e.current_load.load(Ordering::Relaxed))
                    .ok_or_else(|| anyhow::anyhow!("No candidates available"))
            }
        }
    }
    
    /// Automatic failover to next available endpoint
    async fn failover_to_next_endpoint(
        &self,
        candidates: &[Arc<OllamaEndpoint>],
        failed_url: &str,
        request: OllamaRequest,
    ) -> Result<OllamaResponse> {
        // Record failover attempt
        {
            let mut metrics = self.metrics.write().await;
            metrics.failover_count += 1;
        }
        
        // Try remaining candidates
        for candidate in candidates {
            if candidate.url == failed_url {
                continue; // Skip the one that just failed
            }
            
            tracing::info!("Failing over to endpoint: {}", candidate.url);
            
            // Acquire slot
            match candidate.acquire_slot().await {
                Ok(_slot) => {
                    let result = tokio::time::timeout(
                        self.request_timeout,
                        candidate.execute_request(request.clone())
                    ).await;
                    
                    match result {
                        Ok(Ok(response)) => {
                            candidate.request_count.fetch_add(1, Ordering::Relaxed);
                            return Ok(response);
                        }
                        Ok(Err(e)) => {
                            tracing::warn!("Failover to {} failed: {:?}", candidate.url, e);
                            candidate.failure_count.fetch_add(1, Ordering::Relaxed);
                            *candidate.health_status.write().await = HealthStatus::Degraded;
                            continue;
                        }
                        Err(_) => {
                            tracing::warn!("Failover to {} timed out", candidate.url);
                            candidate.failure_count.fetch_add(1, Ordering::Relaxed);
                            *candidate.health_status.write().await = HealthStatus::Degraded;
                            continue;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Could not acquire slot on {}: {:?}", candidate.url, e);
                    continue;
                }
            }
        }
        
        // Record failure
        {
            let mut metrics = self.metrics.write().await;
            metrics.failed_requests += 1;
        }
        
        Err(anyhow::anyhow!("All endpoints failed for this request"))
    }
    
    /// Record successful request
    async fn record_success(&self, latency_ms: u64) {
        let mut metrics = self.metrics.write().await;
        metrics.total_requests += 1;
        metrics.successful_requests += 1;
        
        // Update average latency (exponential moving average)
        let alpha = 0.3; // Weight for new sample
        metrics.average_latency_ms = alpha * latency_ms as f64 
            + (1.0 - alpha) * metrics.average_latency_ms;
    }
    
    /// Get queue metrics
    pub async fn get_metrics(&self) -> QueueMetrics {
        self.metrics.read().await.clone()
    }
}

/// RAII guard for slot acquisition
pub struct SlotGuard {
    #[allow(dead_code)]
    permit: tokio::sync::OwnedSemaphorePermit,
    load_counter: Arc<AtomicUsize>,
    endpoint_url: String,
}

impl Drop for SlotGuard {
    fn drop(&mut self) {
        // Decrement load counter when slot is released
        self.load_counter.fetch_sub(1, Ordering::Relaxed);
        
        tracing::trace!(
            "Released slot on {} (current load: {})",
            self.endpoint_url,
            self.load_counter.load(Ordering::Relaxed)
        );
    }
}

impl OllamaEndpoint {
    /// Acquire execution slot (async-safe, blocks if at max capacity)
    pub async fn acquire_slot(&self) -> Result<SlotGuard> {
        // Use tokio Semaphore for async-safe concurrency limiting
        let permit = self.semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| anyhow::anyhow!("Semaphore closed"))?;
        
        // Increment active request counter
        self.current_load.fetch_add(1, Ordering::Relaxed);
        
        tracing::trace!(
            "Acquired slot on {} (current load: {}/{})",
            self.url,
            self.current_load.load(Ordering::Relaxed),
            self.max_concurrent
        );
        
        Ok(SlotGuard {
            permit,
            load_counter: self.current_load.clone(),
            endpoint_url: self.url.clone(),
        })
    }
    
    /// Execute request on this endpoint
    pub async fn execute_request(&self, request: OllamaRequest) -> Result<OllamaResponse> {
        let response = self.client
            .post(format!("{}/api/generate", self.url))
            .json(&request)
            .send()
            .await
            .context(format!("Failed to send request to {}", self.url))?;
        
        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Ollama API returned error: {}",
                response.status()
            ));
        }
        
        let ollama_response: OllamaResponse = response.json().await
            .context("Failed to parse Ollama response")?;
        
        Ok(ollama_response)
    }
}
