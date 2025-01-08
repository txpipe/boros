use gasket::{
    framework::*,
    messaging::{tokio::ChannelSendAdapter, SendAdapter},
};

use crate::monitor;

use super::{ConfirmationInputPort, Event};

#[derive(Stage)]
#[stage(name = "confirmation", unit = "Event", worker = "Worker")]
pub struct Stage {
    pub input: ConfirmationInputPort,
    pub monitor: ChannelSendAdapter<monitor::Event>,
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

    async fn execute(&mut self, _unit: &Event, stage: &mut Stage) -> Result<(), WorkerError> {
        dbg!("confirmation stage");

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
