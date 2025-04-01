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

    #[error("Decoding error: {0}")]
    Decoding(String),
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
pub trait SigningAdapter: Send + Sync {
    async fn sign(&self, data: Vec<u8>) -> Result<Vec<u8>, SigningError>;
    #[allow(unused)] // to make clippy happy only
    async fn verify(&self, data: Vec<u8>, signature: Vec<u8>) -> Result<bool, SigningError>;
}
