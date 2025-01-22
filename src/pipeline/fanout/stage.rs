use std::time::Duration;

use gasket::framework::*;
use tokio::time::sleep;
use tracing::info;

use crate::{pipeline::Transaction, storage::in_memory_db::CborTransactionsDb, Config};

use super::tx_submit_peer_manager::TxSubmitPeerManager;

#[derive(Stage)]
#[stage(name = "fanout", unit = "Transaction", worker = "Worker")]
pub struct Stage {
    pub cbor_txs_db: CborTransactionsDb,
    pub config: Config,
}

impl Stage {
    pub fn new(cbor_txs_db: CborTransactionsDb, config: Config) -> Self {
        Self {
            cbor_txs_db,
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
        info!("Cbor Transactions Length: {}", stage.cbor_txs_db.cbor_txs.lock().unwrap().len());
        
        if let Some(tx_cbor) = stage.cbor_txs_db.pop_tx() {
            return Ok(WorkSchedule::Unit(Transaction {
                cbor: tx_cbor
            }));
        } else {
            // TODO: should we really have a sleep here?
            sleep(Duration::from_secs(30)).await;
            return Ok(WorkSchedule::Idle);
        }
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
