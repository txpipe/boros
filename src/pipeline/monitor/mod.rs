use std::{pin::Pin, sync::Arc};

use futures::{Stream, TryStreamExt};
use gasket::framework::*;
use pallas::interop::utxorpc::spec::cardano::Tx;
use tracing::info;

use crate::storage::{
    sqlite::{SqliteCursor, SqliteTransaction},
    Cursor, TransactionStatus,
};

pub mod u5c;

#[cfg(test)]
pub mod file;

type Point = (u64, Vec<u8>);

#[derive(Debug)]
pub enum Event {
    RollForward(Point, Vec<Tx>),
    Rollback(Point),
}

type ChainSyncStream = Pin<Box<dyn Stream<Item = anyhow::Result<Event>> + Send>>;

#[async_trait::async_trait]
pub trait ChainSyncAdapter {
    async fn stream(&mut self) -> anyhow::Result<ChainSyncStream>;
}

#[derive(Stage)]
#[stage(name = "monitor", unit = "Event", worker = "Worker")]
pub struct Stage {
    storage: Arc<SqliteTransaction>,
    cursor: Arc<SqliteCursor>,
    stream: ChainSyncStream,
}
impl Stage {
    pub async fn try_new(
        storage: Arc<SqliteTransaction>,
        cursor: Arc<SqliteCursor>,
        mut adapter: Box<dyn ChainSyncAdapter>,
    ) -> anyhow::Result<Self> {
        let stream = adapter.stream().await?;
        Ok(Self {
            storage,
            cursor,
            stream,
        })
    }
}

pub struct Worker;

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(_stage: &Stage) -> Result<Self, WorkerError> {
        Ok(Self)
    }

    async fn schedule(&mut self, stage: &mut Stage) -> Result<WorkSchedule<Event>, WorkerError> {
        if let Some(e) = stage.stream.try_next().await.or_restart()? {
            return Ok(WorkSchedule::Unit(e));
        }

        Ok(WorkSchedule::Idle)
    }

    async fn execute(&mut self, unit: &Event, stage: &mut Stage) -> Result<(), WorkerError> {
        let (slot, hash) = match unit {
            Event::RollForward((slot, hash), txs) => {
                // TODO: validate slot tx "timeout"
                // TODO: change tx to rejected when reach the quantity of attempts.
                //       fanout will try N quantity
                //       send to pending again

                // TODO: analyse possible problem with the query to get all inflight data
                let txs_in_flight = stage
                    .storage
                    .find(TransactionStatus::InFlight)
                    .await
                    .or_retry()?;

                let txs_to_confirm = txs_in_flight
                    .into_iter()
                    .filter(|itx| txs.iter().any(|tx| hex::encode(&tx.hash) == itx.id))
                    .map(|mut tx| {
                        tx.status = TransactionStatus::Confirmed;
                        tx.slot = Some(*slot);
                        tx
                    })
                    .collect();

                stage
                    .storage
                    .update_batch(&txs_to_confirm)
                    .await
                    .or_retry()?;

                info!("Slot {slot} RollForward {} transactions", txs.len());

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
