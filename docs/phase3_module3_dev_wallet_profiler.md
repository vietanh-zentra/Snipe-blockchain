# Module 3: Dev Wallet Profiler

## Mô tả
Trước khi mua token, Module 3 kiểm tra **lịch sử giao dịch của ví dev**. 
Ví dev mới tạo (ít TX, tuổi vài giờ) → nguy cơ rug cao → **SKIP** hoặc **WARN**.

## Luồng hoạt động

```
Bot phát hiện token migration
    ↓
Pre-Buy Filter (pre_buy_filter.rs)
    ↓
┌─── Module 1: Holder Concentration ───┐
│   (chạy song song)                   │
├─── Module 3: Dev Wallet Profiler ────┤  ← BẠN ĐANG Ở ĐÂY
│   (chạy song song)                   │
├─── Module 5: Metadata Checker ───────┤
│   (chạy song song)                   │
└──────────────────────────────────────┘
    ↓
Tổng hợp kết quả → PASS / WARN / FAIL
    ↓
PASS → Mua token
FAIL → Skip token
```

## Chi tiết kỹ thuật

### 1. Module 3 làm gì?

Gọi RPC lấy 50 TX gần nhất của ví dev → phân tích:

| Tiêu chí | Ngưỡng mặc định | Kết quả |
|-----------|-----------------|---------|
| Số TX lịch sử | ≥ 10 TX | PASS |
| Số TX lịch sử | < 10 TX | FAIL |
| Tuổi ví | Tính từ TX cũ nhất | Chỉ log, không block |

### 2. Cách hoạt động

```
check_dev_wallet(dev_pubkey, min_tx_count=10, timeout=1500ms)
    ↓
RPC: get_signatures_for_address_with_config(dev_pubkey, limit=50)
    ↓
Đếm số TX trả về
    ↓
TX cũ nhất → tính tuổi ví (giờ)
    ↓
Nếu tx_count < 10 → Err("Dev wallet has only 3 TXs (min: 10). Age: 2 hours")
Nếu tx_count ≥ 10 → Ok(Some(tx_count))
```

### 3. Struct DevWalletProfile

```rust
pub struct DevWalletProfile {
    pub tx_count: u64,              // Số TX tìm được (max 50)
    pub oldest_tx_timestamp: Option<i64>,  // Unix timestamp TX cũ nhất
    pub estimated_age_hours: Option<u64>,  // Tuổi ví (giờ)
}
```

### 4. Functions

| Function | Vai trò |
|----------|---------|
| `check_dev_wallet()` | Entry point — gọi bởi pre_buy_filter.rs |
| `analyze_dev_wallet()` | Logic chính — query RPC, build profile |

### 5. Log mẫu khi chạy

**Dev wallet đủ tuổi (PASS):**
```
[ANTI-RUG] Mint: 7xKX...abc | Verdict: PASS | Top10: 22.5% | Dev TX: 45 | Duration: 850ms
```

**Dev wallet mới tạo (FAIL):**
```
[ANTI-RUG] ❌ SKIP 7xKX...abc — [M3-Dev] Dev wallet has only 3 TXs (min: 10). Age: 2 hours
```

### 6. Cấu hình (trong config.rs)

```rust
dev_profiler_enabled: true,    // Bật/tắt Module 3
min_dev_tx_count: 10,          // Số TX tối thiểu để pass
filter_timeout_ms: 1_500,     // Timeout RPC call
```

### 7. Files liên quan

| File | Vai trò |
|------|---------|
| `src/modules/anti_rug/dev_wallet_profiler.rs` | Logic chính Module 3 |
| `src/modules/anti_rug/pre_buy_filter.rs` | Gọi check_dev_wallet() |
| `src/modules/anti_rug/config.rs` | Cấu hình ngưỡng |

### 8. Tích hợp với Pre-Buy Filter

Module 3 chạy **song song** với Module 1 và Module 5 trong `pre_buy_filter.rs`:

```rust
let (holder_result, dev_result, meta_result) = join!(
    // Module 1: check_holder_concentration()
    // Module 3: check_dev_wallet()        ← ĐÂY
    // Module 5: check_has_metadata()
);
```

Tổng thời gian = max(M1, M3, M5) — không cộng dồn!
