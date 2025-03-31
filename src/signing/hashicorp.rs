use bip39::Mnemonic;
use vaultrs::{
    client::{VaultClient, VaultClientSettingsBuilder},
    kv2,
};

use super::{Config as VaultConfig, Secret, SecretAdapter, SigningError};

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
        let secret: Secret = kv2::read(&self.client, "secret", &self.config.path)
            .await
            .map_err(SigningError::Client)?;

        let value = secret.values.get(&key).ok_or_else(|| {
            SigningError::SecretNotFound(format!("Key {} not found in secret", self.config.key))
        })?;

        let mnemonic = serde_json::from_value(value.clone())?;

        Ok(mnemonic)
    }
}
