//! # START OF FILE hainet-core/src/networking/service_manager.rs
//! Service Manager - Service lifecycle management and discovery
//!
//! Manages service registration, health checking, and discovery across the mesh network.
//! Services include LLM inference, STT/TTS, distributed storage, and MCP servers.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use libp2p::PeerId;
use uuid::Uuid;
use tracing::{debug, info, warn};

/// Type of service available in the mesh network
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ServiceType {
    /// Large Language Model inference service
    LLM { models: Vec<String> },
    /// Speech-to-Text service
    STT { engine: String },
    /// Text-to-Speech service
    TTS { engine: String },
    /// Distributed storage service
    Storage { capacity_gb: u64 },
    /// Model Context Protocol server
    MCP { servers: Vec<String> },
}

/// Health status of a service
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceHealth {
    /// Service is healthy and accepting requests
    Healthy,
    /// Service is degraded but still functional
    Degraded,
    /// Service is unhealthy and should not receive requests
    Unhealthy,
}

/// Information about a registered service
#[derive(Debug, Clone)]
pub struct ServiceInfo {
    /// Unique service identifier
    pub service_id: Uuid,
    /// Type of service
    pub service_type: ServiceType,
    /// Peer hosting the service
    pub peer_id: PeerId,
    /// Service endpoint URL
    pub endpoint: String,
    /// Current health status
    pub health_status: ServiceHealth,
    /// When service was registered
    pub registered_at: SystemTime,
    /// Last health check timestamp
    pub last_health_check: SystemTime,
    /// Number of consecutive health check failures
    pub consecutive_failures: u32,
}

impl ServiceInfo {
    /// Create new service info
    pub fn new(service_type: ServiceType, peer_id: PeerId, endpoint: String) -> Self {
        let now = SystemTime::now();
        Self {
            service_id: Uuid::new_v4(),
            service_type,
            peer_id,
            endpoint,
            health_status: ServiceHealth::Healthy,
            registered_at: now,
            last_health_check: now,
            consecutive_failures: 0,
        }
    }

    /// Check if service is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self.health_status, ServiceHealth::Healthy)
    }

    /// Check if service is degraded
    pub fn is_degraded(&self) -> bool {
        matches!(self.health_status, ServiceHealth::Degraded)
    }

    /// Check if service is unhealthy
    pub fn is_unhealthy(&self) -> bool {
        matches!(self.health_status, ServiceHealth::Unhealthy)
    }
}

/// Statistics about registered services
#[derive(Debug, Clone)]
pub struct ServiceStats {
    /// Total number of registered services
    pub total_services: usize,
    /// Number of healthy services
    pub healthy_services: usize,
    /// Number of degraded services
    pub degraded_services: usize,
    /// Number of unhealthy services
    pub unhealthy_services: usize,
    /// Services by type
    pub services_by_type: HashMap<String, usize>,
}

/// Service manager for lifecycle management and discovery
pub struct ServiceManager {
    /// All registered services (service_id -> ServiceInfo)
    services: Arc<RwLock<HashMap<Uuid, ServiceInfo>>>,
    /// Services indexed by type for fast discovery
    services_by_type: Arc<RwLock<HashMap<String, Vec<Uuid>>>>,
    /// Services indexed by peer for cleanup
    services_by_peer: Arc<RwLock<HashMap<PeerId, Vec<Uuid>>>>,
}

impl ServiceManager {
    /// Create new service manager
    pub fn new() -> Self {
        info!("Creating new ServiceManager");
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
            services_by_type: Arc::new(RwLock::new(HashMap::new())),
            services_by_peer: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new service
    pub fn register_service(
        &self,
        service_type: ServiceType,
        peer_id: PeerId,
        endpoint: String,
    ) -> Uuid {
        let service_info = ServiceInfo::new(service_type.clone(), peer_id, endpoint.clone());
        let service_id = service_info.service_id;

        info!(
            "Registering service {} for peer {} at {}",
            service_id, peer_id, endpoint
        );

        // Add to main registry
        self.services
            .write()
            .unwrap()
            .insert(service_id, service_info);

        // Index by type
        let type_key = Self::service_type_key(&service_type);
        self.services_by_type
            .write()
            .unwrap()
            .entry(type_key)
            .or_insert_with(Vec::new)
            .push(service_id);

        // Index by peer
        self.services_by_peer
            .write()
            .unwrap()
            .entry(peer_id)
            .or_insert_with(Vec::new)
            .push(service_id);

        service_id
    }

    /// Unregister a service
    pub fn unregister_service(&self, service_id: Uuid) -> bool {
        debug!("Unregistering service {}", service_id);

        let service = {
            let mut services = self.services.write().unwrap();
            services.remove(&service_id)
        };

        if let Some(service) = service {
            // Remove from type index
            let type_key = Self::service_type_key(&service.service_type);
            if let Some(type_services) = self.services_by_type.write().unwrap().get_mut(&type_key) {
                type_services.retain(|id| *id != service_id);
            }

            // Remove from peer index
            if let Some(peer_services) = self.services_by_peer.write().unwrap().get_mut(&service.peer_id) {
                peer_services.retain(|id| *id != service_id);
            }

            info!("Service {} unregistered successfully", service_id);
            true
        } else {
            warn!("Service {} not found for unregistration", service_id);
            false
        }
    }

    /// Unregister all services for a peer (e.g., when peer goes offline)
    pub fn unregister_peer_services(&self, peer_id: &PeerId) -> usize {
        let service_ids: Vec<Uuid> = {
            let peer_services = self.services_by_peer.read().unwrap();
            peer_services.get(peer_id).cloned().unwrap_or_default()
        };

        let count = service_ids.len();
        for service_id in service_ids {
            self.unregister_service(service_id);
        }

        if count > 0 {
            info!("Unregistered {} services for peer {}", count, peer_id);
        }
        count
    }

    /// Discover services by type
    pub fn discover_services(&self, service_type: &ServiceType) -> Vec<ServiceInfo> {
        let type_key = Self::service_type_key(service_type);
        let service_ids = {
            let by_type = self.services_by_type.read().unwrap();
            by_type.get(&type_key).cloned().unwrap_or_default()
        };

        let services = self.services.read().unwrap();
        service_ids
            .iter()
            .filter_map(|id| services.get(id).cloned())
            .collect()
    }

    /// Get all healthy services of a type
    pub fn get_healthy_services(&self, service_type: &ServiceType) -> Vec<ServiceInfo> {
        self.discover_services(service_type)
            .into_iter()
            .filter(|s| s.is_healthy())
            .collect()
    }

    /// Get service by ID
    pub fn get_service(&self, service_id: Uuid) -> Option<ServiceInfo> {
        self.services.read().unwrap().get(&service_id).cloned()
    }

    /// Update service health status
    pub fn update_health(&self, service_id: Uuid, is_healthy: bool) {
        let mut services = self.services.write().unwrap();
        if let Some(service) = services.get_mut(&service_id) {
            service.last_health_check = SystemTime::now();

            if is_healthy {
                // Reset failures and mark healthy
                service.consecutive_failures = 0;
                service.health_status = ServiceHealth::Healthy;
                debug!("Service {} marked healthy", service_id);
            } else {
                // Increment failures and degrade health
                service.consecutive_failures += 1;
                service.health_status = match service.consecutive_failures {
                    1 => ServiceHealth::Degraded,
                    _ => ServiceHealth::Unhealthy,
                };
                warn!(
                    "Service {} health check failed ({} consecutive failures) - now {:?}",
                    service_id, service.consecutive_failures, service.health_status
                );
            }
        }
    }

    /// Get statistics about registered services
    pub fn get_stats(&self) -> ServiceStats {
        let services = self.services.read().unwrap();
        let mut stats = ServiceStats {
            total_services: services.len(),
            healthy_services: 0,
            degraded_services: 0,
            unhealthy_services: 0,
            services_by_type: HashMap::new(),
        };

        for service in services.values() {
            match service.health_status {
                ServiceHealth::Healthy => stats.healthy_services += 1,
                ServiceHealth::Degraded => stats.degraded_services += 1,
                ServiceHealth::Unhealthy => stats.unhealthy_services += 1,
            }

            let type_key = Self::service_type_key(&service.service_type);
            *stats.services_by_type.entry(type_key).or_insert(0) += 1;
        }

        stats
    }

    /// Get all registered services
    pub fn get_all_services(&self) -> Vec<ServiceInfo> {
        self.services.read().unwrap().values().cloned().collect()
    }

    /// Convert service type to string key for indexing
    fn service_type_key(service_type: &ServiceType) -> String {
        match service_type {
            ServiceType::LLM { .. } => "LLM".to_string(),
            ServiceType::STT { .. } => "STT".to_string(),
            ServiceType::TTS { .. } => "TTS".to_string(),
            ServiceType::Storage { .. } => "Storage".to_string(),
            ServiceType::MCP { .. } => "MCP".to_string(),
        }
    }
}

impl Default for ServiceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_peer_id() -> PeerId {
        PeerId::random()
    }

    #[test]
    fn test_service_manager_creation() {
        let manager = ServiceManager::new();
        let stats = manager.get_stats();
        assert_eq!(stats.total_services, 0);
    }

    #[test]
    fn test_service_registration() {
        let manager = ServiceManager::new();
        let peer_id = create_test_peer_id();
        let service_type = ServiceType::LLM {
            models: vec!["gemma3:7b".to_string()],
        };

        let service_id = manager.register_service(
            service_type.clone(),
            peer_id,
            "http://localhost:11434".to_string(),
        );

        let service = manager.get_service(service_id).unwrap();
        assert_eq!(service.peer_id, peer_id);
        assert_eq!(service.endpoint, "http://localhost:11434");
        assert!(service.is_healthy());
    }

    #[test]
    fn test_service_unregistration() {
        let manager = ServiceManager::new();
        let peer_id = create_test_peer_id();
        let service_type = ServiceType::STT {
            engine: "whisper".to_string(),
        };

        let service_id = manager.register_service(
            service_type,
            peer_id,
            "http://localhost:8080".to_string(),
        );

        assert!(manager.unregister_service(service_id));
        assert!(manager.get_service(service_id).is_none());
    }

    #[test]
    fn test_service_discovery() {
        let manager = ServiceManager::new();
        let peer_id = create_test_peer_id();

        let llm_type = ServiceType::LLM {
            models: vec!["gemma3:7b".to_string()],
        };
        manager.register_service(llm_type.clone(), peer_id, "http://localhost:11434".to_string());

        let stt_type = ServiceType::STT {
            engine: "whisper".to_string(),
        };
        manager.register_service(stt_type.clone(), peer_id, "http://localhost:8080".to_string());

        let llm_services = manager.discover_services(&llm_type);
        assert_eq!(llm_services.len(), 1);

        let stt_services = manager.discover_services(&stt_type);
        assert_eq!(stt_services.len(), 1);
    }

    #[test]
    fn test_health_updates() {
        let manager = ServiceManager::new();
        let peer_id = create_test_peer_id();
        let service_type = ServiceType::LLM {
            models: vec!["gemma3:7b".to_string()],
        };

        let service_id = manager.register_service(
            service_type,
            peer_id,
            "http://localhost:11434".to_string(),
        );

        // First failure -> Degraded
        manager.update_health(service_id, false);
        let service = manager.get_service(service_id).unwrap();
        assert!(service.is_degraded());

        // Second failure -> Unhealthy
        manager.update_health(service_id, false);
        let service = manager.get_service(service_id).unwrap();
        assert!(service.is_unhealthy());

        // Recovery -> Healthy
        manager.update_health(service_id, true);
        let service = manager.get_service(service_id).unwrap();
        assert!(service.is_healthy());
    }

    #[test]
    fn test_peer_service_cleanup() {
        let manager = ServiceManager::new();
        let peer_id = create_test_peer_id();

        // Register multiple services for same peer
        let llm_type = ServiceType::LLM {
            models: vec!["gemma3:7b".to_string()],
        };
        manager.register_service(llm_type, peer_id, "http://localhost:11434".to_string());

        let stt_type = ServiceType::STT {
            engine: "whisper".to_string(),
        };
        manager.register_service(stt_type, peer_id, "http://localhost:8080".to_string());

        let stats = manager.get_stats();
        assert_eq!(stats.total_services, 2);

        // Cleanup all services for peer
        let removed = manager.unregister_peer_services(&peer_id);
        assert_eq!(removed, 2);

        let stats = manager.get_stats();
        assert_eq!(stats.total_services, 0);
    }

    #[test]
    fn test_get_healthy_services() {
        let manager = ServiceManager::new();
        let peer_id = create_test_peer_id();

        let service_type = ServiceType::LLM {
            models: vec!["gemma3:7b".to_string()],
        };

        let service_id1 = manager.register_service(
            service_type.clone(),
            peer_id,
            "http://localhost:11434".to_string(),
        );
        let service_id2 = manager.register_service(
            service_type.clone(),
            peer_id,
            "http://localhost:11435".to_string(),
        );

        // Mark one unhealthy
        manager.update_health(service_id2, false);
        manager.update_health(service_id2, false);

        let healthy = manager.get_healthy_services(&service_type);
        assert_eq!(healthy.len(), 1);
        assert_eq!(healthy[0].service_id, service_id1);
    }

    #[test]
    fn test_service_stats() {
        let manager = ServiceManager::new();
        let peer_id = create_test_peer_id();

        let llm_type = ServiceType::LLM {
            models: vec!["gemma3:7b".to_string()],
        };
        let service_id = manager.register_service(llm_type.clone(), peer_id, "http://localhost:11434".to_string());

        let stt_type = ServiceType::STT {
            engine: "whisper".to_string(),
        };
        manager.register_service(stt_type, peer_id, "http://localhost:8080".to_string());

        // Mark one degraded
        manager.update_health(service_id, false);

        let stats = manager.get_stats();
        assert_eq!(stats.total_services, 2);
        assert_eq!(stats.healthy_services, 1);
        assert_eq!(stats.degraded_services, 1);
        assert_eq!(stats.unhealthy_services, 0);
        assert_eq!(*stats.services_by_type.get("LLM").unwrap(), 1);
        assert_eq!(*stats.services_by_type.get("STT").unwrap(), 1);
    }
}
