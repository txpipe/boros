use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub mod hashicorp;
pub mod key;

#[derive(Error, Debug)]
pub enum SigningError {
    #[error("Vault client error: {0}")]
    Client(#[from] vaultrs::error::ClientError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Secret not found error: {0}")]
    SecretNotFound(String),

    #[error("Deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub api_addr: String,
    pub token: String,
    pub path: String,
    pub key: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Secret {
    #[serde(flatten)]
    values: HashMap<String, Value>,
}

#[async_trait::async_trait]
pub trait SecretAdapter<T: Send + Sync + 'static>: Send + Sync {
    async fn retrieve_secret(&self) -> Result<T, SigningError>;
}
