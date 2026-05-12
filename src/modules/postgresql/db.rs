use crate::encryption::{decrypt_private_key, encrypt_private_key};
use super::entities::{anti_rug_filter_log, skipped_tokens_log, trade_history, trading_parameter, wallet};
use super::migration::Migrator;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue::Set, Database, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use sea_orm_migration::MigratorTrait;
use std::env;
use tokio::sync::OnceCell;

type PgResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Fix BUG-2: Shared DB connection pool — khởi tạo 1 lần, tái sử dụng.
/// Tránh mở connection mới mỗi khi log filter result.
static SHARED_DB_POOL: OnceCell<DatabaseConnection> = OnceCell::const_new();

/// Lấy hoặc khởi tạo shared DB connection pool.
pub async fn get_shared_db() -> PgResult<&'static DatabaseConnection> {
    SHARED_DB_POOL
        .get_or_try_init(|| async {
            let db_url = resolve_database_url_from_env()?;
            let db = Database::connect(&db_url).await?;
            Ok(db)
        })
        .await
}

#[derive(Clone, Debug)]
pub struct WalletRecord {
    pub wallet_id: i32,
    pub pubkey: String,
    pub private_key: String,
    pub is_selected: bool,
}

#[derive(Clone, Debug)]
pub struct TradingParameterRecord {
    pub buy_amount_sol: f64,
    pub slippage_percent: u32,
    pub take_profit: f64,
    pub stop_loss: f64,
    pub trailing: f64,
    pub trailing_stop: f64,
    pub priority_fee_micro_lamports: u64,
    pub tip_fee_sol: f64,
    pub is_running: bool,
}

pub async fn init_postgres_and_migrate(database_url: &str) -> PgResult<DatabaseConnection> {
    let db = Database::connect(database_url).await?;
    Migrator::up(&db, None).await?;
    Ok(db)
}

pub fn resolve_database_url_from_env() -> PgResult<String> {
    let user = env::var("POSTGRES_USER")
        .map_err(|_| "Missing POSTGRES_USER in .env")?;
    let password = env::var("POSTGRES_PASSWORD")
        .map_err(|_| "Missing POSTGRES_PASSWORD in .env")?;
    let db_name = env::var("POSTGRES_DB")
        .map_err(|_| "Missing POSTGRES_DB in .env")?;
    let host = env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("POSTGRES_PORT").unwrap_or_else(|_| "5432".to_string());

    Ok(format!(
        "postgres://{}:{}@{}:{}/{}",
        user, password, host, port, db_name
    ))
}

pub async fn load_wallets(
    db: &DatabaseConnection,
    telegram_user_id: i64,
    encryption_password: &str,
) -> PgResult<Vec<WalletRecord>> {
    let rows = wallet::Entity::find()
        .filter(wallet::Column::TelegramUserId.eq(telegram_user_id))
        .order_by_asc(wallet::Column::WalletId)
        .all(db)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(WalletRecord {
            wallet_id: row.wallet_id,
            pubkey: row.pubkey,
            private_key: decrypt_private_key(&row.private_key_encrypted, encryption_password)?,
            is_selected: row.is_selected,
        });
    }
    Ok(out)
}

pub async fn create_wallet(
    db: &DatabaseConnection,
    telegram_user_id: i64,
    pubkey: &str,
    private_key: &str,
    encryption_password: &str,
) -> PgResult<WalletRecord> {
    let max_wallet = wallet::Entity::find()
        .filter(wallet::Column::TelegramUserId.eq(telegram_user_id))
        .order_by_desc(wallet::Column::WalletId)
        .one(db)
        .await?;

    let next_wallet_id = max_wallet.map(|m| m.wallet_id + 1).unwrap_or(1);
    let should_select = next_wallet_id == 1;
    let encrypted = encrypt_private_key(private_key, encryption_password)?;

    let active = wallet::ActiveModel {
        wallet_id: Set(next_wallet_id),
        telegram_user_id: Set(telegram_user_id),
        private_key_encrypted: Set(encrypted),
        pubkey: Set(pubkey.to_string()),
        is_selected: Set(should_select),
        ..Default::default()
    };
    active.insert(db).await?;

    Ok(WalletRecord {
        wallet_id: next_wallet_id,
        pubkey: pubkey.to_string(),
        private_key: private_key.to_string(),
        is_selected: should_select,
    })
}

pub async fn set_selected_wallet(
    db: &DatabaseConnection,
    telegram_user_id: i64,
    wallet_id: i32,
) -> PgResult<bool> {
    let maybe_wallet = wallet::Entity::find()
        .filter(wallet::Column::TelegramUserId.eq(telegram_user_id))
        .filter(wallet::Column::WalletId.eq(wallet_id))
        .one(db)
        .await?;
    if maybe_wallet.is_none() {
        return Ok(false);
    }

    let wallets = wallet::Entity::find()
        .filter(wallet::Column::TelegramUserId.eq(telegram_user_id))
        .all(db)
        .await?;
    for row in wallets {
        let mut active: wallet::ActiveModel = row.into();
        active.is_selected = Set(active.wallet_id.as_ref() == &wallet_id);
        active.update(db).await?;
    }
    Ok(true)
}

pub async fn delete_wallet(
    db: &DatabaseConnection,
    telegram_user_id: i64,
    wallet_id: i32,
) -> PgResult<bool> {
    let row = wallet::Entity::find()
        .filter(wallet::Column::TelegramUserId.eq(telegram_user_id))
        .filter(wallet::Column::WalletId.eq(wallet_id))
        .one(db)
        .await?;
    let Some(row) = row else {
        return Ok(false);
    };

    let active: wallet::ActiveModel = row.into();
    active.delete(db).await?;

    resequence_wallet_ids(db, telegram_user_id).await?;
    ensure_wallet_selected(db, telegram_user_id).await?;
    Ok(true)
}

pub async fn load_wallet_by_wallet_id(
    db: &DatabaseConnection,
    telegram_user_id: i64,
    wallet_id: i32,
    encryption_password: &str,
) -> PgResult<Option<WalletRecord>> {
    let row = wallet::Entity::find()
        .filter(wallet::Column::TelegramUserId.eq(telegram_user_id))
        .filter(wallet::Column::WalletId.eq(wallet_id))
        .one(db)
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    Ok(Some(WalletRecord {
        wallet_id: row.wallet_id,
        pubkey: row.pubkey,
        private_key: decrypt_private_key(&row.private_key_encrypted, encryption_password)?,
        is_selected: row.is_selected,
    }))
}

pub async fn load_or_create_trading_parameters(
    db: &DatabaseConnection,
    telegram_user_id: i64,
    default_buy_amount_sol: f64,
    default_slippage_percent: u32,
    default_take_profit: f64,
    default_stop_loss: f64,
    default_trailing: f64,
    default_trailing_stop: f64,
    default_priority_fee_micro_lamports: u64,
    default_tip_fee_sol: f64,
) -> PgResult<(TradingParameterRecord, bool)> {
    if let Some(row) = trading_parameter::Entity::find()
        .filter(trading_parameter::Column::TelegramUserId.eq(telegram_user_id))
        .one(db)
        .await?
    {
        return Ok((
            TradingParameterRecord {
                buy_amount_sol: row.buy_amount_sol,
                slippage_percent: row.slippage_percent as u32,
                take_profit: row.take_profit,
                stop_loss: row.stop_loss,
                trailing: row.trailing,
                trailing_stop: row.trailing_stop,
                priority_fee_micro_lamports: row.priority_fee_micro_lamports as u64,
                tip_fee_sol: row.tip_fee_sol,
                is_running: row.is_running,
            },
            false,
        ));
    }

    let active = trading_parameter::ActiveModel {
        telegram_user_id: Set(telegram_user_id),
        buy_amount_sol: Set(default_buy_amount_sol),
        slippage_percent: Set(default_slippage_percent as i32),
        take_profit: Set(default_take_profit),
        stop_loss: Set(default_stop_loss),
        trailing: Set(default_trailing),
        trailing_stop: Set(default_trailing_stop),
        priority_fee_micro_lamports: Set(default_priority_fee_micro_lamports as i64),
        tip_fee_sol: Set(default_tip_fee_sol),
        is_running: Set(false),
        ..Default::default()
    };
    active.insert(db).await?;

    Ok((
        TradingParameterRecord {
            buy_amount_sol: default_buy_amount_sol,
            slippage_percent: default_slippage_percent,
            take_profit: default_take_profit,
            stop_loss: default_stop_loss,
            trailing: default_trailing,
            trailing_stop: default_trailing_stop,
            priority_fee_micro_lamports: default_priority_fee_micro_lamports,
            tip_fee_sol: default_tip_fee_sol,
            is_running: false,
        },
        true,
    ))
}

pub async fn save_trading_parameters(
    db: &DatabaseConnection,
    telegram_user_id: i64,
    rec: &TradingParameterRecord,
) -> PgResult<()> {
    let row = trading_parameter::Entity::find()
        .filter(trading_parameter::Column::TelegramUserId.eq(telegram_user_id))
        .one(db)
        .await?;
    let Some(row) = row else {
        let active = trading_parameter::ActiveModel {
            telegram_user_id: Set(telegram_user_id),
            buy_amount_sol: Set(rec.buy_amount_sol),
            slippage_percent: Set(rec.slippage_percent as i32),
            take_profit: Set(rec.take_profit),
            stop_loss: Set(rec.stop_loss),
            trailing: Set(rec.trailing),
            trailing_stop: Set(rec.trailing_stop),
            priority_fee_micro_lamports: Set(rec.priority_fee_micro_lamports as i64),
            tip_fee_sol: Set(rec.tip_fee_sol),
            is_running: Set(rec.is_running),
            ..Default::default()
        };
        active.insert(db).await?;
        return Ok(());
    };

    let mut active: trading_parameter::ActiveModel = row.into();
    active.buy_amount_sol = Set(rec.buy_amount_sol);
    active.slippage_percent = Set(rec.slippage_percent as i32);
    active.take_profit = Set(rec.take_profit);
    active.stop_loss = Set(rec.stop_loss);
    active.trailing = Set(rec.trailing);
    active.trailing_stop = Set(rec.trailing_stop);
    active.priority_fee_micro_lamports = Set(rec.priority_fee_micro_lamports as i64);
    active.tip_fee_sol = Set(rec.tip_fee_sol);
    active.is_running = Set(rec.is_running);
    active.update(db).await?;
    Ok(())
}

async fn resequence_wallet_ids(db: &DatabaseConnection, telegram_user_id: i64) -> PgResult<()> {
    let rows = wallet::Entity::find()
        .filter(wallet::Column::TelegramUserId.eq(telegram_user_id))
        .order_by_asc(wallet::Column::WalletId)
        .all(db)
        .await?;
    for (index, row) in rows.into_iter().enumerate() {
        let mut active: wallet::ActiveModel = row.into();
        active.wallet_id = Set(index as i32 + 1);
        active.update(db).await?;
    }
    Ok(())
}

async fn ensure_wallet_selected(db: &DatabaseConnection, telegram_user_id: i64) -> PgResult<()> {
    let rows = wallet::Entity::find()
        .filter(wallet::Column::TelegramUserId.eq(telegram_user_id))
        .order_by_asc(wallet::Column::WalletId)
        .all(db)
        .await?;
    if rows.is_empty() {
        return Ok(());
    }
    if rows.iter().any(|r| r.is_selected) {
        return Ok(());
    }

    let first_wallet_id = rows[0].wallet_id;
    for row in rows {
        let mut active: wallet::ActiveModel = row.into();
        active.is_selected = Set(active.wallet_id.as_ref() == &first_wallet_id);
        active.update(db).await?;
    }
    Ok(())
}

// ── Anti-Rug Filter Logging ───────────────────────────────────────────────────

/// Record kết quả anti-rug filter cho một token migration.
/// Fire-and-forget — không block execution flow.
pub async fn log_anti_rug_filter_result(
    db: &DatabaseConnection,
    token_mint: &str,
    verdict: &str,
    reject_reason: Option<String>,
    top10_holder_pct: Option<f64>,
    dev_tx_count: Option<u64>,
    genesis_buy_pct: Option<f64>,
    genesis_bundle_detected: bool,
    has_metadata_uri: bool,
    filter_duration_ms: u64,
) -> PgResult<()> {
    let active = anti_rug_filter_log::ActiveModel {
        token_mint: Set(token_mint.to_string()),
        created_at: Set(chrono::Utc::now().into()),
        verdict: Set(verdict.to_string()),
        reject_reason: Set(reject_reason),
        top10_holder_pct: Set(top10_holder_pct),
        dev_tx_count: Set(dev_tx_count.map(|v| v as i64)),
        genesis_buy_pct: Set(genesis_buy_pct),
        genesis_bundle_detected: Set(genesis_bundle_detected),
        has_metadata_uri: Set(has_metadata_uri),
        filter_duration_ms: Set(Some(filter_duration_ms as i64)),
        ..Default::default()
    };
    active.insert(db).await?;
    Ok(())
}

// ── Skipped Tokens Logging ────────────────────────────────────────────────────

/// Lưu token bị skip bởi Anti-Rug filter (Block mode) vào database.
/// Fire-and-forget — gọi từ execute_trade khi token bị block.
pub async fn log_skipped_token(
    token_mint: &str,
    rejection_reason: &str,
) -> PgResult<()> {
    let db = get_shared_db().await?;
    let active = skipped_tokens_log::ActiveModel {
        token_mint: Set(token_mint.to_string()),
        rejection_reason: Set(rejection_reason.to_string()),
        timestamp: Set(chrono::Utc::now().into()),
        ..Default::default()
    };
    active.insert(db).await?;
    Ok(())
}

/// Query N skipped tokens gần nhất (cho Telegram command /skipped).
pub async fn query_recent_skipped_tokens(limit: u64) -> PgResult<Vec<(String, String, String)>> {
    let db = get_shared_db().await?;
    let rows = skipped_tokens_log::Entity::find()
        .order_by_desc(skipped_tokens_log::Column::Timestamp)
        .paginate(db, limit)
        .fetch_page(0)
        .await?;

    let mut results = Vec::new();
    for row in rows {
        results.push((
            row.token_mint,
            row.rejection_reason,
            row.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        ));
    }
    Ok(results)
}

// ── Stats Query ───────────────────────────────────────────────────────────────

/// Stats result for a time period.
#[derive(Debug, Clone, Default)]
pub struct FilterStats {
    pub total: i64,
    pub passed: i64,
    pub failed: i64,
    pub warned: i64,
    pub skipped: i64,
}

/// Query filter stats for different time periods.
/// Returns (today, 7d, 30d, all_time) stats.
pub async fn query_filter_stats() -> PgResult<(FilterStats, FilterStats, FilterStats, FilterStats)> {
    let db = get_shared_db().await?;
    let now = chrono::Utc::now();
    let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
    let week_start = (now - chrono::Duration::days(7)).naive_utc();
    let month_start = (now - chrono::Duration::days(30)).naive_utc();

    // Get all filter logs
    let all_logs = anti_rug_filter_log::Entity::find()
        .all(db)
        .await?;

    let mut today = FilterStats::default();
    let mut week = FilterStats::default();
    let mut month = FilterStats::default();
    let mut all = FilterStats::default();

    for log in &all_logs {
        let ts = log.created_at.naive_utc();
        let verdict = log.verdict.as_str();

        // All time
        all.total += 1;
        match verdict {
            "pass" => all.passed += 1,
            "fail" => all.failed += 1,
            "warn" => all.warned += 1,
            _ => {}
        }

        // Monthly
        if ts >= month_start {
            month.total += 1;
            match verdict {
                "pass" => month.passed += 1,
                "fail" => month.failed += 1,
                "warn" => month.warned += 1,
                _ => {}
            }
        }

        // Weekly
        if ts >= week_start {
            week.total += 1;
            match verdict {
                "pass" => week.passed += 1,
                "fail" => week.failed += 1,
                "warn" => week.warned += 1,
                _ => {}
            }
        }

        // Today
        if ts >= today_start {
            today.total += 1;
            match verdict {
                "pass" => today.passed += 1,
                "fail" => today.failed += 1,
                "warn" => today.warned += 1,
                _ => {}
            }
        }
    }

    // Count skipped tokens per period
    let all_skipped = skipped_tokens_log::Entity::find().all(db).await?;
    for skip in &all_skipped {
        let ts = skip.timestamp.naive_utc();
        all.skipped += 1;
        if ts >= month_start { month.skipped += 1; }
        if ts >= week_start { week.skipped += 1; }
        if ts >= today_start { today.skipped += 1; }
    }

    Ok((today, week, month, all))
}

// ── Trade History Logging & PNL ───────────────────────────────────────────────

/// Record a buy or sell trade into the database.
pub async fn log_trade(
    token_mint: &str,
    trade_type: &str,   // "buy" or "sell"
    sol_amount: f64,
    token_price: f64,
    mc_at_trade: f64,
    tx_hash: &str,
    status: &str,       // "success" or "failed"
) -> PgResult<()> {
    let db = get_shared_db().await?;
    let active = trade_history::ActiveModel {
        token_mint: Set(token_mint.to_string()),
        trade_type: Set(trade_type.to_string()),
        sol_amount: Set(sol_amount),
        token_price: Set(token_price),
        mc_at_trade: Set(mc_at_trade),
        tx_hash: Set(tx_hash.to_string()),
        status: Set(status.to_string()),
        created_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    };
    active.insert(db).await?;
    Ok(())
}

/// PNL stats for a time period.
#[derive(Debug, Clone, Default)]
pub struct TradePnlStats {
    pub total_buys: i64,
    pub total_sells: i64,
    pub successful_buys: i64,
    pub failed_buys: i64,
    pub total_sol_spent: f64,      // sum of buy SOL
    pub total_sol_received: f64,   // sum of sell SOL
    pub realized_pnl: f64,         // received - spent
    pub win_trades: i64,           // sells where received > avg_buy_cost
    pub lose_trades: i64,
    pub win_rate: f64,             // win / (win + lose) * 100
}

/// Query trade PNL stats for different time periods.
/// Returns (today, 7d, 30d, all_time) stats.
pub async fn query_trade_pnl_stats() -> PgResult<(TradePnlStats, TradePnlStats, TradePnlStats, TradePnlStats)> {
    let db = get_shared_db().await?;
    let now = chrono::Utc::now();
    let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
    let week_start = (now - chrono::Duration::days(7)).naive_utc();
    let month_start = (now - chrono::Duration::days(30)).naive_utc();

    let all_trades = trade_history::Entity::find()
        .order_by_asc(trade_history::Column::CreatedAt)
        .all(db)
        .await?;

    let mut today = TradePnlStats::default();
    let mut week = TradePnlStats::default();
    let mut month = TradePnlStats::default();
    let mut all = TradePnlStats::default();

    // Group sells by token_mint to calculate per-token PNL
    // For each sell, find matching buy(s) for same mint to determine win/lose
    use std::collections::HashMap;
    let mut buy_costs: HashMap<String, Vec<f64>> = HashMap::new();

    for trade in &all_trades {
        let ts = trade.created_at.naive_utc();
        let is_buy = trade.trade_type == "buy";
        let is_success = trade.status == "success";

        // Track buy costs per mint
        if is_buy && is_success {
            buy_costs
                .entry(trade.token_mint.clone())
                .or_default()
                .push(trade.sol_amount);
        }

        let periods: Vec<(&mut TradePnlStats, bool)> = vec![
            (&mut all, true),
            (&mut month, ts >= month_start),
            (&mut week, ts >= week_start),
            (&mut today, ts >= today_start),
        ];

        for (stats, in_range) in periods {
            if !in_range { continue; }

            if is_buy {
                stats.total_buys += 1;
                if is_success {
                    stats.successful_buys += 1;
                    stats.total_sol_spent += trade.sol_amount;
                } else {
                    stats.failed_buys += 1;
                }
            } else if is_success {
                stats.total_sells += 1;
                stats.total_sol_received += trade.sol_amount;

                // Determine win/lose: compare sell SOL with avg buy cost
                if let Some(buys) = buy_costs.get(&trade.token_mint) {
                    let avg_buy = buys.iter().sum::<f64>() / buys.len() as f64;
                    if trade.sol_amount > avg_buy {
                        stats.win_trades += 1;
                    } else {
                        stats.lose_trades += 1;
                    }
                }
            }
        }
    }

    // Calculate derived fields
    for stats in [&mut today, &mut week, &mut month, &mut all] {
        stats.realized_pnl = stats.total_sol_received - stats.total_sol_spent;
        let total_closed = stats.win_trades + stats.lose_trades;
        stats.win_rate = if total_closed > 0 {
            (stats.win_trades as f64 / total_closed as f64) * 100.0
        } else {
            0.0
        };
    }

    Ok((today, week, month, all))
}
