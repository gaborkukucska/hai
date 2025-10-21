//!<!-- # START OF FILE hainet-core/src/storage/sync.rs -->
//! P2P File Synchronization
//!
//! Enables file sync across devices in the local hub using content-addressed storage.
//! Implements request/response protocol for fetching content by hash.

use super::{ContentAddressedStore, ContentHash};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Request to sync file from peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequest {
    pub hash: ContentHash,
    pub requester_id: String,
}

/// Response to sync request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncResponse {
    /// Content found and returned
    Success {
        hash: ContentHash,
        content: Vec<u8>,
    },
    /// Content not found on this peer
    NotFound {
        hash: ContentHash,
    },
    /// Error occurred during sync
    Error {
        hash: ContentHash,
        message: String,
    },
}

/// Peer information for sync
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: String,
    pub available_hashes: Vec<ContentHash>,
    pub last_seen: std::time::SystemTime,
}

/// P2P file synchronization manager
pub struct P2PFileSync {
    store: ContentAddressedStore,
    peers: Arc<RwLock<HashMap<String, PeerInfo>>>,
    pending_requests: Arc<RwLock<HashMap<ContentHash, Vec<String>>>>,
}

impl P2PFileSync {
    /// Create new P2P sync manager
    pub fn new(store: ContentAddressedStore) -> Self {
        Self {
            store,
            peers: Arc::new(RwLock::new(HashMap::new())),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a peer with their available content
    pub async fn register_peer(&self, peer_id: String, available_hashes: Vec<ContentHash>) {
        let hash_count = available_hashes.len();
        let peer_info = PeerInfo {
            peer_id: peer_id.clone(),
            available_hashes,
            last_seen: std::time::SystemTime::now(),
        };
        
        self.peers.write().await.insert(peer_id.clone(), peer_info);
        info!("Registered peer {} with {} hashes", peer_id, hash_count);
    }

    /// Unregister a peer
    pub async fn unregister_peer(&self, peer_id: &str) {
        self.peers.write().await.remove(peer_id);
        info!("Unregistered peer {}", peer_id);
    }

    /// Request file from peer by hash
    pub async fn request_file(&self, hash: ContentHash, peer_id: String) -> Result<SyncRequest> {
        // Track pending request
        self.pending_requests.write().await
            .entry(hash)
            .or_insert_with(Vec::new)
            .push(peer_id.clone());

        debug!("Requesting hash {} from peer {}", hash, peer_id);

        Ok(SyncRequest {
            hash,
            requester_id: "local".to_string(), // TODO: Use actual device ID
        })
    }

    /// Handle incoming sync request
    pub async fn handle_request(&self, request: SyncRequest) -> SyncResponse {
        debug!("Handling sync request for hash {} from {}", 
               request.hash, request.requester_id);

        // Try to get content from local store
        match self.store.get(&request.hash).await {
            Ok(content) => {
                info!("Serving content {} ({} bytes) to {}", 
                      request.hash, content.len(), request.requester_id);
                SyncResponse::Success {
                    hash: request.hash,
                    content,
                }
            }
            Err(e) => {
                warn!("Content {} not found: {}", request.hash, e);
                SyncResponse::NotFound {
                    hash: request.hash,
                }
            }
        }
    }

    /// Handle incoming sync response
    pub async fn handle_response(&self, response: SyncResponse) -> Result<()> {
        match response {
            SyncResponse::Success { hash, content } => {
                // Store received content
                let stored_hash = self.store.put(&content, None).await
                    .context("Failed to store synced content")?;

                if stored_hash != hash {
                    anyhow::bail!("Hash mismatch: expected {}, got {}", hash, stored_hash);
                }

                // Remove from pending
                self.pending_requests.write().await.remove(&hash);

                info!("Successfully synced content {} ({} bytes)", hash, content.len());
                Ok(())
            }
            SyncResponse::NotFound { hash } => {
                warn!("Content {} not found on peer", hash);
                // Could try another peer here
                Ok(())
            }
            SyncResponse::Error { hash, message } => {
                warn!("Sync error for {}: {}", hash, message);
                anyhow::bail!("Sync failed: {}", message)
            }
        }
    }

    /// Find peers that have specific content
    pub async fn find_peers_with_content(&self, hash: &ContentHash) -> Vec<String> {
        self.peers.read().await
            .values()
            .filter(|p| p.available_hashes.contains(hash))
            .map(|p| p.peer_id.clone())
            .collect()
    }

    /// Get list of all known peers
    pub async fn list_peers(&self) -> Vec<String> {
        self.peers.read().await.keys().cloned().collect()
    }

    /// Get pending sync requests
    pub async fn pending_count(&self) -> usize {
        self.pending_requests.read().await.len()
    }

    /// Announce local content to peers (for discovery)
    pub async fn announce_content(&self) -> Vec<ContentHash> {
        self.store.list_all().await
    }

    /// Sync specific file from any available peer
    pub async fn sync_from_peers(&self, hash: ContentHash) -> Result<()> {
        // Check if already have it
        if self.store.has(&hash) {
            debug!("Already have content {}", hash);
            return Ok(());
        }

        // Find peers with this content
        let peers = self.find_peers_with_content(&hash).await;
        
        if peers.is_empty() {
            anyhow::bail!("No peers have content {}", hash);
        }

        // Request from first available peer
        // TODO: Implement retry logic and peer selection
        let peer_id = peers[0].clone();
        let _request = self.request_file(hash, peer_id).await?;

        info!("Initiated sync for {} from available peers", hash);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_sync_creation() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let _sync = P2PFileSync::new(store);
    }

    #[tokio::test]
    async fn test_peer_registration() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let sync = P2PFileSync::new(store);

        let hash = ContentHash::from_bytes(b"test");
        sync.register_peer("peer1".to_string(), vec![hash]).await;

        let peers = sync.list_peers().await;
        assert_eq!(peers.len(), 1);
        assert!(peers.contains(&"peer1".to_string()));
    }

    #[tokio::test]
    async fn test_peer_unregistration() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let sync = P2PFileSync::new(store);

        sync.register_peer("peer1".to_string(), vec![]).await;
        assert_eq!(sync.list_peers().await.len(), 1);

        sync.unregister_peer("peer1").await;
        assert_eq!(sync.list_peers().await.len(), 0);
    }

    #[tokio::test]
    async fn test_find_peers_with_content() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let sync = P2PFileSync::new(store);

        let hash = ContentHash::from_bytes(b"shared content");
        
        sync.register_peer("peer1".to_string(), vec![hash]).await;
        sync.register_peer("peer2".to_string(), vec![hash]).await;
        sync.register_peer("peer3".to_string(), vec![]).await;

        let peers = sync.find_peers_with_content(&hash).await;
        assert_eq!(peers.len(), 2);
        assert!(peers.contains(&"peer1".to_string()));
        assert!(peers.contains(&"peer2".to_string()));
    }

    #[tokio::test]
    async fn test_handle_request_not_found() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let sync = P2PFileSync::new(store);

        let hash = ContentHash::from_bytes(b"nonexistent");
        let request = SyncRequest {
            hash,
            requester_id: "peer1".to_string(),
        };

        let response = sync.handle_request(request).await;
        match response {
            SyncResponse::NotFound { hash: h } => assert_eq!(h, hash),
            _ => panic!("Expected NotFound response"),
        }
    }

    #[tokio::test]
    async fn test_handle_request_success() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let sync = P2PFileSync::new(store.clone());

        let data = b"test content";
        let hash = store.put(data, None).await.unwrap();

        let request = SyncRequest {
            hash,
            requester_id: "peer1".to_string(),
        };

        let response = sync.handle_request(request).await;
        match response {
            SyncResponse::Success { hash: h, content } => {
                assert_eq!(h, hash);
                assert_eq!(content, data);
            }
            _ => panic!("Expected Success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_response_success() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let sync = P2PFileSync::new(store.clone());

        let data = b"synced content";
        let hash = ContentHash::from_bytes(data);

        let response = SyncResponse::Success {
            hash,
            content: data.to_vec(),
        };

        sync.handle_response(response).await.unwrap();

        // Verify content was stored
        let retrieved = store.get(&hash).await.unwrap();
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_announce_content() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let sync = P2PFileSync::new(store.clone());

        let data1 = b"file1";
        let data2 = b"file2";
        
        let hash1 = store.put(data1, None).await.unwrap();
        let hash2 = store.put(data2, None).await.unwrap();

        let announced = sync.announce_content().await;
        assert_eq!(announced.len(), 2);
        assert!(announced.contains(&hash1));
        assert!(announced.contains(&hash2));
    }

    #[tokio::test]
    async fn test_pending_requests() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let sync = P2PFileSync::new(store);

        let hash = ContentHash::from_bytes(b"pending");
        
        assert_eq!(sync.pending_count().await, 0);
        
        sync.request_file(hash, "peer1".to_string()).await.unwrap();
        assert_eq!(sync.pending_count().await, 1);
    }
}
