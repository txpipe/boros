use std::{collections::VecDeque, sync::{Arc, Mutex}};

#[derive(Clone)]
pub struct CborTransactionsDb {
    pub cbor_txs_deque: Arc<Mutex<VecDeque<Vec<u8>>>>,
}

impl CborTransactionsDb {
    pub fn new() -> Self {
        Self { cbor_txs_deque: Arc::new(Mutex::new(VecDeque::new())) }
    }

    pub fn enqueue_tx(&self, tx: Vec<u8>) {
        let mut queue = self.cbor_txs_deque.lock().unwrap();
        queue.push_back(tx);
    }

    pub fn dequeue_tx(&self) -> Option<Vec<u8>> {
        let mut queue = self.cbor_txs_deque.lock().unwrap();
        queue.pop_front()
    }
}