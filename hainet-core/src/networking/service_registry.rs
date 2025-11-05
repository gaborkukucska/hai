//! # START OF FILE hainet-core/src/networking/service_registry.rs
//! Service Registry - Centralized service catalog for mesh network
//!
//! Maintains a catalog of all available services across the mesh network.
//! Provides capability matching to find suitable devices for service requirements.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use libp2p::PeerId;
use tracing::{debug, info};

use super::service_manager::{ServiceInfo, ServiceType};
use super::peer_discovery::DeviceCapabilities;
use super::mesh_coordinator::RoleAssignment;

/// Requirements for a service to run on a device
#[derive(Debug, Clone)]
pub struct ServiceRequirements {
    /// Minimum RAM in GB
    pub min_ram_gb: Option<u64>,
    /// Requires GPU
    pub requires_gpu: bool,
    /// Minimum CPU cores
    pub min_cpu_cores: Option<usize>,
    /// Minimum disk space in GB
    pub min_disk_gb: Option<u64>,
}

impl ServiceRequirements {
    /// Create requirements for LLM service
    pub fn for_llm() -> Self {
        Self {
            min_ram_gb: Some(8),
            requires_gpu: false, // Optional but preferred
            min_cpu_cores: Some(4),
            min_disk_gb: Some(20),
        }
    }

    /// Create requirements for STT/TTS service
    pub fn for_stt_tts() -> Self {
        Self {
            min_ram_gb: Some(4),
            requires_gpu: false,
            min_cpu_cores: Some(4),
            min_disk_gb: Some(10),
        }
    }

    /// Create requirements for storage service
    pub fn for_storage() -> Self {
        Self {
            min_ram_gb: Some(2),
            requires_gpu: false,
            min_cpu_cores: Some(2),
            min_disk_gb: Some(500),
        }
    }

    /// Create requirements for MCP server
    pub fn for_mcp() -> Self {
        Self {
            min_ram_gb: Some(2),
            requires_gpu: false,
            min_cpu_cores: Some(2),
            min_disk_gb: Some(5),
        }
    }

    /// Check if device capabilities meet these requirements
    pub fn is_met_by(&self, capabilities: &DeviceCapabilities) -> bool {
        if let Some(min_ram) = self.min_ram_gb {
            if capabilities.ram_gb < min_ram {
                return false;
            }
        }

        if self.requires_gpu && !capabilities.has_gpu {
            return false;
        }

        if let Some(min_cores) = self.min_cpu_cores {
            if (capabilities.cpu_cores as usize) < min_cores {
                return false;
            }
        }

        if let Some(min_disk) = self.min_disk_gb {
            if capabilities.disk_gb < min_disk {
                return false;
            }
        }

        true
    }
}

/// Centralized service registry (runs on Master node)
pub struct ServiceRegistry {
    /// All services indexed by type
    services: Arc<RwLock<HashMap<String, Vec<ServiceInfo>>>>,
    /// Device capabilities indexed by peer
    capabilities: Arc<RwLock<HashMap<PeerId, DeviceCapabilities>>>,
    /// Role assignments indexed by peer
    role_assignments: Arc<RwLock<HashMap<PeerId, RoleAssignment>>>,
}

impl ServiceRegistry {
    /// Create new service registry
    pub fn new() -> Self {
        info!("Creating new ServiceRegistry");
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
            capabilities: Arc::new(RwLock::new(HashMap::new())),
            role_assignments: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a device's capabilities
    pub fn register_capabilities(&self, peer_id: PeerId, capabilities: DeviceCapabilities) {
        debug!("Registering capabilities for peer {}", peer_id);
        self.capabilities.write().unwrap().insert(peer_id, capabilities);
    }

    /// Register a device's role assignment
    pub fn register_role(&self, peer_id: PeerId, role: RoleAssignment) {
        debug!("Registering role for peer {}: {:?}", peer_id, role.assigned_role);
        self.role_assignments.write().unwrap().insert(peer_id, role);
    }

    /// Add a service to the catalog
    pub fn add_service(&self, service: ServiceInfo) {
        let type_key = Self::service_type_key(&service.service_type);
        info!("Adding service {} to catalog (type: {})", service.service_id, type_key);
        
        self.services
            .write()
            .unwrap()
            .entry(type_key)
            .or_insert_with(Vec::new)
            .push(service);
    }

    /// Remove a service from the catalog
    pub fn remove_service(&self, service_id: uuid::Uuid) -> bool {
        let mut services = self.services.write().unwrap();
        let mut found = false;

        for service_list in services.values_mut() {
            if let Some(pos) = service_list.iter().position(|s| s.service_id == service_id) {
                service_list.remove(pos);
                found = true;
                break;
            }
        }

        if found {
            info!("Removed service {} from catalog", service_id);
        } else {
            debug!("Service {} not found in catalog", service_id);
        }

        found
    }

    /// Find devices that meet service requirements
    pub fn match_capabilities(&self, requirements: &ServiceRequirements) -> Vec<(PeerId, DeviceCapabilities)> {
        let capabilities = self.capabilities.read().unwrap();
        
        capabilities
            .iter()
            .filter(|(_, caps)| requirements.is_met_by(caps))
            .map(|(peer, caps)| (*peer, caps.clone()))
            .collect()
    }

    /// Get services by specialized role
    pub fn services_by_role(&self, role: &super::mesh_coordinator::SpecializedRole) -> Vec<ServiceInfo> {
        let role_assignments = self.role_assignments.read().unwrap();
        let services = self.services.read().unwrap();

        // Find peers with this role
        let peers_with_role: Vec<PeerId> = role_assignments
            .iter()
            .filter(|(_, assignment)| assignment.specialized_roles.contains(role))
            .map(|(peer, _)| *peer)
            .collect();

        // Get all services from these peers
        services
            .values()
            .flat_map(|service_list| service_list.iter())
            .filter(|service| peers_with_role.contains(&service.peer_id))
            .cloned()
            .collect()
    }

    /// Get full service catalog
    pub fn get_catalog(&self) -> HashMap<String, Vec<ServiceInfo>> {
        self.services.read().unwrap().clone()
    }

    /// Get services by type
    pub fn get_services_by_type(&self, service_type: &ServiceType) -> Vec<ServiceInfo> {
        let type_key = Self::service_type_key(service_type);
        self.services
            .read()
            .unwrap()
            .get(&type_key)
            .cloned()
            .unwrap_or_default()
    }

    /// Get device capabilities
    pub fn get_capabilities(&self, peer_id: &PeerId) -> Option<DeviceCapabilities> {
        self.capabilities.read().unwrap().get(peer_id).cloned()
    }

    /// Get role assignment
    pub fn get_role(&self, peer_id: &PeerId) -> Option<RoleAssignment> {
        self.role_assignments.read().unwrap().get(peer_id).cloned()
    }

    /// Get registry statistics
    pub fn get_stats(&self) -> RegistryStats {
        let services = self.services.read().unwrap();
        let capabilities = self.capabilities.read().unwrap();
        let roles = self.role_assignments.read().unwrap();

        RegistryStats {
            total_services: services.values().map(|v| v.len()).sum(),
            total_devices: capabilities.len(),
            total_roles_assigned: roles.len(),
            services_by_type: services
                .iter()
                .map(|(k, v)| (k.clone(), v.len()))
                .collect(),
        }
    }

    /// Convert service type to string key
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

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the service registry
#[derive(Debug, Clone)]
pub struct RegistryStats {
    /// Total number of registered services
    pub total_services: usize,
    /// Total number of devices
    pub total_devices: usize,
    /// Total role assignments
    pub total_roles_assigned: usize,
    /// Services by type
    pub services_by_type: HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_peer_id() -> PeerId {
        PeerId::random()
    }

    fn create_test_capabilities(ram_gb: u64, cpu_cores: usize, has_gpu: bool, disk_gb: u64) -> DeviceCapabilities {
        DeviceCapabilities {
            cpu_cores: cpu_cores as u8,
            ram_gb,
            has_gpu,
            gpu_memory_mb: if has_gpu { 8192 } else { 0 },
            disk_gb,
            os: "Linux".to_string(),
            arch: "x86_64".to_string(),
            score: 0.0,
        }
    }

    #[test]
    fn test_registry_creation() {
        let registry = ServiceRegistry::new();
        let stats = registry.get_stats();
        assert_eq!(stats.total_services, 0);
        assert_eq!(stats.total_devices, 0);
    }

    #[test]
    fn test_capability_registration() {
        let registry = ServiceRegistry::new();
        let peer_id = create_test_peer_id();
        let caps = create_test_capabilities(16, 8, true, 500);

        registry.register_capabilities(peer_id, caps.clone());

        let retrieved = registry.get_capabilities(&peer_id).unwrap();
        assert_eq!(retrieved.ram_gb, 16);
        assert_eq!(retrieved.cpu_cores, 8);
        assert!(retrieved.has_gpu);
    }

    #[test]
    fn test_service_addition() {
        let registry = ServiceRegistry::new();
        let peer_id = create_test_peer_id();
        
        let service = ServiceInfo::new(
            ServiceType::LLM { models: vec!["gemma3:7b".to_string()] },
            peer_id,
            "http://localhost:11434".to_string(),
        );

        registry.add_service(service.clone());

        let services = registry.get_services_by_type(&ServiceType::LLM { models: vec![] });
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].service_id, service.service_id);
    }

    #[test]
    fn test_service_removal() {
        let registry = ServiceRegistry::new();
        let peer_id = create_test_peer_id();
        
        let service = ServiceInfo::new(
            ServiceType::STT { engine: "whisper".to_string() },
            peer_id,
            "http://localhost:8080".to_string(),
        );
        let service_id = service.service_id;

        registry.add_service(service);
        assert!(registry.remove_service(service_id));

        let services = registry.get_services_by_type(&ServiceType::STT { engine: String::new() });
        assert_eq!(services.len(), 0);
    }

    #[test]
    fn test_capability_matching_llm() {
        let registry = ServiceRegistry::new();
        
        let peer1 = create_test_peer_id();
        let caps1 = create_test_capabilities(16, 8, true, 500); // Meets requirements
        registry.register_capabilities(peer1, caps1);

        let peer2 = create_test_peer_id();
        let caps2 = create_test_capabilities(4, 2, false, 100); // Does not meet requirements
        registry.register_capabilities(peer2, caps2);

        let requirements = ServiceRequirements::for_llm();
        let matches = registry.match_capabilities(&requirements);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, peer1);
    }

    #[test]
    fn test_capability_matching_storage() {
        let registry = ServiceRegistry::new();
        
        let peer1 = create_test_peer_id();
        let caps1 = create_test_capabilities(4, 4, false, 1000); // Meets storage requirements
        registry.register_capabilities(peer1, caps1);

        let peer2 = create_test_peer_id();
        let caps2 = create_test_capabilities(16, 8, true, 100); // High specs but low disk
        registry.register_capabilities(peer2, caps2);

        let requirements = ServiceRequirements::for_storage();
        let matches = registry.match_capabilities(&requirements);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, peer1);
    }

    #[test]
    fn test_registry_stats() {
        let registry = ServiceRegistry::new();
        
        let peer1 = create_test_peer_id();
        let caps1 = create_test_capabilities(16, 8, true, 500);
        registry.register_capabilities(peer1, caps1);

        let peer2 = create_test_peer_id();
        let caps2 = create_test_capabilities(8, 4, false, 250);
        registry.register_capabilities(peer2, caps2);

        let service1 = ServiceInfo::new(
            ServiceType::LLM { models: vec!["gemma3:7b".to_string()] },
            peer1,
            "http://localhost:11434".to_string(),
        );
        registry.add_service(service1);

        let service2 = ServiceInfo::new(
            ServiceType::STT { engine: "whisper".to_string() },
            peer2,
            "http://localhost:8080".to_string(),
        );
        registry.add_service(service2);

        let stats = registry.get_stats();
        assert_eq!(stats.total_services, 2);
        assert_eq!(stats.total_devices, 2);
        assert_eq!(*stats.services_by_type.get("LLM").unwrap(), 1);
        assert_eq!(*stats.services_by_type.get("STT").unwrap(), 1);
    }
}
