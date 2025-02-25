use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use pallas::interop::utxorpc::spec as u5c;
use serde::Deserialize;
use spec::boros::v1 as spec;
use tonic::transport::Server;
use tracing::{error, info};

use crate::storage::sqlite::SqliteTransaction;

mod state;
mod utxorpc;

pub async fn run(config: Config, tx_storage: Arc<SqliteTransaction>) -> Result<()> {
    tokio::spawn(async move {
        let reflection = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(u5c::submit::FILE_DESCRIPTOR_SET)
            .register_encoded_file_descriptor_set(u5c::cardano::FILE_DESCRIPTOR_SET)
            .register_encoded_file_descriptor_set(protoc_wkt::google::protobuf::FILE_DESCRIPTOR_SET)
            .register_encoded_file_descriptor_set(spec::FILE_DESCRIPTOR_SET)
            .build_v1alpha()
            .unwrap();

        let submit_service = utxorpc::SubmitServiceImpl::new(tx_storage);
        let submit_service =
            u5c::submit::submit_service_server::SubmitServiceServer::new(submit_service);

        let state_service = state::StateServiceImpl::new();
        let state_service = spec::state_service_server::StateServiceServer::new(state_service);

        info!(
            address = config.listen_address.to_string(),
            "GRPC server running"
        );

        let result = Server::builder()
            .add_service(reflection)
            .add_service(submit_service)
            .add_service(state_service)
            .serve(config.listen_address)
            .await;

        if let Err(error) = result {
            error!(?error);
            std::process::exit(1);
        }
    });

    Ok(())
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub listen_address: SocketAddr,
}
