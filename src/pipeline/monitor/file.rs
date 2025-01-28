use std::{
    fs,
    io::{self, BufRead},
    path::Path,
    pin::Pin,
};

use futures::stream;

use super::{Event, MonitorAdapter};

pub struct FileMonitorAdapter {
    values: Vec<String>,
}
impl FileMonitorAdapter {
    pub fn new(file_path: &Path) -> anyhow::Result<Self> {
        let file = fs::File::open(file_path)?;
        let reader = io::BufReader::new(file);
        let values = reader.lines().collect::<io::Result<Vec<String>>>()?;

        Ok(Self { values })
    }
}
impl MonitorAdapter for FileMonitorAdapter {
    fn stream(&mut self) -> Pin<Box<dyn futures::Stream<Item = anyhow::Result<Event>>>> {
        let stream = stream::iter(self.values.clone().into_iter().map(|v| {
            let bytes = hex::decode(&v)?;
            Ok(Event::RollForward(bytes))
        }));

        Box::pin(stream)
    }
}
