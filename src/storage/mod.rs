use std::{fmt::Display, str::FromStr};

use chrono::{DateTime, Utc};
use serde::Deserialize;

pub mod sqlite;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub db_path: String,
}

#[derive(Clone)]
pub struct Transaction {
    pub id: String,
    pub raw: Vec<u8>,
    pub status: TransactionStatus,
    pub priority: TransactionPriority,
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
            status: TransactionStatus::Pending,
            priority: TransactionPriority::Low,
            slot: None,
            dependencies: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

#[derive(Clone)]
pub enum TransactionPriority {
    Low,
    Medium,
    High,
}
impl TryFrom<u32> for TransactionPriority {
    type Error = anyhow::Error;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::High),
            2 => Ok(Self::Medium),
            3 => Ok(Self::Low),
            _ => Err(anyhow::Error::msg("transaction priority not supported")),
        }
    }
}
impl TryFrom<TransactionPriority> for u32 {
    type Error = anyhow::Error;

    fn try_from(value: TransactionPriority) -> Result<Self, Self::Error> {
        match value {
            TransactionPriority::High => Ok(1),
            TransactionPriority::Medium => Ok(2),
            TransactionPriority::Low => Ok(3),
        }
    }
}

#[derive(Clone)]
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
                priority: TransactionPriority::Low,
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
