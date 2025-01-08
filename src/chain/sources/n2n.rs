use std::time::Duration;

use gasket::{framework::*, messaging::Message};
use serde::Deserialize;
use tokio::time::sleep;
use tracing::info;

use crate::chain::{Event, SourceOutputPort};

#[derive(Stage)]
#[stage(name = "n2n", unit = "Event", worker = "Worker")]
pub struct Stage {
    config: Config,
    pub output: SourceOutputPort,
}

#[derive(Deserialize)]
pub struct Config {
    pub peer: String,
}
impl Config {
    pub fn bootstrapper(self) -> Stage {
        let stage = Stage {
            config: self,
            output: Default::default(),
        };

        stage
    }
}

pub struct Worker;

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(stage: &Stage) -> Result<Self, WorkerError> {
        info!("peer connection {}", stage.config.peer);
        Ok(Self)
    }

    async fn schedule(&mut self, _stage: &mut Stage) -> Result<WorkSchedule<Event>, WorkerError> {
        sleep(Duration::from_secs(20)).await;
        Ok(WorkSchedule::Unit(Event::CborBlock(Default::default())))
    }

    async fn execute(&mut self, unit: &Event, stage: &mut Stage) -> Result<(), WorkerError> {
        info!("new block");

        stage
            .output
            .send(Message {
                payload: unit.clone(),
            })
            .await
            .or_panic()?;

        Ok(())
    }
}
