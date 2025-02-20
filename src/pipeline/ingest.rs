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

        let utxos = self.u5c_adapter.fetch_utxos(input_refs).await.or_retry()?;

        let mut pallas_utxos = UTxOs::new();

        for ((tx_hash, index), eracbor) in utxos.iter() {
            let tx_in = TransactionInput {
                transaction_id: *tx_hash,
                index: (*index).into(),
            };
            let input = MultiEraInput::AlonzoCompatible(Box::from(Cow::Owned(tx_in)));
            let output = MultiEraOutput::try_from(eracbor)?;
            info!("Adding UTXO: {:?}", output);
            info!("Adding UTXO: {:?}", input);
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

        let utxos = self.u5c_adapter.fetch_utxos(input_refs).await.or_retry()?;
        
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

        let tx_cbor = "84a800d9010281825820248765b42a369fbadcb4bf83a5ecd518d8b540b714c007722283939aa6d22e6c00018282581d609539a38568b0405e406b00e6e8f27ebfa975ee861c7eceff90529f831a004c4b40825839008d775cd4e9397c2359f1b1c491ebee8541281abba5f3ed432e37882df38fd378b75df6519f23214cee2520d3da870611be24d093f8f6af9c1a00957088021a000325f80b5820d60845709ca93296d89253aeb42a8c968ec33c7472d9df0799fbe063b65dea4a0dd9010281825820111080233d4ac73eb4db4c7468cfd1aaf6d770ca42db2038d79f4b4d6a087c01000ed9010281581c8d775cd4e9397c2359f1b1c491ebee8541281abba5f3ed432e37882d10825839008d775cd4e9397c2359f1b1c491ebee8541281abba5f3ed432e37882df38fd378b75df6519f23214cee2520d3da870611be24d093f8f6af9c1a0071a97e111a0004b8f4a205a182000082d879808219cd0a1a010e7f3607d901028159024d59024a0101003229800aba2aba1aab9faab9eaab9dab9a48888896600264653001300700198039804000cc01c0092225980099b8748008c01cdd500144c8cc8a60022b30013001300a375400d159800980098051baa0028992cc004c008c02cdd5000c660026eb8c038c030dd5000c88c8cc00400400c896600200314a115980098019809000c528c4cc008008c04c00500e20229180798081808000a444b30013300237586002601e6ea80208c966002600e60206ea8006266e3cdd7180998089baa0010058b201e301230103754602460206ea80062660046eb0c004c03cdd50041192cc004c01cc040dd5000c4c966002601060226ea8006266e3cdd7180a18091baa002375c602860246ea80062c8080c04cc044dd5180998089baa0028b201e30123010375402b14a08069164028601a60166ea8c034c02cdd5180698059baa0028b20128acc004c004c028dd500144c966002600460166ea800626464660020026eb0c040c044c044c044c044c044c044c044c044c038dd5003912cc00400629422b30013371e6eb8c04400400e2946266004004602400280690101bae300e300c375400314a08050c034c02cdd5180698059baa300d300b3754005164024804a60146ea801a601a0069112cc004c01000a2b3001300e37540130038b201e8acc004cdc3a400400515980098071baa009801c5900f45900c2018180598060009b8748000c020dd50014528200c180380098019baa0078a4d1365640044c129d8799fd8799f581c9539a38568b0405e406b00e6e8f27ebfa975ee861c7eceff90529f83ffd87a80ff0001f5f6";
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

        let tx_cbor = "84a800d9010281825820248765b42a369fbadcb4bf83a5ecd518d8b540b714c007722283939aa6d22e6c00018282581d609539a38568b0405e406b00e6e8f27ebfa975ee861c7eceff90529f831a004c4b40825839008d775cd4e9397c2359f1b1c491ebee8541281abba5f3ed432e37882df38fd378b75df6519f23214cee2520d3da870611be24d093f8f6af9c1a00957088021a000325f80b5820d60845709ca93296d89253aeb42a8c968ec33c7472d9df0799fbe063b65dea4a0dd9010281825820111080233d4ac73eb4db4c7468cfd1aaf6d770ca42db2038d79f4b4d6a087c01000ed9010281581c8d775cd4e9397c2359f1b1c491ebee8541281abba5f3ed432e37882d10825839008d775cd4e9397c2359f1b1c491ebee8541281abba5f3ed432e37882df38fd378b75df6519f23214cee2520d3da870611be24d093f8f6af9c1a0071a97e111a0004b8f4a205a182000082d879808219cd0a1a010e7f3607d901028159024d59024a0101003229800aba2aba1aab9faab9eaab9dab9a48888896600264653001300700198039804000cc01c0092225980099b8748008c01cdd500144c8cc8a60022b30013001300a375400d159800980098051baa0028992cc004c008c02cdd5000c660026eb8c038c030dd5000c88c8cc00400400c896600200314a115980098019809000c528c4cc008008c04c00500e20229180798081808000a444b30013300237586002601e6ea80208c966002600e60206ea8006266e3cdd7180998089baa0010058b201e301230103754602460206ea80062660046eb0c004c03cdd50041192cc004c01cc040dd5000c4c966002601060226ea8006266e3cdd7180a18091baa002375c602860246ea80062c8080c04cc044dd5180998089baa0028b201e30123010375402b14a08069164028601a60166ea8c034c02cdd5180698059baa0028b20128acc004c004c028dd500144c966002600460166ea800626464660020026eb0c040c044c044c044c044c044c044c044c044c038dd5003912cc00400629422b30013371e6eb8c04400400e2946266004004602400280690101bae300e300c375400314a08050c034c02cdd5180698059baa300d300b3754005164024804a60146ea801a601a0069112cc004c01000a2b3001300e37540130038b201e8acc004cdc3a400400515980098071baa009801c5900f45900c2018180598060009b8748000c020dd50014528200c180380098019baa0078a4d1365640044c129d8799fd8799f581c9539a38568b0405e406b00e6e8f27ebfa975ee861c7eceff90529f83ffd87a80ff0001f5f6";
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

        let tx_cbor = "84a800d9010281825820248765b42a369fbadcb4bf83a5ecd518d8b540b714c007722283939aa6d22e6c00018282581d609539a38568b0405e406b00e6e8f27ebfa975ee861c7eceff90529f831a004c4b40825839008d775cd4e9397c2359f1b1c491ebee8541281abba5f3ed432e37882df38fd378b75df6519f23214cee2520d3da870611be24d093f8f6af9c1a00957088021a000325f80b5820d60845709ca93296d89253aeb42a8c968ec33c7472d9df0799fbe063b65dea4a0dd9010281825820111080233d4ac73eb4db4c7468cfd1aaf6d770ca42db2038d79f4b4d6a087c01000ed9010281581c8d775cd4e9397c2359f1b1c491ebee8541281abba5f3ed432e37882d10825839008d775cd4e9397c2359f1b1c491ebee8541281abba5f3ed432e37882df38fd378b75df6519f23214cee2520d3da870611be24d093f8f6af9c1a0071a97e111a0004b8f4a205a182000082d879808219cd0a1a010e7f3607d901028159024d59024a0101003229800aba2aba1aab9faab9eaab9dab9a48888896600264653001300700198039804000cc01c0092225980099b8748008c01cdd500144c8cc8a60022b30013001300a375400d159800980098051baa0028992cc004c008c02cdd5000c660026eb8c038c030dd5000c88c8cc00400400c896600200314a115980098019809000c528c4cc008008c04c00500e20229180798081808000a444b30013300237586002601e6ea80208c966002600e60206ea8006266e3cdd7180998089baa0010058b201e301230103754602460206ea80062660046eb0c004c03cdd50041192cc004c01cc040dd5000c4c966002601060226ea8006266e3cdd7180a18091baa002375c602860246ea80062c8080c04cc044dd5180998089baa0028b201e30123010375402b14a08069164028601a60166ea8c034c02cdd5180698059baa0028b20128acc004c004c028dd500144c966002600460166ea800626464660020026eb0c040c044c044c044c044c044c044c044c044c038dd5003912cc00400629422b30013371e6eb8c04400400e2946266004004602400280690101bae300e300c375400314a08050c034c02cdd5180698059baa300d300b3754005164024804a60146ea801a601a0069112cc004c01000a2b3001300e37540130038b201e8acc004cdc3a400400515980098071baa009801c5900f45900c2018180598060009b8748000c020dd50014528200c180380098019baa0078a4d1365640044c129d8799fd8799f581c9539a38568b0405e406b00e6e8f27ebfa975ee861c7eceff90529f83ffd87a80ff0001f5f6";
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
}
