//! Batch SOL balance queries via configured `RPC_ENDPOINT` (`RPC_CLIENT` in `bot_config`).

use crate::RPC_CLIENT;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

pub async fn batch_wallet_balances_lamports(pubkeys: &[String]) -> Vec<u64> {
    let mut out = vec![0u64; pubkeys.len()];
    let mut valid_indices: Vec<usize> = Vec::new();
    let mut valid_pks: Vec<Pubkey> = Vec::new();

    for (i, s) in pubkeys.iter().enumerate() {
        if let Ok(pk) = Pubkey::from_str(s.trim()) {
            valid_indices.push(i);
            valid_pks.push(pk);
        }
    }

    if valid_pks.is_empty() {
        return out;
    }

    match RPC_CLIENT.get_multiple_accounts(&valid_pks).await {
        Ok(accounts) => {
            if accounts.len() != valid_indices.len() {
                eprintln!(
                    "batch_wallet_balances_lamports: RPC returned {} accounts for {} pubkeys",
                    accounts.len(),
                    valid_indices.len()
                );
            }
            for (idx, acc) in valid_indices.iter().zip(accounts.iter()) {
                out[*idx] = acc.as_ref().map(|a| a.lamports).unwrap_or(0);
            }
        }
        Err(e) => {
            eprintln!("batch_wallet_balances_lamports: RPC error: {e}");
        }
    }

    out
}

/// Lamports → SOL as `f64`.
#[inline]
pub fn lamports_to_sol(lamports: u64) -> f64 {
    lamports as f64 / 1_000_000_000.0
}

/// Default max wait for wallet balance batch (Telegram UX / slow RPC protection).
pub const WALLET_BALANCE_RPC_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(12);
