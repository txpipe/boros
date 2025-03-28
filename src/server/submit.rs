use std::{pin::Pin, sync::Arc};

use bip39::Mnemonic;
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
    ledger::u5c::U5cDataAdapter,
    queue::chaining::TxChaining,
    signing::{
        key::{
            derive::get_signing_key,
            sign::{sign_transaction, to_built_transaction},
        },
        SecretAdapter,
    },
    storage::{sqlite::SqliteTransaction, Transaction},
    validation::{evaluate_tx, validate_tx},
};

pub struct SubmitServiceImpl {
    tx_storage: Arc<SqliteTransaction>,
    tx_chaining: Arc<TxChaining>,
    u5c_adapter: Arc<dyn U5cDataAdapter>,
    secret_adapter: Arc<dyn SecretAdapter<Mnemonic>>,
}

impl SubmitServiceImpl {
    pub fn new(
        tx_storage: Arc<SqliteTransaction>,
        tx_chaining: Arc<TxChaining>,
        u5c_adapter: Arc<dyn U5cDataAdapter>,
        secret_adapter: Arc<dyn SecretAdapter<Mnemonic>>,
    ) -> Self {
        Self {
            tx_storage,
            tx_chaining,
            u5c_adapter,
            secret_adapter,
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

        let mut txs: Vec<Transaction> = Vec::new();
        let mut hashes = vec![];
        let mut chained_queues = Vec::new();

        let mnemonic = self
            .secret_adapter
            .retrieve_secret()
            .await
            .map_err(|error| {
                error!(?error);
                Status::internal("Error retrieving mnemonic")
            })?;

        for (idx, tx) in message.tx.into_iter().enumerate() {
            let metx = MultiEraTx::decode(&tx.raw).map_err(|error| {
                error!(?error);
                Status::failed_precondition(format!("invalid tx at index {idx}"))
            })?;

            let signing_key = get_signing_key(&mnemonic);
            let built_tx = to_built_transaction(&metx);
            let signed_tx = sign_transaction(built_tx, signing_key);

            let signed_metx = MultiEraTx::decode(&signed_tx.tx_bytes.0).map_err(|error| {
                error!(?error);
                Status::failed_precondition(format!("invalid tx at index {idx}"))
            })?;

            if let Err(error) = validate_tx(&signed_metx, self.u5c_adapter.clone()).await {
                error!(?error);
                continue;
            }

            if let Err(error) = evaluate_tx(&signed_metx, self.u5c_adapter.clone()).await {
                error!(?error);
                continue;
            }

            let hash = signed_metx.hash();

            hashes.push(hash.to_vec().into());
            let mut tx_storage = Transaction::new(hash.to_string(), signed_metx.encode());

            if let Some(queue) = tx.queue {
                if self.tx_chaining.is_chained_queue(&queue) {
                    chained_queues.push(queue.clone());

                    if !self
                        .tx_chaining
                        .is_valid_token(&queue, &tx.lock_token.unwrap_or_default())
                        .await
                    {
                        return Err(Status::permission_denied("invalid lock token"));
                    }
                }

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

        for queue in chained_queues {
            self.tx_chaining.unlock(&queue).await.map_err(|error| {
                error!(?error);
                Status::internal("internal error")
            })?;
        }

        Ok(Response::new(SubmitTxResponse { r#ref: hashes }))
    }

    type LockStateStream = Pin<Box<dyn Stream<Item = Result<LockStateResponse, Status>> + Send>>;
    async fn lock_state(
        &self,
        request: tonic::Request<LockStateRequest>,
    ) -> std::result::Result<tonic::Response<Self::LockStateStream>, tonic::Status> {
        let lock_state_request = request.into_inner();

        if !self.tx_chaining.is_chained_queue(&lock_state_request.queue) {
            return Err(Status::invalid_argument("queue is not chained"));
        }

        let (tx_stream, rx_stream) = mpsc::channel(1);
        self.tx_chaining
            .lock(tx_stream, lock_state_request)
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
