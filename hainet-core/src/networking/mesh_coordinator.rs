//! <!-- # START OF FILE hainet-core/src/networking/mesh_coordinator.rs -->
//! Mesh Coordinator for HAI-Net Master-Slave Coordination
//! 
//! This module implements the master election algorithm and role assignment logic
//! for the HAI-Net mesh network. It manages the mesh topology and coordinates
//! role negotiations between devices.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use libp2p::PeerId;
use tokio::sync::RwLock;
use std::sync::Arc;
use anyhow::{anyhow, Result};
use tracing::{info, warn, debug, error};

use super::peer_discovery::{DeviceCapabilities, DeviceRole, PeerInfo};
use super::registry::{DeviceRegistry, RegistryEvent};

/// Mesh topology state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshState {
    /// Initial state - no master elected
    Initializing,
    /// Election in progress
    Electing,
    /// Master elected, topology being established
    Establishing,
    /// Mesh fully operational
    Operational,
    /// Master failed, re-election needed
    MasterFailure,
    /// Network partition detected
    Partitioned,
}

/// Role assignment for a specific device
#[derive(Debug, Clone)]
pub struct RoleAssignment {
    pub peer_id: PeerId,
    pub assigned_role: DeviceRole,
    pub specialized_roles: Vec<SpecializedRole>,
    pub assigned_at: SystemTime,
}

/// Specialized roles for slave devices
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecializedRole {
    /// Hosts LLM models (requires GPU or high RAM)
    LLMHost,
    /// Handles speech-to-text / text-to-speech (good CPU)
    STTTTSHost,
    /// Runs MCP servers (stable network)
    MCPServerHost,
    /// Provides storage capacity (high disk space)
    StorageNode,
    /// General compute worker
    ComputeWorker,
}

/// Master election result
#[derive(Debug, Clone)]
pub struct ElectionResult {
    pub master_peer_id: PeerId,
    pub master_score: f64,
    pub runner_up_peer_id: Option<PeerId>,
    pub runner_up_score: Option<f64>,
    pub total_candidates: usize,
    pub elected_at: SystemTime,
}

impl ElectionResult {
    /// Check if election was unanimous (single candidate)
    pub fn is_unanimous(&self) -> bool {
        self.total_candidates == 1
    }

    /// Get score margin between master and runner-up
    pub fn score_margin(&self) -> f64 {
        match self.runner_up_score {
            Some(runner_up) => self.master_score - runner_up,
            None => self.master_score,
        }
    }
}

/// Mesh topology coordinator
pub struct MeshCoordinator {
    local_peer_id: PeerId,
    _local_capabilities: DeviceCapabilities,
    state: Arc<RwLock<MeshState>>,
    current_master: Arc<RwLock<Option<PeerId>>>,
    role_assignments: Arc<RwLock<HashMap<PeerId, RoleAssignment>>>,
    registry: Arc<DeviceRegistry>,
    election_timeout: Duration,
    last_election: Arc<RwLock<Option<SystemTime>>>,
}

impl MeshCoordinator {
    /// Create a new mesh coordinator
    pub fn new(
        local_peer_id: PeerId,
        local_capabilities: DeviceCapabilities,
        registry: Arc<DeviceRegistry>,
    ) -> Self {
        Self {
            local_peer_id,
            _local_capabilities: local_capabilities,
            state: Arc::new(RwLock::new(MeshState::Initializing)),
            current_master: Arc::new(RwLock::new(None)),
            role_assignments: Arc::new(RwLock::new(HashMap::new())),
            registry,
            election_timeout: Duration::from_secs(30),
            last_election: Arc::new(RwLock::new(None)),
        }
    }

    /// Create with custom election timeout
    pub fn with_timeout(
        local_peer_id: PeerId,
        local_capabilities: DeviceCapabilities,
        registry: Arc<DeviceRegistry>,
        election_timeout: Duration,
    ) -> Self {
        let mut coordinator = Self::new(local_peer_id, local_capabilities, registry);
        coordinator.election_timeout = election_timeout;
        coordinator
    }

    /// Get current mesh state
    pub async fn state(&self) -> MeshState {
        *self.state.read().await
    }

    /// Set mesh state
    async fn set_state(&self, new_state: MeshState) {
        let mut state = self.state.write().await;
        if *state != new_state {
            info!("Mesh state transition: {:?} -> {:?}", *state, new_state);
            *state = new_state;
        }
    }

    /// Get current master peer ID
    pub async fn current_master(&self) -> Option<PeerId> {
        *self.current_master.read().await
    }

    /// Check if this node is the master
    pub async fn is_master(&self) -> bool {
        let master = self.current_master.read().await;
        master.map_or(false, |m| m == self.local_peer_id)
    }

    /// Elect a master from available candidates
    pub async fn elect_master(&self, candidates: Vec<PeerInfo>) -> Result<ElectionResult> {
        if candidates.is_empty() {
            return Err(anyhow!("No candidates available for master election"));
        }

        self.set_state(MeshState::Electing).await;
        info!("Starting master election with {} candidates", candidates.len());

        // Sort candidates by capability score (descending)
        let mut sorted_candidates: Vec<_> = candidates
            .into_iter()
            .map(|p| {
                let score = p.capabilities.calculate_score();
                (p, score)
            })
            .collect();
        
        sorted_candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Handle tie-breaking: if top scores are equal, use peer_id as tiebreaker
        let top_score = sorted_candidates[0].1;
        let tied_candidates: Vec<_> = sorted_candidates
            .iter()
            .filter(|(_, score)| (*score - top_score).abs() < 0.01)
            .collect();

        let (master_peer, master_score) = if tied_candidates.len() > 1 {
            // Tie detected - use peer_id as deterministic tiebreaker
            info!("Tie detected among {} candidates with score {}", tied_candidates.len(), top_score);
            let winner = tied_candidates
                .iter()
                .max_by_key(|(peer, _)| peer.peer_id)
                .unwrap();
            (winner.0.clone(), winner.1)
        } else {
            (sorted_candidates[0].0.clone(), sorted_candidates[0].1)
        };

        let runner_up = sorted_candidates.get(1).map(|(p, s)| (p.peer_id, *s));

        let result = ElectionResult {
            master_peer_id: master_peer.peer_id,
            master_score,
            runner_up_peer_id: runner_up.map(|(id, _)| id),
            runner_up_score: runner_up.map(|(_, score)| score),
            total_candidates: sorted_candidates.len(),
            elected_at: SystemTime::now(),
        };

        // Update coordinator state
        let mut master = self.current_master.write().await;
        *master = Some(result.master_peer_id);

        let mut last_election = self.last_election.write().await;
        *last_election = Some(result.elected_at);

        info!(
            "Master elected: {} (score: {:.1}, margin: {:.1})",
            result.master_peer_id,
            result.master_score,
            result.score_margin()
        );

        Ok(result)
    }

    /// Assign roles to all devices in the mesh
    pub async fn assign_roles(&self, devices: Vec<PeerInfo>) -> Result<HashMap<PeerId, RoleAssignment>> {
        self.set_state(MeshState::Establishing).await;
        info!("Starting role assignment for {} devices", devices.len());

        let master_id = self.current_master()
            .await
            .ok_or_else(|| anyhow!("No master elected"))?;

        let mut assignments = HashMap::new();

        // Assign master role
        let master_assignment = RoleAssignment {
            peer_id: master_id,
            assigned_role: DeviceRole::Master,
            specialized_roles: vec![
                SpecializedRole::MCPServerHost,  // Master always hosts MCP servers
            ],
            assigned_at: SystemTime::now(),
        };
        assignments.insert(master_id, master_assignment);

        // Collect slave devices (all except master)
        let mut slaves: Vec<_> = devices
            .into_iter()
            .filter(|p| p.peer_id != master_id)
            .collect();

        // Sort slaves by capability score for optimal assignment
        slaves.sort_by(|a, b| {
            b.capabilities.calculate_score()
                .partial_cmp(&a.capabilities.calculate_score())
                .unwrap()
        });

        // Assign specialized roles to slaves
        let mut llm_hosts_assigned = 0;
        let mut stt_tts_hosts_assigned = 0;
        let mut storage_nodes_assigned = 0;

        for slave in slaves {
            let mut specialized_roles = Vec::new();

            // LLM Host: Requires GPU or high RAM (>= 16GB)
            if slave.capabilities.has_gpu || slave.capabilities.ram_gb >= 16 {
                if llm_hosts_assigned < 2 {  // Limit to 2 LLM hosts
                    specialized_roles.push(SpecializedRole::LLMHost);
                    llm_hosts_assigned += 1;
                }
            }

            // STT/TTS Host: Good CPU (>= 4 cores)
            if slave.capabilities.cpu_cores >= 4 && stt_tts_hosts_assigned < 2 {
                specialized_roles.push(SpecializedRole::STTTTSHost);
                stt_tts_hosts_assigned += 1;
            }

            // Storage Node: High disk space (>= 500GB)
            if slave.capabilities.disk_gb >= 500 && storage_nodes_assigned < 3 {
                specialized_roles.push(SpecializedRole::StorageNode);
                storage_nodes_assigned += 1;
            }

            // Default to ComputeWorker if no specialized roles
            if specialized_roles.is_empty() {
                specialized_roles.push(SpecializedRole::ComputeWorker);
            }

            let assignment = RoleAssignment {
                peer_id: slave.peer_id,
                assigned_role: DeviceRole::Slave,
                specialized_roles,
                assigned_at: SystemTime::now(),
            };

            debug!(
                "Assigned slave role to {} with specialized roles: {:?}",
                slave.peer_id, assignment.specialized_roles
            );

            assignments.insert(slave.peer_id, assignment);
        }

        // Store assignments
        let mut role_assignments = self.role_assignments.write().await;
        *role_assignments = assignments.clone();

        self.set_state(MeshState::Operational).await;
        info!("Role assignment complete: {} total assignments", assignments.len());

        Ok(assignments)
    }

    /// Re-assign roles after capability changes or device failures
    pub async fn reassign_roles(&self) -> Result<HashMap<PeerId, RoleAssignment>> {
        info!("Re-assigning roles due to topology change");

        // Get current online devices
        let devices = self.registry.get_online_devices().await;

        // Check if master is still available
        let master_id = self.current_master().await;
        let master_online = master_id
            .map(|id| devices.iter().any(|d| d.peer_id == id))
            .unwrap_or(false);

        if !master_online {
            // Master failed - trigger re-election
            warn!("Master is offline, triggering re-election");
            self.set_state(MeshState::MasterFailure).await;
            
            let election_result = self.elect_master(devices.clone()).await?;
            info!("New master elected: {}", election_result.master_peer_id);
        }

        // Perform role assignment with current topology
        self.assign_roles(devices).await
    }

    /// Get role assignment for a specific peer
    pub async fn get_role_assignment(&self, peer_id: &PeerId) -> Option<RoleAssignment> {
        let assignments = self.role_assignments.read().await;
        assignments.get(peer_id).cloned()
    }

    /// Get all role assignments
    pub async fn get_all_assignments(&self) -> HashMap<PeerId, RoleAssignment> {
        self.role_assignments.read().await.clone()
    }

    /// Check if election timeout has expired
    pub async fn is_election_timeout_expired(&self) -> bool {
        let last_election = self.last_election.read().await;
        
        match *last_election {
            Some(time) => {
                let elapsed = SystemTime::now()
                    .duration_since(time)
                    .unwrap_or(Duration::ZERO);
                elapsed > self.election_timeout
            }
            None => true,  // No election yet
        }
    }

    /// Start monitoring mesh topology
    pub async fn start_monitoring(self: Arc<Self>) -> Result<()> {
        info!("Starting mesh topology monitoring");

        // Subscribe to registry events
        let event_rx = self.registry.event_receiver();

        tokio::spawn(async move {
            let mut rx = event_rx.write().await;
            
            loop {
                match rx.recv().await {
                    Some(event) => {
                        if let Err(e) = self.handle_registry_event(event).await {
                            error!("Error handling registry event: {}", e);
                        }
                    }
                    None => {
                        warn!("Registry event channel closed");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Handle registry events
    async fn handle_registry_event(&self, event: RegistryEvent) -> Result<()> {
        match event {
            RegistryEvent::PeerOffline(peer_id) => {
                // Check if master went offline
                let master_id = self.current_master().await;
                if master_id == Some(peer_id) {
                    warn!("Master {} went offline, triggering re-election", peer_id);
                    self.set_state(MeshState::MasterFailure).await;
                    
                    // Trigger reassignment which includes re-election
                    let _ = self.reassign_roles().await;
                }
            }
            RegistryEvent::CapabilitiesChanged(peer_id, new_capabilities) => {
                info!(
                    "Device {} capabilities changed (new score: {:.1})",
                    peer_id, new_capabilities.score
                );
                
                // Check if the change affects role suitability
                // For now, we'll just log it; reassignment can be triggered manually
            }
            RegistryEvent::PeerDiscovered(_) | RegistryEvent::PeerUpdated(_) => {
                // New peer joined or updated - might need role assignment
                debug!("Peer discovery/update event");
            }
            RegistryEvent::PeerSuspected(peer_id) => {
                debug!("Peer {} suspected offline", peer_id);
            }
        }
        
        Ok(())
    }

    /// Get mesh topology statistics
    pub async fn get_topology_stats(&self) -> TopologyStats {
        let state = self.state().await;
        let master = self.current_master().await;
        let assignments = self.role_assignments.read().await;

        let total_devices = assignments.len();
        let master_count = assignments.values().filter(|a| a.assigned_role == DeviceRole::Master).count();
        let slave_count = assignments.values().filter(|a| a.assigned_role == DeviceRole::Slave).count();
        let standalone_count = assignments.values().filter(|a| a.assigned_role == DeviceRole::Standalone).count();

        let llm_hosts = assignments.values()
            .filter(|a| a.specialized_roles.contains(&SpecializedRole::LLMHost))
            .count();
        let stt_tts_hosts = assignments.values()
            .filter(|a| a.specialized_roles.contains(&SpecializedRole::STTTTSHost))
            .count();
        let storage_nodes = assignments.values()
            .filter(|a| a.specialized_roles.contains(&SpecializedRole::StorageNode))
            .count();

        TopologyStats {
            state,
            master_peer_id: master,
            total_devices,
            master_count,
            slave_count,
            standalone_count,
            llm_hosts,
            stt_tts_hosts,
            storage_nodes,
        }
    }
}

/// Mesh topology statistics
#[derive(Debug, Clone)]
pub struct TopologyStats {
    pub state: MeshState,
    pub master_peer_id: Option<PeerId>,
    pub total_devices: usize,
    pub master_count: usize,
    pub slave_count: usize,
    pub standalone_count: usize,
    pub llm_hosts: usize,
    pub stt_tts_hosts: usize,
    pub storage_nodes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn create_test_capabilities(cpu: u8, ram: u64, has_gpu: bool, disk: u64) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities {
            cpu_cores: cpu,
            ram_gb: ram,
            has_gpu,
            gpu_memory_mb: if has_gpu { 8192 } else { 0 },
            disk_gb: disk,
            os: "Linux".to_string(),
            arch: "x86_64".to_string(),
            score: 0.0,
        };
        caps.score = caps.calculate_score();
        caps
    }

    fn create_test_peer(peer_id: PeerId, caps: DeviceCapabilities) -> PeerInfo {
        PeerInfo::new(
            peer_id,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10)),
            8080,
            caps,
            DeviceRole::Slave,  // Will be assigned by coordinator
        )
    }

    #[tokio::test]
    async fn test_coordinator_creation() {
        let peer_id = PeerId::random();
        let caps = create_test_capabilities(4, 8, false, 256);
        let registry = Arc::new(DeviceRegistry::new(peer_id));
        
        let coordinator = MeshCoordinator::new(peer_id, caps, registry);
        
        assert_eq!(coordinator.state().await, MeshState::Initializing);
        assert!(!coordinator.is_master().await);
    }

    #[tokio::test]
    async fn test_master_election_single_candidate() {
        let peer_id = PeerId::random();
        let caps = create_test_capabilities(4, 8, false, 256);
        let registry = Arc::new(DeviceRegistry::new(peer_id));
        let coordinator = MeshCoordinator::new(peer_id, caps.clone(), registry);

        let candidate = create_test_peer(peer_id, caps);
        let result = coordinator.elect_master(vec![candidate]).await.unwrap();

        assert_eq!(result.master_peer_id, peer_id);
        assert!(result.is_unanimous());
        assert_eq!(result.total_candidates, 1);
        assert_eq!(coordinator.current_master().await, Some(peer_id));
    }

    #[tokio::test]
    async fn test_master_election_multiple_candidates() {
        let local_peer = PeerId::random();
        let high_spec_peer = PeerId::random();
        let low_spec_peer = PeerId::random();
        
        let registry = Arc::new(DeviceRegistry::new(local_peer));
        let coordinator = MeshCoordinator::new(
            local_peer,
            create_test_capabilities(4, 8, false, 256),
            registry,
        );

        let candidates = vec![
            create_test_peer(local_peer, create_test_capabilities(4, 8, false, 256)),     // Score ~60
            create_test_peer(high_spec_peer, create_test_capabilities(16, 64, true, 2048)),  // Score ~400+
            create_test_peer(low_spec_peer, create_test_capabilities(2, 4, false, 128)),   // Score ~30
        ];

        let result = coordinator.elect_master(candidates).await.unwrap();

        assert_eq!(result.master_peer_id, high_spec_peer);  // Highest score wins
        assert_eq!(result.total_candidates, 3);
        assert!(result.score_margin() > 0.0);
    }

    #[tokio::test]
    async fn test_master_election_tie_breaking() {
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();
        
        let registry = Arc::new(DeviceRegistry::new(peer1));
        let coordinator = MeshCoordinator::new(
            peer1,
            create_test_capabilities(4, 8, false, 256),
            registry,
        );

        // Two peers with identical capabilities (tied scores)
        let candidates = vec![
            create_test_peer(peer1, create_test_capabilities(4, 8, false, 256)),
            create_test_peer(peer2, create_test_capabilities(4, 8, false, 256)),
        ];

        let result = coordinator.elect_master(candidates).await.unwrap();

        // One of them should be elected (deterministic via peer_id comparison)
        assert!(result.master_peer_id == peer1 || result.master_peer_id == peer2);
        assert_eq!(result.total_candidates, 2);
    }

    #[tokio::test]
    async fn test_role_assignment() {
        let master_peer = PeerId::random();
        let slave1 = PeerId::random();
        let slave2 = PeerId::random();
        
        let registry = Arc::new(DeviceRegistry::new(master_peer));
        let coordinator = MeshCoordinator::new(
            master_peer,
            create_test_capabilities(16, 64, true, 2048),
            registry,
        );

        // Elect master first
        let devices = vec![
            create_test_peer(master_peer, create_test_capabilities(16, 64, true, 2048)),
            create_test_peer(slave1, create_test_capabilities(8, 16, true, 1024)),
            create_test_peer(slave2, create_test_capabilities(4, 8, false, 512)),
        ];

        coordinator.elect_master(devices.clone()).await.unwrap();

        // Assign roles
        let assignments = coordinator.assign_roles(devices).await.unwrap();

        assert_eq!(assignments.len(), 3);
        assert_eq!(assignments[&master_peer].assigned_role, DeviceRole::Master);
        assert_eq!(assignments[&slave1].assigned_role, DeviceRole::Slave);
        assert_eq!(assignments[&slave2].assigned_role, DeviceRole::Slave);
    }

    #[tokio::test]
    async fn test_specialized_role_assignment_llm_host() {
        let master_peer = PeerId::random();
        let gpu_slave = PeerId::random();
        
        let registry = Arc::new(DeviceRegistry::new(master_peer));
        let coordinator = MeshCoordinator::new(
            master_peer,
            create_test_capabilities(16, 64, true, 2048),
            registry,
        );

        let devices = vec![
            create_test_peer(master_peer, create_test_capabilities(16, 64, true, 2048)),
            create_test_peer(gpu_slave, create_test_capabilities(8, 32, true, 1024)),  // Has GPU
        ];

        coordinator.elect_master(devices.clone()).await.unwrap();
        let assignments = coordinator.assign_roles(devices).await.unwrap();

        // GPU slave should be assigned LLMHost role
        let gpu_assignment = &assignments[&gpu_slave];
        assert!(gpu_assignment.specialized_roles.contains(&SpecializedRole::LLMHost));
    }

    #[tokio::test]
    async fn test_specialized_role_assignment_storage_node() {
        let master_peer = PeerId::random();
        let storage_slave = PeerId::random();
        
        let registry = Arc::new(DeviceRegistry::new(master_peer));
        let coordinator = MeshCoordinator::new(
            master_peer,
            create_test_capabilities(16, 64, true, 2048),
            registry,
        );

        let devices = vec![
            create_test_peer(master_peer, create_test_capabilities(16, 64, true, 2048)),
            create_test_peer(storage_slave, create_test_capabilities(4, 8, false, 2000)),  // High disk
        ];

        coordinator.elect_master(devices.clone()).await.unwrap();
        let assignments = coordinator.assign_roles(devices).await.unwrap();

        // High disk slave should be assigned StorageNode role
        let storage_assignment = &assignments[&storage_slave];
        assert!(storage_assignment.specialized_roles.contains(&SpecializedRole::StorageNode));
    }

    #[tokio::test]
    async fn test_get_role_assignment() {
        let master_peer = PeerId::random();
        let registry = Arc::new(DeviceRegistry::new(master_peer));
        let coordinator = MeshCoordinator::new(
            master_peer,
            create_test_capabilities(16, 64, true, 2048),
            registry,
        );

        let devices = vec![
            create_test_peer(master_peer, create_test_capabilities(16, 64, true, 2048)),
        ];

        coordinator.elect_master(devices.clone()).await.unwrap();
        coordinator.assign_roles(devices).await.unwrap();

        let assignment = coordinator.get_role_assignment(&master_peer).await;
        assert!(assignment.is_some());
        assert_eq!(assignment.unwrap().assigned_role, DeviceRole::Master);
    }

    #[tokio::test]
    async fn test_topology_stats() {
        let master_peer = PeerId::random();
        let slave1 = PeerId::random();
        let slave2 = PeerId::random();
        
        let registry = Arc::new(DeviceRegistry::new(master_peer));
        let coordinator = MeshCoordinator::new(
            master_peer,
            create_test_capabilities(16, 64, true, 2048),
            registry,
        );

        let devices = vec![
            create_test_peer(master_peer, create_test_capabilities(16, 64, true, 2048)),
            create_test_peer(slave1, create_test_capabilities(8, 16, true, 1024)),
            create_test_peer(slave2, create_test_capabilities(4, 8, false, 512)),
        ];

        coordinator.elect_master(devices.clone()).await.unwrap();
        coordinator.assign_roles(devices).await.unwrap();

        let stats = coordinator.get_topology_stats().await;
        assert_eq!(stats.total_devices, 3);
        assert_eq!(stats.master_count, 1);
        assert_eq!(stats.slave_count, 2);
        assert_eq!(stats.state, MeshState::Operational);
    }

    #[tokio::test]
    async fn test_is_master() {
        let master_peer = PeerId::random();
        let slave_peer = PeerId::random();
        
        let registry = Arc::new(DeviceRegistry::new(master_peer));
        let master_coordinator = MeshCoordinator::new(
            master_peer,
            create_test_capabilities(16, 64, true, 2048),
            registry.clone(),
        );
        let slave_coordinator = MeshCoordinator::new(
            slave_peer,
            create_test_capabilities(4, 8, false, 256),
            registry,
        );

        let devices = vec![
            create_test_peer(master_peer, create_test_capabilities(16, 64, true, 2048)),
            create_test_peer(slave_peer, create_test_capabilities(4, 8, false, 256)),
        ];

        master_coordinator.elect_master(devices).await.unwrap();

        // Update slave coordinator's master reference
        let mut slave_master = slave_coordinator.current_master.write().await;
        *slave_master = Some(master_peer);
        drop(slave_master);

        assert!(master_coordinator.is_master().await);
        assert!(!slave_coordinator.is_master().await);
    }
}
