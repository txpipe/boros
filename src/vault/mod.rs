use serde::Deserialize;
use thiserror::Error;

pub mod hashicorp;
pub mod key;

#[derive(Error, Debug)]
pub enum VaultError {
    #[error("Vault client error: {0}")]
    ClientError(#[from] vaultrs::error::ClientError),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub api_addr: String,
    pub token: String,
    pub mount: String,
    pub path: String,
}

#[async_trait::async_trait]
pub trait VaultAdapter<T: Send + Sync + 'static>: Send + Sync {
    async fn store_key(&self) -> Result<(), VaultError>;
    async fn retrieve_key(&self) -> Result<T, VaultError>;
}
