use std::{sync::Arc, time::Duration};

use gasket::framework::*;
use tokio::time::sleep;
use tracing::info;

use crate::{
    priority::{self, distribute_quote},
    storage::{sqlite::SqliteTransaction, Transaction, TransactionStatus},
};

#[derive(Stage)]
#[stage(name = "ingest", unit = "Vec<Transaction>", worker = "Worker")]
pub struct Stage {
    storage: Arc<SqliteTransaction>,
    priority: priority::Config,
}

impl Stage {
    pub fn new(storage: Arc<SqliteTransaction>, priority: priority::Config) -> Self {
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
        let dist_quote = distribute_quote(stage.priority.clone());

        let mut leftover = 0;
        let mut transactions: Vec<Transaction> = Vec::new();

        for (queue, quote) in dist_quote {
            let current_quote = quote + leftover;

            let mut quote_transactions = stage
                .storage
                .next_quote(TransactionStatus::Pending, &queue, current_quote)
                .await
                .or_retry()?;

            let used_quote = transactions.len();
            leftover = current_quote - used_quote;

            transactions.append(&mut quote_transactions);
        }

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
