//! <!-- # START OF FILE hainet-core/src/networking/mod.rs -->
pub mod coordinator;
pub mod discovery;
pub mod peer;

use crate::networking::{
    discovery::DiscoveryBehaviour,
    peer::{PeerBehaviour, PeerEvent},
};
use libp2p::mdns;
use libp2p_swarm_derive::NetworkBehaviour;

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "CombinedEvent")]
pub struct CombinedBehaviour {
    pub discovery: DiscoveryBehaviour,
    pub peer: PeerBehaviour,
}

#[derive(Debug)]
pub enum CombinedEvent {
    Discovery(mdns::Event),
    Peer(PeerEvent),
}

impl From<mdns::Event> for CombinedEvent {
    fn from(event: mdns::Event) -> Self {
        CombinedEvent::Discovery(event)
    }
}

impl From<PeerEvent> for CombinedEvent {
    fn from(event: PeerEvent) -> Self {
        CombinedEvent::Peer(event)
    }
}
