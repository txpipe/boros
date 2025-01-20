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

/// A TxSubmitPeer has:
/// - Its own mempool
/// - An optional PeerClient connection
/// - The peer address and network magic
pub struct TxSubmitPeer {
    mempool: Arc<Mutex<Mempool>>,
    client: Arc<Mutex<Option<PeerClient>>>,
    peer_addr: String,
    network_magic: u64,
    unfulfilled_request: Arc<RwLock<Option<usize>>>,
}

impl TxSubmitPeer {
    /// Lightweight constructor: just set fields and create a new mempool.
    /// No I/O occurs here.
    pub fn new(peer_addr: &str, network_magic: u64) -> Self {
        TxSubmitPeer {
            mempool: Arc::new(Mutex::new(Mempool::new())),
            client: Arc::new(Mutex::new(None)),
            peer_addr: peer_addr.to_string(),
            network_magic,
            unfulfilled_request: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize the peer connection (async).
    /// 1) Connect to the Cardano node at `peer_addr`.
    /// 2) Send the `txsubmission` init message.
    /// 3) Spawn a background task to handle requests.
    /// 4) Return immediately (non-blocking).
    pub async fn init(&mut self) -> Result<(), Error> {
        // 1) Connect to the node
        let mut client = PeerClient::connect(&self.peer_addr, self.network_magic)
            .await
            .map_err(|e| {
                error!(error=?e, peer=%self.peer_addr, "Failed to connect to peer");
                e
            }).unwrap();

        // 2) Initialize the txsubmission mini-protocol
        client.txsubmission().send_init().await.map_err(|e| {
            error!(error=?e, peer=%self.peer_addr, "Failed to send init message");
            e
        }).unwrap();

        // 3) Store it so we can spawn our background loop
        self.client = Arc::new(Mutex::new(Some(client)));

        // 4) Spawn the loop in another task
        self.start_background_task();

        Ok(())
    }

    /// Spawns a background async loop that continuously
    /// waits for new requests from the connected node.
    fn start_background_task(&mut self) {
        let client_arc = Arc::clone(&self.client);
        let mempool_arc = Arc::clone(&self.mempool);
        let unfulfilled_request_arc = Arc::clone(&self.unfulfilled_request);
        let peer_addr = self.peer_addr.clone();

        task::spawn(async move {
            loop {
                // Separate function to handle leftover unfulfilled requests
                async fn process_unfulfilled(
                    mempool: &Mempool,
                    client: &mut PeerClient,
                    unfulfilled_request_arc: Arc<RwLock<Option<usize>>>,
                    request: usize,
                    peer_addr: &str,
                ) -> Option<usize> {
                    let available = mempool.pending_total();

                    if available > 0 {
                        info!(peer=%peer_addr, request, available, "Found enough TXs to fulfill request");
                        reply_txs(mempool, client, unfulfilled_request_arc, 0, request, peer_addr)
                            .await
                            .ok();
                        None
                    } else {
                        info!(peer=%peer_addr, request, available, "Not enough TXs yet; will retry");
                        tokio::time::sleep(Duration::from_secs(10)).await;
                        Some(request)
                    }
                }

                // Separate function to reply with TXs
                async fn reply_txs(
                    mempool: &Mempool,
                    client: &mut PeerClient,
                    unfulfilled_request_arc: Arc<RwLock<Option<usize>>>,
                    ack: usize,
                    req: usize,
                    peer_addr: &str,
                ) -> Result<(), Error> {
                    mempool.acknowledge(ack);

                    let available = mempool.pending_total();
                    if available > 0 {
                        let txs = mempool.request(req);
                        propagate_txs(client, txs, peer_addr).await?;
                        let mut unfulfilled = unfulfilled_request_arc.write().await;
                        *unfulfilled = None;
                    } else {
                        info!(peer=%peer_addr, req, available, "Still not enough TXs; storing unfulfilled request");
                        let mut unfulfilled = unfulfilled_request_arc.write().await;
                        *unfulfilled = Some(req);
                    }

                    Ok(())
                }

                // Check if there's an unfulfilled request
                info!(peer=%peer_addr, "Checking for unfulfilled request...");
                let outstanding_request = {
                    let read_guard = unfulfilled_request_arc.read().await;
                    *read_guard
                };

                match outstanding_request {
                    Some(request) => {
                        // Handle leftover request
                        let mempool_guard = mempool_arc.lock().await;
                        let mut client_guard = client_arc.lock().await;
                        let client_ref = match client_guard.as_mut() {
                            Some(c) => c,
                            None => {
                                warn!(peer=%peer_addr, "No client available; breaking");
                                break;
                            }
                        };

                        process_unfulfilled(
                            &mempool_guard,
                            client_ref,
                            Arc::clone(&unfulfilled_request_arc),
                            request,
                            &peer_addr,
                        )
                        .await;
                    }
                    None => {
                        info!(peer=%peer_addr, "Waiting for next request...");
                        // We lock only to get the next_request
                        let next_req = {
                            let mut client_guard = client_arc.lock().await;
                            let client_ref = match client_guard.as_mut() {
                                Some(c) => c,
                                None => {
                                    warn!(peer=%peer_addr, "No client available; breaking");
                                    break;
                                }
                            };
                            client_ref.txsubmission().next_request().await
                        };

                        let request = match next_req {
                            Ok(r) => {
                                info!(peer=%peer_addr, "Received request from node");
                                r
                            }
                            Err(e) => {
                                error!(peer=%peer_addr, error=?e, "Error reading request; breaking loop");
                                break;
                            }
                        };

                        // Handle the received request
                        match request {
                            Request::TxIds(ack, req) => {
                                info!(peer=%peer_addr, ack, req, "Blocking TxIds request");
                                let ack_usize = ack as usize;
                                let req_usize = req as usize;

                                let mempool_guard = mempool_arc.lock().await;
                                let mut client_guard = client_arc.lock().await;
                                let client_ref = match client_guard.as_mut() {
                                    Some(c) => c,
                                    None => {
                                        warn!(peer=%peer_addr, "No client available; breaking");
                                        break;
                                    }
                                };

                                reply_txs(
                                    &mempool_guard,
                                    client_ref,
                                    Arc::clone(&unfulfilled_request_arc),
                                    ack_usize,
                                    req_usize,
                                    &peer_addr,
                                )
                                .await
                                .ok();
                            }
                            Request::TxIdsNonBlocking(ack, req) => {
                                info!(peer=%peer_addr, ack, req, "Non-blocking TxIds request");
                                let mempool_guard = mempool_arc.lock().await;
                                mempool_guard.acknowledge(ack as usize);

                                let txs = mempool_guard.request(req as usize);
                                drop(mempool_guard); // drop before I/O

                                let mut client_guard = client_arc.lock().await;
                                let client_ref = match client_guard.as_mut() {
                                    Some(c) => c,
                                    None => {
                                        warn!(peer=%peer_addr, "No client available; breaking");
                                        break;
                                    }
                                };
                                propagate_txs(client_ref, txs, &peer_addr).await.ok();
                            }
                            Request::Txs(ids) => {
                                // Collect the Ids hash and log them
                                let ids: Vec<_> = ids.iter().map(|x| (x.0, x.1.clone())).collect();

                                info!(peer=%peer_addr, ids=%ids.iter().map(|x| hex::encode(&x.1)).join(", "), "Tx batch request");

                                let to_send = {
                                    let mempool_guard = mempool_arc.lock().await;
                                    ids.iter()
                                        .filter_map(|x| {
                                            mempool_guard.find_inflight(&Hash::from(x.1.as_slice()))
                                        })
                                        .map(|x| EraTxBody(x.era, x.bytes.clone()))
                                        .collect_vec()
                                };

                                // Log the number of TXs we're sending
                                info!(peer=%peer_addr, count=to_send.len(), "Sending TXs upstream");

                                let mut client_guard = client_arc.lock().await;
                                let client_ref = match client_guard.as_mut() {
                                    Some(c) => c,
                                    None => {
                                        warn!(peer=%peer_addr, "No client available; breaking");
                                        break;
                                    }
                                };

                                if let Err(err) = client_ref.txsubmission().reply_txs(to_send).await {
                                    error!(peer=%peer_addr, error=?err, "Error sending TXs upstream");
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    /// Add a new transaction to the mempool.
    pub async fn add_tx(&self, tx: Vec<u8>) {
        let mempool = self.mempool.lock().await;
        mempool.receive_raw(&tx).unwrap();
    }
}

/// Propagate TX IDs to the node.
async fn propagate_txs(
    client: &mut PeerClient,
    txs: Vec<mempool::Tx>,
    peer_addr: &str,
) -> Result<(), Error> {
    info!(peer=%peer_addr, count=txs.len(), "Propagating TX IDs");

    let payload = txs
        .iter()
        .map(|x| TxIdAndSize(EraTxId(x.era, x.hash.to_vec()), x.bytes.len() as u32))
        .collect_vec();

    client.txsubmission().reply_tx_ids(payload).await.map_err(|e| {
        error!(peer=%peer_addr, error=?e, "Failed to reply with TX IDs");
        e
    }).unwrap();

    Ok(())
}
