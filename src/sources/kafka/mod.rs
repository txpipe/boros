use crate::model::Request;
use core::time::Duration;
use gasket::AsWorkError;

pub struct Stage {
    consumer: kafka::consumer::Consumer,
    output: gasket::OutputPort<Request>,
}

impl gasket::Stage for Stage {
    fn describe(&self) -> gasket::StageDescription {
        gasket::StageDescription {
            name: "kafka-consumer",
            tick_timeout: Some(Duration::from_secs(30)),
            metrics: vec![],
        }
    }

    fn work(&mut self) -> gasket::WorkResult {
        let poll = self.consumer.poll().or_work_err()?;

        for ms in poll.iter() {
            for m in ms.messages() {
                println!("{:?}", m);
            }

            self.consumer.consume_messageset(ms).or_work_err()?;
        }

        self.consumer.commit_consumed().or_work_err()?;

        Ok(gasket::WorkOutcome::Partial)
    }
}
