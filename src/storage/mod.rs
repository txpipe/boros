use chrono::{DateTime, Utc};

pub mod sqlite;
pub mod in_memory_db;

pub enum TransactionPriority {
    LOW,
    MEDIUM,
    HIGH,
}

pub struct TransactionStorage {
    pub id: String,
    pub raw: Vec<u8>,
    pub status: String,
    pub priority: u32,
    pub dependences: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Default for TransactionStorage {
        fn default() -> Self {
            Self {
                id: "hex".into(),
                raw: "hex".into(),
                status: "pending".into(),
                priority: 1,
                dependences: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        }
    }
}
