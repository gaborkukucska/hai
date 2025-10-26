//! <!-- # START OF FILE hainet-core/src/networking/peer.rs -->
use libp2p::ping;
use libp2p_swarm_derive::NetworkBehaviour;

/// A custom `NetworkBehaviour` that combines Ping for liveness checks.
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "PeerEvent")]
pub struct PeerBehaviour {
    pub ping: ping::Behaviour,
}

/// Events emitted by the `PeerBehaviour`.
#[derive(Debug)]
pub enum PeerEvent {
    Ping(ping::Event),
}

impl From<ping::Event> for PeerEvent {
    fn from(event: ping::Event) -> Self {
        PeerEvent::Ping(event)
    }
}
