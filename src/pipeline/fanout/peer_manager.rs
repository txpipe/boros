// use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fmt::Error;
use std::time::Duration;

use pallas::network::miniprotocols::peersharing::PeerAddress;
use rand::seq::{IndexedRandom, SliceRandom};
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::info;

use crate::pipeline::fanout::peer;

use super::peer::Peer;

pub struct PeerManager {
    network_magic: u64,
    peers: RwLock<HashMap<String, Option<Peer>>>,
    desired_peers: u8,
}

impl PeerManager {
    pub fn new(network_magic: u64, peer_addresses: Vec<String>, desired_peers: u8) -> Self {
        PeerManager {
            network_magic,
            peers: RwLock::new(
                peer_addresses
                    .into_iter()
                    .map(|peer_addr| (peer_addr, None))
                    .collect(),
            ),
            desired_peers,
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

    pub async fn pick_peer_rand(&self, exclude: &HashSet<String>) -> Option<String> {
        let peers = self.peers.read().await;

        let candidates: Vec<String> = peers
            .iter()
            .filter_map(|(addr, peer_opt)| {
                if let Some(peer) = peer_opt {
                    if peer.is_alive && peer.is_peer_sharing_enabled && !exclude.contains(addr) {
                        Some(addr.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        if candidates.is_empty() {
            None
        } else {
            candidates.choose(&mut rand::rng()).cloned()
        }
    }

    pub async fn discover_peers(&self, peer_addr: &str) {
        let connected_count = self.connected_peers_count().await;

        let maybe_peer = {
            let mut peers = self.peers.write().await;
            peers.get_mut(peer_addr).and_then(|peer| peer.take())
        };

        if let Some(mut peer) = maybe_peer {
            info!("Discovering peers from {}", peer_addr);
            let discovered = match peer.discover_peers(self.desired_peers).await {
                Ok(d) => d,
                Err(_) => vec![],
            };

            let mut discovered_addresses: Vec<String> = discovered
                .into_iter()
                .filter(|addr| matches!(addr, PeerAddress::V4(_, _)))
                .map(|addr| match addr {
                    PeerAddress::V4(ip, port) => format!("{}:{}", ip, port),
                    _ => unreachable!(),
                })
                .collect();

            info!(
                "Peer {} discovered: {:?} before shuffle",
                peer_addr, discovered_addresses
            );
            discovered_addresses.shuffle(&mut rand::rng());
            info!(
                "Peer {} discovered: {:?} after shuffle",
                peer_addr, discovered_addresses
            );

            // Determine how many peers we need to connect to.
            let needed = if self.desired_peers as usize > connected_count {
                self.desired_peers as usize - connected_count
            } else {
                0
            };

            info!(
                "Peer {} discovered {} addresses; need {}",
                peer_addr,
                discovered_addresses.len(),
                needed
            );

            for new_addr in discovered_addresses.into_iter().take(needed) {
                {
                    let peers = self.peers.read().await;
                    if peers.contains_key(&new_addr) {
                        continue;
                    }
                }

                info!("Adding new peer: {}", new_addr);
                let mut new_peer = peer::Peer::new(&new_addr, self.network_magic);
                let connected = match timeout(Duration::from_secs(5), new_peer.init()).await {
                    Ok(Ok(())) => true,
                    Ok(Err(e)) => {
                        info!("Peer initialization error for {}: {:?}", new_addr, e);
                        false
                    }
                    Err(_) => {
                        info!("Peer connection timed out: {}", new_addr);
                        false
                    }
                };

                let mut peers = self.peers.write().await;

                if connected {
                    peers.insert(new_addr.clone(), Some(new_peer));
                    info!("New peer connected: {}", new_addr);
                } else {
                    peers.insert(new_addr.clone(), None);
                    info!("Failed to connect new peer: {}", new_addr);
                }
            }

            let mut peers = self.peers.write().await;
            peers.insert(peer_addr.to_string(), Some(peer));
        } else {
            tracing::info!("Peer {} not found in manager", peer_addr);
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
