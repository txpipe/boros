use bip39::Mnemonic;
use serde::{Deserialize, Serialize};
use vaultrs::{
    client::{VaultClient, VaultClientSettingsBuilder},
    kv2,
};

use super::{key, Config as VaultConfig, VaultAdapter, VaultError};

#[derive(Serialize, Deserialize, Debug)]
struct Secret<T> {
    secret_key: T,
}

pub struct HashicorpVaultClient {
    client: VaultClient,
    config: VaultConfig,
}

impl HashicorpVaultClient {
    pub fn new(config: VaultConfig) -> Result<Self, VaultError> {
        let client_settings = VaultClientSettingsBuilder::default()
            .address(&config.api_addr)
            .token(&config.token)
            .build()
            .map_err(|e| VaultError::ConfigError(e.to_string()))?;

        let client = VaultClient::new(client_settings).map_err(VaultError::ClientError)?;

        Ok(Self { client, config })
    }
}

#[async_trait::async_trait]
impl VaultAdapter<Mnemonic> for HashicorpVaultClient {
    async fn store_key(&self) -> Result<(), VaultError> {
        let mnemonic = key::generate_mnemonic(&self.config.passphrase);
        let secret = Secret {
            secret_key: mnemonic,
        };

        kv2::set(&self.client, &self.config.mount, &self.config.path, &secret).await?;

        Ok(())
    }

    async fn retrieve_key(&self) -> Result<Mnemonic, VaultError> {
        let secret: Secret<Mnemonic> =
            kv2::read(&self.client, &self.config.mount, &self.config.path).await?;

        Ok(secret.secret_key)
    }
}
