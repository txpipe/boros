use std::{sync::Arc, time::Duration};

use gasket::framework::*;
use tokio::{sync::broadcast::Sender, time::sleep};
use tracing::info;

use super::CAP;
use crate::{
    ledger::u5c::U5cDataAdapter,
    priority::Priority,
    storage::{sqlite::SqliteTransaction, Transaction, TransactionStatus},
};

#[derive(Stage)]
#[stage(name = "ingest", unit = "Vec<Transaction>", worker = "Worker")]
pub struct Stage {
    storage: Arc<SqliteTransaction>,
    priority: Arc<Priority>,
    sender: Sender<Vec<u8>>,
    u5c_adapter: Arc<dyn U5cDataAdapter>,
}

impl Stage {
    pub fn new(
        storage: Arc<SqliteTransaction>,
        priority: Arc<Priority>,
        sender: Sender<Vec<u8>>,
        u5c_adapter: Arc<dyn U5cDataAdapter>,
    ) -> Self {
        Self {
            storage,
            priority,
            sender,
            u5c_adapter,
        }
    }
}

pub struct Worker;

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(_stage: &Stage) -> Result<Self, WorkerError> {
        Ok(Self)
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
        for tx in unit {
            let receiver_count = stage.sender.receiver_count();
            let mut tx = tx.clone();

            if receiver_count > 0 {
                let broadcast_result = stage.sender.send(tx.raw.clone());

                match broadcast_result {
                    Ok(n) => {
                        info!("Transaction {} broadcasted to {} receivers", tx.id, n);

                        let tip = stage.u5c_adapter.fetch_tip().await.or_retry()?;

                        tx.status = TransactionStatus::InFlight;
                        tx.slot = Some(tip.0);

                        stage.storage.update(&tx).await.or_retry()?;
                    }
                    Err(e) => info!("Failed to broadcast transaction: {}", e),
                }
            } else {
                info!("No receivers available for broadcasting");
            }
        }

        Ok(())
    }
}
