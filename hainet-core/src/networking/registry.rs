//! <!-- # START OF FILE hainet-core/src/networking/registry.rs -->
//! Device Registry for HAI-Net Mesh Networking
//! 
//! This module manages the registry of all known devices in the mesh network,
//! tracking their capabilities, health status, and providing event notifications.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, SystemTime};

use libp2p::PeerId;
use tokio::sync::{mpsc, RwLock};
use std::sync::Arc;
use anyhow::Result;
use tracing::{info, warn, debug};

use super::peer_discovery::{DeviceCapabilities, DeviceRole, PeerInfo, PeerStatus};

/// Maximum number of capability history entries to keep per device
const MAX_CAPABILITY_HISTORY: usize = 100;

/// Registry events for mesh coordination
#[derive(Debug, Clone)]
pub enum RegistryEvent {
    /// A new peer was discovered
    PeerDiscovered(PeerInfo),
    /// Peer information was updated
    PeerUpdated(PeerInfo),
    /// Peer went offline
    PeerOffline(PeerId),
    /// Peer is suspected offline (missing heartbeats)
    PeerSuspected(PeerId),
    /// Peer capabilities changed significantly
    CapabilitiesChanged(PeerId, DeviceCapabilities),
}

/// Extended device entry with health tracking
#[derive(Debug, Clone)]
pub struct DeviceEntry {
    /// Peer information
    pub peer_info: PeerInfo,
    /// Health score (0.0-1.0)
    pub health_score: f64,
    /// Consecutive heartbeat failures
    pub consecutive_failures: u32,
    /// Last heartbeat received
    pub last_heartbeat: SystemTime,
    /// Historical capability snapshots
    pub capabilities_history: VecDeque<DeviceCapabilities>,
}

impl DeviceEntry {
    /// Create a new device entry
    pub fn new(peer_info: PeerInfo) -> Self {
        Self {
            peer_info,
            health_score: 1.0,
            consecutive_failures: 0,
            last_heartbeat: SystemTime::now(),
            capabilities_history: VecDeque::with_capacity(MAX_CAPABILITY_HISTORY),
        }
    }

    /// Update heartbeat and reset failure count
    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = SystemTime::now();
        self.consecutive_failures = 0;
        self.health_score = (self.health_score + 0.1).min(1.0); // Gradual recovery
        self.peer_info.update_last_seen();
    }

    /// Record a heartbeat failure
    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        self.health_score = (self.health_score - 0.2).max(0.0);
    }

    /// Add capability snapshot to history
    pub fn add_capability_snapshot(&mut self, capabilities: DeviceCapabilities) {
        if self.capabilities_history.len() >= MAX_CAPABILITY_HISTORY {
            self.capabilities_history.pop_front();
        }
        self.capabilities_history.push_back(capabilities);
    }

    /// Check if device is healthy
    pub fn is_healthy(&self) -> bool {
        self.health_score > 0.5 && self.consecutive_failures < 3
    }

    /// Get time since last heartbeat
    pub fn time_since_heartbeat(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.last_heartbeat)
            .unwrap_or(Duration::ZERO)
    }
}

/// Device registry managing all known mesh nodes
pub struct DeviceRegistry {
    local_peer_id: PeerId,
    devices: Arc<RwLock<HashMap<PeerId, DeviceEntry>>>,
    heartbeat_interval: Duration,
    heartbeat_timeout: Duration,
    event_tx: mpsc::Sender<RegistryEvent>,
    event_rx: Arc<RwLock<mpsc::Receiver<RegistryEvent>>>,
}

impl DeviceRegistry {
    /// Create a new device registry
    pub fn new(local_peer_id: PeerId) -> Self {
        let (event_tx, event_rx) = mpsc::channel(1000);
        
        Self {
            local_peer_id,
            devices: Arc::new(RwLock::new(HashMap::new())),
            heartbeat_interval: Duration::from_secs(5),
            heartbeat_timeout: Duration::from_secs(15),
            event_tx,
            event_rx: Arc::new(RwLock::new(event_rx)),
        }
    }

    /// Create with custom heartbeat configuration
    pub fn with_config(
        local_peer_id: PeerId,
        heartbeat_interval: Duration,
        heartbeat_timeout: Duration,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::channel(1000);
        
        Self {
            local_peer_id,
            devices: Arc::new(RwLock::new(HashMap::new())),
            heartbeat_interval,
            heartbeat_timeout,
            event_tx,
            event_rx: Arc::new(RwLock::new(event_rx)),
        }
    }

    /// Register a new device
    pub async fn register_device(&self, peer_info: PeerInfo) -> Result<()> {
        let mut devices = self.devices.write().await;
        
        if peer_info.peer_id == self.local_peer_id {
            // Don't register self
            return Ok(());
        }
        
        let is_new = !devices.contains_key(&peer_info.peer_id);
        let mut entry = DeviceEntry::new(peer_info.clone());
        entry.add_capability_snapshot(peer_info.capabilities.clone());
        
        devices.insert(peer_info.peer_id, entry);
        
        // Emit event
        if is_new {
            info!("Registered new device: {} ({})", peer_info.peer_id, peer_info.ip_address);
            let _ = self.event_tx.send(RegistryEvent::PeerDiscovered(peer_info)).await;
        } else {
            debug!("Updated device: {}", peer_info.peer_id);
            let _ = self.event_tx.send(RegistryEvent::PeerUpdated(peer_info)).await;
        }
        
        Ok(())
    }

    /// Update heartbeat for a device
    pub async fn update_heartbeat(&self, peer_id: &PeerId) -> Result<()> {
        let mut devices = self.devices.write().await;
        
        if let Some(entry) = devices.get_mut(peer_id) {
            entry.update_heartbeat();
            debug!("Updated heartbeat for {}", peer_id);
        }
        
        Ok(())
    }

    /// Update device capabilities
    pub async fn update_capabilities(
        &self,
        peer_id: &PeerId,
        capabilities: DeviceCapabilities,
    ) -> Result<()> {
        let mut devices = self.devices.write().await;
        
        if let Some(entry) = devices.get_mut(peer_id) {
            let old_score = entry.peer_info.capabilities.score;
            let new_score = capabilities.score;
            
            entry.peer_info.capabilities = capabilities.clone();
            entry.add_capability_snapshot(capabilities.clone());
            
            // Emit event if significant change
            if (old_score - new_score).abs() > 10.0 {
                info!(
                    "Device {} capabilities changed: score {} -> {}",
                    peer_id, old_score, new_score
                );
                let _ = self.event_tx
                    .send(RegistryEvent::CapabilitiesChanged(*peer_id, capabilities))
                    .await;
            }
        }
        
        Ok(())
    }

    /// Mark a device as offline
    pub async fn mark_offline(&self, peer_id: &PeerId) -> Result<()> {
        let mut devices = self.devices.write().await;
        
        if let Some(entry) = devices.get_mut(peer_id) {
            entry.peer_info.status = PeerStatus::Offline;
            entry.health_score = 0.0;
            
            warn!("Device {} marked offline", peer_id);
            let _ = self.event_tx.send(RegistryEvent::PeerOffline(*peer_id)).await;
        }
        
        Ok(())
    }

    /// Get all online devices
    pub async fn get_online_devices(&self) -> Vec<PeerInfo> {
        let devices = self.devices.read().await;
        devices
            .values()
            .filter(|e| e.peer_info.status == PeerStatus::Online)
            .map(|e| e.peer_info.clone())
            .collect()
    }

    /// Get all devices (including offline)
    pub async fn get_all_devices(&self) -> Vec<DeviceEntry> {
        let devices = self.devices.read().await;
        devices.values().cloned().collect()
    }

    /// Get a specific device
    pub async fn get_device(&self, peer_id: &PeerId) -> Option<DeviceEntry> {
        let devices = self.devices.read().await;
        devices.get(peer_id).cloned()
    }

    /// Remove a device from registry
    pub async fn remove_device(&self, peer_id: &PeerId) -> Result<()> {
        let mut devices = self.devices.write().await;
        
        if devices.remove(peer_id).is_some() {
            info!("Removed device {} from registry", peer_id);
        }
        
        Ok(())
    }

    /// Start heartbeat monitor (background task)
    pub async fn start_heartbeat_monitor(self: Arc<Self>) -> Result<()> {
        info!(
            "Starting heartbeat monitor (interval: {:?}, timeout: {:?})",
            self.heartbeat_interval, self.heartbeat_timeout
        );
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.heartbeat_interval);
            
            loop {
                interval.tick().await;
                
                if let Err(e) = self.check_heartbeats().await {
                    warn!("Error checking heartbeats: {}", e);
                }
            }
        });
        
        Ok(())
    }

    /// Check all devices for heartbeat timeouts
    async fn check_heartbeats(&self) -> Result<()> {
        let mut devices = self.devices.write().await;
        let now = SystemTime::now();
        
        for (peer_id, entry) in devices.iter_mut() {
            let time_since = now
                .duration_since(entry.last_heartbeat)
                .unwrap_or(Duration::ZERO);
            
            if time_since > self.heartbeat_timeout {
                // Heartbeat timeout
                if entry.peer_info.status == PeerStatus::Online {
                    entry.peer_info.status = PeerStatus::Suspected;
                    entry.record_failure();
                    
                    warn!(
                        "Device {} suspected offline (no heartbeat for {:?})",
                        peer_id, time_since
                    );
                    let _ = self.event_tx.send(RegistryEvent::PeerSuspected(*peer_id)).await;
                }
                
                // Mark offline after multiple failures
                if entry.consecutive_failures >= 3 {
                    entry.peer_info.status = PeerStatus::Offline;
                    entry.health_score = 0.0;
                    
                    warn!("Device {} marked offline after {} failures", peer_id, entry.consecutive_failures);
                    let _ = self.event_tx.send(RegistryEvent::PeerOffline(*peer_id)).await;
                }
            }
        }
        
        Ok(())
    }

    /// Calculate health score for a device
    pub async fn calculate_health_score(&self, peer_id: &PeerId) -> f64 {
        let devices = self.devices.read().await;
        
        devices
            .get(peer_id)
            .map(|e| e.health_score)
            .unwrap_or(0.0)
    }

    /// Get event receiver
    pub fn event_receiver(&self) -> Arc<RwLock<mpsc::Receiver<RegistryEvent>>> {
        self.event_rx.clone()
    }

    /// Get heartbeat configuration
    pub fn heartbeat_interval(&self) -> Duration {
        self.heartbeat_interval
    }

    pub fn heartbeat_timeout(&self) -> Duration {
        self.heartbeat_timeout
    }

    /// Get registry statistics
    pub async fn get_stats(&self) -> RegistryStats {
        let devices = self.devices.read().await;
        
        let total = devices.len();
        let online = devices.values().filter(|e| e.peer_info.status == PeerStatus::Online).count();
        let suspected = devices.values().filter(|e| e.peer_info.status == PeerStatus::Suspected).count();
        let offline = devices.values().filter(|e| e.peer_info.status == PeerStatus::Offline).count();
        let avg_health = if total > 0 {
            devices.values().map(|e| e.health_score).sum::<f64>() / total as f64
        } else {
            0.0
        };
        
        RegistryStats {
            total_devices: total,
            online_devices: online,
            suspected_devices: suspected,
            offline_devices: offline,
            average_health: avg_health,
        }
    }
}

/// Registry statistics
#[derive(Debug, Clone)]
pub struct RegistryStats {
    pub total_devices: usize,
    pub online_devices: usize,
    pub suspected_devices: usize,
    pub offline_devices: usize,
    pub average_health: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn create_test_capabilities() -> DeviceCapabilities {
        DeviceCapabilities {
            cpu_cores: 4,
            ram_gb: 8,
            has_gpu: false,
            gpu_memory_mb: 0,
            disk_gb: 256,
            os: "Linux".to_string(),
            arch: "x86_64".to_string(),
            score: 60.0,
        }
    }

    fn create_test_peer_info(peer_id: PeerId) -> PeerInfo {
        PeerInfo::new(
            peer_id,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10)),
            8080,
            create_test_capabilities(),
            DeviceRole::Slave,
        )
    }

    #[tokio::test]
    async fn test_registry_creation() {
        let peer_id = PeerId::random();
        let registry = DeviceRegistry::new(peer_id);
        
        assert_eq!(registry.heartbeat_interval(), Duration::from_secs(5));
        assert_eq!(registry.heartbeat_timeout(), Duration::from_secs(15));
    }

    #[tokio::test]
    async fn test_device_registration() {
        let local_peer = PeerId::random();
        let remote_peer = PeerId::random();
        let registry = DeviceRegistry::new(local_peer);
        
        let peer_info = create_test_peer_info(remote_peer);
        registry.register_device(peer_info.clone()).await.unwrap();
        
        let device = registry.get_device(&remote_peer).await.unwrap();
        assert_eq!(device.peer_info.peer_id, remote_peer);
        assert_eq!(device.health_score, 1.0);
        assert_eq!(device.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn test_heartbeat_update() {
        let registry = DeviceRegistry::new(PeerId::random());
        let peer_id = PeerId::random();
        
        let peer_info = create_test_peer_info(peer_id);
        registry.register_device(peer_info).await.unwrap();
        
        // Simulate heartbeat
        tokio::time::sleep(Duration::from_millis(100)).await;
        registry.update_heartbeat(&peer_id).await.unwrap();
        
        let device = registry.get_device(&peer_id).await.unwrap();
        assert_eq!(device.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn test_mark_offline() {
        let registry = DeviceRegistry::new(PeerId::random());
        let peer_id = PeerId::random();
        
        let peer_info = create_test_peer_info(peer_id);
        registry.register_device(peer_info).await.unwrap();
        
        registry.mark_offline(&peer_id).await.unwrap();
        
        let device = registry.get_device(&peer_id).await.unwrap();
        assert_eq!(device.peer_info.status, PeerStatus::Offline);
        assert_eq!(device.health_score, 0.0);
    }

    #[tokio::test]
    async fn test_capability_history() {
        let registry = DeviceRegistry::new(PeerId::random());
        let peer_id = PeerId::random();
        
        let peer_info = create_test_peer_info(peer_id);
        registry.register_device(peer_info).await.unwrap();
        
        // Update capabilities
        let mut new_caps = create_test_capabilities();
        new_caps.ram_gb = 16;
        new_caps.score = new_caps.calculate_score();
        
        registry.update_capabilities(&peer_id, new_caps).await.unwrap();
        
        let device = registry.get_device(&peer_id).await.unwrap();
        assert_eq!(device.capabilities_history.len(), 2); // Initial + update
    }

    #[tokio::test]
    async fn test_get_online_devices() {
        let registry = DeviceRegistry::new(PeerId::random());
        
        // Add online device
        let online_peer = PeerId::random();
        registry.register_device(create_test_peer_info(online_peer)).await.unwrap();
        
        // Add offline device
        let offline_peer = PeerId::random();
        registry.register_device(create_test_peer_info(offline_peer)).await.unwrap();
        registry.mark_offline(&offline_peer).await.unwrap();
        
        let online_devices = registry.get_online_devices().await;
        assert_eq!(online_devices.len(), 1);
        assert_eq!(online_devices[0].peer_id, online_peer);
    }

    #[tokio::test]
    async fn test_registry_stats() {
        let registry = DeviceRegistry::new(PeerId::random());
        
        // Add devices
        for i in 0..5 {
            let peer_id = PeerId::random();
            registry.register_device(create_test_peer_info(peer_id)).await.unwrap();
            
            // Mark some as offline
            if i % 2 == 0 {
                registry.mark_offline(&peer_id).await.unwrap();
            }
        }
        
        let stats = registry.get_stats().await;
        assert_eq!(stats.total_devices, 5);
        assert_eq!(stats.online_devices, 2);
        assert_eq!(stats.offline_devices, 3);
    }
}
