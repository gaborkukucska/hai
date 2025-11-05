//! # START OF FILE hainet-core/src/networking/rpc_client.rs
//! RPC Client - Remote procedure calls with timeout and retry logic

use super::mesh_message::{MeshMessage, MeshResponse, ServicePayload};
use libp2p::PeerId;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// RPC client configuration
#[derive(Debug, Clone)]
pub struct RPCConfig {
    /// Request timeout duration
    pub timeout: Duration,
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial delay between retries
    pub retry_delay: Duration,
    /// Enable exponential backoff
    pub enable_backoff: bool,
}

impl Default for RPCConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_delay: Duration::from_secs(1),
            enable_backoff: true,
        }
    }
}

/// Client-side statistics
#[derive(Debug, Clone, Default)]
pub struct ClientStats {
    /// Total requests attempted
    pub total_requests: u64,
    /// Successfully completed requests
    pub successful_requests: u64,
    /// Failed requests (after all retries)
    pub failed_requests: u64,
    /// Total retry attempts
    pub retries: u64,
    /// Average latency in milliseconds
    pub average_latency_ms: u64,
}

/// RPC client with retry logic
pub struct RPCClient {
    config: RPCConfig,
    http_client: reqwest::Client,
    stats: Arc<RwLock<ClientStats>>,
    local_peer_id: PeerId,
}

impl RPCClient {
    /// Create a new RPC client with default configuration
    pub fn new(local_peer_id: PeerId) -> Self {
        Self::with_config(local_peer_id, RPCConfig::default())
    }

    /// Create RPC client with custom configuration
    pub fn with_config(local_peer_id: PeerId, config: RPCConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config,
            http_client,
            stats: Arc::new(RwLock::new(ClientStats::default())),
            local_peer_id,
        }
    }

    /// Execute RPC call with retry logic
    pub async fn call(
        &self,
        endpoint: &str,
        payload: ServicePayload,
    ) -> Result<MeshResponse, String> {
        let message = MeshMessage::new_request_with_ttl(
            payload,
            self.local_peer_id,
            self.config.timeout,
        );

        self.call_with_message(endpoint, message).await
    }

    /// Execute RPC call with custom message (for advanced use)
    pub async fn call_with_message(
        &self,
        endpoint: &str,
        message: MeshMessage,
    ) -> Result<MeshResponse, String> {
        let start = Instant::now();
        let mut last_error = String::new();

        // Update stats: total requests
        {
            let mut stats = self.stats.write().await;
            stats.total_requests += 1;
        }

        for attempt in 0..=self.config.max_retries {
            match self.send_request(endpoint, &message).await {
                Ok(response) => {
                    // Success - update stats
                    let latency = start.elapsed().as_millis() as u64;
                    let mut stats = self.stats.write().await;
                    stats.successful_requests += 1;
                    stats.retries += attempt as u64;
                    
                    // Update average latency
                    let total = stats.successful_requests;
                    stats.average_latency_ms = 
                        (stats.average_latency_ms * (total - 1) + latency) / total;

                    return Ok(response);
                }
                Err(e) => {
                    last_error = e.clone();
                    
                    // Check if error is retryable
                    if !is_retryable_error(&e) {
                        break;
                    }

                    // Don't retry on last attempt
                    if attempt < self.config.max_retries {
                        let delay = self.calculate_backoff_delay(attempt);
                        tracing::debug!(
                            "RPC call failed (attempt {}/{}), retrying in {:?}: {}",
                            attempt + 1,
                            self.config.max_retries + 1,
                            delay,
                            e
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        // All retries exhausted - update failed stats
        {
            let mut stats = self.stats.write().await;
            stats.failed_requests += 1;
        }

        Err(format!("RPC call failed after {} attempts: {}", 
            self.config.max_retries + 1, last_error))
    }

    /// Call with backup endpoints (failover support)
    pub async fn call_with_backups(
        &self,
        primary: &str,
        backups: &[String],
        payload: ServicePayload,
    ) -> Result<MeshResponse, String> {
        // Try primary first
        match self.call(primary, payload.clone()).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                tracing::warn!("Primary endpoint failed: {}", e);
            }
        }

        // Try backups in order
        for (i, backup) in backups.iter().enumerate() {
            tracing::info!("Trying backup endpoint {}/{}: {}", i + 1, backups.len(), backup);
            match self.call(backup, payload.clone()).await {
                Ok(response) => {
                    tracing::info!("Backup endpoint succeeded");
                    return Ok(response);
                }
                Err(e) => {
                    tracing::warn!("Backup endpoint failed: {}", e);
                }
            }
        }

        Err(format!("All endpoints failed (1 primary + {} backups)", backups.len()))
    }

    /// Send single HTTP request (no retry)
    async fn send_request(
        &self,
        endpoint: &str,
        message: &MeshMessage,
    ) -> Result<MeshResponse, String> {
        // Serialize message
        let json = message.to_json().map_err(|e| format!("Serialization error: {}", e))?;

        // Send HTTP POST
        let response = self.http_client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .body(json)
            .send()
            .await
            .map_err(|e| format!("HTTP error: {}", e))?;

        // Check status code
        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()));
        }

        // Parse response
        let body = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        MeshResponse::from_json(&body)
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Calculate backoff delay based on attempt number
    fn calculate_backoff_delay(&self, attempt: u32) -> Duration {
        if self.config.enable_backoff {
            // Exponential backoff: delay * 2^attempt
            self.config.retry_delay * 2_u32.pow(attempt)
        } else {
            // Constant delay
            self.config.retry_delay
        }
    }

    /// Get current client statistics
    pub async fn get_stats(&self) -> ClientStats {
        self.stats.read().await.clone()
    }

    /// Reset statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = ClientStats::default();
    }
}

/// Check if error is retryable (network errors, timeouts)
fn is_retryable_error(error: &str) -> bool {
    error.contains("timeout") 
        || error.contains("connection") 
        || error.contains("network")
        || error.contains("HTTP error: 5") // 5xx errors are retryable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = RPCConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay, Duration::from_secs(1));
        assert!(config.enable_backoff);
    }

    #[test]
    fn test_client_creation() {
        let peer = PeerId::random();
        let client = RPCClient::new(peer);
        assert_eq!(client.local_peer_id, peer);
    }

    #[test]
    fn test_backoff_calculation() {
        let peer = PeerId::random();
        let config = RPCConfig {
            retry_delay: Duration::from_secs(1),
            enable_backoff: true,
            ..Default::default()
        };
        let client = RPCClient::with_config(peer, config);

        assert_eq!(client.calculate_backoff_delay(0), Duration::from_secs(1));
        assert_eq!(client.calculate_backoff_delay(1), Duration::from_secs(2));
        assert_eq!(client.calculate_backoff_delay(2), Duration::from_secs(4));
    }

    #[test]
    fn test_constant_delay() {
        let peer = PeerId::random();
        let config = RPCConfig {
            retry_delay: Duration::from_secs(2),
            enable_backoff: false,
            ..Default::default()
        };
        let client = RPCClient::with_config(peer, config);

        assert_eq!(client.calculate_backoff_delay(0), Duration::from_secs(2));
        assert_eq!(client.calculate_backoff_delay(1), Duration::from_secs(2));
        assert_eq!(client.calculate_backoff_delay(2), Duration::from_secs(2));
    }

    #[test]
    fn test_retryable_errors() {
        assert!(is_retryable_error("connection timeout"));
        assert!(is_retryable_error("network error"));
        assert!(is_retryable_error("HTTP error: 503"));
        assert!(!is_retryable_error("invalid request"));
        assert!(!is_retryable_error("HTTP error: 404"));
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let peer = PeerId::random();
        let client = RPCClient::new(peer);
        
        // Initial stats
        let stats = client.get_stats().await;
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.successful_requests, 0);
        assert_eq!(stats.failed_requests, 0);

        // Reset stats
        client.reset_stats().await;
        let stats = client.get_stats().await;
        assert_eq!(stats.total_requests, 0);
    }
}
