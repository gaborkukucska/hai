//! <!-- # START OF FILE hainet-core/src/networking/mod.rs -->
pub mod coordinator;
pub mod discovery;
pub mod peer;

// Phase 9A Session 1: Peer Discovery & Device Registry
pub mod peer_discovery;
pub mod registry;
pub mod heartbeat;

// Phase 9A Session 2: Master-Slave Coordination & Role Assignment
pub mod mesh_coordinator;

// Phase 9A Session 3: Service Distribution & Load Balancing
pub mod service_manager;
pub mod load_balancer;
pub mod service_registry;

// Phase 9A Session 4: Mesh Communication Protocol
pub mod mesh_message;
pub mod rpc_client;
pub mod rpc_server;
pub mod multiplexer;

// Phase 9A Session 5: Service Auto-Discovery & Integration
pub mod service_detector;
