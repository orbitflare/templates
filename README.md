# OrbitFlare Templates

Starter templates for building on Solana.

```bash
cargo install orbitflare
orbitflare template --list
orbitflare template --install <name>
```

| Template | Stack | Description |
|----------|-------|-------------|
| [solana-blinks-axum](./solana-blinks-axum) | Rust, Axum, Next.js | Solana Actions (Blinks) server + frontend. Includes transfer, donate, and swap examples. |
| [solana-copy-trader](./solana-copy-trader) | Rust, Tokio, gRPC, Redis | Real-time copy trading engine. Streams transactions via Jetstream, decodes swaps across Jupiter/Raydium/Pump.fun, and mirrors trades with safety checks + Jito MEV protection. |
| [orbit-grpc-indexer](./orbit-grpc-indexer) | Rust, Axum, SeaORM, Next.js | Dual-stream Solana transaction indexer. Combines Jetstream (speed) + Yellowstone (CPIs) into Postgres with REST API, WebSocket live feed, and explorer dashboard. |