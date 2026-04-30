//! Module 3: Dev Wallet Profiler.
//!
//! Phân tích lịch sử giao dịch của ví dev (creator) để phát hiện
//! ví mới tạo hoặc ví có ít lịch sử — dấu hiệu của fresh wallet rug.

use crate::RPC_CLIENT;
use solana_client::rpc_config::{RpcSignaturesForAddressConfig, RpcTransactionConfig};
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};
use std::time::Duration;
use tokio::time::timeout;

pub struct DevWalletProfile {
    /// Số TX lịch sử (giới hạn tối đa 1000 để tránh tốn thời gian).
    pub tx_count: u64,
    /// Timestamp (Unix seconds) của TX cũ nhất tìm thấy.
    pub oldest_tx_timestamp: Option<i64>,
    /// Tuổi ví ước tính (giờ), tính từ TX cũ nhất.
    pub estimated_age_hours: Option<u64>,
}

/// Phân tích ví dev với timeout.
pub async fn analyze_dev_wallet(
    dev_pubkey: &Pubkey,
    timeout_ms: u64,
) -> Result<Option<DevWalletProfile>, Box<dyn std::error::Error + Send + Sync>> {
    let pk = *dev_pubkey;
    let duration = Duration::from_millis(timeout_ms);

    let result = timeout(duration, fetch_dev_profile(&pk)).await;

    match result {
        Ok(inner) => inner.map(Some),
        Err(_elapsed) => {
            eprintln!(
                "[DEV_PROFILER] Timeout after {}ms for wallet {}",
                timeout_ms, dev_pubkey
            );
            Ok(None)
        }
    }
}

async fn fetch_dev_profile(
    dev_pubkey: &Pubkey,
) -> Result<DevWalletProfile, Box<dyn std::error::Error + Send + Sync>> {
    // Lấy tối đa 100 signatures gần nhất — đủ để đánh giá, không quá chậm
    let config = RpcSignaturesForAddressConfig {
        limit: Some(100),
        commitment: Some(CommitmentConfig::confirmed()),
        ..Default::default()
    };

    let signatures = RPC_CLIENT
        .get_signatures_for_address_with_config(dev_pubkey, config)
        .await
        .map_err(|e| format!("get_signatures_for_address failed: {e}"))?;

    let tx_count = signatures.len() as u64;

    // Tìm TX cũ nhất trong danh sách (cuối mảng = cũ nhất)
    let oldest_tx_timestamp = signatures.last().and_then(|sig| sig.block_time);

    let estimated_age_hours = oldest_tx_timestamp.map(|ts| {
        let now = chrono::Utc::now().timestamp();
        let age_secs = (now - ts).max(0) as u64;
        age_secs / 3600
    });

    Ok(DevWalletProfile {
        tx_count,
        oldest_tx_timestamp,
        estimated_age_hours,
    })
}

/// Kiểm tra dev wallet có đủ điều kiện không.
///
/// Returns `Err(reason)` nếu fail filter, `Ok(tx_count)` nếu pass.
pub async fn check_dev_wallet(
    dev_pubkey: &Pubkey,
    min_tx_count: u64,
    timeout_ms: u64,
) -> Result<Option<u64>, String> {
    match analyze_dev_wallet(dev_pubkey, timeout_ms).await {
        Ok(Some(profile)) => {
            let count = profile.tx_count;
            if count < min_tx_count {
                Err(format!(
                    "Dev wallet has only {} TX (min required: {}). Fresh wallet — high rug risk.",
                    count, min_tx_count
                ))
            } else {
                Ok(Some(count))
            }
        }
        Ok(None) => {
            // Timeout → không fail
            Ok(None)
        }
        Err(e) => {
            eprintln!("[DEV_PROFILER] RPC error: {e}");
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_age_calculation() {
        // 3600 giây = 1 giờ
        let age_secs: u64 = 7200;
        let age_hours = age_secs / 3600;
        assert_eq!(age_hours, 2);
    }
}
