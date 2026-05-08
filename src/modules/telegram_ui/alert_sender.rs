//! Telegram Alert Sender — Fix #2 + #4
//!
//! Provides `send_telegram_alert()` to send notifications via Telegram
//! from any module (anti-rug filter, panic-sell, etc.)
//!
//! Uses global bot instance + chat_id from ALLOWED_TELEGRAM_USER_ID.

use once_cell::sync::OnceCell;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::ChatId;

/// Global Telegram bot instance + chat_id for sending alerts.
static ALERT_BOT: OnceCell<Arc<AlertBot>> = OnceCell::new();

struct AlertBot {
    bot: Bot,
    chat_id: ChatId,
}

/// Initialize alert bot (called once at startup).
/// Bot token and user_id are loaded from .env.
pub fn init_alert_bot() {
    let _ = dotenvy::dotenv();
    let token = match std::env::var("TELEGRAM_BOT_TOKEN") {
        Ok(t) => t,
        Err(_) => {
            eprintln!("[ALERT] Missing TELEGRAM_BOT_TOKEN — alerts disabled");
            return;
        }
    };
    let user_id: i64 = match std::env::var("ALLOWED_TELEGRAM_USER_ID") {
        Ok(id) => id.parse().unwrap_or(0),
        Err(_) => {
            eprintln!("[ALERT] Missing ALLOWED_TELEGRAM_USER_ID — alerts disabled");
            return;
        }
    };

    if user_id == 0 {
        eprintln!("[ALERT] Invalid ALLOWED_TELEGRAM_USER_ID — alerts disabled");
        return;
    }

    let bot = Bot::new(token);
    let chat_id = ChatId(user_id);

    let _ = ALERT_BOT.set(Arc::new(AlertBot { bot, chat_id }));
    println!("[ALERT] ✅ Telegram alert sender initialized for user {user_id}");
}

/// Send alert text via Telegram (fire-and-forget, non-blocking).
pub fn send_telegram_alert(message: String) {
    let Some(alert_bot) = ALERT_BOT.get() else {
        // Alert bot not initialized — log only
        eprintln!("[ALERT] Bot not initialized, dropping: {message}");
        return;
    };

    let bot = alert_bot.bot.clone();
    let chat_id = alert_bot.chat_id;

    tokio::spawn(async move {
        if let Err(e) = bot.send_message(chat_id, &message).await {
            eprintln!("[ALERT] Failed to send Telegram alert: {e}");
        }
    });
}

/// Send alert when anti-rug filter skips a token (Fix #2).
pub fn alert_token_filtered(mint: &str, reason: &str) {
    let msg = format!(
        "🛡️ *Anti-Rug Alert*\n\n\
        Token: `{}`\n\
        ❌ *SKIPPED*\n\
        Reason: {}\n\
        \n_Bot will NOT buy this token._",
        mint, reason
    );
    send_telegram_alert(msg);
}

/// Send alert when panic-sell is triggered (Fix #4).
pub fn alert_panic_sell_triggered(mint: &str, seller: &str, drop_pct: f64) {
    let msg = format!(
        "🚨 *PANIC SELL — SELL SUBMITTED*\n\n\
        Token: `{}`\n\
        Seller: `{}`\n\
        Drop: {:.1}%\n\
        \n_Emergency sell submitted via Jito bundle!_",
        mint, seller, drop_pct
    );
    send_telegram_alert(msg);
}

/// Send alert when a buy transaction is submitted (unconfirmed fallback).
pub fn alert_buy_success(mint: &str, price: f64) {
    let msg = format!(
        "📤 *BUY SUBMITTED*\n\n\
        Token: `{}`\n\
        Price: {:.10} SOL\n\
        \n_Transaction sent. Waiting for on-chain confirmation..._",
        mint, price
    );
    send_telegram_alert(msg);
}

/// Send alert when a buy is confirmed SUCCESS on-chain.
pub fn alert_buy_confirmed(mint: &str, price: f64, tx_hash: &str) {
    let msg = format!(
        "✅ *BUY SUCCESS*\n\n\
        Token: `{}`\n\
        Price: {:.10} SOL\n\
        Tx: `{}`\n\
        \n_Token acquired! Monitoring for TP/SL..._",
        mint, price, tx_hash
    );
    send_telegram_alert(msg);
}

/// Send alert when a buy FAILED on-chain.
pub fn alert_buy_failed(mint: &str, reason: &str) {
    let msg = format!(
        "❌ *BUY FAILED*\n\n\
        Token: `{}`\n\
        Reason: `{}`\n\
        \n_Transaction failed on-chain. No funds lost._",
        mint, reason
    );
    send_telegram_alert(msg);
}

/// Send alert when Take Profit is triggered.
pub fn alert_take_profit(mint: &str) {
    let msg = format!(
        "💰 *TAKE PROFIT — SELL SUBMITTED*\n\n\
        Token: `{}`\n\
        \n_Price hit TP target. Selling now!_",
        mint
    );
    send_telegram_alert(msg);
}

/// Send alert when Stop Loss is triggered.
pub fn alert_stop_loss(mint: &str) {
    let msg = format!(
        "🔻 *STOP LOSS — SELL SUBMITTED*\n\n\
        Token: `{}`\n\
        \n_Price dropped below SL threshold. Selling to cut losses._",
        mint
    );
    send_telegram_alert(msg);
}

/// Send alert when Trailing Stop is triggered.
pub fn alert_trailing_stop(mint: &str) {
    let msg = format!(
        "📉 *TRAILING STOP — SELL SUBMITTED*\n\n\
        Token: `{}`\n\
        \n_Price reversed from peak. Trailing stop triggered._",
        mint
    );
    send_telegram_alert(msg);
}
