use std::{sync::Arc, time::Duration};

use gasket::framework::*;
use tokio::time::sleep;
use tracing::info;

use crate::storage::{sqlite::SqliteTransaction, Transaction, TransactionStatus};

#[derive(Stage)]
#[stage(name = "ingest", unit = "Transaction", worker = "Worker")]
pub struct Stage {
    storage: Arc<SqliteTransaction>,
}

impl Stage {
    pub fn new(storage: Arc<SqliteTransaction>) -> Self {
        Self { storage }
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
    ) -> Result<WorkSchedule<Transaction>, WorkerError> {
        if let Some(tx) = stage
            .storage
            .next(TransactionStatus::Pending)
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

        info!("ingest {}", transaction.id);

        transaction.status = TransactionStatus::Validated;
        stage.storage.update(&transaction).await.or_retry()?;

        Ok(())
    }
}
