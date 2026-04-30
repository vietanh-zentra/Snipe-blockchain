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

    // ── Module 2: Panic-Sell Monitor (Post-Buy) ──────────────────────────
    /// Bật/tắt panic-sell monitor (theo dõi dev + top holders sau khi mua).
    pub panic_sell_enabled: bool,
    /// Jito tip (lamports) cho panic-sell bundle (default: 100_000).
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
            warn_only: true, // Bắt đầu bằng warn-only để thu thập data
            holder_filter_enabled: true,
            max_top10_holder_pct: 30.0,
            dev_profiler_enabled: true,
            min_dev_tx_count: 10,
            genesis_detector_enabled: false, // Tắt mặc định vì tốn CU nhiều
            max_genesis_buy_pct: 50.0,
            max_clustered_wallets: 3,
            metadata_checker_enabled: true,
            panic_sell_enabled: true,
            panic_sell_jito_tip_lamports: 100_000,
            panic_sell_watch_top_holders: 3,
            filter_timeout_ms: 1_500,
        }
    }
}
