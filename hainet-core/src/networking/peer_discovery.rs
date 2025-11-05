//! <!-- # START OF FILE hainet-core/src/networking/peer_discovery.rs -->
//! Peer Discovery System for HAI-Net Mesh Networking
//! 
//! This module implements automatic device discovery using mDNS/DNS-SD.
//! Discovered peers are tracked with their capabilities, roles, and health status.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, SystemTime};

use libp2p::PeerId;
use tokio::sync::RwLock;
use std::sync::Arc;
use anyhow::Result;
use tracing::{info, debug};

/// Device role in the mesh network
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceRole {
    /// Master node (coordinates mesh)
    Master,
    /// Slave node (compute worker)
    Slave,
    /// Standalone node (single device)
    Standalone,
    /// UI-only node (mobile/tablet)
    UIOnly,
}

/// Peer status in the mesh
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerStatus {
    /// Peer is online and responding
    Online,
    /// Peer is offline
    Offline,
    /// Peer is suspected offline (missing heartbeats)
    Suspected,
}

/// Device capabilities (reused from Phase 7)
#[derive(Debug, Clone)]
pub struct DeviceCapabilities {
    pub cpu_cores: u8,
    pub ram_gb: u64,
    pub has_gpu: bool,
    pub gpu_memory_mb: u64,
    pub disk_gb: u64,
    pub os: String,
    pub arch: String,
    pub score: f64,  // Weighted capability score for master election
}

impl DeviceCapabilities {
    /// Calculate capability score (RAM 40%, GPU 30%, CPU 20%, Disk 10%)
    pub fn calculate_score(&self) -> f64 {
        let ram_score = (self.ram_gb as f64) * 10.0 * 0.4;
        let gpu_score = if self.has_gpu { 100.0 } else { 0.0 } * 0.3;
        let cpu_score = (self.cpu_cores as f64) * 5.0 * 0.2;
        let disk_score = (self.disk_gb as f64) * 0.1;
        
        ram_score + gpu_score + cpu_score + disk_score
    }
}

/// Information about a discovered peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: PeerId,
    pub ip_address: IpAddr,
    pub port: u16,
    pub capabilities: DeviceCapabilities,
    pub role: DeviceRole,
    pub last_seen: SystemTime,
    pub status: PeerStatus,
    pub hostname: Option<String>,
}

impl PeerInfo {
    /// Create a new PeerInfo
    pub fn new(
        peer_id: PeerId,
        ip_address: IpAddr,
        port: u16,
        capabilities: DeviceCapabilities,
        role: DeviceRole,
    ) -> Self {
        Self {
            peer_id,
            ip_address,
            port,
            capabilities,
            role,
            last_seen: SystemTime::now(),
            status: PeerStatus::Online,
            hostname: None,
        }
    }

    /// Get socket address
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.ip_address, self.port)
    }

    /// Check if peer is online
    pub fn is_online(&self) -> bool {
        self.status == PeerStatus::Online
    }

    /// Update last seen timestamp
    pub fn update_last_seen(&mut self) {
        self.last_seen = SystemTime::now();
        self.status = PeerStatus::Online;
    }
}

/// Peer discovery manager using mDNS
pub struct PeerDiscovery {
    service_name: String,
    local_peer_id: PeerId,
    local_port: u16,
    discovered_peers: Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
    discovery_interval: Duration,
}

impl PeerDiscovery {
    /// Create a new peer discovery manager
    pub fn new(local_peer_id: PeerId, local_port: u16) -> Self {
        Self {
            service_name: "_hainet._tcp.local".to_string(),
            local_peer_id,
            local_port,
            discovered_peers: Arc::new(RwLock::new(HashMap::new())),
            discovery_interval: Duration::from_secs(5),
        }
    }

    /// Create with custom service name and interval
    pub fn with_config(
        local_peer_id: PeerId,
        local_port: u16,
        service_name: String,
        discovery_interval: Duration,
    ) -> Self {
        Self {
            service_name,
            local_peer_id,
            local_port,
            discovered_peers: Arc::new(RwLock::new(HashMap::new())),
            discovery_interval,
        }
    }

    /// Start peer discovery
    /// 
    /// This method should be called to begin discovering peers on the local network.
    /// In a full implementation, this would:
    /// 1. Initialize mDNS service
    /// 2. Start advertising local service
    /// 3. Listen for peer announcements
    /// 4. Update discovered_peers map
    pub async fn start_discovery(&self) -> Result<()> {
        info!(
            "Starting peer discovery for service: {} on port {}",
            self.service_name, self.local_port
        );
        
        // TODO: Integrate with libp2p mDNS behaviour
        // For now, this is a framework for the discovery system
        
        Ok(())
    }

    /// Stop peer discovery
    pub async fn stop_discovery(&self) -> Result<()> {
        info!("Stopping peer discovery");
        Ok(())
    }

    /// Advertise this node's presence
    pub async fn advertise_self(&self, capabilities: DeviceCapabilities, role: DeviceRole) -> Result<()> {
        info!(
            "Advertising self: peer_id={}, role={:?}, score={}",
            self.local_peer_id, role, capabilities.score
        );
        
        // TODO: Broadcast mDNS announcement with TXT records containing:
        // - peer_id
        // - capabilities (encoded)
        // - role
        // - port
        
        Ok(())
    }

    /// Register a discovered peer
    pub async fn register_peer(&self, peer_info: PeerInfo) -> Result<()> {
        let mut peers = self.discovered_peers.write().await;
        
        if peer_info.peer_id == self.local_peer_id {
            // Don't register self
            return Ok(());
        }
        
        if peers.contains_key(&peer_info.peer_id) {
            debug!(
                "Updating existing peer: {} ({})",
                peer_info.peer_id, peer_info.ip_address
            );
        } else {
            info!(
                "Discovered new peer: {} at {} (role: {:?}, score: {})",
                peer_info.peer_id,
                peer_info.ip_address,
                peer_info.role,
                peer_info.capabilities.score
            );
        }
        
        peers.insert(peer_info.peer_id, peer_info);
        Ok(())
    }

    /// Get all discovered peers
    pub async fn get_discovered_peers(&self) -> Vec<PeerInfo> {
        let peers = self.discovered_peers.read().await;
        peers.values().cloned().collect()
    }

    /// Get online peers only
    pub async fn get_online_peers(&self) -> Vec<PeerInfo> {
        let peers = self.discovered_peers.read().await;
        peers
            .values()
            .filter(|p| p.is_online())
            .cloned()
            .collect()
    }

    /// Get a specific peer by ID
    pub async fn get_peer(&self, peer_id: &PeerId) -> Option<PeerInfo> {
        let peers = self.discovered_peers.read().await;
        peers.get(peer_id).cloned()
    }

    /// Remove a peer
    pub async fn remove_peer(&self, peer_id: &PeerId) -> Result<()> {
        let mut peers = self.discovered_peers.write().await;
        if let Some(peer) = peers.remove(peer_id) {
            info!("Removed peer: {} ({})", peer_id, peer.ip_address);
        }
        Ok(())
    }

    /// Update peer status
    pub async fn update_peer_status(&self, peer_id: &PeerId, status: PeerStatus) -> Result<()> {
        let mut peers = self.discovered_peers.write().await;
        if let Some(peer) = peers.get_mut(peer_id) {
            peer.status = status;
            if status == PeerStatus::Online {
                peer.last_seen = SystemTime::now();
            }
            debug!("Updated peer {} status to {:?}", peer_id, status);
        }
        Ok(())
    }

    /// Get discovery interval
    pub fn discovery_interval(&self) -> Duration {
        self.discovery_interval
    }

    /// Get service name
    pub fn service_name(&self) -> &str {
        &self.service_name
    }

    /// Get local peer ID
    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn test_peer_discovery_creation() {
        let peer_id = PeerId::random();
        let discovery = PeerDiscovery::new(peer_id, 8080);
        
        assert_eq!(discovery.local_peer_id(), peer_id);
        assert_eq!(discovery.local_port, 8080);
        assert_eq!(discovery.service_name(), "_hainet._tcp.local");
    }

    #[tokio::test]
    async fn test_peer_registration() {
        let local_peer = PeerId::random();
        let remote_peer = PeerId::random();
        let discovery = PeerDiscovery::new(local_peer, 8080);
        
        let capabilities = DeviceCapabilities {
            cpu_cores: 4,
            ram_gb: 8,
            has_gpu: false,
            gpu_memory_mb: 0,
            disk_gb: 256,
            os: "Linux".to_string(),
            arch: "x86_64".to_string(),
            score: 0.0,
        };
        
        let peer_info = PeerInfo::new(
            remote_peer,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10)),
            8080,
            capabilities,
            DeviceRole::Slave,
        );
        
        discovery.register_peer(peer_info).await.unwrap();
        
        let peers = discovery.get_discovered_peers().await;
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].peer_id, remote_peer);
    }

    #[tokio::test]
    async fn test_capability_score_calculation() {
        let high_spec = DeviceCapabilities {
            cpu_cores: 16,
            ram_gb: 64,
            has_gpu: true,
            gpu_memory_mb: 12288,
            disk_gb: 2048,
            os: "Linux".to_string(),
            arch: "x86_64".to_string(),
            score: 0.0,
        };
        
        let low_spec = DeviceCapabilities {
            cpu_cores: 2,
            ram_gb: 4,
            has_gpu: false,
            gpu_memory_mb: 0,
            disk_gb: 128,
            os: "Linux".to_string(),
            arch: "x86_64".to_string(),
            score: 0.0,
        };
        
        let high_score = high_spec.calculate_score();
        let low_score = low_spec.calculate_score();
        
        assert!(high_score > low_score);
        assert!(high_score > 280.0); // RAM(256) + GPU(30) + CPU(16) + Disk(~200)
        assert!(low_score < 50.0);   // Much lower score
    }

    #[tokio::test]
    async fn test_peer_status_update() {
        let discovery = PeerDiscovery::new(PeerId::random(), 8080);
        let peer_id = PeerId::random();
        
        let mut peer_info = PeerInfo::new(
            peer_id,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10)),
            8080,
            DeviceCapabilities {
                cpu_cores: 4,
                ram_gb: 8,
                has_gpu: false,
                gpu_memory_mb: 0,
                disk_gb: 256,
                os: "Linux".to_string(),
                arch: "x86_64".to_string(),
                score: 0.0,
            },
            DeviceRole::Slave,
        );
        
        peer_info.status = PeerStatus::Suspected;
        discovery.register_peer(peer_info).await.unwrap();
        
        // Update status to Online
        discovery.update_peer_status(&peer_id, PeerStatus::Online).await.unwrap();
        
        let updated_peer = discovery.get_peer(&peer_id).await.unwrap();
        assert_eq!(updated_peer.status, PeerStatus::Online);
    }

    #[tokio::test]
    async fn test_get_online_peers_only() {
        let discovery = PeerDiscovery::new(PeerId::random(), 8080);
        
        // Add online peer
        let online_peer = PeerInfo::new(
            PeerId::random(),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10)),
            8080,
            DeviceCapabilities {
                cpu_cores: 4,
                ram_gb: 8,
                has_gpu: false,
                gpu_memory_mb: 0,
                disk_gb: 256,
                os: "Linux".to_string(),
                arch: "x86_64".to_string(),
                score: 0.0,
            },
            DeviceRole::Slave,
        );
        
        // Add offline peer
        let mut offline_peer = PeerInfo::new(
            PeerId::random(),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 11)),
            8080,
            DeviceCapabilities {
                cpu_cores: 2,
                ram_gb: 4,
                has_gpu: false,
                gpu_memory_mb: 0,
                disk_gb: 128,
                os: "Linux".to_string(),
                arch: "x86_64".to_string(),
                score: 0.0,
            },
            DeviceRole::Slave,
        );
        offline_peer.status = PeerStatus::Offline;
        
        discovery.register_peer(online_peer).await.unwrap();
        discovery.register_peer(offline_peer).await.unwrap();
        
        let online_peers = discovery.get_online_peers().await;
        assert_eq!(online_peers.len(), 1);
        assert_eq!(online_peers[0].status, PeerStatus::Online);
    }
}
