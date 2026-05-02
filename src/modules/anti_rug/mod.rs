//! Anti-Rug Intelligence Layer
//!
//! Module tổng hợp tất cả các thành phần chống rug-pull cho Migration Sniper Bot.
//!
//! # Sub-modules
//! - `config`          — Cấu hình (`AntiRugConfig`)
//! - `filter_result`   — Types cho kết quả filter (`FilterVerdict`, `AntiRugFilterResult`)
//! - `holder_analyzer` — [M1] Kiểm tra holder concentration
//! - `dev_wallet_profiler` — [M3] Kiểm tra lịch sử ví dev
//! - `genesis_detector` — [M4] Phát hiện genesis bundle buy
//! - `metadata_checker` — [M5] Kiểm tra Metaplex metadata
//! - `panic_sell`      — [M2] Monitor + Jito panic-sell
//! - `pre_buy_filter`  — Orchestrator tổng hợp tất cả filters

pub mod config;
pub mod filter_result;
pub mod holder_analyzer;
pub mod dev_wallet_profiler;
pub mod genesis_detector;
pub mod metadata_checker;
pub mod panic_sell;
pub mod pre_buy_filter;

pub use config::{AntiRugConfig, MetadataAction};
pub use filter_result::{AntiRugFilterResult, FilterVerdict};
pub use pre_buy_filter::evaluate_token;
pub use panic_sell::{PanicSellContext, PanicSellMonitorHandle, start_panic_sell_monitor, store_panic_sell_handle, cancel_panic_sell_monitor, PANIC_SELL_HANDLES};
