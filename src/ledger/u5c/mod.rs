use std::{collections::HashMap, pin::Pin, str::FromStr};

use anyhow::bail;
use async_stream::stream;
use chrono::{offset, DateTime, FixedOffset, NaiveDateTime};
use futures::{Stream, TryStreamExt};
use pallas::{
    codec::utils::KeyValuePairs,
    crypto::hash::Hash,
    interop::utxorpc::spec::{
        cardano::{
            CostModel, CostModels, ExPrices, ExUnits, ProtocolVersion, RationalNumber, Tx,
            VotingThresholds,
        },
        query::{
            any_chain_params::{self, Params},
            query_service_client::QueryServiceClient,
            ReadParamsRequest, ReadUtxosRequest, TxoRef,
        },
        sync::{
            any_chain_block, follow_tip_response, sync_service_client::SyncServiceClient, BlockRef,
            FollowTipRequest, ReadTipRequest,
        },
    },
    ledger::{
        primitives::{
            self,
            alonzo::Language,
            conway::{DRepVotingThresholds, PoolVotingThresholds},
            ExUnitPrices, Nonce, NonceVariant,
        },
        traverse::{update::ConwayCostModels, Era},
    },
    validate::utils::{
        AlonzoProtParams, BabbageProtParams, ByronProtParams, ConwayProtParams, EraCbor,
        MultiEraProtocolParameters, ShelleyProtParams,
    },
};
use serde::Deserialize;
use tonic::{
    metadata::{MetadataKey, MetadataValue},
    transport::{Channel, ClientTlsConfig, Uri},
    Request,
};
use tracing::info;

pub type Point = (u64, Vec<u8>);
pub type ChainSyncStream = Pin<Box<dyn Stream<Item = anyhow::Result<Event>> + Send>>;

#[derive(Debug)]
pub enum Event {
    RollForward(Point, Vec<Tx>),
    Rollback(Point),
}

#[derive(Deserialize, Clone)]
pub struct Config {
    uri: String,
    metadata: HashMap<String, String>,
}

// TODO: remove dead_code after the implementation
#[allow(dead_code)]
#[async_trait::async_trait]
pub trait U5cDataAdapter: Send + Sync {
    async fn fetch_tip(&self) -> anyhow::Result<Point>;
    async fn fetch_utxos(
        &self,
        utxo_refs: Vec<(Hash<32>, u32)>,
        era: Era,
    ) -> Result<HashMap<(Hash<32>, u32), EraCbor>, anyhow::Error>;
    async fn stream(&self) -> anyhow::Result<ChainSyncStream>;
    async fn fetch_pparams(&self, era: Era) -> Result<MultiEraProtocolParameters, anyhow::Error>;
}

pub struct U5cDataAdapterImpl {
    channel: Channel,
    metadata: HashMap<String, String>,
    cursor: Option<Point>,
}

impl U5cDataAdapterImpl {
    pub async fn try_new(config: Config, cursor: Option<Point>) -> anyhow::Result<Self> {
        let uri: Uri = config.uri.parse()?;

        let channel = Channel::builder(uri)
            .tls_config(ClientTlsConfig::new().with_webpki_roots())?
            .connect()
            .await?;

        Ok(Self {
            channel,
            metadata: config.metadata,
            cursor,
        })
    }

    fn interceptor(&self, req: &mut Request<()>) {
        self.metadata.clone().into_iter().for_each(|(key, value)| {
            req.metadata_mut().insert(
                MetadataKey::from_str(&key).unwrap(),
                MetadataValue::from_str(&value).unwrap(),
            );
        });
    }
}

#[async_trait::async_trait]
impl U5cDataAdapter for U5cDataAdapterImpl {
    async fn fetch_tip(&self) -> anyhow::Result<Point> {
        let mut client = SyncServiceClient::with_interceptor(
            self.channel.clone(),
            move |mut req: Request<()>| {
                self.interceptor(&mut req);
                Ok(req)
            },
        );

        let response = client
            .read_tip(tonic::Request::new(ReadTipRequest {}))
            .await?
            .into_inner();

        let point = match response.tip {
            Some(ref block_ref) => (block_ref.index, block_ref.hash.to_vec()),
            None => bail!("U5c none tip"),
        };

        Ok(point)
    }

    async fn fetch_utxos(
        &self,
        utxo_refs: Vec<(Hash<32>, u32)>,
        era: Era,
    ) -> Result<HashMap<(Hash<32>, u32), EraCbor>, anyhow::Error> {
        let mut client = QueryServiceClient::with_interceptor(
            self.channel.clone(),
            move |mut req: Request<()>| {
                self.interceptor(&mut req);
                Ok(req)
            },
        );

        let utxo_refs: Vec<TxoRef> = utxo_refs
            .into_iter()
            .map(|(hash, index)| TxoRef {
                hash: tonic::codegen::Bytes::copy_from_slice(AsRef::as_ref(&hash)),
                index,
            })
            .collect();

        let response = client
            .read_utxos(tonic::Request::new(ReadUtxosRequest {
                keys: utxo_refs,
                field_mask: None,
            }))
            .await?
            .into_inner();

        let any_utxos = response.items;
        let mut utxos: HashMap<(Hash<32>, u32), EraCbor> = HashMap::new();

        for any_utxo in any_utxos {
            let native_bytes = any_utxo.native_bytes;

            let txoref = any_utxo.txo_ref.unwrap();
            let txoref = (Hash::from(txoref.hash.as_ref()), txoref.index);

            let era_cbor = EraCbor(era, native_bytes.as_ref().to_vec());

            utxos.insert(txoref, era_cbor);
        }

        Ok(utxos)
    }

    async fn fetch_pparams(&self, era: Era) -> Result<MultiEraProtocolParameters, anyhow::Error> {
        let mut client = QueryServiceClient::with_interceptor(
            self.channel.clone(),
            move |mut req: Request<()>| {
                self.interceptor(&mut req);
                Ok(req)
            },
        );

        let response = client
            .read_params(tonic::Request::new(ReadParamsRequest { field_mask: None }))
            .await?
            .into_inner();

        let pparams = response.values.unwrap_or_default();
        let pparams = pparams.params.unwrap();

        let byron_pparams = get_byron_pparams(&pparams);
        let shelley_pparams = get_shelley_params(&pparams);
        let alonzo_pparams = get_alonzo_pparams(&pparams);
        let babbage_pparams = get_babbage_pparams(&pparams);
        let conway_pparams = get_conway_pparams(&pparams);

        let byron_pparams = MultiEraProtocolParameters::Byron(byron_pparams);
        let shelley_pparams = MultiEraProtocolParameters::Shelley(shelley_pparams);
        let alonzo_pparams = MultiEraProtocolParameters::Alonzo(alonzo_pparams);
        let babbage_pparams = MultiEraProtocolParameters::Babbage(babbage_pparams);
        let conway_pparams = MultiEraProtocolParameters::Conway(conway_pparams);

        let multi_era_pparams = match era {
            Era::Byron => byron_pparams,
            Era::Shelley => shelley_pparams,
            Era::Alonzo => alonzo_pparams,
            Era::Babbage => babbage_pparams,
            Era::Conway => conway_pparams,
            _ => bail!("U5c era not implemented"),
        };

        Ok(multi_era_pparams)
    }

    async fn stream(&self) -> anyhow::Result<ChainSyncStream> {
        info!("U5C connected");

        let mut client = SyncServiceClient::with_interceptor(
            self.channel.clone(),
            move |mut req: Request<()>| {
                self.interceptor(&mut req);
                Ok(req)
            },
        );

        let follow_tip_request = match &self.cursor {
            Some((slot, hash)) => {
                info!("U5C starting from slot {}", slot);
                FollowTipRequest {
                    intersect: vec![BlockRef {
                        index: *slot,
                        hash: hash.clone().into(),
                    }],
                    ..Default::default()
                }
            }
            None => {
                info!("U5C starting from tip");
                FollowTipRequest::default()
            }
        };

        let mut tip_stream = client
            .follow_tip(tonic::Request::new(follow_tip_request))
            .await?
            .into_inner();

        let stream = stream! {
            while let Some(follow_tip) = tip_stream.try_next().await? {
                if let Some(action) = follow_tip.action {
                    match action {
                        follow_tip_response::Action::Apply(any) => {
                            match any.chain.unwrap() {
                                any_chain_block::Chain::Cardano(block) => {
                                    if let Some(body) = block.body {
                                        let header = block.header.unwrap();
                                        yield Ok(Event::RollForward((header.slot, header.hash.to_vec()), body.tx));
                                    }
                                },
                            }
                        },
                        follow_tip_response::Action::Undo(any) => {
                            match any.chain.unwrap() {
                                any_chain_block::Chain::Cardano(block) => {
                                    let header = block.header.unwrap();
                                    yield Ok(Event::Rollback((header.slot, header.hash.to_vec())));
                                },
                            }
                        },
                        follow_tip_response::Action::Reset(_block_ref) => {
                            info!("U5C reset not implemented yet");
                        },
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}

fn get_byron_pparams(pparams: &Params) -> ByronProtParams {
    match pparams {
        any_chain_params::Params::Cardano(params) => ByronProtParams {
            block_version: (0, 0, 0),
            start_time: 0,
            script_version: 0,
            slot_duration: 1,
            max_block_size: params.max_block_body_size + params.max_block_header_size,
            max_header_size: params.max_block_header_size,
            max_tx_size: params.max_tx_size,
            max_proposal_size: 0,
            mpc_thd: 0,
            heavy_del_thd: 0,
            update_vote_thd: 0,
            update_proposal_thd: 0,
            update_implicit: 0,
            soft_fork_rule: (0, 0, 0),
            summand: 0,
            multiplier: 0,
            unlock_stake_epoch: 0,
        },
    }
}

fn get_shelley_params(pparams: &Params) -> ShelleyProtParams {
    match pparams.clone() {
        any_chain_params::Params::Cardano(params) => {
            let monetary_expansion = params.monetary_expansion.unwrap_or_else(|| RationalNumber {
                numerator: 1,
                denominator: 1,
            });

            let treasury_expansion = params.treasury_expansion.unwrap_or_else(|| RationalNumber {
                numerator: 1,
                denominator: 1,
            });

            let pool_influence = params.pool_influence.unwrap_or_else(|| RationalNumber {
                numerator: 1,
                denominator: 1,
            });

            let protocol_version = params
                .protocol_version
                .unwrap_or_else(|| ProtocolVersion { major: 1, minor: 1 });

            ShelleyProtParams {
                system_start: DateTime::<FixedOffset>::from_naive_utc_and_offset(
                    NaiveDateTime::default(),
                    offset::FixedOffset::east_opt(0)
                        .unwrap_or_else(|| offset::FixedOffset::east_opt(0).unwrap()),
                ),
                epoch_length: 0,
                slot_length: 0,
                minfee_a: params.min_fee_coefficient as u32,
                minfee_b: params.min_fee_constant as u32,
                max_block_body_size: params.max_block_body_size as u32,
                max_transaction_size: params.max_tx_size as u32,
                max_block_header_size: params.max_block_header_size as u32,
                key_deposit: params.stake_key_deposit,
                pool_deposit: params.pool_deposit,
                desired_number_of_stake_pools: params.desired_number_of_pools as u32,
                protocol_version: {
                    let proto = protocol_version;
                    (proto.major as u64, proto.minor as u64)
                },
                min_utxo_value: 0,
                min_pool_cost: params.min_pool_cost,
                expansion_rate: primitives::RationalNumber {
                    numerator: monetary_expansion.numerator as u64,
                    denominator: monetary_expansion.denominator as u64,
                },
                treasury_growth_rate: primitives::RationalNumber {
                    numerator: treasury_expansion.numerator as u64,
                    denominator: treasury_expansion.denominator as u64,
                },
                maximum_epoch: 0,
                pool_pledge_influence: primitives::RationalNumber {
                    numerator: pool_influence.numerator as u64,
                    denominator: pool_influence.denominator as u64,
                },
                decentralization_constant: primitives::RationalNumber {
                    numerator: 1,
                    denominator: 1,
                },
                extra_entropy: Nonce {
                    variant: NonceVariant::Nonce,
                    hash: None,
                },
            }
        }
    }
}

fn get_alonzo_pparams(pparams: &Params) -> AlonzoProtParams {
    match pparams.clone() {
        any_chain_params::Params::Cardano(params) => {
            let monetary_expansion = params.monetary_expansion.unwrap_or_else(|| RationalNumber {
                numerator: 1,
                denominator: 1,
            });
            let treasury_expansion = params.treasury_expansion.unwrap_or_else(|| RationalNumber {
                numerator: 1,
                denominator: 1,
            });
            let pool_influence = params.pool_influence.unwrap_or_else(|| RationalNumber {
                numerator: 1,
                denominator: 1,
            });
            let execution_costs = params.prices.unwrap_or_else(|| ExPrices {
                memory: Some(RationalNumber {
                    numerator: 1,
                    denominator: 1,
                }),
                steps: Some(RationalNumber {
                    numerator: 1,
                    denominator: 1,
                }),
            });
            AlonzoProtParams {
                system_start: DateTime::<FixedOffset>::from_naive_utc_and_offset(
                    NaiveDateTime::default(),
                    offset::FixedOffset::east_opt(0)
                        .unwrap_or_else(|| offset::FixedOffset::east_opt(0).unwrap()),
                ),
                epoch_length: 0,
                slot_length: 0,
                minfee_a: params.min_fee_coefficient as u32,
                minfee_b: params.min_fee_constant as u32,
                max_block_body_size: params.max_block_body_size as u32,
                max_transaction_size: params.max_tx_size as u32,
                max_block_header_size: params.max_block_header_size as u32,
                key_deposit: params.stake_key_deposit,
                pool_deposit: params.pool_deposit,
                desired_number_of_stake_pools: params.desired_number_of_pools as u32,
                protocol_version: {
                    let proto = params
                        .protocol_version
                        .unwrap_or_else(|| ProtocolVersion { major: 1, minor: 1 });
                    (proto.major as u64, proto.minor as u64)
                },
                min_pool_cost: params.min_pool_cost,
                ada_per_utxo_byte: params.coins_per_utxo_byte,
                cost_models_for_script_languages: KeyValuePairs::Indef(vec![(
                    Language::PlutusV1,
                    match params.cost_models {
                        Some(cost_models) => {
                            match cost_models.plutus_v1 {
                                Some(plutus_v1) => plutus_v1.values,
                                None => {
                                    vec![] // Provide an empty vec if not found
                                }
                            }
                        }
                        None => {
                            vec![] // Default empty cost models if not found
                        }
                    },
                )]),
                execution_costs: ExUnitPrices {
                    mem_price: primitives::RationalNumber {
                        numerator: execution_costs
                            .memory
                            .clone()
                            .unwrap_or(RationalNumber {
                                numerator: 1,
                                denominator: 5,
                            })
                            .numerator as u64,
                        denominator: execution_costs
                            .memory
                            .clone()
                            .unwrap_or(RationalNumber {
                                numerator: 1,
                                denominator: 1,
                            })
                            .denominator as u64,
                    },
                    step_price: primitives::RationalNumber {
                        numerator: execution_costs
                            .steps
                            .clone()
                            .unwrap_or(RationalNumber {
                                numerator: 1,
                                denominator: 1,
                            })
                            .numerator as u64,
                        denominator: execution_costs
                            .steps
                            .clone()
                            .unwrap_or(RationalNumber {
                                numerator: 1,
                                denominator: 1,
                            })
                            .denominator as u64,
                    },
                },
                max_tx_ex_units: primitives::ExUnits {
                    mem: params
                        .max_execution_units_per_transaction
                        .clone()
                        .unwrap_or(ExUnits {
                            memory: 1,
                            steps: 1,
                        })
                        .memory as u64,
                    steps: params
                        .max_execution_units_per_transaction
                        .clone()
                        .unwrap_or(ExUnits {
                            memory: 1,
                            steps: 1,
                        })
                        .steps as u64,
                },
                max_block_ex_units: primitives::ExUnits {
                    mem: params
                        .max_execution_units_per_block
                        .clone()
                        .unwrap_or(ExUnits {
                            memory: 1,
                            steps: 1,
                        })
                        .memory as u64,
                    steps: params
                        .max_execution_units_per_block
                        .clone()
                        .unwrap_or(ExUnits {
                            memory: 1,
                            steps: 1,
                        })
                        .steps as u64,
                },
                max_value_size: params.max_value_size as u32,
                collateral_percentage: params.collateral_percentage as u32,
                max_collateral_inputs: params.max_collateral_inputs as u32,
                expansion_rate: primitives::RationalNumber {
                    numerator: monetary_expansion.numerator as u64,
                    denominator: monetary_expansion.denominator as u64,
                },
                treasury_growth_rate: primitives::RationalNumber {
                    numerator: treasury_expansion.numerator as u64,
                    denominator: treasury_expansion.denominator as u64,
                },
                maximum_epoch: 0,
                pool_pledge_influence: primitives::RationalNumber {
                    numerator: pool_influence.numerator as u64,
                    denominator: pool_influence.denominator as u64,
                },
                decentralization_constant: primitives::RationalNumber {
                    numerator: 1,
                    denominator: 1,
                },
                extra_entropy: Nonce {
                    variant: NonceVariant::Nonce,
                    hash: None,
                },
            }
        }
    }
}

fn get_babbage_pparams(pparams: &Params) -> BabbageProtParams {
    match pparams.clone() {
        any_chain_params::Params::Cardano(params) => {
            let monetary_expansion = params.monetary_expansion.unwrap_or_else(|| RationalNumber {
                numerator: 1,
                denominator: 1,
            });
            let treasury_expansion = params.treasury_expansion.unwrap_or_else(|| RationalNumber {
                numerator: 1,
                denominator: 1,
            });
            let pool_influence = params.pool_influence.unwrap_or_else(|| RationalNumber {
                numerator: 1,
                denominator: 1,
            });
            let execution_costs = params.prices.unwrap_or_else(|| ExPrices {
                memory: Some(RationalNumber {
                    numerator: 1,
                    denominator: 1,
                }),
                steps: Some(RationalNumber {
                    numerator: 1,
                    denominator: 1,
                }),
            });
            let cost_models = params.cost_models.unwrap_or_else(|| CostModels {
                plutus_v1: Some(CostModel { values: vec![] }),
                plutus_v2: Some(CostModel { values: vec![] }),
                plutus_v3: Some(CostModel { values: vec![] }),
            });
            let max_execution_units_per_transaction = params
                .max_execution_units_per_transaction
                .unwrap_or(ExUnits {
                    memory: 1,
                    steps: 1,
                });
            let max_execution_units_per_block =
                params.max_execution_units_per_block.unwrap_or(ExUnits {
                    memory: 1,
                    steps: 1,
                });
            BabbageProtParams {
                system_start: DateTime::<FixedOffset>::from_naive_utc_and_offset(
                    NaiveDateTime::default(),
                    offset::FixedOffset::east_opt(0)
                        .unwrap_or_else(|| offset::FixedOffset::east_opt(0).unwrap()),
                ),
                epoch_length: 0,
                slot_length: 0,
                minfee_a: params.min_fee_coefficient as u32,
                minfee_b: params.min_fee_constant as u32,
                max_block_body_size: params.max_block_body_size as u32,
                max_transaction_size: params.max_tx_size as u32,
                max_block_header_size: params.max_block_header_size as u32,
                key_deposit: params.stake_key_deposit,
                pool_deposit: params.pool_deposit,
                desired_number_of_stake_pools: params.desired_number_of_pools as u32,
                protocol_version: {
                    let proto = params
                        .protocol_version
                        .as_ref()
                        .unwrap_or(&ProtocolVersion { major: 1, minor: 1 });
                    (proto.major as u64, proto.minor as u64)
                },
                min_pool_cost: params.min_pool_cost,
                ada_per_utxo_byte: params.coins_per_utxo_byte,
                cost_models_for_script_languages: primitives::babbage::CostModels {
                    plutus_v1: Some(
                        cost_models
                            .plutus_v1
                            .unwrap_or(CostModel { values: vec![] })
                            .values,
                    ),
                    plutus_v2: Some(
                        cost_models
                            .plutus_v2
                            .unwrap_or(CostModel { values: vec![] })
                            .values,
                    ),
                },
                execution_costs: ExUnitPrices {
                    mem_price: primitives::RationalNumber {
                        numerator: execution_costs
                            .memory
                            .clone()
                            .unwrap_or(RationalNumber {
                                numerator: 1,
                                denominator: 1,
                            })
                            .numerator as u64,
                        denominator: execution_costs
                            .memory
                            .clone()
                            .unwrap_or(RationalNumber {
                                numerator: 1,
                                denominator: 1,
                            })
                            .denominator as u64,
                    },
                    step_price: primitives::RationalNumber {
                        numerator: execution_costs
                            .steps
                            .clone()
                            .unwrap_or(RationalNumber {
                                numerator: 1,
                                denominator: 1,
                            })
                            .numerator as u64,
                        denominator: execution_costs
                            .steps
                            .clone()
                            .unwrap_or(RationalNumber {
                                numerator: 1,
                                denominator: 1,
                            })
                            .denominator as u64,
                    },
                },
                max_tx_ex_units: primitives::ExUnits {
                    mem: max_execution_units_per_transaction.memory as u64,
                    steps: max_execution_units_per_transaction.steps as u64,
                },
                max_block_ex_units: primitives::ExUnits {
                    mem: max_execution_units_per_block.memory as u64,
                    steps: max_execution_units_per_block.steps as u64,
                },
                max_value_size: params.max_value_size as u32,
                collateral_percentage: params.collateral_percentage as u32,
                max_collateral_inputs: params.max_collateral_inputs as u32,
                expansion_rate: primitives::RationalNumber {
                    numerator: monetary_expansion.numerator as u64,
                    denominator: monetary_expansion.denominator as u64,
                },
                treasury_growth_rate: primitives::RationalNumber {
                    numerator: treasury_expansion.numerator as u64,
                    denominator: treasury_expansion.denominator as u64,
                },
                maximum_epoch: 0,
                pool_pledge_influence: primitives::RationalNumber {
                    numerator: pool_influence.numerator as u64,
                    denominator: pool_influence.denominator as u64,
                },
                decentralization_constant: primitives::RationalNumber {
                    numerator: 1,
                    denominator: 1,
                },
                extra_entropy: Nonce {
                    variant: NonceVariant::Nonce,
                    hash: None,
                },
            }
        }
    }
}

fn get_conway_pparams(pparams: &Params) -> ConwayProtParams {
    match pparams.clone() {
        any_chain_params::Params::Cardano(params) => {
            let pool_influence = params.pool_influence.unwrap_or_else(|| RationalNumber {
                numerator: 1,
                denominator: 1,
            });
            let monetary_expansion = params.monetary_expansion.unwrap_or_else(|| RationalNumber {
                numerator: 1,
                denominator: 1,
            });
            let treasury_expansion = params.treasury_expansion.unwrap_or_else(|| RationalNumber {
                numerator: 1,
                denominator: 1,
            });
            let execution_costs = params.prices.unwrap_or_else(|| ExPrices {
                memory: Some(RationalNumber {
                    numerator: 1,
                    denominator: 1,
                }),
                steps: Some(RationalNumber {
                    numerator: 1,
                    denominator: 1,
                }),
            });
            let cost_models = params.cost_models.unwrap_or_else(|| CostModels {
                plutus_v1: Some(CostModel { values: vec![] }),
                plutus_v2: Some(CostModel { values: vec![] }),
                plutus_v3: Some(CostModel { values: vec![] }),
            });
            let max_execution_units_per_transaction = params
                .max_execution_units_per_transaction
                .unwrap_or(ExUnits::default());
            let max_execution_units_per_block = params
                .max_execution_units_per_block
                .unwrap_or(ExUnits::default());
            let pool_voting_thresholds =
                params
                    .pool_voting_thresholds
                    .unwrap_or_else(|| VotingThresholds {
                        thresholds: vec![
                            RationalNumber {
                                numerator: 1,
                                denominator: 1
                            };
                            5
                        ],
                    });
            let drep_voting_thresholds =
                params
                    .drep_voting_thresholds
                    .unwrap_or_else(|| VotingThresholds {
                        thresholds: vec![
                            RationalNumber {
                                numerator: 1,
                                denominator: 1
                            };
                            10
                        ],
                    });
            let min_fee_script_ref_cost_per_byte = params
                .min_fee_script_ref_cost_per_byte
                .unwrap_or(RationalNumber {
                    numerator: 1,
                    denominator: 5,
                });

            ConwayProtParams {
                system_start: DateTime::<FixedOffset>::from_naive_utc_and_offset(
                    NaiveDateTime::default(),
                    offset::FixedOffset::east_opt(0).unwrap(),
                ),
                epoch_length: 0,
                slot_length: 0,
                minfee_a: params.min_fee_coefficient as u32,
                minfee_b: params.min_fee_constant as u32,
                max_block_body_size: params.max_block_body_size as u32,
                max_transaction_size: params.max_tx_size as u32,
                max_block_header_size: params.max_block_header_size as u32,
                key_deposit: params.stake_key_deposit,
                pool_deposit: params.pool_deposit,
                desired_number_of_stake_pools: params.desired_number_of_pools as u32,
                protocol_version: {
                    let proto = params
                        .protocol_version
                        .as_ref()
                        .unwrap_or(&ProtocolVersion { major: 1, minor: 1 });
                    (proto.major as u64, proto.minor as u64)
                },
                min_pool_cost: params.min_pool_cost,
                ada_per_utxo_byte: params.coins_per_utxo_byte,
                cost_models_for_script_languages: ConwayCostModels {
                    plutus_v1: Some(
                        cost_models
                            .plutus_v1
                            .unwrap_or(CostModel { values: vec![] })
                            .values,
                    ),
                    plutus_v2: Some(
                        cost_models
                            .plutus_v2
                            .unwrap_or(CostModel { values: vec![] })
                            .values,
                    ),
                    plutus_v3: Some(
                        cost_models
                            .plutus_v3
                            .unwrap_or(CostModel { values: vec![] })
                            .values,
                    ),
                },
                execution_costs: ExUnitPrices {
                    mem_price: primitives::RationalNumber {
                        numerator: execution_costs
                            .memory
                            .clone()
                            .unwrap_or(RationalNumber {
                                numerator: 1,
                                denominator: 1,
                            })
                            .numerator as u64,
                        denominator: execution_costs
                            .memory
                            .clone()
                            .unwrap_or(RationalNumber {
                                numerator: 1,
                                denominator: 1,
                            })
                            .denominator as u64,
                    },
                    step_price: primitives::RationalNumber {
                        numerator: execution_costs
                            .steps
                            .clone()
                            .unwrap_or(RationalNumber {
                                numerator: 1,
                                denominator: 1,
                            })
                            .numerator as u64,
                        denominator: execution_costs
                            .steps
                            .clone()
                            .unwrap_or(RationalNumber {
                                numerator: 1,
                                denominator: 1,
                            })
                            .denominator as u64,
                    },
                },
                max_tx_ex_units: primitives::ExUnits {
                    mem: max_execution_units_per_transaction.memory as u64,
                    steps: max_execution_units_per_transaction.steps as u64,
                },
                max_block_ex_units: primitives::ExUnits {
                    mem: max_execution_units_per_block.memory as u64,
                    steps: max_execution_units_per_block.steps as u64,
                },
                max_value_size: params.max_value_size as u32,
                collateral_percentage: params.collateral_percentage as u32,
                max_collateral_inputs: params.max_collateral_inputs as u32,
                expansion_rate: primitives::RationalNumber {
                    numerator: monetary_expansion.numerator as u64,
                    denominator: monetary_expansion.denominator as u64,
                },
                treasury_growth_rate: primitives::RationalNumber {
                    numerator: treasury_expansion.numerator as u64,
                    denominator: treasury_expansion.denominator as u64,
                },
                maximum_epoch: 0,
                pool_pledge_influence: primitives::RationalNumber {
                    numerator: pool_influence.numerator as u64,
                    denominator: pool_influence.denominator as u64,
                },
                pool_voting_thresholds: PoolVotingThresholds {
                    motion_no_confidence: primitives::RationalNumber {
                        numerator: pool_voting_thresholds.thresholds[0].numerator as u64,
                        denominator: pool_voting_thresholds.thresholds[0].denominator as u64,
                    },
                    committee_normal: primitives::RationalNumber {
                        numerator: pool_voting_thresholds.thresholds[1].numerator as u64,
                        denominator: pool_voting_thresholds.thresholds[1].denominator as u64,
                    },
                    committee_no_confidence: primitives::RationalNumber {
                        numerator: pool_voting_thresholds.thresholds[2].numerator as u64,
                        denominator: pool_voting_thresholds.thresholds[2].denominator as u64,
                    },
                    hard_fork_initiation: primitives::RationalNumber {
                        numerator: pool_voting_thresholds.thresholds[3].numerator as u64,
                        denominator: pool_voting_thresholds.thresholds[3].denominator as u64,
                    },
                    security_voting_threshold: primitives::RationalNumber {
                        numerator: pool_voting_thresholds.thresholds[4].numerator as u64,
                        denominator: pool_voting_thresholds.thresholds[4].denominator as u64,
                    },
                },
                drep_voting_thresholds: DRepVotingThresholds {
                    motion_no_confidence: primitives::RationalNumber {
                        numerator: drep_voting_thresholds.thresholds[0].numerator as u64,
                        denominator: drep_voting_thresholds.thresholds[0].denominator as u64,
                    },
                    committee_normal: primitives::RationalNumber {
                        numerator: drep_voting_thresholds.thresholds[1].numerator as u64,
                        denominator: drep_voting_thresholds.thresholds[1].denominator as u64,
                    },
                    committee_no_confidence: primitives::RationalNumber {
                        numerator: drep_voting_thresholds.thresholds[2].numerator as u64,
                        denominator: drep_voting_thresholds.thresholds[2].denominator as u64,
                    },
                    update_constitution: primitives::RationalNumber {
                        numerator: drep_voting_thresholds.thresholds[3].numerator as u64,
                        denominator: drep_voting_thresholds.thresholds[3].denominator as u64,
                    },
                    hard_fork_initiation: primitives::RationalNumber {
                        numerator: drep_voting_thresholds.thresholds[4].numerator as u64,
                        denominator: drep_voting_thresholds.thresholds[4].denominator as u64,
                    },
                    pp_network_group: primitives::RationalNumber {
                        numerator: drep_voting_thresholds.thresholds[5].numerator as u64,
                        denominator: drep_voting_thresholds.thresholds[5].denominator as u64,
                    },
                    pp_economic_group: primitives::RationalNumber {
                        numerator: drep_voting_thresholds.thresholds[6].numerator as u64,
                        denominator: drep_voting_thresholds.thresholds[6].denominator as u64,
                    },
                    pp_technical_group: primitives::RationalNumber {
                        numerator: drep_voting_thresholds.thresholds[7].numerator as u64,
                        denominator: drep_voting_thresholds.thresholds[7].denominator as u64,
                    },
                    pp_governance_group: primitives::RationalNumber {
                        numerator: drep_voting_thresholds.thresholds[8].numerator as u64,
                        denominator: drep_voting_thresholds.thresholds[8].denominator as u64,
                    },
                    treasury_withdrawal: primitives::RationalNumber {
                        numerator: drep_voting_thresholds.thresholds[9].numerator as u64,
                        denominator: drep_voting_thresholds.thresholds[9].denominator as u64,
                    },
                },
                min_committee_size: params.min_committee_size.into(),
                committee_term_limit: params.committee_term_limit,
                governance_action_validity_period: params.governance_action_validity_period,
                governance_action_deposit: params.governance_action_deposit,
                drep_deposit: params.drep_deposit,
                drep_inactivity_period: params.drep_inactivity_period,
                minfee_refscript_cost_per_byte: primitives::RationalNumber {
                    numerator: min_fee_script_ref_cost_per_byte.numerator as u64,
                    denominator: min_fee_script_ref_cost_per_byte.denominator as u64,
                },
            }
        }
    }
}

#[cfg(test)]
mod u5c_tests {
    // use super::*;
    // use std::collections::HashMap;

    // pub struct MockU5cDataAdapter {
    //     pub known_utxos: HashMap<String, Vec<u8>>,
    // }

    // #[async_trait::async_trait]
    // impl U5cDataAdapter for MockU5cDataAdapter {
    //     async fn fetch_tip(&self) -> anyhow::Result<Point> {
    //         todo!()
    //     }

    //     // async fn fetch_utxos(
    //     //     &self,
    //     //     utxo_refs: Vec<TxoRef>,
    //     // ) -> anyhow::Result<HashMap<String, Vec<u8>>> {
    //     //     let mut result = HashMap::new();

    //     //     // Check each requested reference; if known, clone the data into the result
    //     //     for reference in utxo_refs {
    //     //         if let Some(cbor_data) = self.known_utxos.get(reference) {
    //     //             result.insert(reference.clone(), cbor_data.clone());
    //     //         }
    //     //     }

    //     //     Ok(result)
    //     // }

    //     // async fn fetch_pparams(
    //     //     &self,
    //     //     era: Era,
    //     // ) -> Result<MultiEraProtocolParameters, anyhow::Error> {
    //     //     todo!()
    //     // }

    //     // async fn stream(&self) -> anyhow::Result<ChainSyncStream> {
    //     //     todo!()
    //     // }
    // }

    // #[tokio::test]
    // async fn it_returns_found_utxos_only() {
    //     // Arrange: a mock with two known references
    //     let mut known = HashMap::new();
    //     known.insert("abc123#0".to_string(), vec![0x82, 0xa0]); // example CBOR
    //     known.insert("abc123#1".to_string(), vec![0x83, 0x04]);

    //     // let provider = MockU5cDataAdapter { known_utxos: known };

    //     // // We'll request three references, one of which doesn't exist
    //     // let requested = vec![
    //     //     "abc123#0".to_string(),
    //     //     "abc123#1".to_string(),
    //     //     "missing#2".to_string(),
    //     // ];

    //     // Act
    //     // let results = provider.fetch_utxos(&requested).await.unwrap();

    //     // Assert
    //     // assert_eq!(results.len(), 2, "Should only contain two known references");
    //     // assert!(results.contains_key("abc123#0"));
    //     // assert!(results.contains_key("abc123#1"));
    //     // assert!(!results.contains_key("missing#2"));
    // }
}
