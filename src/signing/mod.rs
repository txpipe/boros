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
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub api_addr: String,
    pub token: String,
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Secret {
    #[serde(flatten)]
    secret: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Hash)]
pub enum SecretDataKey {
    Mnemonic,
}

impl From<SecretDataKey> for String {
    fn from(key: SecretDataKey) -> Self {
        match key {
            SecretDataKey::Mnemonic => "mnemonic".to_string(),
        }
    }
}

impl Secret {
    fn value(&self, key: String) -> Option<&Value> {
        self.secret.get(&key)
    }
}

#[async_trait::async_trait]
pub trait SecretAdapter<T: Send + Sync + 'static>: Send + Sync {
    async fn retrieve_secret(&self, key: String) -> Result<T, SigningError>;
}
