use std::{collections::HashMap, str::FromStr};

use async_stream::stream;
use futures::TryStreamExt;
use pallas::interop::utxorpc::spec::sync::{
    any_chain_block, follow_tip_response, sync_service_client::SyncServiceClient, FollowTipRequest,
};
use serde::Deserialize;
use tonic::{
    metadata::{MetadataKey, MetadataValue},
    transport::{Channel, ClientTlsConfig, Uri},
    Request,
};
use tracing::info;

use super::{ChainSyncAdapter, ChainSyncStream, Event};

pub struct UtxoChainSyncAdapter {
    url: String,
    metadata: HashMap<String, String>,
}
impl UtxoChainSyncAdapter {
    pub fn new(config: UtxorpcConfig) -> Self {
        Self {
            url: config.uri,
            metadata: config.metadata,
        }
    }
}

#[async_trait::async_trait]
impl ChainSyncAdapter for UtxoChainSyncAdapter {
    async fn stream(&mut self) -> anyhow::Result<ChainSyncStream> {
        let uri: Uri = self.url.parse()?;

        let channel = Channel::builder(uri)
            .tls_config(ClientTlsConfig::new().with_webpki_roots())?
            .connect()
            .await?;

        info!("utxorpc connected");

        let metadata = self.metadata.clone();
        let mut client =
            SyncServiceClient::with_interceptor(channel, move |mut req: Request<()>| {
                metadata.iter().for_each(|(key, value)| {
                    req.metadata_mut().insert(
                        MetadataKey::from_str(key).unwrap(),
                        MetadataValue::from_str(value).unwrap(),
                    );
                });

                Ok(req)
            });
        let request = tonic::Request::new(FollowTipRequest {
            ..Default::default()
        });

        let response = client.follow_tip(request).await?;
        let mut tip_stream = response.into_inner();

        let stream = stream! {
            while let Some(follow_tip) = tip_stream.try_next().await? {
                if let Some(action) = follow_tip.action {
                    match action {
                        follow_tip_response::Action::Apply(any) => {
                            yield Ok(Event::RollForward(any.native_bytes.to_vec()));
                        },
                        follow_tip_response::Action::Undo(any) => {
                            match any.chain.unwrap() {
                                any_chain_block::Chain::Cardano(block) => {
                                    let header = block.header.unwrap();
                                    yield Ok(Event::Rollback(header.slot, header.hash.to_vec()));
                                },
                            }
                        },
                        follow_tip_response::Action::Reset(block_ref) => {
                            dbg!("reset not implemented yet", block_ref);
                        },
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}

#[derive(Deserialize, Clone)]
pub struct UtxorpcConfig {
    uri: String,
    metadata: HashMap<String, String>,
}
