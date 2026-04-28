use crate::encryption::{decrypt_private_key, encrypt_private_key};
use super::entities::{trading_parameter, wallet};
use super::migration::Migrator;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue::Set, Database, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use sea_orm_migration::MigratorTrait;
use std::env;

type PgResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

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
