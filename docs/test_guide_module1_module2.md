# Hướng Dẫn Test Module 1, 2, 3, 4 + Cách Hoạt Động Chi Tiết

> **Ngày cập nhật:** 01/05/2026
> **Trạng thái:** Module 1 ✅ | Module 2 ✅ | Module 3 ✅ | Module 4 ✅

---

## Mục Lục
1. [Yêu cầu môi trường](#1-yêu-cầu-môi-trường)
2. [Test Module 1: Holder Concentration Filter](#2-test-module-1-holder-concentration-filter)
3. [Test Module 2: Panic-Sell via Jito Bundle](#3-test-module-2-panic-sell-via-jito-bundle)
4. [Cách hoạt động Module 2 (chi tiết kỹ thuật)](#4-cách-hoạt-động-module-2-chi-tiết-kỹ-thuật)
5. [Test Module 3: Dev Wallet Profiler](#5-test-module-3-dev-wallet-profiler)
6. [Cách hoạt động Module 3 (chi tiết kỹ thuật)](#6-cách-hoạt-động-module-3-chi-tiết-kỹ-thuật)
7. [Test Module 4: Genesis Bundle Detector](#7-test-module-4-genesis-bundle-detector)
8. [Cách hoạt động Module 4 (chi tiết kỹ thuật)](#8-cách-hoạt-động-module-4-chi-tiết-kỹ-thuật)
9. [Test Module 5: Metadata Checker](#9-test-module-5-metadata-checker)
10. [Cách hoạt động Module 5 (chi tiết kỹ thuật)](#10-cách-hoạt-động-module-5-chi-tiết-kỹ-thuật)
11. [Xử lý sự cố](#11-xử-lý-sự-cố)

---

## 1. Yêu cầu môi trường

### Biến môi trường (chạy trước mỗi phiên PowerShell)
```powershell
$env:OPENSSL_DIR="C:\Program Files\OpenSSL-Win64"
$env:OPENSSL_LIB_DIR="C:\Program Files\OpenSSL-Win64\lib\VC\x64\MD"
$env:OPENSSL_INCLUDE_DIR="C:\Program Files\OpenSSL-Win64\include"
$env:CC=""
$env:CXX=""
```

### Checklist trước khi chạy
- [ ] Docker Desktop đang **Running** (cho PostgreSQL)
- [ ] File `.env` đã có `TELEGRAM_BOT_TOKEN` hợp lệ
- [ ] File `.env` đã có `ALLOWED_TELEGRAM_USER_ID` đúng
- [ ] File `.env` đã có `GRPC_ENDPOINT` (tùy chọn, cần cho realtime)

### Khởi tạo Database (chỉ lần đầu)
```powershell
cargo run --bin init_db
```
Kết quả mong đợi:
```
✅ PostgreSQL migration complete. Tables are ready.
```

---

## 2. Test Module 1: Holder Concentration Filter

### Module 1 làm gì?
Trước khi mua token, bot kiểm tra **top 10 holders** chiếm bao nhiêu % tổng supply.
Nếu > 30% → token có nguy cơ bị rug → **SKIP** hoặc **WARN** (tùy cấu hình).

### Cách test

#### Bước 1: Chạy bot
```powershell
cargo run --bin sniper_mode
```

#### Bước 2: Mở Telegram → gửi `/start`
Bot sẽ hiển thị menu với các nút:
- 💰 **Wallet management** — quản lý ví
- ⚙️ **Trading parameters** — cấu hình giao dịch
- ▶️ **Start** — bắt đầu snipe

#### Bước 3: Tạo ví test
```
/generate          → Tạo ví mới
/select_1          → Chọn ví #1 để giao dịch
```

#### Bước 4: Quan sát log trong PowerShell
Khi bot phát hiện token migration, bạn sẽ thấy log:
```
[ANTI-RUG] Mint: 7xKX...abc | Verdict: PASS | Top10: 22.5% | Dev TX: None | Duration: 850ms
```

Hoặc nếu token bị reject:
```
[ANTI-RUG] ❌ SKIP 7xKX...abc — [M1-Holder] Top 10 holders own 45.2% (max: 30.0%)
```

### Cấu hình Module 1

Trong file `src/modules/anti_rug/config.rs`:

| Tham số | Giá trị mặc định | Ý nghĩa |
|---------|-------------------|---------|
| `enabled` | `true` | Master switch bật/tắt toàn bộ anti-rug |
| `warn_only` | `true` | `true` = chỉ cảnh báo, vẫn mua. `false` = block mua |
| `holder_filter_enabled` | `true` | Bật/tắt kiểm tra holder |
| `max_top10_holder_pct` | `30.0` | Ngưỡng tối đa (%) top 10 holders |
| `filter_timeout_ms` | `1500` | Timeout cho mỗi filter RPC call |

### Thay đổi chế độ từ WARN → BLOCK

Trong `config.rs`, đổi:
```rust
warn_only: false,  // Sẽ block mua khi filter fail
```

---

## 3. Test Module 2: Panic-Sell via Jito Bundle

### Module 2 làm gì?
Sau khi mua token thành công, bot **tự động theo dõi ví dev** mỗi 500ms.
Nếu phát hiện dev bán > 20% token → bot **bán ngay lập tức** qua Jito bundle.

### Cách test

#### Bước 1: Chạy bot (giống Module 1)
```powershell
cargo run --bin sniper_mode
```

#### Bước 2: Quan sát log khi bot mua token
Sau khi mua thành công, bạn sẽ thấy:
```
[PANIC_SELL] ▶ Started monitoring 1 wallets for mint 7xKX...abc
[PANIC_SELL]   Tracking wallet FdEv...xyz — initial balance: 500000000
```

#### Bước 3: Nếu dev bán (tình huống thực tế)
```
[PANIC_SELL] 🚨 DETECTED! Wallet FdEv...xyz sold 85.0% of token 7xKX...abc. Triggering panic sell!
🚨 PANIC SELL DETECTED — Mint: 7xKX...abc | Seller: FdEv...xyz | Drop: 85.0%
[PANIC_SELL] ✅ Jito bundle submitted: abc123... for mint 7xKX...abc
```

#### Bước 4: Nếu Jito thất bại
```
[PANIC_SELL] Jito bundle failed: Connection timeout. Falling back to normal TX.
[✔ SUBMIT] Transaction(zero slot) submission took: 245ms
```

### Cấu hình Module 2

| Tham số | Giá trị mặc định | Ý nghĩa |
|---------|-------------------|---------|
| `panic_sell_enabled` | `true` | Bật/tắt panic-sell monitor |
| `panic_sell_jito_tip_lamports` | `100_000` | Tip cho Jito validator (= 0.0001 SOL) |
| `panic_sell_watch_top_holders` | `3` | Số top holders theo dõi (ngoài dev) |

### Lưu ý khi test
- Bot cần **gRPC endpoint hoạt động** để nhận migration events
- Nếu chưa có gRPC → bot sẽ chạy nhưng không nhận signal tự động
- Panic-sell monitor **chỉ bắt đầu SAU KHI MUA** — không ảnh hưởng gì nếu chưa mua

---

## 4. Cách hoạt động Module 2 (chi tiết kỹ thuật)

### 4.1 Kiến trúc tổng quan

```
┌─────────────────────────────────────────────────────┐
│                  execute_trade.rs                     │
│                                                       │
│  1. Anti-Rug Filter (Module 1) → PASS?                │
│  2. execute_pumpswap_buy()     → MUA token            │
│  3. start_panic_sell_monitor() → SPAWN background task│
│     ↓                                                 │
│  ┌─────────────────────────────────────┐              │
│  │     panic_sell.rs (background)      │              │
│  │                                     │              │
│  │  Loop mỗi 500ms:                   │              │
│  │    → RPC: get_token_balance(dev)    │              │
│  │    → So sánh vs balance trước       │              │
│  │    → Nếu giảm > 20%:               │              │
│  │        Build sell IX                │              │
│  │        + Jito tip IX                │              │
│  │        Submit Jito Bundle           │              │
│  │        (fallback: send_0slot_tx)    │              │
│  └─────────────────────────────────────┘              │
└─────────────────────────────────────────────────────┘
```

### 4.2 Các bước xử lý chi tiết

#### Bước 1: Khởi tạo monitor
Sau khi `execute_pumpswap_buy()` gửi lệnh mua:
```rust
// File: execute_trade.rs (dòng 120-140)
if anti_rug_cfg.panic_sell_enabled {
    let ctx = PanicSellContext {
        token_mint,
        pumpswap_accounts,
        keypair,
        token_balance,
        token_creator,      // = dev wallet
        watched_wallets: vec![token_creator],
        jito_tip_lamports: 100_000,
        ...
    };
    let _handle = start_panic_sell_monitor(ctx);
}
```

#### Bước 2: Background monitoring loop
```rust
// File: panic_sell.rs — run_monitor()
loop {
    sleep(500ms).await;

    for wallet in watched_wallets {
        let current = get_token_balance(wallet);
        let previous = prev_balances[wallet];

        if current < previous {
            let drop_pct = (previous - current) / previous * 100;
            if drop_pct > 20.0 {
                // 🚨 DEV ĐANG BÁN!
                trigger_jito_panic_sell(&ctx).await;
            }
        }
    }
}
```

#### Bước 3: Build Jito Bundle
```rust
// File: panic_sell.rs — trigger_jito_panic_sell()

// 1. Build sell instructions (giống execute_pumpswap_sell)
let create_ix = ps.get_create_ata_idempotent_ix(&signer);
let sell_ix   = ps.get_sell_ix(&signer, token_balance, creator, cashback);
let close_ix  = ps.close_wsol_ata(&signer);

// 2. Thêm Jito tip
let tip_account = random_jito_tip_account();  // Random 1 trong 8
let tip_ix = system_instruction::transfer(&signer, &tip_account, 100_000);

// 3. Lấy blockhash + Sign + Submit
let blockhash = RPC_CLIENT.get_latest_blockhash().await;
let tx = Transaction::new_signed_with_payer(&ix, Some(&signer), &[&keypair], blockhash);
submit_jito_bundle(vec![tx]).await;
```

#### Bước 4: Submit Jito Bundle
```rust
// HTTP POST tới Jito Block Engine
POST https://mainnet.block-engine.jito.wtf/api/v1/bundles
{
    "jsonrpc": "2.0",
    "method": "sendBundle",
    "params": [["base58_encoded_transaction"]]
}
```

#### Bước 5: Fallback
Nếu Jito thất bại (timeout, API error):
```rust
// Xóa tip instruction, gửi qua 0-slot bình thường
ix.pop(); // Remove tip
send_0slot_transaction(ix, keypair).await;
```

### 4.3 Tại sao dùng Jito Bundle?

| Phương thức | Tốc độ | Chi phí | Ưu tiên |
|-------------|--------|---------|---------|
| Normal TX | ~400ms | Priority fee | Thấp |
| Jito Bundle | ~200ms | Tip 0.0001 SOL | **Cao** (validator ưu tiên) |

Jito bundle được validator **xử lý trước** các TX thường → bot bán TRƯỚC dev.

### 4.4 Files liên quan

| File | Vai trò |
|------|---------|
| `src/modules/anti_rug/panic_sell.rs` | Logic chính — monitor + Jito bundle |
| `src/modules/anti_rug/config.rs` | Cấu hình ngưỡng và tip |
| `src/features/handle_sniper/execute_trade.rs` | Inject monitor sau khi mua |
| `src/features/build_tx/pumpswap_struct.rs` | Build sell instructions |
| `src/features/confirm_tx/send_zero_slot_tx.rs` | Fallback TX sender |

---

## 5. Test Module 3: Dev Wallet Profiler

### Module 3 làm gì?
Trước khi mua token, bot kiểm tra **lịch sử giao dịch của ví dev**.
Ví dev mới tạo (ít TX, tuổi vài giờ) → nguy cơ rug cao → **SKIP** hoặc **WARN**.

Module 3 chạy **song song** với Module 1 và Module 5 → không tăng tổng thời gian filter.

### Cách test

#### Bước 1: Chạy bot
```powershell
cargo run --bin sniper_mode
```

#### Bước 2: Quan sát log trong PowerShell
**Dev wallet đủ tuổi (PASS):**
```
[ANTI-RUG] Mint: 7xKX...abc | Verdict: PASS | Top10: 22.5% | Dev TX: 45 | Duration: 850ms
```

**Dev wallet mới tạo (FAIL):**
```
[ANTI-RUG] ❌ SKIP 7xKX...abc — [M3-Dev] Dev wallet has only 3 TXs (min: 10). Age: 2 hours
```

**RPC timeout:**
```
[ANTI-RUG] ❌ SKIP 7xKX...abc — [M3-Dev] Dev profiler error: Dev wallet RPC timeout
```

### Cấu hình Module 3

| Tham số | Giá trị mặc định | Ý nghĩa |
|---------|-------------------|---------|
| `dev_profiler_enabled` | `true` | Bật/tắt Module 3 |
| `min_dev_tx_count` | `10` | Số TX tối thiểu để pass |
| `filter_timeout_ms` | `1500` | Timeout cho RPC call |

### Thay đổi ngưỡng

Trong `config.rs`:
```rust
min_dev_tx_count: 20,  // Yêu cầu ít nhất 20 TX (nghiêm ngặt hơn)
```

---

## 6. Cách hoạt động Module 3 (chi tiết kỹ thuật)

### 6.1 Luồng xử lý

```
Pre-Buy Filter (pre_buy_filter.rs)
    ↓
┌─── Module 1: Holder Concentration ───┐
│   (chạy song song)                   │
├─── Module 3: Dev Wallet Profiler ────┤
│   (chạy song song)                   │
├─── Module 5: Metadata Checker ───────┤
│   (chạy song song)                   │
└──────────────────────────────────────┘
    ↓
Tổng hợp kết quả → PASS / WARN / FAIL
```

Tổng thời gian = max(M1, M3, M5) — **không cộng dồn!**

### 6.2 Logic chi tiết

```rust
// File: dev_wallet_profiler.rs — check_dev_wallet()

// 1. Query RPC: lấy 50 TX gần nhất của dev wallet
let sigs = RPC_CLIENT
    .get_signatures_for_address_with_config(dev_pubkey, config)
    .await;

// 2. Đếm số TX
let tx_count = sigs.len();  // tối đa 50

// 3. Tính tuổi ví từ TX cũ nhất
let oldest_timestamp = sigs.last().block_time;  // unix timestamp
let age_hours = (now - oldest_timestamp) / 3600;

// 4. So sánh với ngưỡng
if tx_count < min_tx_count {
    return Err("Dev wallet has only 3 TXs (min: 10). Age: 2 hours");
} else {
    return Ok(Some(tx_count));
}
```

### 6.3 Struct DevWalletProfile

```rust
pub struct DevWalletProfile {
    pub tx_count: u64,              // Số TX tìm được (max 50)
    pub oldest_tx_timestamp: Option<i64>,  // Unix timestamp TX cũ nhất
    pub estimated_age_hours: Option<u64>,  // Tuổi ví (giờ)
}
```

### 6.4 Tích hợp trong Pre-Buy Filter

```rust
// File: pre_buy_filter.rs
let (holder_result, dev_result, meta_result) = join!(
    check_holder_concentration(),  // Module 1
    check_dev_wallet(),            // Module 3 ← ĐÂY
    check_has_metadata(),          // Module 5
);
```

### 6.5 Files liên quan

| File | Vai trò |
|------|---------|
| `src/modules/anti_rug/dev_wallet_profiler.rs` | Logic chính Module 3 |
| `src/modules/anti_rug/pre_buy_filter.rs` | Gọi check_dev_wallet() |
| `src/modules/anti_rug/config.rs` | Cấu hình ngưỡng |

---

## 7. Test Module 4: Genesis Bundle Detector

### Module 4 làm gì?
Quét **genesis block** (block đầu tiên tạo token) để phát hiện:
- Nhiều ví mua cùng lúc trong 1 block (bundled buys)
- Tổng % supply bị mua quá lớn (> 50%)

Đây là dấu hiệu **rug-pull có tổ chức**: dev tạo nhiều ví, mua gom supply, rồi dump.

### Cách test

#### Bước 1: Bật Module 4 (mặc định TẮT)
Trong `config.rs`:
```rust
genesis_detector_enabled: true,  // Bật lên
```

> **Lưu ý:** Module 4 mặc định TẮT vì tốn RPC call nặng (get_block).

#### Bước 2: Chạy bot & quan sát log
```powershell
cargo run --bin sniper_mode
```

**Không có bundled buys (PASS):**
```
[ANTI-RUG] Mint: 7xKX...abc | Verdict: PASS | Genesis: 12.5% | Duration: 1200ms
```

**Phát hiện bundle (FAIL):**
```
[ANTI-RUG] ❌ SKIP 7xKX...abc — [M4-Genesis] Genesis bundle detected: 65.3% supply bought by 5 wallets in genesis block
```

### Cấu hình Module 4

| Tham số | Giá trị mặc định | Ý nghĩa |
|---------|-------------------|---------|
| `genesis_detector_enabled` | `false` | Bật/tắt Module 4 |
| `max_genesis_buy_pct` | `50.0` | Ngưỡng tối đa % supply mua trong genesis |
| `max_clustered_wallets` | `3` | Số ví cluster tối đa cho phép |

---

## 8. Cách hoạt động Module 4 (chi tiết kỹ thuật)

### 8.1 Luồng xử lý

```
check_genesis_bundles(mint, creation_slot, total_supply, ...)
    ↓
RPC: get_block_with_config(creation_slot)
    ↓
Lọc transactions liên quan tới mint
    ↓
Đếm buyers + tổng % supply mua
    ↓
Nếu (unique_buyers > 3) VÀ (genesis_buy_pct > 50%):
    → bundle_detected = true → FAIL
Ngược lại:
    → Ok(genesis_buy_pct)
```

### 8.2 Logic phát hiện bundle

```rust
// File: genesis_detector.rs

// 1. Lấy toàn bộ block tại creation_slot
let block = RPC_CLIENT.get_block_with_config(creation_slot, config).await;

// 2. Quét từng transaction trong block
for tx in block.transactions {
    // Bỏ qua TX không liên quan tới mint
    if !tx.contains(mint_address) { continue; }

    // 3. Trích xuất post_token_balances
    for balance in tx.meta.post_token_balances {
        if balance.mint == mint {
            buyer_amounts[owner] += balance.ui_amount;
        }
    }
}

// 4. Tính tổng
let genesis_buy_pct = total_bought / total_supply * 100;
let bundle_detected = unique_buyers > 3 && genesis_buy_pct > 50;
```

### 8.3 Struct GenesisAnalysis

```rust
pub struct GenesisAnalysis {
    pub genesis_buy_pct: f64,     // % supply mua trong genesis
    pub unique_buyers: usize,     // Số ví unique đã mua
    pub bundle_detected: bool,    // Có pattern bundle không
}
```

### 8.4 Tích hợp trong Pre-Buy Filter

Module 4 chạy **SAU** Module 1, 3, 5 (vì cần `creation_slot`):

```rust
// File: pre_buy_filter.rs
let genesis_result = if config.genesis_detector_enabled {
    let total_supply = get_total_supply(mint).await;
    check_genesis_bundles(mint, slot, total_supply, ...).await
};
```

### 8.5 Files liên quan

| File | Vai trò |
|------|---------|
| `src/modules/anti_rug/genesis_detector.rs` | Logic chính Module 4 |
| `src/modules/anti_rug/pre_buy_filter.rs` | Gọi check_genesis_bundles() |
| `src/modules/anti_rug/config.rs` | Cấu hình ngưỡng |

---

## 9. Test Module 5: Metadata Checker

### Module 5 làm gì?
Kiểm tra **Metaplex on-chain metadata** của token. Token hợp lệ thường có URI trỏ tới website/logo.
Token không có metadata → dấu hiệu scam/low-effort → **WARN** (chỉ cảnh báo, không block).

### Cách test

#### Bước 1: Chạy bot
```powershell
cargo run --bin sniper_mode
```

#### Bước 2: Quan sát log trong PowerShell
**Token có metadata (PASS):**
```
[ANTI-RUG] Mint: 7xKX...abc | Verdict: PASS | Has Metadata: true | Duration: 650ms
```

**Token không có metadata (WARN):**
```
[ANTI-RUG] Mint: 7xKX...abc | Verdict: WARN | Has Metadata: false
[M5-Metadata] Token has no metadata URI
```

> **Lưu ý:** Module 5 chỉ **WARN**, không FAIL — vì một số token hợp lệ cũng có thể chưa kịp thêm metadata.

### Cấu hình Module 5

| Tham số | Giá trị mặc định | Ý nghĩa |
|---------|-------------------|---------|
| `metadata_checker_enabled` | `true` | Bật/tắt Module 5 |
| `filter_timeout_ms` | `1500` | Timeout cho RPC call |

---

## 10. Cách hoạt động Module 5 (chi tiết kỹ thuật)

### 10.1 Luồng xử lý

```
check_has_metadata(mint, timeout_ms)
    ↓
Derive Metaplex PDA:
  seeds = ["metadata", METAPLEX_PROGRAM_ID, mint]
    ↓
RPC: get_account(metadata_pda)
    ↓
Account tồn tại?
  NO  → has_uri = false → WARN
  YES → Parse binary data
    ↓
Parse: key(1) + update_auth(32) + mint(32) + name(4+N) + symbol(4+N) + uri(4+N)
    ↓
URI rỗng? → WARN
URI có giá trị? → PASS
```

### 10.2 Parse Metaplex binary data

```rust
// File: metadata_checker.rs — parse_metadata_uri()

// Metaplex Metadata layout:
// Byte 0:      key (enum, 1 byte)
// Byte 1-32:   update_authority (32 bytes)
// Byte 33-64:  mint (32 bytes)
// Byte 65+:    name (4 bytes length + N bytes data)
//              symbol (4 bytes length + N bytes data)
//              uri (4 bytes length + N bytes data)  ← CẦN LẤY

let offset = 65; // Skip key + update_auth + mint
let name = read_length_prefixed_string(data, &mut pos);
let _symbol = read_length_prefixed_string(data, &mut pos);
let uri = read_length_prefixed_string(data, &mut pos);

let has_uri = !uri.trim().is_empty();
```

### 10.3 Struct MetadataCheckResult

```rust
pub struct MetadataCheckResult {
    pub metadata_account_exists: bool,  // Account có tồn tại
    pub has_uri: bool,                  // URI có giá trị
    pub uri: Option<String>,            // URI value
    pub name: Option<String>,           // Token name
}
```

### 10.4 Tích hợp trong Pre-Buy Filter

Chạy **song song** với Module 1 và Module 3:

```rust
// File: pre_buy_filter.rs
let (holder_result, dev_result, meta_result) = join!(
    check_holder_concentration(),  // Module 1
    check_dev_wallet(),            // Module 3
    check_has_metadata(),          // Module 5 ← ĐÂY
);

// Kết quả: chỉ WARN, không FAIL
if !has_metadata {
    warn_reasons.push("[M5-Metadata] Token has no metadata URI");
}
```

### 10.5 Files liên quan

| File | Vai trò |
|------|---------|
| `src/modules/anti_rug/metadata_checker.rs` | Logic chính Module 5 (187 dòng) |
| `src/modules/anti_rug/pre_buy_filter.rs` | Gọi check_has_metadata() |
| `src/modules/anti_rug/config.rs` | Cấu hình bật/tắt |

---

## 11. Xử lý sự cố

### Bot không phản hồi trên Telegram
- Kiểm tra `TELEGRAM_BOT_TOKEN` trong `.env` có đúng không
- Vào `@BotFather` → `/revoke` → tạo token mới

### Lỗi gRPC "dns error"
- gRPC endpoint không hợp lệ hoặc không hỗ trợ Yellowstone
- Đăng ký [Helius Developer](https://helius.dev) ($49/tháng) hoặc [Triton](https://triton.one) (free)
- Bot vẫn chạy Telegram UI bình thường, chỉ không nhận migration events

### Lỗi OpenSSL (mở PowerShell mới)
```powershell
$env:OPENSSL_DIR="C:\Program Files\OpenSSL-Win64"
$env:OPENSSL_LIB_DIR="C:\Program Files\OpenSSL-Win64\lib\VC\x64\MD"
$env:OPENSSL_INCLUDE_DIR="C:\Program Files\OpenSSL-Win64\include"
```

### Lỗi "linking with link.exe failed: ___chkstk_ms"
```powershell
$env:CC=""
$env:CXX=""
Remove-Item -Recurse -Force "target\debug\build\zstd-sys*"
Remove-Item -Recurse -Force "target\debug\deps\*zstd*"
cargo run --bin sniper_mode
```

### Dừng bot
Nhấn **Ctrl + C** trong cửa sổ PowerShell.

---

> **Tất cả 5 modules đã hoàn thành!** M1 ✅ | M2 ✅ | M3 ✅ | M4 ✅ | M5 ✅
