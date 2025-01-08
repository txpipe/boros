use std::sync::Arc;

use anyhow::Result;
use axum::{http::StatusCode, response::IntoResponse};
use gasket::messaging::{
    tokio::{ChannelRecvAdapter, ChannelSendAdapter},
    Message, SendAdapter,
};
use serde::Deserialize;
use tokio::{sync::Mutex, try_join};
use tracing::{error, info};

use crate::monitor;

mod submit;
mod validation;

#[derive(Debug, Clone)]
pub enum Event {
    RawTx(String),
}

pub type ValidationInputPort = gasket::messaging::InputPort<Event>;
pub type ValidationOutputPort = gasket::messaging::OutputPort<Event>;
pub type SubmitInputPort = gasket::messaging::InputPort<Event>;

#[derive(Deserialize, Clone)]
pub struct SubmissionConfig {}

pub async fn run(
    config: SubmissionConfig,
    monitor_sender: ChannelSendAdapter<monitor::Event>,
) -> Result<()> {
    let (server_sender, server_receiver) = gasket::messaging::tokio::mpsc_channel::<Event>(50);

    let server = server(config.clone(), server_sender);
    let pipeline = pipeline(config.clone(), server_receiver, monitor_sender);

    try_join!(server, pipeline)?;

    Ok(())
}

async fn server(_config: SubmissionConfig, sender: ChannelSendAdapter<Event>) -> Result<()> {
    let sender = Arc::new(Mutex::new(sender));
    let addr = "0.0.0.0:5000";
    let app = axum::Router::new().route(
        "/tx",
        axum::routing::post({
            let sender = sender.clone();
            || async move {
                let mut f = sender.lock().await;

                if let Err(error) = f
                    .send(Message {
                        payload: Event::RawTx(String::from("xx")),
                    })
                    .await
                {
                    error!(?error);
                    return (StatusCode::INTERNAL_SERVER_ERROR).into_response();
                }

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
    _config: SubmissionConfig,
    server_receiver: ChannelRecvAdapter<Event>,
    monitor_sender: ChannelSendAdapter<monitor::Event>,
) -> Result<()> {
    tokio::spawn(async {
        let mut validation = validation::Stage {
            input: Default::default(),
            output: Default::default(),
        };

        validation.input.connect(server_receiver);

        let mut submit = submit::Stage {
            input: Default::default(),
            monitor: monitor_sender,
        };
        gasket::messaging::tokio::connect_ports(&mut validation.output, &mut submit.input, 100);

        let policy: gasket::runtime::Policy = Default::default();

        let validation = gasket::runtime::spawn_stage(validation, policy.clone());
        let submit = gasket::runtime::spawn_stage(submit, policy.clone());

        let daemon = gasket::daemon::Daemon::new(vec![validation, submit]);
        daemon.block();
    })
    .await?;

    Ok(())
}
