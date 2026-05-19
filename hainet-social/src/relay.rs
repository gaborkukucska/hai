// START OF FILE hainet-social/src/relay.rs
//! Mesh Relay and Proxy
//! 
//! Implements pure streaming proxy logic (`_relayState`) to enable 
//! zero-copy forwarding of media chunks between peers across the mesh.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use crate::SocialResult;
use crate::packets::NetworkPacket;

/// Represents an active relay route
#[derive(Debug, Clone)]
pub struct RelayRoute {
    /// ID of the peer that requested the relay
    pub source_peer_id: String,
    /// ID of the ultimate destination peer
    pub target_peer_id: String,
    /// Timestamp when this route was established
    pub established_at: u64,
    /// Last activity timestamp to timeout stale routes
    pub last_activity: u64,
}

/// Manages media chunk relaying for the local node
#[derive(Clone)]
pub struct RelayManager {
    local_node_id: String,
    /// Active relay state: mapped by transfer session ID
    active_relays: Arc<RwLock<HashMap<String, RelayRoute>>>,
}

impl RelayManager {
    pub fn new(local_node_id: String) -> Self {
        Self {
            local_node_id,
            active_relays: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Establish a new relay route
    pub async fn establish_route(&self, session_id: String, source_peer_id: String, target_peer_id: String) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let route = RelayRoute {
            source_peer_id,
            target_peer_id,
            established_at: now,
            last_activity: now,
        };

        info!("Establishing relay route {} for {} -> {}", session_id, route.source_peer_id, route.target_peer_id);
        
        let mut relays = self.active_relays.write().await;
        relays.insert(session_id, route);
    }

    /// Process a packet that might need relaying
    pub async fn process_relay_packet(&self, session_id: &str, mut packet: NetworkPacket) -> SocialResult<Option<NetworkPacket>> {
        let mut relays = self.active_relays.write().await;
        
        if let Some(route) = relays.get_mut(session_id) {
            debug!("Relaying packet for session {}", session_id);
            
            // Update activity timestamp
            route.last_activity = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
                
            // Modify packet to ensure correct routing
            packet.header.sender_id = self.local_node_id.clone();
            packet.header.hops = Some(packet.header.hops.unwrap_or(6).saturating_sub(1));
            
            if packet.header.hops == Some(0) {
                debug!("Packet TTL expired during relay for session {}", session_id);
                return Ok(None);
            }
            
            return Ok(Some(packet));
        }
        
        // Not a relay packet we are managing
        Ok(None)
    }

    /// Cleanup stale relay routes
    pub async fn cleanup_stale_routes(&self, timeout_secs: u64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut relays = self.active_relays.write().await;
        
        relays.retain(|session_id, route| {
            let is_active = now.saturating_sub(route.last_activity) < timeout_secs;
            if !is_active {
                info!("Closing stale relay route for session {}", session_id);
            }
            is_active
        });
    }
}
