use anyhow::Result;
use axum::{http::StatusCode, response::IntoResponse, Json};
use gasket::messaging::tokio::ChannelSendAdapter;
use serde::Deserialize;
use tokio::try_join;
use tracing::info;

use crate::monitor;

mod fanout;
mod ingest;

pub async fn run(config: Config, monitor_sender: ChannelSendAdapter<monitor::Event>) -> Result<()> {
    let server = server(config.clone());
    let pipeline = pipeline(config.clone(), monitor_sender);

    try_join!(server, pipeline)?;

    Ok(())
}

async fn server(_config: Config) -> Result<()> {
    let addr = "0.0.0.0:5000";
    let app = axum::Router::new().route(
        "/tx",
        axum::routing::post({
            |Json(input): Json<TxRequest>| async move {
                dbg!(input.tx);

                (StatusCode::ACCEPTED).into_response()
            }
        }),
    );

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    info!(address = addr, "Http server running");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn pipeline(
    _config: Config,
    monitor_sender: ChannelSendAdapter<monitor::Event>,
) -> Result<()> {
    tokio::spawn(async {
        let ingest = ingest::Stage {
            monitor: monitor_sender.clone(),
        };

        let fanout = fanout::Stage {
            monitor: monitor_sender,
        };

        let policy: gasket::runtime::Policy = Default::default();

        let ingest = gasket::runtime::spawn_stage(ingest, policy.clone());
        let fanout = gasket::runtime::spawn_stage(fanout, policy.clone());

        let daemon = gasket::daemon::Daemon::new(vec![ingest, fanout]);
        daemon.block();
    })
    .await?;

    Ok(())
}

#[derive(Deserialize, Clone)]
pub struct Config {}

#[derive(Debug)]
pub struct Transaction {}

#[derive(Debug, Deserialize)]
struct TxRequest {
    tx: String,
}
