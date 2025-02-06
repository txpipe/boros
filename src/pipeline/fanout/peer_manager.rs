// use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::Error;
use std::time::Duration;

use pallas::network::miniprotocols::peersharing::PeerAddress;
use rand::seq::{IndexedMutRandom, IndexedRandom, SliceRandom};
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::info;

use super::peer::Peer;

pub struct PeerManager {
    network_magic: u64,
    peers: RwLock<HashMap<String, Option<Peer>>>,
}

impl PeerManager {
    pub fn new(network_magic: u64, peer_addresses: Vec<String>) -> Self {
        PeerManager {
            network_magic,
            peers: RwLock::new(
                peer_addresses
                    .into_iter()
                    .map(|peer_addr| (peer_addr, None))
                    .collect(),
            ),
        }
    }

    pub async fn init(&mut self) -> Result<(), Error> {
        let mut peers = self.peers.write().await;
        for (peer_addr, peer) in peers.iter_mut() {
            let mut new_peer = Peer::new(peer_addr, self.network_magic);
            new_peer.init().await.unwrap();
            *peer = Some(new_peer);
        }
        drop(peers);

        Ok(())
    }

    pub async fn pick_peer_rand(&self, desired_peers: u8) -> Result<Option<String>, Error> {
        // Acquire a write lock so we can obtain mutable references.
        let mut peers = self.peers.write().await;
        let mut rng = rand::rng();

        // Build a vector of mutable references to candidate peers.
        let mut candidates: Vec<&mut Peer> = peers
            .iter_mut()
            .filter_map(|(_, peer_opt)| {
                if let Some(peer) = peer_opt {
                    if peer.is_alive && peer.is_peer_sharing_enabled {
                        Some(peer)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        // Use `choose_mut` on the mutable slice of candidates.
        // This returns an Option<&mut &mut Peer>.
        if let Some(peer_ref) = candidates.as_mut_slice().choose_mut(&mut rng) {
            // Use `*peer_ref` to dereference the extra layer.
            // This gives you a `&mut Peer` which can be used to call mutable methods.
            info!("Picked random peer: {}", peer_ref.peer_addr);

            let sub_peers = (*peer_ref).discover_peers(desired_peers).await;
            match sub_peers {
                Ok(sub_peers) => {
                    let sub_peers: Vec<String> = sub_peers
                        .into_iter()
                        // @todo: Extend support to include IPv6 addresses. For now we only care about IPv4 peers.
                        .filter(|addr| matches!(addr, PeerAddress::V4(_, _)))
                        .map(|addr| match addr {
                            PeerAddress::V4(ip, port) => format!("{}:{}", ip, port),
                            _ => unreachable!(),
                        })
                        .collect();
                    info!("Discovered peers: {:?}", sub_peers);
                    // For picking a random discovered peer, the regular `choose` works since we
                    // don't need to mutate the string.
                    return Ok(sub_peers.choose(&mut rng).cloned());
                }
                Err(e) => {
                    info!("Failed to discover peers: {:?}", e);
                }
            }
        }

        Ok(None)
    }

    pub async fn add_peer(&self, peer_addr: &String) {
        let mut new_peer = Peer::new(peer_addr, self.network_magic);

        let connected = match timeout(Duration::from_secs(5), new_peer.init()).await {
            Ok(Ok(())) => true,
            Ok(Err(e)) => {
                info!("Peer initialization error for {}: {:?}", peer_addr, e);
                false
            }
            Err(_) => {
                info!("Peer connection timed out: {}", peer_addr);
                false
            }
        };

        let mut peers = self.peers.write().await;

        if connected {
            peers.insert(peer_addr.clone(), Some(new_peer));
            info!("New peer connected: {}", peer_addr);
        } else {
            info!("Failed to connect new peer: {}", peer_addr);
        }
    }

    pub async fn connected_peers_count(&self) -> usize {
        let peers = self.peers.read().await;
        peers
            .iter()
            .filter(|(_, peer)| match *peer {
                Some(peer) => peer.is_alive,
                None => false,
            })
            .count()
    }

    pub async fn add_tx(&self, tx: Vec<u8>) {
        let peers = self.peers.read().await;
        for (_, peer) in peers.iter() {
            if let Some(peer) = peer {
                peer.add_tx(tx.clone()).await;
            }
        }
    }
}
