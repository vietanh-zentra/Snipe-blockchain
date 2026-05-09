use crate::*;
use colored::*;

////Pumpswap trade data handler
pub fn update_status_from_pumpswap_buy_event(
    mut token_data: TokenDatabaseSchema,
    buy_event: PumpswapBuyEvent,
    buy_accounts: PumpswapBuyInstructionAccounts,
    tx_id: String,
) -> TokenDatabaseSchema {
    // P11 fix: Guard against division-by-zero (would produce Inf/NaN, breaking TP/SL logic)
    let base_reserves = buy_event.pool_base_token_reserves as f64 / 10f64.powi(6);
    let updated_token_price = if base_reserves > 0.0 {
        (buy_event.pool_quote_token_reserves as f64 / 10f64.powi(9)) / base_reserves
    } else {
        token_data.token_price // Keep previous price if reserves are 0
    };

    token_data.token_peak_price = token_data.token_peak_price.max(updated_token_price);

    token_data.token_creator = buy_event.coin_creator;
    token_data.token_price = updated_token_price;

    token_data.update_sell_state_flag(tx_id.clone());

    if get_signer_pubkey().map_or(false, |p| buy_event.user == p) {
        info!(
            "[My tx]\t[{}]\t*Hash: {}\t*mint: {}",
            "Buy".green(),
            tx_id,
            buy_accounts.base_mint.to_string()
        );

        enqueue_bot_buy_alert(
            &buy_accounts.base_mint.to_string(),
            buy_event.user,
            &tx_id,
            buy_event.quote_amount_in,
            buy_event.base_amount_out,
        );

        token_data.token_is_purchased = true;
        token_data.token_buying_point_price = updated_token_price;
        token_data.token_balance += buy_event.base_amount_out;
    }

    let _ = TOKEN_DB.upsert(token_data.token_mint.clone(), token_data.clone());
    token_data
}

pub fn update_status_from_pumpswap_sell_event(
    mut token_data: TokenDatabaseSchema,
    sell_event: PumpswapSellEvent,
    sell_accounts: PumpswapSellInstructionAccounts,
    tx_id: String,
) -> Option<TokenDatabaseSchema> {
    // P11 fix: Guard against division-by-zero
    let base_reserves = sell_event.pool_base_token_reserves as f64 / 10f64.powi(6);
    let updated_token_price = if base_reserves > 0.0 {
        (sell_event.pool_quote_token_reserves as f64 / 10f64.powi(9)) / base_reserves
    } else {
        token_data.token_price // Keep previous price if reserves are 0
    };

    token_data.token_creator = sell_event.coin_creator;
    token_data.token_price = updated_token_price;

    token_data.update_sell_state_flag(tx_id.clone());

    if get_signer_pubkey().map_or(false, |p| sell_event.user == p) {
        info!(
            "[My Tx]\t[{}]\t*Hash: {}\t*mint: {}",
            "Sell".red(),
            tx_id,
            sell_accounts.base_mint.to_string()
        );

        enqueue_bot_sell_alert(
            &sell_accounts.base_mint.to_string(),
            sell_event.user,
            &tx_id,
            sell_event.quote_amount_out,
            sell_event.base_amount_in,
        );

        token_data.token_balance = token_data.token_balance.saturating_sub(sell_event.base_amount_in);

        if token_data.token_balance > 0 {
            let _ = TOKEN_DB.upsert(sell_accounts.base_mint.clone(), token_data.clone());
            Some(token_data.clone())
        } else {
            let _ = TOKEN_DB.delete(sell_accounts.base_mint.clone());
            None
        }
    } else {
        let _ = TOKEN_DB.upsert(sell_accounts.base_mint.clone(), token_data.clone());
        Some(token_data.clone())
    }
}
