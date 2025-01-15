use std::time::Duration;

use gasket::{
    framework::*,
    messaging::{tokio::ChannelSendAdapter, SendAdapter},
};
use tokio::time::sleep;
use tracing::info;

use super::Transaction;
use crate::monitor;

#[derive(Stage)]
#[stage(name = "ingest", unit = "Transaction", worker = "Worker")]
pub struct Stage {
    pub monitor: ChannelSendAdapter<monitor::Event>,
}

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
        Ok(WorkSchedule::Unit(Transaction {}))
    }

    async fn execute(&mut self, _unit: &Transaction, stage: &mut Stage) -> Result<(), WorkerError> {
        info!("fanout");

        stage
            .monitor
            .send(gasket::messaging::Message {
                payload: monitor::Event {},
            })
            .await
            .or_panic()?;

        Ok(())
    }
}
