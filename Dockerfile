# ============================================================
# Build backend server only (frontend is pre-built in dist/)
# ============================================================
FROM rust:1.91-slim AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config musl-tools ca-certificates && \
    rustup target add x86_64-unknown-linux-musl && \
    rm -rf /var/lib/apt/lists/*

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs && \
    echo 'pub fn run() {}' > src/lib.rs && \
    cargo build --release --target x86_64-unknown-linux-musl --features backend --bin woody-weed-bot-server || true && \
    rm -rf src

# Build server
COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs && \
    cargo build --release --target x86_64-unknown-linux-musl --features backend --bin woody-weed-bot-server

# ============================================================
# Final minimal image
# ============================================================
FROM alpine:3.20
WORKDIR /app
RUN apk add --no-cache ca-certificates curl

# Backend binary
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/woody-weed-bot-server ./

# Pre-built frontend (WASM app built locally with trunk)
COPY dist ./dist

# Static assets
COPY styles ./styles
COPY assets ./assets
COPY migrations ./migrations

# Railway sets PORT automatically
EXPOSE 8080
CMD ["./woody-weed-bot-server"]
