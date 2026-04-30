# Module 2: Dynamic Panic-Sell via Jito Bundle

## Mô tả
Sau khi bot mua token thành công, Module 2 sẽ **theo dõi ví dev và top holders** liên tục. Khi phát hiện họ bán (balance giảm đột ngột > 20%) → tự động gửi **Jito bundle** để bán token TRƯỚC dev/whale, giảm thiểu thiệt hại rug-pull.

## Luồng hoạt động

```
Bot mua token thành công
    ↓
start_panic_sell_monitor(ctx) ← spawn background task
    ↓
┌─────────── Loop mỗi 500ms ───────────┐
│                                        │
│  get_token_account_balance(dev_wallet) │
│  get_token_account_balance(top_holder) │
│                                        │
│  Nếu balance giảm > 20%:              │
│    ↓                                   │
│    Build sell instructions             │
│    + Jito tip instruction              │
│    ↓                                   │
│    Submit Jito Bundle                  │
│    ↓                                   │
│    Thành công? → Log + Alert           │
│    Thất bại?  → Fallback normal TX     │
│                                        │
└────────────────────────────────────────┘
```

## Chi tiết kỹ thuật

### 1. Jito Bundle là gì?
- Jito cho phép gửi 1-5 transactions cùng lúc dưới dạng "bundle"
- Bundle được ưu tiên xử lý trước transactions thường
- Trả tip cho validator (100,000 lamports ≈ 0.0001 SOL)
- API **miễn phí**, public: `https://mainnet.block-engine.jito.wtf/api/v1/bundles`

### 2. Jito Tip Accounts (chọn ngẫu nhiên 1)
```
96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5
HFqU5x63VTqvQss8hp11i4bVqkfRtQo3EZTJrPaKYWo7
Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY
ADaUMid9yfUytqMBgopwjb2DTLSf5oXbsyq7hPbQELGR
DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh
ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt
DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL
3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT
```

### 3. Files cần sửa

| File | Hành động |
|------|-----------|
| `src/modules/anti_rug/panic_sell.rs` | Thay stub → code đầy đủ |
| `src/features/handle_sniper/execute_trade.rs` | Inject monitor sau khi mua |

### 4. Cấu hình (đã có trong config.rs)
```rust
panic_sell_enabled: true,              // Bật/tắt
panic_sell_jito_tip_lamports: 100_000, // Tip = 0.0001 SOL
panic_sell_watch_top_holders: 3,       // Theo dõi top 3 holders + dev
```

## Lưu ý quan trọng

- **Module 2 hoạt động SAU KHI MUA** — khác Module 1 (pre-buy filter)
- **Mỗi position mở = 1 monitor task riêng** — tự cancel khi bán xong
- **Fallback:** Nếu Jito thất bại → bán qua `send_0slot_transaction()` bình thường
- **Chi phí:** Chỉ tốn tip ~0.0001 SOL mỗi lần trigger (rất rẻ)
