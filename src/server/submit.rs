use std::sync::Arc;

use pallas::ledger::traverse::MultiEraTx;
use spec::boros::v1::submit::{
    submit_service_server::SubmitService, LockStateRequest, LockStateResponse, SubmitTxRequest,
    SubmitTxResponse,
};
use tonic::{Response, Status};
use tracing::{error, info};

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
impl SubmitService for SubmitServiceImpl {
    async fn submit_tx(
        &self,
        request: tonic::Request<SubmitTxRequest>,
    ) -> std::result::Result<tonic::Response<SubmitTxResponse>, tonic::Status> {
        let message = request.into_inner();

        let mut txs: Vec<Transaction> = Vec::default();
        let mut hashes = vec![];

        for (idx, tx) in message.tx.into_iter().enumerate() {
            let hash = MultiEraTx::decode(&tx.raw)
                .map_err(|error| {
                    error!(?error);
                    Status::failed_precondition(format!("invalid tx at index {idx}"))
                })?
                .hash();

            hashes.push(hash.to_vec().into());
            let mut tx_storage = Transaction::new(hash.to_string(), tx.raw.to_vec());
            // TODO: validate if the queue has the lock mechanism activated, if yes, validate the
            // token sent.
            if let Some(queue) = tx.queue {
                // TODO: validate if the queue is configured
                //       if not, the transaction goes to the default queue
                tx_storage.queue = queue;
            }
            txs.push(tx_storage)
        }

        let hashes_str: Vec<String> = hashes.iter().map(hex::encode).collect();
        info!(?hashes_str, "submitting txs");

        self.tx_storage.create(&txs).await.map_err(|error| {
            error!(?error);
            Status::internal("internal error")
        })?;

        Ok(Response::new(SubmitTxResponse { r#ref: hashes }))
    }

    async fn lock_state(
        &self,
        _request: tonic::Request<LockStateRequest>,
    ) -> std::result::Result<tonic::Response<LockStateResponse>, tonic::Status> {
        todo!()
    }
}
