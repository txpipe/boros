use std::{fmt::Display, str::FromStr};

use chrono::{DateTime, Utc};
use serde::Deserialize;

pub mod sqlite;

#[derive(Deserialize)]
pub struct Config {
    pub db_path: String,
}

pub struct Transaction {
    pub id: String,
    pub raw: Vec<u8>,
    pub status: TransactionStatus,
    pub priority: TransactionPriority,
    pub dependences: Option<Vec<String>>,
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
            dependences: None,
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
            1 => Ok(Self::Low),
            2 => Ok(Self::Medium),
            3 => Ok(Self::High),
            _ => Err(anyhow::Error::msg("transaction priority not supported")),
        }
    }
}
impl TryFrom<TransactionPriority> for u32 {
    type Error = anyhow::Error;

    fn try_from(value: TransactionPriority) -> Result<Self, Self::Error> {
        match value {
            TransactionPriority::Low => Ok(1),
            TransactionPriority::Medium => Ok(2),
            TransactionPriority::High => Ok(3),
        }
    }
}

#[derive(Clone)]
pub enum TransactionStatus {
    Pending,
}
impl FromStr for TransactionStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            _ => Err(anyhow::Error::msg("transaction status not supported")),
        }
    }
}
impl Display for TransactionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
        }
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
                dependences: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        }
    }
}
