use std::{env, error::Error};

use anyhow::Result;
use serde::Deserialize;
use tokio::try_join;
use tracing::Level;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod chain;
mod monitor;
mod submission;

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

    let (sender, receiver) = gasket::messaging::tokio::mpsc_channel::<monitor::Event>(50);

    let monitor = monitor::run(config.monitor, receiver);
    let submission = submission::run(config.submission, sender.clone());
    let chain = chain::run(config.chain, sender);

    try_join!(monitor, submission, chain)?;

    Ok(())
}

#[derive(Deserialize)]
struct Config {
    submission: submission::Config,
    monitor: monitor::Config,
    chain: chain::Config,
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
