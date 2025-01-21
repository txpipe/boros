use chrono::{DateTime, Utc};

pub mod sqlite;

pub enum TransactionPriority {
    LOW,
    MEDIUM,
    HIGH,
}

pub struct TransactionStorage {
    pub id: Option<u64>,
    pub tx_cbor: Vec<u8>,
    pub status: String,
    pub priority: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
