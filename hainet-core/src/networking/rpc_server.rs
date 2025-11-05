//! # START OF FILE hainet-core/src/networking/rpc_server.rs
//! RPC Server - Handle incoming mesh requests

use super::mesh_message::{MeshMessage, MeshResponse, ResponsePayload, ServicePayload};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Service handler function type
pub type ServiceHandler = Arc<
    dyn Fn(ServicePayload) -> Result<ResponsePayload, String> + Send + Sync
>;

/// Server-side statistics
#[derive(Debug, Clone, Default)]
pub struct ServerStats {
    /// Total requests received
    pub total_requests: u64,
    /// Successfully processed requests
    pub successful_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Requests per service type
    pub requests_per_service: HashMap<String, u64>,
    /// Average processing time in milliseconds
    pub average_processing_time_ms: u64,
}

/// RPC server for handling incoming requests
pub struct RPCServer {
    handlers: Arc<RwLock<HashMap<String, ServiceHandler>>>,
    stats: Arc<RwLock<ServerStats>>,
    bind_address: String,
}

impl RPCServer {
    /// Create a new RPC server
    pub fn new(bind_address: String) -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(ServerStats::default())),
            bind_address,
        }
    }

    /// Register a service handler
    pub async fn register_handler(&self, service_type: String, handler: ServiceHandler) {
        let mut handlers = self.handlers.write().await;
        handlers.insert(service_type.clone(), handler);
        tracing::info!("Registered handler for service type: {}", service_type);
    }

    /// Unregister a service handler
    pub async fn unregister_handler(&self, service_type: &str) {
        let mut handlers = self.handlers.write().await;
        handlers.remove(service_type);
        tracing::info!("Unregistered handler for service type: {}", service_type);
    }

    /// Handle incoming request
    pub async fn handle_request(&self, message: MeshMessage) -> MeshResponse {
        let start = Instant::now();
        
        // Update stats: total requests
        {
            let mut stats = self.stats.write().await;
            stats.total_requests += 1;
        }

        // Check if message has expired
        if message.is_expired() {
            return MeshResponse::error(
                message.id,
                408,
                "Request has expired".to_string(),
            );
        }

        // Get service type from payload
        let service_type = match &message.payload {
            ServicePayload::LLM { .. } => "llm",
            ServicePayload::STT { .. } => "stt",
            ServicePayload::TTS { .. } => "tts",
            ServicePayload::Storage { .. } => "storage",
            ServicePayload::MCP { .. } => "mcp",
        };

        // Update per-service stats
        {
            let mut stats = self.stats.write().await;
            *stats.requests_per_service.entry(service_type.to_string()).or_insert(0) += 1;
        }

        // Find handler
        let handlers = self.handlers.read().await;
        let handler = match handlers.get(service_type) {
            Some(h) => h.clone(),
            None => {
                return MeshResponse::error(
                    message.id,
                    404,
                    format!("No handler registered for service type: {}", service_type),
                );
            }
        };
        drop(handlers);

        // Execute handler
        match handler(message.payload) {
            Ok(payload) => {
                let processing_time = start.elapsed().as_millis() as u64;
                
                // Update success stats
                {
                    let mut stats = self.stats.write().await;
                    stats.successful_requests += 1;
                    
                    // Update average processing time
                    let total = stats.successful_requests;
                    stats.average_processing_time_ms = 
                        (stats.average_processing_time_ms * (total - 1) + processing_time) / total;
                }

                MeshResponse {
                    request_id: message.id,
                    payload,
                    processing_time_ms: processing_time,
                }
            }
            Err(e) => {
                // Update failure stats
                {
                    let mut stats = self.stats.write().await;
                    stats.failed_requests += 1;
                }

                MeshResponse::error(message.id, 500, e)
            }
        }
    }

    /// Get server bind address
    pub fn bind_address(&self) -> &str {
        &self.bind_address
    }

    /// Get server statistics
    pub async fn get_stats(&self) -> ServerStats {
        self.stats.read().await.clone()
    }

    /// Reset statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = ServerStats::default();
    }

    /// Get list of registered service types
    pub async fn registered_services(&self) -> Vec<String> {
        let handlers = self.handlers.read().await;
        handlers.keys().cloned().collect()
    }
}

/// Create a simple echo handler for testing
pub fn create_echo_handler() -> ServiceHandler {
    Arc::new(|payload| {
        Ok(ResponsePayload::Success {
            data: serde_json::json!({
                "echo": format!("{:?}", payload),
            }),
        })
    })
}

/// Create a mock LLM handler
pub fn create_mock_llm_handler() -> ServiceHandler {
    Arc::new(|payload| {
        match payload {
            ServicePayload::LLM { prompt, model, .. } => {
                Ok(ResponsePayload::Success {
                    data: serde_json::json!({
                        "model": model,
                        "response": format!("Mock response to: {}", prompt),
                    }),
                })
            }
            _ => Err("Invalid payload type for LLM handler".to_string()),
        }
    })
}

/// Create a mock storage handler
pub fn create_mock_storage_handler() -> ServiceHandler {
    Arc::new(|payload| {
        match payload {
            ServicePayload::Storage { operation, path, .. } => {
                Ok(ResponsePayload::Success {
                    data: serde_json::json!({
                        "operation": format!("{:?}", operation),
                        "path": path,
                        "status": "ok",
                    }),
                })
            }
            _ => Err("Invalid payload type for Storage handler".to_string()),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::PeerId;

    #[tokio::test]
    async fn test_server_creation() {
        let server = RPCServer::new("127.0.0.1:8080".to_string());
        assert_eq!(server.bind_address(), "127.0.0.1:8080");
        
        let services = server.registered_services().await;
        assert_eq!(services.len(), 0);
    }

    #[tokio::test]
    async fn test_handler_registration() {
        let server = RPCServer::new("127.0.0.1:8080".to_string());
        
        server.register_handler("test".to_string(), create_echo_handler()).await;
        
        let services = server.registered_services().await;
        assert_eq!(services.len(), 1);
        assert!(services.contains(&"test".to_string()));
    }

    #[tokio::test]
    async fn test_handler_unregistration() {
        let server = RPCServer::new("127.0.0.1:8080".to_string());
        
        server.register_handler("test".to_string(), create_echo_handler()).await;
        server.unregister_handler("test").await;
        
        let services = server.registered_services().await;
        assert_eq!(services.len(), 0);
    }

    #[tokio::test]
    async fn test_request_handling() {
        let server = RPCServer::new("127.0.0.1:8080".to_string());
        server.register_handler("mcp".to_string(), create_echo_handler()).await;

        let peer = PeerId::random();
        let message = MeshMessage::new_request(
            ServicePayload::MCP {
                server: "test".to_string(),
                tool: "test_tool".to_string(),
                arguments: serde_json::json!({}),
            },
            peer,
        );

        let response = server.handle_request(message.clone()).await;
        assert_eq!(response.request_id, message.id);
        assert!(response.is_success());
    }

    #[tokio::test]
    async fn test_missing_handler() {
        let server = RPCServer::new("127.0.0.1:8080".to_string());

        let peer = PeerId::random();
        let message = MeshMessage::new_request(
            ServicePayload::LLM {
                prompt: "test".to_string(),
                model: "gemma3:7b".to_string(),
                options: HashMap::new(),
            },
            peer,
        );

        let response = server.handle_request(message).await;
        assert!(response.is_error());
        
        if let ResponsePayload::Error { code, message } = response.payload {
            assert_eq!(code, 404);
            assert!(message.contains("No handler registered"));
        }
    }

    #[tokio::test]
    async fn test_expired_message() {
        let server = RPCServer::new("127.0.0.1:8080".to_string());
        server.register_handler("mcp".to_string(), create_echo_handler()).await;

        let peer = PeerId::random();
        let message = MeshMessage::new_request_with_ttl(
            ServicePayload::MCP {
                server: "test".to_string(),
                tool: "test".to_string(),
                arguments: serde_json::json!({}),
            },
            peer,
            std::time::Duration::from_millis(1),
        );

        // Wait for expiration
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let response = server.handle_request(message).await;
        assert!(response.is_error());
        
        if let ResponsePayload::Error { code, .. } = response.payload {
            assert_eq!(code, 408);
        }
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let server = RPCServer::new("127.0.0.1:8080".to_string());
        server.register_handler("llm".to_string(), create_mock_llm_handler()).await;

        let peer = PeerId::random();
        let message = MeshMessage::new_request(
            ServicePayload::LLM {
                prompt: "test".to_string(),
                model: "gemma3:7b".to_string(),
                options: HashMap::new(),
            },
            peer,
        );

        server.handle_request(message).await;

        let stats = server.get_stats().await;
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.failed_requests, 0);
        assert_eq!(stats.requests_per_service.get("llm"), Some(&1));
    }

    #[tokio::test]
    async fn test_stats_reset() {
        let server = RPCServer::new("127.0.0.1:8080".to_string());
        server.register_handler("mcp".to_string(), create_echo_handler()).await;

        let peer = PeerId::random();
        let message = MeshMessage::new_request(
            ServicePayload::MCP {
                server: "test".to_string(),
                tool: "test".to_string(),
                arguments: serde_json::json!({}),
            },
            peer,
        );

        server.handle_request(message).await;
        server.reset_stats().await;

        let stats = server.get_stats().await;
        assert_eq!(stats.total_requests, 0);
    }

    #[tokio::test]
    async fn test_mock_llm_handler() {
        let handler = create_mock_llm_handler();
        
        let payload = ServicePayload::LLM {
            prompt: "Hello".to_string(),
            model: "gemma3:7b".to_string(),
            options: HashMap::new(),
        };

        let result = handler(payload);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_storage_handler() {
        let handler = create_mock_storage_handler();
        
        let payload = ServicePayload::Storage {
            operation: super::super::mesh_message::StorageOp::Read,
            path: "/test".to_string(),
            data: None,
        };

        let result = handler(payload);
        assert!(result.is_ok());
    }
}
