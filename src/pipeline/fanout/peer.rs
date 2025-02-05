use std::fmt::Error;
use std::sync::Arc;
use std::time::Duration;

use itertools::Itertools;
use pallas::codec::minicbor::decode::info;
use pallas::crypto::hash::Hash;
use pallas::network::miniprotocols::peersharing::PeerAddress;
use pallas::network::miniprotocols::txsubmission::{EraTxBody, EraTxId, Request};
use pallas::network::{facades::PeerClient, miniprotocols::txsubmission::TxIdAndSize};
use tokio::sync::{Mutex, RwLock};
use tokio::task;
use tracing::{error, info};

use super::mempool::{self, Mempool};

pub struct Peer {
    mempool: Arc<Mutex<Mempool>>,
    peer_addr: String,
    client: Arc<Mutex<Option<PeerClient>>>,
    network_magic: u64,
    unfulfilled_request: Arc<RwLock<Option<usize>>>,
    pub is_peer_sharing_enabled: bool,
    pub is_alive: bool,
}

impl Peer {
    pub fn new(peer_addr: &str, network_magic: u64) -> Self {
        Peer {
            mempool: Arc::new(Mutex::new(Mempool::new())),
            client: Arc::new(Mutex::new(None)),
            peer_addr: peer_addr.to_string(),
            network_magic,
            unfulfilled_request: Arc::new(RwLock::new(None)),
            is_peer_sharing_enabled: false,
            is_alive: false,
        }
    }

    pub async fn init(&mut self) -> Result<(), pallas::network::facades::Error> {
        self.is_peer_sharing_enabled = self.query_peer_sharing_mode().await?;
        info!(peer=%self.peer_addr, "Peer sharing mode: {}", self.is_peer_sharing_enabled);

        let mut client = PeerClient::connect(&self.peer_addr, self.network_magic).await?;
        info!(peer=%self.peer_addr, "Connected to peer");

        client.txsubmission().send_init().await.unwrap();

        info!(peer=%self.peer_addr, "Sent init message");

        self.client = Arc::new(Mutex::new(Some(client)));
        self.is_alive = true;

        self.start_background_task();
        info!(peer=%self.peer_addr, "Started background task");

        Ok(())
    }

    pub async fn discover_peers(&mut self, desired_peers: u8) -> Result<Vec<PeerAddress>, Error> {
        let mut client_guard = self.client.lock().await;
        let client = match client_guard.as_mut() {
            Some(c) => c,
            None => {
                error!(peer = %self.peer_addr, "No client available");
                return Err(Error);
            }
        };

        client
            .peersharing()
            .send_share_request(desired_peers)
            .await
            .unwrap();

        let mut discovered = vec![];
        if let Ok(peers) = client.peersharing().recv_peer_addresses().await {
            info!(peer = %self.peer_addr, "Discovered new peers: {:?}", peers);
            discovered.extend(peers);
        }

        Ok(discovered)
    }

    async fn query_peer_sharing_mode(&self) -> Result<bool, pallas::network::facades::Error> {
        let version_table = PeerClient::query(&self.peer_addr, self.network_magic).await?;

        info!(peer=%self.peer_addr, "Received version table: {:?}", version_table);
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
                info!(peer=%peer_addr, "Checking for unfulfilled request...");
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
                info!(peer=%peer_addr, "Waiting for next request...");
                let next_req = {
                    let mut client_guard = client_arc.lock().await;
                    let client_ref = match client_guard.as_mut() {
                        Some(c) => c,
                        None => {
                            error!(peer=%peer_addr, "No client available; breaking");
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

                // Process the consumer's request
                match request {
                    Request::TxIds(ack, req) => {
                        info!(peer=%peer_addr, ack, req, "Blocking TxIds request");

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
                            &peer_addr,
                            &unfulfilled_request_arc,
                        )
                        .await
                        .ok();
                    }
                    Request::TxIdsNonBlocking(ack, req) => {
                        info!(peer=%peer_addr, ack, req, "Non-blocking TxIds request");
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

                        Self::propagate_txs(client_ref, txs, &peer_addr).await.ok();
                    }
                    Request::Txs(ids) => {
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

                        info!(peer=%peer_addr, count=to_send.len(), "Sending TXs upstream");

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
    ) -> Result<(), Error> {
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
                    return Err(Error);
                }
            };

            info!(peer=%peer_addr, request, available, "Found enough TXs to fulfill request");

            Self::reply_txs(
                &mempool_guard,
                client_ref,
                0,
                request,
                peer_addr,
                unfulfilled_request,
            )
            .await
            .ok();
        } else {
            info!(peer=%peer_addr, request, available, "Not enough TXs yet; will retry");
            tokio::time::sleep(Duration::from_secs(10)).await;
        }

        Ok(())
    }

    async fn reply_txs(
        mempool: &Mempool,
        client: &mut PeerClient,
        ack: usize,
        req: usize,
        peer_addr: &str,
        unfulfilled_request: &Arc<RwLock<Option<usize>>>,
    ) -> Result<(), Error> {
        mempool.acknowledge(ack);

        let available = mempool.pending_total();
        if available > 0 {
            let txs = mempool.request(req);
            Self::propagate_txs(client, txs, peer_addr).await?;
            let mut unfulfilled = unfulfilled_request.write().await;
            *unfulfilled = None;
        } else {
            info!(peer=%peer_addr, req, available, "Still not enough TXs; storing unfulfilled request");
            let mut unfulfilled = unfulfilled_request.write().await;
            *unfulfilled = Some(req);
        }

        Ok(())
    }

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
