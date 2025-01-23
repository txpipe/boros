use anyhow::Result;
use pallas::interop::utxorpc::spec as u5c;
use serde::Deserialize;
use std::net::SocketAddr;
use tonic::transport::Server;
use tracing::{error, info};

mod utxorpc;

pub async fn run(config: Config) -> Result<()> {
    tokio::spawn(async move {
        let reflection = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(u5c::submit::FILE_DESCRIPTOR_SET)
            .build_v1()
            .unwrap();

        let submit_service = utxorpc::SubmitServiceImpl {};
        let submit_service =
            u5c::submit::submit_service_server::SubmitServiceServer::new(submit_service);

        info!(address = config.addr.to_string(), "GRPC server running");

        let result = Server::builder()
            .add_service(reflection)
            .add_service(submit_service)
            .serve(config.addr)
            .await;

        if let Err(error) = result {
            error!(?error);
            std::process::exit(1);
        }
    });

    Ok(())
}

#[derive(Deserialize)]
pub struct Config {
    pub addr: SocketAddr,
}
