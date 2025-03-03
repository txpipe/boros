use std::collections::HashMap;

use thiserror::Error;
use tokio::sync::{broadcast::Receiver, RwLock};
use tracing::error;

use super::peer::{Peer, PeerError};

#[derive(Debug, Error)]
pub enum PeerManagerError {
    #[error("Peer initialization failed: {0}")]
    PeerInitialization(PeerError),
}

pub struct PeerManager {
    network_magic: u64,
    peers: RwLock<HashMap<String, Option<Peer>>>
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

    pub async fn init(&mut self, receiver: &mut Receiver<Vec<u8>>) -> Result<(), PeerManagerError> {
        let mut peers = self.peers.write().await;
        for (peer_addr, peer) in peers.iter_mut() {
            let mut new_peer = Peer::new(peer_addr, self.network_magic, receiver.resubscribe());

            new_peer
                .init()
                .await
                .map_err(PeerManagerError::PeerInitialization)?;
            
            *peer = Some(new_peer);
        }

        Ok(())
    }
}
