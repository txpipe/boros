use std::{env, error::Error};

use anyhow::Result;
use serde::Deserialize;
use tokio::try_join;
use tracing::Level;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod pipeline;
mod server;
mod storage;

#[tokio::main]
async fn main() -> Result<()> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(Level::INFO.into())
        .with_env_var("RUST_LOG")
        .from_env_lossy();

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(env_filter)
        .init();

    let config = Config::new().expect("invalid config file");

    let cbor_txs_db = storage::in_memory_db::CborTransactionsDb::new();

    let pipeline = pipeline::run(cbor_txs_db.clone(), config);
    let server = server::run();

    try_join!(pipeline, server)?;

    Ok(())
}

#[derive(Deserialize)]
struct PeerManager {
    peers: Vec<String>,
}

#[derive(Deserialize)]
struct Config {
    peer_manager: PeerManager,
}

impl Config {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let config = config::Config::builder()
            .add_source(
                config::File::with_name(&env::var("BOROS_CONFIG").unwrap_or("boros.toml".into()))
                    .required(false),
            )
            .add_source(config::Environment::with_prefix("boros").separator("_"))
            .build()?
            .try_deserialize()?;

        Ok(config)
    }
}
