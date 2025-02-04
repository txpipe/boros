use std::{pin::Pin, sync::Arc};

use futures::{Stream, TryStreamExt};
use gasket::framework::*;
use pallas::interop::utxorpc::spec::cardano::Tx;
use tracing::info;

use crate::storage::{sqlite::SqliteTransaction, TransactionStatus};

pub mod u5c;

#[cfg(test)]
pub mod file;

#[derive(Debug)]
pub enum Event {
    RollForward(u64, Vec<Tx>),
    Rollback(u64, Vec<u8>),
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
    stream: ChainSyncStream,
}
impl Stage {
    pub async fn try_new(
        storage: Arc<SqliteTransaction>,
        mut adapter: Box<dyn ChainSyncAdapter>,
    ) -> anyhow::Result<Self> {
        let stream = adapter.stream().await?;
        Ok(Self { storage, stream })
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
        match unit {
            Event::RollForward(slot, txs) => {
                // TODO: validate slot tx "timeout"
                // TODO: change tx to rejected when reach the quantity of attempts.
                //       fanout will try N quantity

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

                info!("RollForward {} txs", txs.len())
            }
            Event::Rollback(slot, _hash) => {
                info!("Rollback slot {slot}");

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

                info!("{} txs updated", txs.len());
            }
        };

        Ok(())
    }
}
