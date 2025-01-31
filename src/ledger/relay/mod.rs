use async_trait::async_trait;

/// A simple trait for returning a list of relay addresses in the format "hostname:port".
#[async_trait]
pub trait RelayDataProvider {
    async fn get_relays(&self) -> Vec<String>;
}

/// A mock provider that returns a pre-defined list of string addresses, e.g., ["relay1:3001", "relay2:3002"].
pub struct MockRelayDataProvider {
    pub mock_relays: Vec<String>,
}

#[async_trait]
impl RelayDataProvider for MockRelayDataProvider {
    async fn get_relays(&self) -> Vec<String> {
        self.mock_relays.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[test]
    async fn it_returns_mock_relays() {
        let provider = MockRelayDataProvider {
            mock_relays: vec![
                "relay1:3001".to_string(),
                "relay2:3002".to_string(),
            ],
        };

        let relays = provider.get_relays().await;

        assert_eq!(relays.len(), 2);
        assert_eq!(relays[0], "relay1:3001");
        assert_eq!(relays[1], "relay2:3002");
    }
}
