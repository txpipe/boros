use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use anyhow::bail;
use rand::{distr::Alphanumeric, Rng};
use spec::boros::v1::submit::{LockStateRequest, LockStateResponse};
use tokio::sync::{
    mpsc::{self, Sender},
    RwLock,
};
use tonic::Status;
use tracing::{error, info};

use crate::storage::sqlite::SqliteTransaction;

use super::Config;

const LOCK_TIMEOUT: Duration = Duration::from_secs(30);

//TODO: change tonic struct to agnostic local struct

type LockEvent = Sender<(Sender<Result<LockStateResponse, Status>>, LockStateRequest)>;

pub struct TxChaining {
    tx_lock: HashMap<String, LockEvent>,
    tx_unlock: HashMap<String, Sender<bool>>,
    state: Arc<RwLock<HashMap<String, String>>>,
}
impl TxChaining {
    pub fn new(tx_storage: Arc<SqliteTransaction>, queues: HashSet<Config>) -> Self {
        let queues: Vec<String> = queues
            .iter()
            .filter(|c| c.chained)
            .map(|c| c.name.clone())
            .collect();

        info!("starting {} chained queues", queues.len());

        let mut map_tx_lock = HashMap::new();
        let mut map_tx_unlock = HashMap::new();
        let state: Arc<RwLock<HashMap<String, String>>> = Arc::new(RwLock::new(HashMap::new()));

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
                let tx_storage = tx_storage_cloned;

                while let Some((tx, _req)) = rx_lock.recv().await {
                    let token: String = rand::rng()
                        .sample_iter(&Alphanumeric)
                        .take(8)
                        .map(char::from)
                        .collect();

                    state
                        .write()
                        .await
                        .entry(queue.clone())
                        .and_modify(|v| *v = token.clone())
                        .or_insert(token.clone());

                    let latest_transaction = tx_storage.latest(&queue).await;
                    if let Err(error) = latest_transaction {
                        error!(?error);
                        let _ = tx
                            .send(Result::<_, Status>::Err(Status::internal("internal error")))
                            .await;
                        drop(tx);
                        continue;
                    }
                    let cbor = latest_transaction.unwrap().map(|t| t.raw);

                    if let Err(error) = tx
                        .send(Result::<_, Status>::Ok(LockStateResponse {
                            lock_token: token,
                            cbor: cbor.map(|c| c.into()),
                        }))
                        .await
                    {
                        error!(?error);
                        drop(tx);
                        continue;
                    }

                    drop(tx);
                    tokio::select! {
                        _ = rx_unlock.recv() => {
                            state.write().await.remove(&queue);
                        }
                        _ = tokio::time::sleep(LOCK_TIMEOUT) => {
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
        tx_response: Sender<Result<LockStateResponse, Status>>,
        request: LockStateRequest,
    ) -> anyhow::Result<()> {
        let Some(tx) = self.tx_lock.get(&request.queue) else {
            bail!("Invalid queue")
        };

        tx.send((tx_response, request)).await?;

        Ok(())
    }

    pub async fn unlock(&self, queue: &str) -> anyhow::Result<()> {
        let Some(tx) = self.tx_unlock.get(queue) else {
            bail!("invalid queue")
        };

        tx.send(true).await?;

        Ok(())
    }

    pub fn is_chained_queue(&self, queue: &str) -> bool {
        self.tx_lock.contains_key(queue)
    }

    pub async fn is_valid_token(&self, queue: &str, token: &str) -> bool {
        if let Some(current_token) = self.state.read().await.get(queue) {
            return current_token.eq(token);
        }
        false
    }
}

#[cfg(test)]
mod chaining_tests {
    use std::{collections::HashSet, sync::Arc};

    use chrono::Utc;
    use spec::boros::v1::submit::LockStateRequest;
    use tokio::sync::mpsc;

    use crate::{
        queue::{
            chaining::{TxChaining, LOCK_TIMEOUT},
            Config,
        },
        storage::sqlite::sqlite_utils_tests::mock_sqlite_transaction,
    };

    #[tokio::test]
    async fn it_should_lock_queue() {
        let storage = Arc::new(mock_sqlite_transaction().await);

        let queues = HashSet::from_iter(vec![Config {
            name: "banana".into(),
            weight: 1,
            chained: true,
        }]);

        let chaining = TxChaining::new(Arc::clone(&storage), queues);

        let (tx_stream, mut rx_stream) = mpsc::channel(1);

        chaining
            .lock(
                tx_stream,
                LockStateRequest {
                    queue: "banana".into(),
                },
            )
            .await
            .unwrap();

        let result = rx_stream.recv().await;

        assert!(result.is_some());
        assert!(result.unwrap().is_ok());
    }

    #[tokio::test]
    async fn it_should_wait_timeout_to_lock_queue() {
        let storage = Arc::new(mock_sqlite_transaction().await);

        let queues = HashSet::from_iter(vec![Config {
            name: "banana".into(),
            weight: 1,
            chained: true,
        }]);

        let chaining = TxChaining::new(Arc::clone(&storage), queues);

        let (tx_stream, _rx_stream) = mpsc::channel(1);

        let time_now = Utc::now();

        chaining
            .lock(
                tx_stream,
                LockStateRequest {
                    queue: "banana".into(),
                },
            )
            .await
            .unwrap();

        let (tx_stream, mut rx_stream) = mpsc::channel(1);
        chaining
            .lock(
                tx_stream,
                LockStateRequest {
                    queue: "banana".into(),
                },
            )
            .await
            .unwrap();

        let result = rx_stream.recv().await;
        assert!(result.is_some());
        assert!(result.unwrap().is_ok());

        let diff = Utc::now().signed_duration_since(time_now).num_seconds() as u64;

        assert!(diff == LOCK_TIMEOUT.as_secs())
    }

    #[tokio::test]
    async fn it_should_lock_many_queue() {
        let storage = Arc::new(mock_sqlite_transaction().await);

        let queues = HashSet::from_iter(vec![
            Config {
                name: "banana".into(),
                weight: 1,
                chained: true,
            },
            Config {
                name: "orange".into(),
                weight: 1,
                chained: true,
            },
        ]);

        let chaining = TxChaining::new(Arc::clone(&storage), queues);

        let time_now = Utc::now();

        let (tx_stream, mut rx_stream) = mpsc::channel(1);
        chaining
            .lock(
                tx_stream,
                LockStateRequest {
                    queue: "banana".into(),
                },
            )
            .await
            .unwrap();
        let result = rx_stream.recv().await;
        assert!(result.is_some());
        assert!(result.unwrap().is_ok());

        let (tx_stream, mut rx_stream) = mpsc::channel(1);
        chaining
            .lock(
                tx_stream,
                LockStateRequest {
                    queue: "orange".into(),
                },
            )
            .await
            .unwrap();
        let result = rx_stream.recv().await;
        assert!(result.is_some());
        assert!(result.unwrap().is_ok());

        let diff = Utc::now().signed_duration_since(time_now).num_seconds() as u64;
        assert!(diff < LOCK_TIMEOUT.as_secs())
    }

    #[tokio::test]
    async fn it_should_unlock_queue() {
        let storage = Arc::new(mock_sqlite_transaction().await);

        let queues = HashSet::from_iter(vec![Config {
            name: "banana".into(),
            weight: 1,
            chained: true,
        }]);

        let chaining = TxChaining::new(Arc::clone(&storage), queues);

        let time_now = Utc::now();

        let (tx_stream, mut rx_stream) = mpsc::channel(1);
        let request = LockStateRequest {
            queue: "banana".into(),
        };
        chaining
            .lock(tx_stream.clone(), request.clone())
            .await
            .unwrap();
        rx_stream.recv().await;

        chaining.unlock("banana").await.unwrap();

        chaining.lock(tx_stream.clone(), request).await.unwrap();
        let result = rx_stream.recv().await;
        assert!(result.is_some());
        assert!(result.unwrap().is_ok());

        let diff = Utc::now().signed_duration_since(time_now).num_seconds() as u64;
        assert!(diff < LOCK_TIMEOUT.as_secs())
    }

    #[tokio::test]
    async fn it_should_return_chained_queue() {
        let storage = Arc::new(mock_sqlite_transaction().await);

        let queues = HashSet::from_iter(vec![Config {
            name: "banana".into(),
            weight: 1,
            chained: true,
        }]);

        let chaining = TxChaining::new(Arc::clone(&storage), queues);

        let result = chaining.is_chained_queue("banana");
        assert!(result);
    }

    #[tokio::test]
    async fn it_should_return_not_chained_queue() {
        let storage = Arc::new(mock_sqlite_transaction().await);

        let queues = HashSet::from_iter(vec![Config {
            name: "banana".into(),
            weight: 1,
            chained: false,
        }]);

        let chaining = TxChaining::new(Arc::clone(&storage), queues);

        let result = chaining.is_chained_queue("banana");
        assert!(!result);
    }

    #[tokio::test]
    async fn it_should_return_token_valid() {
        let storage = Arc::new(mock_sqlite_transaction().await);

        let queues = HashSet::from_iter(vec![Config {
            name: "banana".into(),
            weight: 1,
            chained: true,
        }]);

        let chaining = TxChaining::new(Arc::clone(&storage), queues);

        let (tx_stream, mut rx_stream) = mpsc::channel(1);
        chaining
            .lock(
                tx_stream,
                LockStateRequest {
                    queue: "banana".into(),
                },
            )
            .await
            .unwrap();
        let result = rx_stream.recv().await;
        assert!(result.is_some());
        assert!(result.as_ref().unwrap().is_ok());

        let lock_state = result.unwrap().unwrap();

        let result = chaining
            .is_valid_token("banana", &lock_state.lock_token)
            .await;
        assert!(result);
    }

    #[tokio::test]
    async fn it_should_return_token_invalid() {
        let storage = Arc::new(mock_sqlite_transaction().await);

        let queues = HashSet::from_iter(vec![Config {
            name: "banana".into(),
            weight: 1,
            chained: true,
        }]);

        let chaining = TxChaining::new(Arc::clone(&storage), queues);

        let result = chaining.is_valid_token("banana", "invalid").await;
        assert!(!result);
    }
}
