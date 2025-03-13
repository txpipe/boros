use std::{collections::HashSet, env, error::Error, path, sync::Arc};

use anyhow::Result;
use dotenv::dotenv;
use ledger::u5c::U5cDataAdapterImpl;
use network::peer_manager::PeerManagerConfig;
use queue::{chaining::TxChaining, DEFAULT_QUEUE};
use serde::Deserialize;
use storage::sqlite::{SqliteCursor, SqliteStorage, SqliteTransaction};
use tokio::try_join;
use tracing::Level;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use tx::validator::TxValidator;

mod ledger;
mod network;
mod pipeline;
mod queue;
mod server;
mod storage;
mod tx;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let env_filter = EnvFilter::builder()
        .with_default_directive(Level::INFO.into())
        .with_env_var("RUST_LOG")
        .from_env_lossy();

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(env_filter)
        .init();

    let config = Config::new().expect("invalid config file");

    let storage = Arc::new(SqliteStorage::new(path::Path::new(&config.storage.db_path)).await?);
    storage.migrate().await?;

    let tx_storage = Arc::new(SqliteTransaction::new(Arc::clone(&storage)));
    let tx_chaining = Arc::new(TxChaining::new(
        Arc::clone(&tx_storage),
        config.clone().queues,
    ));

    let cursor_storage = Arc::new(SqliteCursor::new(Arc::clone(&storage)));
    let cursor = cursor_storage.current().await?.map(|c| c.into());

    let u5c_data_adapter = Arc::new(U5cDataAdapterImpl::try_new(config.clone().u5c, cursor).await?);
    let tx_validator = Arc::new(TxValidator::new(u5c_data_adapter.clone()));

    let pipeline = pipeline::run(
        config.clone(),
        Arc::clone(&tx_storage),
        Arc::clone(&cursor_storage),
        Arc::clone(&tx_validator),
        Arc::clone(&u5c_data_adapter),
    );
    let server = server::run(
        config.server,
        Arc::clone(&tx_storage),
        Arc::clone(&tx_chaining),
        Arc::clone(&tx_validator),
    );

    try_join!(pipeline, server)?;

    Ok(())
}

#[derive(Deserialize, Clone)]
struct Config {
    server: server::Config,
    storage: storage::Config,
    peer_manager: PeerManagerConfig,
    monitor: pipeline::monitor::Config,
    #[serde(default)]
    queues: HashSet<queue::Config>,
    u5c: ledger::u5c::Config,
}

impl Config {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let mut config: Config = config::Config::builder()
            .add_source(
                config::File::with_name(&env::var("BOROS_CONFIG").unwrap_or("boros.toml".into()))
                    .required(false),
            )
            .add_source(config::File::with_name("/etc/boros/config.toml").required(false))
            .add_source(config::Environment::with_prefix("boros").separator("_"))
            .build()?
            .try_deserialize()?;

        (!config.queues.iter().any(|q| q.name == DEFAULT_QUEUE))
            .then(|| config.queues.insert(Default::default()));

        Ok(config)
    }
}
