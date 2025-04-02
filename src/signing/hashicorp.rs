use bip39::Mnemonic;
use pallas::{crypto::key::ed25519::Signature, ledger::traverse::MultiEraTx};
use vaultrs::{
    client::{VaultClient, VaultClientSettingsBuilder},
    kv2,
};

use super::{
    key::{
        derive::get_ed25519_keypair,
        sign::{sign_transaction, to_built_transaction},
    },
    Config as VaultConfig, Secret, SigningAdapter, SigningError,
};

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

    async fn get_mnemonic(&self, key: &str) -> Result<Mnemonic, SigningError> {
        let secret: Secret = kv2::read(&self.client, "secret", &self.config.path)
            .await
            .map_err(SigningError::Client)?;

        let value = secret.values.get(key).ok_or_else(|| {
            SigningError::SecretNotFound(format!("Key {} not found in secret", key))
        })?;

        let mnemonic = serde_json::from_value(value.clone())?;
        Ok(mnemonic)
    }
}

#[async_trait::async_trait]
impl SigningAdapter for HashicorpVaultClient {
    async fn sign(&self, data: Vec<u8>) -> Result<Vec<u8>, SigningError> {
        let mnemonic = self.get_mnemonic(&self.config.key).await?;

        let metx = MultiEraTx::decode(&data)
            .map_err(|e| SigningError::Decoding(format!("Failed to decode transaction: {}", e)))?;

        let built_tx = to_built_transaction(&metx);
        let signed_tx = sign_transaction(built_tx, &mnemonic);

        Ok(signed_tx.tx_bytes.0)
    }

    async fn verify(&self, data: Vec<u8>, signature: Vec<u8>) -> Result<bool, SigningError> {
        let mnemonic = self.get_mnemonic(&self.config.key).await?;
        let (_, public_key) = get_ed25519_keypair(&mnemonic);

        let signature = Signature::try_from(signature.as_slice())
            .map_err(|e| SigningError::Decoding(format!("Failed to decode signature: {}", e)))?;

        Ok(public_key.verify(data, &signature))
    }
}
