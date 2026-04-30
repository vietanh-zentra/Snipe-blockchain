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
/// - `Err(reason)` — nếu ví dev fail tiêu chí (< min_tx_count)
pub async fn check_dev_wallet(
    dev_pubkey: &Pubkey,
    min_tx_count: u64,
    timeout_ms: u64,
) -> Result<Option<u64>, String> {
    let profile = analyze_dev_wallet(dev_pubkey, timeout_ms)
        .await
        .map_err(|e| format!("Dev profiler error: {e}"))?;

    match profile {
        None => Ok(None), // Không lấy được data → skip filter, không block

        Some(p) => {
            if p.tx_count < min_tx_count {
                Err(format!(
                    "Dev wallet has only {} TXs (min: {}). Age: {} hours",
                    p.tx_count,
                    min_tx_count,
                    p.estimated_age_hours.unwrap_or(0)
                ))
            } else {
                Ok(Some(p.tx_count))
            }
        }
    }
}
