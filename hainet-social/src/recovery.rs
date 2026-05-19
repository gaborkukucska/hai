// START OF FILE hainet-social/src/recovery.rs
//! Mesh Recovery Broadcasts
//! 
//! Handles recovery broadcasts to find missing chunks or reconnect
//! disconnected peers in the social mesh.

use std::collections::HashSet;
use tracing::{debug, info};
use crate::SocialResult;
use crate::packets::{PacketPayload};
use crate::gossip::GossipEngine;
use std::sync::Arc;

/// Recovery broadcast request for missing chunks
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MissingChunkRequest {
    pub file_id: String,
    pub missing_chunks: Vec<u32>,
    pub original_source_id: Option<String>,
}

/// Mesh recovery manager
pub struct RecoveryManager {
    local_node_id: String,
    gossip_manager: Arc<GossipEngine>,
    recent_requests: std::sync::RwLock<HashSet<String>>,
}

impl RecoveryManager {
    pub fn new(local_node_id: String, gossip_manager: Arc<GossipEngine>) -> Self {
        Self {
            local_node_id,
            gossip_manager,
            recent_requests: std::sync::RwLock::new(HashSet::new()),
        }
    }

    /// Broadcast a request for missing chunks
    pub async fn broadcast_chunk_recovery(&self, request: MissingChunkRequest) -> SocialResult<()> {
        let request_id = format!("{}_{:?}", request.file_id, request.missing_chunks);
        
        {
            let mut recent = self.recent_requests.write().unwrap();
            if !recent.insert(request_id.clone()) {
                debug!("Recovery broadcast already sent recently for this missing set");
                return Ok(());
            }
        }
        
        info!("Broadcasting recovery request for file {}, missing {} chunks", 
            request.file_id, request.missing_chunks.len());
            
        for chunk_idx in &request.missing_chunks {
            let payload = PacketPayload::MediaRequest {
                payload: crate::packets::MediaRequestPayload {
                    media_id: request.file_id.clone(),
                    chunk_index: *chunk_idx,
                    chunk_size: 1024 * 1024, // Assumed 1MB for now
                    access_key: None,
                },
            };
            
            let packet = self.gossip_manager.create_packet(payload);
            
            // Use max TTL to reach the entire mesh
            let mut broadcast_packet = packet;
            broadcast_packet.header.hops = Some(15); // MAX_TTL
            
            // In a real implementation we would emit this packet to the network transport
            // For now, we rely on the caller to dispatch it
        }
        Ok(())
    }

    /// Process an incoming recovery request
    pub async fn handle_recovery_request(&self, request: MissingChunkRequest) -> SocialResult<()> {
        debug!("Received recovery request for file {}, missing {} chunks", 
            request.file_id, request.missing_chunks.len());
            
        // Implementation would:
        // 1. Check local media storage for these chunks
        // 2. If we have them, initiate a direct transfer to the request origin
        
        Ok(())
    }
}
