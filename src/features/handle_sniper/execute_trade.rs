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

    for entry in trade_data.iter() {
        let token_data = entry.value();

        match token_data.sniper_trade_state {
            SniperTradeStatus::Migrated => {
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
            }
            _ => {
                let should_sell = (token_data.tp_state == TPMode::Tp
                    || token_data.tp_state == TPMode::SL
                    || token_data.ts_state == TSMode::TrailingStopTriggered)
                    && token_data.token_balance > 0
                    && token_data.token_sell_status == TokenSellStatus::None;

                if should_sell {
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
            }
        }
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
