use std::fmt::Error;
use std::sync::Arc;
use std::time::Duration;

use itertools::Itertools;
use pallas::crypto::hash::Hash;
use pallas::network::miniprotocols::txsubmission::{EraTxBody, EraTxId, Request};
use pallas::network::{facades::PeerClient, miniprotocols::txsubmission::TxIdAndSize};
use tokio::sync::{Mutex, RwLock};
use tokio::task;
use tracing::{error, info, warn};

use super::mempool::{self, Mempool};

pub struct TxSubmitPeer {
    mempool: Arc<Mutex<Mempool>>,
    client: Arc<Mutex<Option<PeerClient>>>,
    peer_addr: String,
    network_magic: u64,
    unfulfilled_request: Arc<RwLock<Option<usize>>>,
}

impl TxSubmitPeer {
    pub fn new(peer_addr: &str, network_magic: u64) -> Self {
        TxSubmitPeer {
            mempool: Arc::new(Mutex::new(Mempool::new())),
            client: Arc::new(Mutex::new(None)),
            peer_addr: peer_addr.to_string(),
            network_magic,
            unfulfilled_request: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn init(&mut self) -> Result<(), Error> {
        let mut client = PeerClient::connect(&self.peer_addr, self.network_magic)
            .await
            .map_err(|e| {
                error!(error=?e, peer=%self.peer_addr, "Failed to connect to peer");
                e
            })
            .unwrap();

        client
            .txsubmission()
            .send_init()
            .await
            .map_err(|e| {
                error!(error=?e, peer=%self.peer_addr, "Failed to send init message");
                e
            })
            .unwrap();

        self.client = Arc::new(Mutex::new(Some(client)));

        self.start_background_task();

        Ok(())
    }

    fn start_background_task(&self) {
        let this = self.clone_refs();
        task::spawn(async move {
            loop {
                // Check if there's any unfulfilled requests
                info!(peer=%this.peer_addr, "Checking for unfulfilled request...");
                let outstanding_request = *this.unfulfilled_request.read().await;

                // if there is an unfulfilled request, process it
                if let Some(request) = outstanding_request {
                    if !this.process_unfulfilled(request, &this.peer_addr).await {
                        break;
                    }

                    continue;
                }

                // Otherwise, wait for the next request
                info!(peer=%this.peer_addr, "Waiting for next request...");
                let next_req = {
                    let mut client_guard = this.client.lock().await;
                    let client_ref = match client_guard.as_mut() {
                        Some(c) => c,
                        None => {
                            warn!(peer=%this.peer_addr, "No client available; breaking");
                            break;
                        }
                    };
                    client_ref.txsubmission().next_request().await
                };

                let request = match next_req {
                    Ok(r) => {
                        info!(peer=%this.peer_addr, "Received request from node");
                        r
                    }
                    Err(e) => {
                        error!(peer=%this.peer_addr, error=?e, "Error reading request; breaking loop");
                        break;
                    }
                };

                // Process the consumer's request
                match request {
                    Request::TxIds(ack, req) => {
                        info!(peer=%this.peer_addr, ack, req, "Blocking TxIds request");
                        let ack_usize = ack as usize;
                        let req_usize = req as usize;

                        let mempool_guard = this.mempool.lock().await;
                        let mut client_guard = this.client.lock().await;
                        let client_ref = match client_guard.as_mut() {
                            Some(c) => c,
                            None => {
                                warn!(peer=%this.peer_addr, "No client available; breaking");
                                break;
                            }
                        };

                        this.reply_txs(
                            &mempool_guard,
                            client_ref,
                            ack_usize,
                            req_usize,
                            &this.peer_addr,
                        )
                        .await
                        .ok();
                    }
                    Request::TxIdsNonBlocking(ack, req) => {
                        info!(peer=%this.peer_addr, ack, req, "Non-blocking TxIds request");
                        let mempool_guard = this.mempool.lock().await;
                        mempool_guard.acknowledge(ack as usize);

                        let txs = mempool_guard.request(req as usize);
                        drop(mempool_guard);

                        let mut client_guard = this.client.lock().await;
                        let client_ref = match client_guard.as_mut() {
                            Some(c) => c,
                            None => {
                                warn!(peer=%this.peer_addr, "No client available; breaking");
                                break;
                            }
                        };

                        let _ = &this
                            .propagate_txs(client_ref, txs, &this.peer_addr)
                            .await
                            .ok();
                    }
                    Request::Txs(ids) => {
                        let ids: Vec<_> = ids.iter().map(|x| (x.0, x.1.clone())).collect();

                        info!(peer=%this.peer_addr, ids=%ids.iter().map(|x| hex::encode(&x.1)).join(", "), "Tx batch request");

                        let to_send = {
                            let mempool_guard = this.mempool.lock().await;
                            ids.iter()
                                .filter_map(|x| {
                                    mempool_guard.find_inflight(&Hash::from(x.1.as_slice()))
                                })
                                .map(|x| EraTxBody(x.era, x.bytes.clone()))
                                .collect_vec()
                        };

                        info!(peer=%this.peer_addr, count=to_send.len(), "Sending TXs upstream");

                        let mut client_guard = this.client.lock().await;
                        let client_ref = match client_guard.as_mut() {
                            Some(c) => c,
                            None => {
                                warn!(peer=%this.peer_addr, "No client available; breaking");
                                break;
                            }
                        };

                        if let Err(err) = client_ref.txsubmission().reply_txs(to_send).await {
                            error!(peer=%this.peer_addr, error=?err, "Error sending TXs upstream");
                        }
                    }
                }
            }
        });
    }

    fn clone_refs(&self) -> Self {
        Self {
            mempool: Arc::clone(&self.mempool),
            client: Arc::clone(&self.client),
            peer_addr: self.peer_addr.clone(),
            network_magic: self.network_magic,
            unfulfilled_request: Arc::clone(&self.unfulfilled_request),
        }
    }

    pub async fn add_tx(&self, tx: Vec<u8>) {
        let start_await = tokio::time::Instant::now();
        let mempool = self.mempool.lock().await;
        let elapsed = start_await.elapsed();
        mempool.receive_raw(&tx).unwrap();

        info!(peer=%self.peer_addr, elapsed=?elapsed, "Successfully Added TX to mempool");
    }

    async fn process_unfulfilled(&self, request: usize, peer_addr: &str) -> bool {
        let available = {
            let mempool_guard = self.mempool.lock().await;
            mempool_guard.pending_total()
        };

        if available > 0 {
            let mempool_guard = self.mempool.lock().await;
            let mut client_guard = self.client.lock().await;

            let client_ref = match client_guard.as_mut() {
                Some(c) => c,
                None => {
                    warn!(peer=%peer_addr, "No client available; breaking");
                    return false;
                }
            };

            info!(peer=%peer_addr, request, available, "Found enough TXs to fulfill request");

            self.reply_txs(&mempool_guard, client_ref, 0, request, peer_addr)
                .await
                .ok();
        } else {
            info!(peer=%peer_addr, request, available, "Not enough TXs yet; will retry");
            tokio::time::sleep(Duration::from_secs(10)).await;
        }

        true
    }

    async fn reply_txs(
        &self,
        mempool: &Mempool,
        client: &mut PeerClient,
        ack: usize,
        req: usize,
        peer_addr: &str,
    ) -> Result<(), Error> {
        mempool.acknowledge(ack);

        let available = mempool.pending_total();
        if available > 0 {
            let txs = mempool.request(req);
            self.propagate_txs(client, txs, peer_addr).await?;
            let mut unfulfilled = self.unfulfilled_request.write().await;
            *unfulfilled = None;
        } else {
            info!(peer=%peer_addr, req, available, "Still not enough TXs; storing unfulfilled request");
            let mut unfulfilled = self.unfulfilled_request.write().await;
            *unfulfilled = Some(req);
        }

        Ok(())
    }

    async fn propagate_txs(
        &self,
        client: &mut PeerClient,
        txs: Vec<mempool::Tx>,
        peer_addr: &str,
    ) -> Result<(), Error> {
        info!(peer=%peer_addr, count=txs.len(), "Propagating TX IDs");

        let payload = txs
            .iter()
            .map(|x| TxIdAndSize(EraTxId(x.era, x.hash.to_vec()), x.bytes.len() as u32))
            .collect_vec();

        client
            .txsubmission()
            .reply_tx_ids(payload)
            .await
            .map_err(|e| {
                error!(peer=%peer_addr, error=?e, "Failed to reply with TX IDs");
                e
            })
            .unwrap();

        Ok(())
    }
}
