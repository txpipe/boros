use gasket::framework::*;
use tracing::info;

use super::{Event, ValidationInputPort, ValidationOutputPort};

#[derive(Stage)]
#[stage(name = "validation", unit = "Event", worker = "Worker")]
pub struct Stage {
    pub input: ValidationInputPort,
    pub output: ValidationOutputPort,
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
        info!("validation stage");

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
