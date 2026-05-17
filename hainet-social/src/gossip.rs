//! # START OF FILE hainet-social/src/gossip.rs
//! Gossip Protocol Engine — Ported from gChat's networkService.ts
//!
//! Implements daisy-chain broadcast with TTL hop limiting.
//! Packets are forwarded to all connected peers with decremented hop count.
//! Privacy: sender_id is re-stamped at each hop (origin hidden from distant peers).

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::packets::{NetworkPacket, PacketPayload};
use crate::dedup::PacketDedup;
use crate::firewall::TrustFirewall;
use crate::SocialResult;

/// Default maximum hops for gossip propagation (from gChat: 6)
pub const DEFAULT_MAX_HOPS: u8 = 6;

/// Gossip engine managing packet propagation through the mesh
pub struct GossipEngine {
    /// Our node identifier
    local_node_id: String,
    /// Packet deduplication filter
    dedup: Arc<RwLock<PacketDedup>>,
    /// Trust-based firewall
    firewall: Arc<RwLock<TrustFirewall>>,
    /// Connected peer IDs
    connected_peers: Arc<RwLock<HashSet<String>>>,
    /// Maximum gossip hops
    max_hops: u8,
}

impl GossipEngine {
    /// Create a new gossip engine
    pub fn new(local_node_id: String) -> Self {
        Self {
            local_node_id,
            dedup: Arc::new(RwLock::new(PacketDedup::new())),
            firewall: Arc::new(RwLock::new(TrustFirewall::new())),
            connected_peers: Arc::new(RwLock::new(HashSet::new())),
            max_hops: DEFAULT_MAX_HOPS,
        }
    }

    /// Process an incoming packet: validate, dedup, firewall, then return forwarding targets
    ///
    /// Returns:
    /// - `Ok(Some(peers))` — packet is valid, should be forwarded to these peers
    /// - `Ok(None)` — packet was handled locally only (DM, duplicate, expired)
    /// - `Err(...)` — packet rejected (untrusted, invalid)
    pub async fn process_incoming(
        &self,
        packet: &NetworkPacket,
    ) -> SocialResult<Option<Vec<String>>> {
        let packet_id = packet.header.id.as_deref().unwrap_or("unknown");
        let sender = &packet.header.sender_id;

        // 1. Check TTL — drop if expired
        let hops = packet.header.hops.unwrap_or(DEFAULT_MAX_HOPS);
        if hops == 0 {
            debug!(packet_id, "Dropping packet — TTL expired");
            return Ok(None);
        }

        // 2. Dedup — drop if already processed
        {
            let mut dedup = self.dedup.write().await;
            if !dedup.check_and_mark(packet_id) {
                debug!(packet_id, "Dropping duplicate packet");
                return Ok(None);
            }
        }

        // 3. Firewall — check trust level
        // CONNECTION_REQUEST packets are always allowed through (gChat rule)
        let is_connection_request = matches!(
            &packet.payload,
            PacketPayload::ConnectionRequestPacket { .. }
        );

        if !is_connection_request {
            let firewall = self.firewall.read().await;
            if !firewall.is_trusted(sender) {
                warn!(sender, packet_id, "Dropping packet from untrusted peer");
                return Err(crate::SocialError::UntrustedPeer(sender.clone()));
            }
        }

        // 4. Check if this is a directed message (has target_user_id)
        if let Some(target) = &packet.header.target_user_id {
            if target == &self.local_node_id {
                // Message is for us — handle locally, don't forward
                debug!(packet_id, "Packet is directed to us — handling locally");
                return Ok(None);
            }
        }

        // 5. Determine forwarding targets (all connected peers except sender)
        let peers = self.connected_peers.read().await;
        let forward_to: Vec<String> = peers
            .iter()
            .filter(|p| *p != sender && *p != &self.local_node_id)
            .cloned()
            .collect();

        if forward_to.is_empty() {
            return Ok(None);
        }

        info!(
            packet_id,
            hops_remaining = hops - 1,
            forward_count = forward_to.len(),
            "Forwarding packet via daisy-chain"
        );

        Ok(Some(forward_to))
    }

    /// Prepare a packet for forwarding: decrement hops, re-stamp sender (privacy)
    ///
    /// From gChat: "strip originNode, re-stamp senderId" at each hop
    /// This ensures distant peers cannot trace the original sender.
    pub fn prepare_forward(&self, packet: &NetworkPacket) -> Option<NetworkPacket> {
        let hops = packet.header.hops.unwrap_or(DEFAULT_MAX_HOPS);
        if hops <= 1 {
            return None; // Would expire at next hop
        }

        let mut forwarded = packet.clone();
        forwarded.header.hops = Some(hops - 1);
        // Re-stamp sender to our node ID (privacy preservation)
        forwarded.header.sender_id = self.local_node_id.clone();

        Some(forwarded)
    }

    /// Create a new packet originating from this node
    pub fn create_packet(&self, payload: PacketPayload) -> NetworkPacket {
        NetworkPacket {
            header: crate::packets::PacketHeader {
                id: Some(uuid::Uuid::new_v4().to_string()),
                hops: Some(self.max_hops),
                sender_id: self.local_node_id.clone(),
                target_user_id: None,
                signature: None,
            },
            payload,
        }
    }

    /// Register a peer connection
    pub async fn add_peer(&self, peer_id: String) {
        self.connected_peers.write().await.insert(peer_id);
    }

    /// Remove a peer connection
    pub async fn remove_peer(&self, peer_id: &str) {
        self.connected_peers.write().await.remove(peer_id);
    }

    /// Add a peer to the trust list
    pub async fn trust_peer(&self, peer_id: String) {
        self.firewall.write().await.add_trusted(peer_id);
    }

    /// Remove a peer from the trust list
    pub async fn untrust_peer(&self, peer_id: &str) {
        self.firewall.write().await.remove_trusted(peer_id);
    }

    /// Get the number of connected peers
    pub async fn peer_count(&self) -> usize {
        self.connected_peers.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packets::*;

    fn make_post_packet(sender: &str, hops: u8, id: &str) -> NetworkPacket {
        NetworkPacket {
            header: PacketHeader {
                id: Some(id.to_string()),
                hops: Some(hops),
                sender_id: sender.to_string(),
                target_user_id: None,
                signature: None,
            },
            payload: PacketPayload::Post {
                payload: Post {
                    id: "p1".to_string(),
                    author_id: sender.to_string(),
                    author_name: "Test".to_string(),
                    author_avatar: None,
                    author_public_key: "pk".to_string(),
                    origin_node: None,
                    content: "test".to_string(),
                    content_hash: None,
                    image_url: None,
                    media: None,
                    timestamp: 0,
                    votes: None,
                    shares: None,
                    comments_count: None,
                    comments_list: None,
                    privacy: Privacy::Public,
                    is_edited: None,
                    hashtags: None,
                    reactions: None,
                },
            },
        }
    }

    #[tokio::test]
    async fn test_ttl_expiry() {
        let engine = GossipEngine::new("local".to_string());
        let packet = make_post_packet("peer1", 0, "pkt-1");

        let result = engine.process_incoming(&packet).await.unwrap();
        assert!(result.is_none()); // TTL=0 should be dropped
    }

    #[tokio::test]
    async fn test_dedup_drops_second_copy() {
        let engine = GossipEngine::new("local".to_string());
        engine.trust_peer("peer1".to_string()).await;
        engine.add_peer("peer2".to_string()).await;

        let packet = make_post_packet("peer1", 6, "pkt-dup");

        let first = engine.process_incoming(&packet).await.unwrap();
        assert!(first.is_some());

        let second = engine.process_incoming(&packet).await.unwrap();
        assert!(second.is_none()); // Duplicate should be dropped
    }

    #[tokio::test]
    async fn test_untrusted_peer_rejected() {
        let engine = GossipEngine::new("local".to_string());
        // Don't trust peer1

        let packet = make_post_packet("peer1", 6, "pkt-untrust");
        let result = engine.process_incoming(&packet).await;

        assert!(result.is_err()); // Should be rejected
    }

    #[tokio::test]
    async fn test_forward_decrements_hops() {
        let engine = GossipEngine::new("local".to_string());
        let packet = make_post_packet("peer1", 4, "pkt-fwd");

        let forwarded = engine.prepare_forward(&packet).unwrap();
        assert_eq!(forwarded.header.hops, Some(3));
        assert_eq!(forwarded.header.sender_id, "local");
    }

    #[tokio::test]
    async fn test_connection_request_bypasses_firewall() {
        let engine = GossipEngine::new("local".to_string());
        engine.add_peer("peer2".to_string()).await;

        let packet = NetworkPacket {
            header: PacketHeader {
                id: Some("conn-req-1".to_string()),
                hops: Some(6),
                sender_id: "stranger".to_string(),
                target_user_id: None,
                signature: None,
            },
            payload: PacketPayload::ConnectionRequestPacket {
                payload: ConnectionRequest {
                    id: "cr-1".to_string(),
                    from_user_id: "stranger".to_string(),
                    from_username: "new_user".to_string(),
                    from_display_name: "New User".to_string(),
                    from_home_node: "stranger.onion".to_string(),
                    from_encryption_public_key: None,
                    timestamp: 0,
                    signature: None,
                },
            },
        };

        // Stranger is NOT trusted, but CONNECTION_REQUEST should pass through
        let result = engine.process_incoming(&packet).await.unwrap();
        assert!(result.is_some());
    }
}
