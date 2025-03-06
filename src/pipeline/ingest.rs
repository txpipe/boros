use std::{sync::Arc, time::Duration};

use gasket::framework::*;
use tokio::time::sleep;
use tracing::info;

use super::CAP;
use crate::{
    queue::priority::Priority,
    storage::{sqlite::SqliteTransaction, Transaction, TransactionStatus},
};

#[derive(Stage)]
#[stage(name = "ingest", unit = "Vec<Transaction>", worker = "Worker")]
pub struct Stage {
    storage: Arc<SqliteTransaction>,
    priority: Arc<Priority>,
}

impl Stage {
    pub fn new(storage: Arc<SqliteTransaction>, priority: Arc<Priority>) -> Self {
        Self { storage, priority }
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
            .next(TransactionStatus::Pending, CAP)
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
        info!("validating {} transactions", unit.len());

        let transactions = unit
            .iter()
            .map(|tx| {
                let mut tx = tx.clone();
                tx.status = TransactionStatus::Validated;
                tx
            })
            .collect();

        stage.storage.update_batch(&transactions).await.or_retry()?;

        Ok(())
    }
}
