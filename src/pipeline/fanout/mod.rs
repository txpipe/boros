use std::{sync::Arc, time::Duration};

use gasket::framework::*;
use peer::PeerError;
use peer_manager::{PeerManager, PeerManagerError};
use rand::Rng;
use serde::Deserialize;
use thiserror::Error;
use tokio::time::sleep;
use tracing::info;

use crate::{ledger::relay::MockRelayDataAdapter, storage::{sqlite::SqliteTransaction, Transaction, TransactionStatus}};

pub mod mempool;
pub mod peer;
pub mod peer_manager;

#[derive(Error, Debug)]
pub enum FanoutError {
    #[error("peer manager error: {0}")]
    PeerManager(#[from] PeerManagerError),

    #[error("peer error: {0}")]
    Peer(#[from] PeerError),

    #[error("worker error: {0}")]
    Worker(#[from] gasket::framework::WorkerError),
}

pub enum FanoutUnit {
    Transaction(Transaction),
    PeerDiscovery(String),
}

#[derive(Stage)]
#[stage(name = "fanout", unit = "FanoutUnit", worker = "Worker")]
pub struct Stage {
    storage: Arc<SqliteTransaction>,
    config: PeerManagerConfig,
}

impl Stage {
    pub fn new(storage: Arc<SqliteTransaction>, config: PeerManagerConfig) -> Self {
        Self { storage, config }
    }
}

pub struct Worker {
    peer_manager: PeerManager,
    peer_discovery_queue: u8,
    mock_relay_data_adapter: MockRelayDataAdapter,
}

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(stage: &Stage) -> Result<Self, WorkerError> {
        let peer_addresses = stage.config.peers.clone();

        info!("Bootstrap Peer Addresses: {:?}", peer_addresses);

        let mut peer_manager = PeerManager::new(2, peer_addresses);
        peer_manager.init().await.or_retry()?;

        let mock_relay_data_adapter = MockRelayDataAdapter::new();

        Ok(Self {
            peer_manager,
            peer_discovery_queue: 0,
            mock_relay_data_adapter,
        })
    }

    async fn schedule(
        &mut self,
        stage: &mut Stage,
    ) -> Result<WorkSchedule<FanoutUnit>, WorkerError> {
        let desired_count = stage.config.desired_peer_count;
        let peer_per_request = stage.config.peers_per_request;
        let additional_peers_required = desired_count as usize
            - self.peer_manager.connected_peers_count().await;

        if let Some(tx) = stage
            .storage
            .next(TransactionStatus::Validated)
            .await
            .or_retry()?
        {
            info!("Found Transaction: {}", tx.id);
            return Ok(WorkSchedule::Unit(FanoutUnit::Transaction(tx)));
        }

        if self.peer_discovery_queue < additional_peers_required as u8 {
            info!("Additional Peers Required: {}", additional_peers_required);
            let from_pool_relay = self.mock_relay_data_adapter.pick_peer_rand_from_relay().await;
            let from_peer_discovery = self.peer_manager.pick_peer_rand(peer_per_request).await.or_retry()?;

            let mut rng = rand::rng();
            let chosen_peer = if rng.random_bool(0.5) {
                info!("Onchain Relay chosen: {:?}", from_pool_relay);
                from_pool_relay
            } else {
                info!("P2P Relay chosen: {:?}", from_peer_discovery);
                from_peer_discovery
            };

            if let Some(peer_addr) = chosen_peer {
                self.peer_discovery_queue += 1;
                return Ok(WorkSchedule::Unit(FanoutUnit::PeerDiscovery(peer_addr)));
            }
        }

        sleep(Duration::from_secs(1)).await;
        Ok(WorkSchedule::Idle)
    }

    async fn execute(&mut self, unit: &FanoutUnit, stage: &mut Stage) -> Result<(), WorkerError> {
        match unit {
            FanoutUnit::Transaction(tx) => {
                let mut transaction = tx.clone();
                info!("Propagating Transaction: {}", transaction.id);
                self.peer_manager.add_tx(transaction.raw.clone()).await;
                transaction.status = TransactionStatus::InFlight;
                stage.storage.update(&transaction).await.or_retry()?;
            }
            FanoutUnit::PeerDiscovery(peer_addr) => {
                info!("Connecting to peer: {}", peer_addr);
                self.peer_manager.add_peer(peer_addr).await;
                self.peer_discovery_queue -= 1;
            }
        }
        Ok(())
    }
}

#[derive(Deserialize, Clone)]
pub struct PeerManagerConfig {
    peers: Vec<String>,
    desired_peer_count: u8,
    peers_per_request: u8
}

// Test for Fanout Stage
#[cfg(test)]
pub mod mock_ouroboros_tx_submit_server;

#[cfg(test)]
mod fanout_tests {
    use std::{sync::Arc, time::Duration};

    use hex::decode;
    use mock_ouroboros_tx_submit_server::MockOuroborosTxSubmitPeerServer;
    use pallas::ledger::traverse::MultiEraTx;

    use super::*;

    #[tokio::test]
    async fn it_should_fanout_stage() {
        let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();

        let peer_server = Arc::new(MockOuroborosTxSubmitPeerServer::new(
            "0.0.0.0:3001".to_string(),
            2,
        ));
        peer_server.clone().init().await;

        tokio::time::sleep(Duration::from_millis(200)).await;
        let mut tx_submit_peer_client = peer::Peer::new("127.0.0.1:3001", 2);

        tx_submit_peer_client.init().await.unwrap();

        tokio::time::sleep(Duration::from_secs(1)).await;

        // add txs to peer client
        let cbor_data = "84a300d90102828258202000fbfaa10c6b316fe4b23c60b42313fc2ad89b51c7397f82a4a5ca97bd62a9008258202000fbfaa10c6b316fe4b23c60b42313fc2ad89b51c7397f82a4a5ca97bd62a901018182581d606e2e6d54e1a27ad640786852e20c22fb982ebcee4773a2926aae4b391b00000001a1166016021a0002aa3da100d9010281825820524e506f6a872c4ee3ee6d4b9913670c4b441860b3aa5438de92a676e20f527b5840233e9119fa6a3c58ab42bc384f506c2906104ebb059ab4ea6cc79305ff46c7e194d634d23ff775f92e51246e328711e6cbf38aeda01a1885f922047323c68c04f5f6";

        // Read the raw bytes from the request body.
        let raw_cbor = match decode(cbor_data) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::error!("Failed to decode hex string: {:?}", e);
                return;
            }
        };

        let tx = MultiEraTx::decode(&raw_cbor).unwrap();
        let tx_id = tx.hash();

        tracing::info!("Tx Hash: {:?}", tx_id);

        tx_submit_peer_client.add_tx(raw_cbor.clone()).await;

        // wait for server to stop
        tracing::info!("Waiting for server to stop..");
        loop {
            tokio::time::sleep(Duration::from_millis(10000)).await;
            let is_done = peer_server.is_done.read().unwrap();
            tracing::info!("Is Server done: {:?}", *is_done);
            if *is_done {
                break;
            }
            drop(is_done);
        }

        let server_acknowledge_txs = peer_server.acknowledge_txs.lock().unwrap();
        let mut found = false;

        for tx_from_server in server_acknowledge_txs.iter() {
            tracing::info!(
                "Tx from server: {:?}, Tx from client: {:?}",
                tx_from_server,
                tx_id
            );
            if tx_from_server == &tx_id {
                found = true;
                break;
            }
        }
        assert!(found);
    }
}
