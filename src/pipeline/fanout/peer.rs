use std::sync::Arc;
use std::time::Duration;

use itertools::Itertools;
use pallas::crypto::hash::Hash;
use pallas::network::miniprotocols::peersharing::PeerAddress;
use pallas::network::miniprotocols::txsubmission::{EraTxBody, EraTxId, Request};
use pallas::network::{facades::PeerClient, miniprotocols::txsubmission::TxIdAndSize};
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};
use tokio::task;
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
    client: Arc<Mutex<Option<PeerClient>>>,
    network_magic: u64,
    unfulfilled_request: Arc<RwLock<Option<usize>>>,
    pub peer_addr: String,
    pub is_peer_sharing_enabled: bool,
    pub is_alive: bool,
}

impl Peer {
    pub fn new(peer_addr: &str, network_magic: u64) -> Self {
        Self {
            mempool: Arc::new(Mutex::new(Mempool::new())),
            client: Arc::new(Mutex::new(None)),
            peer_addr: peer_addr.to_string(),
            network_magic,
            unfulfilled_request: Arc::new(RwLock::new(None)),
            is_peer_sharing_enabled: false,
            is_alive: false,
        }
    }

    pub async fn init(&mut self) -> Result<(), PeerError> {
        let mut client = PeerClient::connect(&self.peer_addr, self.network_magic)
            .await
            .map_err(|e| {
                PeerError::Initialization(format!("Failed to connect to peer: {:?}", e))
            })?;

        client.txsubmission().send_init().await.map_err(|e| {
            PeerError::Initialization(format!("Failed to send init message to peer: {:?}", e))
        })?;

        self.client = Arc::new(Mutex::new(Some(client)));
        self.is_alive = true;
        info!(peer=%self.peer_addr, "Peer initialized");

        self.start_background_task();

        Ok(())
    }

    pub async fn discover_peers(
        &mut self,
        desired_peers: u8,
    ) -> Result<Vec<PeerAddress>, PeerError> {
        let mut client_guard = self.client.lock().await;
        let client = match client_guard.as_mut() {
            Some(c) => c,
            None => {
                error!(peer = %self.peer_addr, "No client available");
                return Err(PeerError::PeerDiscovery("No client available".to_string()));
            }
        };

        client
            .peersharing()
            .send_share_request(desired_peers)
            .await
            .map_err(|e| {
                PeerError::PeerDiscovery(format!("Failed to send share request: {:?}", e))
            })?;

        let mut discovered = vec![];
        if let Ok(peers) = client
            .peersharing()
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
        let client_arc = Arc::clone(&self.client);
        let mempool_arc = Arc::clone(&self.mempool);
        let unfulfilled_request_arc = Arc::clone(&self.unfulfilled_request);
        let peer_addr = self.peer_addr.clone();

        task::spawn(async move {
            loop {
                // Check if there's any unfulfilled requests
                let outstanding_request = *unfulfilled_request_arc.read().await;

                // if there is an unfulfilled request, process it
                if let Some(request) = outstanding_request {
                    if let Err(err) = Self::process_unfulfilled(
                        request,
                        &peer_addr,
                        &mempool_arc,
                        &client_arc,
                        &unfulfilled_request_arc,
                    )
                    .await
                    {
                        error!(peer=%peer_addr, error=?err, "Error processing unfulfilled request");
                        break;
                    }

                    continue;
                }

                // Otherwise, wait for the next request
                let next_req = {
                    let mut client_guard = client_arc.lock().await;
                    let client_ref = match client_guard.as_mut() {
                        Some(c) => c,
                        None => {
                            error!(peer=%peer_addr, "No client available; breaking");
                            break;
                        }
                    };

                    info!(peer=%peer_addr, "Waiting for next request");
                    client_ref.txsubmission().next_request().await
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
                        let mempool_guard = mempool_arc.lock().await;
                        let mut client_guard = client_arc.lock().await;
                        let client_ref = match client_guard.as_mut() {
                            Some(c) => c,
                            None => {
                                error!(peer=%peer_addr, "No client available; breaking");
                                break;
                            }
                        };

                        Self::reply_txs(
                            &mempool_guard,
                            client_ref,
                            ack as usize,
                            req as usize,
                            &unfulfilled_request_arc,
                        )
                        .await
                        .ok();
                    }
                    Request::TxIdsNonBlocking(ack, req) => {
                        info!(peer=%peer_addr, "Received TX IDs Non-Blocking request: ack={}, req={}", ack, req);
                        let mempool_guard = mempool_arc.lock().await;
                        mempool_guard.acknowledge(ack as usize);

                        let txs = mempool_guard.request(req as usize);
                        drop(mempool_guard);

                        let mut client_guard = client_arc.lock().await;
                        let client_ref = match client_guard.as_mut() {
                            Some(c) => c,
                            None => {
                                error!(peer=%peer_addr, "No client available; breaking");
                                break;
                            }
                        };

                        Self::propagate_txs(client_ref, txs).await.ok();
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

                        let mut client_guard = client_arc.lock().await;
                        let client_ref = match client_guard.as_mut() {
                            Some(c) => c,
                            None => {
                                error!(peer=%peer_addr, "No client available; breaking");
                                break;
                            }
                        };

                        if let Err(err) = client_ref.txsubmission().reply_txs(to_send).await {
                            error!(peer=%peer_addr, error=?err, "Error sending TXs upstream");
                        }
                    }
                }
            }

            // No client available; abort the connection
            let mut final_client_guard = client_arc.lock().await;
            if let Some(client) = final_client_guard.take() {
                error!(peer=%peer_addr, "Aborting tx submit peer client connection...");
                client.abort().await
            }
        });
    }

    pub async fn add_tx(&self, tx: Vec<u8>) {
        let mempool = self.mempool.lock().await;
        mempool.receive_raw(&tx).unwrap();
    }

    async fn process_unfulfilled(
        request: usize,
        peer_addr: &str,
        mempool: &Arc<Mutex<Mempool>>,
        client: &Arc<Mutex<Option<PeerClient>>>,
        unfulfilled_request: &Arc<RwLock<Option<usize>>>,
    ) -> Result<(), PeerError> {
        let available = {
            let mempool_guard = mempool.lock().await;
            mempool_guard.pending_total()
        };

        if available > 0 {
            let mempool_guard = mempool.lock().await;
            let mut client_guard = client.lock().await;

            let client_ref = match client_guard.as_mut() {
                Some(c) => c,
                None => {
                    error!(peer=%peer_addr, "No client available; breaking");
                    return Err(PeerError::TxSubmission("No client available".to_string()));
                }
            };

            Self::reply_txs(&mempool_guard, client_ref, 0, request, unfulfilled_request).await?;
        } else {
            tokio::time::sleep(Duration::from_secs(10)).await;
        }

        Ok(())
    }

    async fn reply_txs(
        mempool: &Mempool,
        client: &mut PeerClient,
        ack: usize,
        req: usize,
        unfulfilled_request: &Arc<RwLock<Option<usize>>>,
    ) -> Result<(), PeerError> {
        mempool.acknowledge(ack);

        let available = mempool.pending_total();
        if available > 0 {
            let txs = mempool.request(req);
            Self::propagate_txs(client, txs).await?;
            let mut unfulfilled = unfulfilled_request.write().await;
            *unfulfilled = None;
        } else {
            let mut unfulfilled = unfulfilled_request.write().await;
            *unfulfilled = Some(req);
        }

        Ok(())
    }

    async fn propagate_txs(
        client: &mut PeerClient,
        txs: Vec<mempool::Tx>,
    ) -> Result<(), PeerError> {
        let payload = txs
            .iter()
            .map(|x| TxIdAndSize(EraTxId(x.era, x.hash.to_vec()), x.bytes.len() as u32))
            .collect_vec();

        client
            .txsubmission()
            .reply_tx_ids(payload)
            .await
            .map_err(|e| {
                PeerError::TxSubmission(format!("Failed to reply TX IDs to peer: {:?}", e))
            })?;

        Ok(())
    }
}
