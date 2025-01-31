use std::sync::{Arc, Mutex, RwLock};

use pallas::network::{
    facades::PeerServer,
    miniprotocols::txsubmission::{EraTxId, Reply, State},
};
use tokio::{net::TcpListener, task};
use tracing::{error, info};

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

        info!("SERVER: init received, now we have agency => requesting TxIds in a loop...");

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

        // Recieve TxIds for the blocking request
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

        // Recieve TxIds for the non-blocking request
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

        // request the Tx bodies
        if let Err(err) = tx_server.request_txs(collected_tx_ids.clone()).await {
            panic!("SERVER: error replying TxIds => {err:?}");
        }

        // Recieve Tx bodies
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
