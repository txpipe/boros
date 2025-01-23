use std::{pin::Pin, sync::Arc};

use futures_core::Stream;
use pallas::{
    interop::utxorpc::spec::submit::{WaitForTxResponse, *},
    ledger::traverse::MultiEraTx,
};
use tonic::{Request, Response, Status};
use tracing::error;

use crate::storage::{sqlite::SqliteTransaction, Transaction};

pub struct SubmitServiceImpl {
    tx_storage: Arc<SqliteTransaction>,
}

impl SubmitServiceImpl {
    pub fn new(tx_storage: Arc<SqliteTransaction>) -> Self {
        Self { tx_storage }
    }
}

#[async_trait::async_trait]
impl submit_service_server::SubmitService for SubmitServiceImpl {
    type WaitForTxStream =
        Pin<Box<dyn Stream<Item = Result<WaitForTxResponse, tonic::Status>> + Send + 'static>>;

    type WatchMempoolStream =
        Pin<Box<dyn Stream<Item = Result<WatchMempoolResponse, tonic::Status>> + Send + 'static>>;

    async fn submit_tx(
        &self,
        request: Request<SubmitTxRequest>,
    ) -> Result<Response<SubmitTxResponse>, Status> {
        let message = request.into_inner();

        // TODO: validate a better structure to have this code.

        let mut txs: Vec<Transaction> = Vec::default();
        let mut hashes = vec![];

        for (idx, tx_bytes) in message.tx.into_iter().flat_map(|x| x.r#type).enumerate() {
            match tx_bytes {
                any_chain_tx::Type::Raw(bytes) => {
                    let tx = MultiEraTx::decode(&bytes).map_err(|error| {
                        error!(?error);
                        Status::failed_precondition(format!("invalid tx at index {idx}"))
                    })?;
                    let hash = tx.hash();

                    hashes.push(hash.to_vec().into());
                    txs.push(Transaction::new(hash.to_string(), bytes.to_vec()))
                }
            }
        }

        self.tx_storage.create(&txs).await.map_err(|error| {
            error!(?error);
            Status::internal("internal error")
        })?;

        Ok(Response::new(SubmitTxResponse { r#ref: hashes }))
    }

    async fn wait_for_tx(
        &self,
        _request: Request<WaitForTxRequest>,
    ) -> Result<Response<Self::WaitForTxStream>, Status> {
        todo!()
    }

    async fn read_mempool(
        &self,
        _request: tonic::Request<ReadMempoolRequest>,
    ) -> Result<tonic::Response<ReadMempoolResponse>, tonic::Status> {
        Err(Status::unimplemented("read_mempool is not yet available"))
    }

    async fn watch_mempool(
        &self,
        _request: tonic::Request<WatchMempoolRequest>,
    ) -> Result<tonic::Response<Self::WatchMempoolStream>, tonic::Status> {
        todo!()
    }

    async fn eval_tx(
        &self,
        _request: tonic::Request<EvalTxRequest>,
    ) -> Result<tonic::Response<EvalTxResponse>, Status> {
        todo!()
    }
}
