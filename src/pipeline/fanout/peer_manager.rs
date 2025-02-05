use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Error;

use pallas::network::miniprotocols::peersharing::PeerAddress;
use tracing::info;

use super::peer::Peer;

pub struct PeerManager {
    network_magic: u64,
    peers: RefCell<HashMap<String, Option<Peer>>>,
    desired_peers: u8,
}

impl PeerManager {
    pub fn new(network_magic: u64, peer_addresses: Vec<String>, desired_peers: u8) -> Self {
        PeerManager {
            network_magic,
            peers: RefCell::new(
                peer_addresses
                    .into_iter()
                    .map(|peer_addr| (peer_addr, None))
                    .collect(),
            ),
            desired_peers,
        }
    }

    pub async fn init(&mut self) -> Result<(), Error> {
        // Initialize Bootstrap Peers and collect discovered peers
        let mut peers = self.peers.borrow_mut();
        for (peer_addr, peer) in peers.iter_mut() {
            let mut new_peer = Peer::new(peer_addr, self.network_magic);
            new_peer.init().await.unwrap();

            *peer = Some(new_peer);
        }

        Ok(())
    }

    pub async fn discover_peers(&mut self) {
        let mut discovered_peers: Vec<String> = vec![];
        let mut peers = self.peers.borrow_mut();
        for (peer_addr, peer) in peers.iter_mut() {
            if let Some(peer) = peer {
                let local_discovered_peers = peer.discover_peers(self.desired_peers).await.unwrap();
                let local_discovered_peers = local_discovered_peers
                    .into_iter()
                    // @todo: Filter IP4 only temporary only support IP4
                    .filter(|addr| match addr {
                        PeerAddress::V4(_, _) => true,
                        _ => false,
                    })
                    .map(|addr| match addr {
                        PeerAddress::V4(addr, port) => format!("{}:{}", addr.to_string(), port),
                        _ => todo!(),
                    })
                    .collect::<Vec<String>>();

                info!(
                    "Discovered Peers: {:?} from {:?}",
                    local_discovered_peers, peer_addr
                );
                discovered_peers.extend(local_discovered_peers);
            }
        }
    }

    pub async fn add_tx(&self, tx: Vec<u8>) {
        let peers = self.peers.borrow();
        for (_, peer) in peers.iter() {
            if let Some(peer) = peer {
                peer.add_tx(tx.clone()).await;
            }
        }
    }
}
