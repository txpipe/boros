use itertools::Itertools;
use pallas::{crypto::hash::Hash, ledger::traverse::MultiEraTx};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{debug, error};

type TxHash = Hash<32>;

#[derive(Debug, Error)]
pub enum MempoolError {
    #[error("traverse error: {0}")]
    Traverse(#[from] pallas::ledger::traverse::Error),

    #[error("decode error: {0}")]
    Decode(#[from] pallas::codec::minicbor::decode::Error),
    
    #[error("lock error: {0}")]
    Lock(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Tx {
    pub hash: TxHash,
    pub era: u16,
    pub bytes: Vec<u8>,
    // TODO: we'll improve this to track number of confirmations in further iterations.
    pub confirmed: bool,
}

#[derive(Clone)]
pub enum TxStage {
    Inflight,
    Acknowledged,
}

// TODO: validate clippy unused fields
#[allow(dead_code)]
#[derive(Clone)]
pub struct Event {
    new_stage: TxStage,
    tx: Tx,
}

#[derive(Default)]
struct MempoolState {
    inflight: Vec<Tx>,
    acknowledged: HashMap<TxHash, Tx>,
}

/// A very basic, FIFO, single consumer mempool
#[derive(Clone)]
pub struct Mempool {
    mempool: Arc<RwLock<MempoolState>>,
    updates: broadcast::Sender<Event>,
}

impl Mempool {
    pub fn new() -> Self {
        let mempool = Arc::new(RwLock::new(MempoolState::default()));
        let (updates, _) = broadcast::channel(16);

        Self { mempool, updates }
    }

    pub fn notify(&self, new_stage: TxStage, tx: Tx) {
        if self.updates.send(Event { new_stage, tx }).is_err() {
            debug!("no mempool update receivers");
        }
    }

    fn receive(&self, tx: Tx) -> Result<(), MempoolError> {
        let mut state = self.mempool.write()
            .map_err(|e| MempoolError::Lock(format!("Failed to acquire write lock: {}", e)))?;

        state.inflight.push(tx.clone());
        self.notify(TxStage::Inflight, tx);

        debug!(
            inflight = state.inflight.len(),
            acknowledged = state.acknowledged.len(),
            "mempool state changed"
        );
        
        Ok(())
    }

    pub fn receive_raw(&self, cbor: &[u8]) -> Result<Tx, MempoolError> {
        let tx = MultiEraTx::decode(cbor)?;

        let hash = tx.hash();

        let tx = Tx {
            hash,
            // TODO: this is a hack to make the era compatible with the ledger
            era: u16::from(tx.era()) - 1,
            bytes: cbor.into(),
            confirmed: false,
        };

        self.receive(tx.clone())?;

        Ok(tx)
    }

    pub fn acknowledge(&self, count: usize) -> Result<(), MempoolError> {
        debug!(n = count, "acknowledging txs");

        let mut state = self.mempool.write()
            .map_err(|e| MempoolError::Lock(format!("Failed to acquire write lock: {}", e)))?;

        let selected = state.inflight.drain(..count).collect_vec();

        for tx in selected {
            state.acknowledged.insert(tx.hash, tx.clone());
            self.notify(TxStage::Acknowledged, tx.clone());
        }

        debug!(
            inflight = state.inflight.len(),
            acknowledged = state.acknowledged.len(),
            "mempool state changed"
        );
        
        Ok(())
    }

    pub fn find_inflight(&self, tx_hash: &TxHash) -> Result<Option<Tx>, MempoolError> {
        let state = self.mempool.read()
            .map_err(|e| MempoolError::Lock(format!("Failed to acquire read lock: {}", e)))?;
            
        Ok(state.inflight.iter().find(|x| x.hash.eq(tx_hash)).cloned())
    }
}
