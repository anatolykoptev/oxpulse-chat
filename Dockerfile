# syntax=docker/dockerfile:1.4

FROM rust:1.88-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Layer 1: deps only
COPY Cargo.toml Cargo.lock ./
COPY crates/signaling/Cargo.toml crates/signaling/Cargo.toml
COPY crates/turn/Cargo.toml crates/turn/Cargo.toml
COPY crates/server/Cargo.toml crates/server/Cargo.toml
RUN mkdir -p crates/signaling/src crates/turn/src crates/server/src && \
    echo "fn main(){}" > crates/server/src/main.rs && \
    touch crates/signaling/src/lib.rs crates/turn/src/lib.rs crates/server/src/lib.rs
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked --bin oxpulse-chat 2>/dev/null || true

# Layer 2: source — clean workspace crates so cargo rebuilds them
COPY crates/ crates/
COPY config/ config/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo clean -p oxpulse-signaling -p oxpulse-turn -p oxpulse-chat --release 2>/dev/null || true && \
    cargo build --release --locked --bin oxpulse-chat && \
    cp target/release/oxpulse-chat /binary

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /binary /usr/local/bin/oxpulse-chat
COPY assets/room/ /app/room/

ENV PORT=3000
EXPOSE 3000

CMD ["oxpulse-chat"]
