# ============================================================
# Stage 1: Build frontend (WASM via Trunk + Dioxus)
# ============================================================
FROM rust:1.91-slim AS frontend
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config ca-certificates curl xz-utils brotli \
    && rm -rf /var/lib/apt/lists/*

# Add wasm32 target
RUN rustup target add wasm32-unknown-unknown

# Install trunk + wasm-bindgen-cli + wasm-opt from prebuilt binaries
RUN curl -fsSL https://github.com/trunk-rs/trunk/releases/download/v0.21.5/trunk-x86_64-unknown-linux-gnu.tar.gz \
        | tar -xz -C /usr/local/bin trunk \
    && curl -fsSL https://github.com/rustwasm/wasm-bindgen/releases/download/0.2.121/wasm-bindgen-0.2.121-x86_64-unknown-linux-musl.tar.gz \
        | tar -xz --strip-components=1 -C /usr/local/bin wasm-bindgen-0.2.121-x86_64-unknown-linux-musl/wasm-bindgen \
    && curl -fsSL https://github.com/WebAssembly/binaryen/releases/download/version_129/binaryen-version_129-x86_64-linux.tar.gz \
        | tar -xz -C /tmp \
    && mv /tmp/binaryen-version_129/bin/wasm-opt /usr/local/bin/wasm-opt \
    && rm -rf /tmp/binaryen-version_129 \
    && trunk --version && wasm-bindgen --version && wasm-opt --version

# Copy sources required for trunk build
COPY Cargo.toml Cargo.lock Trunk.toml index.html build.rs ./
COPY src ./src
COPY styles ./styles
COPY assets ./assets

# Build the WASM frontend (release mode via Trunk.toml)
# BUILD_VERSION arg lets CI inject a deterministic version when .git is absent.
ARG BUILD_VERSION=docker
ENV BUILD_VERSION_OVERRIDE=$BUILD_VERSION
# Shrink WASM binary: optimize for size and abort on panic
ENV CARGO_PROFILE_RELEASE_OPT_LEVEL=z
ENV CARGO_PROFILE_RELEASE_PANIC=abort
RUN trunk build --release
# Run wasm-opt manually because Trunk's bundled version is too old for modern
# rustc features (bulk-memory / nontrapping-float-to-int).
RUN find dist -name '*.wasm' -exec wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int {} -o {} \;
# Pre-compress static assets so we can serve them directly without runtime CPU overhead.
RUN find dist -type f \( -name '*.html' -o -name '*.js' -o -name '*.css' -o -name '*.wasm' -o -name '*.svg' \) \
    -exec gzip -9 -k {} \; \
    -exec brotli -q 11 -k {} \;

# SRI disabled via Trunk.toml no_sri=true — no sed stripping needed

# ============================================================
# Stage 2: Build backend server (Rust musl static)
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

# Freshly built WASM frontend
COPY --from=frontend /app/dist ./dist

# Static assets (CSS, images) — also referenced from /assets in code
COPY styles ./styles
COPY assets ./assets
COPY migrations ./migrations

# Railway sets PORT automatically - no EXPOSE needed
CMD ["./woody-weed-bot-server"]
