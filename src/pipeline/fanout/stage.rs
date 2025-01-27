use std::{sync::Arc, time::Duration};

use gasket::framework::*;
use tokio::time::sleep;
use tracing::info;

use crate::{pipeline::Transaction, storage::sqlite::SqliteTransaction, Config};

use super::tx_submit_peer_manager::TxSubmitPeerManager;

#[derive(Stage)]
#[stage(name = "fanout", unit = "Transaction", worker = "Worker")]
pub struct Stage {
    pub tx_storage:  Arc<SqliteTransaction>,
    pub config: Config,
}

impl Stage {
    pub fn new(config: Config, tx_storage: Arc<SqliteTransaction>) -> Self {
        Self {
            tx_storage,
            config,
        }
    }
}

pub struct Worker {
    tx_submit_peer_manager: TxSubmitPeerManager,
}

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(_stage: &Stage) -> Result<Self, WorkerError> {
        // Load configuration and Start Clients
        let peer_addresses = _stage.config.peer_manager.peers.clone();

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
        // info!("Cbor Transactions Length: {}", stage.tx_storage);
        
        // if let Some(tx_cbor) = stage.cbor_txs_db.pop_tx() {
        //     return Ok(WorkSchedule::Unit(Transaction {
        //         cbor: tx_cbor
        //     }));
        // } else {
        //     // TODO: should we really have a sleep here?
        //     sleep(Duration::from_secs(30)).await;
        //     return Ok(WorkSchedule::Idle);
        // }
        return Ok(WorkSchedule::Idle);
    }

    async fn execute(
        &mut self,
        unit: &Transaction,
        _stage: &mut Stage,
    ) -> Result<(), WorkerError> {
        info!("fanout stage");

        // extract cbor from unit and pass it to tx_submit_peer_manager
        // comment out for now until we have a proper tx to submit
        let tx_cbor = unit.cbor.clone();
        self.tx_submit_peer_manager.add_tx(tx_cbor).await;

        Ok(())
    }
}
