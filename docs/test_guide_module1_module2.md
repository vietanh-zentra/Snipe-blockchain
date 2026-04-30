# Hướng Dẫn Test Module 1 & Module 2 + Cách Hoạt Động Module 2

> **Ngày cập nhật:** 01/05/2026
> **Trạng thái:** Module 1 ✅ | Module 2 ✅

---

## Mục Lục
1. [Yêu cầu môi trường](#1-yêu-cầu-môi-trường)
2. [Test Module 1: Holder Concentration Filter](#2-test-module-1-holder-concentration-filter)
3. [Test Module 2: Panic-Sell via Jito Bundle](#3-test-module-2-panic-sell-via-jito-bundle)
4. [Cách hoạt động Module 2 (chi tiết kỹ thuật)](#4-cách-hoạt-động-module-2-chi-tiết-kỹ-thuật)
5. [Xử lý sự cố](#5-xử-lý-sự-cố)

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

## 5. Xử lý sự cố

### Bot không phản hồi trên Telegram
- Kiểm tra `TELEGRAM_BOT_TOKEN` trong `.env` có đúng không
- Vào `@BotFather` → `/revoke` → tạo token mới

### Lỗi gRPC "dns error"
- gRPC endpoint không hợp lệ hoặc không hỗ trợ Yellowstone
- Đăng ký [Helius Developer](https://helius.dev) ($49/tháng) hoặc [Triton](https://triton.one) (free)
- Bot vẫn chạy Telegram UI bình thường, chỉ không nhận migration events

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

> **Tiếp theo:** Module 3 (Dev Wallet Profiler) — kiểm tra tuổi ví dev, lịch sử giao dịch.
