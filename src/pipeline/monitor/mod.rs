use std::time::Duration;

use gasket::framework::*;
use tokio::time::sleep;
use tracing::info;

pub struct Block;

#[derive(Stage)]
#[stage(name = "monitor", unit = "Block", worker = "Worker")]
pub struct Stage {}

pub struct Worker;

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(_stage: &Stage) -> Result<Self, WorkerError> {
        Ok(Self)
    }

    async fn schedule(&mut self, _stage: &mut Stage) -> Result<WorkSchedule<Block>, WorkerError> {
        // TODO: fetch data from network
        sleep(Duration::from_secs(30)).await;
        Ok(WorkSchedule::Unit(Block {}))
    }

    async fn execute(&mut self, _unit: &Block, _stage: &mut Stage) -> Result<(), WorkerError> {
        info!("monitor");

        Ok(())
    }
}
