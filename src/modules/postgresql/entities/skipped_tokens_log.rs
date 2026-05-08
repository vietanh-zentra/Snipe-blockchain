use sea_orm::entity::prelude::*;

/// SeaORM entity cho bảng skipped_tokens_log.
/// Lưu lịch sử các token bị bỏ qua bởi Anti-Rug filter (Block mode).
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "skipped_tokens_log")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i64,
    /// Địa chỉ token mint (base58, 44 chars).
    pub token_mint: String,
    /// Lý do bị từ chối.
    pub rejection_reason: String,
    /// Thời điểm bị skip.
    pub timestamp: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
