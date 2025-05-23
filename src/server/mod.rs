use std::{collections::HashSet, sync::Arc};

use anyhow::Result;
use futures::try_join;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use crate::{
    ledger::u5c::U5cDataAdapter,
    queue::{self, chaining::TxChaining},
    storage::sqlite::SqliteTransaction,
};

mod grpc;
mod trp;

pub async fn serve(
    config: Config,
    queues: HashSet<queue::Config>,
    u5c_adapter: Arc<dyn U5cDataAdapter>,
    tx_storage: Arc<SqliteTransaction>,
    tx_chaining: Arc<TxChaining>,
    cancellation_token: CancellationToken,
) -> Result<()> {
    let grpc_task = config.grpc.map(|cfg| {
        let queues = queues.clone();
        let u5c_adapter = Arc::clone(&u5c_adapter);
        let tx_storage = Arc::clone(&tx_storage);
        let tx_chaining = Arc::clone(&tx_chaining);
        let cancellation_token = cancellation_token.clone();

        grpc::run(
            cfg,
            queues,
            u5c_adapter,
            tx_storage,
            tx_chaining,
            cancellation_token,
        )
    });

    let trp_task = config.trp.map(|cfg| {
        let tx_storage = Arc::clone(&tx_storage);
        let cancellation_token = cancellation_token.clone();

        trp::run(cfg, tx_storage, cancellation_token)
    });

    try_join!(
        async {
            if let Some(task) = grpc_task {
                return task.await;
            }

            Ok(())
        },
        async {
            if let Some(task) = trp_task {
                return task.await;
            }
            Ok(())
        },
    )?;

    Ok(())
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub grpc: Option<grpc::Config>,
    pub trp: Option<trp::Config>,
}
