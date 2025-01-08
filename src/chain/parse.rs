use gasket::framework::*;

use super::{Event, ParseInputPort, ParseOutputPort};

#[derive(Stage)]
#[stage(name = "parse", unit = "Event", worker = "Worker")]
pub struct Stage {
    pub input: ParseInputPort,
    pub output: ParseOutputPort,
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

    async fn execute(&mut self, unit: &Event, stage: &mut Stage) -> Result<(), WorkerError> {
        dbg!("parse stage");

        stage
            .output
            .send(gasket::messaging::Message {
                payload: unit.clone(),
            })
            .await
            .or_panic()?;

        Ok(())
    }
}
