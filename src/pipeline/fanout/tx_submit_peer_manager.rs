use std::collections::HashMap;
use std::fmt::Error;

use super::tx_submit_peer::TxSubmitPeer;

pub struct TxSubmitPeerManager {
    network_magic: u64,
    peers: HashMap<String, Option<TxSubmitPeer>>,
}

impl TxSubmitPeerManager {
    pub fn new(network_magic: u64, peer_addresses: Vec<String>) -> Self {
        TxSubmitPeerManager {
            network_magic,
            peers: peer_addresses
                .into_iter()
                .map(|peer_addr| (peer_addr, None))
                .collect(),
        }
    }

    pub async fn init(&mut self) -> Result<(), Error> {
        for (peer_addr, peer) in self.peers.iter_mut() {
            let mut txsubmitpeer = TxSubmitPeer::new(peer_addr, self.network_magic);
            txsubmitpeer.init().await.unwrap();
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
