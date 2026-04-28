use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "wallet")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub wallet_id: i32,
    pub telegram_user_id: i64,
    pub private_key_encrypted: String,
    pub pubkey: String,
    pub is_selected: bool,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
