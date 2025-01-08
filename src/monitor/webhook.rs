use gasket::framework::*;
use serde::Deserialize;

use super::{Event, HookInputPort};

#[derive(Stage)]
#[stage(name = "webhook", unit = "Event", worker = "Worker")]
pub struct Stage {
    config: Config,
    pub input: HookInputPort,
}

#[derive(Deserialize)]
pub struct Config {
    pub url: String,
}
impl Config {
    pub fn bootstrapper(self) -> Stage {
        let stage = Stage {
            config: self,
            input: Default::default(),
        };

        stage
    }
}

pub struct Worker;

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(_stage: &Stage) -> Result<Self, WorkerError> {
        Ok(Self)
    }

    async fn schedule(&mut self, stage: &mut Stage) -> Result<WorkSchedule<Event>, WorkerError> {
        let evt = stage.input.recv().await.or_panic()?;
        Ok(WorkSchedule::Unit(evt.payload))
    }

    async fn execute(&mut self, _unit: &Event, _stage: &mut Stage) -> Result<(), WorkerError> {
        dbg!("webhook stage");
        Ok(())
    }
}
