use chrono::{DateTime, Utc};

pub mod sqlite;

pub struct TransactionStorage {
    pub id: Option<u64>,
    pub tx_cbor: Vec<u8>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
