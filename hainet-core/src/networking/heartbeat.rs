//! <!-- # START OF FILE hainet-core/src/networking/heartbeat.rs -->
//! Heartbeat System for HAI-Net Mesh Networking
//! 
//! This module implements the heartbeat protocol for peer health monitoring.
//! It sends periodic heartbeats to peers and tracks their responses.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use libp2p::PeerId;
use tokio::sync::RwLock;
use std::sync::Arc;
use anyhow::Result;
use tracing::{info, warn, debug};

use super::peer_discovery::PeerStatus;
use super::registry::DeviceRegistry;

/// Heartbeat state for a single peer
#[derive(Debug, Clone)]
pub struct HeartbeatState {
    /// Last time we sent a heartbeat
    pub last_sent: SystemTime,
    /// Last time we received a heartbeat response
    pub last_received: SystemTime,
    /// Round-trip time for heartbeat
    pub rtt: Duration,
    /// Number of consecutive missed heartbeats
    pub missed_count: u32,
}

impl HeartbeatState {
    /// Create a new heartbeat state
    pub fn new() -> Self {
        let now = SystemTime::now();
        Self {
            last_sent: now,
            last_received: now,
            rtt: Duration::ZERO,
            missed_count: 0,
        }
    }

    /// Update with received heartbeat
    pub fn update_received(&mut self) {
        let now = SystemTime::now();
        self.rtt = now
            .duration_since(self.last_sent)
            .unwrap_or(Duration::ZERO);
        self.last_received = now;
        self.missed_count = 0;
    }

    /// Record a missed heartbeat
    pub fn record_missed(&mut self) {
        self.missed_count += 1;
    }

    /// Check if peer is healthy based on heartbeat
    pub fn is_healthy(&self) -> bool {
        self.missed_count < 3
    }

    /// Get time since last received heartbeat
    pub fn time_since_received(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.last_received)
            .unwrap_or(Duration::ZERO)
    }
}

impl Default for HeartbeatState {
    fn default() -> Self {
        Self::new()
    }
}

/// Heartbeat manager for peer health monitoring
pub struct HeartbeatManager {
    registry: Arc<DeviceRegistry>,
    interval: Duration,
    peers: Arc<RwLock<HashMap<PeerId, HeartbeatState>>>,
    running: Arc<RwLock<bool>>,
}

impl HeartbeatManager {
    /// Create a new heartbeat manager
    pub fn new(registry: Arc<DeviceRegistry>) -> Self {
        let interval = registry.heartbeat_interval();
        
        Self {
            registry,
            interval,
            peers: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Create with custom interval
    pub fn with_interval(registry: Arc<DeviceRegistry>, interval: Duration) -> Self {
        Self {
            registry,
            interval,
            peers: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the heartbeat system
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            warn!("Heartbeat manager already running");
            return Ok(());
        }
        
        *running = true;
        info!("Starting heartbeat manager (interval: {:?})", self.interval);
        
        // Spawn heartbeat sender task
        let peers = self.peers.clone();
        let registry = self.registry.clone();
        let interval = self.interval;
        let running_flag = self.running.clone();
        
        tokio::spawn(async move {
            let mut tick_interval = tokio::time::interval(interval);
            
            loop {
                tick_interval.tick().await;
                
                let is_running = *running_flag.read().await;
                if !is_running {
                    break;
                }
                
                if let Err(e) = Self::send_heartbeats_to_peers(&registry, &peers).await {
                    warn!("Error sending heartbeats: {}", e);
                }
            }
            
            info!("Heartbeat sender task stopped");
        });
        
        Ok(())
    }

    /// Stop the heartbeat system
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = false;
        info!("Stopping heartbeat manager");
        Ok(())
    }

    /// Send heartbeats to all online peers
    async fn send_heartbeats_to_peers(
        registry: &Arc<DeviceRegistry>,
        peers: &Arc<RwLock<HashMap<PeerId, HeartbeatState>>>,
    ) -> Result<()> {
        let online_devices = registry.get_online_devices().await;
        let mut peers_map = peers.write().await;
        
        for peer_info in online_devices {
            let peer_id = peer_info.peer_id;
            
            // Get or create heartbeat state
            let state = peers_map.entry(peer_id).or_insert_with(HeartbeatState::new);
            
            // Update last sent time
            state.last_sent = SystemTime::now();
            
            // TODO: Send actual heartbeat message via libp2p
            // For now, this is a framework for the heartbeat system
            debug!("Sending heartbeat to {}", peer_id);
        }
        
        Ok(())
    }

    /// Send a heartbeat to a specific peer
    pub async fn send_heartbeat(&self, peer_id: &PeerId) -> Result<()> {
        let mut peers = self.peers.write().await;
        
        let state = peers.entry(*peer_id).or_insert_with(HeartbeatState::new);
        state.last_sent = SystemTime::now();
        
        debug!("Sent heartbeat to {}", peer_id);
        
        // TODO: Send actual heartbeat message via libp2p
        
        Ok(())
    }

    /// Handle received heartbeat from a peer
    pub async fn handle_heartbeat(&self, from: &PeerId) -> Result<()> {
        let mut peers = self.peers.write().await;
        
        let state = peers.entry(*from).or_insert_with(HeartbeatState::new);
        state.update_received();
        
        debug!(
            "Received heartbeat from {} (RTT: {:?})",
            from, state.rtt
        );
        
        // Update registry
        self.registry.update_heartbeat(from).await?;
        
        Ok(())
    }

    /// Check health status of a peer
    pub async fn check_peer_health(&self, peer_id: &PeerId) -> PeerStatus {
        let peers = self.peers.read().await;
        
        if let Some(state) = peers.get(peer_id) {
            if state.is_healthy() {
                PeerStatus::Online
            } else if state.missed_count < 5 {
                PeerStatus::Suspected
            } else {
                PeerStatus::Offline
            }
        } else {
            PeerStatus::Offline
        }
    }

    /// Get heartbeat state for a peer
    pub async fn get_peer_state(&self, peer_id: &PeerId) -> Option<HeartbeatState> {
        let peers = self.peers.read().await;
        peers.get(peer_id).cloned()
    }

    /// Get all peer heartbeat states
    pub async fn get_all_states(&self) -> HashMap<PeerId, HeartbeatState> {
        let peers = self.peers.read().await;
        peers.clone()
    }

    /// Remove a peer from heartbeat tracking
    pub async fn remove_peer(&self, peer_id: &PeerId) -> Result<()> {
        let mut peers = self.peers.write().await;
        
        if peers.remove(peer_id).is_some() {
            debug!("Removed peer {} from heartbeat tracking", peer_id);
        }
        
        Ok(())
    }

    /// Get heartbeat statistics
    pub async fn get_stats(&self) -> HeartbeatStats {
        let peers = self.peers.read().await;
        
        let total_peers = peers.len();
        let healthy_peers = peers.values().filter(|s| s.is_healthy()).count();
        let avg_rtt = if total_peers > 0 {
            peers.values().map(|s| s.rtt.as_millis() as f64).sum::<f64>() / total_peers as f64
        } else {
            0.0
        };
        
        HeartbeatStats {
            total_peers,
            healthy_peers,
            average_rtt_ms: avg_rtt,
        }
    }

    /// Check if heartbeat manager is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Get heartbeat interval
    pub fn interval(&self) -> Duration {
        self.interval
    }
}

/// Heartbeat statistics
#[derive(Debug, Clone)]
pub struct HeartbeatStats {
    pub total_peers: usize,
    pub healthy_peers: usize,
    pub average_rtt_ms: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heartbeat_state_creation() {
        let state = HeartbeatState::new();
        
        assert_eq!(state.missed_count, 0);
        assert_eq!(state.rtt, Duration::ZERO);
        assert!(state.is_healthy());
    }

    #[test]
    fn test_heartbeat_state_update() {
        let mut state = HeartbeatState::new();
        
        // Simulate sending
        std::thread::sleep(Duration::from_millis(10));
        
        // Simulate receiving
        state.update_received();
        
        assert_eq!(state.missed_count, 0);
        assert!(state.rtt > Duration::ZERO);
        assert!(state.is_healthy());
    }

    #[test]
    fn test_heartbeat_state_missed() {
        let mut state = HeartbeatState::new();
        
        state.record_missed();
        assert_eq!(state.missed_count, 1);
        assert!(state.is_healthy());
        
        state.record_missed();
        state.record_missed();
        assert_eq!(state.missed_count, 3);
        assert!(!state.is_healthy());
    }

    #[tokio::test]
    async fn test_heartbeat_manager_creation() {
        let peer_id = PeerId::random();
        let registry = Arc::new(DeviceRegistry::new(peer_id));
        let manager = HeartbeatManager::new(registry);
        
        assert_eq!(manager.interval(), Duration::from_secs(5));
        assert!(!manager.is_running().await);
    }

    #[tokio::test]
    async fn test_heartbeat_manager_start_stop() {
        let peer_id = PeerId::random();
        let registry = Arc::new(DeviceRegistry::new(peer_id));
        let manager = HeartbeatManager::new(registry);
        
        manager.start().await.unwrap();
        assert!(manager.is_running().await);
        
        manager.stop().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!manager.is_running().await);
    }

    #[tokio::test]
    async fn test_send_heartbeat() {
        let peer_id = PeerId::random();
        let registry = Arc::new(DeviceRegistry::new(peer_id));
        let manager = HeartbeatManager::new(registry);
        
        let remote_peer = PeerId::random();
        manager.send_heartbeat(&remote_peer).await.unwrap();
        
        let state = manager.get_peer_state(&remote_peer).await.unwrap();
        assert_eq!(state.missed_count, 0);
    }

    #[tokio::test]
    async fn test_handle_heartbeat() {
        let peer_id = PeerId::random();
        let registry = Arc::new(DeviceRegistry::new(peer_id));
        let manager = HeartbeatManager::new(registry);
        
        let remote_peer = PeerId::random();
        
        // Send heartbeat first
        manager.send_heartbeat(&remote_peer).await.unwrap();
        
        // Simulate small delay
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Handle received heartbeat
        manager.handle_heartbeat(&remote_peer).await.unwrap();
        
        let state = manager.get_peer_state(&remote_peer).await.unwrap();
        assert_eq!(state.missed_count, 0);
        assert!(state.rtt > Duration::ZERO);
    }

    #[tokio::test]
    async fn test_check_peer_health() {
        let peer_id = PeerId::random();
        let registry = Arc::new(DeviceRegistry::new(peer_id));
        let manager = HeartbeatManager::new(registry);
        
        let remote_peer = PeerId::random();
        
        // Initially, peer should be offline (not tracked)
        let status = manager.check_peer_health(&remote_peer).await;
        assert_eq!(status, PeerStatus::Offline);
        
        // Send heartbeat
        manager.send_heartbeat(&remote_peer).await.unwrap();
        manager.handle_heartbeat(&remote_peer).await.unwrap();
        
        // Should be online now
        let status = manager.check_peer_health(&remote_peer).await;
        assert_eq!(status, PeerStatus::Online);
    }

    #[tokio::test]
    async fn test_heartbeat_stats() {
        let peer_id = PeerId::random();
        let registry = Arc::new(DeviceRegistry::new(peer_id));
        let manager = HeartbeatManager::new(registry);
        
        // Add some peers
        for _ in 0..3 {
            let remote_peer = PeerId::random();
            manager.send_heartbeat(&remote_peer).await.unwrap();
            tokio::time::sleep(Duration::from_millis(5)).await;
            manager.handle_heartbeat(&remote_peer).await.unwrap();
        }
        
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_peers, 3);
        assert_eq!(stats.healthy_peers, 3);
        assert!(stats.average_rtt_ms > 0.0);
    }

    #[tokio::test]
    async fn test_remove_peer() {
        let peer_id = PeerId::random();
        let registry = Arc::new(DeviceRegistry::new(peer_id));
        let manager = HeartbeatManager::new(registry);
        
        let remote_peer = PeerId::random();
        manager.send_heartbeat(&remote_peer).await.unwrap();
        
        assert!(manager.get_peer_state(&remote_peer).await.is_some());
        
        manager.remove_peer(&remote_peer).await.unwrap();
        
        assert!(manager.get_peer_state(&remote_peer).await.is_none());
    }
}
