//! Module 4: Genesis Bundle Detector.
//!
//! Phát hiện token có bị "bundled buy" ngay tại genesis block không.
//! Kỹ thuật rug phổ biến: dev tạo nhiều ví, mua hàng loạt trong block đầu tiên
//! → chiếm phần lớn supply → dump sau khi giá lên.
//!
//! Tiêu chí:
//! - Tổng % supply được mua trong genesis block (max 50%)
//! - Số ví clustered (mua cùng block, max 3)

use crate::RPC_CLIENT;
use solana_client::rpc_config::RpcBlockConfig;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};
use solana_transaction_status_client_types::{TransactionDetails, UiTransactionEncoding};
use std::{collections::HashMap, time::Duration};

/// Kết quả phân tích genesis block.
pub struct GenesisAnalysis {
    /// Tổng % supply được mua trong genesis block.
    pub genesis_buy_pct: f64,
    /// Số ví unique đã mua trong genesis block.
    pub unique_buyers: usize,
    /// Có phát hiện bundle pattern không.
    pub bundle_detected: bool,
}

/// Phân tích genesis block — tìm bundled buys.
///
/// Quét block tại `creation_slot` để tìm các TX liên quan tới `mint`.
/// Đếm số ví mua và tổng % supply mua được.
pub async fn analyze_genesis_block(
    mint: &Pubkey,
    creation_slot: u64,
    total_supply: f64,
    max_genesis_buy_pct: f64,
    max_clustered_wallets: u32,
    timeout_ms: u64,
) -> Result<Option<GenesisAnalysis>, Box<dyn std::error::Error + Send + Sync>> {
    if total_supply <= 0.0 {
        return Ok(None);
    }

    let config = RpcBlockConfig {
        encoding: Some(UiTransactionEncoding::Base64),
        transaction_details: Some(TransactionDetails::Full),
        rewards: Some(false),
        commitment: Some(CommitmentConfig::confirmed()),
        max_supported_transaction_version: Some(0),
    };

    // Lấy block data với timeout
    let result = tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        RPC_CLIENT.get_block_with_config(creation_slot, config),
    )
    .await;

    let block = match result {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => {
            return Err(format!("RPC get_block error: {e}").into());
        }
        Err(_) => {
            return Err("Genesis detector RPC timeout".into());
        }
    };

    // Phân tích các transactions trong block
    let mint_str = mint.to_string();
    let mut buyer_amounts: HashMap<String, f64> = HashMap::new();

    if let Some(transactions) = block.transactions {
        for tx_with_meta in &transactions {
            // Fix #9: Trực tiếp quét post_token_balances thay vì serialize toàn bộ TX

            // Trích xuất thông tin từ transaction meta
            if let Some(meta) = &tx_with_meta.meta {
                // Tìm sự thay đổi balance liên quan tới token
                if let solana_transaction_status_client_types::option_serializer::OptionSerializer::Some(post_balances) = &meta.post_token_balances {
                    for token_balance in post_balances.iter() {
                        let token_mint_str = token_balance.mint.clone();
                        if token_mint_str != mint_str {
                            continue;
                        }

                        // Lấy owner và số lượng token
                        if let solana_transaction_status_client_types::option_serializer::OptionSerializer::Some(owner) =
                            &token_balance.owner
                        {
                            let amount = token_balance
                                .ui_token_amount
                                .ui_amount
                                .unwrap_or(0.0);

                            if amount > 0.0 {
                                // Use insert (overwrite) instead of +=
                                // because post_token_balances is CUMULATIVE.
                                // If a wallet appears in multiple TXs in the same block,
                                // its post_balance already includes all previous buys.
                                // Using += would double-count → values >100%.
                                buyer_amounts.insert(owner.clone(), amount);
                            }
                        }
                    }
                }
            }
        }
    }

    if buyer_amounts.is_empty() {
        return Ok(Some(GenesisAnalysis {
            genesis_buy_pct: 0.0,
            unique_buyers: 0,
            bundle_detected: false,
        }));
    }

    // Tổng % supply được mua trong genesis block
    let total_bought: f64 = buyer_amounts.values().sum();
    let genesis_buy_pct = ((total_bought / total_supply) * 100.0).min(100.0);
    let unique_buyers = buyer_amounts.len();

    // Detect bundle pattern: nhiều ví mua cùng block + tổng % cao
    let bundle_detected = unique_buyers as u32 > max_clustered_wallets
        && genesis_buy_pct > max_genesis_buy_pct;

    Ok(Some(GenesisAnalysis {
        genesis_buy_pct,
        unique_buyers,
        bundle_detected,
    }))
}

/// Kiểm tra genesis bundles — gọi bởi pre_buy_filter.rs.
///
/// Trả về:
/// - `Ok(Some(genesis_buy_pct))` — % supply mua trong genesis
/// - `Err(reason)` — nếu vượt ngưỡng cho phép
pub async fn check_genesis_bundles(
    mint: &Pubkey,
    creation_slot: u64,
    total_supply: f64,
    max_genesis_buy_pct: f64,
    max_clustered_wallets: u32,
    timeout_ms: u64,
) -> Result<Option<f64>, String> {
    let analysis = analyze_genesis_block(
        mint,
        creation_slot,
        total_supply,
        max_genesis_buy_pct,
        max_clustered_wallets,
        timeout_ms,
    )
    .await
    .map_err(|e| format!("Genesis detector error: {e}"))?;

    match analysis {
        None => Ok(None),

        Some(a) => {
            if a.bundle_detected {
                Err(format!(
                    "Genesis bundle detected: {:.1}% supply bought by {} wallets in genesis block",
                    a.genesis_buy_pct, a.unique_buyers
                ))
            } else {
                Ok(Some(a.genesis_buy_pct))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_analysis_struct() {
        let analysis = GenesisAnalysis {
            genesis_buy_pct: 45.5,
            unique_buyers: 5,
            bundle_detected: true,
        };
        assert_eq!(analysis.unique_buyers, 5);
        assert!((analysis.genesis_buy_pct - 45.5).abs() < 0.01);
        assert!(analysis.bundle_detected);
    }

    #[test]
    fn test_bundle_detection_logic() {
        let max_clustered_wallets: u32 = 3;
        let max_genesis_buy_pct: f64 = 50.0;

        // 5 wallets bought 60% → BUNDLE DETECTED
        let unique_buyers: usize = 5;
        let genesis_buy_pct: f64 = 60.0;
        let detected = unique_buyers as u32 > max_clustered_wallets
            && genesis_buy_pct > max_genesis_buy_pct;
        assert!(detected, "5 wallets + 60% should detect bundle");

        // 2 wallets bought 80% → NOT detected (few wallets, normal whale)
        let few_buyers: usize = 2;
        let detected2 = few_buyers as u32 > max_clustered_wallets
            && genesis_buy_pct > max_genesis_buy_pct;
        assert!(!detected2, "2 wallets should not trigger bundle (could be 1 whale)");
    }

    #[test]
    fn test_genesis_buy_pct_calculation() {
        let total_supply: f64 = 1_000_000.0;

        // buyer_amounts simulation
        let mut buyer_amounts: HashMap<String, f64> = HashMap::new();
        buyer_amounts.insert("wallet_a".to_string(), 200_000.0);
        buyer_amounts.insert("wallet_b".to_string(), 150_000.0);
        buyer_amounts.insert("wallet_c".to_string(), 50_000.0);

        let total_bought: f64 = buyer_amounts.values().sum();
        let pct = (total_bought / total_supply) * 100.0;

        assert!((pct - 40.0).abs() < 0.01, "400k/1M = 40%");
        assert_eq!(buyer_amounts.len(), 3, "3 unique buyers");
    }

    #[test]
    fn test_safe_genesis_no_bundle() {
        let max_clustered = 3u32;
        let max_pct = 50.0;

        // Only 1 buyer at 10% → safe
        let buyers = 1usize;
        let pct = 10.0;
        let detected = buyers as u32 > max_clustered && pct > max_pct;
        assert!(!detected, "1 buyer at 10% is safe");
    }
}
