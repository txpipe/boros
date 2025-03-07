use std::{sync::Arc, time::Duration};

use gasket::framework::*;
use tokio::time::sleep;
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
    u5c_adapter: Arc<dyn U5cDataAdapter>,
    pub sender: gasket::messaging::OutputPort<Vec<u8>>,
}

impl Stage {
    pub fn new(
        storage: Arc<SqliteTransaction>,
        priority: Arc<Priority>,
        u5c_adapter: Arc<dyn U5cDataAdapter>,
    ) -> Self {
        Self {
            storage,
            priority,
            u5c_adapter,
            sender: Default::default(),
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
            let mut tx = tx.clone();
            let message = gasket::messaging::Message::from(tx.raw.clone());

            if let Err(e) = stage.sender.send(message).await {
                info!("Failed to broadcast transaction: {}", e);
            } else {
                info!("Transaction {} broadcasted to receivers", tx.id);

                let tip = stage.u5c_adapter.fetch_tip().await.or_retry()?;
                tx.status = TransactionStatus::InFlight;
                tx.slot = Some(tip.0);
                stage.storage.update(&tx).await.or_retry()?;
            }
        }

        Ok(())
    }
}
