use std::sync::Arc;

use anyhow::Result;
use tokio::sync::broadcast;

use crate::{
    ledger::{
        relay::{MockRelayDataAdapter, RelayDataAdapter},
        u5c::{Point, U5cDataAdapterImpl},
    },
    priority::Priority,
    storage::{
        sqlite::{SqliteCursor, SqliteTransaction},
        Cursor,
    },
    Config,
};

use crate::peer::peer_manager::PeerManager;

pub mod ingest;
pub mod monitor;
pub mod peersharing;

const CAP: u16 = 50;

pub async fn run(
    config: Config,
    tx_storage: Arc<SqliteTransaction>,
    cursor_storage: Arc<SqliteCursor>,
) -> Result<()> {
    let cursor = cursor_storage.current().await?.map(|c| c.into());
    let _relay_adapter: Arc<dyn RelayDataAdapter + Send + Sync> =
        Arc::new(MockRelayDataAdapter::new());
    let u5c_data_adapter = Arc::new(U5cDataAdapterImpl::try_new(config.u5c, cursor).await?);

    let (sender, _) = broadcast::channel::<Vec<u8>>(CAP as usize);
    let receiver = sender.subscribe();

    let peer_addrs = config.peer_manager.peers.clone();
    let peer_manager = PeerManager::new(2, peer_addrs, receiver);

    peer_manager.init().await?;
    
    let peer_manager = Arc::new(peer_manager);

    let priority = Arc::new(Priority::new(tx_storage.clone(), config.queues));

    let ingest = ingest::Stage::new(tx_storage.clone(), priority.clone(), sender);
    let monitor = monitor::Stage::new(
        config.monitor,
        u5c_data_adapter.clone(),
        tx_storage.clone(),
        cursor_storage.clone(),
    );
    let _peersharing = peersharing::Stage::new(
        config.peer_manager.clone(),
        peer_manager.clone(),
        _relay_adapter,
    );

    let policy: gasket::runtime::Policy = Default::default();

    let ingest = gasket::runtime::spawn_stage(ingest, policy.clone());
    let monitor = gasket::runtime::spawn_stage(monitor, policy.clone());

    let daemon = gasket::daemon::Daemon::new(vec![ingest, monitor]);
    daemon.block();

    Ok(())
}

impl From<Cursor> for Point {
    fn from(value: Cursor) -> Self {
        (value.slot, value.hash)
    }
}
