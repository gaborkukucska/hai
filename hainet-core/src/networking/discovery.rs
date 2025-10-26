//! <!-- # START OF FILE hainet-core/src/networking/discovery.rs -->
use libp2p::mdns;
use libp2p_swarm_derive::NetworkBehaviour;

/// A custom `NetworkBehaviour` that combines mDNS for peer discovery.
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "mdns::Event")]
pub struct DiscoveryBehaviour {
    pub mdns: mdns::tokio::Behaviour,
}
