use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "trading_parameter")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub telegram_user_id: i64,
    pub buy_amount_sol: f64,
    pub slippage_percent: i32,
    pub take_profit: f64,
    pub stop_loss: f64,
    pub trailing: f64,
    pub trailing_stop: f64,
    pub priority_fee_micro_lamports: i64,
    pub tip_fee_sol: f64,
    pub is_running: bool,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
