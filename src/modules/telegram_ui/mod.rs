pub mod alert_sender;
pub mod run_state;

use once_cell::sync::Lazy;
use crate::modules::postgresql::{
    TradingParameterRecord, create_wallet, delete_wallet, init_postgres_and_migrate,
    load_or_create_trading_parameters, load_wallet_by_wallet_id, load_wallets,
    resolve_database_url_from_env, save_trading_parameters, set_selected_wallet,
};
use run_state::BOT_RUN_STATE;
use solana_sdk::signature::{Keypair, Signer};
use std::env;
use std::sync::Arc;
use sea_orm::DatabaseConnection;
use teloxide::dispatching::UpdateFilterExt;
use teloxide::dptree;
use teloxide::prelude::*;
use teloxide::types::{
    CallbackQuery, ChatId, InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton, KeyboardMarkup,
    ReplyMarkup,
};
use tokio::sync::RwLock;
use tokio::time::timeout;
use crate::config::*;
use crate::modules::rpc::{
    batch_wallet_balances_lamports, lamports_to_sol, WALLET_BALANCE_RPC_TIMEOUT,
};

type BotResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
type BotValueResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

const MENU_WALLET: &str = "💰 Wallet management";
const MENU_PARAMS: &str = "⚙️ Trading parameters";
const MENU_ANTIRUG: &str = "🛡️ Anti-Rug";
const MENU_START: &str = "▶️ Start";
const MENU_STOP: &str = "⏹ Stop";

#[derive(Clone, Debug)]
pub struct WalletEntry {
    pub wallet_id: i32,
    pub pubkey: String,
    pub private_key: String,
}

#[derive(Clone, Debug)]
pub struct TradingParams {
    pub buy_amount_sol: f64,
    pub slippage_percent: u32,
    pub take_profit: f64,
    pub stop_loss: f64,
    pub trailing: f64,
    pub trailing_stop: f64,
    pub priority_fee_micro_lamports: u64,
    pub tip_fee_sol: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PendingInput {
    None,
    ImportPrivateKey,
    BuyAmountCustom,
    SlippageCustom,
    TakeProfitCustom,
    StopLossCustom,
    TrailingCustom,
    TrailingStopCustom,
    PriorityFeeCustom,
    TipFeeCustom,
}

#[derive(Clone, Debug)]
pub struct TelegramUiState {
    pub wallets: Vec<WalletEntry>,
    pub selected_wallet_id: Option<i32>,
    pub trading: TradingParams,
    pub is_running: bool,
    pending_input: PendingInput,
}

impl Default for TelegramUiState {
    fn default() -> Self {
        Self {
            wallets: Vec::new(),
            selected_wallet_id: None,
            trading: TradingParams {
                buy_amount_sol: default_buy_amount_sol(),
                slippage_percent: default_slippage_percent(),
                take_profit: default_take_profit(),
                stop_loss: default_stop_loss(),
                trailing: default_trailing(),
                trailing_stop: default_trailing_stop(),
                priority_fee_micro_lamports: default_priority_fee_micro_lamport(),
                tip_fee_sol: default_third_party_fee(),
            },
            is_running: false,
            pending_input: PendingInput::None,
        }
    }
}

pub static TELEGRAM_UI_STATE: Lazy<Arc<RwLock<TelegramUiState>>> =
    Lazy::new(|| Arc::new(RwLock::new(TelegramUiState::default())));

pub async fn start_telegram_ui() -> BotResult {
    let _ = dotenvy::dotenv();

    let token = env::var("TELEGRAM_BOT_TOKEN")
        .map_err(|_| "Missing TELEGRAM_BOT_TOKEN in .env or environment")?;
    let allowed_id_raw = env::var("ALLOWED_TELEGRAM_USER_ID")
        .map_err(|_| "Missing ALLOWED_TELEGRAM_USER_ID in .env or environment")?;
    let allowed_user_id: i64 = allowed_id_raw.parse().map_err(|_| {
        "ALLOWED_TELEGRAM_USER_ID must be a valid i64 telegram user id"
    })?;
    let database_url = resolve_database_url_from_env()?;
    let wallet_encryption_password = env::var("WALLET_ENCRYPTION_PASSWORD")
        .map_err(|_| "Missing WALLET_ENCRYPTION_PASSWORD in .env or environment")?;

    let db = init_postgres_and_migrate(&database_url).await?;
    bootstrap_state_from_db(
        &db,
        allowed_user_id,
        &wallet_encryption_password,
        TELEGRAM_UI_STATE.clone(),
    )
    .await?;

    let bot = Bot::new(token);
    println!("🤖 Telegram UI started for allowed user id: {allowed_user_id}");

    let message_handler = Update::filter_message().endpoint(handle_message);
    let callback_handler = Update::filter_callback_query().endpoint(handle_callback);
    let handler = dptree::entry()
        .branch(message_handler)
        .branch(callback_handler);

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![
            allowed_user_id,
            TELEGRAM_UI_STATE.clone(),
            db,
            wallet_encryption_password
        ])
        .build()
        .dispatch()
        .await;

    Ok(())
}

async fn handle_message(
    bot: Bot,
    msg: Message,
    allowed_user_id: i64,
    state: Arc<RwLock<TelegramUiState>>,
    db: DatabaseConnection,
    wallet_encryption_password: String,
) -> BotResult {
    let chat_id = msg.chat.id;
    let text = match msg.text() {
        Some(t) => t.trim(),
        None => return Ok(()),
    };

    // /myid is intentionally available before authorization check so
    // you can discover the ID to place into ALLOWED_TELEGRAM_USER_ID.
    if text.eq_ignore_ascii_case("/myid") {
        let maybe_id = msg.from().as_ref().map(|u| u.id.0 as i64);
        match maybe_id {
            Some(user_id) => {
                bot.send_message(chat_id, format!("Your Telegram user id: {user_id}"))
                    .await?;
            }
            None => {
                bot.send_message(chat_id, "Could not determine your Telegram user id")
                    .await?;
            }
        }
        return Ok(());
    }

    if (text.eq_ignore_ascii_case("/start") || text.eq_ignore_ascii_case("/menu"))
        && !is_allowed_message_user(&msg, allowed_user_id)
    {
        match msg.from().as_ref().map(|u| u.id.0 as i64) {
            Some(user_id) => {
                bot.send_message(chat_id, format_not_authorized_message(user_id))
                    .await?;
            }
            None => {
                bot.send_message(
                    chat_id,
                    "Not authorized\n\nCould not determine your Telegram user id.",
                )
                .await?;
            }
        }
        return Ok(());
    }

    if !is_allowed_message_user(&msg, allowed_user_id) {
        return Ok(());
    }

    if text.eq_ignore_ascii_case("/start") || text.eq_ignore_ascii_case("/menu") {
        send_main_menu(&bot, chat_id, &state).await?;
        return Ok(());
    }

    if text == MENU_WALLET {
        open_wallet_management(&bot, chat_id, &state, None).await?;
        return Ok(());
    }
    if text == MENU_PARAMS {
        open_trading_parameters(&bot, chat_id, &state, None).await?;
        return Ok(());
    }
    if text == MENU_ANTIRUG {
        send_anti_rug_menu(&bot, chat_id, &state).await?;
        return Ok(());
    }
    if text == MENU_START || text == MENU_STOP {
        handle_menu_start_stop_text(&bot, chat_id, allowed_user_id, &state, &db, text).await?;
        return Ok(());
    }

    // Handle pending user text input first.
    {
        let pending = state.read().await.pending_input.clone();
        if pending != PendingInput::None {
            handle_pending_input(
                &bot,
                chat_id,
                allowed_user_id,
                &state,
                &db,
                &wallet_encryption_password,
                text,
                pending,
            )
            .await?;
            return Ok(());
        }
    }

    if text.eq_ignore_ascii_case("/generate") {
        command_generate_wallet(
            &bot,
            chat_id,
            allowed_user_id,
            &state,
            &db,
            &wallet_encryption_password,
        )
        .await?;
        return Ok(());
    }

    if text.starts_with("/import_key") {
        let mut split = text.splitn(2, ' ');
        let _ = split.next();
        if let Some(key) = split.next() {
            import_private_key_and_reply(
                &bot,
                chat_id,
                allowed_user_id,
                &state,
                &db,
                &wallet_encryption_password,
                key.trim(),
            )
            .await?;
        } else {
            state.write().await.pending_input = PendingInput::ImportPrivateKey;
            bot.send_message(
                chat_id,
                "Send private key in next message.\nSupported format: base58 secret key.",
            )
            .await?;
        }
        return Ok(());
    }

    if text.starts_with("/select_") {
        command_select_wallet(&bot, chat_id, allowed_user_id, &state, &db, text).await?;
        return Ok(());
    }

    if text.starts_with("/delete_") {
        command_delete_wallet(
            &bot,
            chat_id,
            allowed_user_id,
            &state,
            &db,
            &wallet_encryption_password,
            text,
        )
        .await?;
        return Ok(());
    }

    if text.starts_with("/show_key_") {
        command_show_wallet_key(
            &bot,
            chat_id,
            allowed_user_id,
            &db,
            &wallet_encryption_password,
            text,
        )
        .await?;
        return Ok(());
    }

    if text.eq_ignore_ascii_case("/wallets") {
        send_wallet_management_help(&bot, chat_id, &state).await?;
        return Ok(());
    }

    bot.send_message(
        chat_id,
        "Unknown command. Use the menu buttons, /myid, /menu, /wallets, /generate, /import_key, /select_N, /delete_N, /show_key_N",
    )
    .await?;
    Ok(())
}

async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    allowed_user_id: i64,
    state: Arc<RwLock<TelegramUiState>>,
    db: DatabaseConnection,
) -> BotResult {
    let Some(data) = q.data.as_deref() else {
        return Ok(());
    };

    let chat_id = match q.message.as_ref().map(|m| m.chat.id) {
        Some(id) => id,
        None => return Ok(()),
    };

    let user_id = q.from.id.0 as i64;
    if user_id != allowed_user_id {
        bot.answer_callback_query(q.id.clone()).await?;
        if data == "bot_toggle" {
            bot.send_message(chat_id, format_not_authorized_message(user_id))
                .await?;
        }
        return Ok(());
    }

    bot.answer_callback_query(q.id.clone()).await?;

    match data {
        "main_wallets" => {
            open_wallet_management(&bot, chat_id, &state, q.message.as_ref())
                .await?;
        }
        "main_trading" => {
            open_trading_parameters(&bot, chat_id, &state, q.message.as_ref())
                .await?;
        }
        "trading_buy" => {
            send_buy_amount_menu(&bot, chat_id).await?;
        }
        "trading_slippage" => {
            send_slippage_menu(&bot, chat_id).await?;
        }
        "trading_tp" => {
            send_take_profit_menu(&bot, chat_id).await?;
        }
        "trading_sl" => {
            send_stop_loss_menu(&bot, chat_id).await?;
        }
        "trading_trailing" => {
            send_trailing_menu(&bot, chat_id).await?;
        }
        "trading_trailing_stop" => {
            send_trailing_stop_menu(&bot, chat_id).await?;
        }
        "trading_priority_fee" => {
            send_priority_fee_menu(&bot, chat_id).await?;
        }
        "trading_tip_fee" => {
            send_tip_fee_menu(&bot, chat_id).await?;
        }
        "buy_0.1" => set_buy_amount_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 0.1).await?,
        "buy_0.5" => set_buy_amount_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 0.5).await?,
        "buy_1.0" => set_buy_amount_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 1.0).await?,
        "buy_custom" => {
            state.write().await.pending_input = PendingInput::BuyAmountCustom;
            bot.send_message(chat_id, "Send custom buy amount in SOL (example: 0.25)")
                .await?;
        }
        "slippage_30" => set_slippage_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 30).await?,
        "slippage_50" => set_slippage_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 50).await?,
        "slippage_100" => set_slippage_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 100).await?,
        "slippage_custom" => {
            state.write().await.pending_input = PendingInput::SlippageCustom;
            bot.send_message(chat_id, "Send custom slippage percent (example: 75)")
                .await?;
        }
        "tp_110" => set_take_profit_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 110.0).await?,
        "tp_120" => set_take_profit_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 120.0).await?,
        "tp_150" => set_take_profit_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 150.0).await?,
        "tp_custom" => {
            state.write().await.pending_input = PendingInput::TakeProfitCustom;
            bot.send_message(chat_id, "Send custom take profit % (example: 130)")
                .await?;
        }
        "sl_70" => set_stop_loss_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 70.0).await?,
        "sl_80" => set_stop_loss_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 80.0).await?,
        "sl_90" => set_stop_loss_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 90.0).await?,
        "sl_custom" => {
            state.write().await.pending_input = PendingInput::StopLossCustom;
            bot.send_message(chat_id, "Send custom stop loss % (example: 75)")
                .await?;
        }
        "trail_105" => set_trailing_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 105.0).await?,
        "trail_110" => set_trailing_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 110.0).await?,
        "trail_120" => set_trailing_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 120.0).await?,
        "trail_custom" => {
            state.write().await.pending_input = PendingInput::TrailingCustom;
            bot.send_message(chat_id, "Send custom trailing trigger % (example: 115)")
                .await?;
        }
        "ts_5" => set_trailing_stop_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 5.0).await?,
        "ts_10" => set_trailing_stop_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 10.0).await?,
        "ts_20" => set_trailing_stop_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 20.0).await?,
        "ts_custom" => {
            state.write().await.pending_input = PendingInput::TrailingStopCustom;
            bot.send_message(chat_id, "Send custom trailing stop % (example: 15)")
                .await?;
        }
        "priority_50000" => {
            set_priority_fee_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 50_000).await?
        }
        "priority_100000" => {
            set_priority_fee_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 100_000).await?
        }
        "priority_200000" => {
            set_priority_fee_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 200_000).await?
        }
        "priority_custom" => {
            state.write().await.pending_input = PendingInput::PriorityFeeCustom;
            bot.send_message(
                chat_id,
                "Send custom priority fee in micro-lamports (example: 150000)\nFormula: priority_fee = cu * micro_lamports / 1e15, cu=200000",
            )
                .await?;
        }
        "tip_0.0005" => {
            set_tip_fee_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 0.0005).await?
        }
        "tip_0.001" => {
            set_tip_fee_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 0.001).await?
        }
        "tip_0.002" => {
            set_tip_fee_and_reply(&bot, chat_id, allowed_user_id, &state, &db, 0.002).await?
        }
        "tip_custom" => {
            state.write().await.pending_input = PendingInput::TipFeeCustom;
            bot.send_message(chat_id, "Send custom tip fee in SOL (example: 0.0015)")
                .await?;
        }
        "bot_toggle" => {
            let (was_running, has_wallet) = {
                let g = state.read().await;
                (g.is_running, g.selected_wallet_id.is_some())
            };
            let status_msg = if was_running {
                state.write().await.is_running = false;
                log_bot_run_status_changed(false);
                "Bot is stopped"
            } else if !has_wallet {
                bot.send_message(
                    chat_id,
                    "Cannot start: no wallet selected.\nUse /generate or /import_key, then /select_N.",
                )
                .await?;
                return Ok(());
            } else {
                state.write().await.is_running = true;
                log_bot_run_status_changed(true);
                "Bot is started"
            };
            persist_trading_state(&db, allowed_user_id, &state).await?;
            sync_bot_run_state_from_ui(&state).await;
            let is_running = state.read().await.is_running;
            bot.send_message(chat_id, status_msg)
                .reply_markup(main_menu_reply_keyboard(is_running))
                .await?;
            if let Some(m) = q.message.as_ref() {
                let body = m.text().unwrap_or("");
                let (is_running, trading) = {
                    let g = state.read().await;
                    (g.is_running, g.trading.clone())
                };
                if let Some(kb) = trading_inline_keyboard_for_message(body, is_running, &trading) {
                    let _ = bot
                        .edit_message_reply_markup(m.chat.id, m.id)
                        .reply_markup(kb)
                        .await;
                }
            }
        }
        "go_main" => {
            show_main_menu(&bot, chat_id, &state, q.message.as_ref()).await?;
        }
        // ── Anti-Rug toggle callbacks (Brief L555, L562, L669) ────────────
        "ar_master_toggle" => {
            let mut run = BOT_RUN_STATE.write().await;
            run.anti_rug.enabled = !run.anti_rug.enabled;
            let status = if run.anti_rug.enabled { "ON" } else { "OFF" };
            drop(run);
            bot.send_message(chat_id, format!("🛡️ Anti-Rug: {status}")).await?;
            send_anti_rug_menu(&bot, chat_id, &state).await?;
        }
        "ar_warn_toggle" => {
            let mut run = BOT_RUN_STATE.write().await;
            run.anti_rug.warn_only = !run.anti_rug.warn_only;
            let mode = if run.anti_rug.warn_only { "WARN ONLY" } else { "BLOCK" };
            drop(run);
            bot.send_message(chat_id, format!("Mode: {mode}")).await?;
            send_anti_rug_menu(&bot, chat_id, &state).await?;
        }
        "ar_holder_toggle" => {
            let mut run = BOT_RUN_STATE.write().await;
            run.anti_rug.holder_filter_enabled = !run.anti_rug.holder_filter_enabled;
            let s = if run.anti_rug.holder_filter_enabled { "ON" } else { "OFF" };
            drop(run);
            bot.send_message(chat_id, format!("M1 Holder Filter: {s}")).await?;
            send_anti_rug_menu(&bot, chat_id, &state).await?;
        }
        "ar_dev_toggle" => {
            let mut run = BOT_RUN_STATE.write().await;
            run.anti_rug.dev_profiler_enabled = !run.anti_rug.dev_profiler_enabled;
            let s = if run.anti_rug.dev_profiler_enabled { "ON" } else { "OFF" };
            drop(run);
            bot.send_message(chat_id, format!("M3 Dev Profiler: {s}")).await?;
            send_anti_rug_menu(&bot, chat_id, &state).await?;
        }
        "ar_genesis_toggle" => {
            let mut run = BOT_RUN_STATE.write().await;
            run.anti_rug.genesis_detector_enabled = !run.anti_rug.genesis_detector_enabled;
            let s = if run.anti_rug.genesis_detector_enabled { "ON" } else { "OFF" };
            drop(run);
            bot.send_message(chat_id, format!("M4 Genesis Detector: {s}")).await?;
            send_anti_rug_menu(&bot, chat_id, &state).await?;
        }
        "ar_meta_toggle" => {
            let mut run = BOT_RUN_STATE.write().await;
            run.anti_rug.metadata_checker_enabled = !run.anti_rug.metadata_checker_enabled;
            let s = if run.anti_rug.metadata_checker_enabled { "ON" } else { "OFF" };
            drop(run);
            bot.send_message(chat_id, format!("M5 Metadata Checker: {s}")).await?;
            send_anti_rug_menu(&bot, chat_id, &state).await?;
        }
        "ar_panic_toggle" => {
            let mut run = BOT_RUN_STATE.write().await;
            run.anti_rug.panic_sell_enabled = !run.anti_rug.panic_sell_enabled;
            let s = if run.anti_rug.panic_sell_enabled { "ON" } else { "OFF" };
            drop(run);
            bot.send_message(chat_id, format!("M2 Panic-Sell: {s}")).await?;
            send_anti_rug_menu(&bot, chat_id, &state).await?;
        }
        "ar_holder30" => { BOT_RUN_STATE.write().await.anti_rug.max_top10_holder_pct = 30.0; bot.send_message(chat_id, "Max holder: 30%").await?; }
        "ar_holder40" => { BOT_RUN_STATE.write().await.anti_rug.max_top10_holder_pct = 40.0; bot.send_message(chat_id, "Max holder: 40%").await?; }
        "ar_holder50" => { BOT_RUN_STATE.write().await.anti_rug.max_top10_holder_pct = 50.0; bot.send_message(chat_id, "Max holder: 50%").await?; }
        _ => {}
    }

    Ok(())
}

fn is_allowed_message_user(msg: &Message, allowed_user_id: i64) -> bool {
    msg.from().as_ref().map(|u| u.id.0 as i64) == Some(allowed_user_id)
}

fn format_not_authorized_message(user_id: i64) -> String {
    format!(
        "Not authorized\n\nYour Telegram user id: {user_id}\n\nPlease configure ALLOWED_TELEGRAM_USER_ID in .env with this id."
    )
}

fn escape_html_for_telegram(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Private key hidden behind spoiler effect; tap to reveal, then long-press to copy.
fn format_private_key_spoiler_html(private_key: &str) -> String {
    let safe = escape_html_for_telegram(private_key);
    format!("<span class=\"tg-spoiler\">{}</span>", safe)
}

/// User-facing wallet label, e.g. `No1.`, `No2.`
fn format_wallet_no(wallet_id: i32) -> String {
    format!("{}.", wallet_id)
}

const MAIN_MENU_TITLE: &str = " 🖐 Welcome to the Migration Sniper 🤖 Bot!!!";

fn main_menu_reply_keyboard(is_running: bool) -> ReplyMarkup {
    let start_stop = if is_running { MENU_STOP } else { MENU_START };
    let kb = KeyboardMarkup::new(vec![
        vec![KeyboardButton::new(MENU_WALLET), KeyboardButton::new(MENU_PARAMS)],
        vec![KeyboardButton::new(MENU_ANTIRUG), KeyboardButton::new(start_stop)],
    ])
    .resize_keyboard(true);
    ReplyMarkup::Keyboard(kb)
}

fn format_sol_button_value(x: f64) -> String {
    let s = format!("{:.4}", x);
    let t = s.trim_end_matches('0').trim_end_matches('.');
    format!("{t} SOL")
}

fn trading_menu_keyboard(_is_running: bool, trading: &TradingParams) -> InlineKeyboardMarkup {
    let buy_lbl = format!("Buy amount ({})", format_sol_button_value(trading.buy_amount_sol));
    let slip_lbl = format!("Slippage ({}%)", trading.slippage_percent);
    let tp_lbl = format!("Take profit ({}%)", format_pct_value(trading.take_profit));
    let sl_lbl = format!("Stop loss ({}%)", format_pct_value(trading.stop_loss));
    let trail_lbl = format!("Trailing ({}%)", format_pct_value(trading.trailing));
    let trail_stop_lbl = format!("Trailing stop ({}%)", format_pct_value(trading.trailing_stop));
    let pri_lbl = format!("Priority fee micro-lamports ({})", trading.priority_fee_micro_lamports);
    let tip_lbl = format!("Tip fee ({})", format_sol_button_value(trading.tip_fee_sol));
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(buy_lbl, "trading_buy")],
        vec![InlineKeyboardButton::callback(slip_lbl, "trading_slippage")],
        vec![InlineKeyboardButton::callback(tp_lbl, "trading_tp")],
        vec![InlineKeyboardButton::callback(sl_lbl, "trading_sl")],
        vec![InlineKeyboardButton::callback(trail_lbl, "trading_trailing")],
        vec![InlineKeyboardButton::callback(trail_stop_lbl, "trading_trailing_stop")],
        vec![InlineKeyboardButton::callback(pri_lbl, "trading_priority_fee")],
        vec![InlineKeyboardButton::callback(tip_lbl, "trading_tip_fee")],
        vec![InlineKeyboardButton::callback("Back to main menu", "go_main")],
    ])
}

fn format_pct_value(v: f64) -> String {
    let s = format!("{:.2}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// After Start/Stop on the trading (inline) screen, refresh inline buttons only.
fn trading_inline_keyboard_for_message(
    message_text: &str,
    is_running: bool,
    trading: &TradingParams,
) -> Option<InlineKeyboardMarkup> {
    if message_text.contains("Trading parameters") {
        Some(trading_menu_keyboard(is_running, trading))
    } else {
        None
    }
}

async fn format_wallet_management_text(state: &TelegramUiState) -> String {
    let wallet_rows = if state.wallets.is_empty() {
        "No wallets yet.".to_string()
    } else {
        let pubkeys: Vec<String> = state.wallets.iter().map(|w| w.pubkey.clone()).collect();
        let balances_lamports = match timeout(
            WALLET_BALANCE_RPC_TIMEOUT,
            batch_wallet_balances_lamports(&pubkeys),
        )
        .await
        {
            Ok(v) => v,
            Err(_) => {
                eprintln!("wallet balance fetch timed out");
                vec![0u64; state.wallets.len()]
            }
        };

        let mut sections = Vec::new();
        for (i, w) in state.wallets.iter().enumerate() {
            let selected_mark = if state.selected_wallet_id == Some(w.wallet_id) {
                " ✅"
            } else {
                ""
            };
            let sol_balance = lamports_to_sol(*balances_lamports.get(i).unwrap_or(&0));
            sections.push(format!(
                "{} <code>{}</code>{}\nBalance: {}",
                format_wallet_no(w.wallet_id),
                w.pubkey,
                selected_mark,
                format_sol_button_value(sol_balance)
            ));
        }
        sections.join("\n\n")
    };

    format!(
        "💰 Wallet management\n\n\
Commands:\n\
/generate - generate new wallet\n\
/import_key - import private key via next message\n\
/select_N - select wallet id N for trading\n\
/delete_N - delete wallet by id N\n\
/show_key_N - show private key of wallet N\n\
/wallets - show this menu\n\n\
Wallet list:\n{}",
        wallet_rows
    )
}

fn format_trading_parameters_text(trading: &TradingParams) -> String {
    let priority_fee_sol = priority_fee_sol_from_micro_lamports(
        trading.priority_fee_micro_lamports,
        default_cu(),
    );
    format!(
        "⚙️ Trading parameters\n\n\
Buy amount: {} SOL\n\
Slippage: {}%\n\
Take profit: {}%\n\
Stop loss: {}%\n\
Trailing: {}%\n\
Trailing stop: {}%\n\
Priority fee micro-lamports: {}\n\
Priority fee (SOL): {}\n\
Tip fee: {} SOL\n\
Formula: priority_fee = cu * micro_lamports / 1e15 (cu={})",
        trading.buy_amount_sol,
        trading.slippage_percent,
        format_pct_value(trading.take_profit),
        format_pct_value(trading.stop_loss),
        format_pct_value(trading.trailing),
        format_pct_value(trading.trailing_stop),
        trading.priority_fee_micro_lamports,
        priority_fee_sol,
        trading.tip_fee_sol,
        default_cu()
    )
}

fn log_bot_run_status_changed(is_running: bool) {
    println!(
        "[TELEGRAM_UI] Bot status changed: {}",
        if is_running { "RUNNING" } else { "STOPPED" }
    );
}

fn log_trading_param_changed(name: &str, value: impl std::fmt::Display) {
    println!("[TELEGRAM_UI] Trading parameter changed: {name} = {value}");
}

async fn handle_menu_start_stop_text(
    bot: &Bot,
    chat_id: ChatId,
    allowed_user_id: i64,
    state: &Arc<RwLock<TelegramUiState>>,
    db: &DatabaseConnection,
    text: &str,
) -> BotResult {
    let (is_running, has_wallet) = {
        let g = state.read().await;
        (g.is_running, g.selected_wallet_id.is_some())
    };
    let want_start = text == MENU_START;
    if want_start {
        if is_running {
            bot.send_message(chat_id, "Bot is already running.")
                .reply_markup(main_menu_reply_keyboard(true))
                .await?;
            return Ok(());
        }
        if !has_wallet {
            bot.send_message(
                chat_id,
                "Cannot start: no wallet selected.\nUse /generate or /import_key, then /select_N.",
            )
            .await?;
            return Ok(());
        }
        state.write().await.is_running = true;
        log_bot_run_status_changed(true);
        persist_trading_state(db, allowed_user_id, state).await?;
        sync_bot_run_state_from_ui(state).await;
        bot.send_message(chat_id, "Bot is started")
            .reply_markup(main_menu_reply_keyboard(true))
            .await?;
        return Ok(());
    }
    if !is_running {
        bot.send_message(chat_id, "Bot is already stopped.")
            .reply_markup(main_menu_reply_keyboard(false))
            .await?;
        return Ok(());
    }
    state.write().await.is_running = false;
    log_bot_run_status_changed(false);
    persist_trading_state(db, allowed_user_id, state).await?;
    sync_bot_run_state_from_ui(state).await;
    bot.send_message(chat_id, "Bot is stopped")
        .reply_markup(main_menu_reply_keyboard(false))
        .await?;
    Ok(())
}

async fn show_main_menu(
    bot: &Bot,
    chat_id: ChatId,
    state: &Arc<RwLock<TelegramUiState>>,
    replace: Option<&Message>,
) -> BotResult {
    let is_running = state.read().await.is_running;
    if let Some(m) = replace {
        bot.edit_message_text(m.chat.id, m.id, MAIN_MENU_TITLE).await?;
    } else {
        bot.send_message(chat_id, MAIN_MENU_TITLE)
            .reply_markup(main_menu_reply_keyboard(is_running))
            .await?;
    }
    Ok(())
}

async fn open_wallet_management(
    bot: &Bot,
    chat_id: ChatId,
    state: &Arc<RwLock<TelegramUiState>>,
    replace: Option<&Message>,
) -> BotResult {
    let (wallet_state, is_running) = {
        let g = state.read().await;
        (g.clone(), g.is_running)
    };
    let text = format_wallet_management_text(&wallet_state).await;
    let markup = main_menu_reply_keyboard(is_running);
    if let Some(m) = replace {
        bot.edit_message_text(m.chat.id, m.id, text)
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
    } else {
        bot.send_message(chat_id, text)
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(markup)
            .await?;
    }
    Ok(())
}

async fn open_trading_parameters(
    bot: &Bot,
    chat_id: ChatId,
    state: &Arc<RwLock<TelegramUiState>>,
    replace: Option<&Message>,
) -> BotResult {
    let (text, is_running, trading) = {
        let g = state.read().await;
        (
            format_trading_parameters_text(&g.trading),
            g.is_running,
            g.trading.clone(),
        )
    };
    let markup = trading_menu_keyboard(is_running, &trading);
    if let Some(m) = replace {
        bot.edit_message_text(m.chat.id, m.id, text)
            .reply_markup(markup)
            .await?;
    } else {
        bot.send_message(chat_id, text).reply_markup(markup).await?;
    }
    Ok(())
}

async fn send_main_menu(
    bot: &Bot,
    chat_id: ChatId,
    state: &Arc<RwLock<TelegramUiState>>,
) -> BotResult {
    show_main_menu(bot, chat_id, state, None).await
}

async fn send_wallet_management_help(
    bot: &Bot,
    chat_id: ChatId,
    state: &Arc<RwLock<TelegramUiState>>,
) -> BotResult {
    open_wallet_management(bot, chat_id, state, None).await
}

async fn send_buy_amount_menu(bot: &Bot, chat_id: ChatId) -> BotResult {
    let kb = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("0.1 SOL", "buy_0.1"),
            InlineKeyboardButton::callback("0.5 SOL", "buy_0.5"),
            InlineKeyboardButton::callback("1 SOL", "buy_1.0"),
        ],
        vec![InlineKeyboardButton::callback("Custom", "buy_custom")],
    ]);
    bot.send_message(chat_id, "Set buy amount").reply_markup(kb).await?;
    Ok(())
}

async fn send_slippage_menu(bot: &Bot, chat_id: ChatId) -> BotResult {
    let kb = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("30%", "slippage_30"),
            InlineKeyboardButton::callback("50%", "slippage_50"),
            InlineKeyboardButton::callback("100%", "slippage_100"),
        ],
        vec![InlineKeyboardButton::callback("Custom", "slippage_custom")],
    ]);
    bot.send_message(chat_id, "Set slippage").reply_markup(kb).await?;
    Ok(())
}

async fn send_take_profit_menu(bot: &Bot, chat_id: ChatId) -> BotResult {
    let kb = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("110%", "tp_110"),
            InlineKeyboardButton::callback("120%", "tp_120"),
            InlineKeyboardButton::callback("150%", "tp_150"),
        ],
        vec![InlineKeyboardButton::callback("Custom", "tp_custom")],
    ]);
    bot.send_message(chat_id, "Set take profit %\n(e.g. 120 = sell when price reaches 120% of buy price)")
        .reply_markup(kb).await?;
    Ok(())
}

async fn send_stop_loss_menu(bot: &Bot, chat_id: ChatId) -> BotResult {
    let kb = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("70%", "sl_70"),
            InlineKeyboardButton::callback("80%", "sl_80"),
            InlineKeyboardButton::callback("90%", "sl_90"),
        ],
        vec![InlineKeyboardButton::callback("Custom", "sl_custom")],
    ]);
    bot.send_message(chat_id, "Set stop loss %\n(e.g. 80 = sell when price drops to 80% of buy price)")
        .reply_markup(kb).await?;
    Ok(())
}

async fn send_trailing_menu(bot: &Bot, chat_id: ChatId) -> BotResult {
    let kb = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("105%", "trail_105"),
            InlineKeyboardButton::callback("110%", "trail_110"),
            InlineKeyboardButton::callback("120%", "trail_120"),
        ],
        vec![InlineKeyboardButton::callback("Custom", "trail_custom")],
    ]);
    bot.send_message(chat_id, "Set trailing trigger %\n(e.g. 110 = activate trailing when price reaches 110% of buy price)")
        .reply_markup(kb).await?;
    Ok(())
}

async fn send_trailing_stop_menu(bot: &Bot, chat_id: ChatId) -> BotResult {
    let kb = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("5%", "ts_5"),
            InlineKeyboardButton::callback("10%", "ts_10"),
            InlineKeyboardButton::callback("20%", "ts_20"),
        ],
        vec![InlineKeyboardButton::callback("Custom", "ts_custom")],
    ]);
    bot.send_message(chat_id, "Set trailing stop %\n(e.g. 10 = sell when price drops 10% from peak)")
        .reply_markup(kb).await?;
    Ok(())
}

async fn send_priority_fee_menu(bot: &Bot, chat_id: ChatId) -> BotResult {
    let kb = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("50000", "priority_50000"),
            InlineKeyboardButton::callback("100000", "priority_100000"),
            InlineKeyboardButton::callback("200000", "priority_200000"),
        ],
        vec![InlineKeyboardButton::callback("Custom", "priority_custom")],
    ]);
    bot.send_message(
        chat_id,
        "Set priority fee in micro-lamports\nFormula: priority_fee = cu * micro_lamports / 1e15, cu=200000",
    )
    .reply_markup(kb)
    .await?;
    Ok(())
}

async fn send_tip_fee_menu(bot: &Bot, chat_id: ChatId) -> BotResult {
    let kb = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("0.0005 SOL", "tip_0.0005"),
            InlineKeyboardButton::callback("0.001 SOL", "tip_0.001"),
            InlineKeyboardButton::callback("0.002 SOL", "tip_0.002"),
        ],
        vec![InlineKeyboardButton::callback("Custom", "tip_custom")],
    ]);
    bot.send_message(chat_id, "Set tip fee (SOL)")
        .reply_markup(kb)
        .await?;
    Ok(())
}

async fn command_generate_wallet(
    bot: &Bot,
    chat_id: ChatId,
    telegram_user_id: i64,
    state: &Arc<RwLock<TelegramUiState>>,
    db: &DatabaseConnection,
    wallet_encryption_password: &str,
) -> BotResult {
    let wallet = Keypair::new();
    let private_key = wallet.to_base58_string();
    let pubkey = wallet.pubkey().to_string();

    let created = create_wallet(
        db,
        telegram_user_id,
        &pubkey,
        &private_key,
        wallet_encryption_password,
    )
    .await?;

    let mut guard = state.write().await;
    guard.wallets.push(WalletEntry {
        wallet_id: created.wallet_id,
        pubkey: created.pubkey.clone(),
        private_key: created.private_key,
    });
    if created.is_selected {
        guard.selected_wallet_id = Some(created.wallet_id);
    }
    drop(guard);
    sync_bot_run_state_from_ui(state).await;

    let pk_safe = escape_html_for_telegram(&pubkey);
    let sk_spoiler = format_private_key_spoiler_html(&private_key);
    bot.send_message(
        chat_id,
        format!(
            "Generated wallet {}\n\n\
🔑 Pubkey:\n<code>{}</code>\n\n\
🔐 Private key (tap to reveal):\n\
{}",
            format_wallet_no(created.wallet_id),
            pk_safe,
            sk_spoiler,
        ),
    )
    .parse_mode(teloxide::types::ParseMode::Html)
    .await?;
    Ok(())
}

async fn import_private_key_and_reply(
    bot: &Bot,
    chat_id: ChatId,
    telegram_user_id: i64,
    state: &Arc<RwLock<TelegramUiState>>,
    db: &DatabaseConnection,
    wallet_encryption_password: &str,
    key: &str,
) -> BotResult {
    let keypair = match parse_private_key(key) {
        Ok(k) => k,
        Err(e) => {
            bot.send_message(chat_id, format!("Import failed: {e}")).await?;
            return Ok(());
        }
    };
    let pubkey = keypair.pubkey().to_string();
    let private_key = keypair.to_base58_string();
    let created = create_wallet(
        db,
        telegram_user_id,
        &pubkey,
        &private_key,
        wallet_encryption_password,
    )
    .await?;

    let mut guard = state.write().await;
    guard.wallets.push(WalletEntry {
        wallet_id: created.wallet_id,
        pubkey: created.pubkey.clone(),
        private_key: created.private_key,
    });
    if created.is_selected {
        guard.selected_wallet_id = Some(created.wallet_id);
    }
    drop(guard);
    sync_bot_run_state_from_ui(state).await;

    bot.send_message(
        chat_id,
        format!(
            "Imported wallet {} <code>{}</code>",
            format_wallet_no(created.wallet_id),
            escape_html_for_telegram(&created.pubkey),
        ),
    )
    .parse_mode(teloxide::types::ParseMode::Html)
        .await?;
    Ok(())
}

fn parse_private_key(raw: &str) -> Result<Keypair, &'static str> {
    let normalized = raw.trim();
    if normalized.is_empty() {
        return Err("private key is empty");
    }
    let bytes = bs58::decode(normalized)
        .into_vec()
        .map_err(|_| "invalid base58 private key")?;
    let key_bytes: [u8; 64] = bytes
        .try_into()
        .map_err(|_| "private key must decode to 64 bytes")?;
    Keypair::try_from(&key_bytes[..]).map_err(|_| "invalid private key bytes")
}

async fn command_select_wallet(
    bot: &Bot,
    chat_id: ChatId,
    telegram_user_id: i64,
    state: &Arc<RwLock<TelegramUiState>>,
    db: &DatabaseConnection,
    text: &str,
) -> BotResult {
    let wallet_id = parse_suffix_index(text, "/select_")? as i32;
    if !set_selected_wallet(db, telegram_user_id, wallet_id).await? {
        bot.send_message(chat_id, "Invalid wallet id").await?;
        return Ok(());
    }

    let mut guard = state.write().await;
    guard.selected_wallet_id = Some(wallet_id);
    let pk = guard
        .wallets
        .iter()
        .find(|w| w.wallet_id == wallet_id)
        .map(|w| w.pubkey.clone())
        .unwrap_or_else(|| "unknown".to_string());
    drop(guard);
    sync_bot_run_state_from_ui(state).await;
    let pk_safe = escape_html_for_telegram(&pk);
    bot.send_message(
        chat_id,
        format!("Selected wallet {} <code>{}</code>", format_wallet_no(wallet_id), pk_safe),
    )
    .parse_mode(teloxide::types::ParseMode::Html)
    .await?;
    Ok(())
}

async fn command_delete_wallet(
    bot: &Bot,
    chat_id: ChatId,
    telegram_user_id: i64,
    state: &Arc<RwLock<TelegramUiState>>,
    db: &DatabaseConnection,
    wallet_encryption_password: &str,
    text: &str,
) -> BotResult {
    let wallet_id = parse_suffix_index(text, "/delete_")? as i32;
    if !delete_wallet(db, telegram_user_id, wallet_id).await? {
        bot.send_message(chat_id, "Invalid wallet id").await?;
        return Ok(());
    }

    let wallets = load_wallets(db, telegram_user_id, wallet_encryption_password).await?;
    let mut guard = state.write().await;
    guard.wallets = wallets
        .iter()
        .map(|w| WalletEntry {
            wallet_id: w.wallet_id,
            pubkey: w.pubkey.clone(),
            private_key: w.private_key.clone(),
        })
        .collect();
    guard.selected_wallet_id = wallets.iter().find(|w| w.is_selected).map(|w| w.wallet_id);
    drop(guard);
    sync_bot_run_state_from_ui(state).await;

    bot.send_message(
        chat_id,
        format!("Deleted wallet {}", format_wallet_no(wallet_id)),
    )
        .await?;
    Ok(())
}

async fn command_show_wallet_key(
    bot: &Bot,
    chat_id: ChatId,
    telegram_user_id: i64,
    db: &DatabaseConnection,
    wallet_encryption_password: &str,
    text: &str,
) -> BotResult {
    let wallet_id = parse_suffix_index(text, "/show_key_")? as i32;
    let maybe_wallet =
        load_wallet_by_wallet_id(db, telegram_user_id, wallet_id, wallet_encryption_password)
            .await?;
    let Some(wallet) = maybe_wallet else {
        bot.send_message(chat_id, "Invalid wallet id").await?;
        return Ok(());
    };

    let pk_safe = escape_html_for_telegram(&wallet.pubkey);
    let sk = format_private_key_spoiler_html(&wallet.private_key);
    bot.send_message(
        chat_id,
        format!(
            "Wallet {} pubkey:\n<code>{}</code>\n\nPrivate key (tap to reveal):\n{}",
            format_wallet_no(wallet.wallet_id),
            pk_safe,
            sk
        ),
    )
    .parse_mode(teloxide::types::ParseMode::Html)
    .await?;
    Ok(())
}

fn parse_suffix_index(text: &str, prefix: &str) -> BotValueResult<usize> {
    let Some(suffix) = text.strip_prefix(prefix) else {
        return Err("invalid command".into());
    };
    let idx = suffix
        .parse::<usize>()
        .map_err(|_| "invalid wallet index, use /command_N format")?;
    Ok(idx)
}

async fn handle_pending_input(
    bot: &Bot,
    chat_id: ChatId,
    telegram_user_id: i64,
    state: &Arc<RwLock<TelegramUiState>>,
    db: &DatabaseConnection,
    wallet_encryption_password: &str,
    text: &str,
    pending: PendingInput,
) -> BotResult {
    match pending {
        PendingInput::ImportPrivateKey => {
            state.write().await.pending_input = PendingInput::None;
            import_private_key_and_reply(
                bot,
                chat_id,
                telegram_user_id,
                state,
                db,
                wallet_encryption_password,
                text,
            )
            .await?;
        }
        PendingInput::BuyAmountCustom => {
            let parsed = text.parse::<f64>();
            let Ok(amount) = parsed else {
                bot.send_message(chat_id, "Invalid SOL amount").await?;
                return Ok(());
            };
            state.write().await.pending_input = PendingInput::None;
            set_buy_amount_and_reply(bot, chat_id, telegram_user_id, state, db, amount).await?;
        }
        PendingInput::SlippageCustom => {
            let parsed = text.parse::<u32>();
            let Ok(percent) = parsed else {
                bot.send_message(chat_id, "Invalid slippage percent").await?;
                return Ok(());
            };
            state.write().await.pending_input = PendingInput::None;
            set_slippage_and_reply(bot, chat_id, telegram_user_id, state, db, percent).await?;
        }
        PendingInput::TakeProfitCustom => {
            let Ok(val) = text.parse::<f64>() else {
                bot.send_message(chat_id, "Invalid take profit value").await?;
                return Ok(());
            };
            state.write().await.pending_input = PendingInput::None;
            set_take_profit_and_reply(bot, chat_id, telegram_user_id, state, db, val).await?;
        }
        PendingInput::StopLossCustom => {
            let Ok(val) = text.parse::<f64>() else {
                bot.send_message(chat_id, "Invalid stop loss value").await?;
                return Ok(());
            };
            state.write().await.pending_input = PendingInput::None;
            set_stop_loss_and_reply(bot, chat_id, telegram_user_id, state, db, val).await?;
        }
        PendingInput::TrailingCustom => {
            let Ok(val) = text.parse::<f64>() else {
                bot.send_message(chat_id, "Invalid trailing value").await?;
                return Ok(());
            };
            state.write().await.pending_input = PendingInput::None;
            set_trailing_and_reply(bot, chat_id, telegram_user_id, state, db, val).await?;
        }
        PendingInput::TrailingStopCustom => {
            let Ok(val) = text.parse::<f64>() else {
                bot.send_message(chat_id, "Invalid trailing stop value").await?;
                return Ok(());
            };
            state.write().await.pending_input = PendingInput::None;
            set_trailing_stop_and_reply(bot, chat_id, telegram_user_id, state, db, val).await?;
        }
        PendingInput::PriorityFeeCustom => {
            let parsed = text.parse::<u64>();
            let Ok(priority_fee_micro_lamports) = parsed else {
                bot.send_message(chat_id, "Invalid priority fee micro-lamports")
                    .await?;
                return Ok(());
            };
            state.write().await.pending_input = PendingInput::None;
            set_priority_fee_and_reply(
                bot,
                chat_id,
                telegram_user_id,
                state,
                db,
                priority_fee_micro_lamports,
            )
            .await?;
        }
        PendingInput::TipFeeCustom => {
            let parsed = text.parse::<f64>();
            let Ok(tip_fee_sol) = parsed else {
                bot.send_message(chat_id, "Invalid tip fee SOL").await?;
                return Ok(());
            };
            state.write().await.pending_input = PendingInput::None;
            set_tip_fee_and_reply(bot, chat_id, telegram_user_id, state, db, tip_fee_sol).await?;
        }
        PendingInput::None => {}
    }
    Ok(())
}

async fn set_buy_amount_and_reply(
    bot: &Bot,
    chat_id: ChatId,
    telegram_user_id: i64,
    state: &Arc<RwLock<TelegramUiState>>,
    db: &DatabaseConnection,
    value: f64,
) -> BotResult {
    if value <= 0.0 {
        bot.send_message(chat_id, "Buy amount must be > 0").await?;
        return Ok(());
    }
    state.write().await.trading.buy_amount_sol = value;
    log_trading_param_changed("buy_amount_sol", value);
    persist_trading_state(db, telegram_user_id, state).await?;
    sync_bot_run_state_from_ui(state).await;
    bot.send_message(chat_id, format!("Buy amount updated: {value} SOL"))
        .await?;
    Ok(())
}

async fn set_slippage_and_reply(
    bot: &Bot,
    chat_id: ChatId,
    telegram_user_id: i64,
    state: &Arc<RwLock<TelegramUiState>>,
    db: &DatabaseConnection,
    value: u32,
) -> BotResult {
    if value == 0 {
        bot.send_message(chat_id, "Slippage must be > 0").await?;
        return Ok(());
    }
    state.write().await.trading.slippage_percent = value;
    log_trading_param_changed("slippage_percent", value);
    persist_trading_state(db, telegram_user_id, state).await?;
    sync_bot_run_state_from_ui(state).await;
    bot.send_message(chat_id, format!("Slippage updated: {value}%"))
        .await?;
    Ok(())
}

async fn set_take_profit_and_reply(
    bot: &Bot,
    chat_id: ChatId,
    telegram_user_id: i64,
    state: &Arc<RwLock<TelegramUiState>>,
    db: &DatabaseConnection,
    value: f64,
) -> BotResult {
    if value <= 0.0 {
        bot.send_message(chat_id, "Take profit must be > 0").await?;
        return Ok(());
    }
    state.write().await.trading.take_profit = value;
    log_trading_param_changed("take_profit", value);
    persist_trading_state(db, telegram_user_id, state).await?;
    sync_bot_run_state_from_ui(state).await;
    bot.send_message(chat_id, format!("Take profit updated: {value}%"))
        .await?;
    Ok(())
}

async fn set_stop_loss_and_reply(
    bot: &Bot,
    chat_id: ChatId,
    telegram_user_id: i64,
    state: &Arc<RwLock<TelegramUiState>>,
    db: &DatabaseConnection,
    value: f64,
) -> BotResult {
    if value <= 0.0 {
        bot.send_message(chat_id, "Stop loss must be > 0").await?;
        return Ok(());
    }
    state.write().await.trading.stop_loss = value;
    log_trading_param_changed("stop_loss", value);
    persist_trading_state(db, telegram_user_id, state).await?;
    sync_bot_run_state_from_ui(state).await;
    bot.send_message(chat_id, format!("Stop loss updated: {value}%"))
        .await?;
    Ok(())
}

async fn set_trailing_and_reply(
    bot: &Bot,
    chat_id: ChatId,
    telegram_user_id: i64,
    state: &Arc<RwLock<TelegramUiState>>,
    db: &DatabaseConnection,
    value: f64,
) -> BotResult {
    if value <= 0.0 {
        bot.send_message(chat_id, "Trailing trigger must be > 0").await?;
        return Ok(());
    }
    state.write().await.trading.trailing = value;
    log_trading_param_changed("trailing", value);
    persist_trading_state(db, telegram_user_id, state).await?;
    sync_bot_run_state_from_ui(state).await;
    bot.send_message(chat_id, format!("Trailing trigger updated: {value}%"))
        .await?;
    Ok(())
}

async fn set_trailing_stop_and_reply(
    bot: &Bot,
    chat_id: ChatId,
    telegram_user_id: i64,
    state: &Arc<RwLock<TelegramUiState>>,
    db: &DatabaseConnection,
    value: f64,
) -> BotResult {
    if value <= 0.0 {
        bot.send_message(chat_id, "Trailing stop must be > 0").await?;
        return Ok(());
    }
    state.write().await.trading.trailing_stop = value;
    log_trading_param_changed("trailing_stop", value);
    persist_trading_state(db, telegram_user_id, state).await?;
    sync_bot_run_state_from_ui(state).await;
    bot.send_message(chat_id, format!("Trailing stop updated: {value}%"))
        .await?;
    Ok(())
}

async fn set_priority_fee_and_reply(
    bot: &Bot,
    chat_id: ChatId,
    telegram_user_id: i64,
    state: &Arc<RwLock<TelegramUiState>>,
    db: &DatabaseConnection,
    value: u64,
) -> BotResult {
    if value == 0 {
        bot.send_message(chat_id, "Priority fee micro-lamports must be > 0")
            .await?;
        return Ok(());
    }
    state.write().await.trading.priority_fee_micro_lamports = value;
    log_trading_param_changed("priority_fee_micro_lamports", value);
    persist_trading_state(db, telegram_user_id, state).await?;
    sync_bot_run_state_from_ui(state).await;
    let sol = priority_fee_sol_from_micro_lamports(value, default_cu());
    bot.send_message(
        chat_id,
        format!(
            "Priority fee updated: {} micro-lamports (~{} SOL with cu={})",
            value,
            sol,
            default_cu()
        ),
    )
    .await?;
    Ok(())
}

async fn set_tip_fee_and_reply(
    bot: &Bot,
    chat_id: ChatId,
    telegram_user_id: i64,
    state: &Arc<RwLock<TelegramUiState>>,
    db: &DatabaseConnection,
    value: f64,
) -> BotResult {
    if value <= 0.0 {
        bot.send_message(chat_id, "Tip fee must be > 0 SOL").await?;
        return Ok(());
    }
    state.write().await.trading.tip_fee_sol = value;
    log_trading_param_changed("tip_fee_sol", value);
    persist_trading_state(db, telegram_user_id, state).await?;
    sync_bot_run_state_from_ui(state).await;
    bot.send_message(chat_id, format!("Tip fee updated: {value} SOL"))
        .await?;
    Ok(())
}

async fn bootstrap_state_from_db(
    db: &DatabaseConnection,
    telegram_user_id: i64,
    wallet_encryption_password: &str,
    state: Arc<RwLock<TelegramUiState>>,
) -> BotResult {
    let wallets = load_wallets(db, telegram_user_id, wallet_encryption_password).await?;
    let (trading, _is_first_run) = load_or_create_trading_parameters(
        db,
        telegram_user_id,
        default_buy_amount_sol(),
        default_slippage_percent(),
        default_take_profit(),
        default_stop_loss(),
        default_trailing(),
        default_trailing_stop(),
        default_priority_fee_micro_lamport(),
        default_third_party_fee(),
    )
    .await?;

    let mut guard = state.write().await;
    guard.wallets = wallets
        .iter()
        .map(|w| WalletEntry {
            wallet_id: w.wallet_id,
            pubkey: w.pubkey.clone(),
            private_key: w.private_key.clone(),
        })
        .collect();
    guard.selected_wallet_id = wallets.iter().find(|w| w.is_selected).map(|w| w.wallet_id);
    guard.trading.buy_amount_sol = trading.buy_amount_sol;
    guard.trading.slippage_percent = trading.slippage_percent;
    guard.trading.take_profit = trading.take_profit;
    guard.trading.stop_loss = trading.stop_loss;
    guard.trading.trailing = trading.trailing;
    guard.trading.trailing_stop = trading.trailing_stop;
    guard.trading.priority_fee_micro_lamports = trading.priority_fee_micro_lamports;
    guard.trading.tip_fee_sol = trading.tip_fee_sol;
    guard.is_running = trading.is_running;
    drop(guard);
    sync_bot_run_state_from_ui(&state).await;
    Ok(())
}

async fn persist_trading_state(
    db: &DatabaseConnection,
    telegram_user_id: i64,
    state: &Arc<RwLock<TelegramUiState>>,
) -> BotResult {
    let snapshot = {
        let guard = state.read().await;
        TradingParameterRecord {
            buy_amount_sol: guard.trading.buy_amount_sol,
            slippage_percent: guard.trading.slippage_percent,
            take_profit: guard.trading.take_profit,
            stop_loss: guard.trading.stop_loss,
            trailing: guard.trading.trailing,
            trailing_stop: guard.trading.trailing_stop,
            priority_fee_micro_lamports: guard.trading.priority_fee_micro_lamports,
            tip_fee_sol: guard.trading.tip_fee_sol,
            is_running: guard.is_running,
        }
    };
    save_trading_parameters(db, telegram_user_id, &snapshot).await?;
    Ok(())
}

async fn sync_bot_run_state_from_ui(state: &Arc<RwLock<TelegramUiState>>) {
    let snapshot = state.read().await.clone();
    let selected = snapshot
        .selected_wallet_id
        .and_then(|id| snapshot.wallets.iter().find(|w| w.wallet_id == id).cloned());

    let mut run = BOT_RUN_STATE.write().await;
    run.is_running = snapshot.is_running;
    run.selected_wallet_pubkey = selected.as_ref().map(|w| w.pubkey.clone());
    run.selected_wallet_private_key = selected.as_ref().map(|w| w.private_key.clone());
    run.trading.buy_amount_sol = snapshot.trading.buy_amount_sol;
    run.trading.slippage = snapshot.trading.slippage_percent as f64;
    run.trading.take_profit = snapshot.trading.take_profit;
    run.trading.stop_loss = snapshot.trading.stop_loss;
    run.trading.trailing = snapshot.trading.trailing;
    run.trading.trailing_stop = snapshot.trading.trailing_stop;
    run.trading.priority_fee_micro_lamports = snapshot.trading.priority_fee_micro_lamports;
    run.trading.tip_fee_sol = snapshot.trading.tip_fee_sol;
}

/// Hiển thị menu Anti-Rug với trạng thái hiện tại và nút toggle.
async fn send_anti_rug_menu(
    bot: &Bot,
    chat_id: ChatId,
    _state: &Arc<RwLock<TelegramUiState>>,
) -> BotResult {
    let run = BOT_RUN_STATE.read().await;
    let cfg = &run.anti_rug;

    let on_off = |b: bool| if b { "✅ ON" } else { "❌ OFF" };

    let text = format!(
        "🛡️ Anti-Rug Intelligence Layer\n\n\
         Master: {}\n\
         Mode: {}\n\n\
         M1 Holder Filter: {} (max {}%)\n\
         M2 Panic-Sell: {}\n\
         M3 Dev Profiler: {} (min {}TX, {}h)\n\
         M4 Genesis Detector: {}\n\
         M5 Metadata Checker: {} ({})\n\n\
         Timeout: {}ms | Jito tip: {} SOL",
        on_off(cfg.enabled),
        if cfg.warn_only { "⚠️ WARN ONLY" } else { "🚫 BLOCK" },
        on_off(cfg.holder_filter_enabled), cfg.max_top10_holder_pct,
        on_off(cfg.panic_sell_enabled),
        on_off(cfg.dev_profiler_enabled), cfg.min_dev_tx_count, cfg.min_wallet_age_hours,
        on_off(cfg.genesis_detector_enabled),
        on_off(cfg.metadata_checker_enabled), cfg.metadata_empty_action.as_str(),
        cfg.filter_timeout_ms,
        cfg.panic_sell_jito_tip_lamports as f64 / 1_000_000_000.0,
    );
    drop(run);

    let kb = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("🔌 Master ON/OFF", "ar_master_toggle"),
            InlineKeyboardButton::callback("⚠️ Warn/Block", "ar_warn_toggle"),
        ],
        vec![
            InlineKeyboardButton::callback("📊 M1 Holder", "ar_holder_toggle"),
            InlineKeyboardButton::callback("🚨 M2 Panic", "ar_panic_toggle"),
        ],
        vec![
            InlineKeyboardButton::callback("👤 M3 Dev", "ar_dev_toggle"),
            InlineKeyboardButton::callback("🔍 M4 Genesis", "ar_genesis_toggle"),
        ],
        vec![
            InlineKeyboardButton::callback("📝 M5 Metadata", "ar_meta_toggle"),
        ],
        vec![
            InlineKeyboardButton::callback("30%", "ar_holder30"),
            InlineKeyboardButton::callback("40%", "ar_holder40"),
            InlineKeyboardButton::callback("50%", "ar_holder50"),
        ],
    ]);

    bot.send_message(chat_id, text)
        .reply_markup(kb)
        .await?;

    Ok(())
}
