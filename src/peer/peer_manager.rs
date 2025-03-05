use std::collections::HashMap;
use std::time::Duration;

use pallas::network::miniprotocols::peersharing::PeerAddress;
use rand::seq::{IndexedMutRandom, IndexedRandom};
use serde::Deserialize;
use thiserror::Error;
use tokio::sync::{broadcast::Receiver, RwLock};
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
    receiver: Receiver<Vec<u8>>,
}

impl PeerManager {
    pub fn new(
        network_magic: u64,
        peer_addresses: Vec<String>,
        receiver: Receiver<Vec<u8>>,
    ) -> Self {
        let peers = peer_addresses
            .into_iter()
            .map(|peer_addr| (peer_addr, None))
            .collect();

        Self {
            network_magic,
            peers: RwLock::new(peers),
            receiver,
        }
    }

    pub async fn init(&self) -> Result<(), PeerManagerError> {
        let mut peers = self.peers.write().await;
        for (peer_addr, peer) in peers.iter_mut() {
            let mut new_peer =
                Peer::new(peer_addr, self.network_magic, self.receiver.resubscribe());

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

        let mut candidates: Vec<&mut Peer> = peers
            .iter_mut()
            .filter_map(|(_, peer_opt)| {
                peer_opt
                    .as_mut()
                    .filter(|peer| peer.is_alive && peer.is_peer_sharing_enabled)
            })
            .collect();

        if let Some(peer_ref) = candidates.as_mut_slice().choose_mut(&mut rng) {
            let sub_peers = (*peer_ref)
                .discover_peers(peers_per_request)
                .await
                .map_err(PeerManagerError::PeerDiscovery)?;

            let sub_peers: Vec<String> = sub_peers
                .into_iter()
                .filter(|addr| matches!(addr, PeerAddress::V4(_, _)))
                .map(|addr| match addr {
                    PeerAddress::V4(ip, port) => format!("{}:{}", ip, port),
                    _ => todo!(),
                })
                .collect();

            return Ok(sub_peers.choose(&mut rng).cloned());
        }

        Ok(None)
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

        let mut new_peer = Peer::new(peer_addr, self.network_magic, self.receiver.resubscribe());
        let timeout_duration = Duration::from_secs(5);

        match timeout(timeout_duration, new_peer.init()).await {
            Ok(Ok(())) => {
                info!("Peer {} connected successfully", peer_addr);
                let mut peers = self.peers.write().await;
                // Convert the &str to a String if necessary.
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
            .iter()
            .filter(|(_, peer)| match *peer {
                Some(peer) => peer.is_alive,
                None => false,
            })
            .count()
    }
}

#[derive(Deserialize, Clone)]
pub struct PeerManagerConfig {
    pub peers: Vec<String>,
    pub desired_peer_count: u8,
    pub peers_per_request: u8,
}
