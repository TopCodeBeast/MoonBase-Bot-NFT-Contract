use core::convert::TryFrom;
use ed25519_dalek::Verifier;
use near_sdk::{env, log, Balance, Promise, StorageUsage};

pub(crate) fn refund_extra_storage_deposit(storage_used: StorageUsage, used_balance: Balance) {
    let required_cost = env::storage_byte_cost() * Balance::from(storage_used);
    let attached_deposit = env::attached_deposit()
        .checked_sub(used_balance)
        .expect("not enough attached balance");

    assert!(
        required_cost <= attached_deposit,
        "not enough attached balance {}",
        required_cost,
    );

    let refund = attached_deposit - required_cost;
    if refund > 1 {
        Promise::new(env::predecessor_account_id()).transfer(refund);
    }
}

pub(crate) fn verify(message: Vec<u8>, sign: Vec<u8>, pk: Vec<u8>) {
    let pk = ed25519_dalek::PublicKey::from_bytes(&pk).unwrap();
    if sign.len() != 64 {
        panic!("Invalid signature data length.");
    }
    let mut sig_data: [u8; 64] = [0; 64];
    for i in 0..64 {
        sig_data[i] = sign.get(i).unwrap_or(&0).clone();
    }
    let sign = ed25519_dalek::Signature::try_from(sig_data).unwrap();
    match pk.verify(&message, &sign) {
        Ok(_) => log!("verify ok"),
        Err(_) => panic!("verify error"),
    }
}
