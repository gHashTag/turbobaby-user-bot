# ============================================================
# Stage 1: Build WASM frontend with Trunk
# ============================================================
FROM rust:1.91-slim AS frontend-builder
WORKDIR /app

# Install trunk + wasm target
RUN rustup target add wasm32-unknown-unknown && \
    cargo install trunk

# Cache Rust dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/lib.rs && \
    cargo build --target wasm32-unknown-unknown --lib || true && rm -rf src

# Build frontend
COPY index.html ./
COPY Trunk.toml ./
COPY styles ./styles
COPY assets ./assets
COPY src ./src
RUN trunk build --release

# ============================================================
# Stage 2: Build backend server (native Linux)
# ============================================================
FROM rust:1.91-slim AS backend-builder
WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config musl-tools ca-certificates && \
    rustup target add x86_64-unknown-linux-musl && \
    rm -rf /var/lib/apt/lists/*

# Cache Rust dependencies
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
# Stage 3: Final minimal image
# ============================================================
FROM alpine:3.20
WORKDIR /app
RUN apk add --no-cache ca-certificates curl

# Backend binary
COPY --from=backend-builder /app/target/x86_64-unknown-linux-musl/release/woody-weed-bot-server ./

# Frontend WASM app (built by trunk)
COPY --from=frontend-builder /app/dist ./dist

# Static assets
COPY styles ./styles
COPY assets ./assets
COPY migrations ./migrations

# Railway sets PORT automatically
EXPOSE 8080
CMD ["./woody-weed-bot-server"]
