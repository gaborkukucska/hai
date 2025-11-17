//!<!-- # START OF FILE hainet-core/src/storage/sync_protocol.rs -->
//! Advanced Synchronization Protocol with Merkle Trees
//!
//! Provides efficient diff detection and incremental sync using Merkle tree-based
//! content comparison. Enables bandwidth-optimized synchronization between nodes.
//!
//! ## Constitutional Compliance
//! - Article I (Privacy First): All sync happens within trusted local hub
//! - Article III (Decentralization): Peer-to-peer sync without central coordination

use super::cas::{ContentAddressedStore, ContentHash};
use super::crdt::{NodeId, VectorClock};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Merkle tree node for efficient diff detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleNode {
    /// Hash of this node's content
    pub hash: ContentHash,
    /// Children nodes (empty for leaf nodes)
    pub children: Vec<MerkleNode>,
    /// Content hashes at this level (for leaf nodes)
    pub content: Vec<ContentHash>,
}

impl MerkleNode {
    /// Create leaf node from content hashes
    pub fn leaf(content: Vec<ContentHash>) -> Self {
        // Hash all content together
        let combined: Vec<u8> = content
            .iter()
            .flat_map(|h| h.as_bytes().to_vec())
            .collect();
        
        Self {
            hash: ContentHash::from_bytes(&combined),
            children: Vec::new(),
            content,
        }
    }

    /// Create internal node from children
    pub fn internal(children: Vec<MerkleNode>) -> Self {
        // Hash all children hashes together
        let combined: Vec<u8> = children
            .iter()
            .flat_map(|c| c.hash.as_bytes().to_vec())
            .collect();
        
        Self {
            hash: ContentHash::from_bytes(&combined),
            children,
            content: Vec::new(),
        }
    }

    /// Check if this is a leaf node
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Get all content hashes in this subtree
    pub fn all_content(&self) -> Vec<ContentHash> {
        if self.is_leaf() {
            self.content.clone()
        } else {
            self.children
                .iter()
                .flat_map(|c| c.all_content())
                .collect()
        }
    }
}

/// Merkle tree for content set
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleTree {
    root: MerkleNode,
    /// Branching factor (number of children per internal node)
    branching_factor: usize,
}

impl MerkleTree {
    /// Build Merkle tree from content hashes
    pub fn build(content: Vec<ContentHash>, branching_factor: usize) -> Self {
        if content.is_empty() {
            return Self {
                root: MerkleNode::leaf(Vec::new()),
                branching_factor,
            };
        }

        let root = Self::build_recursive(content, branching_factor);
        Self {
            root,
            branching_factor,
        }
    }

    /// Recursive tree building
    fn build_recursive(content: Vec<ContentHash>, branching_factor: usize) -> MerkleNode {
        if content.len() <= branching_factor {
            // Create leaf node
            return MerkleNode::leaf(content);
        }

        // Split into chunks and create children
        let chunk_size = (content.len() + branching_factor - 1) / branching_factor;
        let children: Vec<MerkleNode> = content
            .chunks(chunk_size)
            .map(|chunk| Self::build_recursive(chunk.to_vec(), branching_factor))
            .collect();

        MerkleNode::internal(children)
    }

    /// Get root hash
    pub fn root_hash(&self) -> &ContentHash {
        &self.root.hash
    }

    /// Compare with another tree and find differences
    pub fn diff(&self, other: &MerkleTree) -> Vec<ContentHash> {
        Self::diff_nodes(&self.root, &other.root)
    }

    /// Recursive diff comparison
    fn diff_nodes(local: &MerkleNode, remote: &MerkleNode) -> Vec<ContentHash> {
        // If hashes match, no differences
        if local.hash == remote.hash {
            return Vec::new();
        }

        // Both are leaves - return all content from remote
        if local.is_leaf() && remote.is_leaf() {
            return remote.content.clone();
        }

        // One is leaf, other is internal - need full subtree
        if local.is_leaf() || remote.is_leaf() {
            return remote.all_content();
        }

        // Both internal - recursively compare children
        let mut differences = Vec::new();
        let max_children = local.children.len().max(remote.children.len());

        for i in 0..max_children {
            match (local.children.get(i), remote.children.get(i)) {
                (Some(l), Some(r)) => {
                    differences.extend(Self::diff_nodes(l, r));
                }
                (None, Some(r)) => {
                    differences.extend(r.all_content());
                }
                (Some(_), None) => {
                    // Local has content remote doesn't - not a difference for sync
                }
                (None, None) => {
                    // Both empty - shouldn't happen given loop condition but handle anyway
                }
            }
        }

        differences
    }

    /// Get tree statistics
    pub fn stats(&self) -> TreeStats {
        Self::collect_stats(&self.root)
    }

    /// Collect statistics recursively
    fn collect_stats(node: &MerkleNode) -> TreeStats {
        if node.is_leaf() {
            TreeStats {
                total_nodes: 1,
                leaf_nodes: 1,
                internal_nodes: 0,
                total_content: node.content.len(),
                max_depth: 0,
            }
        } else {
            let mut stats = TreeStats {
                total_nodes: 1,
                leaf_nodes: 0,
                internal_nodes: 1,
                total_content: 0,
                max_depth: 0,
            };

            for child in &node.children {
                let child_stats = Self::collect_stats(child);
                stats.total_nodes += child_stats.total_nodes;
                stats.leaf_nodes += child_stats.leaf_nodes;
                stats.internal_nodes += child_stats.internal_nodes;
                stats.total_content += child_stats.total_content;
                stats.max_depth = stats.max_depth.max(child_stats.max_depth + 1);
            }

            stats
        }
    }
}

/// Tree statistics
#[derive(Debug, Clone)]
pub struct TreeStats {
    pub total_nodes: usize,
    pub leaf_nodes: usize,
    pub internal_nodes: usize,
    pub total_content: usize,
    pub max_depth: usize,
}

/// Sync request with Merkle tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequest {
    pub from_node: NodeId,
    pub to_node: NodeId,
    pub merkle_tree: MerkleTree,
    pub vector_clock: VectorClock,
}

/// Sync response with differences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResponse {
    pub from_node: NodeId,
    pub missing_content: Vec<ContentHash>,
    pub vector_clock: VectorClock,
}

/// Sync session tracking
#[derive(Debug, Clone)]
pub struct SyncSession {
    pub session_id: String,
    pub local_node: NodeId,
    pub remote_node: NodeId,
    pub started_at: std::time::SystemTime,
    pub local_clock: VectorClock,
    pub remote_clock: VectorClock,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub content_sent: usize,
    pub content_received: usize,
}

impl SyncSession {
    /// Create new sync session
    pub fn new(local_node: NodeId, remote_node: NodeId) -> Self {
        let session_id = format!("{}_{}", local_node.0, remote_node.0);
        Self {
            session_id,
            local_node,
            remote_node,
            started_at: std::time::SystemTime::now(),
            local_clock: VectorClock::new(),
            remote_clock: VectorClock::new(),
            bytes_sent: 0,
            bytes_received: 0,
            content_sent: 0,
            content_received: 0,
        }
    }

    /// Calculate duration
    pub fn duration(&self) -> std::time::Duration {
        self.started_at
            .elapsed()
            .unwrap_or(std::time::Duration::from_secs(0))
    }

    /// Calculate throughput (bytes per second)
    pub fn throughput(&self) -> f64 {
        let duration_secs = self.duration().as_secs_f64();
        if duration_secs == 0.0 {
            return 0.0;
        }
        (self.bytes_sent + self.bytes_received) as f64 / duration_secs
    }
}

/// Advanced sync protocol manager
pub struct SyncProtocol {
    local_node: NodeId,
    store: ContentAddressedStore,
    active_sessions: Arc<RwLock<HashMap<String, SyncSession>>>,
    vector_clock: Arc<RwLock<VectorClock>>,
    merkle_cache: Arc<RwLock<Option<MerkleTree>>>,
}

impl SyncProtocol {
    /// Create new sync protocol manager
    pub fn new(local_node: NodeId, store: ContentAddressedStore) -> Self {
        let mut clock = VectorClock::new();
        clock.increment(&local_node);

        Self {
            local_node,
            store,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            vector_clock: Arc::new(RwLock::new(clock)),
            merkle_cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Build Merkle tree from local content
    pub async fn build_merkle_tree(&self, branching_factor: usize) -> Result<MerkleTree> {
        let content = self.store.list_all().await;
        
        debug!(
            "Building Merkle tree for {} content items with branching factor {}",
            content.len(),
            branching_factor
        );

        let tree = MerkleTree::build(content, branching_factor);
        
        // Cache the tree
        *self.merkle_cache.write().await = Some(tree.clone());
        
        info!(
            "Built Merkle tree with root hash {}",
            tree.root_hash()
        );

        Ok(tree)
    }

    /// Get cached Merkle tree or build new one
    pub async fn get_merkle_tree(&self, branching_factor: usize) -> Result<MerkleTree> {
        let cache = self.merkle_cache.read().await;
        
        if let Some(tree) = cache.as_ref() {
            return Ok(tree.clone());
        }
        
        drop(cache);
        
        // Build new tree if not cached
        self.build_merkle_tree(branching_factor).await
    }

    /// Invalidate Merkle tree cache (call when content changes)
    pub async fn invalidate_cache(&self) {
        *self.merkle_cache.write().await = None;
        debug!("Invalidated Merkle tree cache");
    }

    /// Create sync request to send to peer
    pub async fn create_sync_request(
        &self,
        remote_node: NodeId,
        branching_factor: usize,
    ) -> Result<SyncRequest> {
        let tree = self.get_merkle_tree(branching_factor).await?;
        let clock = self.vector_clock.read().await.clone();

        Ok(SyncRequest {
            from_node: self.local_node.clone(),
            to_node: remote_node,
            merkle_tree: tree,
            vector_clock: clock,
        })
    }

    /// Handle incoming sync request
    pub async fn handle_sync_request(
        &self,
        request: SyncRequest,
        branching_factor: usize,
    ) -> Result<SyncResponse> {
        debug!(
            "Handling sync request from node {}",
            request.from_node
        );

        // Build local tree
        let local_tree = self.get_merkle_tree(branching_factor).await?;

        // Compare trees to find differences
        let missing = local_tree.diff(&request.merkle_tree);

        info!(
            "Found {} content items missing from {}",
            missing.len(),
            request.from_node
        );

        // Update vector clock
        let mut clock = self.vector_clock.write().await;
        clock.merge(&request.vector_clock);
        clock.increment(&self.local_node);

        Ok(SyncResponse {
            from_node: self.local_node.clone(),
            missing_content: missing,
            vector_clock: clock.clone(),
        })
    }

    /// Start sync session with peer
    pub async fn start_session(&self, remote_node: NodeId) -> String {
        let session = SyncSession::new(self.local_node.clone(), remote_node);
        let session_id = session.session_id.clone();

        self.active_sessions
            .write()
            .await
            .insert(session_id.clone(), session);

        info!("Started sync session {}", session_id);
        session_id
    }

    /// Update session statistics
    pub async fn update_session_stats(
        &self,
        session_id: &str,
        bytes_sent: u64,
        bytes_received: u64,
        content_sent: usize,
        content_received: usize,
    ) -> Result<()> {
        let mut sessions = self.active_sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .context("Session not found")?;

        session.bytes_sent += bytes_sent;
        session.bytes_received += bytes_received;
        session.content_sent += content_sent;
        session.content_received += content_received;

        Ok(())
    }

    /// End sync session
    pub async fn end_session(&self, session_id: &str) -> Option<SyncSession> {
        let session = self.active_sessions.write().await.remove(session_id);
        
        if let Some(ref s) = session {
            info!(
                "Ended sync session {} - Duration: {:?}, Throughput: {:.2} KB/s, Content sent: {}, Content received: {}",
                session_id,
                s.duration(),
                s.throughput() / 1024.0,
                s.content_sent,
                s.content_received
            );
        }

        session
    }

    /// Get active session
    pub async fn get_session(&self, session_id: &str) -> Option<SyncSession> {
        self.active_sessions.read().await.get(session_id).cloned()
    }

    /// Get all active sessions
    pub async fn active_sessions(&self) -> Vec<SyncSession> {
        self.active_sessions.read().await.values().cloned().collect()
    }

    /// Perform full sync with peer (orchestration method)
    pub async fn sync_with_peer(
        &self,
        remote_node: NodeId,
        branching_factor: usize,
    ) -> Result<SyncStats> {
        let session_id = self.start_session(remote_node.clone()).await;
        
        // Create and send sync request
        let request = self.create_sync_request(remote_node, branching_factor).await?;
        
        // This is a local simulation - in production this would send over network
        let response = self.handle_sync_request(request, branching_factor).await?;

        // Calculate stats
        let stats = SyncStats {
            session_id: session_id.clone(),
            content_differences: response.missing_content.len(),
            bytes_transferred: 0, // Would be calculated from actual transfer
            duration: std::time::Duration::from_secs(0), // Would be actual duration
        };

        self.end_session(&session_id).await;

        Ok(stats)
    }

    /// Get vector clock
    pub async fn get_vector_clock(&self) -> VectorClock {
        self.vector_clock.read().await.clone()
    }

    /// Update vector clock
    pub async fn update_vector_clock(&self, other: &VectorClock) {
        let mut clock = self.vector_clock.write().await;
        clock.merge(other);
        clock.increment(&self.local_node);
    }
}

/// Sync statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStats {
    pub session_id: String,
    pub content_differences: usize,
    pub bytes_transferred: u64,
    pub duration: std::time::Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_merkle_leaf_node() {
        let hash1 = ContentHash::from_bytes(b"content1");
        let hash2 = ContentHash::from_bytes(b"content2");
        let node = MerkleNode::leaf(vec![hash1, hash2]);

        assert!(node.is_leaf());
        assert_eq!(node.content.len(), 2);
        assert!(node.children.is_empty());
    }

    #[test]
    fn test_merkle_tree_build() {
        let hashes: Vec<ContentHash> = (0..10)
            .map(|i| ContentHash::from_bytes(format!("content{}", i).as_bytes()))
            .collect();

        let tree = MerkleTree::build(hashes.clone(), 3);
        
        // Check root exists
        assert!(!tree.root.is_leaf() || hashes.len() <= 3);
    }

    #[test]
    fn test_merkle_tree_empty() {
        let tree = MerkleTree::build(Vec::new(), 3);
        assert!(tree.root.is_leaf());
        assert!(tree.root.content.is_empty());
    }

    #[test]
    fn test_merkle_tree_diff_identical() {
        let hashes: Vec<ContentHash> = (0..5)
            .map(|i| ContentHash::from_bytes(format!("content{}", i).as_bytes()))
            .collect();

        let tree1 = MerkleTree::build(hashes.clone(), 3);
        let tree2 = MerkleTree::build(hashes, 3);

        let diff = tree1.diff(&tree2);
        assert!(diff.is_empty());
    }

    #[test]
    fn test_merkle_tree_diff_different() {
        let hashes1: Vec<ContentHash> = (0..5)
            .map(|i| ContentHash::from_bytes(format!("content{}", i).as_bytes()))
            .collect();

        let hashes2: Vec<ContentHash> = (0..7)
            .map(|i| ContentHash::from_bytes(format!("content{}", i).as_bytes()))
            .collect();

        let tree1 = MerkleTree::build(hashes1, 3);
        let tree2 = MerkleTree::build(hashes2, 3);

        let diff = tree1.diff(&tree2);
        assert!(!diff.is_empty());
    }

    #[test]
    fn test_tree_stats() {
        let hashes: Vec<ContentHash> = (0..10)
            .map(|i| ContentHash::from_bytes(format!("content{}", i).as_bytes()))
            .collect();

        let tree = MerkleTree::build(hashes, 3);
        let stats = tree.stats();

        assert_eq!(stats.total_content, 10);
        assert!(stats.leaf_nodes > 0);
        assert!(stats.total_nodes > 0);
    }

    #[tokio::test]
    async fn test_sync_protocol_creation() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let node_id = NodeId::new("test");

        let _protocol = SyncProtocol::new(node_id, store);
    }

    #[tokio::test]
    async fn test_build_merkle_tree() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        
        // Add some content
        store.put(b"content1", None).await.unwrap();
        store.put(b"content2", None).await.unwrap();
        store.put(b"content3", None).await.unwrap();

        let node_id = NodeId::new("test");
        let protocol = SyncProtocol::new(node_id, store);

        let tree = protocol.build_merkle_tree(3).await.unwrap();
        let stats = tree.stats();

        assert_eq!(stats.total_content, 3);
    }

    #[tokio::test]
    async fn test_merkle_tree_caching() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        store.put(b"content1", None).await.unwrap();

        let node_id = NodeId::new("test");
        let protocol = SyncProtocol::new(node_id, store);

        // Build tree (should cache)
        let tree1 = protocol.build_merkle_tree(3).await.unwrap();
        
        // Get cached tree
        let tree2 = protocol.get_merkle_tree(3).await.unwrap();
        
        assert_eq!(tree1.root_hash(), tree2.root_hash());
    }

    #[tokio::test]
    async fn test_create_sync_request() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        store.put(b"content", None).await.unwrap();

        let local_node = NodeId::new("local");
        let remote_node = NodeId::new("remote");
        let protocol = SyncProtocol::new(local_node.clone(), store);

        let request = protocol.create_sync_request(remote_node.clone(), 3).await.unwrap();

        assert_eq!(request.from_node, local_node);
        assert_eq!(request.to_node, remote_node);
    }

    #[tokio::test]
    async fn test_sync_session_lifecycle() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let protocol = SyncProtocol::new(NodeId::new("local"), store);

        let remote = NodeId::new("remote");
        let session_id = protocol.start_session(remote).await;

        // Check session exists
        let session = protocol.get_session(&session_id).await;
        assert!(session.is_some());

        // End session
        let ended = protocol.end_session(&session_id).await;
        assert!(ended.is_some());

        // Check session removed
        let session = protocol.get_session(&session_id).await;
        assert!(session.is_none());
    }

    #[tokio::test]
    async fn test_vector_clock_updates() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let node_id = NodeId::new("local");
        let protocol = SyncProtocol::new(node_id.clone(), store);

        let initial_clock = protocol.get_vector_clock().await;
        assert_eq!(initial_clock.get(&node_id), 1);

        // Update with another clock
        let mut other_clock = VectorClock::new();
        other_clock.increment(&NodeId::new("remote"));
        
        protocol.update_vector_clock(&other_clock).await;

        let updated_clock = protocol.get_vector_clock().await;
        assert_eq!(updated_clock.get(&NodeId::new("remote")), 1);
        assert_eq!(updated_clock.get(&node_id), 2); // Incremented after merge
    }
}
