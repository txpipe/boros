use std::sync::{Arc, Mutex, RwLock};
use tokio::{net::TcpListener, task};
use tracing::{error, info};

use pallas::network::{
    facades::PeerServer,
    miniprotocols::txsubmission::{EraTxId, Reply, State},
};

pub struct MockOuroborosTxSubmitPeerServer {
    pub socket_addr: String,
    pub network_magic: u64,
    pub acknowledge_txs: Arc<Mutex<Vec<pallas::crypto::hash::Hash<32>>>>,
    pub is_done: Arc<RwLock<bool>>,
}

impl MockOuroborosTxSubmitPeerServer {
    pub fn new(socket_addr: String, network_magic: u64) -> Self {
        Self {
            socket_addr,
            network_magic,
            acknowledge_txs: Arc::new(Mutex::new(vec![])),
            is_done: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn init(self: Arc<Self>) {
        task::spawn(async move {
            let tcp_listener = TcpListener::bind(&self.socket_addr)
                .await
                .expect("Failed to bind");

            info!(
                "SERVER: MockOuroborosTxSubmitPeerServer listening on: {}",
                &self.socket_addr
            );

            loop {
                match PeerServer::accept(&tcp_listener, self.network_magic).await {
                    Ok(peer_server) => {
                        let this = Arc::clone(&self);
                        task::spawn(async move {
                            this.start_background_task(peer_server).await;
                        });
                    }
                    Err(err) => {
                        info!("SERVER: Accept error: {:?}", err);
                    }
                }
            }
        });
    }

    async fn start_background_task(&self, mut peer_server: PeerServer) {
        let mut collected_tx_ids: Vec<EraTxId> = vec![];
        let addr = peer_server.accepted_address().unwrap();

        info!("SERVER: New connection from {addr}");
        let tx_server = peer_server.txsubmission();

        info!("SERVER: waiting for init (agency is theirs)");
        if let Err(err) = tx_server.wait_for_init().await {
            error!("SERVER: error waiting for init: {:?}", err);
            return;
        }

        info!("SERVER: init received, now we have agency");

        let mut acknowledge = 0u16;
        let count = 3u16;

        info!("SERVER: Current State: {:?}", tx_server.state());
        info!("SERVER: requesting TxIds (blocking=true)");

        // Request TxIds Blocking
        if matches!(tx_server.state(), State::Idle) {
            if let Err(err) = tx_server
                .acknowledge_and_request_tx_ids(true, acknowledge, count)
                .await
            {
                panic!("SERVER: error requesting TxIds => {err:?}");
            }
        }

        // Receive TxIds for the blocking request
        let txids_reply = match tx_server.receive_next_reply().await {
            Ok(reply) => reply,
            Err(err) => {
                panic!("SERVER: error receiving next reply => {err:?}");
            }
        };

        // Process the TxIds
        if let Reply::TxIds(ids_and_sizes) = txids_reply {
            let num_ids = ids_and_sizes.len() as u32;
            info!(
                "SERVER: got TxIds => {} total, ack so far => {}",
                num_ids, acknowledge
            );

            let new_tx_ids = ids_and_sizes
                .into_iter()
                .map(|tx_id_and_size| tx_id_and_size.0)
                .collect::<Vec<_>>();

            collected_tx_ids.extend(new_tx_ids);

            info!("SERVER: appended {} new TxIds to collected list", num_ids);
            info!("SERVER: next request will be non-blocking");
        }

        // Request TxIds Non-Blocking
        if matches!(tx_server.state(), State::Idle) {
            if let Err(err) = tx_server
                .acknowledge_and_request_tx_ids(false, 0, count)
                .await
            {
                panic!("SERVER: error requesting TxIds => {err:?}");
            }
        }

        // Receive TxIds for the non-blocking request
        let txids_reply = match tx_server.receive_next_reply().await {
            Ok(reply) => reply,
            Err(err) => {
                panic!("SERVER: error receiving next reply => {err:?}");
            }
        };

        // Process the TxIds
        if let Reply::TxIds(ids_and_sizes) = txids_reply {
            let num_ids = ids_and_sizes.len() as u32;
            info!(
                "SERVER: got TxIds => {} total, ack so far => {}",
                num_ids, acknowledge
            );

            let new_tx_ids = ids_and_sizes
                .into_iter()
                .map(|tx_id_and_size| tx_id_and_size.0)
                .collect::<Vec<_>>();

            collected_tx_ids.extend(new_tx_ids);

            info!("SERVER: appended {} new TxIds to collected list", num_ids);
            info!("SERVER: next request will be non-blocking");
        }

        // Request the Tx bodies
        if let Err(err) = tx_server.request_txs(collected_tx_ids.clone()).await {
            panic!("SERVER: error replying TxIds => {err:?}");
        }

        // Receive Tx bodies
        let txs_reply = match tx_server.receive_next_reply().await {
            Ok(reply) => reply,
            Err(err) => {
                panic!("SERVER: error receiving next reply => {err:?}");
            }
        };

        // Process the Tx bodies
        if let Reply::Txs(bodies) = txs_reply {
            info!(
                "SERVER: State => {:?}, got Tx bodies => {}",
                tx_server.state(),
                bodies.len()
            );
            acknowledge += bodies.len() as u16;
            let to_acknowledge_ids = collected_tx_ids.drain(..bodies.len()).collect::<Vec<_>>();
            let mut acknowledge_txs = self.acknowledge_txs.lock().unwrap();
            to_acknowledge_ids.iter().for_each(|EraTxId(_, hash)| {
                acknowledge_txs.push(pallas::crypto::hash::Hash::new(
                    hash.as_slice().try_into().unwrap(),
                ));
            });
            info!(
                "SERVER: ack so far => {acknowledge}, State => {:?}, agency is theirs",
                tx_server.state()
            );
            info!("SERVER: got Tx bodies => {}", bodies.len());
        }

        info!("SERVER: done, closing connection");
        peer_server.abort().await;
        *self.is_done.write().unwrap() = true;
        info!("SERVER: connection closed");
    }
}

#[cfg(test)]
mod broadcast_tests {

    use tracing::info;

    #[tokio::test]
    async fn it_should_demonstrate_processing_speed_differences() {
        use gasket::messaging::{InputPort, Message, OutputPort};
        use std::sync::Arc;
        use std::time::{Duration, Instant};
        use tokio::sync::RwLock;
        use tracing::info;

        // Initialize logging
        let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();

        // 1. Spawn two mock servers - one fast and one slow
        let fast_server = Arc::new(
            crate::network::mock_ouroboros_tx_submit_server::MockOuroborosTxSubmitPeerServer::new(
                "0.0.0.0:3001".to_string(),
                2,
            ),
        );
        let slow_server = Arc::new(
            crate::network::mock_ouroboros_tx_submit_server::MockOuroborosTxSubmitPeerServer::new(
                "0.0.0.0:3002".to_string(),
                2,
            ),
        );

        // Initialize both servers
        fast_server.clone().init().await;
        slow_server.clone().init().await;

        // Set up channels for both servers
        let (fast_sender, fast_receiver) =
            gasket::messaging::tokio::broadcast_channel::<Vec<u8>>(5);
        let (slow_sender, slow_receiver) =
            gasket::messaging::tokio::broadcast_channel::<Vec<u8>>(5);

        let mut fast_output = OutputPort::<Vec<u8>>::default();
        fast_output.connect(fast_sender.clone());

        let mut slow_output = OutputPort::<Vec<u8>>::default();
        slow_output.connect(slow_sender.clone());

        // Allow time for servers to start up
        tokio::time::sleep(Duration::from_millis(200)).await;

        // 2. Set up peer clients
        let mut fast_client = crate::network::peer::Peer::new("127.0.0.1:3001", 2);
        let mut slow_client = crate::network::peer::Peer::new("127.0.0.1:3002", 2);

        let mut fast_input = InputPort::<Vec<u8>>::default();
        fast_input.connect(fast_receiver.clone());
        fast_client.input = Arc::new(RwLock::new(fast_input));

        let mut slow_input = InputPort::<Vec<u8>>::default();
        slow_input.connect(slow_receiver.clone());
        slow_client.input = Arc::new(RwLock::new(slow_input));

        // Initialize clients
        fast_client.init().await.unwrap();
        slow_client.init().await.unwrap();

        // 3. Run the test for a fixed duration, sending transactions continuously
        let test_duration = Duration::from_secs(5);
        let start_time = Instant::now();

        // Define different intervals for fast and slow senders
        let fast_tx_interval = Duration::from_millis(50);
        let slow_tx_interval = Duration::from_millis(150);

        // Generate sample transactions to send
        let sample_txs = vec![
            vec![1u8, 2, 3],
            vec![4u8, 5, 6],
            vec![7u8, 8, 9],
            vec![10u8, 11, 12],
            vec![13u8, 14, 15],
            vec![16u8, 17, 18],
            vec![19u8, 20, 21],
            vec![22u8, 23, 24],
        ];

        // Spawn two tasks: one sending quickly, the other slowly
        let fast_task = {
            let mut fast_output = fast_output;
            let sample_txs = sample_txs.clone();
            tokio::spawn(async move {
                while start_time.elapsed() < test_duration {
                    for tx in &sample_txs {
                        let message = Message::from(tx.clone());
                        fast_output.send(message).await.unwrap();
                        tokio::time::sleep(fast_tx_interval).await;
                        if start_time.elapsed() >= test_duration {
                            break;
                        }
                    }
                }
            })
        };

        let slow_task = {
            let mut slow_output = slow_output;
            let sample_txs = sample_txs.clone();
            tokio::spawn(async move {
                while start_time.elapsed() < test_duration {
                    for tx in &sample_txs {
                        let message = Message::from(tx.clone());
                        slow_output.send(message).await.unwrap();
                        tokio::time::sleep(slow_tx_interval).await;
                        if start_time.elapsed() >= test_duration {
                            break;
                        }
                    }
                }
            })
        };

        // Wait for both tasks to complete
        fast_task.await.unwrap();
        slow_task.await.unwrap();

        // Allow time for final processing
        tokio::time::sleep(Duration::from_secs(1)).await;

        // 4. Count and compare processed transactions
        let fast_tx_count = fast_server.acknowledge_txs.lock().unwrap().len();
        let slow_tx_count = slow_server.acknowledge_txs.lock().unwrap().len();

        info!(
            "Fast server processed {} transactions, Slow server processed {} transactions",
            fast_tx_count, slow_tx_count
        );

        // 5. Assert that the fast server processed more transactions
        assert!(
        fast_tx_count > slow_tx_count,
        "Fast server should process more transactions than slow server, but got: fast={}, slow={}",
        fast_tx_count,
        slow_tx_count
    );
    }
}
