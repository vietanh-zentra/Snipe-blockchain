//! Module 2: Dynamic Panic-Sell via Jito Bundle.
//!
//! Sau khi mua token thành công, monitor ví dev + top holders qua gRPC.
//! Khi phát hiện SELL intent → gửi Jito bundle để bán TRƯỚC dev/whale.
//!
//! Jito Block Engine REST API:
//!   POST https://mainnet.block-engine.jito.wtf/api/v1/bundles
//!
//! Tip accounts (random 1 trong 8):
//!   96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5
//!   HFqU5x63VTqvQss8hp11i4bVqkfRtQo3EZTJrPaKYWo7
//!   ...

use crate::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signer::Signer,
    signature::Keypair,
    system_instruction,
    transaction::Transaction,
};
use std::{
    collections::HashSet,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::time::sleep;

/// Danh sách Jito tip accounts (chọn ngẫu nhiên 1 per bundle).
pub const JITO_TIP_ACCOUNTS: &[&str] = &[
    "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
    "HFqU5x63VTqvQss8hp11i4bVqkfRtQo3EZTJrPaKYWo7",
    "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY",
    "ADaUMid9yfUytqMBgopwjb2DTLSf5oXbsyq7hPbQELGR",
    "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh",
    "ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt",
    "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL",
    "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT",
];

const JITO_BUNDLE_ENDPOINT: &str = "https://mainnet.block-engine.jito.wtf/api/v1/bundles";

/// Handle để cancel panic-sell monitor từ bên ngoài.
pub struct PanicSellMonitorHandle {
    cancel: Arc<AtomicBool>,
}

impl PanicSellMonitorHandle {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl Drop for PanicSellMonitorHandle {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Thông tin cần thiết để monitor panic-sell.
pub struct PanicSellContext {
    pub token_mint: Pubkey,
    pub pumpswap_accounts: PumpSwapStruct,
    pub keypair: Keypair,
    pub token_balance: u64,
    pub token_creator: Pubkey,
    pub is_cashback_coin: bool,
    pub jito_tip_lamports: u64,
    /// Danh sách ví cần theo dõi (dev + top holders).
    pub watched_wallets: Vec<Pubkey>,
}

/// Bắt đầu monitoring panic-sell cho một token position.
///
/// Spawns một task riêng theo dõi các ví qua polling RPC.
/// Trả về handle để cancel khi vị thế đã đóng.
pub fn start_panic_sell_monitor(ctx: PanicSellContext) -> PanicSellMonitorHandle {
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = cancel.clone();

    tokio::spawn(async move {
        run_monitor(ctx, cancel_clone).await;
    });

    PanicSellMonitorHandle { cancel }
}

async fn run_monitor(ctx: PanicSellContext, cancel: Arc<AtomicBool>) {
    info!(
        "[PANIC_SELL] Started monitoring {} wallets for mint {}",
        ctx.watched_wallets.len(),
        ctx.token_mint
    );

    // Poll mỗi 500ms để kiểm tra token balance của các ví được theo dõi
    // Approach: so sánh token balance hiện tại vs block trước
    // Nếu balance giảm đột ngột → trigger panic sell
    let mut prev_balances: std::collections::HashMap<Pubkey, u64> = std::collections::HashMap::new();

    // Khởi tạo baseline balances
    for wallet in &ctx.watched_wallets {
        if let Ok(balance) = get_token_balance_for_wallet(&ctx.token_mint, wallet).await {
            prev_balances.insert(*wallet, balance);
        }
    }

    let poll_interval = Duration::from_millis(500);
    let mut triggered = false;

    loop {
        if cancel.load(Ordering::Relaxed) {
            info!("[PANIC_SELL] Monitor cancelled for {}", ctx.token_mint);
            break;
        }

        if triggered {
            break;
        }

        sleep(poll_interval).await;

        // Kiểm tra từng wallet
        for wallet in &ctx.watched_wallets {
            if cancel.load(Ordering::Relaxed) {
                break;
            }

            let current_balance =
                get_token_balance_for_wallet(&ctx.token_mint, wallet).await.unwrap_or(0);

            let prev = *prev_balances.get(wallet).unwrap_or(&0);

            // Detect significant sell: balance giảm > 20% trong 1 poll cycle
            if prev > 0 && current_balance < prev {
                let drop_pct = (prev - current_balance) as f64 / prev as f64 * 100.0;
                if drop_pct > 20.0 {
                    info!(
                        "[PANIC_SELL] 🚨 DETECTED! Wallet {} sold {:.1}% of token {}. Triggering panic sell!",
                        wallet, drop_pct, ctx.token_mint
                    );

                    enqueue_panic_sell_detected_alert(
                        &ctx.token_mint.to_string(),
                        &wallet.to_string(),
                        drop_pct,
                    );

                    // Trigger panic sell via Jito bundle
                    triggered = true;
                    trigger_jito_panic_sell(&ctx).await;
                    break;
                }
            }

            prev_balances.insert(*wallet, current_balance);
        }
    }

    info!("[PANIC_SELL] Monitor stopped for {}", ctx.token_mint);
}

/// Lấy token balance của một wallet cho mint cụ thể.
async fn get_token_balance_for_wallet(mint: &Pubkey, wallet: &Pubkey) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let ata = spl_associated_token_account::get_associated_token_address(wallet, mint);
    match RPC_CLIENT.get_token_account_balance(&ata).await {
        Ok(balance) => Ok(balance.amount.parse::<u64>().unwrap_or(0)),
        Err(_) => Ok(0), // Account không tồn tại → balance 0
    }
}

/// Gửi Jito bundle để panic sell vị thế hiện tại.
async fn trigger_jito_panic_sell(ctx: &PanicSellContext) {
    let keypair = ctx.keypair.insecure_clone();
    let signer_pubkey = keypair.pubkey();

    // Build sell instructions (tương tự execute_pumpswap_sell)
    let mut ps = ctx.pumpswap_accounts;
    let mut ix: Vec<Instruction> = Vec::new();

    let create_ix = ps.get_create_ata_idempotent_ix(&signer_pubkey);
    let sell_ix = ps.get_sell_ix(
        &signer_pubkey,
        ctx.token_balance,
        ctx.token_creator,
        ctx.is_cashback_coin,
    );
    let close_ix = ps.close_wsol_ata(&signer_pubkey);

    ix.extend(create_ix);
    ix.push(sell_ix);
    ix.push(close_ix);

    // Thêm Jito tip instruction
    let tip_account = random_jito_tip_account();
    let tip_ix = system_instruction::transfer(
        &signer_pubkey,
        &tip_account,
        ctx.jito_tip_lamports,
    );
    ix.push(tip_ix);

    // Lấy blockhash mới nhất
    let recent_blockhash = match RECENT_BLOCKHASH.try_read() {
        Ok(guard) => *guard,
        Err(_) => {
            eprintln!("[PANIC_SELL] Cannot get blockhash");
            // Fallback: gửi via normal transaction
            tokio::spawn(async move {
                let _ = send_0slot_transaction(ix, keypair).await;
            });
            return;
        }
    };

    let tx = Transaction::new_signed_with_payer(
        &ix,
        Some(&signer_pubkey),
        &[&keypair],
        recent_blockhash,
    );

    // Submit Jito bundle
    match submit_jito_bundle(vec![tx]).await {
        Ok(bundle_id) => {
            info!(
                "[PANIC_SELL] ✅ Jito bundle submitted: {} for mint {}",
                bundle_id, ctx.token_mint
            );
        }
        Err(e) => {
            eprintln!("[PANIC_SELL] Jito bundle failed: {e}. Falling back to normal TX.");
            // Fallback to normal send
            let keypair2 = ctx.keypair.insecure_clone();
            let ix_clone = ix.clone();
            tokio::spawn(async move {
                let _ = send_0slot_transaction(ix_clone, keypair2).await;
            });
        }
    }
}

/// Chọn ngẫu nhiên một Jito tip account.
fn random_jito_tip_account() -> Pubkey {
    use rand::Rng;
    let idx = rand::thread_rng().gen_range(0..JITO_TIP_ACCOUNTS.len());
    Pubkey::from_str(JITO_TIP_ACCOUNTS[idx]).expect("Valid Jito tip account")
}

/// Submit một Jito bundle gồm nhiều transactions.
///
/// Returns bundle ID nếu thành công.
async fn submit_jito_bundle(
    transactions: Vec<Transaction>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();

    // Serialize transactions sang base58
    let encoded_txs: Vec<String> = transactions
        .iter()
        .map(|tx| {
            let serialized = bincode::serialize(tx).expect("TX serialize");
            bs58::encode(serialized).into_string()
        })
        .collect();

    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "sendBundle",
        "params": [encoded_txs]
    });

    let response = client
        .post(JITO_BUNDLE_ENDPOINT)
        .json(&payload)
        .timeout(Duration::from_secs(5))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;

    if let Some(result) = body.get("result") {
        Ok(result.as_str().unwrap_or("unknown").to_string())
    } else if let Some(err) = body.get("error") {
        Err(format!("Jito API error: {}", err).into())
    } else {
        Err("Unknown Jito response".into())
    }
}

// ── Placeholder cho alert function (sẽ implement trong telegram_alert) ─────

fn enqueue_panic_sell_detected_alert(mint: &str, seller_wallet: &str, drop_pct: f64) {
    let msg = format!(
        "🚨 *PANIC SELL DETECTED*\n\
        Mint: `{}`\n\
        Seller: `{}`\n\
        Balance drop: {:.1}%\n\
        → Triggering emergency sell via Jito!",
        mint, seller_wallet, drop_pct
    );
    // Queue Telegram alert
    if let Some(sender) = crate::modules::telegram_alert::ALERT_SENDER.get() {
        let _ = sender.try_send(msg);
    }
}
