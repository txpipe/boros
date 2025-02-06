// use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::Error;
use std::time::Duration;

use pallas::network::miniprotocols::peersharing::PeerAddress;
use rand::seq::{IndexedRandom, SliceRandom};
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::info;

use super::peer::Peer;

pub struct PeerManager {
    network_magic: u64,
    peers: RwLock<HashMap<String, Option<Peer>>>,
    peers_per_request: u8,
}

impl PeerManager {
    pub fn new(network_magic: u64, peer_addresses: Vec<String>, peers_per_request: u8) -> Self {
        PeerManager {
            network_magic,
            peers: RwLock::new(
                peer_addresses
                    .into_iter()
                    .map(|peer_addr| (peer_addr, None))
                    .collect(),
            ),
            peers_per_request,
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

    pub async fn pick_peer_rand(&self) -> Result<Option<String>, Error> {
        let peers = self.peers.read().await;

        let candidates: Vec<String> = peers
            .iter()
            .filter_map(|(addr, peer_opt)| {
                if let Some(peer) = peer_opt {
                    if peer.is_alive && peer.is_peer_sharing_enabled {
                        Some(addr.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        let random_peer = candidates.choose(&mut rand::rng()).cloned();

        if let Some(peer_addr) = random_peer {
            info!("Picked random peer: {}", peer_addr);

            let random_discovered_peer = self.discover_peers(peer_addr).await?;
            info!("Random peer discovered: {:?}", random_discovered_peer);
            return Ok(random_discovered_peer);
        }

        Ok(None)
    }

    async fn discover_peers(&self, peer_addr: String) -> Result<Option<String>, Error> {
        info!("Discovering peers from {}", peer_addr);
        let maybe_peer = {
            let mut peers = self.peers.write().await;
            peers.get_mut(&peer_addr).and_then(|peer| peer.take())
        };

        if let Some(mut peer) = maybe_peer {
            info!("Discovering peers from {}", peer_addr);
            let discovered = match peer.discover_peers(self.peers_per_request).await {
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

            let random_discovered_peer = discovered_addresses.choose(&mut rand::rng()).cloned();

            let mut peers = self.peers.write().await;
            peers.insert(peer_addr.clone(), Some(peer));

            return Ok(random_discovered_peer);
        }

        Ok(None)
    }

    pub async fn init_discovered_peer(&self, peer_addr: &String) {
        let mut new_peer = Peer::new(&peer_addr, self.network_magic);

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
