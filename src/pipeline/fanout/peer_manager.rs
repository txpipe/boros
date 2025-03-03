use std::collections::HashMap;
use std::time::Duration;

use pallas::network::miniprotocols::peersharing::PeerAddress;
use rand::seq::{IndexedMutRandom, IndexedRandom};
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{error, info, warn};

use super::peer::{Peer, PeerError};

#[derive(Debug, Error)]
pub enum PeerManagerError {
    #[error("Peer initialization failed: {0}")]
    PeerInitialization(PeerError),

    #[error("Peer discovery error: {0}")]
    PeerDiscovery(PeerError),
}

pub struct PeerManager {
    network_magic: u64,
    peers: RwLock<HashMap<String, Option<Peer>>>,
}

impl PeerManager {
    pub fn new(network_magic: u64, peer_addresses: Vec<String>) -> Self {
        let peers = peer_addresses
            .into_iter()
            .map(|peer_addr| (peer_addr, None))
            .collect();

        Self {
            network_magic,
            peers: RwLock::new(peers),
        }
    }

    pub async fn init(&mut self) -> Result<(), PeerManagerError> {
        let mut peers = self.peers.write().await;
        for (peer_addr, peer) in peers.iter_mut() {
            let mut new_peer = Peer::new(peer_addr, self.network_magic);

            new_peer.is_peer_sharing_enabled = new_peer
                .query_peer_sharing_mode()
                .await
                .map_err(PeerManagerError::PeerInitialization)?;

            new_peer
                .init()
                .await
                .map_err(PeerManagerError::PeerInitialization)?;

            *peer = Some(new_peer);
        }

        Ok(())
    }

    pub async fn pick_peer_rand(
        &self,
        peers_per_request: u8,
    ) -> Result<Option<String>, PeerManagerError> {
        let mut peers = self.peers.write().await;
        let mut rng = rand::rng();

        let mut candidates: Vec<_> = peers
            .values_mut()
            .filter_map(|slot| slot.as_mut())
            .filter(|peer| peer.is_alive && peer.is_peer_sharing_enabled)
            .collect();

        let Some(peer) = candidates.choose_mut(&mut rng) else {
            return Ok(None);
        };

        let discovered_peers = peer
            .discover_peers(peers_per_request)
            .await
            .map_err(PeerManagerError::PeerDiscovery)?;

        let peer_addrs: Vec<_> = discovered_peers
            .into_iter()
            .filter_map(|addr| match addr {
                PeerAddress::V4(ip, port) => Some(format!("{ip}:{port}")),
                _ => None,
            })
            .collect();

        Ok(peer_addrs.choose(&mut rng).cloned())
    }

    /// Checks if a peer already exists.
    pub async fn is_peer_exist(&self, peer_addr: &str) -> bool {
        let peers = self.peers.read().await;
        peers.contains_key(peer_addr)
    }

    /// Adds a new peer if it does not already exist.
    pub async fn add_peer(&self, peer_addr: &str) {
        if self.is_peer_exist(peer_addr).await {
            warn!("Peer {} already exists", peer_addr);
            return;
        }

        let timeout_duration = Duration::from_secs(5);
        let mut new_peer = Peer::new(peer_addr, self.network_magic);

        // Query the peer-sharing mode with a timeout.
        match timeout(timeout_duration, new_peer.query_peer_sharing_mode()).await {
            Ok(Ok(sharing_mode)) => {
                new_peer.is_peer_sharing_enabled = sharing_mode;
            }
            Ok(Err(e)) => {
                error!("Peer {peer_addr} sharing query error: {e:?}");
            }
            Err(_) => {
                error!("Peer {peer_addr} sharing query timed out");
            }
        }

        // Peer initialization with a timeout.
        match timeout(timeout_duration, new_peer.init()).await {
            Ok(Ok(())) => {
                info!("Peer {} connected successfully", peer_addr);
                let mut peers = self.peers.write().await;
                peers.insert(peer_addr.to_string(), Some(new_peer));
            }
            Ok(Err(e)) => {
                error!("Peer {} initialization error: {:?}", peer_addr, e);
            }
            Err(_) => {
                error!("Peer {} connection timed out", peer_addr);
            }
        }
    }

    pub async fn connected_peers_count(&self) -> usize {
        let peers = self.peers.read().await;
        peers
            .values()
            .filter_map(|maybe_peer| maybe_peer.as_ref())
            .filter(|peer| peer.is_alive)
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
