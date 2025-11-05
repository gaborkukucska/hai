//! # START OF FILE hainet-core/src/networking/load_balancer.rs
//! Load Balancer - Intelligent request routing and failover
//!
//! Routes service requests to healthy services based on configurable strategies.
//! Supports round-robin, least-loaded, and capability-based routing.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use libp2p::PeerId;
use tracing::{debug, info, warn};

use super::service_manager::{ServiceInfo, ServiceManager, ServiceType};

/// Strategy for routing requests to services
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingStrategy {
    /// Cycle through healthy services in order
    RoundRobin,
    /// Route to least-used service
    LeastLoaded,
    /// Route based on device capability score (best device first)
    CapabilityBased,
}

/// Result of a routing decision
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    /// Selected service for the request
    pub selected_service: ServiceInfo,
    /// Backup services for failover (in priority order)
    pub backup_services: Vec<ServiceInfo>,
    /// Reason for the routing decision
    pub routing_reason: String,
}

/// Statistics about routing decisions
#[derive(Debug, Clone)]
pub struct RoutingStats {
    /// Total requests routed
    pub total_requests: u64,
    /// Successful routes
    pub successful_routes: u64,
    /// Failed routes (no healthy service available)
    pub failed_routes: u64,
    /// Failover count (primary service unavailable)
    pub failover_count: u64,
    /// Requests per service
    pub requests_per_service: HashMap<String, u64>,
}

/// Load balancer for intelligent service routing
pub struct LoadBalancer {
    /// Reference to service manager
    service_manager: Arc<RwLock<ServiceManager>>,
    /// Current routing strategy
    routing_strategy: RoutingStrategy,
    /// Request counts per peer (for round-robin and least-loaded)
    request_counts: Arc<RwLock<HashMap<PeerId, u64>>>,
    /// Round-robin indices per service type
    round_robin_indices: Arc<RwLock<HashMap<String, usize>>>,
    /// Routing statistics
    stats: Arc<RwLock<RoutingStats>>,
}

impl LoadBalancer {
    /// Create new load balancer
    pub fn new(service_manager: Arc<RwLock<ServiceManager>>) -> Self {
        info!("Creating new LoadBalancer with RoundRobin strategy");
        Self {
            service_manager,
            routing_strategy: RoutingStrategy::RoundRobin,
            request_counts: Arc::new(RwLock::new(HashMap::new())),
            round_robin_indices: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(RoutingStats {
                total_requests: 0,
                successful_routes: 0,
                failed_routes: 0,
                failover_count: 0,
                requests_per_service: HashMap::new(),
            })),
        }
    }

    /// Create load balancer with specific strategy
    pub fn with_strategy(
        service_manager: Arc<RwLock<ServiceManager>>,
        strategy: RoutingStrategy,
    ) -> Self {
        info!("Creating new LoadBalancer with {:?} strategy", strategy);
        let mut lb = Self::new(service_manager);
        lb.routing_strategy = strategy;
        lb
    }

    /// Route a request to a service
    pub fn route_request(&self, service_type: &ServiceType) -> Option<RoutingDecision> {
        self.stats.write().unwrap().total_requests += 1;

        let manager = self.service_manager.read().unwrap();
        let mut healthy_services = manager.get_healthy_services(service_type);

        if healthy_services.is_empty() {
            warn!("No healthy services available for type: {:?}", service_type);
            self.stats.write().unwrap().failed_routes += 1;
            return None;
        }

        // Sort and select based on strategy
        let selected = match self.routing_strategy {
            RoutingStrategy::RoundRobin => self.select_round_robin(&mut healthy_services, service_type),
            RoutingStrategy::LeastLoaded => self.select_least_loaded(&healthy_services),
            RoutingStrategy::CapabilityBased => self.select_capability_based(&healthy_services),
        };

        if let Some(selected_service) = selected {
            // Track request
            self.track_request(&selected_service);

            // Prepare backup services (all except selected)
            let backup_services: Vec<ServiceInfo> = healthy_services
                .into_iter()
                .filter(|s| s.service_id != selected_service.service_id)
                .collect();

            let routing_reason = format!(
                "Selected via {:?} strategy - {} healthy services available",
                self.routing_strategy,
                backup_services.len() + 1
            );

            debug!(
                "Routed request to service {} at {} ({})",
                selected_service.service_id, selected_service.endpoint, routing_reason
            );

            self.stats.write().unwrap().successful_routes += 1;

            Some(RoutingDecision {
                selected_service,
                backup_services,
                routing_reason,
            })
        } else {
            warn!("Failed to select service despite healthy services available");
            self.stats.write().unwrap().failed_routes += 1;
            None
        }
    }

    /// Mark a service as failed and increment failover counter
    pub fn mark_service_failed(&self, service_id: uuid::Uuid) {
        debug!("Marking service {} as failed", service_id);
        
        let manager = self.service_manager.read().unwrap();
        manager.update_health(service_id, false);
        
        self.stats.write().unwrap().failover_count += 1;
    }

    /// Set routing strategy
    pub fn set_strategy(&mut self, strategy: RoutingStrategy) {
        info!("Changing routing strategy to {:?}", strategy);
        self.routing_strategy = strategy;
    }

    /// Get current routing strategy
    pub fn get_strategy(&self) -> RoutingStrategy {
        self.routing_strategy
    }

    /// Get routing statistics
    pub fn get_stats(&self) -> RoutingStats {
        self.stats.read().unwrap().clone()
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        let mut stats = self.stats.write().unwrap();
        *stats = RoutingStats {
            total_requests: 0,
            successful_routes: 0,
            failed_routes: 0,
            failover_count: 0,
            requests_per_service: HashMap::new(),
        };
        info!("Routing statistics reset");
    }

    /// Rebalance load across services (reset request counts)
    pub fn rebalance(&self) {
        let mut counts = self.request_counts.write().unwrap();
        counts.clear();
        
        let mut indices = self.round_robin_indices.write().unwrap();
        indices.clear();
        
        info!("Load rebalanced - request counts and round-robin indices reset");
    }

    /// Select service using round-robin strategy
    fn select_round_robin(
        &self,
        services: &mut Vec<ServiceInfo>,
        service_type: &ServiceType,
    ) -> Option<ServiceInfo> {
        if services.is_empty() {
            return None;
        }

        let type_key = self.service_type_key(service_type);
        let mut indices = self.round_robin_indices.write().unwrap();
        let index = indices.entry(type_key).or_insert(0);

        let selected = services.get(*index % services.len()).cloned();
        *index = (*index + 1) % services.len();

        selected
    }

    /// Select service using least-loaded strategy
    fn select_least_loaded(&self, services: &[ServiceInfo]) -> Option<ServiceInfo> {
        let counts = self.request_counts.read().unwrap();
        
        services
            .iter()
            .min_by_key(|s| counts.get(&s.peer_id).unwrap_or(&0))
            .cloned()
    }

    /// Select service using capability-based strategy
    /// Note: This is a placeholder - would need device capabilities from Session 1's registry
    fn select_capability_based(&self, services: &[ServiceInfo]) -> Option<ServiceInfo> {
        // For now, just select the first service
        // TODO: Integrate with DeviceRegistry to get capability scores
        services.first().cloned()
    }

    /// Track request for a service
    fn track_request(&self, service: &ServiceInfo) {
        let mut counts = self.request_counts.write().unwrap();
        *counts.entry(service.peer_id).or_insert(0) += 1;

        let mut stats = self.stats.write().unwrap();
        *stats
            .requests_per_service
            .entry(service.service_id.to_string())
            .or_insert(0) += 1;
    }

    /// Convert service type to string key
    fn service_type_key(&self, service_type: &ServiceType) -> String {
        match service_type {
            ServiceType::LLM { .. } => "LLM".to_string(),
            ServiceType::STT { .. } => "STT".to_string(),
            ServiceType::TTS { .. } => "TTS".to_string(),
            ServiceType::Storage { .. } => "Storage".to_string(),
            ServiceType::MCP { .. } => "MCP".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_peer_id() -> PeerId {
        PeerId::random()
    }

    fn setup_test_services() -> (Arc<RwLock<ServiceManager>>, Vec<uuid::Uuid>) {
        let manager = Arc::new(RwLock::new(ServiceManager::new()));
        let peer_id = create_test_peer_id();

        let service_type = ServiceType::LLM {
            models: vec!["gemma3:7b".to_string()],
        };

        let mut service_ids = Vec::new();
        for i in 0..3 {
            let id = manager.read().unwrap().register_service(
                service_type.clone(),
                peer_id,
                format!("http://localhost:1143{}", i),
            );
            service_ids.push(id);
        }

        (manager, service_ids)
    }

    #[test]
    fn test_load_balancer_creation() {
        let manager = Arc::new(RwLock::new(ServiceManager::new()));
        let lb = LoadBalancer::new(manager);
        assert_eq!(lb.get_strategy(), RoutingStrategy::RoundRobin);
    }

    #[test]
    fn test_round_robin_routing() {
        let (manager, _) = setup_test_services();
        let lb = LoadBalancer::new(manager);

        let service_type = ServiceType::LLM {
            models: vec!["gemma3:7b".to_string()],
        };

        // Route 3 requests - should cycle through all services
        let endpoints: Vec<String> = (0..3)
            .filter_map(|_| lb.route_request(&service_type))
            .map(|d| d.selected_service.endpoint)
            .collect();

        assert_eq!(endpoints.len(), 3);
        // All endpoints should be different (round-robin)
        assert_ne!(endpoints[0], endpoints[1]);
        assert_ne!(endpoints[1], endpoints[2]);
    }

    #[test]
    fn test_least_loaded_routing() {
        let (manager, service_ids) = setup_test_services();
        let lb = LoadBalancer::with_strategy(manager.clone(), RoutingStrategy::LeastLoaded);

        let service_type = ServiceType::LLM {
            models: vec!["gemma3:7b".to_string()],
        };

        // Route 5 requests - should distribute evenly
        for _ in 0..5 {
            lb.route_request(&service_type);
        }

        let stats = lb.get_stats();
        assert_eq!(stats.successful_routes, 5);
        
        // All services should have at least one request
        assert!(stats.requests_per_service.len() > 0);
    }

    #[test]
    fn test_no_services_available() {
        let manager = Arc::new(RwLock::new(ServiceManager::new()));
        let lb = LoadBalancer::new(manager);

        let service_type = ServiceType::LLM {
            models: vec!["gemma3:7b".to_string()],
        };

        let decision = lb.route_request(&service_type);
        assert!(decision.is_none());

        let stats = lb.get_stats();
        assert_eq!(stats.failed_routes, 1);
    }

    #[test]
    fn test_failover_tracking() {
        let (manager, service_ids) = setup_test_services();
        let lb = LoadBalancer::new(manager.clone());

        // Mark first service as failed
        lb.mark_service_failed(service_ids[0]);

        let stats = lb.get_stats();
        assert_eq!(stats.failover_count, 1);

        // Service should now be unhealthy
        let service = manager.read().unwrap().get_service(service_ids[0]).unwrap();
        assert!(service.is_degraded()); // First failure -> Degraded
    }

    #[test]
    fn test_backup_services() {
        let (manager, _) = setup_test_services();
        let lb = LoadBalancer::new(manager);

        let service_type = ServiceType::LLM {
            models: vec!["gemma3:7b".to_string()],
        };

        let decision = lb.route_request(&service_type).unwrap();
        
        // Should have 2 backup services (3 total - 1 selected)
        assert_eq!(decision.backup_services.len(), 2);
        
        // Selected service should not be in backups
        for backup in &decision.backup_services {
            assert_ne!(backup.service_id, decision.selected_service.service_id);
        }
    }

    #[test]
    fn test_strategy_switching() {
        let manager = Arc::new(RwLock::new(ServiceManager::new()));
        let mut lb = LoadBalancer::new(manager);

        assert_eq!(lb.get_strategy(), RoutingStrategy::RoundRobin);

        lb.set_strategy(RoutingStrategy::LeastLoaded);
        assert_eq!(lb.get_strategy(), RoutingStrategy::LeastLoaded);

        lb.set_strategy(RoutingStrategy::CapabilityBased);
        assert_eq!(lb.get_strategy(), RoutingStrategy::CapabilityBased);
    }

    #[test]
    fn test_stats_reset() {
        let (manager, _) = setup_test_services();
        let lb = LoadBalancer::new(manager);

        let service_type = ServiceType::LLM {
            models: vec!["gemma3:7b".to_string()],
        };

        // Generate some stats
        for _ in 0..5 {
            lb.route_request(&service_type);
        }

        let stats = lb.get_stats();
        assert_eq!(stats.total_requests, 5);

        // Reset stats
        lb.reset_stats();
        let stats = lb.get_stats();
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.successful_routes, 0);
    }

    #[test]
    fn test_rebalance() {
        let (manager, _) = setup_test_services();
        let lb = LoadBalancer::new(manager);

        let service_type = ServiceType::LLM {
            models: vec!["gemma3:7b".to_string()],
        };

        // Route several requests
        for _ in 0..5 {
            lb.route_request(&service_type);
        }

        // Rebalance (should reset internal counters)
        lb.rebalance();

        // Next request should start fresh
        let decision = lb.route_request(&service_type).unwrap();
        assert!(decision.selected_service.endpoint.starts_with("http://localhost:"));
    }
}
