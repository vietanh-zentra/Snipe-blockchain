use std::env;
use std::sync::RwLock;

use once_cell::sync::Lazy;
use solana_sdk::pubkey::Pubkey;
use teloxide::prelude::*;
use teloxide::types::ParseMode;
use teloxide::utils::html::{bold, code_inline, link};
use tokio::sync::mpsc::{self, UnboundedSender};

type AlertResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Debug)]
struct AlertMessage {
    text: String,
    parse_mode: Option<ParseMode>,
}

static ALERT_TX: Lazy<RwLock<Option<UnboundedSender<AlertMessage>>>> =
    Lazy::new(|| RwLock::new(None));

pub fn format_migration_alert_html(mint: &str, signature: &str) -> String {
    let suffix = solscan_cluster_suffix();
    let token_url = format!("https://solscan.io/token/{mint}{suffix}");
    let tx_url = format!("https://solscan.io/tx/{signature}{suffix}");

    format!(
        "{}\n\n{}\n{}\n\n{}\n{}",
        bold("🎯 New token is migrated"),
        bold("Mint"),
        code_inline(mint),
        link(&token_url, "View token on Solscan"),
        link(&tx_url, "View transaction on Solscan"),
    )
}

pub fn format_bot_buy_alert_html(
    mint: &str,
    signer_pubkey: Pubkey,
    signature: &str,
    sol_amount: u64,
    token_amount: u64,
) -> String {
    let suffix = solscan_cluster_suffix();
    let token_url = format!("https://solscan.io/token/{mint}{suffix}");
    let tx_url = format!("https://solscan.io/tx/{signature}{suffix}");
    format!(
        "{}\n\n{}: {}\n{}: {}\n{}\n\n{}\n{}\n{}",
        bold("🟢 Bot buy confirmed"),
        bold("Mint"),
        code_inline(mint),
        bold("Signer"),
        code_inline(&signer_pubkey.to_string()),
        format!("SOL amount: {:.4}", sol_amount as f64 / 1e9),
        format!("Token amount: {:.4}", token_amount as f64 / 1e6),
        link(&token_url, "View token on Solscan"),
        link(&tx_url, "View transaction on Solscan"),
    )
}

pub fn format_bot_sell_alert_html(
    mint: &str,
    signer_pubkey: Pubkey,
    signature: &str,
    sol_amount: u64,
    token_amount: u64,
) -> String {
    let suffix = solscan_cluster_suffix();
    let token_url = format!("https://solscan.io/token/{mint}{suffix}");
    let tx_url = format!("https://solscan.io/tx/{signature}{suffix}");
    format!(
        "{}\n\n{}: {}\n{}: {}\n{}\n\n{}\n{}\n{}",
        bold("🔴 Bot sell confirmed"),
        bold("Mint"),
        code_inline(mint),
        bold("Signer"),
        code_inline(&signer_pubkey.to_string()),
        format!("Token amount: {:.4}", token_amount as f64 / 1e6),
        format!("SOL amount: {:.4}", sol_amount as f64 / 1e9),
        link(&token_url, "View token on Solscan"),
        link(&tx_url, "View transaction on Solscan"),
    )
}

pub fn format_tp_triggered_alert_html(mint: &str, buy_price: f64, current_price: f64, tp_pct: f64) -> String {
    format!(
        "{}\n\n{}: {}\nBuy price: {:.10} SOL\nCurrent price: {:.10} SOL\nTP threshold: {}%",
        bold("🟢 Take Profit triggered"),
        bold("Mint"),
        code_inline(mint),
        buy_price,
        current_price,
        tp_pct,
    )
}

pub fn format_sl_triggered_alert_html(mint: &str, buy_price: f64, current_price: f64, sl_pct: f64) -> String {
    format!(
        "{}\n\n{}: {}\nBuy price: {:.10} SOL\nCurrent price: {:.10} SOL\nSL threshold: {}%",
        bold("🔴 Stop Loss triggered"),
        bold("Mint"),
        code_inline(mint),
        buy_price,
        current_price,
        sl_pct,
    )
}

pub fn format_trailing_reached_alert_html(mint: &str, buy_price: f64, current_price: f64, trailing_pct: f64) -> String {
    format!(
        "{}\n\n{}: {}\nBuy price: {:.10} SOL\nCurrent price: {:.10} SOL\nTrailing trigger: {}%",
        bold("📈 Trailing activated"),
        bold("Mint"),
        code_inline(mint),
        buy_price,
        current_price,
        trailing_pct,
    )
}

pub fn format_trailing_stop_triggered_alert_html(
    mint: &str,
    peak_price: f64,
    current_price: f64,
    trailing_stop_pct: f64,
) -> String {
    format!(
        "{}\n\n{}: {}\nPeak price: {:.10} SOL\nCurrent price: {:.10} SOL\nTrailing stop: {}%",
        bold("🔻 Trailing Stop triggered"),
        bold("Mint"),
        code_inline(mint),
        peak_price,
        current_price,
        trailing_stop_pct,
    )
}

pub fn enqueue_tp_triggered_alert(mint: &str, buy_price: f64, current_price: f64, tp_pct: f64) {
    enqueue_telegram_alert_html(format_tp_triggered_alert_html(mint, buy_price, current_price, tp_pct));
}

pub fn enqueue_sl_triggered_alert(mint: &str, buy_price: f64, current_price: f64, sl_pct: f64) {
    enqueue_telegram_alert_html(format_sl_triggered_alert_html(mint, buy_price, current_price, sl_pct));
}

pub fn enqueue_trailing_reached_alert(mint: &str, buy_price: f64, current_price: f64, trailing_pct: f64) {
    enqueue_telegram_alert_html(format_trailing_reached_alert_html(mint, buy_price, current_price, trailing_pct));
}

pub fn enqueue_trailing_stop_triggered_alert(mint: &str, peak_price: f64, current_price: f64, trailing_stop_pct: f64) {
    enqueue_telegram_alert_html(format_trailing_stop_triggered_alert_html(mint, peak_price, current_price, trailing_stop_pct));
}

fn solscan_cluster_suffix() -> String {
    match env::var("SOLSCAN_CLUSTER")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "devnet" => "?cluster=devnet".to_string(),
        "testnet" => "?cluster=testnet".to_string(),
        _ => String::new(),
    }
}

/// Enqueues a formatted migration alert (HTML parse mode).
pub fn enqueue_migration_detected_alert(mint: &str, signature: &str) {
    let html = format_migration_alert_html(mint, signature);
    enqueue_telegram_alert_html(html);
}

pub fn enqueue_bot_buy_alert(
    mint: &str,
    signer_pubkey: Pubkey,
    signature: &str,
    sol_amount: u64,
    token_amount: u64,
) {
    let html = format_bot_buy_alert_html(mint, signer_pubkey, signature, sol_amount, token_amount);
    enqueue_telegram_alert_html(html);
}

pub fn enqueue_bot_sell_alert(mint: &str, signer_pubkey: Pubkey, signature: &str, sol_amount: u64, token_amount: u64) {
    let html = format_bot_sell_alert_html(mint, signer_pubkey, signature, sol_amount, token_amount);
    enqueue_telegram_alert_html(html);
}

pub async fn start_telegram_alert_worker() -> AlertResult<()> {
    let _ = dotenvy::dotenv();

    let token = env::var("TELEGRAM_BOT_TOKEN")
        .map_err(|_| "Missing TELEGRAM_BOT_TOKEN in .env or environment")?;
    let allowed_id_raw = env::var("ALLOWED_TELEGRAM_USER_ID")
        .map_err(|_| "Missing ALLOWED_TELEGRAM_USER_ID in .env or environment")?;
    let allowed_user_id: i64 = allowed_id_raw
        .parse()
        .map_err(|_| "ALLOWED_TELEGRAM_USER_ID must be a valid i64 telegram user id")?;

    let mut tx_guard = ALERT_TX
        .write()
        .map_err(|_| "Failed to acquire telegram alert sender write lock")?;
    if tx_guard.is_some() {
        return Ok(());
    }

    let (tx, mut rx) = mpsc::unbounded_channel::<AlertMessage>();
    *tx_guard = Some(tx);
    drop(tx_guard);

    tokio::spawn(async move {
        let bot = Bot::new(token);
        let chat_id = ChatId(allowed_user_id);

        while let Some(message) = rx.recv().await {
            let send = bot.send_message(chat_id, message.text);
            let send = match message.parse_mode {
                Some(mode) => send.parse_mode(mode),
                None => send,
            };
            if let Err(err) = send.await {
                eprintln!("Telegram alert send failed: {err}");
            }
        }
    });

    Ok(())
}

pub fn enqueue_telegram_alert(text: impl Into<String>) {
    enqueue_alert(AlertMessage {
        text: text.into(),
        parse_mode: None,
    });
}

pub fn enqueue_telegram_alert_html(text: impl Into<String>) {
    enqueue_alert(AlertMessage {
        text: text.into(),
        parse_mode: Some(ParseMode::Html),
    });
}

fn enqueue_alert(message: AlertMessage) {
    let sender = ALERT_TX.read().ok().and_then(|guard| guard.clone());
    if let Some(tx) = sender {
        let _ = tx.send(message);
    }
}
