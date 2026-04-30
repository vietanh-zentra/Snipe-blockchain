//! Module 4: Genesis Bundle Detector.
//!
//! Phát hiện dev dùng Jito bundles để mua bulk supply trong cùng block
//! tạo token (genesis block). Đây là dấu hiệu mạnh nhất của coordinated rug.
//!
//! Logic:
//! 1. Lấy slot của migration transaction (block index khi pool được tạo)
//! 2. Fetch block đó qua RPC
//! 3. Tìm tất cả buy transactions liên quan đến token mint này
//! 4. Tính tổng % supply được mua trong genesis block
//! 5. Nếu > threshold → Fail

use crate::{RPC_CLIENT, PUMPFUN_PROGRAM_ID};
use solana_client::rpc_config::{RpcBlockConfig, RpcTransactionConfig};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::{TransactionDetails, UiTransactionEncoding};
use std::time::Duration;
use tokio::time::timeout;

pub struct GenesisAnalysis {
    /// % supply mua trong genesis block (0-100).
    pub genesis_buy_pct: f64,
    /// Số unique wallets mua trong genesis block.
    pub unique_buyers: usize,
    /// True nếu phát hiện bundled buys (nhiều wallets mua cùng block).
    pub bundle_detected: bool,
}

/// Phân tích genesis block của một token migration.
///
/// `creation_slot` là slot mà pool creation TX nằm trong.
pub async fn analyze_genesis_block(
    mint: &Pubkey,
    creation_slot: u64,
    total_supply: f64,
    max_genesis_buy_pct: f64,
    max_clustered_wallets: u32,
    timeout_ms: u64,
) -> Result<Option<GenesisAnalysis>, Box<dyn std::error::Error + Send + Sync>> {
    let mint = *mint;
    let duration = Duration::from_millis(timeout_ms);

    let result = timeout(duration, async move {
        fetch_genesis_data(&mint, creation_slot, total_supply).await
    })
    .await;

    match result {
        Ok(inner) => inner.map(Some),
        Err(_elapsed) => {
            eprintln!(
                "[GENESIS_DETECTOR] Timeout after {}ms for slot {}",
                timeout_ms, creation_slot
            );
            Ok(None)
        }
    }
}

async fn fetch_genesis_data(
    mint: &Pubkey,
    slot: u64,
    total_supply: f64,
) -> Result<GenesisAnalysis, Box<dyn std::error::Error + Send + Sync>> {
    let config = RpcBlockConfig {
        encoding: Some(UiTransactionEncoding::Base64),
        transaction_details: Some(TransactionDetails::Full),
        rewards: Some(false),
        commitment: None,
        max_supported_transaction_version: Some(0),
    };

    let block = RPC_CLIENT
        .get_block_with_config(slot, config)
        .await
        .map_err(|e| format!("get_block failed for slot {slot}: {e}"))?;

    let transactions = match block.transactions {
        Some(txs) => txs,
        None => return Err("Block has no transactions".into()),
    };

    // Scan transactions tìm buy instructions liên quan đến mint này
    // Đây là heuristic: tìm TX có account references tới mint
    let mint_str = mint.to_string();
    let mut unique_buyers: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut total_bought_ui: f64 = 0.0;

    for tx_with_meta in &transactions {
        if let Some(meta) = &tx_with_meta.meta {
            // Kiểm tra nếu TX có liên quan đến mint này qua post token balances
            let post_balances = match &meta.post_token_balances {
                solana_transaction_status::option_serializer::OptionSerializer::Some(b) => b,
                _ => continue,
            };

            for balance in post_balances {
                if balance.mint == mint_str {
                    if let Some(ui_amount) = balance.ui_token_amount.ui_amount {
                        if ui_amount > 0.0 {
                            // Tìm thấy buyer — lấy owner
                            if let solana_transaction_status::option_serializer::OptionSerializer::Some(owner) =
                                &balance.owner
                            {
                                unique_buyers.insert(owner.clone());
                                total_bought_ui += ui_amount;
                            }
                        }
                    }
                }
            }
        }
    }

    let genesis_buy_pct = if total_supply > 0.0 {
        (total_bought_ui / total_supply) * 100.0
    } else {
        0.0
    };

    let bundle_detected = unique_buyers.len() >= 2; // Nhiều ví mua cùng block

    Ok(GenesisAnalysis {
        genesis_buy_pct,
        unique_buyers: unique_buyers.len(),
        bundle_detected,
    })
}

/// Kiểm tra genesis block và return Err(reason) nếu fail.
pub async fn check_genesis_bundles(
    mint: &Pubkey,
    creation_slot: u64,
    total_supply: f64,
    max_genesis_buy_pct: f64,
    max_clustered_wallets: u32,
    timeout_ms: u64,
) -> Result<Option<f64>, String> {
    match analyze_genesis_block(
        mint,
        creation_slot,
        total_supply,
        max_genesis_buy_pct,
        max_clustered_wallets,
        timeout_ms,
    )
    .await
    {
        Ok(Some(analysis)) => {
            if analysis.genesis_buy_pct > max_genesis_buy_pct {
                Err(format!(
                    "Genesis block: {:.1}% supply bought by {} wallets (max: {:.1}%)",
                    analysis.genesis_buy_pct, analysis.unique_buyers, max_genesis_buy_pct
                ))
            } else {
                Ok(Some(analysis.genesis_buy_pct))
            }
        }
        Ok(None) => Ok(None), // timeout
        Err(e) => {
            eprintln!("[GENESIS_DETECTOR] Error: {e}");
            Ok(None)
        }
    }
}
