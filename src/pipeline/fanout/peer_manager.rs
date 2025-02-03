use std::collections::HashMap;
use std::fmt::Error;

use pallas::network::miniprotocols::peersharing::PeerAddress;
use tracing::info;

use super::peer::Peer;

pub struct PeerManager {
    network_magic: u64,
    peers: HashMap<String, Option<Peer>>,
    discovered_peers: Vec<PeerAddress>
}

impl PeerManager {
    pub fn new(network_magic: u64, peer_addresses: Vec<String>) -> Self {
        PeerManager {
            network_magic,
            peers: peer_addresses
                .into_iter()
                .map(|peer_addr| (peer_addr, None))
                .collect(),
            discovered_peers: vec![]
        }
    }

    pub async fn init(&mut self) -> Result<(), Error> {
        for (peer_addr, peer) in self.peers.iter_mut() {
            let mut txsubmitpeer = Peer::new(peer_addr, self.network_magic);
            txsubmitpeer.init().await.unwrap();
            if let Ok(new_peers) = txsubmitpeer.discover_peers().await {
                info!("Discovered peers: {:?}", new_peers);
                self.discovered_peers.extend(new_peers);
            }
            *peer = Some(txsubmitpeer);
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
