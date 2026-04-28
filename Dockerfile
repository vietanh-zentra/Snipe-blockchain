FROM rust:1.86-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY jito_protos ./jito_protos
COPY src ./src

RUN cargo build --release --bin init_db --bin sniper_mode

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/init_db /usr/local/bin/init_db
COPY --from=builder /app/target/release/sniper_mode /usr/local/bin/sniper_mode

ENV RUST_LOG=info

