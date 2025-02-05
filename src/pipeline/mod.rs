use std::sync::Arc;

use anyhow::Result;

use crate::{
    ledger::u5c::{Point, U5cDataAdapterImpl},
    storage::{
        sqlite::{SqliteCursor, SqliteTransaction},
        Cursor,
    },
    Config,
};

pub mod fanout;
pub mod ingest;
pub mod monitor;

pub async fn run(
    config: Config,
    tx_storage: Arc<SqliteTransaction>,
    cursor_storage: Arc<SqliteCursor>,
) -> Result<()> {
    let cursor = cursor_storage.current().await?.map(|c| c.into());
    let adapter = Arc::new(U5cDataAdapterImpl::try_new(config.u5c, cursor).await?);

    let ingest = ingest::Stage::new(tx_storage.clone());
    let fanout = fanout::Stage::new(config.peer_manager, adapter.clone(), tx_storage.clone());

    let monitor = monitor::Stage::new(
        config.monitor,
        adapter.clone(),
        tx_storage.clone(),
        cursor_storage.clone(),
    );

    let policy: gasket::runtime::Policy = Default::default();

    let ingest = gasket::runtime::spawn_stage(ingest, policy.clone());
    let fanout = gasket::runtime::spawn_stage(fanout, policy.clone());
    let monitor = gasket::runtime::spawn_stage(monitor, policy.clone());

    let daemon = gasket::daemon::Daemon::new(vec![ingest, fanout, monitor]);
    daemon.block();

    Ok(())
}

impl From<Cursor> for Point {
    fn from(value: Cursor) -> Self {
        (value.slot, value.hash)
    }
}
