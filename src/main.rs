use std::{collections::HashSet, env, error::Error, path, sync::Arc};

use anyhow::Result;
use dotenv::dotenv;
use network::peer_manager::PeerManagerConfig;
use priority::DEFAULT_QUEUE;
use serde::Deserialize;
use storage::sqlite::{SqliteCursor, SqliteStorage, SqliteTransaction};
use tokio::try_join;
use tracing::Level;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod ledger;
mod network;
mod pipeline;
mod priority;
mod server;
mod storage;

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

    let tx_storage = Arc::new(SqliteTransaction::new(storage.clone()));
    let cursor_storage = Arc::new(SqliteCursor::new(storage.clone()));

    let pipeline = pipeline::run(config.clone(), tx_storage.clone(), cursor_storage.clone());
    let server = server::run(config.server, tx_storage.clone());

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
    queues: HashSet<priority::QueueConfig>,
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
