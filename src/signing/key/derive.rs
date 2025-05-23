use bip39::Mnemonic;
use cryptoxide::{hmac::Hmac, pbkdf2::pbkdf2, sha2::Sha512};
use ed25519_bip32::{XPrv, XPub, XPRV_SIZE};
use pallas::{
    crypto::{
        hash::Hasher,
        key::ed25519::{self, PublicKey, SecretKey, SecretKeyExtended},
    },
    ledger::addresses::{
        Address, Network, ShelleyAddress, ShelleyDelegationPart, ShelleyPaymentPart,
    },
};

fn from_bip39_mnenomic(mnemonic: String, password: String) -> anyhow::Result<ed25519_bip32::XPrv> {
    let bip39 = Mnemonic::parse(mnemonic)?;
    let entropy = bip39.to_entropy();

    let mut pbkdf2_result = [0; XPRV_SIZE];

    const ITER: u32 = 4096;

    let mut mac = Hmac::new(Sha512::new(), password.as_bytes());
    pbkdf2(&mut mac, &entropy, ITER, &mut pbkdf2_result);

    Ok(XPrv::normalize_bytes_force3rd(pbkdf2_result))
}

pub fn get_ed25519_keypair(mnemonic: &Mnemonic) -> (SecretKeyExtended, PublicKey) {
    let account_key = generate_account_key(mnemonic);
    let (private_key, _) = generate_payment_keypair(&account_key);

    to_ed25519_keypair(&private_key)
}

pub fn generate_account_key(mnemonic: &Mnemonic) -> XPrv {
    let root_key = from_bip39_mnenomic(mnemonic.to_string(), "".into()).unwrap();
    root_key
        .derive(ed25519_bip32::DerivationScheme::V2, 1852 | 0x80000000)
        .derive(ed25519_bip32::DerivationScheme::V2, 1815 | 0x80000000)
        .derive(ed25519_bip32::DerivationScheme::V2, 0x80000000)
}

pub fn generate_payment_keypair(account_key: &XPrv) -> (XPrv, XPub) {
    let external_key = account_key.derive(ed25519_bip32::DerivationScheme::V2, 0);
    let private_key = external_key.derive(ed25519_bip32::DerivationScheme::V2, 0);
    let public_key = private_key.public();
    (private_key, public_key)
}

pub fn generate_delegation_keypair(account_key: &XPrv) -> (XPrv, XPub) {
    let delegation_key = account_key.derive(ed25519_bip32::DerivationScheme::V2, 2);
    let private_key = delegation_key.derive(ed25519_bip32::DerivationScheme::V2, 0);
    let public_key = private_key.public();
    (private_key, public_key)
}

pub fn generate_address(public_key: &XPub) -> Address {
    let payment_hash = Hasher::<224>::hash(&public_key.as_ref().to_vec()[..32]);
    let address = ShelleyAddress::new(
        Network::Testnet,
        ShelleyPaymentPart::key_hash(payment_hash),
        ShelleyDelegationPart::Null,
    );

    Address::Shelley(address)
}

pub fn generate_address_with_delegation(account_key: &XPrv, public_key: &XPub) -> Address {
    let payment_hash = Hasher::<224>::hash(public_key.as_ref().to_vec().as_slice());

    let (_, public_key) = generate_delegation_keypair(account_key);
    let delegation_hash = Hasher::<224>::hash(public_key.as_ref().to_vec().as_slice());

    let address = ShelleyAddress::new(
        Network::Testnet,
        ShelleyPaymentPart::key_hash(payment_hash),
        ShelleyDelegationPart::key_hash(delegation_hash),
    );

    Address::Shelley(address)
}

pub fn to_ed25519_keypair(private_key: &XPrv) -> (SecretKeyExtended, PublicKey) {
    let private_key =
        unsafe { SecretKeyExtended::from_bytes_unchecked(private_key.extended_secret_key()) };

    let public_key = private_key.public_key();
    (private_key, public_key)
}
