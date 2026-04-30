# Deploy Bot lên VPS Frankfurt

## Thông tin VPS
- **IP:** 154.43.52.31
- **Username:** root
- **Password:** v)pio=0NZBoP
- **Location:** Frankfurt (cùng gRPC + Jito)

---

## Bước 1: SSH vào VPS (từ PowerShell)
```powershell
ssh root@154.43.52.31
```
Password: `v)pio=0NZBoP`

## Bước 2: Cài đặt dependencies (chạy trên VPS)
```bash
# Update system
apt update && apt upgrade -y

# Install build tools
apt install -y build-essential pkg-config libssl-dev protobuf-compiler git curl

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env

# Install PostgreSQL
apt install -y postgresql postgresql-contrib
systemctl start postgresql
systemctl enable postgresql

# Tạo database
sudo -u postgres psql -c "CREATE USER sniper_user WITH PASSWORD 'sniper_pass_123';"
sudo -u postgres psql -c "CREATE DATABASE sniper_db OWNER sniper_user;"
sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE sniper_db TO sniper_user;"
```

## Bước 3: Upload code lên VPS (từ PowerShell local)
```powershell
# Nén project (loại bỏ target/ để nhẹ hơn)
cd C:\Users\ASUS\Snipe-blockchain
tar -czf sniper-bot.tar.gz --exclude='target' --exclude='.git' .

# Upload lên VPS
scp sniper-bot.tar.gz root@154.43.52.31:/root/
```

## Bước 4: Giải nén và build trên VPS
```bash
# SSH vào VPS
ssh root@154.43.52.31

# Giải nén
mkdir -p /root/Snipe-blockchain
cd /root/Snipe-blockchain
tar -xzf /root/sniper-bot.tar.gz

# Build (lần đầu mất ~5-10 phút)
cargo build --release
```

## Bước 5: Khởi tạo database
```bash
cargo run --release --bin init_db
```

## Bước 6: Chạy bot
```bash
# Chạy foreground (test)
cargo run --release --bin sniper_mode

# Chạy background (production)
nohup cargo run --release --bin sniper_mode > bot.log 2>&1 &

# Xem log
tail -f bot.log
```

## Bước 7: Chạy bot như service (tự restart)
```bash
cat > /etc/systemd/service/sniper-bot.service << 'EOF'
[Unit]
Description=Migration Sniper Bot
After=network.target postgresql.service

[Service]
Type=simple
User=root
WorkingDirectory=/root/Snipe-blockchain
ExecStart=/root/.cargo/bin/cargo run --release --bin sniper_mode
Restart=always
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable sniper-bot
systemctl start sniper-bot

# Xem log
journalctl -u sniper-bot -f
```

## Các lệnh quản lý
```bash
systemctl status sniper-bot    # Kiểm tra trạng thái
systemctl stop sniper-bot      # Dừng bot
systemctl restart sniper-bot   # Restart bot
journalctl -u sniper-bot -f    # Xem log realtime
```
