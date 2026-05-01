# 🔍 BÁO CÁO KIỂM TRA CODE — GÓC NHÌN KHÁCH HÀNG
# Anti-Rug Intelligence Layer Review

**Reviewer:** Client (Simulated)  
**Ngày:** 2026-05-01  
**Phiên bản:** Phase 1 Final

---

## 🟢 ĐIỂM MẠNH (Đã làm tốt)

### 1. Kiến trúc module rõ ràng ✅
- 9 file trong `anti_rug/`, mỗi file có nhiệm vụ riêng
- Separation of concerns tốt: config / filter / result / orchestrator
- Module exports gọn gàng qua `mod.rs`

### 2. Logic gốc được bảo toàn ✅
- `execute_pumpswap_buy()` — không thay đổi
- `execute_pumpswap_sell()` — không thay đổi
- Chỉ refactor cấu trúc trong `execute_trade()`, logic behavior giữ nguyên

### 3. Error handling tốt ✅
- Không có `unwrap()` trong production path
- RPC error → trả `Ok(None)` thay vì crash — tránh false positive
- Timeout cho mỗi RPC call (1500ms default)

### 4. Async performance ✅
- `tokio::join!` chạy M1, M3, M5 song song — giảm latency
- DB log fire-and-forget (`tokio::spawn`) — không block trade execution
- Alert sender fire-and-forget

### 5. Documentation tốt ✅
- `//!` module docs cho mỗi file
- `///` function docs cho public functions
- Comments tiếng Việt giải thích logic

---

## 🔴 LỖI NGHIÊM TRỌNG (Critical Bugs)

### BUG-1: Panic-Sell Monitor Handle bị DROP ngay lập tức ⚠️⚠️⚠️

**File:** `execute_trade.rs` dòng 181  
**Vấn đề:**
```rust
let _handle = start_panic_sell_monitor(ctx);
// Fix #13: handle sẽ tự cancel khi drop (implement Drop trait)
// TODO: Lưu handle vào map để cancel khi TP/SL bán xong
```

`_handle` là local variable → bị **DROP ngay cuối block `for`** → trigger `cancel()` → monitor **DỪNG NGAY** sau vài milliseconds!

**Hậu quả:** Module 2 (Panic-Sell) **KHÔNG BAO GIỜ HOẠT ĐỘNG** trong thực tế. Dev dump token → bot KHÔNG bán kịp → **mất tiền**.

**Fix cần thiết:**
```rust
// Cần lưu handle vào global map:
lazy_static! {
    static ref PANIC_SELL_HANDLES: DashMap<Pubkey, PanicSellMonitorHandle> = DashMap::new();
}
// Trong execute_trade:
PANIC_SELL_HANDLES.insert(token_data.token_mint, handle);
// Khi sell xong:
PANIC_SELL_HANDLES.remove(&mint);
```

**Mức độ:** 🔴 CRITICAL — Module 2 hoàn toàn vô dụng

---

### BUG-2: DB Connection mở mới mỗi lần log filter ⚠️

**File:** `execute_trade.rs` dòng 108-109  
**Vấn đề:**
```rust
tokio::spawn(async move {
    if let Ok(db_url) = resolve_database_url_from_env() {
        if let Ok(db) = sea_orm::Database::connect(&db_url).await {
            // ...
        }
    }
});
```

Mỗi token migration → mở **1 DB connection mới** → không dùng connection pool → tốn tài nguyên, có thể **exhaust PostgreSQL connections** khi traffic cao.

**Fix:** Dùng shared `DatabaseConnection` (connection pool) thay vì tạo mới mỗi lần.

**Mức độ:** 🟡 MEDIUM — chạy được nhưng không tối ưu

---

## 🟡 VẤN ĐỀ TRUNG BÌNH (Medium Issues)

### ISSUE-3: `warn_only` mode — Token WARN vẫn bị MUA

**File:** `execute_trade.rs` dòng 128  
```rust
if filter_result.verdict.is_fail() && !anti_rug_cfg.warn_only {
    continue; // Skip
}
```

Khi `warn_only = true`:
- Token **FAIL** → vẫn mua (đúng ý — warn only)
- Nhưng **không có log khác biệt** giữa token PASS vs FAIL khi warn_only

**Khuyến nghị:** Thêm log rõ ràng khi warn_only cho phép mua token nghi ngờ.

---

### ISSUE-4: Genesis Detector mặc định TẮT

**File:** `config.rs` dòng 59  
```rust
genesis_detector_enabled: false, // Tắt mặc định vì tốn CU nhiều
```

Module 4 là module phát hiện rug phổ biến nhất (bundled buy) nhưng mặc định **TẮT**. Khách hàng không biết phải bật → giảm hiệu quả anti-rug.

**Khuyến nghị:** Bật mặc định hoặc có hướng dẫn rõ ràng trên Telegram UI.

---

### ISSUE-5: Metadata Checker chỉ WARN, không FAIL

**File:** `pre_buy_filter.rs` dòng 149-151  
```rust
if !has_metadata {
    warn_reasons.push("[M5-Metadata] Token has no metadata URI".to_string());
}
```

Token **không có metadata** (tên, ảnh, URI) là dấu hiệu rug rất rõ nhưng chỉ WARN → bot vẫn mua.

**Khuyến nghị:** Cho phép config chuyển metadata check thành FAIL mode.

---

### ISSUE-6: Dev Profiler dùng RPC timeout nhưng trả về Error

**File:** `dev_wallet_profiler.rs` dòng 52-54  
```rust
Err(_) => {
    return Err("Dev wallet RPC timeout".into());
}
```

Khi timeout → trả `Err` → `pre_buy_filter` xử lý thành **FAIL**. Nhưng `holder_analyzer` timeout → trả `Ok(None)` → **PASS**.

**Không nhất quán:** Cùng timeout nhưng M1 cho pass, M3 cho fail.

**Khuyến nghị:** Thống nhất: timeout = skip filter (không block), giống M1.

---

### ISSUE-7: Không có rate limiting cho Telegram alerts

**File:** `alert_sender.rs`  
Nếu nhiều token bị skip liên tục → gửi hàng loạt messages → có thể bị **Telegram rate limit** (max ~30 messages/second).

**Khuyến nghị:** Thêm debounce hoặc batch alerts.

---

## 🟠 VẤN ĐỀ NHỎ (Minor Issues)

### ISSUE-8: Unit tests không test async functions
- 16 tests đều là synchronous (`#[test]`)
- Không test RPC calls thực tế (cần mock hoặc `#[tokio::test]`)
- Chỉ test logic tính toán thuần túy

### ISSUE-9: `creation_slot` fetch thêm latency
```rust
let creation_slot = match RPC_CLIENT
    .get_signatures_for_address(&mint)
    .await { ... };
```
Thêm 1 RPC call **TRƯỚC** filter → tăng latency ~200-500ms → chậm hơn sniper không có anti-rug.

### ISSUE-10: Hardcoded Jito tip 8 accounts
Nếu Jito thay đổi tip accounts → bot gửi tip sai → bundle bị reject → panic sell thất bại.

### ISSUE-11: `rand::RngExt` API unstable
```rust
use rand::RngExt;
let idx = rand::rng().random_range(0..JITO_TIP_ACCOUNTS.len());
```
`RngExt` là API mới, có thể thay đổi trong version rand tiếp theo.

---

## 📊 TỔNG KẾT ĐÁNH GIÁ

| Hạng mục | Điểm | Ghi chú |
|----------|------|---------|
| Kiến trúc | 9/10 | Modular, clean separation |
| Code quality | 8/10 | Documented, consistent style |
| Error handling | 7/10 | Tốt nhưng không nhất quán (ISSUE-6) |
| Testing | 5/10 | Chỉ unit test sync, thiếu integration |
| Performance | 7/10 | Song song tốt, nhưng thêm latency (ISSUE-9) |
| Security | 6/10 | BUG-1 làm M2 vô dụng |
| Production readiness | 6/10 | Cần fix BUG-1 trước khi nạp tiền |

### ⚠️ KHUYẾN NGHỊ TRƯỚC KHI PRODUCTION

1. **FIX NGAY BUG-1** — Panic-Sell handle bị drop → M2 không hoạt động
2. **FIX BUG-2** — Dùng connection pool cho DB
3. **Bật Genesis Detector** — Module quan trọng nhất lại bị tắt
4. **Thống nhất timeout behavior** — Tất cả modules nên xử lý timeout giống nhau
5. **Test với tiền thật nhỏ** — 0.01 SOL để verify toàn bộ flow

---

> **Kết luận:** Code đạt chất lượng tốt cho Phase 1 prototype. Tuy nhiên BUG-1 (Panic-Sell handle drop) là lỗi nghiêm trọng cần fix trước khi đưa vào production. Các module filter (M1, M3, M5) hoạt động đúng logic. Module M4 cần được bật mặc định.
