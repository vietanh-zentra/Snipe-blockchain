//! Module 3: Dev Wallet Profiler.
//!
//! [STUB] — Sẽ implement chi tiết ở Phase 2.
//! Hiện tại chỉ return Ok(None) để không block build.

use solana_sdk::pubkey::Pubkey;

pub struct DevWalletProfile {
    pub tx_count: u64,
    pub oldest_tx_timestamp: Option<i64>,
    pub estimated_age_hours: Option<u64>,
}

/// Stub: luôn trả Ok(None) — chưa implement.
pub async fn analyze_dev_wallet(
    _dev_pubkey: &Pubkey,
    _timeout_ms: u64,
) -> Result<Option<DevWalletProfile>, Box<dyn std::error::Error + Send + Sync>> {
    Ok(None)
}

/// Stub: luôn trả Ok(None) — chưa implement.
pub async fn check_dev_wallet(
    _dev_pubkey: &Pubkey,
    _min_tx_count: u64,
    _timeout_ms: u64,
) -> Result<Option<u64>, String> {
    Ok(None)
}
