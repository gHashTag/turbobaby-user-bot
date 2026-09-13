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
    cargo build --release --target x86_64-unknown-linux-musl --features backend --bin turbobaby-bot-server || true && \
    rm -rf src

# Build server. We include a SOURCE_CACHE_BUST arg so a forced bump
# invalidates this layer and makes Railway rebuild the Rust binary even when
# only backend source changed in a way Docker didn't detect.
ARG BUILD_VERSION=docker
ARG SOURCE_CACHE_BUST=24
ENV BUILD_VERSION_OVERRIDE=$BUILD_VERSION
COPY src ./src
COPY migrations ./migrations
RUN echo "source-bust=${SOURCE_CACHE_BUST}" && \
    find src -type f -exec touch {} + && \
    cargo build --release --target x86_64-unknown-linux-musl --features backend --bin turbobaby-bot-server

# ============================================================
# Stage 3: Final minimal runtime image
# ============================================================
FROM alpine:3.20
WORKDIR /app
RUN apk add --no-cache ca-certificates curl

# Backend binary
COPY --from=backend /app/target/x86_64-unknown-linux-musl/release/turbobaby-bot-server ./

# Force Railway to rebuild the COPY layer — busts Docker cache when dist changes.
ARG DIST_CACHE_BUST=79
ENV DIST_CACHE_BUST=$DIST_CACHE_BUST

# Freshly built WASM frontend (pre-built in CI or committed to repo)
COPY dist ./dist
# Trunk names the bundle after the Cargo target. Validate the output by shape,
# not by a product name that can drift during a rename, and fail the image build
# when either half is missing. A warning-only check would ship a backend that
# serves no front end.
RUN set -eu; \
    js_count="$(find dist -maxdepth 1 -type f -name '*.js' | wc -l | tr -d ' ')"; \
    wasm_count="$(find dist -maxdepth 1 -type f -name '*_bg.wasm' | wc -l | tr -d ' ')"; \
    if [ "$js_count" -ne 1 ] || [ "$wasm_count" -ne 1 ]; then \
      echo "expected exactly one top-level Trunk JS/WASM bundle; found js=$js_count wasm=$wasm_count" >&2; \
      exit 1; \
    fi; \
    ls -la dist/*.js dist/*_bg.wasm

# Static assets (CSS, images) — also referenced from /assets in code
COPY styles ./styles
COPY assets ./assets
COPY migrations ./migrations

# Railway sets PORT automatically - no EXPOSE needed
CMD ["./turbobaby-bot-server"]
