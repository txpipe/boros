use std::{
    fs,
    io::{self, BufRead},
    path::Path,
    time::Duration,
};

use async_stream::stream;
use pallas::ledger::traverse::MultiEraBlock;
use tokio::time::sleep;

use super::{ChainSyncAdapter, ChainSyncStream, Event};

pub struct FileChainSyncAdapter {
    values: Vec<String>,
}
impl FileChainSyncAdapter {
    pub fn try_new() -> anyhow::Result<Self> {
        let file = fs::File::open(Path::new("test/blocks"))?;
        let reader = io::BufReader::new(file);
        let values = reader.lines().collect::<io::Result<Vec<String>>>()?;

        Ok(Self { values })
    }
}

#[async_trait::async_trait]
impl ChainSyncAdapter for FileChainSyncAdapter {
    async fn stream(&mut self) -> anyhow::Result<ChainSyncStream> {
        let values = self.values.clone();

        let stream = stream! {
            for item in values.into_iter() {
                let bytes = hex::decode(&item)?;
                let block = MultiEraBlock::decode(&bytes)?;
                let _txs = block.txs();
                // TODO: map pallas tx to u5c tx

                yield Ok(Event::RollForward(Default::default()));
                sleep(Duration::from_secs(20)).await;
            }
        };

        Ok(Box::pin(stream))
    }
}
