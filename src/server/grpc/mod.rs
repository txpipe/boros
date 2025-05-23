use std::{collections::HashSet, net::SocketAddr, sync::Arc};

use anyhow::Result;
use serde::Deserialize;
use spec::boros::v1 as spec;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;
use tracing::info;

use crate::{
    ledger::u5c::U5cDataAdapter,
    queue::{self, chaining::TxChaining},
    storage::sqlite::SqliteTransaction,
};

mod submit;

pub async fn run(
    config: Config,
    queues: HashSet<queue::Config>,
    u5c_adapter: Arc<dyn U5cDataAdapter>,
    tx_storage: Arc<SqliteTransaction>,
    tx_chaining: Arc<TxChaining>,
    cancellation_token: CancellationToken,
) -> Result<()> {
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(protoc_wkt::google::protobuf::FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(spec::submit::FILE_DESCRIPTOR_SET)
        .build_v1alpha()
        .unwrap();

    let submit_service = submit::SubmitServiceImpl::new(
        Arc::clone(&tx_storage),
        Arc::clone(&tx_chaining),
        Arc::clone(&u5c_adapter),
        queues,
    );
    let submit_service =
        spec::submit::submit_service_server::SubmitServiceServer::new(submit_service);

    info!(
        address = config.listen_address.to_string(),
        "GRPC server running"
    );

    Server::builder()
        .add_service(reflection)
        .add_service(submit_service)
        .serve_with_shutdown(config.listen_address, cancellation_token.cancelled())
        .await?;

    info!("gracefully shut down grpc");

    Ok(())
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub listen_address: SocketAddr,
}
