use std::sync::Arc;

use anyhow::Result;
use pallas::ledger::traverse::MultiEraTx;
use rocket::data::{Data, ToByteUnit};
use rocket::http::Status;
use rocket::{post, routes, State};
use tracing::error;

use crate::storage::sqlite::SqliteTransaction;
use crate::storage::Transaction;

/// POST endpoint that accepts "application/cbor" data.
/// Reads the body as raw bytes, then pushes it into our mempool.
#[post("/api/submit/tx", format = "application/cbor", data = "<cbor_data>")]
async fn submit_tx(
    cbor_data: Data<'_>,
    tx_storage: &State<Arc<SqliteTransaction>>,
) -> Result<String, Status> {
    // Limit how many bytes we read (16 KB here).
    let max_size = 16.kilobytes();

    // Read the raw bytes from the request body.
    let bytes = match cbor_data.open(max_size).into_bytes().await {
        Ok(buf) => buf,
        Err(_) => return Err(Status::PayloadTooLarge),
    };

    // The `bytes.value` is a `Vec<u8>` containing the raw CBOR data.
    let raw_cbor = bytes.value;

    tracing::info!("Tx Cbor: {:?}", hex::encode(&raw_cbor));

    let parsed_tx = MultiEraTx::decode(&raw_cbor).unwrap();
    let tx_hash = parsed_tx.hash();

    let mut txs: Vec<Transaction> = Vec::default();
    txs.push(Transaction::new(tx_hash.to_string(), raw_cbor.clone()));
    // Store the transaction in our mempool.
    // We'll lock the mutex, then push the new transaction.
    // cbor_txs_db.push_tx(raw_cbor.clone());
    tx_storage.create(&txs).await.map_err(|error| {
        error!(?error);
        Status::InternalServerError
    })?;

    // Return the transaction hash as a response.
    Ok(hex::encode(tx_hash))
}

pub async fn run(tx_storage: Arc<SqliteTransaction>) -> Result<()> {
    let _ = rocket::build()
        .manage(tx_storage)
        .mount("/", routes![submit_tx])
        .launch()
        .await?;
    Ok(())
}
