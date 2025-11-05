//! # START OF FILE hainet-core/src/networking/mesh_message.rs
//! Mesh Communication Protocol - Message types and serialization

use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Message type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    /// Request message from client
    Request,
    /// Response message from server
    Response,
    /// Error response
    Error,
    /// Heartbeat/keepalive message
    Heartbeat,
}

/// Storage operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageOp {
    Read,
    Write,
    Delete,
    List,
}

/// Service-specific payload types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServicePayload {
    /// Large Language Model request
    LLM {
        prompt: String,
        model: String,
        #[serde(default)]
        options: HashMap<String, String>,
    },
    /// Speech-to-Text request
    STT {
        #[serde(with = "serde_bytes")]
        audio_data: Vec<u8>,
        language: Option<String>,
    },
    /// Text-to-Speech request
    TTS {
        text: String,
        voice: String,
    },
    /// Storage operation request
    Storage {
        operation: StorageOp,
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(with = "serde_bytes")]
        data: Option<Vec<u8>>,
    },
    /// MCP tool call request
    MCP {
        server: String,
        tool: String,
        arguments: serde_json::Value,
    },
}

/// Response payload wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum ResponsePayload {
    /// Successful response with data
    Success { data: serde_json::Value },
    /// Error response with code and message
    Error { code: u16, message: String },
}

/// Core mesh message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshMessage {
    /// Unique message identifier
    pub id: Uuid,
    /// Message type (Request/Response/Error/Heartbeat)
    pub message_type: MessageType,
    /// Service-specific payload
    pub payload: ServicePayload,
    /// Sender peer ID (serialized as string)
    #[serde(with = "peer_id_serde")]
    pub sender: PeerId,
    /// Message creation timestamp (Unix milliseconds)
    pub timestamp: u64,
    /// Time-to-live in milliseconds
    pub ttl_ms: u64,
}

/// Response wrapper with request correlation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshResponse {
    /// Original request ID
    pub request_id: Uuid,
    /// Response payload
    pub payload: ResponsePayload,
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
}

impl MeshMessage {
    /// Create a new request message
    pub fn new_request(payload: ServicePayload, sender: PeerId) -> Self {
        Self {
            id: Uuid::new_v4(),
            message_type: MessageType::Request,
            payload,
            sender,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            ttl_ms: 30_000, // Default: 30 seconds
        }
    }

    /// Create a new request with custom TTL
    pub fn new_request_with_ttl(
        payload: ServicePayload,
        sender: PeerId,
        ttl: Duration,
    ) -> Self {
        let mut msg = Self::new_request(payload, sender);
        msg.ttl_ms = ttl.as_millis() as u64;
        msg
    }

    /// Create a heartbeat message
    pub fn new_heartbeat(sender: PeerId) -> Self {
        Self {
            id: Uuid::new_v4(),
            message_type: MessageType::Heartbeat,
            payload: ServicePayload::MCP {
                server: "heartbeat".to_string(),
                tool: "ping".to_string(),
                arguments: serde_json::json!({}),
            },
            sender,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            ttl_ms: 5_000, // 5 seconds for heartbeat
        }
    }

    /// Serialize message to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize message from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Check if message has expired based on TTL
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        now > self.timestamp + self.ttl_ms
    }

    /// Get time remaining until expiration
    pub fn time_remaining(&self) -> Option<Duration> {
        if self.is_expired() {
            return None;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        let remaining_ms = (self.timestamp + self.ttl_ms).saturating_sub(now);
        Some(Duration::from_millis(remaining_ms))
    }
}

impl MeshResponse {
    /// Create a successful response
    pub fn success(request_id: Uuid, data: serde_json::Value, processing_time_ms: u64) -> Self {
        Self {
            request_id,
            payload: ResponsePayload::Success { data },
            processing_time_ms,
        }
    }

    /// Create an error response
    pub fn error(request_id: Uuid, code: u16, message: String) -> Self {
        Self {
            request_id,
            payload: ResponsePayload::Error { code, message },
            processing_time_ms: 0,
        }
    }

    /// Serialize response to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize response from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Check if response is successful
    pub fn is_success(&self) -> bool {
        matches!(self.payload, ResponsePayload::Success { .. })
    }

    /// Check if response is an error
    pub fn is_error(&self) -> bool {
        matches!(self.payload, ResponsePayload::Error { .. })
    }
}

/// Custom serialization for PeerId (libp2p doesn't implement Serialize)
mod peer_id_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(peer_id: &PeerId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&peer_id.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PeerId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_peer() -> PeerId {
        PeerId::random()
    }

    #[test]
    fn test_message_creation() {
        let peer = create_test_peer();
        let payload = ServicePayload::LLM {
            prompt: "Hello".to_string(),
            model: "gemma3:7b".to_string(),
            options: HashMap::new(),
        };

        let message = MeshMessage::new_request(payload, peer);

        assert_eq!(message.message_type, MessageType::Request);
        assert_eq!(message.sender, peer);
        assert_eq!(message.ttl_ms, 30_000);
        assert!(!message.is_expired());
    }

    #[test]
    fn test_message_serialization() {
        let peer = create_test_peer();
        let payload = ServicePayload::MCP {
            server: "hainet-files".to_string(),
            tool: "read_file".to_string(),
            arguments: serde_json::json!({"path": "/test.txt"}),
        };

        let message = MeshMessage::new_request(payload, peer);
        let json = message.to_json().unwrap();
        let deserialized = MeshMessage::from_json(&json).unwrap();

        assert_eq!(message.id, deserialized.id);
        assert_eq!(message.message_type, deserialized.message_type);
    }

    #[test]
    fn test_ttl_expiration() {
        let peer = create_test_peer();
        let payload = ServicePayload::MCP {
            server: "test".to_string(),
            tool: "test".to_string(),
            arguments: serde_json::json!({}),
        };

        let message = MeshMessage::new_request_with_ttl(
            payload,
            peer,
            Duration::from_millis(1),
        );

        std::thread::sleep(Duration::from_millis(10));
        assert!(message.is_expired());
        assert!(message.time_remaining().is_none());
    }

    #[test]
    fn test_payload_types() {
        let peer = create_test_peer();

        // LLM payload
        let llm = ServicePayload::LLM {
            prompt: "test".to_string(),
            model: "gemma3:7b".to_string(),
            options: HashMap::new(),
        };
        let msg1 = MeshMessage::new_request(llm, peer);
        assert!(matches!(msg1.payload, ServicePayload::LLM { .. }));

        // STT payload
        let stt = ServicePayload::STT {
            audio_data: vec![1, 2, 3],
            language: Some("en".to_string()),
        };
        let msg2 = MeshMessage::new_request(stt, peer);
        assert!(matches!(msg2.payload, ServicePayload::STT { .. }));

        // Storage payload
        let storage = ServicePayload::Storage {
            operation: StorageOp::Read,
            path: "/test".to_string(),
            data: None,
        };
        let msg3 = MeshMessage::new_request(storage, peer);
        assert!(matches!(msg3.payload, ServicePayload::Storage { .. }));
    }

    #[test]
    fn test_response_creation() {
        let request_id = Uuid::new_v4();

        // Success response
        let success = MeshResponse::success(
            request_id,
            serde_json::json!({"result": "ok"}),
            150,
        );
        assert!(success.is_success());
        assert!(!success.is_error());
        assert_eq!(success.processing_time_ms, 150);

        // Error response
        let error = MeshResponse::error(request_id, 500, "Internal error".to_string());
        assert!(!error.is_success());
        assert!(error.is_error());
    }

    #[test]
    fn test_heartbeat_message() {
        let peer = create_test_peer();
        let heartbeat = MeshMessage::new_heartbeat(peer);

        assert_eq!(heartbeat.message_type, MessageType::Heartbeat);
        assert_eq!(heartbeat.ttl_ms, 5_000);
        assert!(!heartbeat.is_expired());
    }
}
