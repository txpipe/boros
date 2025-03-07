use std::{sync::Arc, time::Duration};

use gasket::framework::*;
use rand::{seq::IndexedRandom, Rng};
use thiserror::Error;
use tokio::time::sleep;
use tracing::info;

use crate::{
    ledger::relay::RelayDataAdapter,
    peer::{
        peer::PeerError,
        peer_manager::{PeerManager, PeerManagerConfig, PeerManagerError},
    },
};

#[derive(Error, Debug)]
pub enum FanoutError {
    #[error("peer manager error: {0}")]
    PeerManager(#[from] PeerManagerError),

    #[error("peer error: {0}")]
    Peer(#[from] PeerError),

    #[error("worker error: {0}")]
    Worker(#[from] gasket::framework::WorkerError),
}

#[derive(Stage)]
#[stage(name = "peer_discovery", unit = "String", worker = "Worker")]
pub struct Stage {
    config: PeerManagerConfig,
    peer_manager: Arc<PeerManager>,
    relay_adapter: Arc<dyn RelayDataAdapter + Send + Sync>,
}

impl Stage {
    pub fn new(
        config: PeerManagerConfig,
        peer_manager: Arc<PeerManager>,
        relay_adapter: Arc<dyn RelayDataAdapter + Send + Sync>,
    ) -> Self {
        Self {
            config,
            peer_manager,
            relay_adapter,
        }
    }
}

pub struct Worker {
    peer_discovery_queue: u8,
    relay_adapter: Arc<dyn RelayDataAdapter + Send + Sync>,
}

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(stage: &Stage) -> Result<Self, WorkerError> {
        let relay_adapter = stage.relay_adapter.clone();

        Ok(Self {
            peer_discovery_queue: 0,
            relay_adapter,
        })
    }

    async fn schedule(&mut self, stage: &mut Stage) -> Result<WorkSchedule<String>, WorkerError> {
        let desired_count = stage.config.desired_peer_count;
        let peer_per_request = stage.config.peers_per_request;
        let additional_peers_required =
            desired_count as usize - stage.peer_manager.connected_peers_count().await;

        if self.peer_discovery_queue < additional_peers_required as u8 {
            info!("Additional Peers Required: {}", additional_peers_required);
            let from_pool_relay = self.relay_adapter.get_relays().await;
            let from_pool_relay = from_pool_relay.choose(&mut rand::rng()).cloned();
            let from_peer_discovery = stage
                .peer_manager
                .pick_peer_rand(peer_per_request)
                .await
                .or_retry()?;

            let mut rng = rand::rng();
            let chosen_peer = if rng.random_bool(0.5) {
                info!("Onchain Relay chosen: {:?}", from_pool_relay);
                from_pool_relay
            } else {
                info!("P2P Relay chosen: {:?}", from_peer_discovery);
                from_peer_discovery
            };

            if let Some(peer_addr) = chosen_peer {
                self.peer_discovery_queue += 1;
                return Ok(WorkSchedule::Unit(peer_addr));
            }
        }

        sleep(Duration::from_secs(1)).await;
        Ok(WorkSchedule::Idle)
    }

    async fn execute(&mut self, unit: &String, stage: &mut Stage) -> Result<(), WorkerError> {
        stage.peer_manager.add_peer(&unit).await;
        self.peer_discovery_queue -= 1;

        Ok(())
    }
}
