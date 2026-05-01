# Kế Hoạch Fix 13 Vấn Đề Audit — Anti-Rug Intelligence Layer

## 🔴 THIẾU (5 vấn đề — cần bổ sung ngay)

### #1: DB log KHÔNG được gọi
- **Vấn đề:** `log_anti_rug_filter_result()` có trong `db.rs` nhưng không ai gọi
- **Fix:** Thêm gọi hàm này trong `execute_trade.rs` sau khi filter chạy xong
- **File:** `execute_trade.rs`

### #2: Telegram alert không có
- **Vấn đề:** Khi filter skip token → chỉ `info!()`, không gửi Telegram
- **Fix:** Thêm `enqueue_token_filtered_alert()` gửi message qua Telegram channel
- **File:** `execute_trade.rs`, tạo mới alert function

### #3: Telegram UI toggle chưa tích hợp
- **Vấn đề:** `AntiRugConfig` hardcode default, không đọc từ `BOT_RUN_STATE`
- **Fix:** Đã có `run_state.anti_rug` trong execute_trade.rs — cần verify chuỗi kết nối
- **File:** Verify config flow

### #4: Panic-sell alert chỉ log console
- **Vấn đề:** `log_panic_sell_alert()` chỉ `info!()`, không gửi Telegram/DB
- **Fix:** Thêm Telegram alert + DB log khi panic sell trigger
- **File:** `panic_sell.rs`

### #5: `creation_slot` luôn = None
- **Vấn đề:** `evaluate_token(&mint, &dev, None, ...)` → Module 4 không bao giờ chạy
- **Fix:** Lấy slot từ `TokenDatabaseSchema` hoặc RPC
- **File:** `execute_trade.rs`, `token_db_schema.rs`

---

## 🟡 HẠN CHẾ (8 vấn đề — tối ưu)

### #6: `warn_only: true` mặc định
- **Fix:** Giữ nguyên cho dev, note rõ cần đổi `false` cho production

### #7: Jito tip quá thấp
- **Fix:** Tăng default từ 100,000 → 1,000,000 lamports (0.001 SOL)

### #8: Panic-sell polling 500ms (chậm hơn gRPC)
- **Fix:** OK cho v1, note cần upgrade lên gRPC stream sau

### #9: Genesis detector dùng `format!("{:?}", tx)` + `contains()`
- **Fix:** Thay bằng check `post_token_balances` trực tiếp, bỏ serialize string

### #10: Module 5 chỉ return bool
- **Fix:** Mở rộng `AntiRugFilterResult` thêm field `metadata_uri`, `token_name`

### #11: Không có unit test cho Module 2,3,4
- **Fix:** Thêm basic unit tests

### #12: `rand::RngExt` API
- **Fix:** Verify Cargo.toml đúng version rand

### #13: Panic-sell monitor không cancel khi đã bán
- **Fix:** Cancel handle khi `TokenSellStatus` chuyển thành `SellTradeSubmitted`

---

## Thứ tự thực hiện
1. Fix #1 (DB log) + #5 (creation_slot) — critical path
2. Fix #9 (genesis contains) + #7 (Jito tip) — quick fixes
3. Fix #13 (monitor cancel) — resource leak
4. Fix #10 (metadata detail) — data quality
5. Fix #2 + #4 (Telegram alerts) — cần tìm Telegram sender channel
6. Fix #3 (UI toggle) — verify config flow
7. Fix #11 (unit tests) — last
