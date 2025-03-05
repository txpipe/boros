use std::{collections::HashMap, pin::Pin, sync::Arc, time::Duration};

use anyhow::{bail, ensure};
use pallas::ledger::traverse::MultiEraTx;
use rand::{distr::Alphanumeric, Rng};
use spec::boros::v1::submit::{
    submit_service_server::SubmitService, LockStateRequest, LockStateResponse, SubmitTxRequest,
    SubmitTxResponse,
};
use tokio::sync::{
    mpsc::{self, Sender},
    RwLock,
};
use tokio_stream::{wrappers::ReceiverStream, Stream};
use tonic::{Response, Status};
use tracing::{error, info};

use crate::storage::{sqlite::SqliteTransaction, Transaction};

const LOCK_TIMEOUT: Duration = Duration::from_secs(30);

pub struct SubmitServiceImpl {
    tx_storage: Arc<SqliteTransaction>,
    tx_chaining_handle: TxChainingHandle,
}

impl SubmitServiceImpl {
    pub fn new(tx_storage: Arc<SqliteTransaction>) -> Self {
        // TODO: use the correct queue
        let tx_chaining_handle = TxChainingHandle::new(
            ["tx_chaining_queue".into(), "tx_chaining_queue2".into()].into(),
            Arc::clone(&tx_storage),
        );

        Self {
            tx_storage,
            tx_chaining_handle,
        }
    }
}

type Queue = String;
struct TxChainingHandle {
    tx_lock: HashMap<Queue, Sender<(Sender<Result<LockStateResponse, Status>>, LockStateRequest)>>,
    tx_unlock: HashMap<Queue, Sender<bool>>,
    state: Arc<RwLock<HashMap<Queue, String>>>,
}
impl TxChainingHandle {
    pub fn new(queues: Vec<Queue>, tx_storage: Arc<SqliteTransaction>) -> Self {
        let mut map_tx_lock = HashMap::new();
        let mut map_tx_unlock = HashMap::new();
        let state: Arc<RwLock<HashMap<Queue, String>>> = Arc::new(RwLock::new(HashMap::new()));

        for queue in queues {
            let (tx_lock, mut rx_lock) =
                mpsc::channel::<(Sender<Result<LockStateResponse, Status>>, LockStateRequest)>(256);
            let (tx_unlock, mut rx_unlock) = mpsc::channel::<bool>(64);

            let state_cloned = Arc::clone(&state);
            let queue_cloned = queue.clone();
            let tx_storage_cloned = Arc::clone(&tx_storage);
            tokio::spawn(async move {
                let queue = queue_cloned;
                let state = state_cloned;
                let _tx_storage = tx_storage_cloned;

                while let Some((tx, _req)) = rx_lock.recv().await {
                    let token: String = rand::rng()
                        .sample_iter(&Alphanumeric)
                        .take(7)
                        .map(char::from)
                        .collect();

                    state
                        .write()
                        .await
                        .entry(queue.clone())
                        .and_modify(|v| *v = token.clone())
                        .or_insert(token.clone());

                    if let Err(error) = tx
                        .send(Result::<_, Status>::Ok(LockStateResponse {
                            lock_token: token,
                            cbor: "tx cbor".into(),
                        }))
                        .await
                    {
                        error!(?error);
                        continue;
                    }

                    drop(tx);

                    tokio::select! {
                        _ = rx_unlock.recv() => {
                            dbg!("unlock");
                            state.write().await.remove(&queue);
                        }
                        _ = tokio::time::sleep(LOCK_TIMEOUT) => {
                            dbg!("timeout");
                            state.write().await.remove(&queue);
                        },
                    }
                }
            });

            map_tx_lock.insert(queue.clone(), tx_lock);
            map_tx_unlock.insert(queue, tx_unlock);
        }

        Self {
            tx_lock: map_tx_lock,
            tx_unlock: map_tx_unlock,
            state,
        }
    }

    pub async fn lock(
        &self,
        queue: &str,
        tx_response: Sender<Result<LockStateResponse, Status>>,
        request: LockStateRequest,
    ) -> anyhow::Result<()> {
        let Some(tx) = self.tx_lock.get(queue) else {
            bail!("Invalid queue")
        };

        tx.send((tx_response, request)).await?;

        Ok(())
    }

    pub async fn unlock(&self, queue: &str, token: &str) -> anyhow::Result<()> {
        let Some(tx) = self.tx_unlock.get(queue) else {
            bail!("invalid queue")
        };

        if let Some(current_token) = self.state.read().await.get(queue) {
            ensure!(current_token.eq(token), "invalid queue token");
        }

        tx.send(true).await?;

        Ok(())
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
                self.tx_chaining_handle
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
        self.tx_chaining_handle
            .lock(&queue, tx_stream, lock_state_request)
            .await
            .map_err(|error| {
                error!(?error);
                Status::internal("invalid request")
            })?;

        let output_stream = ReceiverStream::new(rx_stream);
        Ok(Response::new(
            Box::pin(output_stream) as Self::LockStateStream
        ))
    }
}
