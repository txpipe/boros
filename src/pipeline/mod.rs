use anyhow::Result;

use crate::{storage::in_memory_db::CborTransactionsDb, Config};

pub mod fanout;
pub mod ingest;
pub mod monitor;

#[derive(Debug)]
pub struct Transaction {
    pub cbor: Vec<u8>,
}

pub async fn run(cbor_txs_db: CborTransactionsDb, config: Config) -> Result<()> {
    tokio::spawn(async {
        let ingest = ingest::Stage {};
        let fanout = fanout::Stage { cbor_txs_db: cbor_txs_db, config: config };
        let monitor = monitor::Stage {};

        let policy: gasket::runtime::Policy = Default::default();

        let ingest = gasket::runtime::spawn_stage(ingest, policy.clone());
        let fanout = gasket::runtime::spawn_stage(fanout, policy.clone());
        let monitor = gasket::runtime::spawn_stage(monitor, policy.clone());

        let daemon = gasket::daemon::Daemon::new(vec![ingest, fanout, monitor]);
        daemon.block();
    })
    .await?;

    Ok(())
}
