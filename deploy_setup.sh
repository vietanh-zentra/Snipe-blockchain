#!/bin/bash
set -e

echo "========================================="
echo "  🚀 Sniper Bot VPS Setup Script"
echo "  Ubuntu 24.04 LTS"
echo "========================================="

# ── Step 1: System Update ──────────────────────────────
echo ""
echo "📦 [1/6] Updating system packages..."
apt update -y && apt upgrade -y

# ── Step 2: Install Build Dependencies ─────────────────
echo ""
echo "🔧 [2/6] Installing build dependencies..."
apt install -y \
  build-essential \
  pkg-config \
  libssl-dev \
  protobuf-compiler \
  git \
  curl \
  cmake

echo "  ✅ protoc version: $(protoc --version)"

# ── Step 3: Install Rust ───────────────────────────────
echo ""
echo "🦀 [3/6] Installing Rust..."
if command -v rustc &> /dev/null; then
  echo "  ✅ Rust already installed: $(rustc --version)"
else
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source $HOME/.cargo/env
  echo "  ✅ Rust installed: $(rustc --version)"
fi

# Make sure cargo is in PATH for this session
source $HOME/.cargo/env 2>/dev/null || true

# ── Step 4: Install & Configure PostgreSQL ─────────────
echo ""
echo "🐘 [4/6] Setting up PostgreSQL..."
apt install -y postgresql postgresql-contrib
systemctl start postgresql
systemctl enable postgresql

# Create user and database
sudo -u postgres psql -tc "SELECT 1 FROM pg_roles WHERE rolname='sniper_user'" | grep -q 1 || \
  sudo -u postgres psql -c "CREATE USER sniper_user WITH PASSWORD 'Xk9#mP2vLq7@nR4w';"

sudo -u postgres psql -tc "SELECT 1 FROM pg_database WHERE datname='sniper_db'" | grep -q 1 || \
  sudo -u postgres psql -c "CREATE DATABASE sniper_db OWNER sniper_user;"

sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE sniper_db TO sniper_user;" 2>/dev/null || true

echo "  ✅ PostgreSQL ready — user: sniper_user, db: sniper_db"

# ── Step 5: Create .env ───────────────────────────────
echo ""
echo "📝 [5/6] Creating .env file..."

ENV_FILE="/root/Snipe-blockchain/.env"
if [ -f "$ENV_FILE" ]; then
  echo "  ⚠️  .env already exists, backing up to .env.bak"
  cp "$ENV_FILE" "${ENV_FILE}.bak"
fi

mkdir -p /root/Snipe-blockchain

cat > "$ENV_FILE" << 'ENVEOF'
# ═══════════════════════════════════════════════════════════
# TELEGRAM BOT
# ═══════════════════════════════════════════════════════════
TELEGRAM_BOT_TOKEN=8623563315:AAELHdspKIN2O37l4H-xqBy8wwVKBW8u4e4
ALLOWED_TELEGRAM_USER_ID=5123702171

# ═══════════════════════════════════════════════════════════
# WALLET ENCRYPTION (DO NOT LOSE THIS!)
# ═══════════════════════════════════════════════════════════
WALLET_ENCRYPTION_PASSWORD=V3ry$tr0ng_3ncrypt10n_P@ssw0rd_2026!

# ═══════════════════════════════════════════════════════════
# POSTGRESQL
# ═══════════════════════════════════════════════════════════
POSTGRES_USER=sniper_user
POSTGRES_PASSWORD=Xk9#mP2vLq7@nR4w
POSTGRES_DB=sniper_db
POSTGRES_HOST=localhost
POSTGRES_PORT=5432

# ═══════════════════════════════════════════════════════════
# SOLANA RPC + gRPC
# Defaults: public endpoints (rate-limited)
# Upgrade to Helius/QuickNode for production
# ═══════════════════════════════════════════════════════════
# RPC_ENDPOINT=https://mainnet.helius-rpc.com/?api-key=YOUR_KEY
# GRPC_ENDPOINT=https://atlas-mainnet.helius-rpc.com
# GRPC_TOKEN=YOUR_KEY
ENVEOF

chmod 600 "$ENV_FILE"
echo "  ✅ .env created at $ENV_FILE"

# ── Step 6: Create logs directory ──────────────────────
echo ""
echo "📂 [6/6] Creating assets/logs directory..."
mkdir -p /root/Snipe-blockchain/src/assets/logs
echo "  ✅ Logs directory ready"

echo ""
echo "========================================="
echo "  ✅ VPS Setup Complete!"
echo ""
echo "  Next steps:"
echo "  1. Upload source code to /root/Snipe-blockchain/"
echo "  2. cd /root/Snipe-blockchain && cargo build --release"
echo "  3. cargo run --release --bin init_db"
echo "  4. cargo run --release --bin sniper_mode"
echo "========================================="
