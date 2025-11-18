//! # START OF FILE hainet-core/src/networking/service_detector.rs
//! Service Auto-Discovery - Detects running services on mesh devices
//!
//! Automatically discovers Ollama, Whisper, Piper, and MCP servers running
//! across the mesh network and registers them with the ServiceRegistry.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use libp2p::PeerId;
use tracing::{debug, info};
use tokio::time::{timeout, Duration};

use super::service_manager::{ServiceInfo, ServiceType};
use super::service_registry::ServiceRegistry;
use super::mesh_coordinator::MeshCoordinator;

/// Discovered service information
#[derive(Debug, Clone)]
pub struct DiscoveredService {
    /// Service type
    pub service_type: ServiceType,
    /// Device where service is running
    pub peer_id: PeerId,
    /// Service endpoint (HTTP URL or other)
    pub endpoint: String,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Service detection configuration
#[derive(Debug, Clone)]
pub struct DetectorConfig {
    /// Timeout for service probes (default: 5s)
    pub probe_timeout: Duration,
    /// Ollama default port
    pub ollama_port: u16,
    /// Whisper default port
    pub whisper_port: u16,
    /// Piper default port
    pub piper_port: u16,
    /// MCP servers default port (HTTP transport)
    pub mcp_port: u16,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            probe_timeout: Duration::from_secs(5),
            ollama_port: 11434,
            whisper_port: 8090,
            piper_port: 8091,
            mcp_port: 8092,
        }
    }
}

/// Service auto-discovery system
pub struct ServiceDetector {
    /// Mesh coordinator for device discovery
    mesh_coordinator: Arc<MeshCoordinator>,
    /// Service registry for registration
    service_registry: Arc<ServiceRegistry>,
    /// Detection configuration
    config: DetectorConfig,
    /// HTTP client for probing services
    http_client: reqwest::Client,
    /// Discovered services cache
    discovered_services: Arc<RwLock<Vec<DiscoveredService>>>,
}

impl ServiceDetector {
    /// Create new service detector
    pub fn new(
        mesh_coordinator: Arc<MeshCoordinator>,
        service_registry: Arc<ServiceRegistry>,
    ) -> Self {
        Self::with_config(mesh_coordinator, service_registry, DetectorConfig::default())
    }

    /// Create service detector with custom configuration
    pub fn with_config(
        mesh_coordinator: Arc<MeshCoordinator>,
        service_registry: Arc<ServiceRegistry>,
        config: DetectorConfig,
    ) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(config.probe_timeout)
            .build()
            .expect("Failed to build HTTP client");

        info!("ServiceDetector initialized with {:?}", config);

        Self {
            mesh_coordinator,
            service_registry,
            config,
            http_client,
            discovered_services: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Discover all services on the mesh network
    pub async fn discover_all(&self) -> Result<Vec<DiscoveredService>> {
        info!("Starting service discovery across mesh network...");

        let all_services = Vec::new();

        // Get all role assignments from mesh coordinator
        let role_assignments = self.mesh_coordinator.get_all_assignments().await;

        for (peer_id, assignment) in role_assignments {
            // Get device info from service registry to extract IP and hostname
            // The ServiceRegistry should have stored PeerInfo when devices were registered
            // For now, we'll need to enhance the discovery to pass IP addresses
            // This will be improved in the integration test phase
            
            // Discover services based on specialized roles
            // Note: In real deployment, IP addresses come from PeerInfo during registration
            for role in &assignment.specialized_roles {
                // Skip discovery for now - will be implemented with proper IP tracking
                // This will be resolved when we add a method to get PeerInfo from assignments
                debug!("Role {:?} assigned to peer {}, but IP resolution pending", role, peer_id);
            }
        }

        // Update cache
        *self.discovered_services.write().unwrap() = all_services.clone();

        info!("Service discovery complete: {} services found", all_services.len());
        Ok(all_services)
    }

    /// Probe for Ollama LLM service
    async fn probe_ollama(&self, peer_id: PeerId, ip: &str) -> Option<DiscoveredService> {
        let endpoint = format!("http://{}:{}", ip, self.config.ollama_port);
        debug!("Probing Ollama at {}", endpoint);

        // Try to fetch Ollama's /api/tags endpoint (lists available models)
        let tags_url = format!("{}/api/tags", endpoint);

        match timeout(
            self.config.probe_timeout,
            self.http_client.get(&tags_url).send(),
        )
        .await
        {
            Ok(Ok(response)) if response.status().is_success() => {
                // Parse response to get model list
                if let Ok(body) = response.json::<serde_json::Value>().await {
                    if let Some(models) = body.get("models").and_then(|m| m.as_array()) {
                        let model_names: Vec<String> = models
                            .iter()
                            .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(String::from))
                            .collect();

                        info!("✓ Found Ollama at {} with {} models", endpoint, model_names.len());

                        let mut metadata = HashMap::new();
                        metadata.insert("models".to_string(), model_names.join(","));

                        return Some(DiscoveredService {
                            service_type: ServiceType::LLM { models: model_names },
                            peer_id,
                            endpoint,
                            metadata,
                        });
                    }
                }

                // Ollama is running but couldn't parse models
                info!("✓ Found Ollama at {} (model list unavailable)", endpoint);
                Some(DiscoveredService {
                    service_type: ServiceType::LLM { models: vec![] },
                    peer_id,
                    endpoint,
                    metadata: HashMap::new(),
                })
            }
            _ => {
                debug!("Ollama not found at {}", endpoint);
                None
            }
        }
    }

    /// Probe for Whisper STT service
    async fn probe_whisper(&self, peer_id: PeerId, ip: &str) -> Option<DiscoveredService> {
        let endpoint = format!("http://{}:{}", ip, self.config.whisper_port);
        debug!("Probing Whisper at {}", endpoint);

        // Try to fetch Whisper's health endpoint
        let health_url = format!("{}/health", endpoint);

        match timeout(
            self.config.probe_timeout,
            self.http_client.get(&health_url).send(),
        )
        .await
        {
            Ok(Ok(response)) if response.status().is_success() => {
                info!("✓ Found Whisper at {}", endpoint);

                let mut metadata = HashMap::new();
                metadata.insert("engine".to_string(), "whisper".to_string());

                Some(DiscoveredService {
                    service_type: ServiceType::STT {
                        engine: "whisper".to_string(),
                    },
                    peer_id,
                    endpoint,
                    metadata,
                })
            }
            _ => {
                debug!("Whisper not found at {}", endpoint);
                None
            }
        }
    }

    /// Probe for Piper TTS service
    async fn probe_piper(&self, peer_id: PeerId, ip: &str) -> Option<DiscoveredService> {
        let endpoint = format!("http://{}:{}", ip, self.config.piper_port);
        debug!("Probing Piper at {}", endpoint);

        // Try to fetch Piper's health endpoint
        let health_url = format!("{}/health", endpoint);

        match timeout(
            self.config.probe_timeout,
            self.http_client.get(&health_url).send(),
        )
        .await
        {
            Ok(Ok(response)) if response.status().is_success() => {
                info!("✓ Found Piper at {}", endpoint);

                let mut metadata = HashMap::new();
                metadata.insert("engine".to_string(), "piper".to_string());

                Some(DiscoveredService {
                    service_type: ServiceType::TTS {
                        engine: "piper".to_string(),
                    },
                    peer_id,
                    endpoint,
                    metadata,
                })
            }
            _ => {
                debug!("Piper not found at {}", endpoint);
                None
            }
        }
    }

    /// Probe for MCP servers
    async fn probe_mcp_servers(&self, peer_id: PeerId, ip: &str) -> Vec<DiscoveredService> {
        debug!("Probing MCP servers at {}", ip);

        // MCP servers typically run via stdio, but we check for HTTP-based MCP servers
        // Known servers: hainet-files, hainet-dev, hainet-system
        let mut services = Vec::new();

        let known_servers = vec![
            ("hainet-files", 8092),
            ("hainet-dev", 8093),
            ("hainet-system", 8094),
        ];

        for (server_name, port) in known_servers {
            let endpoint = format!("http://{}:{}", ip, port);
            let health_url = format!("{}/health", endpoint);

            match timeout(
                self.config.probe_timeout,
                self.http_client.get(&health_url).send(),
            )
            .await
            {
                Ok(Ok(response)) if response.status().is_success() => {
                    info!("✓ Found MCP server {} at {}", server_name, endpoint);

                    let mut metadata = HashMap::new();
                    metadata.insert("server_name".to_string(), server_name.to_string());

                    services.push(DiscoveredService {
                        service_type: ServiceType::MCP {
                            servers: vec![server_name.to_string()],
                        },
                        peer_id,
                        endpoint,
                        metadata,
                    });
                }
                _ => {
                    debug!("MCP server {} not found at {}", server_name, endpoint);
                }
            }
        }

        services
    }

    /// Register all discovered services with the service registry
    pub async fn register_discovered(&self) -> Result<usize> {
        let services = self.discover_all().await?;

        let mut registered_count = 0;

        for discovered in services {
            let service_info = ServiceInfo::new(
                discovered.service_type.clone(),
                discovered.peer_id,
                discovered.endpoint.clone(),
            );

            self.service_registry.add_service(service_info);
            registered_count += 1;

            info!(
                "Registered service: {} at {}",
                Self::service_type_name(&discovered.service_type),
                discovered.endpoint
            );
        }

        Ok(registered_count)
    }

    /// Get cached discovered services
    pub fn get_cached_services(&self) -> Vec<DiscoveredService> {
        self.discovered_services.read().unwrap().clone()
    }

    /// Get service type name for logging
    fn service_type_name(service_type: &ServiceType) -> &str {
        match service_type {
            ServiceType::LLM { .. } => "LLM",
            ServiceType::STT { .. } => "STT",
            ServiceType::TTS { .. } => "TTS",
            ServiceType::Storage { .. } => "Storage",
            ServiceType::MCP { .. } => "MCP",
        }
    }

    /// Check if a specific service type is available
    pub fn has_service_type(&self, service_type_name: &str) -> bool {
        self.discovered_services
            .read()
            .unwrap()
            .iter()
            .any(|s| Self::service_type_name(&s.service_type) == service_type_name)
    }

    /// Get discovery statistics
    pub fn get_stats(&self) -> DetectorStats {
        let services = self.discovered_services.read().unwrap();

        let mut by_type = HashMap::new();
        for service in services.iter() {
            let type_name = Self::service_type_name(&service.service_type);
            *by_type.entry(type_name.to_string()).or_insert(0) += 1;
        }

        DetectorStats {
            total_services: services.len(),
            services_by_type: by_type,
        }
    }
}

/// Service detector statistics
#[derive(Debug, Clone)]
pub struct DetectorStats {
    /// Total discovered services
    pub total_services: usize,
    /// Services by type
    pub services_by_type: HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::networking::registry::DeviceRegistry;

    fn create_test_peer_id() -> PeerId {
        PeerId::random()
    }

    fn create_test_capabilities(ram_gb: u64, has_gpu: bool) -> DeviceCapabilities {
        DeviceCapabilities {
            cpu_cores: 4,
            ram_gb,
            has_gpu,
            gpu_memory_mb: if has_gpu { 8192 } else { 0 },
            disk_gb: 500,
            os: "Linux".to_string(),
            arch: "x86_64".to_string(),
            score: 0.0,
        }
    }

    #[test]
    fn test_detector_creation() {
        let peer_id = create_test_peer_id();
        let caps = create_test_capabilities(8, false);
        let registry = Arc::new(DeviceRegistry::new(peer_id));
        let coordinator = Arc::new(MeshCoordinator::new(peer_id, caps, registry.clone()));
        let service_registry = Arc::new(ServiceRegistry::new());

        let detector = ServiceDetector::new(coordinator, service_registry);

        let stats = detector.get_stats();
        assert_eq!(stats.total_services, 0);
    }

    #[test]
    fn test_detector_with_custom_config() {
        let peer_id = create_test_peer_id();
        let caps = create_test_capabilities(8, false);
        let registry = Arc::new(DeviceRegistry::new(peer_id));
        let coordinator = Arc::new(MeshCoordinator::new(peer_id, caps, registry.clone()));
        let service_registry = Arc::new(ServiceRegistry::new());

        let config = DetectorConfig {
            probe_timeout: Duration::from_secs(10),
            ollama_port: 11434,
            whisper_port: 8090,
            piper_port: 8091,
            mcp_port: 8092,
        };

        let detector = ServiceDetector::with_config(coordinator, service_registry, config.clone());

        assert_eq!(detector.config.probe_timeout, Duration::from_secs(10));
        assert_eq!(detector.config.ollama_port, 11434);
    }

    #[test]
    fn test_cached_services_empty() {
        let peer_id = create_test_peer_id();
        let caps = create_test_capabilities(8, false);
        let registry = Arc::new(DeviceRegistry::new(peer_id));
        let coordinator = Arc::new(MeshCoordinator::new(peer_id, caps, registry.clone()));
        let service_registry = Arc::new(ServiceRegistry::new());

        let detector = ServiceDetector::new(coordinator, service_registry);

        let cached = detector.get_cached_services();
        assert_eq!(cached.len(), 0);
    }

    #[test]
    fn test_has_service_type_empty() {
        let peer_id = create_test_peer_id();
        let caps = create_test_capabilities(8, false);
        let registry = Arc::new(DeviceRegistry::new(peer_id));
        let coordinator = Arc::new(MeshCoordinator::new(peer_id, caps, registry.clone()));
        let service_registry = Arc::new(ServiceRegistry::new());

        let detector = ServiceDetector::new(coordinator, service_registry);

        assert!(!detector.has_service_type("LLM"));
        assert!(!detector.has_service_type("STT"));
    }

    #[test]
    fn test_detector_stats() {
        let peer_id = create_test_peer_id();
        let caps = create_test_capabilities(8, false);
        let registry = Arc::new(DeviceRegistry::new(peer_id));
        let coordinator = Arc::new(MeshCoordinator::new(peer_id, caps, registry.clone()));
        let service_registry = Arc::new(ServiceRegistry::new());

        let detector = ServiceDetector::new(coordinator, service_registry);

        let stats = detector.get_stats();
        assert_eq!(stats.total_services, 0);
        assert_eq!(stats.services_by_type.len(), 0);
    }
}
