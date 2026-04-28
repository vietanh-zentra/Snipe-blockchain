use once_cell::sync::Lazy;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::{
    default_buy_amount_sol, default_priority_fee_micro_lamport,
    default_slippage_percent, default_third_party_fee,
    default_take_profit, default_stop_loss, default_trailing, default_trailing_stop,
};

#[derive(Clone, Debug)]
pub struct RunTradingParams {
    pub buy_amount_sol: f64,
    pub slippage: f64,
    pub take_profit: f64,
    pub stop_loss: f64,
    pub trailing: f64,
    pub trailing_stop: f64,
    pub priority_fee_micro_lamports: u64,
    pub tip_fee_sol: f64,
}

impl Default for RunTradingParams {
    fn default() -> Self {
        Self {
            buy_amount_sol: default_buy_amount_sol(),
            slippage: default_slippage_percent() as f64,
            take_profit: default_take_profit(),
            stop_loss: default_stop_loss(),
            trailing: default_trailing(),
            trailing_stop: default_trailing_stop(),
            priority_fee_micro_lamports: default_priority_fee_micro_lamport(),
            tip_fee_sol: default_third_party_fee(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TelegramBotRunState {
    pub is_running: bool,
    pub selected_wallet_pubkey: Option<String>,
    pub selected_wallet_private_key: Option<String>,
    pub trading: RunTradingParams,
}

impl TelegramBotRunState {
    pub fn selected_wallet_keypair(
        &self,
    ) -> Result<Option<Keypair>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(sk) = &self.selected_wallet_private_key else {
            return Ok(None);
        };

        let bytes = bs58::decode(sk)
            .into_vec()
            .map_err(|_| "invalid selected wallet private key (base58 decode failed)")?;
        let key_bytes: [u8; 64] = bytes
            .try_into()
            .map_err(|_| "invalid selected wallet private key length")?;
        let keypair =
            Keypair::try_from(&key_bytes[..]).map_err(|_| "invalid selected wallet keypair")?;
        Ok(Some(keypair))
    }
}

pub static BOT_RUN_STATE: Lazy<Arc<RwLock<TelegramBotRunState>>> =
    Lazy::new(|| Arc::new(RwLock::new(TelegramBotRunState::default())));

pub fn is_running() -> bool {
    BOT_RUN_STATE
        .try_read()
        .map(|s| s.is_running)
        .unwrap_or(false)
}

pub fn get_signer_pubkey() -> Option<Pubkey> {
    let guard = BOT_RUN_STATE.try_read().ok()?;
    guard
        .selected_wallet_pubkey
        .as_ref()
        .and_then(|s| Pubkey::from_str(s.trim()).ok())
}

/// Returns take profit as a multiplier (e.g. 120.0% → 1.2).
pub fn get_take_profit() -> f64 {
    BOT_RUN_STATE
        .try_read()
        .map(|s| s.trading.take_profit / 100.0)
        .unwrap_or(default_take_profit() / 100.0)
}

/// Returns stop loss as a multiplier (e.g. 80.0% → 0.8).
pub fn get_stop_loss() -> f64 {
    BOT_RUN_STATE
        .try_read()
        .map(|s| s.trading.stop_loss / 100.0)
        .unwrap_or(default_stop_loss() / 100.0)
}

/// Returns trailing trigger as a multiplier (e.g. 110.0% → 1.1).
pub fn get_trailing() -> f64 {
    BOT_RUN_STATE
        .try_read()
        .map(|s| s.trading.trailing / 100.0)
        .unwrap_or(default_trailing() / 100.0)
}

/// Returns trailing stop as a fraction (e.g. 10.0% → 0.1).
pub fn get_trailing_stop() -> f64 {
    BOT_RUN_STATE
        .try_read()
        .map(|s| s.trading.trailing_stop / 100.0)
        .unwrap_or(default_trailing_stop() / 100.0)
}