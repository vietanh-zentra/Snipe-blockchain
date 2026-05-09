//! Module 1: Pre-Migration Holder Concentration Analysis.
//!
//! Sử dụng Solana RPC để lấy top 10 holders của token mint và tính %
//! supply họ nắm giữ. Nếu vượt ngưỡng cấu hình → filter Fail.

use crate::RPC_CLIENT;
use solana_sdk::pubkey::Pubkey;
use std::time::Duration;
use tokio::time::timeout;

pub struct HolderAnalysis {
    pub top10_holder_pct: f64,
    pub holder_count: usize,
}

/// Phân tích phân bố holder của `mint`.
///
/// # Returns
/// - `Ok(Some(analysis))` — thành công
/// - `Ok(None)` — timeout hoặc dữ liệu không đủ để phân tích
/// - `Err(e)` — lỗi RPC nghiêm trọng
pub async fn analyze_holders(
    mint: &Pubkey,
    timeout_ms: u64,
) -> Result<Option<HolderAnalysis>, Box<dyn std::error::Error + Send + Sync>> {
    let mint = *mint;
    let duration = Duration::from_millis(timeout_ms);

    let result = timeout(duration, async move {
        fetch_holder_concentration(&mint).await
    })
    .await;

    match result {
        Ok(inner) => inner.map(Some),
        Err(_elapsed) => {
            eprintln!(
                "[HOLDER_ANALYZER] Timeout after {}ms for mint {}",
                timeout_ms, mint
            );
            Ok(None) // timeout → treat as warning, không fail
        }
    }
}

async fn fetch_holder_concentration(
    mint: &Pubkey,
) -> Result<HolderAnalysis, Box<dyn std::error::Error + Send + Sync>> {
    use crate::constants::addresses::{PUMPSWAP_PROGRAM_ID, PUMPFUN_PROGRAM_ID};

    // Lấy top 20 largest token accounts (lấy dư để chính xác hơn)
    let largest_accounts = RPC_CLIENT
        .get_token_largest_accounts(mint)
        .await
        .map_err(|e| format!("get_token_largest_accounts failed: {e}"))?;

    if largest_accounts.is_empty() {
        return Err("No token accounts found".into());
    }

    // Lấy total supply
    let supply_response = RPC_CLIENT
        .get_token_supply(mint)
        .await
        .map_err(|e| format!("get_token_supply failed: {e}"))?;

    let total_supply_ui = supply_response.ui_amount.unwrap_or(0.0);
    if total_supply_ui <= 0.0 {
        return Err("Total supply is zero".into());
    }

    // Tạo danh sách LP pool PDA addresses cần loại bỏ
    let mut excluded_addresses: Vec<Pubkey> = Vec::new();

    // PumpSwap pool PDA (seed: "pool" + mint + WSOL)
    let (pda1, _) = Pubkey::find_program_address(
        &[b"pool", mint.as_ref(), crate::constants::addresses::WSOL.as_ref()],
        &PUMPSWAP_PROGRAM_ID,
    );
    excluded_addresses.push(pda1);

    // Reverse order pool PDA (seed: "pool" + WSOL + mint)
    let (pda2, _) = Pubkey::find_program_address(
        &[b"pool", crate::constants::addresses::WSOL.as_ref(), mint.as_ref()],
        &PUMPSWAP_PROGRAM_ID,
    );
    excluded_addresses.push(pda2);

    // PumpFun bonding curve PDA
    let (pda3, _) = Pubkey::find_program_address(
        &[b"bonding-curve", mint.as_ref()],
        &PUMPFUN_PROGRAM_ID,
    );
    excluded_addresses.push(pda3);

    // Lọc bỏ LP pool accounts, chỉ giữ ví thật
    let filtered_accounts: Vec<_> = largest_accounts
        .iter()
        .filter(|acc| {
            if let Ok(pubkey) = acc.address.parse::<Pubkey>() {
                // Loại account nếu address là PDA đã biết
                !excluded_addresses.contains(&pubkey)
            } else {
                true
            }
        })
        .collect();

    // Tính top 10 holders (hoặc ít hơn nếu không đủ)
    let top_n = filtered_accounts.len().min(10);
    let top10_sum: f64 = filtered_accounts[..top_n]
        .iter()
        .map(|acc| acc.amount.ui_amount.unwrap_or(0.0))
        .sum();

    let top10_pct = (top10_sum / total_supply_ui) * 100.0;

    Ok(HolderAnalysis {
        top10_holder_pct: top10_pct,
        holder_count: filtered_accounts.len(),
    })
}

/// Kiểm tra holder concentration theo config.
///
/// Returns `Err(reason)` nếu fail filter.
pub async fn check_holder_concentration(
    mint: &Pubkey,
    max_top10_pct: f64,
    timeout_ms: u64,
) -> Result<Option<f64>, String> {
    match analyze_holders(mint, timeout_ms).await {
        Ok(Some(analysis)) => {
            let pct = analysis.top10_holder_pct;
            if pct > max_top10_pct {
                Err(format!(
                    "Top 10 holders own {:.1}% supply (max allowed: {:.1}%)",
                    pct, max_top10_pct
                ))
            } else {
                Ok(Some(pct))
            }
        }
        Ok(None) => {
            // Timeout — không fail, trả None
            Ok(None)
        }
        Err(e) => {
            eprintln!("[HOLDER_ANALYZER] RPC error: {e}");
            // RPC error → không fail (tránh false positive khi node chậm)
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concentration_threshold() {
        // Test logic tính % concentration
        let total = 1_000_000.0_f64;
        let top10_sum = 320_000.0_f64;
        let pct = (top10_sum / total) * 100.0;
        assert!((pct - 32.0).abs() < 0.01);
        // 32% > 30% → nên fail
        assert!(pct > 30.0);
    }
}
