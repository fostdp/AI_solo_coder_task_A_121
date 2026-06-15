FROM rust:1.80-alpine3.20 AS builder

RUN apk add --no-cache musl-dev perl make

WORKDIR /app

COPY backend/Cargo.toml backend/Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs

RUN cargo build --release --target x86_64-unknown-linux-musl 2>/dev/null || true

COPY backend/src ./src
COPY config ./config

RUN touch src/main.rs && cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:3.20

RUN apk add --no-cache ca-certificates tzdata curl

ENV TZ=Asia/Shanghai
ENV RUST_BACKTRACE=1
ENV RUST_LOG=info,bridge_monitoring_backend=debug

WORKDIR /app

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/bridge_monitoring_backend /usr/local/bin/
COPY frontend ./frontend
COPY config ./config

RUN addgroup -S app && adduser -S app -G app
USER app

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/api/v1/health || exit 1

EXPOSE 8080

CMD ["bridge_monitoring_backend"]
