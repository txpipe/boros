use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct CborTransactionsDb {
    pub cbor_txs: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl CborTransactionsDb {
    pub fn new() -> Self {
        Self {
            cbor_txs: Arc::new(Mutex::new(vec![])),
        }
    }

    pub fn push_tx(&self, tx: Vec<u8>) {
        let mut txs = self.cbor_txs.lock().unwrap();
        txs.push(tx);
    }

    pub fn pop_tx(&self) -> Option<Vec<u8>> {
        let mut txs = self.cbor_txs.lock().unwrap();

        if txs.len() == 0 {
            None
        } else {
            Some(txs.remove(0))
        }
    }
}
