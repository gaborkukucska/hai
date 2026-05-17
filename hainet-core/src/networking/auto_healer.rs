//! <!-- # START OF FILE hainet-core/src/networking/auto_healer.rs -->
//! Auto Healer - Network Resilience & Recovery System
//!
//! Monitors peer and service health, triggering recovery actions when failures are detected.
//! Works in conjunction with HeartbeatManager and ServiceManager.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use anyhow::Result;
use libp2p::PeerId;

use super::registry::DeviceRegistry;
use super::service_manager::ServiceManager;
use super::heartbeat::HeartbeatManager;
use super::peer_discovery::PeerStatus;

/// Configuration for AutoHealer
#[derive(Debug, Clone)]
pub struct AutoHealerConfig {
    /// How often to check for failures
    pub check_interval: Duration,
    /// Whether auto-healing is enabled
    pub enabled: bool,
}

impl Default for AutoHealerConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(5),
            enabled: true,
        }
    }
}

/// AutoHealer component
pub struct AutoHealer {
    registry: Arc<DeviceRegistry>,
    service_manager: Arc<ServiceManager>,
    heartbeat_manager: Arc<HeartbeatManager>,
    config: AutoHealerConfig,
    running: Arc<RwLock<bool>>,
}

impl AutoHealer {
    /// Create a new AutoHealer
    pub fn new(
        registry: Arc<DeviceRegistry>,
        service_manager: Arc<ServiceManager>,
        heartbeat_manager: Arc<HeartbeatManager>,
        config: AutoHealerConfig,
    ) -> Self {
        Self {
            registry,
            service_manager,
            heartbeat_manager,
            config,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the auto-healing monitor loop
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            warn!("AutoHealer already running");
            return Ok(());
        }

        *running = true;
        info!("Starting AutoHealer (interval: {:?})", self.config.check_interval);

        let registry = self.registry.clone();
        let service_manager = self.service_manager.clone();
        let heartbeat_manager = self.heartbeat_manager.clone();
        let interval = self.config.check_interval;
        let running_flag = self.running.clone();

        tokio::spawn(async move {
            let mut tick_interval = tokio::time::interval(interval);

            loop {
                tick_interval.tick().await;

                if !*running_flag.read().await {
                    break;
                }

                if let Err(e) = Self::run_health_check(
                    &registry,
                    &service_manager,
                    &heartbeat_manager
                ).await {
                    error!("AutoHealer health check failed: {}", e);
                }
            }

            info!("AutoHealer stopped");
        });

        Ok(())
    }

    /// Stop the auto-healing monitor
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = false;
        info!("Stopping AutoHealer");
        Ok(())
    }

    /// Run a single health check iteration
    async fn run_health_check(
        registry: &Arc<DeviceRegistry>,
        service_manager: &Arc<ServiceManager>,
        heartbeat_manager: &Arc<HeartbeatManager>,
    ) -> Result<()> {
        // 1. Check Peer Health
        let monitored_devices = registry.get_monitored_devices().await;
        for device in monitored_devices {
            let status = heartbeat_manager.check_peer_health(&device.peer_id).await;
            
            match status {
                PeerStatus::Offline => {
                    warn!("Peer {} detected offline by heartbeat", device.peer_id);
                    // Mark as offline in registry
                    registry.update_status(&device.peer_id, PeerStatus::Offline).await?;
                    
                    // Handle services for this offline peer
                    Self::handle_peer_failure(device.peer_id, service_manager).await?;
                }
                PeerStatus::Suspected => {
                    debug!("Peer {} suspected offline (missed heartbeats)", device.peer_id);
                    registry.update_status(&device.peer_id, PeerStatus::Suspected).await?;
                }
                PeerStatus::Online => {
                    // Ensure registry reflects online status
                    if device.status != PeerStatus::Online {
                        registry.update_status(&device.peer_id, PeerStatus::Online).await?;
                    }
                }
            }
        }

        // 2. Check Service Health (Generic)
        // This would involve checking if services are reachable, but for now we rely on 
        // the peer status and explicit service health reports.
        // Future: Active probing of service endpoints.

        Ok(())
    }

    /// Handle a detected peer failure
    /// Handle a detected peer failure
    async fn handle_peer_failure(
        peer_id: PeerId,
        service_manager: &Arc<ServiceManager>,
    ) -> Result<()> {
        warn!("Handling failure for peer {}", peer_id);
        
        // 1. Get all services on this peer
        let services = service_manager.get_services_by_peer(&peer_id);
        
        // 2. Mark them as unhealthy immediately
        for service in services {
            warn!("Marking service {} on failed peer as UNHEALTHY", service.service_id);
            service_manager.mark_service_unhealthy(service.service_id);
        }
        
        Ok(())
    }
}
