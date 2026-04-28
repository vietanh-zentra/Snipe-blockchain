#[macro_export]
macro_rules! solscan {
    ($signature:expr) => {
        format!("https://solscan.io/tx/{}", $signature)
    };
}