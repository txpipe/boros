use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{net::TcpListener, sync::RwLock, task};
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
    pub processing_delay: Option<Duration>,
}

impl MockOuroborosTxSubmitPeerServer {
    pub fn new(socket_addr: String, network_magic: u64) -> Self {
        Self {
            socket_addr,
            network_magic,
            acknowledge_txs: Arc::new(Mutex::new(vec![])),
            is_done: Arc::new(RwLock::new(false)),
            processing_delay: None,
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
        let count = 5u16;

        info!("SERVER: Current State: {:?}", tx_server.state());
        info!("SERVER: requesting TxIds (blocking=true)");

        if let Some(delay) = self.processing_delay {
            tokio::time::sleep(delay).await;
        }

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
        *self.is_done.write().await = true;
        info!("SERVER: connection closed");
    }
}

#[cfg(test)]
mod broadcast_tests {

    use pallas::{
        crypto::hash::Hash,
        ledger::primitives::{
            conway::{
                PostAlonzoTransactionOutput, PseudoTransactionOutput, TransactionBody, Tx, Value,
                WitnessSet,
            },
            Fragment, TransactionInput,
        },
    };
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
        let fast_server =
            crate::network::mock_ouroboros_tx_submit_server::MockOuroborosTxSubmitPeerServer::new(
                "0.0.0.0:3001".to_string(),
                2,
            );
        let mut slow_server =
            crate::network::mock_ouroboros_tx_submit_server::MockOuroborosTxSubmitPeerServer::new(
                "0.0.0.0:3002".to_string(),
                2,
            );

        slow_server.processing_delay = Some(Duration::from_secs(5));

        let fast_server = Arc::new(fast_server);
        let slow_server = Arc::new(slow_server);

        // Initialize both servers
        fast_server.clone().init().await;
        slow_server.clone().init().await;

        let tx_interval = Duration::from_millis(50);

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
        tokio::time::sleep(Duration::from_millis(10)).await;

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

        // Generate sample transactions to send
        let sample_txs = generate_sample_transactions();

        while start_time.elapsed() < test_duration {
            for tx in &sample_txs {
                let message = Message::from(tx.clone());
                // Send the same transaction to both outputs at the same time
                fast_output.send(message.clone()).await.unwrap();
                slow_output.send(message.clone()).await.unwrap();

                // Use the same interval for both
                tokio::time::sleep(tx_interval).await;
                if start_time.elapsed() >= test_duration {
                    break;
                }
            }
        }

        // Allow time for final processing
        tokio::time::sleep(Duration::from_secs(3)).await;

        // 4. Count and compare processed transactions
        let fast_tx_count = fast_server.acknowledge_txs.lock().unwrap().len();
        let slow_tx_count = slow_server.acknowledge_txs.lock().unwrap().len();

        info!(
            "Fast server processed {} transactions, Slow server processed {} transactions",
            fast_tx_count, slow_tx_count
        );

        // 5. Assert that the fast server processed more transactions
        assert_eq!(sample_txs.len(), fast_tx_count);
        assert_ne!(sample_txs.len(), slow_tx_count);

        // After current assertions, add this code:
        // Extract the transaction hashes for analysis
        let fast_tx_hashes = fast_server.acknowledge_txs.lock().unwrap().clone();
        let slow_tx_hashes = slow_server.acknowledge_txs.lock().unwrap().clone();

        // Count transactions that were processed by fast but lagged in slow
        let mut lagged_txs = 0;
        for fast_hash in &fast_tx_hashes {
            if !slow_tx_hashes.contains(fast_hash) {
                lagged_txs += 1;
            }
        }

        info!(
            "Fast server processed all {} transactions. Slow server lagged on {} transactions.",
            fast_tx_count, lagged_txs
        );

        // Assert that some transactions were lagged (but not all)
        assert!(
            lagged_txs > 0,
            "Expected some transactions to lag in slow server"
        );
        assert!(
            slow_tx_count > 0,
            "Slow server should process some transactions"
        );
    }

    fn generate_sample_transactions() -> Vec<Vec<u8>> {
        let mut sample_txs = Vec::new();

        for i in 0..10 {
            let input = TransactionInput {
                transaction_id: Hash::<32>::from([i as u8; 32]),
                index: i as u64,
            };

            let output = PseudoTransactionOutput::PostAlonzo(PostAlonzoTransactionOutput {
                address: vec![100 + i as u8; 28].into(),
                value: Value::Coin((i as u64 + 1) * 1_000_000),
                datum_option: None,
                script_ref: None,
            });

            let tx = Tx {
                transaction_body: TransactionBody {
                    inputs: vec![input].into(),
                    outputs: vec![output],
                    fee: (i as u64 + 1) * 100_000,
                    ttl: Some(1_000_000),
                    validity_interval_start: None,
                    certificates: None,
                    withdrawals: None,
                    auxiliary_data_hash: None,
                    mint: None,
                    script_data_hash: None,
                    collateral: None,
                    required_signers: None,
                    network_id: None,
                    collateral_return: None,
                    total_collateral: None,
                    reference_inputs: None,
                    voting_procedures: None,
                    proposal_procedures: None,
                    treasury_value: None,
                    donation: None,
                },
                transaction_witness_set: WitnessSet {
                    vkeywitness: None,
                    native_script: None,
                    bootstrap_witness: None,
                    plutus_v1_script: None,
                    plutus_v2_script: None,
                    plutus_v3_script: None,
                    plutus_data: None,
                    redeemer: None,
                },
                success: true,
                auxiliary_data: None.into(),
            };

            // Serialize the transaction to CBOR bytes
            match tx.encode_fragment() {
                Ok(bytes) => sample_txs.push(bytes),
                Err(e) => {
                    // Log error but continue with other transactions
                    tracing::error!("Failed to serialize transaction: {:?}", e);
                    continue;
                }
            }
        }

        sample_txs
    }
}
