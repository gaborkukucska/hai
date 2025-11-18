//! <!-- # START OF FILE hainet-core/src/networking/coordinator.rs -->
use super::{discovery::DiscoveryBehaviour, peer::PeerBehaviour};
use libp2p::{
    futures::StreamExt,
    identity, noise,
    swarm::SwarmEvent, yamux, PeerId, Swarm, Transport,
};
use libp2p_swarm_derive::NetworkBehaviour;
use std::error::Error;
use std::time::Duration;
use tracing::{info, warn};

/// The combined `NetworkBehaviour` for the coordinator.
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "CoordinatorEvent")]
pub struct CoordinatorBehaviour {
    discovery: DiscoveryBehaviour,
    peer: PeerBehaviour,
}

/// Events emitted by the `CoordinatorBehaviour`.
#[derive(Debug)]
pub enum CoordinatorEvent {
    Discovery(libp2p::mdns::Event),
    Peer(super::peer::PeerEvent),
}

impl From<libp2p::mdns::Event> for CoordinatorEvent {
    fn from(event: libp2p::mdns::Event) -> Self {
        CoordinatorEvent::Discovery(event)
    }
}

impl From<super::peer::PeerEvent> for CoordinatorEvent {
    fn from(event: super::peer::PeerEvent) -> Self {
        CoordinatorEvent::Peer(event)
    }
}

/// The network coordinator.
pub struct Coordinator {
    swarm: Swarm<CoordinatorBehaviour>,
}

impl Coordinator {
    /// Creates a new `Coordinator`.
    pub async fn new() -> Result<Self, Box<dyn Error>> {
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        info!("Local Peer ID: {}", local_peer_id);

        let behaviour = CoordinatorBehaviour {
            discovery: DiscoveryBehaviour {
                mdns: libp2p::mdns::tokio::Behaviour::new(
                    libp2p::mdns::Config::default(),
                    local_peer_id,
                )?,
            },
            peer: PeerBehaviour {
                ping: libp2p::ping::Behaviour::new(libp2p::ping::Config::new()),
            },
        };

        let mut swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                Default::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|_key| behaviour)?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        Ok(Self { swarm })
    }

    /// Runs the coordinator event loop.
    pub async fn run(&mut self) {
        loop {
            match self.swarm.select_next_some().await {
                SwarmEvent::Behaviour(CoordinatorEvent::Discovery(
                    libp2p::mdns::Event::Discovered(peers),
                )) => {
                    for (peer_id, multiaddr) in peers {
                        info!("Discovered peer: {} with address {}", peer_id, multiaddr);
                        self.swarm.dial(multiaddr).unwrap();
                    }
                }
                SwarmEvent::Behaviour(CoordinatorEvent::Peer(super::peer::PeerEvent::Ping(
                    event,
                ))) => {
                    info!("Ping event: {:?}", event);
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    info!("Connection established with: {}", peer_id);
                }
                SwarmEvent::ConnectionClosed { peer_id, .. } => {
                    warn!("Connection closed with: {}", peer_id);
                }
                _ => {}
            }
        }
    }
}
