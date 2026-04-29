FROM rust:1.91-slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

# Cache dependencies layer
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs && \
    cargo build --release || true && rm -rf src

# Copy source — build online using DATABASE_URL from Railway env
COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates libssl3 curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/woody-weed-bot ./
COPY --from=builder /app/migrations ./migrations
ENV PORT=3000
EXPOSE 3000
CMD ["./woody-weed-bot"]
