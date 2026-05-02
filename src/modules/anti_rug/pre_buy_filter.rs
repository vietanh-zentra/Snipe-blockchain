//! Pre-Buy Filter Orchestrator.
//!
//! Chạy tất cả Anti-Rug filter modules song song và tổng hợp kết quả
//! thành một `AntiRugFilterResult` duy nhất. Đây là entry point duy nhất
//! được gọi từ `execute_trade.rs`.

use crate::RPC_CLIENT;
use solana_sdk::pubkey::Pubkey;
use std::time::Instant;
use tokio::join;

use super::{
    config::{AntiRugConfig, MetadataAction},
    dev_wallet_profiler::check_dev_wallet,
    filter_result::{AntiRugFilterResult, FilterVerdict},
    holder_analyzer::check_holder_concentration,
    metadata_checker::check_metadata,
    genesis_detector::check_genesis_bundles,
};

/// Đánh giá token trước khi mua.
///
/// Chạy tất cả enabled filters song song với `tokio::join!`.
/// Không có filter nào block lẫn nhau — độ trễ tổng thể = max(filter latencies).
pub async fn evaluate_token(
    mint: &Pubkey,
    dev: &Pubkey,
    creation_slot: Option<u64>,
    config: &AntiRugConfig,
) -> AntiRugFilterResult {
    let start = Instant::now();

    if !config.enabled {
        return AntiRugFilterResult::disabled_pass();
    }

    // Chạy song song tất cả enabled filters
    let (holder_result, dev_result, meta_result) = join!(
        // Module 1: Holder Concentration
        async {
            if config.holder_filter_enabled {
                check_holder_concentration(
                    mint,
                    config.max_top10_holder_pct,
                    config.filter_timeout_ms,
                )
                .await
            } else {
                Ok(None)
            }
        },
        // Module 3: Dev Wallet Profiler
        async {
            if config.dev_profiler_enabled {
                check_dev_wallet(dev, config.min_dev_tx_count, config.min_wallet_age_hours, config.block_cex_funded, config.filter_timeout_ms).await
            } else {
                Ok(None)
            }
        },
        // Module 5: Metadata Checker — Fix #10: trả về chi tiết thay vì bool
        async {
            if config.metadata_checker_enabled {
                check_metadata(mint, config.filter_timeout_ms).await.ok().flatten()
            } else {
                None
            }
        },
    );

    // Module 4: Genesis Detector (chạy riêng vì cần tham số extra)
    let genesis_result = if config.genesis_detector_enabled {
        if let Some(slot) = creation_slot {
            // Lấy total supply để tính %
            let total_supply = get_total_supply(mint).await.unwrap_or(0.0);
            check_genesis_bundles(
                mint,
                slot,
                total_supply,
                config.max_genesis_buy_pct,
                config.max_clustered_wallets,
                config.filter_timeout_ms,
            )
            .await
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    };

    let elapsed_ms = start.elapsed().as_millis() as u64;

    // Tổng hợp results và build verdict
    build_verdict(
        holder_result,
        dev_result,
        genesis_result,
        meta_result,
        &config.metadata_empty_action,
        config.require_metadata_uri,
        elapsed_ms,
    )
}

fn build_verdict(
    holder_result: Result<Option<f64>, String>,
    dev_result: Result<Option<u64>, String>,
    genesis_result: Result<Option<f64>, String>,
    meta_result: Option<super::metadata_checker::MetadataCheckResult>,
    metadata_action: &MetadataAction,
    require_metadata_uri: bool,
    duration_ms: u64,
) -> AntiRugFilterResult {
    let mut fail_reasons: Vec<String> = Vec::new();
    let mut warn_reasons: Vec<String> = Vec::new();

    // Module 1: Holder Concentration
    let top10_pct = match holder_result {
        Err(reason) => {
            fail_reasons.push(format!("[M1-Holder] {}", reason));
            None
        }
        Ok(pct) => pct,
    };

    // Module 3: Dev Wallet Profiler
    let dev_tx_count = match dev_result {
        Err(reason) => {
            fail_reasons.push(format!("[M3-Dev] {}", reason));
            None
        }
        Ok(count) => count,
    };

    // Module 4: Genesis Detector
    let genesis_buy_pct = match genesis_result {
        Err(reason) => {
            fail_reasons.push(format!("[M4-Genesis] {}", reason));
            None
        }
        Ok(pct) => pct,
    };

    // Module 5: Metadata Checker — Brief V.M5 L405: metadata_empty_action
    let (has_metadata, metadata_uri, token_name) = match &meta_result {
        Some(m) => (
            m.has_uri,
            m.uri.clone(),
            m.name.clone(),
        ),
        None => (false, None, None),
    };
    if !has_metadata {
        // Brief L404: require_metadata_uri = false → skip metadata action entirely
        let effective_action = if !require_metadata_uri {
            &MetadataAction::Allow
        } else {
            metadata_action
        };
        match effective_action {
            MetadataAction::Skip => {
                fail_reasons.push("[M5-Metadata] Token has no metadata URI (action: skip)".to_string());
            }
            MetadataAction::Warn => {
                warn_reasons.push("[M5-Metadata] Token has no metadata URI".to_string());
            }
            MetadataAction::Allow => {} // Do nothing
        }
    }
    // Brief L358: Tổng hợp risk score (0–100)
    let mut risk_score: u32 = 0;
    // Fail reasons = 25 points each (max 75)
    risk_score += (fail_reasons.len() as u32).min(3) * 25;
    // Warn reasons = 10 points each (max 20)
    risk_score += (warn_reasons.len() as u32).min(2) * 10;
    // Holder concentration: 0-30% = 0pts, 30-50% = +10, 50%+ = +20
    if let Some(pct) = top10_pct {
        if pct > 50.0 { risk_score += 20; }
        else if pct > 30.0 { risk_score += 10; }
    }
    // No metadata = +5
    if !has_metadata { risk_score += 5; }
    // Genesis bundle = +15
    if genesis_buy_pct.map(|p| p > 30.0).unwrap_or(false) { risk_score += 15; }
    risk_score = risk_score.min(100);

    // Build final verdict
    let verdict = if !fail_reasons.is_empty() {
        let combined = fail_reasons.join("; ");
        FilterVerdict::Fail(combined)
    } else if !warn_reasons.is_empty() {
        let combined = warn_reasons.join("; ");
        FilterVerdict::Warn(combined)
    } else {
        FilterVerdict::Pass
    };

    AntiRugFilterResult {
        verdict,
        top10_holder_pct: top10_pct,
        dev_tx_count,
        genesis_buy_pct,
        genesis_bundle_detected: genesis_buy_pct.map(|p| p > 30.0).unwrap_or(false),
        has_metadata_uri: has_metadata,
        metadata_uri,
        token_name,
        filter_duration_ms: duration_ms,
        risk_score,
    }
}

async fn get_total_supply(mint: &Pubkey) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
    let supply = RPC_CLIENT
        .get_token_supply(mint)
        .await
        .map_err(|e| format!("get_token_supply failed: {e}"))?;
    Ok(supply.ui_amount.unwrap_or(0.0))
}
