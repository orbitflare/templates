FROM rust:1.93-slim AS builder

RUN apt-get update && apt-get install -y \
    protobuf-compiler libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY jetstream_protos/ jetstream_protos/
COPY copy-trader/ copy-trader/
COPY migrations/ migrations/

RUN cargo build --release --bin copy-trader

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/copy-trader /usr/local/bin/
COPY config.example.yml /app/config.yml

WORKDIR /app
ENTRYPOINT ["copy-trader"]
CMD ["--config", "/app/config.yml"]
