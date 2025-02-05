use std::{
    sync::Arc,
    time::Duration
};

use gasket::framework::*;
use peer_manager::PeerManager;
use serde::Deserialize;
use tokio::time::sleep;
use tracing::info;

use crate::storage::{sqlite::SqliteTransaction, Transaction, TransactionStatus};

pub mod mempool;
pub mod peer;
pub mod peer_manager;

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
    discovery_queue: Vec<String>,
}

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(stage: &Stage) -> Result<Self, WorkerError> {
        let peer_addresses = stage.config.peers.clone();
        let desired_peer_count = stage.config.desired_peer_count;

        info!("Peer Addresses: {:?}", peer_addresses);

        let mut peer_manager = PeerManager::new(2, peer_addresses, desired_peer_count);
        peer_manager.init().await.unwrap();

        Ok(Self { peer_manager, discovery_queue: vec![] })
    }

    async fn schedule(
        &mut self,
        stage: &mut Stage,
    ) -> Result<WorkSchedule<FanoutUnit>, WorkerError> {
        if let Some(tx) = stage
            .storage
            .next(TransactionStatus::Validated)
            .await
            .or_retry()?
        {
            return Ok(WorkSchedule::Unit(FanoutUnit::Transaction(tx)));
        }

        if let Some(peer_addr) = self.discovery_queue.drain(0..1).next() {
            return Ok(WorkSchedule::Unit(FanoutUnit::PeerDiscovery(peer_addr)));
        }


        sleep(Duration::from_secs(1)).await;
        Ok(WorkSchedule::Idle)
    }

    async fn execute(&mut self, unit: &FanoutUnit, stage: &mut Stage) -> Result<(), WorkerError> {
        match unit {
            FanoutUnit::Transaction(tx) => {
                let mut transaction = tx.clone();
                info!("Processing Transaction: {}", transaction.id);
                // For a transaction unit, forward the raw tx bytes to the peer manager for txsubmission.
                self.peer_manager.add_tx(transaction.raw.clone()).await;
                transaction.status = TransactionStatus::InFlight;
                stage.storage.update(&transaction).await.or_retry()?;
            }
            FanoutUnit::PeerDiscovery(_) => {
                info!("Processing PeerSharing unit: Discovering new peers");
                self.peer_manager.discover_peers().await;
            }
        }
        Ok(())
    }
}

#[derive(Deserialize, Clone)]
pub struct PeerManagerConfig {
    peers: Vec<String>,
    desired_peer_count: u8,
    peers_per_request: u8,
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
