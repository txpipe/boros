use std::{pin::Pin, sync::Arc};

use pallas::ledger::traverse::MultiEraTx;
use spec::boros::v1::submit::{
    submit_service_server::SubmitService, LockStateRequest, LockStateResponse, SubmitTxRequest,
    SubmitTxResponse,
};
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, Stream};
use tonic::{Response, Status};
use tracing::{error, info};

use crate::{
    queue::chaining::TxChaining,
    storage::{sqlite::SqliteTransaction, Transaction},
};

pub struct SubmitServiceImpl {
    tx_storage: Arc<SqliteTransaction>,
    tx_chaining: Arc<TxChaining>,
}

impl SubmitServiceImpl {
    pub fn new(tx_storage: Arc<SqliteTransaction>, tx_chaining: Arc<TxChaining>) -> Self {
        Self {
            tx_storage,
            tx_chaining,
        }
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
                // TODO: validate  if the queue need to validate the tx_chaining
                //       move the unlock to injest later
                self.tx_chaining
                    .unlock(&queue, &tx.lock_token.unwrap_or_default())
                    .await
                    .map_err(|error| {
                        error!(?error);
                        Status::permission_denied(format!("invalid lock token"))
                    })?;

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

    type LockStateStream = Pin<Box<dyn Stream<Item = Result<LockStateResponse, Status>> + Send>>;
    async fn lock_state(
        &self,
        request: tonic::Request<LockStateRequest>,
    ) -> std::result::Result<tonic::Response<Self::LockStateStream>, tonic::Status> {
        let lock_state_request = request.into_inner();
        let queue = lock_state_request.queue.clone();

        let (tx_stream, rx_stream) = mpsc::channel(1);
        self.tx_chaining
            .lock(&queue, tx_stream, lock_state_request)
            .await
            .map_err(|error| {
                error!(?error);
                Status::invalid_argument("invalid queue request")
            })?;

        let output_stream = ReceiverStream::new(rx_stream);
        Ok(Response::new(
            Box::pin(output_stream) as Self::LockStateStream
        ))
    }
}
