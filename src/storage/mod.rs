use std::{fmt::Display, str::FromStr};

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::priority::DEFAULT_QUEUE;

pub mod sqlite;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub db_path: String,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: String,
    pub raw: Vec<u8>,
    pub status: TransactionStatus,
    pub queue: String,
    pub slot: Option<u64>,
    pub dependencies: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl Transaction {
    pub fn new(id: String, raw: Vec<u8>) -> Self {
        Self {
            id,
            raw,
            //status: TransactionStatus::Pending,
            status: TransactionStatus::Validated,
            queue: DEFAULT_QUEUE.into(),
            slot: None,
            dependencies: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum TransactionStatus {
    Pending,
    Validated,
    InFlight,
    Confirmed,
}
impl FromStr for TransactionStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "validated" => Ok(Self::Validated),
            "inflight" => Ok(Self::InFlight),
            "confirmed" => Ok(Self::Confirmed),
            _ => Err(anyhow::Error::msg("transaction status not supported")),
        }
    }
}
impl Display for TransactionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Validated => write!(f, "validated"),
            Self::InFlight => write!(f, "inflight"),
            Self::Confirmed => write!(f, "confirmed"),
        }
    }
}

#[derive(Clone)]
pub struct TransactionState {
    pub queue: String,
    pub count: usize,
}

#[derive(Clone)]
pub struct Cursor {
    pub slot: u64,
    pub hash: Vec<u8>,
}
impl Cursor {
    pub fn new(slot: u64, hash: Vec<u8>) -> Self {
        Self { slot, hash }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Default for Transaction {
        fn default() -> Self {
            Self {
                id: "hex".into(),
                raw: "hex".into(),
                status: TransactionStatus::Pending,
                queue: DEFAULT_QUEUE.into(),
                slot: None,
                dependencies: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        }
    }

    impl Default for Cursor {
        fn default() -> Self {
            Self {
                slot: 1,
                hash: "hex".into(),
            }
        }
    }
}
