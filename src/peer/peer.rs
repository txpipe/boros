use std::sync::Arc;
use std::time::Duration;
use std::vec;

use gasket::messaging::InputPort;
use itertools::Itertools;
use pallas::crypto::hash::Hash;
use pallas::network::miniprotocols::{
    peersharing::Client as PeerSharingClient, peersharing::PeerAddress,
};
use pallas::network::miniprotocols::{
    txsubmission::Client as TxSubmitClient,
    txsubmission::{EraTxBody, EraTxId, Request},
};
use pallas::network::multiplexer::RunningPlexer;
use pallas::network::{facades::PeerClient, miniprotocols::txsubmission::TxIdAndSize};
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};
use tokio::task;
use tokio::time::timeout;
use tracing::{error, info};

use super::mempool::{self, Mempool, MempoolError};

#[derive(Debug, Error)]
pub enum PeerError {
    #[error("Peer initialization failed: {0}")]
    Initialization(String),

    #[error("Peer discovery failed: {0}")]
    PeerDiscovery(String),

    #[error("Tx Submission failed: {0}")]
    TxSubmission(String),

    #[error("Mempool Error: {0}")]
    Mempool(#[from] MempoolError),
}

pub struct Peer {
    mempool: Arc<Mutex<Mempool>>,
    plexer_client: Arc<Mutex<Option<RunningPlexer>>>,
    tx_submit_client: Arc<Mutex<Option<TxSubmitClient>>>,
    peer_sharing_client: Arc<Mutex<Option<PeerSharingClient>>>,
    network_magic: u64,
    pub input: Arc<RwLock<InputPort<Vec<u8>>>>,
    pub peer_addr: String,
    pub is_peer_sharing_enabled: bool,
    pub is_alive: bool,
}

impl Peer {
    pub fn new(peer_addr: &str, network_magic: u64) -> Self {
        Self {
            mempool: Arc::new(Mutex::new(Mempool::new())),
            plexer_client: Arc::new(Mutex::new(None)),
            tx_submit_client: Arc::new(Mutex::new(None)),
            peer_sharing_client: Arc::new(Mutex::new(None)),
            peer_addr: peer_addr.to_string(),
            network_magic,
            input: Default::default(),
            is_peer_sharing_enabled: false,
            is_alive: false,
        }
    }

    pub async fn init(&mut self) -> Result<(), PeerError> {
        let client = PeerClient::connect(&self.peer_addr, self.network_magic)
            .await
            .map_err(|e| {
                PeerError::Initialization(format!("Failed to connect to peer: {:?}", e))
            })?;

        let mut tx_submit_client = client.txsubmission;
        let peer_sharing_client = client.peersharing;
        let plexer_client = client.plexer;

        tx_submit_client.send_init().await.map_err(|e| {
            PeerError::Initialization(format!("Failed to send init message to peer: {:?}", e))
        })?;

        self.tx_submit_client = Arc::new(Mutex::new(Some(tx_submit_client)));
        self.peer_sharing_client = Arc::new(Mutex::new(Some(peer_sharing_client)));
        self.plexer_client = Arc::new(Mutex::new(Some(plexer_client)));

        self.is_alive = true;
        info!(peer=%self.peer_addr, "Peer initialized");

        self.start_background_task();

        Ok(())
    }

    pub async fn discover_peers(
        &mut self,
        desired_peers: u8,
    ) -> Result<Vec<PeerAddress>, PeerError> {
        let mut client_guard = self.peer_sharing_client.lock().await;
        let peer_sharing_client = match client_guard.as_mut() {
            Some(c) => c,
            None => {
                return Err(PeerError::PeerDiscovery(
                    "Peer sharing client not available".to_string(),
                ));
            }
        };

        peer_sharing_client
            .send_share_request(desired_peers)
            .await
            .map_err(|e| {
                PeerError::PeerDiscovery(format!("Failed to send share request: {:?}", e))
            })?;

        let mut discovered = vec![];
        if let Ok(peers) = peer_sharing_client
            .recv_peer_addresses()
            .await
            .map_err(|e| {
                PeerError::PeerDiscovery(format!("Failed to receive peer addresses: {:?}", e))
            })
        {
            discovered.extend(peers);
        }

        Ok(discovered)
    }

    pub async fn query_peer_sharing_mode(&self) -> Result<bool, PeerError> {
        let version_table = PeerClient::handshake_query(&self.peer_addr, self.network_magic)
            .await
            .map_err(|e| {
                PeerError::Initialization(format!("Failed to query peer sharing mode: {:?}", e))
            })?;

        let version_data = version_table
            .values
            .iter()
            .max_by_key(|(version, _)| *version)
            .map(|(_, data)| data);

        if let Some(data) = version_data {
            if Some(1) == data.peer_sharing {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn start_background_task(&self) {
        let plexer_client = Arc::clone(&self.plexer_client);
        let tx_submit_client = Arc::clone(&self.tx_submit_client);
        let mempool_arc = Arc::clone(&self.mempool);
        let peer_addr = self.peer_addr.clone();
        let receiver = Arc::clone(&self.input);

        task::spawn(async move {
            loop {
                // Wait for the next request
                let next_req = {
                    let mut client_guard = tx_submit_client.lock().await;
                    let tx_submit_client_ref = match client_guard.as_mut() {
                        Some(c) => c,
                        None => {
                            error!(peer=%peer_addr, "No client available; breaking");
                            break;
                        }
                    };

                    info!(peer=%peer_addr, "Waiting for next request");
                    tx_submit_client_ref.next_request().await
                };

                let request = match next_req {
                    Ok(r) => r,
                    Err(e) => {
                        error!(peer=%peer_addr, error=?e, "Error reading request; breaking loop");
                        break;
                    }
                };

                // Process the consumer's request
                match request {
                    Request::TxIds(ack, req) => {
                        info!(peer=%peer_addr, "Received TX IDs Blocking request: ack={}, req={}", ack, req);
                        let mut client_guard = tx_submit_client.lock().await;
                        let tx_submit_client_ref = match client_guard.as_mut() {
                            Some(c) => c,
                            None => {
                                error!(peer=%peer_addr, "No client available; breaking");
                                break;
                            }
                        };

                        let mempool_guard = mempool_arc.lock().await;
                        mempool_guard.acknowledge(ack.into());

                        let txs = if let Ok(txs) =
                            Self::collect_transactions(&mempool_guard, &receiver, req.into()).await
                        {
                            txs
                        } else {
                            vec![]
                        };

                        info!("Requested {}; received {}", req, txs.len());

                        if let Err(err) = Self::propagate_txs(tx_submit_client_ref, txs).await {
                            error!(peer=%peer_addr, error=?err, "Error propagating TXs");
                        }
                    }
                    Request::TxIdsNonBlocking(ack, req) => {
                        info!(peer=%peer_addr, "Received TX IDs Non-Blocking request: ack={}, req={}", ack, req);
                        let mempool_guard = mempool_arc.lock().await;
                        mempool_guard.acknowledge(ack.into());

                        let timeout_duration = Duration::from_secs(1);
                        let txs = match timeout(
                            timeout_duration,
                            Self::collect_transactions(&mempool_guard, &receiver, req.into()),
                        )
                        .await
                        {
                            Ok(Ok(txs)) => txs,
                            Ok(Err(e)) => {
                                error!(peer=%peer_addr, error=?e, "Failed to collect transactions");
                                vec![]
                            }
                            Err(_) => vec![],
                        };

                        let mut client_guard = tx_submit_client.lock().await;
                        let tx_submit_client_ref = match client_guard.as_mut() {
                            Some(c) => c,
                            None => {
                                error!(peer=%peer_addr, "No client available; breaking");
                                break;
                            }
                        };

                        if let Err(err) = Self::propagate_txs(tx_submit_client_ref, txs).await {
                            error!(peer=%peer_addr, error=?err, "Error propagating TXs");
                        }
                    }
                    Request::Txs(ids) => {
                        let ids: Vec<_> = ids.iter().map(|x| (x.0, x.1.clone())).collect();
                        info!(peer=%peer_addr, "Received TX Body download request: {:?}", ids.iter().map(|x| hex::encode(x.1.as_slice())).collect::<Vec<String>>());

                        let to_send = {
                            let mempool_guard = mempool_arc.lock().await;
                            ids.iter()
                                .filter_map(|x| {
                                    mempool_guard.find_inflight(&Hash::from(x.1.as_slice()))
                                })
                                .map(|x| EraTxBody(x.era, x.bytes.clone()))
                                .collect_vec()
                        };

                        let mut client_guard = tx_submit_client.lock().await;
                        let tx_submit_client_ref = match client_guard.as_mut() {
                            Some(c) => c,
                            None => {
                                error!(peer=%peer_addr, "No client available; breaking");
                                break;
                            }
                        };

                        if let Err(err) = tx_submit_client_ref.reply_txs(to_send).await {
                            error!(peer=%peer_addr, error=?err, "Error sending TXs upstream");
                        }
                    }
                }
            }

            // No client available; abort the connection
            let mut plexer_client_guard = plexer_client.lock().await;
            if let Some(client) = plexer_client_guard.take() {
                error!(peer=%peer_addr, "Aborting peer client connection...");
                client.abort().await
            }
        });
    }

    async fn propagate_txs(
        tx_submit_client: &mut TxSubmitClient,
        txs: Vec<mempool::Tx>,
    ) -> Result<(), PeerError> {
        let payload = txs
            .iter()
            .map(|x| TxIdAndSize(EraTxId(x.era, x.hash.to_vec()), x.bytes.len() as u32))
            .collect_vec();

        tx_submit_client.reply_tx_ids(payload).await.map_err(|e| {
            PeerError::TxSubmission(format!("Failed to reply TX IDs to peer: {:?}", e))
        })?;

        Ok(())
    }

    async fn collect_transactions(
        mempool: &Mempool,
        receiver: &Arc<RwLock<InputPort<Vec<u8>>>>,
        req: usize,
    ) -> Result<Vec<mempool::Tx>, PeerError> {
        let mut txs = vec![];

        for _ in 0..req {
            let received_msg = {
                let mut receiver_guard = receiver.write().await;
                receiver_guard.recv().await
            }
            .map_err(|e| PeerError::TxSubmission(format!("Failed to receive message: {:?}", e)))?;

            let tx_raw = received_msg.payload;
            let tx = mempool.receive_raw(&tx_raw)?;
            txs.push(tx);
        }

        Ok(txs)
    }
}
