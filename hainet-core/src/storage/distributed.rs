//!<!-- # START OF FILE hainet-core/src/storage/distributed.rs -->
//! Distributed Storage Layer
//!
//! Enables content distribution and replication across multiple nodes in the local hub.
//! Uses CRDT-based metadata for tracking content location and replication status.
//!
//! ## Constitutional Compliance
//! - Article I (Privacy First): All storage remains within local hub mesh
//! - Article III (Decentralization): No central coordinator for storage decisions
//! - Article IV (Community Focus): Voluntary resource sharing with configurable quotas

use super::cas::{ContentAddressedStore, ContentHash};
use super::crdt::{LWWElementSet, NodeId, Timestamp};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Node capability information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapacity {
    pub node_id: NodeId,
    pub available_bytes: u64,
    pub total_bytes: u64,
    pub cpu_cores: usize,
    pub is_online: bool,
    pub last_seen: std::time::SystemTime,
}

impl NodeCapacity {
    /// Calculate usage percentage (0.0 - 1.0)
    pub fn usage(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        let used = self.total_bytes.saturating_sub(self.available_bytes);
        used as f64 / self.total_bytes as f64
    }

    /// Check if node has capacity for content
    pub fn can_store(&self, size: u64) -> bool {
        self.is_online && self.available_bytes >= size
    }
}

/// Content replication metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationMetadata {
    pub hash: ContentHash,
    pub size: u64,
    pub replicas: LWWElementSet<NodeId>,
    pub desired_replicas: usize,
    pub created_at: std::time::SystemTime,
}

impl ReplicationMetadata {
    /// Create new replication metadata
    pub fn new(hash: ContentHash, size: u64, desired_replicas: usize) -> Self {
        Self {
            hash,
            size,
            replicas: LWWElementSet::new(),
            desired_replicas,
            created_at: std::time::SystemTime::now(),
        }
    }

    /// Add replica location
    pub fn add_replica(&mut self, node_id: NodeId, timestamp: Timestamp) {
        self.replicas.insert(node_id, timestamp);
    }

    /// Remove replica location
    pub fn remove_replica(&mut self, node_id: NodeId, timestamp: Timestamp) {
        self.replicas.remove(node_id, timestamp);
    }

    /// Check if content is on specific node
    pub fn is_on_node(&self, node_id: &NodeId) -> bool {
        self.replicas.contains(node_id)
    }

    /// Get list of nodes with this content
    pub fn replica_nodes(&self) -> Vec<NodeId> {
        self.replicas.elements().cloned().collect()
    }

    /// Check if replication goal is met
    pub fn is_sufficiently_replicated(&self) -> bool {
        self.replicas.len() >= self.desired_replicas
    }

    /// Get replication health (0.0 - 1.0)
    pub fn health(&self) -> f64 {
        let current = self.replicas.len() as f64;
        let desired = self.desired_replicas as f64;
        (current / desired).min(1.0)
    }
}

/// Storage allocation strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AllocationStrategy {
    /// Prefer nodes with most available space
    MostAvailable,
    /// Balance usage across all nodes
    Balanced,
    /// Prefer nodes with least CPU load
    LeastLoaded,
    /// Random selection
    Random,
}

/// Distributed storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedConfig {
    /// Default replication factor
    pub default_replication_factor: usize,
    /// Allocation strategy
    pub allocation_strategy: AllocationStrategy,
    /// Minimum free space threshold (bytes)
    pub min_free_space: u64,
    /// Maximum content size for single node (bytes)
    pub max_content_size: u64,
    /// Garbage collection threshold (usage percentage)
    pub gc_threshold: f64,
}

impl Default for DistributedConfig {
    fn default() -> Self {
        Self {
            default_replication_factor: 2,
            allocation_strategy: AllocationStrategy::Balanced,
            min_free_space: 1024 * 1024 * 1024, // 1 GB
            max_content_size: 100 * 1024 * 1024, // 100 MB
            gc_threshold: 0.9,                    // 90% usage
        }
    }
}

/// Distributed storage manager
pub struct DistributedStorage {
    local_node: NodeId,
    local_store: ContentAddressedStore,
    config: DistributedConfig,
    node_capacities: Arc<RwLock<HashMap<NodeId, NodeCapacity>>>,
    replication_metadata: Arc<RwLock<HashMap<ContentHash, ReplicationMetadata>>>,
    timestamp_counter: Arc<RwLock<u64>>,
}

impl DistributedStorage {
    /// Create new distributed storage manager
    pub fn new(
        local_node: NodeId,
        local_store: ContentAddressedStore,
        config: DistributedConfig,
    ) -> Self {
        Self {
            local_node,
            local_store,
            config,
            node_capacities: Arc::new(RwLock::new(HashMap::new())),
            replication_metadata: Arc::new(RwLock::new(HashMap::new())),
            timestamp_counter: Arc::new(RwLock::new(0)),
        }
    }

    /// Get next timestamp
    async fn next_timestamp(&self) -> Timestamp {
        let mut counter = self.timestamp_counter.write().await;
        *counter += 1;
        Timestamp::now(*counter)
    }

    /// Register node capacity
    pub async fn register_node(&self, capacity: NodeCapacity) {
        let node_id = capacity.node_id.clone();
        self.node_capacities
            .write()
            .await
            .insert(node_id.clone(), capacity);
        info!("Registered node {} capacity", node_id);
    }

    /// Update node capacity
    pub async fn update_node_capacity(&self, node_id: &NodeId, capacity: NodeCapacity) {
        self.node_capacities
            .write()
            .await
            .insert(node_id.clone(), capacity);
        debug!("Updated capacity for node {}", node_id);
    }

    /// Mark node as offline
    pub async fn mark_node_offline(&self, node_id: &NodeId) {
        if let Some(capacity) = self.node_capacities.write().await.get_mut(node_id) {
            capacity.is_online = false;
            warn!("Node {} marked offline", node_id);
        }
    }

    /// Get all online nodes
    pub async fn online_nodes(&self) -> Vec<NodeId> {
        self.node_capacities
            .read()
            .await
            .values()
            .filter(|c| c.is_online)
            .map(|c| c.node_id.clone())
            .collect()
    }

    /// Select nodes for content placement
    pub async fn select_storage_nodes(
        &self,
        content_size: u64,
        replica_count: usize,
    ) -> Result<Vec<NodeId>> {
        let capacities = self.node_capacities.read().await;

        // Filter nodes that can store content
        let mut candidates: Vec<_> = capacities
            .values()
            .filter(|c| c.can_store(content_size) && c.available_bytes >= self.config.min_free_space)
            .collect();

        if candidates.is_empty() {
            anyhow::bail!("No nodes available with sufficient capacity");
        }

        // Sort by allocation strategy
        match self.config.allocation_strategy {
            AllocationStrategy::MostAvailable => {
                candidates.sort_by(|a, b| b.available_bytes.cmp(&a.available_bytes));
            }
            AllocationStrategy::Balanced => {
                candidates.sort_by(|a, b| a.usage().partial_cmp(&b.usage()).unwrap());
            }
            AllocationStrategy::LeastLoaded => {
                candidates.sort_by(|a, b| a.cpu_cores.cmp(&b.cpu_cores).reverse());
            }
            AllocationStrategy::Random => {
                // Already random order from HashMap
            }
        }

        // Select top N candidates
        let selected: Vec<NodeId> = candidates
            .into_iter()
            .take(replica_count)
            .map(|c| c.node_id.clone())
            .collect();

        if selected.len() < replica_count {
            warn!(
                "Only {} nodes available, requested {} replicas",
                selected.len(),
                replica_count
            );
        }

        Ok(selected)
    }

    /// Store content with replication
    pub async fn store(
        &self,
        content: &[u8],
        replica_count: Option<usize>,
    ) -> Result<(ContentHash, Vec<NodeId>)> {
        let content_size = content.len() as u64;

        // Validate content size
        if content_size > self.config.max_content_size {
            anyhow::bail!(
                "Content size {} exceeds maximum {}",
                content_size,
                self.config.max_content_size
            );
        }

        // Store locally first
        let hash = self
            .local_store
            .put(content, None)
            .await
            .context("Failed to store content locally")?;

        // Determine replica count
        let replicas = replica_count.unwrap_or(self.config.default_replication_factor);

        // Create replication metadata
        let mut metadata = ReplicationMetadata::new(hash, content_size, replicas);
        let timestamp = self.next_timestamp().await;
        metadata.add_replica(self.local_node.clone(), timestamp);

        // Select additional nodes for replication (only if we need more than local)
        let target_nodes = if replicas > 1 {
            self.select_storage_nodes(content_size, replicas - 1)
                .await
                .unwrap_or_else(|_| Vec::new()) // Gracefully handle no nodes available
        } else {
            Vec::new()
        };

        // Store metadata
        self.replication_metadata
            .write()
            .await
            .insert(hash, metadata);

        info!(
            "Stored content {} ({} bytes) with {} replicas planned",
            hash,
            content_size,
            replicas
        );

        Ok((hash, target_nodes))
    }

    /// Record replica creation on remote node
    pub async fn record_replica(&self, hash: ContentHash, node_id: NodeId) -> Result<()> {
        let mut metadata_map = self.replication_metadata.write().await;
        let metadata = metadata_map
            .get_mut(&hash)
            .context("Replication metadata not found")?;

        let timestamp = self.next_timestamp().await;
        metadata.add_replica(node_id.clone(), timestamp);

        info!("Recorded replica of {} on node {}", hash, node_id);
        Ok(())
    }

    /// Get replication metadata
    pub async fn get_metadata(&self, hash: &ContentHash) -> Option<ReplicationMetadata> {
        self.replication_metadata.read().await.get(hash).cloned()
    }

    /// Get content locations
    pub async fn locate_content(&self, hash: &ContentHash) -> Vec<NodeId> {
        self.replication_metadata
            .read()
            .await
            .get(hash)
            .map(|m| m.replica_nodes())
            .unwrap_or_default()
    }

    /// Check replication health for all content
    pub async fn check_replication_health(&self) -> Vec<(ContentHash, f64)> {
        self.replication_metadata
            .read()
            .await
            .iter()
            .map(|(hash, metadata)| (*hash, metadata.health()))
            .collect()
    }

    /// Get under-replicated content
    pub async fn under_replicated_content(&self) -> Vec<ContentHash> {
        self.replication_metadata
            .read()
            .await
            .iter()
            .filter(|(_, m)| !m.is_sufficiently_replicated())
            .map(|(h, _)| *h)
            .collect()
    }

    /// Delete content and update metadata
    pub async fn delete(&self, hash: &ContentHash) -> Result<()> {
        // Delete from local store
        self.local_store.delete(hash).await?;

        // Remove from replication metadata
        let mut metadata_map = self.replication_metadata.write().await;
        if let Some(metadata) = metadata_map.get_mut(hash) {
            let timestamp = self.next_timestamp().await;
            metadata.remove_replica(self.local_node.clone(), timestamp);

            // If no more replicas, remove metadata entirely
            if metadata.replicas.is_empty() {
                metadata_map.remove(hash);
            }
        }

        info!("Deleted content {}", hash);
        Ok(())
    }

    /// Garbage collection - remove orphaned content
    pub async fn garbage_collect(&self) -> Result<Vec<ContentHash>> {
        let mut removed = Vec::new();

        // Get nodes that are offline or over threshold
        let capacities = self.node_capacities.read().await;
        let overloaded_nodes: Vec<_> = capacities
            .values()
            .filter(|c| c.usage() >= self.config.gc_threshold)
            .map(|c| c.node_id.clone())
            .collect();

        if overloaded_nodes.is_empty() {
            debug!("No nodes require garbage collection");
            return Ok(removed);
        }

        drop(capacities); // Release lock

        // Find content to remove
        let metadata_map = self.replication_metadata.read().await;
        for (hash, metadata) in metadata_map.iter() {
            // If sufficiently replicated and on overloaded node
            if metadata.is_sufficiently_replicated()
                && metadata.is_on_node(&self.local_node)
                && overloaded_nodes.contains(&self.local_node)
            {
                // Check if can safely remove local copy
                let other_replicas: Vec<_> = metadata
                    .replica_nodes()
                    .into_iter()
                    .filter(|n| n != &self.local_node)
                    .collect();

                if !other_replicas.is_empty() {
                    // Safe to remove - other replicas exist
                    removed.push(*hash);
                }
            }
        }

        drop(metadata_map); // Release lock

        // Remove selected content
        for hash in &removed {
            self.delete(hash).await?;
        }

        info!("Garbage collection removed {} items", removed.len());
        Ok(removed)
    }

    /// Get storage statistics
    pub async fn stats(&self) -> StorageStats {
        let metadata = self.replication_metadata.read().await;
        let capacities = self.node_capacities.read().await;

        let total_content = metadata.len();
        let total_replicas: usize = metadata.values().map(|m| m.replicas.len()).sum();
        let under_replicated = metadata
            .values()
            .filter(|m| !m.is_sufficiently_replicated())
            .count();

        let online_nodes = capacities.values().filter(|c| c.is_online).count();
        let total_nodes = capacities.len();

        let total_capacity: u64 = capacities.values().map(|c| c.total_bytes).sum();
        let available_capacity: u64 = capacities.values().map(|c| c.available_bytes).sum();

        StorageStats {
            total_content,
            total_replicas,
            under_replicated,
            online_nodes,
            total_nodes,
            total_capacity,
            available_capacity,
        }
    }
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_content: usize,
    pub total_replicas: usize,
    pub under_replicated: usize,
    pub online_nodes: usize,
    pub total_nodes: usize,
    pub total_capacity: u64,
    pub available_capacity: u64,
}

impl StorageStats {
    /// Get overall usage percentage
    pub fn usage(&self) -> f64 {
        if self.total_capacity == 0 {
            return 0.0;
        }
        let used = self.total_capacity - self.available_capacity;
        used as f64 / self.total_capacity as f64
    }

    /// Get average replicas per content
    pub fn avg_replicas(&self) -> f64 {
        if self.total_content == 0 {
            return 0.0;
        }
        self.total_replicas as f64 / self.total_content as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_node_capacity_usage() {
        let capacity = NodeCapacity {
            node_id: NodeId::new("test"),
            available_bytes: 500,
            total_bytes: 1000,
            cpu_cores: 4,
            is_online: true,
            last_seen: std::time::SystemTime::now(),
        };

        assert_eq!(capacity.usage(), 0.5);
        assert!(capacity.can_store(400));
        assert!(!capacity.can_store(600));
    }

    #[test]
    fn test_replication_metadata() {
        let hash = ContentHash::from_bytes(b"test");
        let mut metadata = ReplicationMetadata::new(hash, 1024, 3);

        let node1 = NodeId::new("node1");
        let node2 = NodeId::new("node2");
        let ts = Timestamp::now(1);

        metadata.add_replica(node1.clone(), ts);
        assert!(metadata.is_on_node(&node1));
        assert!(!metadata.is_sufficiently_replicated());

        metadata.add_replica(node2.clone(), ts);
        assert_eq!(metadata.replica_nodes().len(), 2);
        assert!(!metadata.is_sufficiently_replicated());

        metadata.add_replica(NodeId::new("node3"), ts);
        assert!(metadata.is_sufficiently_replicated());
        assert_eq!(metadata.health(), 1.0);
    }

    #[tokio::test]
    async fn test_distributed_storage_creation() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let node_id = NodeId::new("local");
        let config = DistributedConfig::default();

        let _storage = DistributedStorage::new(node_id, store, config);
    }

    #[tokio::test]
    async fn test_node_registration() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let node_id = NodeId::new("local");
        let config = DistributedConfig::default();
        let storage = DistributedStorage::new(node_id, store, config);

        let capacity = NodeCapacity {
            node_id: NodeId::new("remote1"),
            available_bytes: 10_000_000,
            total_bytes: 100_000_000,
            cpu_cores: 4,
            is_online: true,
            last_seen: std::time::SystemTime::now(),
        };

        storage.register_node(capacity).await;

        let online = storage.online_nodes().await;
        assert_eq!(online.len(), 1);
    }

    #[tokio::test]
    async fn test_node_selection_most_available() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let node_id = NodeId::new("local");
        let mut config = DistributedConfig::default();
        config.allocation_strategy = AllocationStrategy::MostAvailable;
        config.min_free_space = 1000;

        let storage = DistributedStorage::new(node_id, store, config);

        // Register nodes with different capacities
        storage
            .register_node(NodeCapacity {
                node_id: NodeId::new("node1"),
                available_bytes: 5000,
                total_bytes: 10000,
                cpu_cores: 2,
                is_online: true,
                last_seen: std::time::SystemTime::now(),
            })
            .await;

        storage
            .register_node(NodeCapacity {
                node_id: NodeId::new("node2"),
                available_bytes: 8000,
                total_bytes: 10000,
                cpu_cores: 4,
                is_online: true,
                last_seen: std::time::SystemTime::now(),
            })
            .await;

        let selected = storage.select_storage_nodes(1000, 2).await.unwrap();
        assert_eq!(selected.len(), 2);
        // node2 should be first (most available)
        assert_eq!(selected[0], NodeId::new("node2"));
    }

    #[tokio::test]
    async fn test_store_and_locate() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let node_id = NodeId::new("local");
        let mut config = DistributedConfig::default();
        config.min_free_space = 0; // Allow storage even with no registered nodes
        let storage = DistributedStorage::new(node_id.clone(), store, config);

        let data = b"test content for distribution";
        let (hash, _) = storage.store(data, Some(1)).await.unwrap(); // Only 1 replica (local)

        let locations = storage.locate_content(&hash).await;
        assert_eq!(locations.len(), 1); // Only local for now
        assert_eq!(locations[0], node_id);
    }

    #[tokio::test]
    async fn test_replication_health() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let node_id = NodeId::new("local");
        let mut config = DistributedConfig::default();
        config.min_free_space = 0; // Allow storage even with no registered nodes
        let storage = DistributedStorage::new(node_id, store, config);

        let data = b"health test";
        let (hash, _) = storage.store(data, Some(3)).await.unwrap();

        let health = storage.check_replication_health().await;
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].0, hash);
        // 1 replica out of 3 desired = 0.33 health
        assert!((health[0].1 - 0.333).abs() < 0.01);

        let under_rep = storage.under_replicated_content().await;
        assert_eq!(under_rep.len(), 1);
        assert_eq!(under_rep[0], hash);
    }

    #[tokio::test]
    async fn test_stats() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let node_id = NodeId::new("local");
        let mut config = DistributedConfig::default();
        config.min_free_space = 0; // Allow storage even with no registered nodes
        let storage = DistributedStorage::new(node_id, store, config);

        storage
            .register_node(NodeCapacity {
                node_id: NodeId::new("node1"),
                available_bytes: 6000,
                total_bytes: 10000,
                cpu_cores: 2,
                is_online: true,
                last_seen: std::time::SystemTime::now(),
            })
            .await;

        storage.store(b"test1", Some(1)).await.unwrap(); // Only 1 replica (local)
        storage.store(b"test2", Some(1)).await.unwrap(); // Only 1 replica (local)

        let stats = storage.stats().await;
        assert_eq!(stats.total_content, 2);
        assert_eq!(stats.online_nodes, 1);
        assert_eq!(stats.total_nodes, 1);
    }
}
