use std::sync::Arc;

use futures::TryStreamExt;
use gasket::framework::*;
use serde::Deserialize;
use tracing::info;

use crate::{
    ledger::u5c::{ChainSyncStream, Event, U5cDataAdapter},
    storage::{
        sqlite::{SqliteCursor, SqliteTransaction},
        Cursor, Transaction, TransactionStatus,
    },
};

#[derive(Deserialize, Clone)]
pub struct Config {
    pub retry_slot_diff: u64,
}

#[derive(Stage)]
#[stage(name = "monitor", unit = "Event", worker = "Worker")]
pub struct Stage {
    config: Config,
    adapter: Arc<dyn U5cDataAdapter>,
    storage: Arc<SqliteTransaction>,
    cursor: Arc<SqliteCursor>,
}
impl Stage {
    pub fn new(
        config: Config,
        adapter: Arc<dyn U5cDataAdapter>,
        storage: Arc<SqliteTransaction>,
        cursor: Arc<SqliteCursor>,
    ) -> Self {
        Self {
            config,
            adapter,
            storage,
            cursor,
        }
    }
}

pub struct Worker {
    stream: ChainSyncStream,
}

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(stage: &Stage) -> Result<Self, WorkerError> {
        let stream = stage.adapter.stream().await.or_panic()?;

        Ok(Self { stream })
    }

    async fn schedule(&mut self, _stage: &mut Stage) -> Result<WorkSchedule<Event>, WorkerError> {
        if let Some(e) = self.stream.try_next().await.or_restart()? {
            return Ok(WorkSchedule::Unit(e));
        }

        Ok(WorkSchedule::Idle)
    }

    async fn execute(&mut self, unit: &Event, stage: &mut Stage) -> Result<(), WorkerError> {
        let (slot, hash) = match unit {
            Event::RollForward((slot, hash), txs) => {
                info!("Slot {slot} RollForward");

                let txs_inflight = stage
                    .storage
                    .find(TransactionStatus::InFlight)
                    .await
                    .or_retry()?;

                let txs_confirm: Vec<Transaction> = txs_inflight
                    .clone()
                    .into_iter()
                    .filter(|itx| txs.iter().any(|tx| hex::encode(&tx.hash) == itx.id))
                    .map(|mut tx| {
                        tx.status = TransactionStatus::Confirmed;
                        tx.slot = Some(*slot);
                        tx
                    })
                    .collect();
                if !txs_confirm.is_empty() {
                    info!("Confirmed {} transactions", txs_confirm.len());
                    stage.storage.update_batch(&txs_confirm).await.or_retry()?;
                }

                let txs_retry: Vec<Transaction> = txs_inflight
                    .into_iter()
                    .filter(|itx| {
                        (slot - itx.slot.unwrap()) > stage.config.retry_slot_diff
                            && !txs_confirm.iter().any(|tx| tx.id == itx.id)
                    })
                    .map(|mut tx| {
                        tx.status = TransactionStatus::Pending;
                        tx.slot = None;
                        tx
                    })
                    .collect();
                if !txs_retry.is_empty() {
                    info!("Slot {slot} Retry {} transactions", txs_retry.len());
                    stage.storage.update_batch(&txs_retry).await.or_retry()?;
                }

                (slot, hash)
            }
            Event::Rollback((slot, hash)) => {
                let txs = stage.storage.find_to_rollback(*slot).await.or_retry()?;

                let txs = txs
                    .into_iter()
                    .map(|mut tx| {
                        if tx.slot.unwrap() > *slot {
                            tx.status = TransactionStatus::InFlight;
                            tx.slot = None;
                            return tx;
                        }

                        tx.slot = Some(*slot);
                        tx
                    })
                    .collect();

                stage.storage.update_batch(&txs).await.or_retry()?;

                info!("Slot {slot} Rollback {} transactions", txs.len());

                (slot, hash)
            }
        };

        stage
            .cursor
            .set(&Cursor::new(*slot, hash.to_vec()))
            .await
            .or_retry()?;

        Ok(())
    }
}
