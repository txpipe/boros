use async_trait::async_trait;
use std::collections::HashMap;
use thiserror::Error;

/// Errors related to querying UTxOs.
#[derive(Debug, Error)]
pub enum UtxoQueryError {
    #[error("network error")]
    Network(#[from] std::io::Error),
    #[error("unknown error: {0}")]
    Unknown(String),
}

#[async_trait]
pub trait UtxoDataAdapter {
    async fn fetch_utxos(
        &self,
        utxo_refs: &[String],
    ) -> Result<HashMap<String, Vec<u8>>, UtxoQueryError>;
}

pub struct MockUtxoDataAdapter {
    pub known_utxos: HashMap<String, Vec<u8>>,
}

#[async_trait]
impl UtxoDataAdapter for MockUtxoDataAdapter {
    async fn fetch_utxos(
        &self,
        utxo_refs: &[String],
    ) -> Result<HashMap<String, Vec<u8>>, UtxoQueryError> {
        let mut result = HashMap::new();

        // Check each requested reference; if known, clone the data into the result
        for reference in utxo_refs {
            if let Some(cbor_data) = self.known_utxos.get(reference) {
                result.insert(reference.clone(), cbor_data.clone());
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn it_returns_found_utxos_only() {
        // Arrange: a mock with two known references
        let mut known = HashMap::new();
        known.insert("abc123#0".to_string(), vec![0x82, 0xa0]); // example CBOR
        known.insert("abc123#1".to_string(), vec![0x83, 0x04]);

        let provider = MockUtxoDataAdapter { known_utxos: known };

        // We'll request three references, one of which doesn't exist
        let requested = vec![
            "abc123#0".to_string(),
            "abc123#1".to_string(),
            "missing#2".to_string(),
        ];

        // Act
        let results = provider.fetch_utxos(&requested).await.unwrap();

        // Assert
        assert_eq!(results.len(), 2, "Should only contain two known references");
        assert!(results.contains_key("abc123#0"));
        assert!(results.contains_key("abc123#1"));
        assert!(!results.contains_key("missing#2"));
    }
}
