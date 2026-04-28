use std::sync::Mutex;

use once_cell::sync::Lazy;
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    hash::Hash,
};
use tokio::time::{Duration, sleep};

use crate::*;

static RECENT_HASH: Lazy<Mutex<Hash>> = Lazy::new(|| Mutex::new(Hash::default()));

fn set_slot(value: Hash) {
    let mut slot = RECENT_HASH.lock().unwrap();
    *slot = value;
}

pub fn get_slot() -> Hash {
    let slot = RECENT_HASH.lock().unwrap();
    *slot
}

pub async fn recent_blockhash_handler() {
    loop {
        match RPC_CLIENT.get_latest_blockhash_with_commitment(CommitmentConfig {
            commitment: CommitmentLevel::Processed,
        }).await {
            Ok((latest_blockhash, _)) => {
                set_slot(latest_blockhash);
                break;
            }
            Err(_) => {
                sleep(Duration::from_millis(200)).await;
            }
        };
    }

    sleep(Duration::from_millis(500)).await;
}