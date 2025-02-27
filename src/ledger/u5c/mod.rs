use std::{collections::HashMap, pin::Pin, str::FromStr, vec};

use anyhow::bail;
use async_stream::stream;
use futures::{Stream, TryStreamExt};
use pallas::{
    codec::utils::KeyValuePairs, crypto::hash::Hash, interop::utxorpc::spec::{
        cardano::{
            CostModel, Tx,
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
    }, ledger::{
        primitives::{
            self, conway::{DRepVotingThresholds, PoolVotingThresholds}, ExUnitPrices
        },
        traverse::{update::ConwayCostModels, Era}, validate::utils::{ConwayProtParams, EraCbor, MultiEraProtocolParameters},
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

        let multi_era_pparams = map_multi_era_pparams(&pparams, era)?;

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

fn map_multi_era_pparams(
    pparams: &Params,
    era: Era,
) -> Result<MultiEraProtocolParameters, anyhow::Error> {
    let multi_era_pparams = match era {
        Era::Conway => {
            let conway_pparams = map_conway_pparams(pparams);
            MultiEraProtocolParameters::Conway(conway_pparams)
        }
        _ => unimplemented!("Era not supported yet"),
    };

    Ok(multi_era_pparams)
}

fn map_conway_pparams(pparams: &Params) -> ConwayProtParams {
    match pparams.clone() {
        any_chain_params::Params::Cardano(params) => {
            let pool_influence = params.pool_influence.unwrap();
            let monetary_expansion = params.monetary_expansion.unwrap();
            let treasury_expansion = params.treasury_expansion.unwrap();
            let execution_costs = params.prices.unwrap();
            let cost_models = params.cost_models.unwrap();
            let max_execution_units_per_transaction =
                params.max_execution_units_per_transaction.unwrap();
            let max_execution_units_per_block = params.max_execution_units_per_block.unwrap();
            let pool_voting_thresholds = params.pool_voting_thresholds.unwrap();
            let drep_voting_thresholds = params.drep_voting_thresholds.unwrap();
            let min_fee_script_ref_cost_per_byte = params.min_fee_script_ref_cost_per_byte.unwrap();
            ConwayProtParams {
                system_start: chrono::DateTime::parse_from_rfc3339("2022-10-25T00:00:00Z").unwrap(),
                epoch_length: 86400,
                slot_length: 1,
                minfee_a: params.min_fee_coefficient as u32,
                minfee_b: params.min_fee_constant as u32,
                max_block_body_size: params.max_block_body_size as u32,
                max_transaction_size: params.max_tx_size as u32,
                max_block_header_size: params.max_block_header_size as u32,
                key_deposit: params.stake_key_deposit,
                pool_deposit: params.pool_deposit,
                desired_number_of_stake_pools: params.desired_number_of_pools as u32,
                protocol_version: {
                    let proto = params.protocol_version.unwrap();
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
                    unknown: KeyValuePairs::from(vec![]),
                },
                execution_costs: ExUnitPrices {
                    mem_price: primitives::RationalNumber {
                        numerator: execution_costs.memory.clone().unwrap().numerator as u64,
                        denominator: execution_costs.memory.clone().unwrap().denominator as u64,
                    },
                    step_price: primitives::RationalNumber {
                        numerator: execution_costs.steps.clone().unwrap().numerator as u64,
                        denominator: execution_costs.steps.clone().unwrap().denominator as u64,
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
                maximum_epoch: 18,
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
