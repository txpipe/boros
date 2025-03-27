use bip39::{rand_core::OsRng, Mnemonic};
use pallas::{
    crypto::{hash::Hasher, key::ed25519::PublicKey},
    ledger::addresses::{
        Address, Network, ShelleyAddress, ShelleyDelegationPart, ShelleyPaymentPart,
    },
    wallet::keystore::{
        hd::{Bip32PrivateKey, Bip32PublicKey},
        PrivateKey,
    },
};

pub fn generate_mnemonic(passphrase: &str) -> Mnemonic {
    Bip32PrivateKey::generate_with_mnemonic(OsRng, passphrase.into()).1
}

/// @TODO remove dead code after finalizing POC
#[allow(dead_code)]
pub fn generate_account_key(mnemonic: &Mnemonic, passphrase: &str) -> Bip32PrivateKey {
    let root_key =
        Bip32PrivateKey::from_bip39_mnenomic(mnemonic.to_string(), passphrase.into()).unwrap();
    root_key
        .derive(1852 | 0x80000000)
        .derive(1815 | 0x80000000)
        .derive(0x80000000)
}

#[allow(dead_code)]
pub fn generate_payment_keypair(
    account_key: &Bip32PrivateKey,
) -> (Bip32PrivateKey, Bip32PublicKey) {
    let external_key = account_key.derive(0);
    let private_key = external_key.derive(0);
    let public_key = private_key.to_public();
    (private_key, public_key)
}

#[allow(dead_code)]
pub fn generate_delegation_keypair(
    account_key: &Bip32PrivateKey,
) -> (Bip32PrivateKey, Bip32PublicKey) {
    let delegation_key = account_key.derive(2);
    let private_key = delegation_key.derive(0);
    let public_key = private_key.to_public();
    (private_key, public_key)
}

#[allow(dead_code)]
pub fn generate_address(public_key: &Bip32PublicKey) -> Address {
    let payment_hash = Hasher::<224>::hash(&public_key.as_bytes()[..32]);
    let address = ShelleyAddress::new(
        Network::Testnet,
        ShelleyPaymentPart::key_hash(payment_hash),
        ShelleyDelegationPart::Null,
    );

    Address::Shelley(address)
}

#[allow(dead_code)]
pub fn generate_address_with_delegation(
    account_key: &Bip32PrivateKey,
    public_key: &Bip32PublicKey,
) -> Address {
    let payment_hash = Hasher::<224>::hash(public_key.as_bytes().as_slice());

    let (_, public_key) = generate_delegation_keypair(account_key);
    let delegation_hash = Hasher::<224>::hash(public_key.as_bytes().as_slice());

    let address = ShelleyAddress::new(
        Network::Testnet,
        ShelleyPaymentPart::key_hash(payment_hash),
        ShelleyDelegationPart::key_hash(delegation_hash),
    );

    Address::Shelley(address)
}

#[allow(dead_code)]
pub fn to_ed25519_keypair(private_key: &Bip32PrivateKey) -> (PrivateKey, PublicKey) {
    let private_key = private_key.to_ed25519_private_key();
    let public_key = private_key.public_key();
    (private_key, public_key)
}
