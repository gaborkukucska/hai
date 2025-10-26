//! <!-- # START OF FILE hainet-core/src/networking/coordinator.rs -->
use super::{discovery::DiscoveryBehaviour, peer::PeerBehaviour, CombinedBehaviour, CombinedEvent};
use libp2p::{
    futures::StreamExt,
    identity,
    swarm::SwarmEvent,
    PeerId, Swarm,
};
use std::error::Error;
use std::time::Duration;
use tracing::{info, warn};

/// The network coordinator.
pub struct Coordinator {
    swarm: Swarm<CombinedBehaviour>,
}

impl Coordinator {
    /// Creates a new `Coordinator`.
    pub async fn new() -> Result<Self, Box<dyn Error>> {
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        info!("Local Peer ID: {}", local_peer_id);

        let behaviour = CombinedBehaviour {
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
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
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
                SwarmEvent::Behaviour(CombinedEvent::Discovery(
                    libp2p::mdns::Event::Discovered(peers),
                )) => {
                    for (peer_id, multiaddr) in peers {
                        info!("Discovered peer: {} with address {}", peer_id, multiaddr);
                        if let Err(e) = self.swarm.dial(multiaddr) {
                            warn!("Failed to dial peer {}: {}", peer_id, e);
                        }
                    }
                }
                SwarmEvent::Behaviour(CombinedEvent::Peer(
                    crate::networking::peer::PeerEvent::Ping(event),
                )) => match event.result {
                    Ok(rtt) => {
                        info!(
                            "Ping successful to peer: {} with RTT: {:?}",
                            event.peer, rtt
                        );
                    }
                    Err(e) => {
                        warn!("Ping failed to peer: {}: {}", event.peer, e);
                    }
                },
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!("Listening on {}", address);
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
