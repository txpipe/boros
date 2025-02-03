use std::pin::Pin;

use futures::{Stream, TryStreamExt};
use gasket::framework::*;
use pallas::interop::utxorpc::spec::cardano::Tx;
use tracing::info;

pub mod u5c;

#[cfg(test)]
pub mod file;

#[derive(Debug)]
pub enum Event {
    RollForward(Vec<Tx>),
    Rollback(u64, Vec<u8>),
}

type ChainSyncStream = Pin<Box<dyn Stream<Item = anyhow::Result<Event>> + Send>>;

#[async_trait::async_trait]
pub trait ChainSyncAdapter {
    async fn stream(&mut self) -> anyhow::Result<ChainSyncStream>;
}

#[derive(Stage)]
#[stage(name = "monitor", unit = "Event", worker = "Worker")]
pub struct Stage {
    stream: ChainSyncStream,
}
impl Stage {
    pub async fn try_new(mut adapter: Box<dyn ChainSyncAdapter>) -> anyhow::Result<Self> {
        let stream = adapter.stream().await?;
        Ok(Self { stream })
    }
}

pub struct Worker;

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(_stage: &Stage) -> Result<Self, WorkerError> {
        Ok(Self)
    }

    async fn schedule(&mut self, stage: &mut Stage) -> Result<WorkSchedule<Event>, WorkerError> {
        if let Some(e) = stage.stream.try_next().await.or_restart()? {
            return Ok(WorkSchedule::Unit(e));
        }

        Ok(WorkSchedule::Idle)
    }

    async fn execute(&mut self, unit: &Event, _stage: &mut Stage) -> Result<(), WorkerError> {
        match unit {
            Event::RollForward(v) => info!("RollForward {} txs", v.len()),
            Event::Rollback(slot, _hash) => info!("Rollback slot {slot}"),
        };

        Ok(())
    }
}
