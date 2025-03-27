use pallas::{
    ledger::traverse::MultiEraTx,
    txbuilder::{BuildConway, BuiltTransaction, Bytes, Bytes32, StagingTransaction},
    wallet::keystore::PrivateKey,
};

pub fn to_built_transaction(metx: &MultiEraTx) -> BuiltTransaction {
    let staging_tx = StagingTransaction::new();
    let built_tx = staging_tx.build_conway_raw().unwrap();

    BuiltTransaction {
        version: built_tx.version,
        era: built_tx.era,
        status: built_tx.status,
        tx_hash: Bytes32(*metx.hash()),
        tx_bytes: Bytes(metx.encode()),
        signatures: None,
    }
}

pub fn sign_transaction(built_tx: BuiltTransaction, signing_key: PrivateKey) -> BuiltTransaction {
    let signed_tx = built_tx.sign(signing_key);
    match signed_tx {
        Ok(tx) => tx,
        _ => panic!("Failed to sign transaction"),
    }
}
