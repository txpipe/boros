// src/pipeline/broadcast/mod.rs
use std::{sync::Arc, time::Duration};

use gasket::framework::*;
use peer_manager::PeerManager;
use tokio::sync::broadcast;
use tokio::time::sleep;
use tracing::info;

use crate::{
    priority::Priority,
    storage::{sqlite::SqliteTransaction, Transaction, TransactionStatus},
};

use super::{fanout::PeerManagerConfig, CAP};

pub mod mempool;
pub mod peer;
pub mod peer_manager;

#[derive(Stage)]
#[stage(name = "broadcast", unit = "Vec<Transaction>", worker = "Worker")]
pub struct Stage {
    config: PeerManagerConfig,
    storage: Arc<SqliteTransaction>,
    priority: Arc<Priority>,
}

impl Stage {
    pub fn new(config: PeerManagerConfig, storage: Arc<SqliteTransaction>, priority: Arc<Priority>) -> Self {
        Self { config, storage, priority }
    }
}

pub struct Worker {
    tx: broadcast::Sender<Vec<u8>>
}

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(stage: &Stage) -> Result<Self, WorkerError> {
        let (tx, _) = broadcast::channel::<Vec<u8>>(CAP as usize);
        let peer_addresses = stage.config.peers.clone();

        info!("Bootstrap Peer Addresses: {:?}", peer_addresses);

        let mut peer_manager = PeerManager::new(2, peer_addresses);
        let mut rx = tx.subscribe();
        peer_manager.init(&mut rx).await.or_retry()?;

        Ok(Self { tx })
    }

    async fn schedule(
        &mut self,
        stage: &mut Stage,
    ) -> Result<WorkSchedule<Vec<Transaction>>, WorkerError> {
        let transactions = stage
            .priority
            .next(TransactionStatus::Validated, CAP)
            .await
            .or_retry()?;

        if !transactions.is_empty() {
            return Ok(WorkSchedule::Unit(transactions));
        }

        sleep(Duration::from_secs(1)).await;
        Ok(WorkSchedule::Idle)
    }

    async fn execute(
        &mut self,
        unit: &Vec<Transaction>,
        stage: &mut Stage,
    ) -> Result<(), WorkerError> {
        for transaction in unit {
            let mut tx = transaction.clone();
            info!("Broadcasting Transaction: {}", tx.id);

            let receiver_count = self.tx.receiver_count();
            if receiver_count > 0 {
                let broadcast_result = self.tx.send(tx.raw.clone());
                match broadcast_result {
                    Ok(n) => info!("Transaction {} broadcasted to {} receivers", tx.id, n),
                    Err(e) => info!("Failed to broadcast transaction: {}", e),
                }
            } else {
                info!("No receivers available for broadcasting");
            }

            tx.status = TransactionStatus::InFlight;
            stage.storage.update(&tx).await.or_retry()?;
        }

        Ok(())
    }
}
