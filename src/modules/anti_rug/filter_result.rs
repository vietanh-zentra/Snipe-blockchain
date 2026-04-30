/// Kết quả đánh giá của từng filter module.
#[derive(Debug, Clone)]
pub enum FilterVerdict {
    /// Token passed — an toàn để mua.
    Pass,
    /// Token bị loại — lý do được ghi rõ.
    Fail(String),
    /// Token đáng ngờ nhưng không block (warn_only mode).
    Warn(String),
}

impl FilterVerdict {
    pub fn is_fail(&self) -> bool {
        matches!(self, FilterVerdict::Fail(_))
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            FilterVerdict::Fail(r) | FilterVerdict::Warn(r) => Some(r.as_str()),
            FilterVerdict::Pass => None,
        }
    }

    pub fn as_db_str(&self) -> &'static str {
        match self {
            FilterVerdict::Pass => "pass",
            FilterVerdict::Fail(_) => "fail",
            FilterVerdict::Warn(_) => "warn",
        }
    }
}

/// Kết quả tổng hợp của toàn bộ Anti-Rug filter pipeline.
#[derive(Debug, Clone)]
pub struct AntiRugFilterResult {
    /// Verdict cuối cùng (tổng hợp từ tất cả filters).
    pub verdict: FilterVerdict,
    /// % supply do top 10 holders nắm giữ (None nếu filter bị tắt hoặc timeout).
    pub top10_holder_pct: Option<f64>,
    /// Số TX lịch sử của dev wallet.
    pub dev_tx_count: Option<u64>,
    /// % supply được mua trong genesis block.
    pub genesis_buy_pct: Option<f64>,
    /// True nếu detect được clustered buys trong genesis block.
    pub genesis_bundle_detected: bool,
    /// True nếu token có Metaplex metadata URI.
    pub has_metadata_uri: bool,
    /// Tổng thời gian chạy filter (ms).
    pub filter_duration_ms: u64,
}

impl AntiRugFilterResult {
    /// Tạo result Pass đơn giản khi anti-rug bị tắt hoàn toàn.
    pub fn disabled_pass() -> Self {
        Self {
            verdict: FilterVerdict::Pass,
            top10_holder_pct: None,
            dev_tx_count: None,
            genesis_buy_pct: None,
            genesis_bundle_detected: false,
            has_metadata_uri: false,
            filter_duration_ms: 0,
        }
    }
}
