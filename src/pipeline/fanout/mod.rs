pub mod mempool;
pub mod stage;
pub mod tx_submit_peer;
pub mod tx_submit_peer_manager;
pub mod mock_ouroboros_tx_submit_server;

pub use stage::Stage;

// Test for Fanout Stage
#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use hex::decode;
    use mock_ouroboros_tx_submit_server::MockOuroborosTxSubmitPeerServer;
    use pallas::ledger::traverse::MultiEraTx;

    use crate::{Config, PeerManagerConfig};

    use super::*;

    #[tokio::test]
    async fn test_fanout_stage() {
        let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();

        let peer_server = Arc::new(MockOuroborosTxSubmitPeerServer::new("0.0.0.0:3001".to_string(), 2));
        peer_server.clone().init().await;

        tokio::time::sleep(Duration::from_millis(200)).await;
        let mut tx_submit_peer_client = tx_submit_peer::TxSubmitPeer::new("127.0.0.1:3001", 2);

        tx_submit_peer_client.init().await.unwrap();

        tokio::time::sleep(Duration::from_secs(1)).await;

        // add txs to peer client
        let cbor_data = "84a300d90102828258202000fbfaa10c6b316fe4b23c60b42313fc2ad89b51c7397f82a4a5ca97bd62a9008258202000fbfaa10c6b316fe4b23c60b42313fc2ad89b51c7397f82a4a5ca97bd62a901018182581d606e2e6d54e1a27ad640786852e20c22fb982ebcee4773a2926aae4b391b00000001a1166016021a0002aa3da100d9010281825820524e506f6a872c4ee3ee6d4b9913670c4b441860b3aa5438de92a676e20f527b5840233e9119fa6a3c58ab42bc384f506c2906104ebb059ab4ea6cc79305ff46c7e194d634d23ff775f92e51246e328711e6cbf38aeda01a1885f922047323c68c04f5f6";

        // Read the raw bytes from the request body.
        let raw_cbor = match decode(cbor_data) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::error!("Failed to decode hex string: {:?}", e);
                return;
            }
        };

        let config = Config {
            peer_manager: PeerManagerConfig {
                peers: vec!["".to_string()],
            },
            server: todo!(),
            storage: todo!(),
        };
        let tx = MultiEraTx::decode(&raw_cbor).unwrap();
        let tx_id = tx.hash();

        tracing::info!("Tx Hash: {:?}", tx_id);
        
        // There is a deadlock here, need to debug
        tx_submit_peer_client.add_tx(raw_cbor.clone()).await;
        
        // wait for server to stop
        loop {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let is_done = peer_server.is_done.read().unwrap();
            tracing::info!("Is Server done: {:?}", *is_done);
            if *is_done {
                break;
            }
            drop(is_done);
        }

        let server_acknowledge_txs = peer_server.acknowledge_txs.lock().unwrap();
        let mut found = false;

        for tx_from_server in server_acknowledge_txs.iter() {
            tracing::info!("Tx from server: {:?}, Tx from client: {:?}", tx_from_server, tx_id);
            if tx_from_server == &tx_id {
                found = true;
                break;
            }
        }
        assert!(found);
    }
}