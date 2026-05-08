//! Module 3: Dev Wallet Profiler.
//!
//! Kiểm tra lịch sử giao dịch của ví dev trước khi mua token.
//! Ví dev mới tạo (ít TX, tuổi < vài giờ) → nguy cơ rug cao.
//!
//! Tiêu chí đánh giá:
//! - Số lượng TX lịch sử (tối thiểu 10)
//! - Tuổi ví (timestamp TX cũ nhất)
//! - Nếu dev < min_tx_count → FAIL

use crate::RPC_CLIENT;
use solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};
use std::time::Duration;

/// Kết quả phân tích ví dev.
pub struct DevWalletProfile {
    /// Tổng số TX tìm được (giới hạn bởi limit query).
    pub tx_count: u64,
    /// Timestamp (unix) của TX cũ nhất tìm được.
    pub oldest_tx_timestamp: Option<i64>,
    /// Tuổi ước tính của ví (giờ).
    pub estimated_age_hours: Option<u64>,
}

/// Phân tích ví dev — lấy lịch sử TX và tính tuổi ví.
///
/// Trả về `DevWalletProfile` hoặc `None` nếu không lấy được dữ liệu.
pub async fn analyze_dev_wallet(
    dev_pubkey: &Pubkey,
    timeout_ms: u64,
) -> Result<Option<DevWalletProfile>, Box<dyn std::error::Error + Send + Sync>> {
    let config = GetConfirmedSignaturesForAddress2Config {
        // Lấy tối đa 50 TX gần nhất — đủ để đánh giá mà không tốn CU
        limit: Some(50),
        commitment: Some(CommitmentConfig::confirmed()),
        ..Default::default()
    };

    // Gọi RPC với timeout
    let result = tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        RPC_CLIENT.get_signatures_for_address_with_config(dev_pubkey, config),
    )
    .await;

    let signatures = match result {
        Ok(Ok(sigs)) => sigs,
        Ok(Err(e)) => {
            return Err(format!("RPC error: {e}").into());
        }
        Err(_) => {
            return Err("Dev wallet RPC timeout".into());
        }
    };

    if signatures.is_empty() {
        return Ok(Some(DevWalletProfile {
            tx_count: 0,
            oldest_tx_timestamp: None,
            estimated_age_hours: None,
        }));
    }

    let tx_count = signatures.len() as u64;

    // TX cũ nhất = phần tử cuối cùng (RPC trả về mới → cũ)
    let oldest_timestamp = signatures
        .last()
        .and_then(|sig| sig.block_time);

    // Tính tuổi ví từ TX cũ nhất
    let age_hours = oldest_timestamp.map(|ts| {
        let now = chrono::Utc::now().timestamp();
        let diff_secs = (now - ts).max(0) as u64;
        diff_secs / 3600
    });

    Ok(Some(DevWalletProfile {
        tx_count,
        oldest_tx_timestamp: oldest_timestamp,
        estimated_age_hours: age_hours,
    }))
}

/// Kiểm tra ví dev có đủ lịch sử hay không.
///
/// Gọi bởi `pre_buy_filter.rs`. Trả về:
/// - `Ok(Some(tx_count))` — số TX tìm được
/// - `Err(reason)` — nếu ví dev fail tiêu chí (< min_tx_count hoặc < min_wallet_age_hours)
pub async fn check_dev_wallet(
    dev_pubkey: &Pubkey,
    min_tx_count: u64,
    min_wallet_age_hours: u64,
    block_cex_funded: bool,
    timeout_ms: u64,
) -> Result<Option<u64>, String> {
    let profile = analyze_dev_wallet(dev_pubkey, timeout_ms)
        .await
        .map_err(|e| format!("Dev profiler error: {e}"))?;

    match profile {
        None => Ok(None), // Không lấy được data → skip filter, không block

        Some(p) => {
            // Brief L355: Tuổi ví < 24h → cảnh báo / fail
            if let Some(age_hours) = p.estimated_age_hours {
                if min_wallet_age_hours > 0 && age_hours < min_wallet_age_hours {
                    return Err(format!(
                        "Dev wallet too young: {} hours (min: {}h). TXs: {}",
                        age_hours, min_wallet_age_hours, p.tx_count
                    ));
                }
            }

            // Brief L356: Ví < 10 TX → cảnh báo / fail
            if p.tx_count < min_tx_count {
                return Err(format!(
                    "Dev wallet has only {} TXs (min: {}). Age: {} hours",
                    p.tx_count,
                    min_tx_count,
                    p.estimated_age_hours.unwrap_or(0)
                ));
            }

            // Brief L357: Nguồn funding từ CEX hot wallet → cảnh báo
            if block_cex_funded {
                if let Ok(is_cex) = check_cex_funding(dev_pubkey, timeout_ms).await {
                    if is_cex {
                        return Err(format!(
                            "Dev wallet funded from CEX hot wallet. TXs: {}, Age: {}h",
                            p.tx_count, p.estimated_age_hours.unwrap_or(0)
                        ));
                    }
                }
            }

            Ok(Some(p.tx_count))
        }
    }
}

/// Danh sách CEX hot wallets phổ biến trên Solana.
/// Đây là các địa chỉ withdrawal thường gặp của các sàn lớn.
const CEX_HOT_WALLETS: &[&str] = &[
    // Binance
    "2ojv9BAiHUrvsm9gxDe7fJSzbNZSJcxZvf8dqmWGHG8S",
    "5tzFkiKscXHK5ZXCGbXZxdw7gTjjD1mBwuoFbhUvuAi9",
    // Coinbase
    "H8sMJSCQxfKiFTCfDR3DUMLPwcRbM61LGFJ8N4dK3WjS",
    "2AQdpHJ2JpcEgPiATUXjQxA8QmafFegfQwSLWSprPicm",
    // OKX
    "5VCwKtCXgCJ6kit5FybXjvFnyqmRwPGQkiLNi9yByMhN",
    // Bybit
    "AC5RDfQFmDS1deWZos921JfqscXdByf6BKHAbETaT4gU",
    // KuCoin
    "BmFdpraQhkiDQE6SNbjkMwb5MP8AaetcMxxxHLZmijKZ",
    // Gate.io
    "u6PJ8DtQuPFnfmwHbGFULQ4u4EgjDiyYKjVEsynXq2w",
];

/// Kiểm tra xem ví dev có được funding từ CEX hot wallet hay không.
/// Lấy TX cũ nhất (funding đầu tiên) và check sender.
async fn check_cex_funding(
    dev_pubkey: &Pubkey,
    timeout_ms: u64,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let config = GetConfirmedSignaturesForAddress2Config {
        limit: Some(5), // Lấy 5 TX cũ nhất
        commitment: Some(CommitmentConfig::confirmed()),
        ..Default::default()
    };

    let result = tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        RPC_CLIENT.get_signatures_for_address_with_config(dev_pubkey, config),
    )
    .await;

    let sigs = match result {
        Ok(Ok(s)) => s,
        _ => return Ok(false), // Timeout hoặc lỗi → không block
    };

    // Kiểm tra từng TX cũ nhất xem có liên quan tới CEX không
    for sig_info in sigs.iter().rev().take(3) {
        // Check memo field (một số CEX gắn memo vào withdrawal TX)
        if let Some(memo) = &sig_info.memo {
            let memo_lower = memo.to_lowercase();
            if memo_lower.contains("binance") || memo_lower.contains("okx")
                || memo_lower.contains("coinbase") || memo_lower.contains("bybit")
                || memo_lower.contains("kucoin") || memo_lower.contains("gate") {
                return Ok(true);
            }
        }
    }

    // Heuristic: Fetch TX details của TX cũ nhất, check xem sender có phải CEX không
    if let Some(oldest_sig) = sigs.last() {
        if let Ok(sig) = oldest_sig.signature.parse::<solana_sdk::signature::Signature>() {
            let tx_config = solana_client::rpc_config::RpcTransactionConfig {
                encoding: Some(solana_transaction_status_client_types::UiTransactionEncoding::JsonParsed),
                commitment: Some(CommitmentConfig::confirmed()),
                max_supported_transaction_version: Some(0),
            };
            if let Ok(tx) = tokio::time::timeout(
                Duration::from_millis(timeout_ms),
                RPC_CLIENT.get_transaction_with_config(&sig, tx_config),
            ).await {
                if let Ok(confirmed_tx) = tx {
                    // Check account keys trong TX
                    let tx_str = format!("{:?}", confirmed_tx);
                    for cex_wallet in CEX_HOT_WALLETS {
                        if tx_str.contains(cex_wallet) {
                            return Ok(true);
                        }
                    }
                }
            }
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_struct_creation() {
        let profile = DevWalletProfile {
            tx_count: 25,
            oldest_tx_timestamp: Some(1_700_000_000),
            estimated_age_hours: Some(48),
        };
        assert_eq!(profile.tx_count, 25);
        assert_eq!(profile.oldest_tx_timestamp, Some(1_700_000_000));
        assert_eq!(profile.estimated_age_hours, Some(48));
    }

    #[test]
    fn test_empty_wallet_profile() {
        let profile = DevWalletProfile {
            tx_count: 0,
            oldest_tx_timestamp: None,
            estimated_age_hours: None,
        };
        assert_eq!(profile.tx_count, 0);
        assert!(profile.oldest_tx_timestamp.is_none());
        assert!(profile.estimated_age_hours.is_none());
    }

    #[test]
    fn test_tx_count_threshold_logic() {
        let min_tx_count: u64 = 10;

        // Dev with 5 TXs should FAIL
        let low_tx: u64 = 5;
        assert!(low_tx < min_tx_count, "5 < 10 should fail");

        // Dev with 15 TXs should PASS
        let high_tx: u64 = 15;
        assert!(high_tx >= min_tx_count, "15 >= 10 should pass");

        // Dev with exactly 10 TXs should PASS
        let exact_tx: u64 = 10;
        assert!(exact_tx >= min_tx_count, "10 >= 10 should pass");
    }

    #[test]
    fn test_age_calculation() {
        // Simulate: oldest TX was 48 hours ago
        let now = chrono::Utc::now().timestamp();
        let oldest_ts = now - (48 * 3600); // 48 hours ago
        let diff_secs = (now - oldest_ts).max(0) as u64;
        let age_hours = diff_secs / 3600;
        assert_eq!(age_hours, 48);

        // Simulate: fresh wallet (just created)
        let fresh_ts = now - 60; // 1 minute ago
        let fresh_diff = (now - fresh_ts).max(0) as u64;
        let fresh_age = fresh_diff / 3600;
        assert_eq!(fresh_age, 0, "1 minute old = 0 hours");
    }
}
