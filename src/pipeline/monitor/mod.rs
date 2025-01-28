use std::pin::Pin;

use futures::{Stream, TryStreamExt};
use gasket::framework::*;
use tracing::info;

pub mod file;

#[derive(Debug)]
pub enum Event {
    RollForward(Vec<u8>),
}

pub trait MonitorAdapter {
    fn stream(&mut self) -> Pin<Box<dyn Stream<Item = anyhow::Result<Event>>>>;
}

#[derive(Stage)]
#[stage(name = "monitor", unit = "Event", worker = "Worker")]
pub struct Stage {
    adapter: Box<dyn MonitorAdapter + Send>,
}
impl Stage {
    pub fn new(adapter: Box<dyn MonitorAdapter + Send>) -> Self {
        Self { adapter }
    }
}

pub struct Worker;

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(_stage: &Stage) -> Result<Self, WorkerError> {
        Ok(Self)
    }

    async fn schedule(&mut self, stage: &mut Stage) -> Result<WorkSchedule<Event>, WorkerError> {
        info!("monitor waiting next event");
        if let Some(e) = stage.adapter.stream().try_next().await.or_restart()? {
            return Ok(WorkSchedule::Unit(e));
        }

        Ok(WorkSchedule::Idle)
    }

    async fn execute(&mut self, unit: &Event, _stage: &mut Stage) -> Result<(), WorkerError> {
        info!("monitor");

        match unit {
            Event::RollForward(v) => info!("RollForward {}", hex::encode(v)),
        };

        Ok(())
    }
}
