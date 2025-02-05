use std::collections::HashMap;
use std::fmt::Error;

use tracing::info;

use super::peer::Peer;

pub struct PeerManagerInitConfig {
    pub desired_peers: u8,
}

pub struct PeerManager {
    network_magic: u64,
    peers: HashMap<String, Option<Peer>>,
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
        // Initialize Bootstrap Peers and collect discovered peers
        for (peer_addr, peer) in self.peers.iter_mut() {
            let mut new_peer = Peer::new(peer_addr, self.network_magic);
            new_peer.init().await.unwrap();
            
            // // If peer sharing is enabled, discover peers
            // if new_peer.get_is_peer_sharing_enabled_field() {
                
            //     let discovered_peers = new_peer.discover_peers(config.desired_peers).await.map_err(
            //         |e| {
            //             info!("Error discovering peers: {:?}", e);
            //             e
            //         },
            //     )?;
            // }

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
