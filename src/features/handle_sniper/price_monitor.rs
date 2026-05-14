//! Price Monitor — Background timer loop
//!
//! Mục đích: Poll giá token từ pool reserves qua RPC mỗi POLL_INTERVAL_MS,
//! cập nhật token_price trong TOKEN_DB, và trực tiếp gửi lệnh bán khi SL/TP trigger.
//!
//! FIX CRITICAL BUG: execute_trade() chỉ đọc từ gRPC event DashMap, KHÔNG đọc TOKEN_DB.
//! Nếu price_monitor chỉ update state mà không gọi sell → lệnh bán KHÔNG BAO GIỜ được gửi.

use crate::*;
use solana_sdk::signer::Signer;
use tokio::time::{Duration, sleep};


/// Interval giữa mỗi lần poll giá (ms). Mặc định 5 giây.
const POLL_INTERVAL_MS: u64 = 5_000;

/// Background task: poll giá tất cả token đang held mỗi POLL_INTERVAL_MS.
/// Nếu phát hiện SL/TP → GỌI TRỰC TIẾP execute_pumpswap_sell().
pub async fn run_price_monitor_loop() {
    info!("[PRICE_MONITOR] 🚀 Started — polling every {}ms", POLL_INTERVAL_MS);

    loop {
        sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;

        if !is_running() {
            continue;
        }

        // Lấy danh sách token đang held
        let held_tokens: Vec<TokenDatabaseSchema> = TOKEN_DB
            .list_all()
            .unwrap_or_default()
            .into_iter()
            .map(|(_, td)| td)
            .filter(|td| {
                td.token_balance > 0
                    && td.token_is_purchased
                    && td.token_sell_status == TokenSellStatus::None
            })
            .collect();

        if held_tokens.is_empty() {
            continue;
        }

        let run_state = BOT_RUN_STATE.read().await.clone();
        let keypair = match run_state.selected_wallet_keypair() {
            Ok(Some(kp)) => kp,
            _ => continue,
        };
        let signer_pubkey = keypair.pubkey();

        for token_data in held_tokens {
            let mint = token_data.token_mint;
            let pool_quote_ata = token_data.pumpswap_ix_accounts.pool_quote_token_account;
            let pool_base_ata = token_data.pumpswap_ix_accounts.pool_base_token_account;

            let quote_result = RPC_CLIENT.get_token_account_balance(&pool_quote_ata).await;
            let base_result = RPC_CLIENT.get_token_account_balance(&pool_base_ata).await;

            match (quote_result, base_result) {
                (Ok(quote_balance), Ok(base_balance)) => {
                    let quote_amount = quote_balance.amount.parse::<f64>().unwrap_or(0.0) / 1e9;
                    let base_amount = base_balance.amount.parse::<f64>().unwrap_or(0.0) / 1e6;

                    if base_amount <= 0.0 {
                        continue;
                    }

                    let new_price = quote_amount / base_amount;

                    if let Some(mut db_entry) = TOKEN_DB.get(mint).ok().flatten() {
                        let old_price = db_entry.token_price;
                        db_entry.token_price = new_price;
                        db_entry.token_peak_price = db_entry.token_peak_price.max(new_price);

                        // Kiểm tra SL/TP với giá mới
                        db_entry.update_sell_state_flag(format!("price_poll_{}", mint));

                        let should_sell = (db_entry.tp_state == TPMode::Tp
                            || db_entry.tp_state == TPMode::SL
                            || db_entry.ts_state == TSMode::TrailingStopTriggered)
                            && db_entry.token_sell_status == TokenSellStatus::None;

                        info!(
                            "[PRICE_MONITOR] Mint: {} | Old: {:.8} | New: {:.8} | SellTrigger: {}",
                            mint, old_price, new_price, should_sell
                        );

                        if should_sell {
                            info!(
                                "[PRICE_MONITOR] 🔔 SL/TP triggered for {} — sending sell TX directly",
                                mint
                            );

                            // Cancel panic-sell monitor để tránh double-sell
                            crate::modules::anti_rug::panic_sell::cancel_panic_sell_monitor(&mint);

                            // Đánh dấu trước khi gửi TX để tránh gửi 2 lần
                            db_entry.token_sell_status = TokenSellStatus::SellTradeSubmitted;
                            let _ = TOKEN_DB.upsert(mint, db_entry.clone());

                            // Telegram alert
                            let mint_str = mint.to_string();
                            if db_entry.tp_state == TPMode::Tp {
                                crate::modules::telegram_ui::alert_sender::alert_take_profit(&mint_str);
                            } else if db_entry.tp_state == TPMode::SL {
                                crate::modules::telegram_ui::alert_sender::alert_stop_loss(&mint_str);
                            } else {
                                crate::modules::telegram_ui::alert_sender::alert_trailing_stop(&mint_str);
                            }

                            // Gửi lệnh bán trực tiếp
                            execute_pumpswap_sell(
                                &db_entry.pumpswap_ix_accounts,
                                &keypair,
                                &signer_pubkey,
                                db_entry.token_balance,
                                db_entry.token_creator,
                                db_entry.is_cashback_coin,
                                new_price,
                                run_state.trading.slippage,
                            );
                        } else {
                            let _ = TOKEN_DB.upsert(mint, db_entry);
                        }
                    }
                }
                (Err(e), _) | (_, Err(e)) => {
                    info!("[PRICE_MONITOR] ⚠️ RPC error for {}: {}", mint, e);
                }
            }
        }
    }
}
