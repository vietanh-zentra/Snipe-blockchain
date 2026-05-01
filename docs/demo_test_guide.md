# 🧪 Hướng Dẫn Test Demo — Bot Thật, Sàn Thật, Không Tiền Thật

## Mục đích
Chạy bot trên mainnet Solana thật, Telegram thật, nhưng **ví rỗng = không mất tiền**.
Bot sẽ nhận migration events, chạy Anti-Rug filter, log kết quả — bạn quan sát mọi thứ.

---

## BƯỚC 1: Đảm bảo bot đang chạy trên VPS

SSH vào VPS (nếu chưa):
```bash
ssh root@154.43.52.31 -o PubkeyAuthentication=no
# Password: v)pio=0NZBoP
```

Kiểm tra bot:
```bash
systemctl status sniper-bot
```

Nếu chưa chạy:
```bash
cd /root/Snipe-blockchain && cargo run --release --bin sniper_mode
```

---

## BƯỚC 2: Mở Telegram → tìm bot

1. Mở **Telegram** trên điện thoại hoặc desktop
2. Tìm bot bằng cách paste link: `https://t.me/` + tên bot của bạn
   - Hoặc vào **@BotFather** → `/mybots` → chọn bot → xem username
3. Bấm **Start** hoặc gõ `/start`

Bạn sẽ thấy menu:
```
🖐 Welcome to the Migration Sniper 🤖 Bot!!!

💰 Wallet management    ⚙️ Trading parameters
              ▶️ Start
```

---

## BƯỚC 3: Kiểm tra User ID

Gõ trong chat bot:
```
/myid
```

Bot sẽ trả lời:
```
Your Telegram user id: 5123702171
```

Nếu ID khớp với `.env` → bạn có quyền admin ✅

---

## BƯỚC 4: Tạo ví mới (rỗng, test)

Gõ:
```
/generate
```

Bot sẽ tạo ví Solana mới và hiện:
```
No1. 7xKXabc123...  ✅
Balance: 0 SOL
```

**Ví này rỗng = 0 SOL = bot KHÔNG THỂ mua bất kỳ token nào** → an toàn 100%.

---

## BƯỚC 5: Xem Trading Parameters

Bấm nút **⚙️ Trading parameters**

Bạn sẽ thấy:
```
⚙️ Trading parameters

Buy amount: 0.1 SOL
Slippage: 50%
Take profit: 120%
Stop loss: 80%
...
```

**KHÔNG CẦN SỬA** — vì ví rỗng nên bot không mua được.

---

## BƯỚC 6: Bấm START

Bấm nút **▶️ Start**

Bot trả lời:
```
Bot is started
```

Từ giờ bot đang **lắng nghe migration events** từ PumpSwap trên Solana mainnet.

---

## BƯỚC 7: Quan sát log trên VPS

Quay lại terminal SSH VPS, xem log realtime:

Nếu chạy trực tiếp:
```
(log tự hiện trong terminal)
```

Nếu chạy systemd:
```bash
journalctl -u sniper-bot -f
```

### Log bạn sẽ thấy (khi có token migrate):

```
[ANTI-RUG] Mint: 7xKX...abc | Verdict: warn | Top10: 45.2% | Dev TX: 3 | Duration: 850ms
[ANTI-RUG] ❌ SKIP 7xKX...abc — [M1-Holder] Top 10 holders hold 45.2% (max: 30.0%)
```

Hoặc:
```
[ANTI-RUG] Mint: 9yZM...def | Verdict: pass | Top10: 18.5% | Dev TX: 25 | Duration: 620ms
```

Sau đó bot cố mua nhưng **FAIL vì ví rỗng** (không đủ SOL) → an toàn.

---

## BƯỚC 8: Kiểm tra Telegram Alert

Khi filter skip token, bạn sẽ nhận message trên Telegram:
```
🛡️ Anti-Rug Alert

Token: 7xKX...abc
❌ SKIPPED
Reason: [M1-Holder] Top 10 holders hold 45.2%

Bot sẽ KHÔNG mua token này.
```

---

## BƯỚC 9: Kiểm tra Database (optional)

SSH vào VPS và chạy:
```bash
sudo -u postgres psql -d sniper_db -c "SELECT token_mint, verdict, reject_reason, filter_duration_ms FROM anti_rug_filter_log ORDER BY created_at DESC LIMIT 10;"
```

Bạn sẽ thấy bảng kết quả filter.

---

## BƯỚC 10: Dừng bot khi test xong

Trong Telegram: bấm **⏹ Stop**

Hoặc trên VPS:
```bash
systemctl stop sniper-bot
```

---

## ✅ Checklist "Demo thành công"

| Mục | Kiểm tra |
|-----|----------|
| Bot start trên Telegram | Thấy "Bot is started" |
| /myid trả đúng ID | 5123702171 |
| Ví được tạo | /generate hoạt động |
| Log hiện trên VPS | Thấy [ANTI-RUG] logs |
| Telegram alert | Nhận message khi skip token |
| DB log | Query thấy data |
| Ví rỗng = an toàn | Bot fail mua vì 0 SOL |

---

## ⚠️ Lưu ý quan trọng

- **Ví rỗng = KHÔNG MẤT TIỀN** — bot cố gửi TX mua nhưng sẽ fail
- **warn_only = true** (mặc định) — bot log nhưng không block token
- Muốn test mua thật → nạp 0.01-0.05 SOL vào ví generate
- Muốn production → đổi `warn_only: false` sau khi calibrate 24h
