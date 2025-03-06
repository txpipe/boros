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

//TODO: change tonic to local struct
pub struct TxChaining {
    tx_lock: HashMap<String, Sender<(Sender<Result<LockStateResponse, Status>>, LockStateRequest)>>,
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

    pub async fn unlock(&self, queue: &str) -> anyhow::Result<()> {
        let Some(tx) = self.tx_unlock.get(queue) else {
            bail!("invalid queue")
        };

        tx.send(true).await?;

        Ok(())
    }

    pub fn is_chained_queue(&self, queue: &str) -> bool {
        self.tx_lock.get(queue).is_some()
    }

    pub async fn is_valid_token(&self, queue: &str, token: &str) -> bool {
        if let Some(current_token) = self.state.read().await.get(queue) {
            return current_token.eq(token);
        }
        false
    }
}
