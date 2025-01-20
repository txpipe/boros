use std::time::Duration;

use gasket::framework::*;
use tokio::time::sleep;
use tracing::info;

use crate::pipeline::Transaction;

use super::tx_submit_peer_manager::TxSubmitPeerManager;

#[derive(Stage)]
#[stage(name = "fanout", unit = "Transaction", worker = "Worker")]
pub struct Stage {}

pub struct Worker {
    tx_submit_peer_manager: TxSubmitPeerManager,
}

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(_stage: &Stage) -> Result<Self, WorkerError> {
        // Load configuration and Start Clients
        let peer_addresses = vec![
            "preview-node.play.dev.cardano.org:3001".to_string(),
            "adaboy-preview-1c.gleeze.com:5000".to_string(),
            "testicles.kiwipool.org:9720".to_string(),
        ];
        
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
        _stage: &mut Stage,
    ) -> Result<WorkSchedule<Transaction>, WorkerError> {
        // TODO: fetch data from db
        sleep(Duration::from_secs(30)).await;
        // Pass transaction bytes (maybe add a cbor field (?))
        Ok(WorkSchedule::Unit(Transaction {
            cbor: vec![0, 1, 2, 3],
        }))
    }

    async fn execute(
        &mut self,
        _unit: &Transaction,
        _stage: &mut Stage,
    ) -> Result<(), WorkerError> {
        info!("fanout stage");

        // extract cbor from unit and pass it to tx_submit_peer_manager
        // comment out for now until we have a proper tx to submit
        // let tx_cbor = unit.cbor.clone();
        // self.tx_submit_peer_manager.add_tx(tx_cbor).await;

        Ok(())
    }
}
