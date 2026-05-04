# ============================================================
# Stage 1: Build frontend (WASM via Trunk + Dioxus)
# ============================================================
FROM rust:1.91-slim AS frontend
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

# Install wasm32 target + trunk + wasm-bindgen
RUN rustup target add wasm32-unknown-unknown \
    && cargo install --locked trunk@0.21.5 \
    && cargo install --locked wasm-bindgen-cli@0.2.95

# Copy sources required for trunk build
COPY Cargo.toml Cargo.lock Trunk.toml index.html ./
COPY src ./src
COPY styles ./styles
COPY assets ./assets

# Build the WASM frontend (release mode via Trunk.toml)
RUN trunk build --release

# ============================================================
# Stage 2: Build backend server (Rust musl static)
# ============================================================
FROM rust:1.91-slim AS backend
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
# Stage 3: Final minimal runtime image
# ============================================================
FROM alpine:3.20
WORKDIR /app
RUN apk add --no-cache ca-certificates curl

# Backend binary
COPY --from=backend /app/target/x86_64-unknown-linux-musl/release/woody-weed-bot-server ./

# Freshly built WASM frontend
COPY --from=frontend /app/dist ./dist

# Static assets (CSS, images) — also referenced from /assets in code
COPY styles ./styles
COPY assets ./assets
COPY migrations ./migrations

# Railway sets PORT automatically
EXPOSE 8080
CMD ["./woody-weed-bot-server"]
