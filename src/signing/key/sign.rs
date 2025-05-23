use bip39::Mnemonic;
use pallas::{
    ledger::traverse::MultiEraTx,
    txbuilder::{BuildConway, BuiltTransaction, Bytes, Bytes32, StagingTransaction},
};

use super::derive::get_ed25519_keypair;

pub fn to_built_transaction(tx: &MultiEraTx) -> BuiltTransaction {
    let staging_tx = StagingTransaction::new();
    let built_tx = staging_tx.build_conway_raw().unwrap();

    BuiltTransaction {
        version: built_tx.version,
        era: built_tx.era,
        status: built_tx.status,
        tx_hash: Bytes32(*tx.hash()),
        tx_bytes: Bytes(tx.encode()),
        signatures: None,
    }
}

pub fn sign_transaction(built_tx: BuiltTransaction, mnemonic: &Mnemonic) -> BuiltTransaction {
    let (signing_key, _) = get_ed25519_keypair(mnemonic);

    let signed_tx = built_tx.sign(&signing_key);

    match signed_tx {
        Ok(tx) => tx,
        _ => panic!("Failed to sign transaction"),
    }
}
