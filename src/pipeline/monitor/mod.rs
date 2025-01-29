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
    fn stream(&self) -> Pin<Box<dyn Stream<Item = anyhow::Result<Event>>>>;
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

pub struct Worker {
    stream: Pin<Box<dyn Stream<Item = anyhow::Result<Event>>>>,
}

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(stage: &Stage) -> Result<Self, WorkerError> {
        let stream = stage.adapter.stream();
        Ok(Self { stream })
    }

    async fn schedule(&mut self, _stage: &mut Stage) -> Result<WorkSchedule<Event>, WorkerError> {
        if let Some(e) = self.stream.try_next().await.or_restart()? {
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
