use std::collections::HashMap;
use std::fmt::Error;

use tracing::info;

use super::peer::Peer;

pub struct PeerManagerInitConfig {
    pub desired_peers: u8
}

pub struct PeerManager {
    network_magic: u64,
    peers: HashMap<String, Option<Peer>>
}

impl PeerManager {
    pub fn new(network_magic: u64, peer_addresses: Vec<String>) -> Self {
        PeerManager {
            network_magic,
            peers: peer_addresses
                .into_iter()
                .map(|peer_addr| (peer_addr, None))
                .collect(),
        }
    }

    pub async fn init(&mut self, config: PeerManagerInitConfig) -> Result<(), Error> {
        let mut collected_discovered_peers = vec![];
        
        // Initialize Bootstrap Peers and collect discovered peers
        for (peer_addr, peer) in self.peers.iter_mut() {

            let mut new_peer = Peer::new(peer_addr, self.network_magic);
            new_peer.init().await.unwrap();

            if let Ok(new_peers) = new_peer.discover_peers(config.desired_peers).await {
                info!("Discovered peers: {:?}", new_peers);
                // Merge the discovered peers into the collected list
                collected_discovered_peers.extend(new_peers);
            }

            *peer = Some(new_peer);

        }

        Ok(())
    }

    pub async fn add_tx(&self, tx: Vec<u8>) {
        for (_, peer) in self.peers.iter() {
            if let Some(peer) = peer {
                peer.add_tx(tx.clone()).await;
            }
        }
    }
}
