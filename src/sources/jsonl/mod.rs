use crate::model::Request;
use core::time::Duration;
use gasket::{AsWorkError, OutputPort};
use std::io::BufRead;

pub struct Stage {
    lines: std::io::Lines<std::io::BufReader<std::fs::File>>,
    output: gasket::OutputPort<Request>,
}

impl gasket::Stage for Stage {
    fn describe(&self) -> gasket::StageDescription {
        gasket::StageDescription {
            name: "jsonl-source",
            tick_timeout: Some(Duration::from_secs(5)),
            metrics: vec![],
        }
    }

    fn work(&mut self) -> gasket::WorkResult {
        let next = self.lines.next();

        match next {
            Some(result) => {
                let line = result.or_work_err()?;
                let request: Request = serde_json::from_str(&line).or_work_err()?;
                self.output.send(request.into())?;
                Ok(gasket::WorkOutcome::Partial)
            }
            None => Ok(gasket::WorkOutcome::Done),
        }
    }
}

pub struct Config {
    pub path: String,
}

impl TryFrom<Config> for Stage {
    type Error = crate::Error;

    fn try_from(value: Config) -> Result<Self, Self::Error> {
        let file = std::fs::File::open(&value.path)
            .map_err(|err| crate::Error::ConfigError(err.to_string()))?;
        let buf = std::io::BufReader::new(file);

        Ok(Stage {
            lines: buf.lines(),
            output: OutputPort::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use gasket::StageState;

    use crate::model::{Record, Request};

    use super::{Config, Stage};

    #[test]
    fn stage_sends_correct_outputs() {
        let config = Config {
            path: Path::new("sample.jsonl").to_str().unwrap().to_owned(),
        };

        let mut stage = Stage::try_from(config).unwrap();

        let expected = vec![
            Request::AirDrop(Record {}),
            Request::SignedTx(Record {}),
            Request::AirDrop(Record {}),
            Request::SignedTx(Record {}),
        ];

        let tether = gasket::quick_output_test!(stage.output, expected);

        assert_eq!(tether.check_state(), StageState::Done);
    }
}
