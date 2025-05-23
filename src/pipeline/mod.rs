use std::sync::Arc;

use anyhow::Result;
use gasket::messaging::tokio::ChannelSendAdapter;

use crate::{
    ledger::{
        relay::{MockRelayDataAdapter, RelayDataAdapter},
        u5c::{Point, U5cDataAdapter},
    },
    network::peer_manager::PeerManager,
    queue::priority::Priority,
    signing::{hashicorp::HashicorpVaultClient, SigningAdapter},
    storage::{
        sqlite::{SqliteCursor, SqliteTransaction},
        Cursor,
    },
    Config,
};

pub mod ingest;
pub mod monitor;
pub mod peer_discovery;

const CAP: u16 = 50;

pub async fn run(
    config: Config,
    u5c_data_adapter: Arc<dyn U5cDataAdapter>,
    tx_storage: Arc<SqliteTransaction>,
    cursor_storage: Arc<SqliteCursor>,
) -> Result<()> {
    let relay_adapter: Arc<dyn RelayDataAdapter + Send + Sync> =
        Arc::new(MockRelayDataAdapter::new());

    let (sender, _) = gasket::messaging::tokio::broadcast_channel::<Vec<u8>>(CAP as usize);

    let broadcast_sender = match &sender {
        ChannelSendAdapter::Broadcast(sender) => sender.clone(),
        _ => panic!("Expected broadcast sender"),
    };

    let peer_addrs = config.peer_manager.peers.clone();
    let peer_manager = PeerManager::new(2, peer_addrs, broadcast_sender);

    peer_manager.init().await?;

    let peer_manager = Arc::new(peer_manager);

    let priority = Arc::new(Priority::new(tx_storage.clone(), config.queues.clone()));
    let signing_adapter: Option<Arc<dyn SigningAdapter>> = config
        .signing
        .as_ref()
        .map(|cfg| {
            HashicorpVaultClient::new(cfg.clone())
                .map(|c| Arc::new(c) as Arc<dyn SigningAdapter>)
        })
        .transpose()?;

    let mut ingest = ingest::Stage::new(
        tx_storage.clone(),
        priority.clone(),
        u5c_data_adapter.clone(),
        signing_adapter,
        config.clone(),
    );

    ingest.output.connect(sender);

    let monitor = monitor::Stage::new(
        config.monitor,
        u5c_data_adapter.clone(),
        tx_storage.clone(),
        cursor_storage.clone(),
    );
    let peer_discovery = peer_discovery::Stage::new(
        config.peer_manager.clone(),
        peer_manager.clone(),
        relay_adapter,
    );

    let policy: gasket::runtime::Policy = Default::default();

    let ingest = gasket::runtime::spawn_stage(ingest, policy.clone());
    let monitor = gasket::runtime::spawn_stage(monitor, policy.clone());
    let peer_discovery = gasket::runtime::spawn_stage(peer_discovery, policy.clone());

    let daemon = gasket::daemon::Daemon::new(vec![ingest, monitor, peer_discovery]);
    daemon.block();

    Ok(())
}

impl From<Cursor> for Point {
    fn from(value: Cursor) -> Self {
        (value.slot, value.hash)
    }
}
