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
