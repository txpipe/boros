use std::{path::Path, sync::Arc};

use anyhow::Result;
use monitor::file::FileMonitorAdapter;

use crate::{storage::sqlite::SqliteTransaction, Config};

pub mod fanout;
pub mod ingest;
pub mod monitor;

pub async fn run(config: Config, tx_storage: Arc<SqliteTransaction>) -> Result<()> {
    let ingest = ingest::Stage::new(tx_storage.clone());
    let fanout = fanout::Stage::new(tx_storage.clone(), config.peer_manager);

    let adapter = FileMonitorAdapter::new(Path::new("test/blocks"))?;
    let monitor = monitor::Stage::new(Box::new(adapter));

    let policy: gasket::runtime::Policy = Default::default();

    let ingest = gasket::runtime::spawn_stage(ingest, policy.clone());
    let fanout = gasket::runtime::spawn_stage(fanout, policy.clone());
    let monitor = gasket::runtime::spawn_stage(monitor, policy.clone());

    let daemon = gasket::daemon::Daemon::new(vec![ingest, fanout, monitor]);
    daemon.block();

    Ok(())
}
