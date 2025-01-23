use anyhow::Result;
use pallas::interop::utxorpc::spec as u5c;
use std::{net::SocketAddr, str::FromStr};
use tonic::transport::Server;
use tracing::{error, info};

mod utxorpc;

pub async fn run() -> Result<()> {
    tokio::spawn(async {
        let submit_service = utxorpc::SubmitServiceImpl {};
        let submit_service =
            u5c::submit::submit_service_server::SubmitServiceServer::new(submit_service);

        let reflection = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(u5c::submit::FILE_DESCRIPTOR_SET)
            .register_encoded_file_descriptor_set(protoc_wkt::google::protobuf::FILE_DESCRIPTOR_SET)
            .build_v1()
            .unwrap();

        let mut server = Server::builder().accept_http1(true);

        let address = "0.0.0.0:5000";
        info!(address, "GRPC server running");

        let result = SocketAddr::from_str(address);
        if let Err(error) = result {
            error!(?error);
            std::process::exit(1);
        }

        let result = server
            .add_service(reflection)
            .add_service(submit_service)
            .serve(result.unwrap())
            .await;

        if let Err(error) = result {
            error!(?error);
            std::process::exit(1);
        }
    });

    Ok(())
}
