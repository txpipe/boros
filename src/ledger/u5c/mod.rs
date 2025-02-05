use std::{collections::HashMap, pin::Pin, str::FromStr};

use anyhow::bail;
use async_stream::stream;
use futures::{Stream, TryStreamExt};
use pallas::interop::utxorpc::spec::{
    cardano::Tx,
    sync::{
        any_chain_block, follow_tip_response, sync_service_client::SyncServiceClient, BlockRef,
        FollowTipRequest, ReadTipRequest,
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
    async fn fetch_utxos(&self, utxo_refs: &[String]) -> anyhow::Result<HashMap<String, Vec<u8>>>;
    async fn stream(&self) -> anyhow::Result<ChainSyncStream>;
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
