use bip39::Mnemonic;
use pallas::txbuilder::{BuildConway, BuiltTransaction, Bytes, Bytes32, StagingTransaction};

use crate::storage::Transaction;

use super::derive::get_signing_key;

pub fn to_built_transaction(tx: Transaction) -> BuiltTransaction {
    let staging_tx = StagingTransaction::new();
    let built_tx = staging_tx.build_conway_raw().unwrap();

    let tx_hash: [u8; 32] = match hex::decode(tx.id) {
        Ok(hash) => match hash.try_into() {
            Ok(hash) => hash,
            Err(_) => panic!("Failed to convert transaction ID to hash"),
        },
        Err(_) => panic!("Failed to decode transaction ID"),
    };

    BuiltTransaction {
        version: built_tx.version,
        era: built_tx.era,
        status: built_tx.status,
        tx_hash: Bytes32(tx_hash),
        tx_bytes: Bytes(tx.raw),
        signatures: None,
    }
}

pub fn sign_transaction(built_tx: BuiltTransaction, mnemonic: &Mnemonic) -> BuiltTransaction {
    let signing_key = get_signing_key(mnemonic);
    let signed_tx = built_tx.sign(signing_key);
    match signed_tx {
        Ok(tx) => tx,
        _ => panic!("Failed to sign transaction"),
    }
}
