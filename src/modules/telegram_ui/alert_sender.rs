//! Telegram Alert Sender — Fix #2 + #4
//!
//! Cung cấp hàm `send_telegram_alert()` để gửi thông báo qua Telegram
//! từ bất kỳ module nào (anti-rug filter, panic-sell, etc.)
//!
//! Sử dụng global bot instance + chat_id từ ALLOWED_TELEGRAM_USER_ID.

use once_cell::sync::OnceCell;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::ChatId;

/// Global Telegram bot instance + chat_id để gửi alert.
static ALERT_BOT: OnceCell<Arc<AlertBot>> = OnceCell::new();

struct AlertBot {
    bot: Bot,
    chat_id: ChatId,
}

/// Khởi tạo alert bot (gọi 1 lần khi start).
/// Bot token và user_id lấy từ .env.
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

/// Gửi alert text qua Telegram (fire-and-forget, không block).
pub fn send_telegram_alert(message: String) {
    let Some(alert_bot) = ALERT_BOT.get() else {
        // Alert bot chưa khởi tạo — chỉ log
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

/// Gửi alert khi anti-rug filter skip token (Fix #2).
pub fn alert_token_filtered(mint: &str, reason: &str) {
    let msg = format!(
        "🛡️ *Anti-Rug Alert*\n\n\
        Token: `{}`\n\
        ❌ *SKIPPED*\n\
        Reason: {}\n\
        \n_Bot sẽ KHÔNG mua token này._",
        mint, reason
    );
    send_telegram_alert(msg);
}

/// Gửi alert khi panic-sell được trigger (Fix #4).
pub fn alert_panic_sell_triggered(mint: &str, seller: &str, drop_pct: f64) {
    let msg = format!(
        "🚨 *PANIC SELL TRIGGERED*\n\n\
        Token: `{}`\n\
        Seller: `{}`\n\
        Drop: {:.1}%\n\
        \n_Bot đang bán khẩn cấp qua Jito bundle!_",
        mint, seller, drop_pct
    );
    send_telegram_alert(msg);
}

/// Gửi alert khi mua token thành công.
pub fn alert_buy_success(mint: &str, price: f64) {
    let msg = format!(
        "✅ *BUY SUCCESS*\n\n\
        Token: `{}`\n\
        Price: {:.10} SOL\n\
        \n_Panic-sell monitor đang theo dõi..._",
        mint, price
    );
    send_telegram_alert(msg);
}
