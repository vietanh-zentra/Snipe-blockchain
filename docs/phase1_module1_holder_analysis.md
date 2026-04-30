# Module 1: Pre-Migration Holder Analysis — Giải thích chi tiết

---

## 🎯 Mục đích: Nó giải quyết vấn đề gì?

Tưởng tượng tình huống thực tế:

```
Dev tạo token → dùng 10 ví ẩn mua 70% supply với giá rẻ
→ Token migrate lên PumpSwap
→ Bot thấy migration → MUA NGAY
→ Dev bán 70% supply → giá sập 90%
→ Bot MẤT TIỀN
```

**Module 1 ngăn chặn điều này** bằng cách kiểm tra: *"Ai đang nắm giữ bao nhiêu % token này?"* TRƯỚC khi mua.

---

## 🔬 Cơ chế hoạt động — Step by step

```
Token migrate phát hiện
        ↓
Bot gọi Module 1 (trước khi mua)
        ↓
┌─────────────────────────────────────────┐
│  Solana RPC: get_token_largest_accounts │
│  → Lấy danh sách 10 ví có nhiều token  │
│     nhất                                │
│                                         │
│  Ví dụ kết quả:                        │
│  Wallet A: 25,000,000 tokens  (25%)    │
│  Wallet B: 18,000,000 tokens  (18%)    │
│  Wallet C:  5,000,000 tokens   (5%)    │
│  ... (7 ví nữa)                        │
└─────────────────────────────────────────┘
        ↓
┌─────────────────────────────────────────┐
│  Solana RPC: get_token_supply          │
│  → Total supply = 100,000,000 tokens   │
└─────────────────────────────────────────┘
        ↓
┌─────────────────────────────────────────┐
│  Tính toán:                            │
│  top10_pct = sum(top10) / total * 100  │
│  = (25% + 18% + 5% + ...) = 65%       │
└─────────────────────────────────────────┘
        ↓
┌─────────────────────────────────────────┐
│  So sánh với ngưỡng (mặc định: 30%):  │
│  65% > 30% → ❌ FAIL → KHÔNG MUA      │
│  25% < 30% → ✅ PASS → CHO PHÉP MUA  │
└─────────────────────────────────────────┘
```

---

## 📊 Ví dụ thực tế — 3 kịch bản

| Token | Top 10 nắm giữ | Verdict | Lý do |
|-------|----------------|---------|-------|
| Token A (rug) | 78% | ❌ FAIL | Dev + đồng bọn gom hết |
| Token B (nghi ngờ) | 35% | ⚠️ WARN | Hơi tập trung, cẩn thận |
| Token C (healthy) | 22% | ✅ PASS | Phân bổ hợp lý |

---

## 💡 Tại sao ngưỡng 30% là hợp lý?

```
Token mới migrate từ bonding curve (PumpSwap):
- Bonding curve = nhiều người mua nhỏ lẻ theo giá tăng dần
- Token "sạch": supply rải rác nhiều holders
- Token "bẩn": dev/bots gom phần lớn supply lúc giá còn thấp

Ngưỡng 30%:
- < 30%: Top 10 nắm dưới 30% → supply phân tán → ít rủi ro
- > 30%: Tập trung quá → 1-2 ví bán là giá sập
```

---

## ⚡ Về mặt kỹ thuật — Code đã implement

File `holder_analyzer.rs` đã tạo làm đúng những bước trên:

```rust
// Bước 1: Lấy top holders
let largest_accounts = RPC_CLIENT
    .get_token_largest_accounts(mint).await?;

// Bước 2: Lấy total supply
let supply_response = RPC_CLIENT
    .get_token_supply(mint).await?;

// Bước 3: Tính %
let top10_pct = (top10_sum / total_supply_ui) * 100.0;

// Bước 4: So sánh ngưỡng
if top10_pct > max_top10_pct {
    return Err("Top 10 holders own 65% supply (max: 30%)");
}
```

**Timeout 1.5s**: Nếu RPC chậm/không trả lời trong 1.5 giây → tự động bỏ qua filter này (không block lệnh mua) để tránh miss cơ hội vì node lag.

---

## 🎛️ User có thể điều chỉnh gì?

Qua **Telegram UI** (chưa implement, sẽ làm hôm nay):

| Tham số | Default | Ý nghĩa |
|---------|---------|---------|
| `holder_filter_enabled` | `true` | Bật/tắt module này |
| `max_top10_holder_pct` | `30%` | Điều chỉnh ngưỡng chặt/lỏng |
| `warn_only` | `true` (ban đầu) | Chỉ log, chưa block mua thật |

> **Lưu ý**: Ban đầu để `warn_only = true` để thu thập data thực tế 24-48h, sau đó calibrate ngưỡng 30% có phù hợp không rồi mới set `warn_only = false`.

---

## ✅ Tóm tắt một câu

> **Module 1 hỏi: "Token này có bị gom bởi ít người quá không?" — Nếu có → bỏ qua, vì chỉ cần 1-2 ví bán là bot lỗ ngay.**
