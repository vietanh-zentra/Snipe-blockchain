use dashmap::DashMap;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;

use crate::*;


pub async fn execute_trade(trade_data: &DashMap<Pubkey, TokenDatabaseSchema>) {
    let run_state = BOT_RUN_STATE.read().await.clone();
    if !run_state.is_running {
        return;
    }
    let keypair = match run_state.selected_wallet_keypair() {
        Ok(Some(kp)) => kp,
        _ => return,
    };
    let signer_pubkey = keypair.pubkey();

    // ── Bước 1: Thu thập dữ liệu từ DashMap vào Vec ────────────────────
    // QUAN TRỌNG: Không gọi .await bên trong vòng lặp DashMap.iter()
    // vì DashMap::Ref không implement Send → gây compile error hoặc deadlock.
    let mut tokens_to_buy: Vec<TokenDatabaseSchema> = Vec::new();
    let mut tokens_to_sell: Vec<TokenDatabaseSchema> = Vec::new();

    for entry in trade_data.iter() {
        let token_data = entry.value();
        match token_data.sniper_trade_state {
            SniperTradeStatus::Migrated => {
                tokens_to_buy.push(token_data.clone());
            }
            _ => {
                let should_sell = (token_data.tp_state == TPMode::Tp
                    || token_data.tp_state == TPMode::SL
                    || token_data.ts_state == TSMode::TrailingStopTriggered)
                    && token_data.token_balance > 0
                    && token_data.token_sell_status == TokenSellStatus::None;

                if should_sell {
                    tokens_to_sell.push(token_data.clone());
                }
            }
        }
    }
    // DashMap read guards đã được drop tại đây — an toàn để .await

    // ── Bước 2: Xử lý các token cần BÁN (không cần Anti-Rug filter) ────
    for token_data in &tokens_to_sell {
        // Fix BUG-1: Cancel panic-sell monitor khi bán token (TP/SL)
        crate::modules::anti_rug::panic_sell::cancel_panic_sell_monitor(
            &token_data.token_mint,
        );

        execute_pumpswap_sell(
            &token_data.pumpswap_ix_accounts,
            &keypair,
            &signer_pubkey,
            token_data.token_balance,
            token_data.token_creator,
            token_data.is_cashback_coin,
            token_data.token_price,
            run_state.trading.slippage,
        );

        // Send sell notification based on reason
        let mint_str = token_data.token_mint.to_string();
        if token_data.tp_state == TPMode::Tp {
            crate::modules::telegram_ui::alert_sender::alert_take_profit(&mint_str);
        } else if token_data.tp_state == TPMode::SL {
            crate::modules::telegram_ui::alert_sender::alert_stop_loss(&mint_str);
        } else if token_data.ts_state == TSMode::TrailingStopTriggered {
            crate::modules::telegram_ui::alert_sender::alert_trailing_stop(&mint_str);
        }

        if let Some(mut db_entry) = TOKEN_DB.get(token_data.token_mint).ok().flatten() {
            db_entry.token_sell_status = TokenSellStatus::SellTradeSubmitted;
            let _ = TOKEN_DB.upsert(token_data.token_mint, db_entry);
        }
    }

    // ── Bước 3: Xử lý các token cần MUA (chạy Anti-Rug filter trước) ───
    for token_data in &tokens_to_buy {
        // ── Anti-Rug Pre-Buy Filter ──────────────────────────────────
        let anti_rug_cfg = run_state.anti_rug.clone();
        if anti_rug_cfg.enabled {
            let mint = token_data.token_mint;
            let dev  = token_data.token_creator;

            // Fix #5: Lấy creation slot từ RPC (transaction signature gần nhất của mint)
            let creation_slot = match RPC_CLIENT
                .get_signatures_for_address(&mint)
                .await
            {
                Ok(sigs) if !sigs.is_empty() => sigs.last().and_then(|s| Some(s.slot)),
                _ => None,
            };

            let filter_result = evaluate_token(&mint, &dev, creation_slot, &anti_rug_cfg).await;

            // Log kết quả
            let mint_str = mint.to_string();
            let verdict_str = filter_result.verdict.as_db_str();
            let reject_reason = filter_result.verdict.reason().map(|s| s.to_string());
            info!(
                "[ANTI-RUG] Mint: {} | Verdict: {} | Risk: {}/100 | Top10: {:?}% | Dev TX: {:?} | Genesis: {:?}% | Meta: {} | Duration: {}ms",
                mint_str, verdict_str,
                filter_result.risk_score,
                filter_result.top10_holder_pct,
                filter_result.dev_tx_count,
                filter_result.genesis_buy_pct,
                filter_result.has_metadata_uri,
                filter_result.filter_duration_ms,
            );

            // Fix #1 + BUG-2: Log kết quả filter vào PostgreSQL (fire-and-forget, shared pool)
            let db_mint = mint_str.clone();
            let db_verdict = verdict_str.to_string();
            let db_reason = reject_reason.clone();
            let db_top10 = filter_result.top10_holder_pct;
            let db_dev_tx = filter_result.dev_tx_count;
            let db_genesis = filter_result.genesis_buy_pct;
            let db_genesis_bundle = filter_result.genesis_bundle_detected;
            let db_meta = filter_result.has_metadata_uri;
            let db_duration = filter_result.filter_duration_ms;
            tokio::spawn(async move {
                if let Ok(db) = crate::modules::postgresql::db::get_shared_db().await {
                    let _ = crate::modules::postgresql::db::log_anti_rug_filter_result(
                        db,
                        &db_mint,
                        &db_verdict,
                        db_reason,
                        db_top10,
                        db_dev_tx,
                        db_genesis,
                        db_genesis_bundle,
                        db_meta,
                        db_duration,
                    )
                    .await;
                }
            });

            // If FAIL and not warn_only → skip, don't buy
            if filter_result.verdict.is_fail() && !anti_rug_cfg.warn_only {
                let reason_str = reject_reason.as_deref().unwrap_or("unknown reason");
                info!(
                    "[ANTI-RUG] ❌ SKIP {} — {}",
                    mint_str, reason_str
                );
                // Fix #2: Send Telegram alert when skipping token
                crate::modules::telegram_ui::alert_sender::alert_token_filtered(
                    &mint_str, reason_str,
                );
                // Log skipped token to PostgreSQL for customer analysis
                {
                    let mint_for_db = mint_str.clone();
                    let reason_for_db = reason_str.to_string();
                    tokio::spawn(async move {
                        if let Err(e) = crate::modules::postgresql::db::log_skipped_token(
                            &mint_for_db, &reason_for_db,
                        ).await {
                            eprintln!("[SKIPPED_LOG] DB insert error: {e}");
                        }
                    });
                }
                // Mark in TOKEN_DB to avoid reprocessing
                if let Some(mut db_entry) = TOKEN_DB.get(mint).ok().flatten() {
                    db_entry.sniper_trade_state = SniperTradeStatus::RugDetected;
                    let _ = TOKEN_DB.upsert(mint, db_entry);
                }
                continue; // Skip this token
            }

            // If FAIL but warn_only → warn but still buy
            if filter_result.verdict.is_fail() && anti_rug_cfg.warn_only {
                let reason_str = reject_reason.as_deref().unwrap_or("unknown reason");
                info!(
                    "[ANTI-RUG] ⚠️ WARN {} — {} (buying anyway)",
                    mint_str, reason_str
                );
                let warn_msg = format!(
                    "⚠️ *Anti-Rug Warning*\n\n\
                    Token: `{}`\n\
                    Risk: {}/100\n\
                    Reason: {}\n\n\
                    _Buying anyway (Warn-Only mode)_",
                    mint_str,
                    filter_result.risk_score,
                    reason_str
                );
                crate::modules::telegram_ui::alert_sender::send_telegram_alert(warn_msg);
            }
        }
        // ── Kết thúc Anti-Rug Filter ─────────────────────────────────

        // Token đã pass filter → thực hiện MUA
        // Panic sell context is passed in so monitor only starts after buy is confirmed on-chain
        let anti_rug_cfg = run_state.anti_rug.clone();
        let panic_sell_enabled = anti_rug_cfg.enabled && anti_rug_cfg.panic_sell_enabled;

        // Pre-build watched wallets list (needs async RPC calls)
        let mut watched_wallets: Vec<Pubkey> = Vec::new();
        if panic_sell_enabled {
            watched_wallets.push(token_data.token_creator); // Dev wallet
            let top_n = anti_rug_cfg.panic_sell_watch_top_holders as usize;
            if top_n > 0 {
                if let Ok(largest) = RPC_CLIENT
                    .get_token_largest_accounts(&token_data.token_mint)
                    .await
                {
                    for acc in largest.iter().take(top_n) {
                        if let Ok(token_acc_pubkey) = acc.address.parse::<Pubkey>() {
                            if let Ok(account) = RPC_CLIENT.get_account(&token_acc_pubkey).await {
                                if account.data.len() >= 64 {
                                    let owner = Pubkey::try_from(&account.data[32..64])
                                        .unwrap_or_default();
                                    if owner != token_data.token_creator
                                        && owner != signer_pubkey
                                        && owner != Pubkey::default()
                                    {
                                        watched_wallets.push(owner);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        execute_pumpswap_buy(
            &token_data.pumpswap_ix_accounts,
            &keypair,
            &signer_pubkey,
            run_state.trading.buy_amount_sol,
            token_data.token_price,
            token_data.token_creator,
            token_data.is_cashback_coin,
            run_state.trading.slippage,
            // Panic sell context — only used after buy confirmation
            panic_sell_enabled,
            token_data.token_mint,
            token_data.token_balance,
            anti_rug_cfg.panic_sell_jito_tip_lamports,
            watched_wallets,
        );
        // Buy notification + panic sell monitor are now handled inside execute_pumpswap_buy

        if let Some(mut db_entry) = TOKEN_DB.get(token_data.token_mint).ok().flatten() {
            db_entry.sniper_trade_state = SniperTradeStatus::BuySubmitted;
            let _ = TOKEN_DB.upsert(token_data.token_mint, db_entry);
        }
    }
}

/// Builds create-ATA + wrap SOL + buy instructions and spawns confirmation.
/// buy_amount_sol is the SOL amount to spend (e.g. 0.1); token_price_sol is current pool price.
/// Panic sell monitor only starts AFTER buy is confirmed on-chain.
pub fn execute_pumpswap_buy(
    pumpswap_struct: &PumpSwapStruct,
    keypair: &Keypair,
    signer_pubkey: &Pubkey,
    buy_amount_sol: f64,
    token_price_sol: f64,
    token_creator: Pubkey,
    is_cashback_enabled: bool,
    slippage: f64,
    // Panic sell context
    panic_sell_enabled: bool,
    token_mint: Pubkey,
    token_balance: u64,
    panic_sell_jito_tip: u64,
    watched_wallets: Vec<Pubkey>,
) {
    let mut ps = *pumpswap_struct;

    let mut ix: Vec<Instruction> = Vec::new();
    let create_ix: Vec<Instruction> = ps.get_create_ata_idempotent_ix(signer_pubkey);
    let wsol_ix = ps.get_wsol_ix(signer_pubkey, buy_amount_sol, slippage);
    let buy_ix: Instruction = ps.get_buy_ix(
        signer_pubkey,
        token_creator,
        is_cashback_enabled,
        token_price_sol,
        buy_amount_sol,
        slippage,
    );
    let close_ix: Instruction = ps.close_wsol_ata(signer_pubkey);

    ix.extend(create_ix);
    ix.extend(wsol_ix);
    ix.push(buy_ix);
    ix.push(close_ix);

    let keypair_owned = keypair.insecure_clone();
    let mint_str = ps.base_mint.to_string();
    let price = token_price_sol;
    let ps_copy = ps; // Copy for panic sell context
    tokio::spawn(async move {
        match send_0slot_transaction(ix, keypair_owned.insecure_clone()).await {
            Ok(Some(hash)) => {
                // Wait and confirm on-chain
                use solana_sdk::signature::Signature;
                use std::str::FromStr;
                if let Ok(sig) = Signature::from_str(&hash) {
                    // Poll RPC for confirmation (max 15 seconds)
                    let mut confirmed = false;
                    for _ in 0..15 {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        if let Ok(status) = RPC_CLIENT.get_signature_status(&sig).await {
                            if let Some(result) = status {
                                if result.is_ok() {
                                    confirmed = true;
                                    break;
                                } else {
                                    // Transaction failed on-chain
                                    info!("[❌ BUY FAILED] Mint: {} | Hash: {}", mint_str, hash);
                                    crate::modules::telegram_ui::alert_sender::alert_buy_failed(&mint_str, &hash);
                                    return;
                                }
                            }
                        }
                    }
                    if confirmed {
                        info!("[✅ BUY SUCCESS] Mint: {} | Hash: {}", mint_str, hash);
                        crate::modules::telegram_ui::alert_sender::alert_buy_confirmed(&mint_str, price, &hash);

                        // P2 fix: Fetch actual token balance from TOKEN_DB
                        // (gRPC event handler updates it after buy confirms)
                        let actual_balance = crate::TOKEN_DB
                            .get(token_mint)
                            .ok()
                            .flatten()
                            .map(|td| td.token_balance)
                            .unwrap_or(0);

                        if panic_sell_enabled && !watched_wallets.is_empty() && actual_balance > 0 {
                            use crate::modules::anti_rug::panic_sell::{PanicSellContext, start_panic_sell_monitor};

                            info!(
                                "[PANIC_SELL] 🔍 Watching {} wallets for mint {}",
                                watched_wallets.len(), token_mint
                            );

                            let ctx = PanicSellContext {
                                token_mint,
                                pumpswap_accounts: ps_copy,
                                keypair: keypair_owned.insecure_clone(),
                                token_balance: actual_balance, // P2 fix: use live balance, not stale
                                token_creator,
                                is_cashback_coin: is_cashback_enabled,
                                jito_tip_lamports: panic_sell_jito_tip,
                                watched_wallets,
                            };

                            let handle = start_panic_sell_monitor(ctx);
                            crate::modules::anti_rug::panic_sell::store_panic_sell_handle(
                                token_mint, handle,
                            );
                            info!(
                                "[PANIC_SELL] 🔍 Monitor started for mint {}",
                                token_mint
                            );
                        }
                    } else {
                        info!("[⏳ BUY UNCONFIRMED] Mint: {} | Hash: {}", mint_str, hash);
                        crate::modules::telegram_ui::alert_sender::alert_buy_success(&mint_str, price);
                    }
                }
            }
            Ok(None) => {
                info!("[❌ BUY REJECTED] Mint: {} — tx rejected by RPC", mint_str);
                crate::modules::telegram_ui::alert_sender::alert_buy_failed(&mint_str, "rejected");
            }
            Err(e) => {
                info!("[❌ BUY ERROR] Mint: {} — {}", mint_str, e);
                crate::modules::telegram_ui::alert_sender::alert_buy_failed(&mint_str, &e.to_string());
            }
        }
    });
}

/// Builds sell instruction and spawns confirmation.
pub fn execute_pumpswap_sell(
    pumpswap_struct: &PumpSwapStruct,
    keypair: &Keypair,
    signer_pubkey: &Pubkey,
    sell_amount: u64,
    token_creator: Pubkey,
    is_cashback_enabled: bool,
    token_price: f64,
    slippage: f64,
) {
    let mut ps = *pumpswap_struct;
    let mut ix: Vec<Instruction> = Vec::new();
    let create_ix: Vec<Instruction> = ps.get_create_ata_idempotent_ix(signer_pubkey);
    // P6 fix: Calculate min SOL output with slippage protection
    // sell_amount = base units (6 decimals), token_price = SOL per token
    let expected_sol_lamports = (sell_amount as f64 / 1e6) * token_price * 1e9;
    let min_sol_out = (expected_sol_lamports * (100.0 - slippage) / 100.0).max(1.0) as u64;
    let sell_ix: Instruction = ps.get_sell_ix(
        signer_pubkey,
        sell_amount,
        token_creator,
        is_cashback_enabled,
        min_sol_out,
    );
    let close_ix: Instruction = ps.close_wsol_ata(signer_pubkey);

    ix.extend(create_ix);
    ix.push(sell_ix);
    ix.push(close_ix);

    let keypair_owned = keypair.insecure_clone();
    tokio::spawn(async move {
        let _ = send_0slot_transaction(ix, keypair_owned).await;
    });
}
