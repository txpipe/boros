use std::{borrow::Cow, sync::Arc, time::Duration};

use gasket::framework::*;
use pallas::{
    crypto::hash::Hash,
    ledger::{
        primitives::TransactionInput,
        traverse::{wellknown::GenesisValues, MultiEraInput, MultiEraOutput, MultiEraTx},
    },
    validate::{
        phase_one::validate_tx,
        uplc::{script_context::SlotConfig, tx, EvalReport},
        utils::{AccountState, CertState, Environment, UTxOs},
    },
};
use tokio::time::sleep;
use tracing::info;

use crate::{
    ledger::u5c::U5cDataAdapter,
    priority::Priority,
    storage::{sqlite::SqliteTransaction, Transaction, TransactionStatus},
};

#[derive(Stage)]
#[stage(name = "ingest", unit = "Vec<Transaction>", worker = "Worker")]
pub struct Stage {
    storage: Arc<SqliteTransaction>,
    priority: Arc<Priority>,
    u5c_adapter: Arc<dyn U5cDataAdapter>,
}

impl Stage {
    pub fn new(
        storage: Arc<SqliteTransaction>,
        priority: Arc<Priority>,
        u5c_adapter: Arc<dyn U5cDataAdapter>,
    ) -> Self {
        Self {
            storage,
            priority,
            u5c_adapter,
        }
    }

    async fn validate_tx<'a>(&self, tx: &MultiEraTx<'a>) -> Result<(), anyhow::Error> {
        let (block_slot, block_hash_vec) = self.u5c_adapter.fetch_tip().await.or_retry()?;
        let block_hash_vec: [u8; 32] = block_hash_vec.try_into().unwrap();
        let block_hash: Hash<32> = Hash::from(block_hash_vec);

        let tip = (block_slot, block_hash);

        let network_magic = 2;

        let era = tx.era();

        let pparams = self.u5c_adapter.fetch_pparams(era).await.or_retry()?;

        let genesis_values = GenesisValues::from_magic(network_magic.into()).unwrap();

        let env = Environment {
            prot_params: pparams.clone(),
            prot_magic: network_magic,
            block_slot: tip.0,
            network_id: genesis_values.network_id as u8,
            acnt: Some(AccountState::default()),
        };

        let input_refs = tx
            .requires()
            .iter()
            .map(|input: &MultiEraInput<'_>| (*input.hash(), input.index() as u32))
            .collect::<Vec<(Hash<32>, u32)>>();

        let utxos = self
            .u5c_adapter
            .fetch_utxos(input_refs, era)
            .await
            .or_retry()?;

        let mut pallas_utxos = UTxOs::new();

        for ((tx_hash, index), eracbor) in utxos.iter() {
            let tx_in = TransactionInput {
                transaction_id: *tx_hash,
                index: (*index).into(),
            };
            let input = MultiEraInput::AlonzoCompatible(Box::from(Cow::Owned(tx_in)));
            let output = MultiEraOutput::try_from(eracbor)?;
            pallas_utxos.insert(input, output);
        }

        validate_tx(tx, 0, &env, &pallas_utxos, &mut CertState::default())?;

        Ok(())
    }

    async fn evaluate_tx<'a>(&self, tx: &MultiEraTx<'a>) -> Result<EvalReport, anyhow::Error> {
        let era = tx.era();

        let pparams = self.u5c_adapter.fetch_pparams(era).await.or_retry()?;

        let slot_config = SlotConfig::default();

        let input_refs = tx
            .requires()
            .iter()
            .map(|input: &MultiEraInput<'_>| (*input.hash(), input.index() as u32))
            .collect::<Vec<(Hash<32>, u32)>>();

        let utxos = self
            .u5c_adapter
            .fetch_utxos(input_refs, era)
            .await
            .or_retry()?;

        let utxos = utxos
            .iter()
            .map(|((tx_hash, index), eracbor)| (From::from((*tx_hash, *index)), eracbor.clone()))
            .collect();

        let report = tx::eval_tx(tx, &pparams, &utxos, &slot_config).or_retry()?;

        Ok(report)
    }
}

pub struct Worker;

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(_stage: &Stage) -> Result<Self, WorkerError> {
        Ok(Self)
    }

    async fn schedule(
        &mut self,
        stage: &mut Stage,
    ) -> Result<WorkSchedule<Vec<Transaction>>, WorkerError> {
        let transactions = stage
            .priority
            .next(TransactionStatus::Pending)
            .await
            .or_retry()?;

        if !transactions.is_empty() {
            return Ok(WorkSchedule::Unit(transactions));
        }

        sleep(Duration::from_secs(1)).await;
        Ok(WorkSchedule::Idle)
    }

    async fn execute(
        &mut self,
        unit: &Vec<Transaction>,
        stage: &mut Stage,
    ) -> Result<(), WorkerError> {
        info!("validating {} transactions", unit.len());

        let mut transactions = vec![];

        for tx in unit.iter() {
            let mut tx = tx.clone();
            let metx = MultiEraTx::decode(AsRef::as_ref(&tx.raw)).ok().unwrap();

            stage.validate_tx(&metx).await.or_retry()?;
            stage.evaluate_tx(&metx).await.or_retry()?;

            tx.status = TransactionStatus::Validated;
            transactions.push(tx);
        }

        stage.storage.update_batch(&transactions).await.or_retry()?;

        Ok(())
    }
}

#[cfg(test)]
mod ingest_tests {

    use std::sync::Arc;

    use crate::ledger::u5c::U5cDataAdapterImpl;
    use crate::pipeline::ingest;
    use crate::priority::Priority;
    use crate::storage::{
        sqlite::sqlite_utils_tests::{mock_sqlite_cursor, mock_sqlite_transaction},
        Transaction, TransactionStatus,
    };
    use crate::Config;


    /// Test file = conway9.tx
    /// This test is expected to pass because the transaction is valid.
    #[tokio::test]
    async fn it_should_validate_tx() {
        let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
        let storage = Arc::new(mock_sqlite_transaction().await);
        let cursor = Arc::new(mock_sqlite_cursor().await);
        let cursor = cursor.current().await.unwrap().map(|c| c.into());
        let config = Config::new().expect("invalid config file");
        let u5c_data_adapter = Arc::new(
            U5cDataAdapterImpl::try_new(config.u5c, cursor)
                .await
                .unwrap(),
        );
        let priority = Arc::new(Priority::new(storage.clone(), config.queues));

        let stage = ingest::Stage::new(storage.clone(), priority, u5c_data_adapter);

        let tx_cbor = include_str!("../../test/conway9.tx");
        let mut tx = Transaction::new(1.to_string(), hex::decode(tx_cbor).unwrap());
        tx.status = TransactionStatus::Pending;

        let metx = pallas::ledger::traverse::MultiEraTx::decode(AsRef::as_ref(&tx.raw))
            .ok()
            .unwrap();
        let validation_result = stage.validate_tx(&metx).await;

        assert!(
            validation_result.is_ok(),
            "Validation failed: {:?}",
            validation_result
        );
    }

    /// Test file = conway9.tx
    /// This test is expected to pass because the transaction is valid.
    #[tokio::test]
    async fn it_should_evaluate_tx() {
        let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
        let storage = Arc::new(mock_sqlite_transaction().await);
        let cursor = Arc::new(mock_sqlite_cursor().await);
        let cursor = cursor.current().await.unwrap().map(|c| c.into());
        let config = Config::new().expect("invalid config file");
        let u5c_data_adapter = Arc::new(
            U5cDataAdapterImpl::try_new(config.u5c, cursor)
                .await
                .unwrap(),
        );
        let priority = Arc::new(Priority::new(storage.clone(), config.queues));

        let stage = ingest::Stage::new(storage.clone(), priority, u5c_data_adapter);

        let tx_cbor = include_str!("../../test/conway9.tx");
        let mut tx = Transaction::new(1.to_string(), hex::decode(tx_cbor).unwrap());
        tx.status = TransactionStatus::Pending;

        let metx = pallas::ledger::traverse::MultiEraTx::decode(AsRef::as_ref(&tx.raw))
            .ok()
            .unwrap();
        let evaluation_result = stage.evaluate_tx(&metx).await;

        assert!(
            evaluation_result.is_ok(),
            "Evaluation failed: {:?}",
            evaluation_result.iter().len()
        );
    }

    /// Test file = conway9.tx
    /// This test is expected to pass because the transaction is valid.
    #[tokio::test]
    async fn it_should_validate_and_evaluate_tx() {
        let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
        let storage = Arc::new(mock_sqlite_transaction().await);
        let cursor = Arc::new(mock_sqlite_cursor().await);
        let cursor = cursor.current().await.unwrap().map(|c| c.into());
        let config = Config::new().expect("invalid config file");
        let u5c_data_adapter = Arc::new(
            U5cDataAdapterImpl::try_new(config.u5c, cursor)
                .await
                .unwrap(),
        );
        let priority = Arc::new(Priority::new(storage.clone(), config.queues));

        let stage = ingest::Stage::new(storage.clone(), priority, u5c_data_adapter);

        let tx_cbor = include_str!("../../test/conway9.tx");
        let mut tx = Transaction::new(1.to_string(), hex::decode(tx_cbor).unwrap());
        tx.status = TransactionStatus::Pending;

        let metx = pallas::ledger::traverse::MultiEraTx::decode(AsRef::as_ref(&tx.raw))
            .ok()
            .unwrap();
        let validation_result = stage.validate_tx(&metx).await;
        let evaluation_result = stage.evaluate_tx(&metx).await;
        tx.status = TransactionStatus::Validated;

        assert!(
            validation_result.is_ok(),
            "Validation failed: {:?}",
            validation_result
        );
        assert!(
            evaluation_result.is_ok(),
            "Evaluation failed: {:?}",
            evaluation_result.iter().len()
        );
        assert_eq!(
            tx.status,
            TransactionStatus::Validated,
            "Transaction status was not updated to Validated."
        );
    }

    /// Test file = conway8.tx
    /// This test is expected to fail because the transaction is unwitnessed.
    #[tokio::test]
    async fn it_should_not_validate_tx() {
        let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
        let storage = Arc::new(mock_sqlite_transaction().await);
        let cursor = Arc::new(mock_sqlite_cursor().await);
        let cursor = cursor.current().await.unwrap().map(|c| c.into());
        let config = Config::new().expect("invalid config file");
        let u5c_data_adapter = Arc::new(
            U5cDataAdapterImpl::try_new(config.u5c, cursor)
                .await
                .unwrap(),
        );
        let priority = Arc::new(Priority::new(storage.clone(), config.queues));

        let stage = ingest::Stage::new(storage.clone(), priority, u5c_data_adapter);

        let tx_cbor = include_str!("../../test/conway7.tx");
        let mut tx = Transaction::new(1.to_string(), hex::decode(tx_cbor).unwrap());
        tx.status = TransactionStatus::Pending;

        let metx = pallas::ledger::traverse::MultiEraTx::decode(AsRef::as_ref(&tx.raw))
            .ok()
            .unwrap();
        let validation_result = stage.validate_tx(&metx).await;

        assert!(
            validation_result.is_err(),
            "Validation failed: {:?}",
            validation_result
        );
    }

    /// Test file = conway10.tx
    /// This test is expected to fail because
    /// the transaction script hash is invalid.
    #[tokio::test]
    async fn it_should_not_evaluate_tx() {
        let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
        let storage = Arc::new(mock_sqlite_transaction().await);
        let cursor = Arc::new(mock_sqlite_cursor().await);
        let cursor = cursor.current().await.unwrap().map(|c| c.into());
        let config = Config::new().expect("invalid config file");
        let u5c_data_adapter = Arc::new(
            U5cDataAdapterImpl::try_new(config.u5c, cursor)
                .await
                .unwrap(),
        );
        let priority = Arc::new(Priority::new(storage.clone(), config.queues));

        let stage = ingest::Stage::new(storage.clone(), priority, u5c_data_adapter);

        let tx_cbor = include_str!("../../test/conway10.tx");
        let mut tx = Transaction::new(1.to_string(), hex::decode(tx_cbor).unwrap());
        tx.status = TransactionStatus::Pending;

        let metx = pallas::ledger::traverse::MultiEraTx::decode(AsRef::as_ref(&tx.raw))
            .ok()
            .unwrap();
        let evaluation_result = stage.evaluate_tx(&metx).await;

        assert!(
            evaluation_result.is_err(),
            "Evaluation failed: {:?}",
            evaluation_result.iter().len()
        );
    }
}
