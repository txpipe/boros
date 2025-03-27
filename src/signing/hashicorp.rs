use bip39::Mnemonic;
use vaultrs::{
    client::{VaultClient, VaultClientSettingsBuilder},
    kv2,
};

use super::{Config as VaultConfig, Secret, SecretAdapter, SigningError};

pub enum SecretEngine {
    KV2,
}

impl SecretEngine {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::KV2 => "secret",
        }
    }
}

pub struct HashicorpVaultClient {
    client: VaultClient,
    config: VaultConfig,
}

impl HashicorpVaultClient {
    pub fn new(config: VaultConfig) -> Result<Self, SigningError> {
        let client_settings = VaultClientSettingsBuilder::default()
            .address(&config.api_addr)
            .token(&config.token)
            .build()
            .map_err(|e| SigningError::Config(e.to_string()))?;

        let client = VaultClient::new(client_settings).map_err(SigningError::Client)?;

        Ok(Self { client, config })
    }
}

#[async_trait::async_trait]
impl SecretAdapter<Mnemonic> for HashicorpVaultClient {
    async fn retrieve_secret(&self, key: String) -> Result<Mnemonic, SigningError> {
        let secret: Secret =
            kv2::read(&self.client, SecretEngine::KV2.as_str(), &self.config.path).await?;

        let value = secret
            .value(key)
            .ok_or_else(|| SigningError::SecretNotFound("Mnemonic not found in secret".into()))?;

        let mnemonic = serde_json::from_value(value.clone())
            .map_err(|e| SigningError::Config(e.to_string()))?;

        Ok(mnemonic)
    }
}
