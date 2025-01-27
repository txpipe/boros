use itertools::Itertools;
use pallas::{crypto::hash::Hash, ledger::traverse::MultiEraTx};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::debug;

type TxHash = Hash<32>;

#[derive(Debug, Error)]
pub enum MempoolError {
    #[error("traverse error: {0}")]
    TraverseError(#[from] pallas::ledger::traverse::Error),

    #[error("decode error: {0}")]
    DecodeError(#[from] pallas::codec::minicbor::decode::Error),
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
    Pending,
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
    pending: Vec<Tx>,
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

    fn receive(&self, tx: Tx) {
        let mut state = self.mempool.write().unwrap();

        state.pending.push(tx.clone());
        self.notify(TxStage::Pending, tx);

        debug!(
            pending = state.pending.len(),
            inflight = state.inflight.len(),
            acknowledged = state.acknowledged.len(),
            "mempool state changed"
        );
    }

    pub fn receive_raw(&self, cbor: &[u8]) -> Result<TxHash, MempoolError> {
        let tx = MultiEraTx::decode(cbor)?;

        let hash = tx.hash();

        let tx = Tx {
            hash,
            // TODO: this is a hack to make the era compatible with the ledger
            era: u16::from(tx.era()) - 1,
            bytes: cbor.into(),
            confirmed: false,
        };

        self.receive(tx);

        Ok(hash)
    }

    pub fn request(&self, desired: usize) -> Vec<Tx> {
        let available = self.pending_total();
        self.request_exact(std::cmp::min(desired, available))
    }

    pub fn request_exact(&self, count: usize) -> Vec<Tx> {
        let mut state = self.mempool.write().unwrap();

        let selected = state.pending.drain(..count).collect_vec();

        for tx in selected.iter() {
            state.inflight.push(tx.clone());
            self.notify(TxStage::Inflight, tx.clone());
        }

        debug!(
            pending = state.pending.len(),
            inflight = state.inflight.len(),
            acknowledged = state.acknowledged.len(),
            "mempool state changed"
        );

        selected
    }

    pub fn acknowledge(&self, count: usize) {
        debug!(n = count, "acknowledging txs");

        let mut state = self.mempool.write().unwrap();

        let selected = state.inflight.drain(..count).collect_vec();

        for tx in selected {
            state.acknowledged.insert(tx.hash, tx.clone());
            self.notify(TxStage::Acknowledged, tx.clone());
        }

        debug!(
            pending = state.pending.len(),
            inflight = state.inflight.len(),
            acknowledged = state.acknowledged.len(),
            "mempool state changed"
        );
    }

    pub fn find_inflight(&self, tx_hash: &TxHash) -> Option<Tx> {
        let state = self.mempool.read().unwrap();
        state.inflight.iter().find(|x| x.hash.eq(tx_hash)).cloned()
    }

    pub fn pending_total(&self) -> usize {
        let state = self.mempool.read().unwrap();
        state.pending.len()
    }
}
