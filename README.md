# Telegram Migration Sniper Bot (Docker Compose)

This project runs with 3 separate services:

- `postgres`: PostgreSQL database
- `init_db`: initializes/migrates DB tables
- `migration_sniper_bot`: main sniper + Telegram UI bot

All runtime credentials are read from `.env`.

## 1) Configure `.env`

Fill these required fields:

- `TELEGRAM_BOT_TOKEN`
- `ALLOWED_TELEGRAM_USER_ID`
- `WALLET_ENCRYPTION_PASSWORD`
- `POSTGRES_USER`
- `POSTGRES_PASSWORD`
- `POSTGRES_DB`

Optional:

- `POSTGRES_PORT` (default `5432`)
- `POSTGRES_HOST` (ignored in Compose services; Compose sets it to `postgres`)
- `SOLSCAN_CLUSTER` — empty or unset for **mainnet** Solscan links; set to `devnet` or `testnet` so token/tx links include `?cluster=...`

## 2) Build images

```bash
docker compose build
```

## 3) Start PostgreSQL only

```bash
docker compose up -d postgres
```

## 4) Initialize DB (create/migrate tables)

```bash
docker compose run --rm init_db
```

This will connect to PostgreSQL and create required tables automatically.

## 5) Start sniper bot service

```bash
docker compose up -d migration_sniper_bot
```

## Useful commands

- Follow bot logs:

```bash
docker compose logs -f migration_sniper_bot
```

- Follow postgres logs:

```bash
docker compose logs -f postgres
```

- Stop everything:

```bash
docker compose down
```

- Stop and remove DB data too:

```bash
docker compose down -v
```
