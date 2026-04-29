FROM rust:1.91-slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config musl-tools ca-certificates && \
    rustup target add x86_64-unknown-linux-musl && \
    rm -rf /var/lib/apt/lists/*

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs && \
    cargo build --release --target x86_64-unknown-linux-musl || true && rm -rf src

# Build
COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs && cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:3.20
WORKDIR /app
RUN apk add --no-cache ca-certificates curl
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/woody-weed-bot ./
COPY --from=builder /app/migrations ./migrations
ENV PORT=3000
EXPOSE 3000
CMD ["./woody-weed-bot"]
