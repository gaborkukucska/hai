//! # START OF FILE hainet-core/src/networking/multiplexer.rs
//! Request Multiplexer - Concurrent request management

use super::mesh_message::{MeshMessage, MeshResponse};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{oneshot, RwLock};
use uuid::Uuid;

/// Pending request state
struct PendingRequest {
    message: MeshMessage,
    started_at: SystemTime,
    response_tx: oneshot::Sender<Result<MeshResponse, String>>,
}

/// Multiplexer statistics
#[derive(Debug, Clone, Default)]
pub struct MultiplexerStats {
    /// Currently active requests
    pub active_requests: usize,
    /// Total completed requests
    pub completed_requests: u64,
    /// Total timed-out requests
    pub timed_out_requests: u64,
    /// Times max concurrent limit was reached
    pub max_concurrent_reached: u64,
}

/// Request multiplexer for concurrent operations
pub struct RequestMultiplexer {
    pending_requests: Arc<RwLock<HashMap<Uuid, PendingRequest>>>,
    max_concurrent: usize,
    stats: Arc<RwLock<MultiplexerStats>>,
}

impl RequestMultiplexer {
    /// Create a new multiplexer with default settings (max 100 concurrent)
    pub fn new() -> Self {
        Self::with_max_concurrent(100)
    }

    /// Create multiplexer with custom max concurrent requests
    pub fn with_max_concurrent(max_concurrent: usize) -> Self {
        Self {
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            max_concurrent,
            stats: Arc::new(RwLock::new(MultiplexerStats::default())),
        }
    }

    /// Submit a request and get a future for the response
    pub async fn submit(
        &self,
        message: MeshMessage,
    ) -> Result<oneshot::Receiver<Result<MeshResponse, String>>, String> {
        // Check if we've reached max concurrent
        {
            let pending = self.pending_requests.read().await;
            if pending.len() >= self.max_concurrent {
                let mut stats = self.stats.write().await;
                stats.max_concurrent_reached += 1;
                return Err(format!(
                    "Max concurrent requests reached ({})",
                    self.max_concurrent
                ));
            }
        }

        // Create oneshot channel for response
        let (tx, rx) = oneshot::channel();

        // Store pending request
        let request_id = message.id;
        let pending_request = PendingRequest {
            message,
            started_at: SystemTime::now(),
            response_tx: tx,
        };

        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(request_id, pending_request);
        }

        tracing::debug!("Request submitted: {}", request_id);
        Ok(rx)
    }

    /// Complete a request with a response
    pub async fn complete(&self, request_id: Uuid, response: MeshResponse) -> Result<(), String> {
        let pending_request = {
            let mut pending = self.pending_requests.write().await;
            pending.remove(&request_id)
        };

        match pending_request {
            Some(req) => {
                // Update stats
                {
                    let mut stats = self.stats.write().await;
                    stats.completed_requests += 1;
                }

                // Send response (ignore error if receiver dropped)
                let _ = req.response_tx.send(Ok(response));
                tracing::debug!("Request completed: {}", request_id);
                Ok(())
            }
            None => Err(format!("Request not found: {}", request_id)),
        }
    }

    /// Fail a request with an error
    pub async fn fail(&self, request_id: Uuid, error: String) -> Result<(), String> {
        let pending_request = {
            let mut pending = self.pending_requests.write().await;
            pending.remove(&request_id)
        };

        match pending_request {
            Some(req) => {
                // Send error (ignore if receiver dropped)
                let _ = req.response_tx.send(Err(error.clone()));
                tracing::warn!("Request failed: {} - {}", request_id, error);
                Ok(())
            }
            None => Err(format!("Request not found: {}", request_id)),
        }
    }

    /// Timeout an expired request
    pub async fn timeout(&self, request_id: Uuid) -> Result<(), String> {
        let pending_request = {
            let mut pending = self.pending_requests.write().await;
            pending.remove(&request_id)
        };

        match pending_request {
            Some(req) => {
                // Update stats
                {
                    let mut stats = self.stats.write().await;
                    stats.timed_out_requests += 1;
                }

                // Send timeout error
                let _ = req.response_tx.send(Err("Request timed out".to_string()));
                tracing::warn!("Request timed out: {}", request_id);
                Ok(())
            }
            None => Err(format!("Request not found: {}", request_id)),
        }
    }

    /// Cleanup expired requests based on TTL
    pub async fn cleanup_expired(&self) -> usize {
        let mut expired_ids = Vec::new();

        // Find expired requests
        {
            let pending = self.pending_requests.read().await;
            for (id, req) in pending.iter() {
                if req.message.is_expired() {
                    expired_ids.push(*id);
                }
            }
        }

        // Timeout expired requests
        let count = expired_ids.len();
        for id in expired_ids {
            let _ = self.timeout(id).await;
        }

        if count > 0 {
            tracing::info!("Cleaned up {} expired requests", count);
        }

        count
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> MultiplexerStats {
        let pending = self.pending_requests.read().await;
        let mut stats = self.stats.read().await.clone();
        stats.active_requests = pending.len();
        stats
    }

    /// Reset statistics (keeps active requests)
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        stats.completed_requests = 0;
        stats.timed_out_requests = 0;
        stats.max_concurrent_reached = 0;
    }

    /// Get number of active requests
    pub async fn active_count(&self) -> usize {
        let pending = self.pending_requests.read().await;
        pending.len()
    }

    /// Cancel a specific request
    pub async fn cancel(&self, request_id: Uuid) -> Result<(), String> {
        self.fail(request_id, "Request cancelled".to_string()).await
    }

    /// Cancel all pending requests
    pub async fn cancel_all(&self) -> usize {
        let request_ids: Vec<Uuid> = {
            let pending = self.pending_requests.read().await;
            pending.keys().copied().collect()
        };

        let count = request_ids.len();
        for id in request_ids {
            let _ = self.cancel(id).await;
        }

        if count > 0 {
            tracing::warn!("Cancelled {} pending requests", count);
        }

        count
    }
}

impl Default for RequestMultiplexer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::PeerId;
    use super::super::mesh_message::ServicePayload;

    fn create_test_message() -> MeshMessage {
        let peer = PeerId::random();
        MeshMessage::new_request(
            ServicePayload::MCP {
                server: "test".to_string(),
                tool: "test".to_string(),
                arguments: serde_json::json!({}),
            },
            peer,
        )
    }

    #[tokio::test]
    async fn test_multiplexer_creation() {
        let mux = RequestMultiplexer::new();
        assert_eq!(mux.max_concurrent, 100);
        assert_eq!(mux.active_count().await, 0);
    }

    #[tokio::test]
    async fn test_custom_max_concurrent() {
        let mux = RequestMultiplexer::with_max_concurrent(50);
        assert_eq!(mux.max_concurrent, 50);
    }

    #[tokio::test]
    async fn test_submit_request() {
        let mux = RequestMultiplexer::new();
        let message = create_test_message();
        
        let rx = mux.submit(message.clone()).await;
        assert!(rx.is_ok());
        assert_eq!(mux.active_count().await, 1);
    }

    #[tokio::test]
    async fn test_complete_request() {
        let mux = RequestMultiplexer::new();
        let message = create_test_message();
        let request_id = message.id;
        
        let mut rx = mux.submit(message).await.unwrap();
        
        let response = MeshResponse::success(
            request_id,
            serde_json::json!({"result": "ok"}),
            100,
        );
        
        mux.complete(request_id, response.clone()).await.unwrap();
        
        let result = rx.try_recv();
        assert!(result.is_ok());
        assert_eq!(mux.active_count().await, 0);
        
        let stats = mux.get_stats().await;
        assert_eq!(stats.completed_requests, 1);
    }

    #[tokio::test]
    async fn test_fail_request() {
        let mux = RequestMultiplexer::new();
        let message = create_test_message();
        let request_id = message.id;
        
        let mut rx = mux.submit(message).await.unwrap();
        
        mux.fail(request_id, "Test error".to_string()).await.unwrap();
        
        let result = rx.try_recv();
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_err());
        assert_eq!(mux.active_count().await, 0);
    }

    #[tokio::test]
    async fn test_timeout_request() {
        let mux = RequestMultiplexer::new();
        let message = create_test_message();
        let request_id = message.id;
        
        let mut rx = mux.submit(message).await.unwrap();
        
        mux.timeout(request_id).await.unwrap();
        
        let result = rx.try_recv();
        assert!(result.is_ok());
        
        let stats = mux.get_stats().await;
        assert_eq!(stats.timed_out_requests, 1);
    }

    #[tokio::test]
    async fn test_max_concurrent_limit() {
        let mux = RequestMultiplexer::with_max_concurrent(2);
        
        let msg1 = create_test_message();
        let msg2 = create_test_message();
        let msg3 = create_test_message();
        
        assert!(mux.submit(msg1).await.is_ok());
        assert!(mux.submit(msg2).await.is_ok());
        assert!(mux.submit(msg3).await.is_err());
        
        let stats = mux.get_stats().await;
        assert_eq!(stats.max_concurrent_reached, 1);
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let mux = RequestMultiplexer::new();
        
        let peer = PeerId::random();
        let message = MeshMessage::new_request_with_ttl(
            ServicePayload::MCP {
                server: "test".to_string(),
                tool: "test".to_string(),
                arguments: serde_json::json!({}),
            },
            peer,
            Duration::from_millis(1),
        );
        
        mux.submit(message).await.unwrap();
        
        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        let cleaned = mux.cleanup_expired().await;
        assert_eq!(cleaned, 1);
        assert_eq!(mux.active_count().await, 0);
        
        let stats = mux.get_stats().await;
        assert_eq!(stats.timed_out_requests, 1);
    }

    #[tokio::test]
    async fn test_cancel_request() {
        let mux = RequestMultiplexer::new();
        let message = create_test_message();
        let request_id = message.id;
        
        mux.submit(message).await.unwrap();
        mux.cancel(request_id).await.unwrap();
        
        assert_eq!(mux.active_count().await, 0);
    }

    #[tokio::test]
    async fn test_cancel_all() {
        let mux = RequestMultiplexer::new();
        
        mux.submit(create_test_message()).await.unwrap();
        mux.submit(create_test_message()).await.unwrap();
        mux.submit(create_test_message()).await.unwrap();
        
        let cancelled = mux.cancel_all().await;
        assert_eq!(cancelled, 3);
        assert_eq!(mux.active_count().await, 0);
    }

    #[tokio::test]
    async fn test_stats_reset() {
        let mux = RequestMultiplexer::new();
        let message = create_test_message();
        let request_id = message.id;
        
        let rx = mux.submit(message).await.unwrap();
        let response = MeshResponse::success(request_id, serde_json::json!({}), 100);
        mux.complete(request_id, response).await.unwrap();
        
        mux.reset_stats().await;
        
        let stats = mux.get_stats().await;
        assert_eq!(stats.completed_requests, 0);
        assert_eq!(stats.timed_out_requests, 0);
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let mux = Arc::new(RequestMultiplexer::new());
        let mut handles = vec![];
        
        for _ in 0..10 {
            let mux_clone = Arc::clone(&mux);
            let handle = tokio::spawn(async move {
                let message = create_test_message();
                let request_id = message.id;
                
                let rx = mux_clone.submit(message).await.unwrap();
                
                let response = MeshResponse::success(
                    request_id,
                    serde_json::json!({}),
                    50,
                );
                
                mux_clone.complete(request_id, response).await.unwrap();
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        let stats = mux.get_stats().await;
        assert_eq!(stats.completed_requests, 10);
        assert_eq!(mux.active_count().await, 0);
    }
}
