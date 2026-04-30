use sea_orm::entity::prelude::*;

/// SeaORM entity cho bảng anti_rug_filter_log.
/// Mỗi record là kết quả filter của một token migration event.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "anti_rug_filter_log")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i64,
    /// Địa chỉ token mint (base58, 44 chars).
    pub token_mint: String,
    /// Thời điểm filter chạy.
    pub created_at: DateTimeWithTimeZone,
    /// Verdict: 'pass', 'fail', hoặc 'warn'.
    pub verdict: String,
    /// Lý do reject (nếu verdict = 'fail' hoặc 'warn').
    pub reject_reason: Option<String>,
    /// % supply mà top 10 holders nắm giữ.
    pub top10_holder_pct: Option<f64>,
    /// Số TX lịch sử của dev wallet.
    pub dev_tx_count: Option<i64>,
    /// % supply được mua trong genesis block.
    pub genesis_buy_pct: Option<f64>,
    /// True nếu detect genesis bundle buying.
    pub genesis_bundle_detected: bool,
    /// True nếu token có Metaplex metadata URI.
    pub has_metadata_uri: bool,
    /// Tổng thời gian filter chạy (milliseconds).
    pub filter_duration_ms: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
