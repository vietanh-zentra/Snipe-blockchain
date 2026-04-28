use crate::*;
use solana_sdk::pubkey::Pubkey;

#[derive(Clone, Debug)]
pub struct TokenDatabaseSchema {
    pub token_mint: Pubkey,
    pub token_creator: Pubkey,
    pub is_cashback_coin: bool,
    pub token_price: f64,
    pub token_peak_price: f64,
    pub token_is_purchased: bool,
    pub token_balance: u64,
    pub token_buying_point_price: f64,
    pub sniper_trade_state: SniperTradeStatus,
    pub tp_state: TPMode,
    pub tracked_tp_state: TPMode,
    pub ts_state: TSMode,
    pub tracked_ts_state: TSMode,
    pub pumpswap_ix_accounts: PumpSwapStruct,
    pub token_sell_status: TokenSellStatus,
}

impl TokenDatabaseSchema {
    pub fn new_from_token_migration(
        migrate_accounts: MigrateInstructionAccounts,
        create_pool_accounts: CreatePoolInstructionAccounts,
        create_pool_event_data: CreatePoolEventData,
        create_pool_instruction_data: CreatePoolInstructionData,
    ) -> Self {
        let token_price = (create_pool_event_data.quote_amount_in as f64 / 10f64.powi(9))
            / (create_pool_event_data.base_amount_in as f64 / 10f64.powi(6));

        let token_data = Self {
            token_mint: create_pool_event_data.base_mint,
            token_creator: create_pool_event_data.coin_creator,
            is_cashback_coin: create_pool_instruction_data.is_cashback_coin,
            token_price: token_price,
            token_peak_price: token_price,
            token_is_purchased: false,
            token_balance: 0,
            token_buying_point_price: 0.0,
            sniper_trade_state: SniperTradeStatus::Migrated,
            tp_state: TPMode::None,
            tracked_tp_state: TPMode::None,
            ts_state: TSMode::None,
            tracked_ts_state: TSMode::None,
            pumpswap_ix_accounts: PumpSwapStruct::from_migrate(
                &migrate_accounts,
                &create_pool_accounts,
                &create_pool_event_data,
            ),
            token_sell_status: TokenSellStatus::None,
        };


        let _ = TOKEN_DB.upsert(create_pool_event_data.base_mint.clone(), token_data.clone());

        token_data
    }

    /// Single TP and SL from runtime (DB).
    pub fn update_sell_state_flag(&mut self, _tx_id: String) {
        if self.token_balance > 0 {
            let tp = get_take_profit();
            let sl = get_stop_loss();
            let trailing = get_trailing();
            let trailing_stop = get_trailing_stop();

            //TP + SL
            self.tp_state = if self.token_price > self.token_buying_point_price * tp
                && self.tp_state < TPMode::Tp
            {
                update!(
                    "[TP_UPDATED]\t*MINT: {}
                    \t*TP STATE: {:?} -> Tp,
                    \t*MC VARIANT: {} SOL (BUY) -> {} SOL (NOW)",
                    self.token_mint,
                    self.tp_state,
                    self.token_buying_point_price * TOKEN_TOTAL_SUPPLY as f64,
                    self.token_price * TOKEN_TOTAL_SUPPLY as f64,
                );
                enqueue_tp_triggered_alert(
                    &self.token_mint.to_string(),
                    self.token_buying_point_price,
                    self.token_price,
                    tp * 100.0,
                );
                TPMode::Tp
            } else if self.token_price < self.token_buying_point_price * sl
                && self.tp_state < TPMode::SL
            {
                update!(
                    "[TP_UPDATED]\t*MINT: {}
                    \t*TP STATE: {:?} -> SL,
                    \t*MC VARIANT: {} SOL (BUY) -> {} SOL (NOW)",
                    self.token_mint,
                    self.tp_state,
                    self.token_buying_point_price * TOKEN_TOTAL_SUPPLY as f64,
                    self.token_price * TOKEN_TOTAL_SUPPLY as f64,
                );
                enqueue_sl_triggered_alert(
                    &self.token_mint.to_string(),
                    self.token_buying_point_price,
                    self.token_price,
                    sl * 100.0,
                );
                TPMode::SL
            } else {
                self.tp_state
            };

            //Trailing Stop
            self.ts_state = if self.ts_state == TSMode::TrailingTriggered
                && self.token_price < self.token_peak_price * (1.0 - trailing_stop)
            {
                info!(
                    "[TS_UPDATED] => MINT : {}
                \t* TS STATE : {:?} -> {:?},
                \t* CURRENT HOLDING: {}",
                    self.token_mint,
                    self.ts_state,
                    TSMode::TrailingStopTriggered,
                    self.token_balance
                );
                enqueue_trailing_stop_triggered_alert(
                    &self.token_mint.to_string(),
                    self.token_peak_price,
                    self.token_price,
                    trailing_stop * 100.0,
                );
                TSMode::TrailingStopTriggered
            } else if self.token_price > self.token_buying_point_price * trailing
                && self.ts_state < TSMode::TrailingTriggered
            {
                info!(
                    "[TS_UPDATED] => MINT : {}
                \t* TS STATE : {:?} -> {:?},
                \t* CURRENT HOLDING: {}",
                    self.token_mint,
                    self.ts_state,
                    TSMode::TrailingTriggered,
                    self.token_balance
                );
                enqueue_trailing_reached_alert(
                    &self.token_mint.to_string(),
                    self.token_buying_point_price,
                    self.token_price,
                    trailing * 100.0,
                );
                TSMode::TrailingTriggered
            } else {
                self.ts_state
            };
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Copy)]
pub enum TPMode {
    None,
    Tp,
    SL,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Copy)]
pub enum TokenSellStatus {
    None,
    SellTradeSubmitted,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum SniperTradeStatus {
    None,
    Migrated,
    BuySubmitted,
    SellSubmitted,
    BuyConfirmed,
    SellConfirmed,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Copy)]
pub enum TSMode {
    None,
    TrailingTriggered,
    TrailingStopTriggered,
}
