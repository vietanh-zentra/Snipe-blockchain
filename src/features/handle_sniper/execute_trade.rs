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
        );
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
                "[ANTI-RUG] Mint: {} | Verdict: {} | Top10: {:?}% | Dev TX: {:?} | Genesis: {:?}% | Meta: {} | Duration: {}ms",
                mint_str, verdict_str,
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

            // Nếu FAIL và không phải warn_only → bỏ qua, không mua
            if filter_result.verdict.is_fail() && !anti_rug_cfg.warn_only {
                let reason_str = reject_reason.as_deref().unwrap_or("unknown reason");
                info!(
                    "[ANTI-RUG] ❌ SKIP {} — {}",
                    mint_str, reason_str
                );
                // Fix #2: Gửi Telegram alert khi skip token
                crate::modules::telegram_ui::alert_sender::alert_token_filtered(
                    &mint_str, reason_str,
                );
                // Đánh dấu trong TOKEN_DB để tránh process lại
                if let Some(mut db_entry) = TOKEN_DB.get(mint).ok().flatten() {
                    db_entry.sniper_trade_state = SniperTradeStatus::RugDetected;
                    let _ = TOKEN_DB.upsert(mint, db_entry);
                }
                continue; // Bỏ qua token này, xử lý token tiếp theo
            }
        }
        // ── Kết thúc Anti-Rug Filter ─────────────────────────────────

        // Token đã pass filter → thực hiện MUA
        execute_pumpswap_buy(
            &token_data.pumpswap_ix_accounts,
            &keypair,
            &signer_pubkey,
            run_state.trading.buy_amount_sol,
            token_data.token_price,
            token_data.token_creator,
            token_data.is_cashback_coin,
            run_state.trading.slippage,
        );

        if let Some(mut db_entry) = TOKEN_DB.get(token_data.token_mint).ok().flatten() {
            db_entry.sniper_trade_state = SniperTradeStatus::BuySubmitted;
            let _ = TOKEN_DB.upsert(token_data.token_mint, db_entry);
        }

        // ── Module 2: Panic-Sell Monitor (post-buy) ──────────────────
        let anti_rug_cfg = run_state.anti_rug.clone();
        if anti_rug_cfg.panic_sell_enabled {
            use crate::modules::anti_rug::panic_sell::{PanicSellContext, start_panic_sell_monitor};

            let ctx = PanicSellContext {
                token_mint: token_data.token_mint,
                pumpswap_accounts: token_data.pumpswap_ix_accounts,
                keypair: keypair.insecure_clone(),
                token_balance: token_data.token_balance,
                token_creator: token_data.token_creator,
                is_cashback_coin: token_data.is_cashback_coin,
                jito_tip_lamports: anti_rug_cfg.panic_sell_jito_tip_lamports,
                watched_wallets: vec![token_data.token_creator],
            };

            let handle = start_panic_sell_monitor(ctx);
            // Fix BUG-1: Lưu handle vào global map để monitor không bị cancel
            // Handle sẽ bị remove + cancel khi TP/SL bán xong
            crate::modules::anti_rug::panic_sell::store_panic_sell_handle(
                token_data.token_mint, handle,
            );
            info!(
                "[PANIC_SELL] 🔍 Monitor started for mint {}",
                token_data.token_mint
            );
        }
        // ── End Module 2 ─────────────────────────────────────────────
    }
}

/// Builds create-ATA + wrap SOL + buy instructions and spawns confirmation.
/// buy_amount_sol is the SOL amount to spend (e.g. 0.1); token_price_sol is current pool price.
pub fn execute_pumpswap_buy(
    pumpswap_struct: &PumpSwapStruct,
    keypair: &Keypair,
    signer_pubkey: &Pubkey,
    buy_amount_sol: f64,
    token_price_sol: f64,
    token_creator: Pubkey,
    is_cashback_enabled: bool,
    slippage: f64,
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
    tokio::spawn(async move {
        let _ = send_0slot_transaction(ix, keypair_owned).await;
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
) {
    let mut ps = *pumpswap_struct;
    let mut ix: Vec<Instruction> = Vec::new();
    let create_ix: Vec<Instruction> = ps.get_create_ata_idempotent_ix(signer_pubkey);
    let sell_ix: Instruction = ps.get_sell_ix(
        signer_pubkey,
        sell_amount,
        token_creator,
        is_cashback_enabled,
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
