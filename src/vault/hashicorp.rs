use bip39::Mnemonic;
use serde::{Deserialize, Serialize};
use vaultrs::{
    client::{VaultClient, VaultClientSettingsBuilder},
    kv2,
};

use super::{key, Config as VaultConfig, VaultAdapter, VaultError};

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
impl VaultAdapter for HashicorpVaultClient {
    async fn store_key(&self) -> Result<(), VaultError> {
        let mnemonic = key::generate_mnemonic();
        let secret = Secret { mnemonic };

        kv2::set(&self.client, &self.config.mount, &self.config.path, &secret).await?;

        Ok(())
    }

    async fn retrieve_key(&self) -> Result<Mnemonic, VaultError> {
        let secret: Secret = kv2::read(&self.client, &self.config.mount, &self.config.path).await?;

        Ok(secret.mnemonic)
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Secret {
    mnemonic: Mnemonic,
}
