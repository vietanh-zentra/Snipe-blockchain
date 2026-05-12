use sea_orm::entity::prelude::*;

/// SeaORM entity for trade_history table.
/// Stores buy/sell trade records for PNL and win rate tracking.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "trade_history")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i64,
    /// Token mint address (base58).
    pub token_mint: String,
    /// "buy" or "sell".
    pub trade_type: String,
    /// SOL amount spent (buy) or received (sell).
    pub sol_amount: f64,
    /// Token price at time of trade.
    pub token_price: f64,
    /// Market cap at time of trade.
    pub mc_at_trade: f64,
    /// Transaction hash.
    pub tx_hash: String,
    /// "success" or "failed".
    pub status: String,
    /// Timestamp of trade.
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
