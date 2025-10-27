//!<!-- # START OF FILE hainet-core/src/storage/coordinator.rs -->
//! Storage Coordinator
//!
//! Orchestrates distributed storage operations across the local hub mesh.
//! Manages master/slave coordination, health monitoring, rebalancing, and
//! automatic failover for the distributed storage system.
//!
//! ## Constitutional Compliance
//! - Article I (Privacy First): All coordination happens within local hub
//! - Article III (Decentralization): Dynamic master election, no permanent leader
//! - Article IV (Community Focus): Fair resource allocation across nodes

use super::cas::ContentAddressedStore;
use super::crdt::NodeId;
use super::distributed::{DistributedConfig, DistributedStorage, NodeCapacity, StorageStats};
use super::sync_protocol::SyncProtocol;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Node role in the storage mesh
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    /// Master coordinator (elected from available nodes)
    Master,
    /// Slave node providing storage capacity
    Slave,
    /// Standalone node (single-node deployment)
    Standalone,
}

/// Coordinator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorConfig {
    /// Health check interval
    pub health_check_interval_secs: u64,
    /// Node considered offline after this many missed checks
    pub offline_threshold: usize,
    /// Rebalancing threshold (usage difference between nodes)
    pub rebalance_threshold: f64,
    /// Automatic rebalancing enabled
    pub auto_rebalance: bool,
    /// Master election timeout
    pub election_timeout_secs: u64,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            health_check_interval_secs: 30,
            offline_threshold: 3,
            rebalance_threshold: 0.2, // 20% usage difference
            auto_rebalance: true,
            election_timeout_secs: 10,
        }
    }
}

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub node_id: NodeId,
    pub is_healthy: bool,
    pub last_seen: std::time::SystemTime,
    pub consecutive_failures: usize,
}

/// Rebalancing plan
#[derive(Debug, Clone)]
pub struct RebalancePlan {
    pub source_node: NodeId,
    pub target_node: NodeId,
    pub content_hashes: Vec<super::cas::ContentHash>,
    pub estimated_bytes: u64,
}

/// Storage coordinator
pub struct StorageCoordinator {
    local_node: NodeId,
    role: Arc<RwLock<NodeRole>>,
    distributed_storage: Arc<DistributedStorage>,
    sync_protocol: Arc<SyncProtocol>,
    config: CoordinatorConfig,
    health_checks: Arc<RwLock<HashMap<NodeId, HealthCheck>>>,
    is_running: Arc<RwLock<bool>>,
}

impl StorageCoordinator {
    /// Create new storage coordinator
    pub fn new(
        local_node: NodeId,
        store: ContentAddressedStore,
        storage_config: DistributedConfig,
        coordinator_config: CoordinatorConfig,
    ) -> Self {
        let distributed_storage = Arc::new(DistributedStorage::new(
            local_node.clone(),
            store.clone(),
            storage_config,
        ));

        let sync_protocol = Arc::new(SyncProtocol::new(local_node.clone(), store));

        Self {
            local_node,
            role: Arc::new(RwLock::new(NodeRole::Standalone)),
            distributed_storage,
            sync_protocol,
            config: coordinator_config,
            health_checks: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Get current node role
    pub async fn role(&self) -> NodeRole {
        *self.role.read().await
    }

    /// Set node role
    pub async fn set_role(&self, role: NodeRole) {
        *self.role.write().await = role;
        info!("Node {} role set to {:?}", self.local_node, role);
    }

    /// Start coordinator background tasks
    pub async fn start(&self) -> Result<()> {
        let mut running = self.is_running.write().await;
        if *running {
            return Ok(());
        }
        *running = true;
        drop(running);

        info!("Starting storage coordinator for node {}", self.local_node);

        // Start health monitoring task
        let coordinator = self.clone_arc();
        tokio::spawn(async move {
            coordinator.health_monitor_loop().await;
        });

        // Start rebalancing task if master
        if self.role().await == NodeRole::Master {
            let coordinator = self.clone_arc();
            tokio::spawn(async move {
                coordinator.rebalancing_loop().await;
            });
        }

        Ok(())
    }

    /// Stop coordinator
    pub async fn stop(&self) {
        *self.is_running.write().await = false;
        info!("Stopping storage coordinator");
    }

    /// Clone Arc references for background tasks
    fn clone_arc(&self) -> Arc<Self> {
        Arc::new(Self {
            local_node: self.local_node.clone(),
            role: Arc::clone(&self.role),
            distributed_storage: Arc::clone(&self.distributed_storage),
            sync_protocol: Arc::clone(&self.sync_protocol),
            config: self.config.clone(),
            health_checks: Arc::clone(&self.health_checks),
            is_running: Arc::clone(&self.is_running),
        })
    }

    /// Health monitoring loop
    async fn health_monitor_loop(self: Arc<Self>) {
        while *self.is_running.read().await {
            if let Err(e) = self.perform_health_checks().await {
                warn!("Health check failed: {}", e);
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(
                self.config.health_check_interval_secs,
            ))
            .await;
        }
    }

    /// Perform health checks on all nodes
    async fn perform_health_checks(&self) -> Result<()> {
        let online_nodes = self.distributed_storage.online_nodes().await;
        let mut health_checks = self.health_checks.write().await;

        for node_id in online_nodes {
            if node_id == self.local_node {
                continue; // Don't check self
            }

            // In production, this would ping the node
            // For now, we just update the health check record
            let check = health_checks.entry(node_id.clone()).or_insert_with(|| {
                HealthCheck {
                    node_id: node_id.clone(),
                    is_healthy: true,
                    last_seen: std::time::SystemTime::now(),
                    consecutive_failures: 0,
                }
            });

            // Update last seen
            check.last_seen = std::time::SystemTime::now();
            check.is_healthy = true;
            check.consecutive_failures = 0;
        }

        // Mark nodes as unhealthy if they haven't been seen
        for check in health_checks.values_mut() {
            let elapsed = check
                .last_seen
                .elapsed()
                .unwrap_or(std::time::Duration::from_secs(0));

            if elapsed.as_secs() > self.config.health_check_interval_secs * 2 {
                check.consecutive_failures += 1;

                if check.consecutive_failures >= self.config.offline_threshold {
                    check.is_healthy = false;
                    self.distributed_storage
                        .mark_node_offline(&check.node_id)
                        .await;
                }
            }
        }

        debug!("Completed health checks for {} nodes", health_checks.len());
        Ok(())
    }

    /// Rebalancing loop (only runs on master)
    async fn rebalancing_loop(self: Arc<Self>) {
        while *self.is_running.read().await {
            if self.role().await != NodeRole::Master {
                break;
            }

            if self.config.auto_rebalance {
                if let Err(e) = self.check_and_rebalance().await {
                    warn!("Rebalancing failed: {}", e);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(300)).await; // Every 5 minutes
        }
    }

    /// Check storage balance and rebalance if needed
    async fn check_and_rebalance(&self) -> Result<()> {
        let stats = self.distributed_storage.stats().await;

        if stats.online_nodes < 2 {
            debug!("Not enough nodes for rebalancing");
            return Ok(());
        }

        // Find nodes that need rebalancing
        let plans = self.create_rebalancing_plans().await?;

        if plans.is_empty() {
            debug!("No rebalancing needed");
            return Ok(());
        }

        info!("Executing {} rebalancing plans", plans.len());

        for plan in plans {
            self.execute_rebalancing_plan(plan).await?;
        }

        Ok(())
    }

    /// Create rebalancing plans
    async fn create_rebalancing_plans(&self) -> Result<Vec<RebalancePlan>> {
        // This is a simplified version
        // In production, this would analyze actual node usage and create optimal plans
        Ok(Vec::new())
    }

    /// Execute rebalancing plan
    async fn execute_rebalancing_plan(&self, plan: RebalancePlan) -> Result<()> {
        info!(
            "Rebalancing {} items from {} to {}",
            plan.content_hashes.len(),
            plan.source_node,
            plan.target_node
        );

        // In production, this would:
        // 1. Copy content from source to target
        // 2. Verify successful copy
        // 3. Update replication metadata
        // 4. Remove from source if over-replicated

        Ok(())
    }

    /// Elect new master (simple algorithm - lowest node ID wins)
    pub async fn elect_master(&self, candidates: Vec<NodeId>) -> Option<NodeId> {
        if candidates.is_empty() {
            return None;
        }

        // Simple election: lowest lexicographic node ID
        let mut sorted = candidates;
        sorted.sort_by(|a, b| a.0.cmp(&b.0));

        Some(sorted[0].clone())
    }

    /// Join storage mesh as slave
    pub async fn join_mesh(&self, master_node: NodeId) -> Result<()> {
        info!("Joining storage mesh with master {}", master_node);

        self.set_role(NodeRole::Slave).await;

        // Register with master (in production, this would be network call)
        // For now, we just log it

        Ok(())
    }

    /// Promote to master
    pub async fn promote_to_master(&self) -> Result<()> {
        info!("Promoting node {} to master", self.local_node);

        self.set_role(NodeRole::Master).await;

        // Start master-specific tasks
        let coordinator = self.clone_arc();
        tokio::spawn(async move {
            coordinator.rebalancing_loop().await;
        });

        Ok(())
    }

    /// Get storage statistics
    pub async fn get_stats(&self) -> StorageStats {
        self.distributed_storage.stats().await
    }

    /// Get health status
    pub async fn get_health_status(&self) -> Vec<HealthCheck> {
        self.health_checks.read().await.values().cloned().collect()
    }

    /// Trigger manual rebalancing
    pub async fn trigger_rebalancing(&self) -> Result<()> {
        if self.role().await != NodeRole::Master {
            anyhow::bail!("Only master can trigger rebalancing");
        }

        self.check_and_rebalance().await
    }

    /// Get distributed storage reference
    pub fn distributed_storage(&self) -> &Arc<DistributedStorage> {
        &self.distributed_storage
    }

    /// Get sync protocol reference
    pub fn sync_protocol(&self) -> &Arc<SyncProtocol> {
        &self.sync_protocol
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_coordinator_creation() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let node_id = NodeId::new("test");

        let coordinator = StorageCoordinator::new(
            node_id,
            store,
            DistributedConfig::default(),
            CoordinatorConfig::default(),
        );

        assert_eq!(coordinator.role().await, NodeRole::Standalone);
    }

    #[tokio::test]
    async fn test_set_role() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let node_id = NodeId::new("test");

        let coordinator = StorageCoordinator::new(
            node_id,
            store,
            DistributedConfig::default(),
            CoordinatorConfig::default(),
        );

        coordinator.set_role(NodeRole::Master).await;
        assert_eq!(coordinator.role().await, NodeRole::Master);

        coordinator.set_role(NodeRole::Slave).await;
        assert_eq!(coordinator.role().await, NodeRole::Slave);
    }

    #[tokio::test]
    async fn test_master_election() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let node_id = NodeId::new("local");

        let coordinator = StorageCoordinator::new(
            node_id,
            store,
            DistributedConfig::default(),
            CoordinatorConfig::default(),
        );

        let candidates = vec![
            NodeId::new("node3"),
            NodeId::new("node1"),
            NodeId::new("node2"),
        ];

        let master = coordinator.elect_master(candidates).await;
        assert_eq!(master, Some(NodeId::new("node1"))); // Lowest lexicographic
    }

    #[tokio::test]
    async fn test_master_election_empty() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let node_id = NodeId::new("local");

        let coordinator = StorageCoordinator::new(
            node_id,
            store,
            DistributedConfig::default(),
            CoordinatorConfig::default(),
        );

        let master = coordinator.elect_master(Vec::new()).await;
        assert_eq!(master, None);
    }

    #[tokio::test]
    async fn test_join_mesh() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let node_id = NodeId::new("slave");

        let coordinator = StorageCoordinator::new(
            node_id,
            store,
            DistributedConfig::default(),
            CoordinatorConfig::default(),
        );

        let master = NodeId::new("master");
        coordinator.join_mesh(master).await.unwrap();

        assert_eq!(coordinator.role().await, NodeRole::Slave);
    }

    #[tokio::test]
    async fn test_promote_to_master() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let node_id = NodeId::new("node");

        let coordinator = StorageCoordinator::new(
            node_id,
            store,
            DistributedConfig::default(),
            CoordinatorConfig::default(),
        );

        coordinator.promote_to_master().await.unwrap();
        assert_eq!(coordinator.role().await, NodeRole::Master);
    }

    #[tokio::test]
    async fn test_get_stats() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let node_id = NodeId::new("node");

        let coordinator = StorageCoordinator::new(
            node_id,
            store,
            DistributedConfig::default(),
            CoordinatorConfig::default(),
        );

        let stats = coordinator.get_stats().await;
        assert_eq!(stats.online_nodes, 0);
        assert_eq!(stats.total_nodes, 0);
    }

    #[tokio::test]
    async fn test_health_checks_initialization() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();
        let node_id = NodeId::new("node");

        let coordinator = StorageCoordinator::new(
            node_id,
            store,
            DistributedConfig::default(),
            CoordinatorConfig::default(),
        );

        let health = coordinator.get_health_status().await;
        assert!(health.is_empty());
    }
}
