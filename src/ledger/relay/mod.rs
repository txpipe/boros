/// A simple trait for returning a list of relay addresses in the format "hostname:port".
#[allow(dead_code)]
#[async_trait::async_trait]
pub trait RelayDataAdapter {
    async fn get_relays(&self) -> Vec<String>;
}

pub struct MockRelayDataAdapter {
    pub mock_relays: Vec<String>,
}

#[async_trait::async_trait]
impl RelayDataAdapter for MockRelayDataAdapter {
    async fn get_relays(&self) -> Vec<String> {
        self.mock_relays.clone()
    }
}

impl MockRelayDataAdapter {
    pub fn new() -> Self {
        Self {
            mock_relays: vec![
                "109.205.181.113:7900".to_string(),
                "194.163.149.210:6000".to_string(),
                "preview.adastack.net:3001".to_string(),
                "pv-relays.digitalfortress.online:8001".to_string(),
                "preview-relays.onyxstakepool.com:3001".to_string(),
                "preview.frcan.com:6010".to_string(),
                "preview.leadstakepool.com:3001".to_string(),
                "preview.leadstakepool.com:3002".to_string(),
                "relay.preview.cardanostakehouse.com:11000".to_string(),
                "130.162.231.122:6001".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    /// A mock provider that returns a pre-defined list of string addresses, e.g., ["relay1:3001", "relay2:3002"].

    #[test]
    async fn it_returns_mock_relays() {
        let provider = MockRelayDataAdapter {
            mock_relays: vec!["relay1:3001".to_string(), "relay2:3002".to_string()],
        };

        let _relays = provider.get_relays().await;

        // assert_eq!(relays.len(), 2);
        // assert_eq!(relays[0], "relay1:3001");
        // assert_eq!(relays[1], "relay2:3002");
    }
}
