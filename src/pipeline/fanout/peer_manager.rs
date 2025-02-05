use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::Error;
use std::time::Duration;

use pallas::network::miniprotocols::peersharing::PeerAddress;
use rand::seq::IndexedRandom;
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
        // Initialize Bootstrap Peers and collect discovered peers
        let mut peers = self.peers.write().await;
        for (peer_addr, peer) in peers.iter_mut() {
            let mut new_peer = Peer::new(peer_addr, self.network_magic);
            new_peer.init().await.unwrap();
            *peer = Some(new_peer);
        }
        drop(peers);
        self.discover_peers().await;

        Ok(())
    }

    pub async fn select_peer_for_discovery(&self) -> Option<String> {
        let peers = self.peers.read().await;
        let connected_peers: Vec<String> = peers
            .iter()
            .filter_map(|(addr, peer_opt)| {
                if let Some(peer) = peer_opt {
                    if peer.is_alive {
                        Some(addr.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        if connected_peers.is_empty() {
            None
        } else {
            let mut rng = rand::rng();
            connected_peers.choose(&mut rng).cloned()
        }
    }

    pub async fn discover_peers(&mut self) {
        let mut discovered_peers: Vec<String> = vec![];
        let mut peers = self.peers.write().await;
        for (peer_addr, peer) in peers.iter_mut() {
            if let Some(peer) = peer {
                if !peer.is_peer_sharing_enabled {
                    continue;
                }
                let local_discovered_peers = peer.discover_peers(self.desired_peers).await.unwrap();
                let local_discovered_peers = local_discovered_peers
                    .into_iter()
                    // @todo: Extend support to include IPv6 addresses. For now we only care about IPv4 peers
                    .filter(|addr| matches!(addr, PeerAddress::V4(_, _)))
                    .map(|addr| match addr {
                        PeerAddress::V4(addr, port) => format!("{}:{}", addr, port),
                        _ => todo!(),
                    })
                    .collect::<Vec<String>>();
                info!(
                    "Discovered peers: {:?} from {:?}",
                    local_discovered_peers, peer_addr
                );
                discovered_peers.extend(local_discovered_peers);
            }
        }

        info!("Local Discovered peers: {:?}", discovered_peers);
        for peer_addr in discovered_peers {
            let connected_peers_count = peers
                .iter()
                .filter(|(_, peer)| match *peer {
                    Some(peer) => peer.is_alive,
                    None => false,
                })
                .count();

            info!("total peers: {}", connected_peers_count);

            if connected_peers_count >= self.desired_peers as usize {
                info!("Desired peers reached");
                break;
            }

            if let Entry::Vacant(e) = peers.entry(peer_addr) {
                let key = e.key().clone();
                info!("Adding new peer: {}", key);
                let mut new_peer = Peer::new(&key, self.network_magic);
                info!("Peer created: {}", key);

                // Wrap the init future with a timeout of 5 seconds.
                let connected = match timeout(Duration::from_secs(5), new_peer.init()).await {
                    Ok(Ok(())) => true, // The init future completed in time.
                    Ok(Err(e)) => {
                        info!("Peer initialization error for {}: {:?}", key, e);
                        false
                    },
                    Err(_) => {
                        // Timeout occurred.
                        info!("Peer connection timed out: {}", key);
                        false
                    }
                };

                if connected {
                    e.insert(Some(new_peer));
                    info!("Peer connected: {}", key);
                } else {
                    e.insert(None);
                    info!("Peer connection failure, discarding: {}", key);
                }
            }
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
