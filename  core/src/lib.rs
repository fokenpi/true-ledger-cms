pub mod ledger;
pub mod sync;

pub use ledger::{Account, AccountType, Posting, Transaction, Ledger};
pub use sync::{SyncDoc, SyncableLedger, SyncError};

use libp2p::{
    identity, noise, tcp, yamux, PeerId, Swarm, SwarmEvent,
    Transport, NetworkBehaviour, gossipsub, mdns,
};
use tokio::sync::mpsc;
use std::time::Duration;

#[derive(NetworkBehaviour)]
struct LedgerBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

pub struct SyncClient {
    swarm: Swarm<LedgerBehaviour>,
    event_rx: mpsc::UnboundedReceiver<SwarmEvent<LedgerBehaviour>>,
}

impl SyncClient {
    pub async fn new() -> Self {
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        let transport = tcp::tokio::Transport::default()
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&local_key).unwrap())
            .multiplex(yamux::Config::default())
            .boxed();

        let mut mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id).unwrap();
        mdns.set_ttl(Duration::from_secs(30));

        let gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(local_key),
            gossipsub::Config::default(),
        ).unwrap();

        let behaviour = LedgerBehaviour { gossipsub, mdns };
        let mut swarm = Swarm::new(transport, behaviour, local_peer_id);

        let topic = gossipsub::IdentTopic::new("true-ledger-sync");
        swarm.behaviour_mut().gossipsub.subscribe(&topic).unwrap();

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            loop {
                match swarm.select_next_some().await {
                    SwarmEvent::Behaviour(event) => event_tx.send(SwarmEvent::Behaviour(event)).unwrap(),
                    _ => {}
                }
            }
        });

        Self { swarm, event_rx }
    }

    pub async fn sync_with_peer(&mut self, data: Vec<u8>) {
        let topic = gossipsub::IdentTopic::new("true-ledger-sync");
        self.swarm.behaviour_mut().gossipsub.publish(topic, data).unwrap();
    }
}