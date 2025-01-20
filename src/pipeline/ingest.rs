use std::time::Duration;

use gasket::framework::*;
use tokio::time::sleep;
use tracing::info;

use super::Transaction;

#[derive(Stage)]
#[stage(name = "ingest", unit = "Transaction", worker = "Worker")]
pub struct Stage {}

pub struct Worker;

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(_stage: &Stage) -> Result<Self, WorkerError> {
        Ok(Self)
    }

    async fn schedule(
        &mut self,
        _stage: &mut Stage,
    ) -> Result<WorkSchedule<Transaction>, WorkerError> {
        // TODO: fetch data from db
        sleep(Duration::from_secs(30)).await;
        Ok(WorkSchedule::Unit(Transaction {
            cbor: vec![0, 1, 2, 3],
        }))
    }

    async fn execute(
        &mut self,
        _unit: &Transaction,
        _stage: &mut Stage,
    ) -> Result<(), WorkerError> {
        info!("ingest");
        
        Ok(())
    }
}
