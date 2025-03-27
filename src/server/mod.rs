use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use bip39::Mnemonic;
use serde::Deserialize;
use spec::boros::v1 as spec;
use tonic::transport::Server;
use tracing::{error, info};

use crate::{
    ledger::u5c::U5cDataAdapter, queue::chaining::TxChaining,
    storage::sqlite::SqliteTransaction, signing::SecretAdapter,
};

mod submit;

pub async fn run(
    config: Config,
    u5c_adapter: Arc<dyn U5cDataAdapter>,
    secret_adapter: Arc<dyn SecretAdapter<Mnemonic>>,
    tx_storage: Arc<SqliteTransaction>,
    tx_chaining: Arc<TxChaining>,
) -> Result<()> {
    tokio::spawn(async move {
        let reflection = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(protoc_wkt::google::protobuf::FILE_DESCRIPTOR_SET)
            .register_encoded_file_descriptor_set(spec::submit::FILE_DESCRIPTOR_SET)
            .build_v1alpha()
            .unwrap();

        let submit_service = submit::SubmitServiceImpl::new(
            Arc::clone(&tx_storage),
            Arc::clone(&tx_chaining),
            Arc::clone(&u5c_adapter),
            Arc::clone(&secret_adapter),
        );
        let submit_service =
            spec::submit::submit_service_server::SubmitServiceServer::new(submit_service);

        info!(
            address = config.listen_address.to_string(),
            "GRPC server running"
        );

        let result = Server::builder()
            .add_service(reflection)
            .add_service(submit_service)
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
