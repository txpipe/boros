use std::collections::HashMap;
use std::time::Duration;

use pallas::network::miniprotocols::peersharing::PeerAddress;
use rand::seq::{IndexedMutRandom, IndexedRandom};
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{error, info};

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
        desired_peers: u8,
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
                .discover_peers(desired_peers)
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

    pub async fn add_peer(&self, peer_addr: &String) {
        let mut new_peer = Peer::new(peer_addr, self.network_magic);

        let connected = match timeout(Duration::from_secs(5), new_peer.init()).await {
            Ok(Ok(())) => {
                info!("Peer {} connected successfully", peer_addr);
                true
            }
            Ok(Err(e)) => {
                error!("Peer {} initialization error: {:?}", peer_addr, e);
                false
            }
            Err(_) => {
                error!("Peer {} connection timed out", peer_addr);
                false
            }
        };

        if connected {
            let mut peers = self.peers.write().await;
            peers.insert(peer_addr.clone(), Some(new_peer));
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
