use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use anyhow::{Error, Result};
use jsonrpsee::server::{RpcModule, Server};
use serde::Deserialize;
use tokio::try_join;
use tokio_util::sync::CancellationToken;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::{ledger::u5c::U5cDataAdapter, storage::sqlite::SqliteTransaction};

mod methods;

#[derive(Clone)]
pub struct Context {
    config: Config,
    tx_storage: Arc<SqliteTransaction>,
    u5c_adapter: Arc<dyn U5cDataAdapter>,
}

pub async fn run(
    config: Config,
    tx_storage: Arc<SqliteTransaction>,
    u5c_adapter: Arc<dyn U5cDataAdapter>,
    cancellation_token: CancellationToken,
) -> Result<()> {
    let cors_layer = if config.permissive_cors.unwrap_or_default() {
        CorsLayer::permissive()
    } else {
        CorsLayer::new()
    };

    let middleware = ServiceBuilder::new().layer(cors_layer);
    let server = Server::builder()
        .set_http_middleware(middleware)
        .build(config.listen_address)
        .await?;

    let mut module = RpcModule::new(Context {
        config: config.clone(),
        tx_storage: tx_storage.clone(),
        u5c_adapter: u5c_adapter.clone(),
    });

    module.register_async_method("trp.resolve", |params, context, _| async {
        methods::trp_resolve(params, context).await
    })?;
    module.register_async_method("trp.submit", |params, context, _| async {
        methods::trp_submit(params, context).await
    })?;

    module.register_async_method("health", |_, context, _| async {
        methods::health(context).await
    })?;

    info!(
        address = config.listen_address.to_string(),
        "TRP server running"
    );

    let handle = server.start(module);

    let server = async {
        handle.clone().stopped().await;
        Ok::<(), Error>(())
    };

    let cancellation = async {
        cancellation_token.cancelled().await;
        info!("gracefully shuting down trp");
        let _ = handle.stop(); // Empty result with AlreadyStoppedError, can be ignored.
        Ok::<(), Error>(())
    };

    try_join!(server, cancellation)?;

    Ok(())
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub uri: String,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub listen_address: SocketAddr,
    pub permissive_cors: Option<bool>,
}
