use std::time::Duration;

use gasket::framework::*;
use tokio::time::sleep;

use super::{Event, ValidationInputPort, ValidationOutputPort};

#[derive(Stage)]
#[stage(name = "validation", unit = "Event", worker = "Worker")]
pub struct Stage {
    pub input: ValidationInputPort,
    pub output: ValidationOutputPort,
}

#[derive(Default)]
pub struct Worker;
impl From<&Stage> for Worker {
    fn from(_: &Stage) -> Self {
        Self
    }
}

gasket::impl_mapper!(|_worker: Worker, stage: Stage, unit: Event| => {
    let output = unit.clone();

    dbg!("validation stage");
    sleep(Duration::from_secs(5)).await;

    output
});
