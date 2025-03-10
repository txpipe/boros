use std::sync::Arc;

use anyhow::Result;

use crate::{
    ledger::{
        relay::{MockRelayDataAdapter, RelayDataAdapter},
        u5c::{Point, U5cDataAdapterImpl},
    },
    queue::priority::Priority,
    storage::{
        sqlite::{SqliteCursor, SqliteTransaction},
        Cursor,
    },
    Config,
};

pub mod fanout;
pub mod ingest;
pub mod monitor;

const CAP: u16 = 50;

pub async fn run(
    config: Config,
    tx_storage: Arc<SqliteTransaction>,
    cursor_storage: Arc<SqliteCursor>,
) -> Result<()> {
    let cursor = cursor_storage.current().await?.map(|c| c.into());
    let relay_adapter: Arc<dyn RelayDataAdapter + Send + Sync> =
        Arc::new(MockRelayDataAdapter::new());
    let u5c_data_adapter = Arc::new(U5cDataAdapterImpl::try_new(config.u5c, cursor).await?);

    let priority = Arc::new(Priority::new(tx_storage.clone(), config.queues));

    let ingest = ingest::Stage::new(tx_storage.clone(), priority.clone());
    let fanout = fanout::Stage::new(
        config.peer_manager,
        relay_adapter.clone(),
        u5c_data_adapter.clone(),
        tx_storage.clone(),
        priority.clone(),
    );

    let monitor = monitor::Stage::new(
        config.monitor,
        u5c_data_adapter.clone(),
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
