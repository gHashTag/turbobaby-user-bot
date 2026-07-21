# ============================================================
# Stage 1: Build backend server (Rust musl static)
# ============================================================
FROM rust:1.91-slim AS backend
WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config musl-tools ca-certificates && \
    rustup target add x86_64-unknown-linux-musl && \
    rm -rf /var/lib/apt/lists/*

# Cache dependencies
COPY Cargo.toml Cargo.lock build.rs ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs && \
    echo 'pub fn run() {}' > src/lib.rs && \
    cargo build --release --target x86_64-unknown-linux-musl --features backend --bin woody-weed-bot-server || true && \
    rm -rf src

# Build server
ARG BUILD_VERSION=docker
ENV BUILD_VERSION_OVERRIDE=$BUILD_VERSION
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

# Force Railway to rebuild the COPY layer — busts Docker cache when dist changes.
ARG DIST_CACHE_BUST=14
ENV DIST_CACHE_BUST=$DIST_CACHE_BUST

# Freshly built WASM frontend (pre-built in CI or committed to repo)
COPY dist ./dist
RUN ls -la dist/woody-weed-bot-*.js dist/woody-weed-bot-*.wasm 2>/dev/null || echo "WASM bundles not found in dist/"

# Static assets (CSS, images) — also referenced from /assets in code
COPY styles ./styles
COPY assets ./assets
COPY migrations ./migrations

# Railway sets PORT automatically - no EXPOSE needed
CMD ["./woody-weed-bot-server"]
