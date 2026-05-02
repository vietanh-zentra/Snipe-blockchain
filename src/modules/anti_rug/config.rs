/// Hành động khi token không có metadata URI (Brief V.M5 L405).
#[derive(Debug, Clone, PartialEq)]
pub enum MetadataAction {
    /// Bỏ qua token (fail filter).
    Skip,
    /// Cảnh báo nhưng vẫn cho mua.
    Warn,
    /// Cho phép mua bình thường.
    Allow,
}

impl MetadataAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            MetadataAction::Skip => "skip",
            MetadataAction::Warn => "warn",
            MetadataAction::Allow => "allow",
        }
    }
}

/// Cấu hình Anti-Rug Intelligence Layer.
/// Có thể điều chỉnh qua Telegram UI và được persist vào DB.
#[derive(Debug, Clone)]
pub struct AntiRugConfig {
    /// Master switch — tắt toàn bộ anti-rug filter.
    pub enabled: bool,

    /// Warn-only mode — log và alert nhưng KHÔNG block lệnh mua.
    /// Dùng để thu thập data trong giai đoạn calibration.
    pub warn_only: bool,

    // ── Module 1: Holder Concentration Filter ────────────────────────────
    /// Bật/tắt kiểm tra holder concentration.
    pub holder_filter_enabled: bool,
    /// Ngưỡng tối đa % supply mà top 10 holders có thể nắm (default: 30.0%).
    /// Nếu vượt ngưỡng → Fail.
    pub max_top10_holder_pct: f64,

    // ── Module 3: Dev Wallet Profiler ─────────────────────────────────────
    /// Bật/tắt kiểm tra lịch sử ví dev.
    pub dev_profiler_enabled: bool,
    /// Số TX lịch sử tối thiểu của dev wallet để pass (default: 10).
    pub min_dev_tx_count: u64,
    /// Tuổi ví dev tối thiểu (giờ). Ví < min_wallet_age_hours → Fail (Brief L355, L361).
    pub min_wallet_age_hours: u64,
    /// Cảnh báo/fail nếu dev wallet được fund từ CEX hot wallet (Brief L357, L363).
    pub block_cex_funded: bool,

    // ── Module 4: Genesis Bundle Detector ────────────────────────────────
    /// Bật/tắt genesis block cluster detection.
    pub genesis_detector_enabled: bool,
    /// Ngưỡng tối đa % supply được mua trong genesis block (default: 50.0%).
    pub max_genesis_buy_pct: f64,
    /// Số ví clustered tối đa cho phép trong genesis (default: 3).
    pub max_clustered_wallets: u32,

    // ── Module 5: Metadata Checker ───────────────────────────────────────
    /// Bật/tắt kiểm tra Metaplex metadata.
    pub metadata_checker_enabled: bool,
    /// Hành động khi token không có metadata URI (Brief V.M5 L405).
    pub metadata_empty_action: MetadataAction,
    /// Yêu cầu token phải có metadata URI (Brief L404).
    pub require_metadata_uri: bool,

    // ── Module 2: Panic-Sell Monitor (Post-Buy) ──────────────────────────
    /// Bật/tắt panic-sell monitor (theo dõi dev + top holders sau khi mua).
    pub panic_sell_enabled: bool,
    /// Jito tip (lamports) cho panic-sell bundle (default: 1_000_000 = 0.001 SOL).
    pub panic_sell_jito_tip_lamports: u64,
    /// Số holder lớn nhất cần theo dõi (ngoài dev), default: 3.
    pub panic_sell_watch_top_holders: u32,

    // ── Shared ───────────────────────────────────────────────────────────
    /// Timeout cho mỗi filter RPC call (ms, default: 1500).
    pub filter_timeout_ms: u64,
}

impl Default for AntiRugConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            warn_only: false, // Production mode — BLOCK token fail filter
            holder_filter_enabled: true,
            max_top10_holder_pct: 30.0,
            dev_profiler_enabled: true,
            min_dev_tx_count: 10,
            min_wallet_age_hours: 24, // Brief L361
            block_cex_funded: true,   // Brief L363
            genesis_detector_enabled: false, // Tắt mặc định vì tốn CU nhiều
            max_genesis_buy_pct: 50.0,
            max_clustered_wallets: 3,
            metadata_checker_enabled: true,
            metadata_empty_action: MetadataAction::Warn,
            require_metadata_uri: true, // Brief L404
            panic_sell_enabled: true,
            panic_sell_jito_tip_lamports: 1_000_000, // 0.001 SOL
            panic_sell_watch_top_holders: 3,
            filter_timeout_ms: 1_500,
        }
    }
}
