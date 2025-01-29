use std::{
    fs,
    io::{self, BufRead},
    path::Path,
    pin::Pin,
    time::Duration,
};

use async_stream::stream;
use tokio::time::sleep;

use super::{Event, MonitorAdapter};

pub struct FileMonitorAdapter {
    values: Vec<String>,
}
impl FileMonitorAdapter {
    pub fn try_new() -> anyhow::Result<Self> {
        let file = fs::File::open(Path::new("test/blocks"))?;
        let reader = io::BufReader::new(file);
        let values = reader.lines().collect::<io::Result<Vec<String>>>()?;

        Ok(Self { values })
    }
}
impl MonitorAdapter for FileMonitorAdapter {
    fn stream(&self) -> Pin<Box<dyn futures::Stream<Item = anyhow::Result<Event>>>> {
        let values = self.values.clone();

        let stream = stream! {
            for item in values.into_iter() {
                let bytes = hex::decode(&item)?;
                yield Ok(Event::RollForward(bytes));
                sleep(Duration::from_secs(20)).await;
            }
        };

        Box::pin(stream)
    }
}
