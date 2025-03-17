use std::{borrow::Cow, sync::Arc, time::Duration};

use gasket::framework::*;
use gasket::messaging::{Message, OutputPort};
use pallas::{
    crypto::hash::Hash,
    ledger::{
        primitives::TransactionInput,
        traverse::{wellknown::GenesisValues, MultiEraInput, MultiEraOutput, MultiEraTx},
        validate::{
            phase_one::validate_tx,
            phase_two::evaluate_tx,
            uplc::{script_context::SlotConfig, EvalReport},
            utils::{AccountState, CertState, Environment, UTxOs},
        },
    },
};
use tokio::time::sleep;
use tracing::info;

use super::CAP;
use crate::{
    ledger::u5c::U5cDataAdapter,
    queue::priority::Priority,
    storage::{sqlite::SqliteTransaction, Transaction, TransactionStatus},
};

#[derive(Stage)]
#[stage(name = "ingest", unit = "Vec<Transaction>", worker = "Worker")]
pub struct Stage {
    storage: Arc<SqliteTransaction>,
    priority: Arc<Priority>,
    u5c_adapter: Arc<dyn U5cDataAdapter>,
    pub output: OutputPort<Vec<u8>>,
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
            output: Default::default(),
        }
    }

    async fn validate_tx(&self, tx: &MultiEraTx<'_>) -> Result<(), anyhow::Error> {
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

    async fn evaluate_tx(&self, tx: &MultiEraTx<'_>) -> Result<EvalReport, anyhow::Error> {
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

        let report = evaluate_tx(tx, &pparams, &utxos, &slot_config).or_retry()?;

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
        let queued_len = stage.output.len();
        let current_cap = CAP - queued_len as u16;

        if current_cap == 0 {
            return Ok(WorkSchedule::Idle);
        }

        let transactions = stage
            .priority
            .next(TransactionStatus::Pending, current_cap)
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
        for tx in unit {
            let mut tx = tx.clone();
            let metx = MultiEraTx::decode(&tx.raw).map_err(|_| WorkerError::Recv)?;

            stage.validate_tx(&metx).await.or_retry()?;
            stage.evaluate_tx(&metx).await.or_retry()?;

            let message = Message::from(tx.raw.clone());

            if let Err(e) = stage.output.send(message).await {
                info!("Failed to broadcast transaction: {}", e);
            } else {
                info!("Transaction {} broadcasted to receivers", tx.id);

                let tip = stage.u5c_adapter.fetch_tip().await.or_retry()?;
                tx.status = TransactionStatus::InFlight;
                tx.slot = Some(tip.0);
                stage.storage.update(&tx).await.or_retry()?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod ingest_tests {
    use std::collections::{HashMap, HashSet};
    use std::net::{SocketAddr, SocketAddrV4};
    use std::str::FromStr;
    use std::sync::Arc;

    use anyhow::Ok;
    use pallas::codec::utils::KeyValuePairs;
    use pallas::crypto::hash::Hash;
    use pallas::ledger::primitives::conway::{
        CostModels, DRepVotingThresholds, PoolVotingThresholds,
    };
    use pallas::ledger::primitives::{ExUnitPrices, ExUnits, RationalNumber};
    use pallas::ledger::traverse::Era;
    use pallas::ledger::validate::utils::{ConwayProtParams, EraCbor, MultiEraProtocolParameters};

    use crate::ledger::u5c::{ChainSyncStream, Point, U5cDataAdapter};
    use crate::network::peer_manager::PeerManagerConfig;
    use crate::pipeline::ingest;
    use crate::queue::{priority::Priority, Config as QueueConfig};
    use crate::{
        ledger::u5c::Config as U5cConfig,
        pipeline::monitor::Config as MonitorConfig,
        server::Config as ServerConfig,
        storage::{
            sqlite::sqlite_utils_tests::mock_sqlite_transaction, Config as StorageConfig,
            Transaction, TransactionStatus,
        },
        Config as MainConfig,
    };

    /// Test file = conway6.tx
    /// This test is expected to pass because the transaction is valid.
    #[tokio::test]
    async fn it_should_validate_tx() {
        let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
        let storage = Arc::new(mock_sqlite_transaction().await);
        let config = init_config();
        let u5c_data_adapter = Arc::new(MockU5CAdapter);
        let priority = Arc::new(Priority::new(storage.clone(), config.queues));

        let stage = ingest::Stage::new(storage.clone(), priority, u5c_data_adapter);

        let tx_cbor = include_str!("../../test/conway6.tx");
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

    /// Test file = conway6.tx
    /// This test is expected to pass because the transaction is valid.
    #[tokio::test]
    async fn it_should_evaluate_tx() {
        let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
        let storage = Arc::new(mock_sqlite_transaction().await);
        let config = init_config();
        let u5c_data_adapter = Arc::new(MockU5CAdapter);
        let priority = Arc::new(Priority::new(storage.clone(), config.queues));

        let stage = ingest::Stage::new(storage.clone(), priority, u5c_data_adapter);

        let tx_cbor = include_str!("../../test/conway6.tx");
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

    /// Test file = conway6.tx
    /// This test is expected to pass because the transaction is valid.
    #[tokio::test]
    async fn it_should_validate_and_evaluate_tx() {
        let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
        let storage = Arc::new(mock_sqlite_transaction().await);
        let config = init_config();
        let u5c_data_adapter = Arc::new(MockU5CAdapter);
        let priority = Arc::new(Priority::new(storage.clone(), config.queues));

        let stage = ingest::Stage::new(storage.clone(), priority, u5c_data_adapter);

        let tx_cbor = include_str!("../../test/conway6.tx");
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

    /// Test file = conway3.tx
    /// This test is expected to fail because the transaction is unwitnessed.
    #[tokio::test]
    async fn it_should_not_validate_tx() {
        let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
        let storage = Arc::new(mock_sqlite_transaction().await);
        let config = init_config();
        let u5c_data_adapter = Arc::new(MockU5CAdapter);
        let priority = Arc::new(Priority::new(storage.clone(), config.queues));

        let stage = ingest::Stage::new(storage.clone(), priority, u5c_data_adapter);

        let tx_cbor = include_str!("../../test/conway3.tx");
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

    /// Test file = conway4.tx
    /// This test is expected to fail because
    /// the transaction script hash is invalid.
    #[tokio::test]
    async fn it_should_not_evaluate_tx() {
        let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
        let storage = Arc::new(mock_sqlite_transaction().await);
        let config = init_config();
        let u5c_data_adapter = Arc::new(MockU5CAdapter);
        let priority = Arc::new(Priority::new(storage.clone(), config.queues));

        let stage = ingest::Stage::new(storage.clone(), priority, u5c_data_adapter);

        let tx_cbor = include_str!("../../test/conway4.tx");
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

    fn init_config() -> MainConfig {
        let server = ServerConfig {
            listen_address: SocketAddr::V4(SocketAddrV4::from_str("0.0.0.0:50052").unwrap()),
        };

        let storage = StorageConfig {
            db_path: "boros.db".to_string(),
        };

        let peer_manager = PeerManagerConfig {
            peers: vec![
                "preview-r1.panl.org:3015".to_string(),
                "adaboy-preview-1c.gleeze.com:5000".to_string(),
                "testicles.kiwipool.org:9720".to_string(),
            ],
            desired_peer_count: 10,
            peers_per_request: 10,
        };

        let monitor = MonitorConfig {
            retry_slot_diff: 1000,
        };

        let queues: HashSet<QueueConfig> = vec![QueueConfig {
            name: "banana".to_string(),
            weight: 2,
            chained: false,
        }]
        .into_iter()
        .collect();

        let u5c = U5cConfig {
            uri: "u5c uri".to_string(),
            metadata: HashMap::new(),
        };

        MainConfig {
            server,
            storage,
            peer_manager,
            monitor,
            queues,
            u5c,
        }
    }

    const BLOCK_SLOT: u64 = 75139211;
    const BLOCK_HASH_BYTES: [u8; 32] = [
        127, 78, 39, 223, 11, 203, 187, 52, 135, 20, 143, 204, 179, 96, 87, 177, 182, 107, 20, 103,
        233, 81, 164, 111, 86, 100, 110, 82, 216, 24, 103, 112,
    ];
    struct MockU5CAdapter;

    #[async_trait::async_trait]
    impl U5cDataAdapter for MockU5CAdapter {
        async fn fetch_tip(&self) -> Result<Point, anyhow::Error> {
            Ok((BLOCK_SLOT, BLOCK_HASH_BYTES.to_vec()))
        }

        async fn fetch_pparams(
            &self,
            _era: Era,
        ) -> Result<MultiEraProtocolParameters, anyhow::Error> {
            Ok(mock_pparams())
        }

        async fn fetch_utxos(
            &self,
            utxo_refs: Vec<(Hash<32>, u32)>,
            era: Era,
        ) -> Result<HashMap<(Hash<32>, u32), EraCbor>, anyhow::Error> {
            let dummy_utxos = build_dummy_utxos(era);

            Ok(utxo_refs
                .into_iter()
                .filter_map(|key| dummy_utxos.get(&key).map(|cbor| (key, cbor.clone())))
                .collect())
        }

        async fn stream(&self) -> anyhow::Result<ChainSyncStream> {
            todo!()
        }
    }

    #[allow(dead_code)]
    fn build_dummy_utxos(era: Era) -> HashMap<(Hash<32>, u32), EraCbor> {
        let test_utxos = [
            (
                "1239c5041f4086d058751fdcc469c2d6c623d77a1105d673e0a96a954b3ca367",
                0,
                vec![
                    130, 88, 57, 0, 70, 147, 192, 172, 82, 93, 4, 92, 176, 164, 231, 91, 211, 173,
                    189, 105, 86, 179, 183, 68, 232, 141, 33, 224, 65, 252, 155, 99, 13, 240, 146,
                    0, 100, 25, 228, 105, 224, 199, 120, 118, 164, 153, 18, 75, 249, 3, 115, 91,
                    67, 76, 121, 137, 247, 168, 9, 10, 26, 0, 76, 75, 64,
                ],
            ),
            (
                "5a3b16ae983c3623b6f516a1a1bcafc124ddd7bebf8406f541c27f24a97d8d8c",
                0,
                vec![
                    163, 0, 88, 29, 112, 250, 174, 96, 7, 44, 69, 209, 33, 182, 229, 138, 227, 92,
                    98, 70, 147, 238, 61, 173, 158, 168, 237, 118, 94, 182, 247, 111, 159, 1, 26,
                    0, 24, 203, 38, 3, 216, 24, 88, 173, 130, 3, 88, 169, 88, 167, 1, 1, 0, 50, 50,
                    50, 50, 50, 50, 37, 51, 48, 2, 50, 50, 50, 50, 50, 83, 51, 0, 115, 55, 14, 144,
                    1, 24, 4, 27, 170, 0, 17, 50, 51, 34, 83, 51, 0, 163, 55, 14, 144, 0, 24, 5,
                    155, 170, 0, 81, 50, 50, 83, 51, 0, 243, 1, 16, 2, 21, 51, 48, 12, 51, 112,
                    233, 0, 1, 128, 105, 186, 160, 3, 19, 55, 30, 110, 184, 192, 64, 192, 56, 221,
                    80, 3, 155, 174, 48, 16, 48, 14, 55, 84, 96, 32, 96, 28, 110, 168, 0, 197, 133,
                    141, 215, 24, 7, 128, 9, 128, 97, 186, 160, 5, 22, 48, 12, 0, 19, 0, 195, 0,
                    208, 1, 48, 9, 55, 84, 0, 34, 198, 1, 70, 1, 96, 6, 96, 18, 0, 70, 1, 0, 4, 96,
                    16, 0, 38, 0, 134, 234, 128, 4, 82, 97, 54, 86, 87, 52, 170, 231, 85, 92, 242,
                    171, 159, 87, 66, 174, 137,
                ],
            ),
            (
                "5c7b2d3c2b871962e3af3785013addb1c66a60c18db73482109aee3f90400ed4",
                0,
                vec![
                    163, 0, 88, 29, 112, 250, 174, 96, 7, 44, 69, 209, 33, 182, 229, 138, 227, 92,
                    98, 70, 147, 238, 61, 173, 158, 168, 237, 118, 94, 182, 247, 111, 159, 1, 26,
                    5, 245, 225, 0, 2, 130, 1, 216, 24, 74, 216, 121, 159, 69, 104, 101, 108, 108,
                    111, 255,
                ],
            ),
            (
                "1d45a092dcfc7e074e33476640d585c9c6853c452991ca053f469f2cd3837d4b",
                0,
                vec![
                    163, 0, 88, 29, 112, 4, 190, 241, 104, 195, 74, 168, 212, 116, 34, 35, 49, 211,
                    6, 86, 117, 79, 223, 213, 150, 0, 87, 66, 192, 251, 151, 137, 37, 1, 26, 0, 15,
                    66, 64, 2, 130, 1, 216, 24, 67, 216, 121, 128,
                ],
            ),
            (
                "97d3baae626b92325447b97256fb07662c8183dca99c53b93916093f2b92cbf3",
                0,
                vec![
                    130, 88, 57, 0, 70, 147, 192, 172, 82, 93, 4, 92, 176, 164, 231, 91, 211, 173,
                    189, 105, 86, 179, 183, 68, 232, 141, 33, 224, 65, 252, 155, 99, 13, 240, 146,
                    0, 100, 25, 228, 105, 224, 199, 120, 118, 164, 153, 18, 75, 249, 3, 115, 91,
                    67, 76, 121, 137, 247, 168, 9, 10, 26, 0, 76, 75, 64,
                ],
            ),
        ];

        test_utxos
            .iter()
            .map(|(hash_str, index, cbor)| {
                let hash = Hash::from_str(hash_str).unwrap();
                ((hash, *index), EraCbor(era, cbor.clone()))
            })
            .collect()
    }

    #[allow(dead_code)]
    fn mock_pparams() -> MultiEraProtocolParameters {
        let pparams = ConwayProtParams {
            system_start: chrono::DateTime::parse_from_rfc3339("2022-10-25T00:00:00Z").unwrap(),
            epoch_length: 86400,
            slot_length: 1,
            minfee_a: 44,
            minfee_b: 155381,
            max_block_body_size: 90112,
            max_transaction_size: 16384,
            max_block_header_size: 1100,
            key_deposit: 2000000,
            pool_deposit: 500000000,
            desired_number_of_stake_pools: 500,
            protocol_version: (9, 0),
            min_pool_cost: 170000000,
            ada_per_utxo_byte: 4310,
            cost_models_for_script_languages: CostModels {
                plutus_v1: Some(vec![
                    100788, 420, 1, 1, 1000, 173, 0, 1, 1000, 59957, 4, 1, 11183, 32, 201305, 8356,
                    4, 16000, 100, 16000, 100, 16000, 100, 16000, 100, 16000, 100, 16000, 100, 100,
                    100, 16000, 100, 94375, 32, 132994, 32, 61462, 4, 72010, 178, 0, 1, 22151, 32,
                    91189, 769, 4, 2, 85848, 228465, 122, 0, 1, 1, 1000, 42921, 4, 2, 24548, 29498,
                    38, 1, 898148, 27279, 1, 51775, 558, 1, 39184, 1000, 60594, 1, 141895, 32,
                    83150, 32, 15299, 32, 76049, 1, 13169, 4, 22100, 10,
                ]),
                plutus_v2: Some(vec![
                    100788, 420, 1, 1, 1000, 173, 0, 1, 1000, 59957, 4, 1, 11183, 32, 201305, 8356,
                    4, 16000, 100, 16000, 100, 16000, 100, 16000, 100, 16000, 100, 16000, 100, 100,
                    100, 16000, 100, 94375, 32, 132994, 32, 61462, 4, 72010, 178, 0, 1, 22151, 32,
                    91189, 769, 4, 2, 85848, 228465, 122, 0, 1, 1, 1000, 42921, 4, 2, 24548, 29498,
                    38, 1, 898148, 27279, 1, 51775, 558, 1, 39184, 1000, 60594, 1, 141895, 32,
                    83150, 32, 15299, 32, 76049, 1, 13169, 4, 22100, 10, 28999, 74, 1, 28999, 74,
                    1, 43285, 552, 1, 44749, 541, 1, 33852, 32, 68246, 32, 72362, 32, 7243, 32,
                    7391, 32, 11546, 32, 85848, 228465, 122, 0, 1, 1, 90434, 519, 0, 1, 74433, 32,
                    85848, 228465, 122, 0, 1, 1, 85848, 228465, 122, 0, 1, 1, 955506, 213312, 0, 2,
                    270652, 22588, 4, 1457325, 64566, 4, 20467, 1, 4, 0, 141992, 32, 100788, 420,
                    1, 1, 81663, 32, 59498, 32, 20142, 32, 24588, 32, 20744, 32, 25933, 32, 24623,
                    32, 43053543, 10, 53384111, 14333, 10,
                ]),
                plutus_v3: Some(vec![
                    100788, 420, 1, 1, 1000, 173, 0, 1, 1000, 59957, 4, 1, 11183, 32, 201305, 8356,
                    4, 16000, 100, 16000, 100, 16000, 100, 16000, 100, 16000, 100, 16000, 100, 100,
                    100, 16000, 100, 94375, 32, 132994, 32, 61462, 4, 72010, 178, 0, 1, 22151, 32,
                    91189, 769, 4, 2, 85848, 123203, 7305, -900, 1716, 549, 57, 85848, 0, 1, 1,
                    1000, 42921, 4, 2, 24548, 29498, 38, 1, 898148, 27279, 1, 51775, 558, 1, 39184,
                    1000, 60594, 1, 141895, 32, 83150, 32, 15299, 32, 76049, 1, 13169, 4, 22100,
                    10, 28999, 74, 1, 28999, 74, 1, 43285, 552, 1, 44749, 541, 1, 33852, 32, 68246,
                    32, 72362, 32, 7243, 32, 7391, 32, 11546, 32, 85848, 123203, 7305, -900, 1716,
                    549, 57, 85848, 0, 1, 90434, 519, 0, 1, 74433, 32, 85848, 123203, 7305, -900,
                    1716, 549, 57, 85848, 0, 1, 1, 85848, 123203, 7305, -900, 1716, 549, 57, 85848,
                    0, 1, 955506, 213312, 0, 2, 270652, 22588, 4, 1457325, 64566, 4, 20467, 1, 4,
                    0, 141992, 32, 100788, 420, 1, 1, 81663, 32, 59498, 32, 20142, 32, 24588, 32,
                    20744, 32, 25933, 32, 24623, 32, 43053543, 10, 53384111, 14333, 10, 43574283,
                    26308, 10,
                ]),
                unknown: KeyValuePairs::from(vec![]),
            },
            execution_costs: ExUnitPrices {
                mem_price: RationalNumber {
                    numerator: 577,
                    denominator: 10000,
                },
                step_price: RationalNumber {
                    numerator: 721,
                    denominator: 10000000,
                },
            },
            max_tx_ex_units: ExUnits {
                mem: 14000000,
                steps: 10000000000,
            },
            max_block_ex_units: ExUnits {
                mem: 62000000,
                steps: 20000000000,
            },
            max_value_size: 5000,
            collateral_percentage: 150,
            max_collateral_inputs: 3,
            expansion_rate: RationalNumber {
                numerator: 6442451,
                denominator: 2147483648,
            },
            treasury_growth_rate: RationalNumber {
                numerator: 13421773,
                denominator: 67108864,
            },
            maximum_epoch: 18,
            pool_pledge_influence: RationalNumber {
                numerator: 5033165,
                denominator: 16777216,
            },
            pool_voting_thresholds: PoolVotingThresholds {
                motion_no_confidence: RationalNumber {
                    numerator: 51,
                    denominator: 100,
                },
                committee_normal: RationalNumber {
                    numerator: 51,
                    denominator: 100,
                },
                committee_no_confidence: RationalNumber {
                    numerator: 51,
                    denominator: 100,
                },
                hard_fork_initiation: RationalNumber {
                    numerator: 51,
                    denominator: 100,
                },
                security_voting_threshold: RationalNumber {
                    numerator: 51,
                    denominator: 100,
                },
            },
            drep_voting_thresholds: DRepVotingThresholds {
                motion_no_confidence: RationalNumber {
                    numerator: 67,
                    denominator: 100,
                },
                committee_normal: RationalNumber {
                    numerator: 67,
                    denominator: 100,
                },
                committee_no_confidence: RationalNumber {
                    numerator: 3,
                    denominator: 5,
                },
                update_constitution: RationalNumber {
                    numerator: 3,
                    denominator: 4,
                },
                hard_fork_initiation: RationalNumber {
                    numerator: 3,
                    denominator: 5,
                },
                pp_network_group: RationalNumber {
                    numerator: 67,
                    denominator: 100,
                },
                pp_economic_group: RationalNumber {
                    numerator: 67,
                    denominator: 100,
                },
                pp_technical_group: RationalNumber {
                    numerator: 67,
                    denominator: 100,
                },
                pp_governance_group: RationalNumber {
                    numerator: 3,
                    denominator: 4,
                },
                treasury_withdrawal: RationalNumber {
                    numerator: 67,
                    denominator: 100,
                },
            },
            min_committee_size: 0,
            committee_term_limit: 365,
            governance_action_validity_period: 30,
            governance_action_deposit: 100000000000,
            drep_deposit: 500000000,
            drep_inactivity_period: 20,
            minfee_refscript_cost_per_byte: RationalNumber {
                numerator: 15,
                denominator: 1,
            },
        };

        MultiEraProtocolParameters::Conway(pparams)
    }
}
