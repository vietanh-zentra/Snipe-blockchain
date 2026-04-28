#[derive(Debug, Clone, Copy)]
pub struct StandardConfig {
    pub default_buy_amount_sol: f64,
    pub default_slippage_percent: u32,
    pub default_cu: u32,
    pub default_priority_fee_micro_lamport: u64,
    pub default_third_party_fee: f64,
    pub default_take_profit: f64,
    pub default_stop_loss: f64,
    pub default_trailing: f64,
    pub default_trailing_stop: f64,
}

pub const STANDARD_CONFIG: StandardConfig = StandardConfig {
    default_buy_amount_sol: 0.1,
    default_slippage_percent: 50,
    default_cu: 200_000,
    default_priority_fee_micro_lamport: 100_000,
    default_third_party_fee: 0.001,
    default_take_profit: 120.0,
    default_stop_loss: 80.0,
    default_trailing: 110.0,
    default_trailing_stop: 10.0,
};

pub fn default_buy_amount_sol() -> f64 {
    STANDARD_CONFIG.default_buy_amount_sol
}

pub fn default_slippage_percent() -> u32 {
    STANDARD_CONFIG.default_slippage_percent
}

pub fn default_cu() -> u32 {
    STANDARD_CONFIG.default_cu
}

pub fn default_priority_fee_micro_lamport() -> u64 {
    STANDARD_CONFIG.default_priority_fee_micro_lamport
}

pub fn default_third_party_fee() -> f64 {
    STANDARD_CONFIG.default_third_party_fee
}

pub fn default_take_profit() -> f64 {
    STANDARD_CONFIG.default_take_profit
}

pub fn default_stop_loss() -> f64 {
    STANDARD_CONFIG.default_stop_loss
}

pub fn default_trailing() -> f64 {
    STANDARD_CONFIG.default_trailing
}

pub fn default_trailing_stop() -> f64 {
    STANDARD_CONFIG.default_trailing_stop
}

pub fn priority_fee_sol_from_micro_lamports(micro_lamports: u64, cu: u32) -> f64 {
    cu as f64 * micro_lamports as f64 / 1_000_000_000_000_000.0
}

pub fn default_priority_fee_sol() -> f64 {
    priority_fee_sol_from_micro_lamports(
        STANDARD_CONFIG.default_priority_fee_micro_lamport,
        STANDARD_CONFIG.default_cu,
    )
}