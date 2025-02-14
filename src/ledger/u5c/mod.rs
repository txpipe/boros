use std::{collections::HashMap, pin::Pin, str::FromStr};

use anyhow::bail;
use async_stream::stream;
use chrono::{offset, DateTime, FixedOffset, NaiveDateTime};
use futures::{Stream, TryStreamExt};
use pallas::{
    applying::{utils::{ByronProtParams, ShelleyProtParams}, MultiEraProtocolParameters},
    interop::utxorpc::spec::{
        cardano::Tx,
        query::{any_chain_params, query_service_client::QueryServiceClient, ReadParamsRequest},
        sync::{
            any_chain_block, follow_tip_response, sync_service_client::SyncServiceClient, BlockRef,
            FollowTipRequest, ReadTipRequest,
        },
    },
    ledger::primitives::{Nonce, NonceVariant, RationalNumber},
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
    async fn fetch_utxos(&self, utxo_refs: &[String]) -> anyhow::Result<HashMap<String, Vec<u8>>>;
    async fn stream(&self) -> anyhow::Result<ChainSyncStream>;
    async fn get_pparams(&self) -> Result<MultiEraProtocolParameters, anyhow::Error>;
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
    async fn fetch_utxos(&self, _utxo_refs: &[String]) -> anyhow::Result<HashMap<String, Vec<u8>>> {
        todo!()
    }

    async fn get_pparams(&self) -> Result<MultiEraProtocolParameters, anyhow::Error> {
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
        let _byron_pparams = match pparams.clone() {
            any_chain_params::Params::Cardano(params) => ByronProtParams {
                block_version: (0, 0, 0),
                start_time: 0,
                script_version: 0,
                slot_duration: 0,
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
        };
        let _shelley_pparams = match pparams.clone() {
            any_chain_params::Params::Cardano(params) => {
                let monetary_expansion = params.monetary_expansion.unwrap();
                let treasury_expansion = params.treasury_expansion.unwrap();
                let pool_influence = params.pool_influence.unwrap();
                ShelleyProtParams {
                    system_start: DateTime::<FixedOffset>::from_naive_utc_and_offset(NaiveDateTime::default(), offset::FixedOffset::east_opt(0).unwrap()),
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
                    protocol_version: { let proto = params.protocol_version.as_ref().unwrap(); (proto.major as u64, proto.minor as u64) },
                    min_utxo_value: 0,
                    min_pool_cost: params.min_pool_cost,
                    expansion_rate: RationalNumber {
                        numerator: monetary_expansion.numerator as u64,
                        denominator: monetary_expansion.denominator as u64,
                    },
                    treasury_growth_rate: RationalNumber {
                        numerator: treasury_expansion.numerator as u64,
                        denominator: treasury_expansion.denominator as u64,
                    },
                    maximum_epoch: 0,
                    pool_pledge_influence: RationalNumber {
                        numerator: pool_influence.numerator as u64,
                        denominator: pool_influence.denominator as u64,
                    },
                    decentralization_constant: RationalNumber {
                        numerator: 0,
                        denominator: 0,
                    },
                    extra_entropy: Nonce { variant: NonceVariant::Nonce , hash: None },
                }
            },
        };
        // let _alonzo_pparams = todo!();
        // let _babbage_pparams = todo!();
        // let _conway_pparams = todo!();

        // need to understand this because it is not clear
        let multiera_pparams = MultiEraProtocolParameters::Byron(_byron_pparams);

        Ok(multiera_pparams)
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

#[cfg(test)]
mod u5c_tests {
    use super::*;
    use std::collections::HashMap;

    pub struct MockU5cDataAdapter {
        pub known_utxos: HashMap<String, Vec<u8>>,
    }

    #[async_trait::async_trait]
    impl U5cDataAdapter for MockU5cDataAdapter {
        async fn fetch_tip(&self) -> anyhow::Result<Point> {
            todo!()
        }

        async fn fetch_utxos(
            &self,
            utxo_refs: &[String],
        ) -> anyhow::Result<HashMap<String, Vec<u8>>> {
            let mut result = HashMap::new();

            // Check each requested reference; if known, clone the data into the result
            for reference in utxo_refs {
                if let Some(cbor_data) = self.known_utxos.get(reference) {
                    result.insert(reference.clone(), cbor_data.clone());
                }
            }

            Ok(result)
        }

        async fn get_pparams(&self) -> Result<MultiEraProtocolParameters, anyhow::Error> {
            todo!()
        }

        async fn stream(&self) -> anyhow::Result<ChainSyncStream> {
            todo!()
        }
    }

    #[tokio::test]
    async fn it_returns_found_utxos_only() {
        // Arrange: a mock with two known references
        let mut known = HashMap::new();
        known.insert("abc123#0".to_string(), vec![0x82, 0xa0]); // example CBOR
        known.insert("abc123#1".to_string(), vec![0x83, 0x04]);

        let provider = MockU5cDataAdapter { known_utxos: known };

        // We'll request three references, one of which doesn't exist
        let requested = vec![
            "abc123#0".to_string(),
            "abc123#1".to_string(),
            "missing#2".to_string(),
        ];

        // Act
        let results = provider.fetch_utxos(&requested).await.unwrap();

        // Assert
        assert_eq!(results.len(), 2, "Should only contain two known references");
        assert!(results.contains_key("abc123#0"));
        assert!(results.contains_key("abc123#1"));
        assert!(!results.contains_key("missing#2"));
    }
}
