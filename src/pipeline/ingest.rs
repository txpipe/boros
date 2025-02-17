use std::{sync::Arc, time::Duration};

use gasket::framework::*;
use tokio::time::sleep;
use tracing::info;

use crate::{
    priority::Priority,
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
            .next(TransactionStatus::Pending)
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
        _unit: &Vec<Transaction>,
        _stage: &mut Stage,
    ) -> Result<(), WorkerError> {
        //todo!();

        info!("ingest");
        //let mut transaction = unit.clone();
        //
        //transaction.status = TransactionStatus::Validated;
        //stage.storage.update(&transaction).await.or_retry()?;
        //
        Ok(())
    }
}
