use std::{sync::Arc, time::Duration};

use gasket::framework::*;
use serde::Deserialize;
use tokio::time::sleep;
use tracing::info;
use tx_submit_peer_manager::TxSubmitPeerManager;

use crate::storage::{sqlite::SqliteTransaction, Transaction, TransactionStatus};

pub mod mempool;
pub mod tx_submit_peer;
pub mod tx_submit_peer_manager;

#[derive(Stage)]
#[stage(name = "fanout", unit = "Transaction", worker = "Worker")]
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
    tx_submit_peer_manager: TxSubmitPeerManager,
}

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(stage: &Stage) -> Result<Self, WorkerError> {
        // Load configuration and Start Clients
        let peer_addresses = stage.config.peers.clone();

        info!("Peer Addresses: {:?}", peer_addresses);

        // Proof of Concept: TxSubmitPeerManager
        // Pass Config Network Magic and Peer Addresses
        let mut tx_submit_peer_manager = TxSubmitPeerManager::new(2, peer_addresses);
        tx_submit_peer_manager.init().await.unwrap();

        Ok(Self {
            tx_submit_peer_manager,
        })
    }

    async fn schedule(
        &mut self,
        stage: &mut Stage,
    ) -> Result<WorkSchedule<Transaction>, WorkerError> {
        if let Some(tx) = stage
            .storage
            .next(TransactionStatus::Validated)
            .await
            .or_retry()?
        {
            return Ok(WorkSchedule::Unit(tx));
        }

        sleep(Duration::from_secs(1)).await;
        Ok(WorkSchedule::Idle)
    }

    async fn execute(&mut self, unit: &Transaction, stage: &mut Stage) -> Result<(), WorkerError> {
        let mut transaction = unit.clone();
        info!("fanout {}", transaction.id);

        // extract cbor from unit and pass it to tx_submit_peer_manager
        // comment out for now until we have a proper tx to submit
        self.tx_submit_peer_manager
            .add_tx(transaction.raw.clone())
            .await;

        transaction.status = TransactionStatus::InFlight;
        stage.storage.update(&transaction).await.or_retry()?;

        Ok(())
    }
}

#[derive(Deserialize, Clone)]
pub struct PeerManagerConfig {
    peers: Vec<String>,
}

// Test for Fanout Stage
#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::sqlite::{SqliteStorage, SqliteTransaction};
    use std::sync::Arc;
    use std::vec;

    #[tokio::test]
    async fn test_fanout_stage() {
        let sqlite_storage = SqliteStorage::ephemeral().await.unwrap();
        let tx_storage = Arc::new(SqliteTransaction::new(sqlite_storage));

        let config = PeerManagerConfig {
            peers: vec!["".to_string()],
        };
        // Run mock node

        // Run Fanout Stage
        let _fanout = Stage::new(tx_storage.clone(), config);
    }
}
