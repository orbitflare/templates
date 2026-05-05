# Solana Copy Trader

Real-time Solana copy trading engine. Monitors target wallets, detects swap transactions across DEXs, and mirrors trades via Jupiter Swap API with configurable position sizing, safety checks, and Jito MEV protection.

Runs a **dual-stream** ingest: Jetstream for raw latency on direct swaps, Yellowstone gRPC for completeness (the inner-instruction CPI chain that Jetstream's shred-decoded view cannot see). Both feed a single channel with Redis dedup, so you get the speed of one and the coverage of the other.

---

## Architecture

```mermaid
flowchart TD
    J[Jetstream gRPC<br/>fast, top-level only] --> M[Unified channel<br/>Redis dedup]
    Y[Yellowstone gRPC<br/>complete, +CPI inner ix] --> M
    M --> C[Decoder pipeline]

    C --> C1[Jupiter v6]
    C --> C2[Raydium AMM/CPMM]
    C --> C3[Pump.fun]

    C1 & C2 & C3 --> D[TradeIntent]

    D --> E[Safety Filter + Rate Limiter]
    E <--> F[(Redis)]

    E --> G[Execution Engine]
    G --> G1[Jupiter Quote]
    G1 --> G2[Position Sizing]
    G2 --> G3[Simulate tx]
    G3 --> G4[Send tx / Jito bundle]
    G4 --> G5[Confirm]

    G5 --> H[Output Layer]
    H --> H1[JSON logs]
    H --> H2[Prometheus :9090]
    H --> H3[PostgreSQL journal]
    H --> H4[Telegram]
```

The decoder tries top-level instructions first (the path Jetstream provides). If nothing matches and the transaction came from Yellowstone with inner instructions, it walks the CPI chain - this is what catches aggregator/router swaps (Axiom, etc.) that Jetstream alone would miss.

---

## Prerequisites

| Dependency               | Version  | Purpose                                              |
|--------------------------|----------|------------------------------------------------------|
| Rust                     | ≥ 1.83   | Build toolchain                                      |
| Docker & Compose         | ≥ 24.0   | Container orchestration                              |
| OrbitFlare account       | —        | RPC API key (URL `?api_key=`); gRPC URLs for Jetstream and Yellowstone |
| Solana keypair           | —        | Trader wallet (signs transactions)                   |

Grab Jetstream and Yellowstone gRPC URLs from [OrbitFlare](https://orbitflare.com/login).

Yellowstone is required for full coverage. Set `yellowstone.enabled: false` in config only if you knowingly accept missing CPI-routed trades.

---

## Quick Start

### 1. Install and scaffold

```bash
cargo install orbitflare
orbitflare template --install solana-copy-trader
cd solana-copy-trader
cp .env.example .env
cp config.example.yml config.yml
```

Edit `.env`:

```env
JETSTREAM_GRPC_ENDPOINT=http://fra.jetstream.orbitflare.com
YELLOWSTONE_GRPC_ENDPOINT=http://fra.rpc.orbitflare.com:10000
ORBITFLARE_RPC_URL=http://fra.rpc.orbitflare.com?api_key=YOUR_API_KEY
TRADER_KEYPAIR_PATH=/keys/trader.json
REDIS_URL=redis://redis:6379
DATABASE_URL=postgres://copytrader:password@postgres:5432/copytrader
```

Telegram push notifications are optional. Enable them by setting `notifications.telegram.enabled: true` in `config.yml` and adding `TELEGRAM_BOT_TOKEN` / `TELEGRAM_CHAT_ID` to `.env`.

### 2. Run with Docker Compose

```bash
# Dry run mode (default) — watches and logs, never sends transactions
docker compose up -d

# Watch logs
docker compose logs -f copy-trader

# Production mode — executes real trades
DRY_RUN=false docker compose up -d
```

### 3. Run locally (development)

```bash
# Start dependencies
docker compose up -d redis postgres

# Build and run
cargo build --release
RUST_LOG=info ./target/release/copy-trader --config config.yml --dry-run
```

---

## CLI

```
copy-trader [OPTIONS]

OPTIONS:
    -c, --config <PATH>       Config file path [default: config.yml]
    -d, --dry-run              Force dry run mode (overrides config)
    -v, --verbose              Increase log verbosity (-v = debug, -vv = trace)
        --wallet <ADDRESS>     Track a single wallet (overrides config targets)
        --validate             Validate config and exit
        --migrate              Run database migrations and exit
```

### Common usage

```bash
# Validate configuration without starting
copy-trader --validate

# Quick test: track one wallet in dry-run
copy-trader --wallet "WhaleAddress123" --dry-run -v

# Production
copy-trader --config /app/config.yml
```

---

## Configuration Reference

See [`config.example.yml`](config.example.yml) for the full annotated configuration.

| Field | Valid values |
|---|---|
| `sizing.mode` | `fixed`, `proportional`, `max_cap` |
| `fees.strategy` | `dynamic`, `fixed`, `aggressive` |
| `confirm.method` | `poll`, `websocket` |
| `log.format` | `json`, `pretty` |

---

## Prometheus Metrics

Exposed on `:9090/metrics` when `metrics.enabled = true`.

| Metric                                     | Type      | Labels                     | Description                                |
|--------------------------------------------|-----------|----------------------------|--------------------------------------------|
| `copytrader_trades_total`                  | Counter   | `target`, `status`, `dex`  | Total trades by outcome                    |
| `copytrader_trade_latency_ms`              | Histogram | `target`, `dex`            | End-to-end latency (detect → confirm)      |
| `copytrader_simulation_latency_ms`         | Histogram | —                          | `simulateTransaction` round-trip time      |
| `copytrader_slippage_bps`                  | Histogram | `dex`                      | Actual slippage observed on filled trades  |
| `copytrader_open_positions`                | Gauge     | —                          | Current open position count                |
| `copytrader_portfolio_exposure_sol`        | Gauge     | —                          | Total SOL value in open positions          |
| `copytrader_stream_reconnects_total`       | Counter   | —                          | Jetstream reconnection count               |
| `copytrader_yellowstone_reconnects_total`  | Counter   | —                          | Yellowstone reconnection count             |
| `copytrader_stream_lag_slots`              | Gauge     | —                          | Slots behind tip (Jetstream)               |
| `copytrader_cpi_swaps_detected_total`      | Counter   | —                          | Swaps decoded from inner instructions only (Yellowstone wins these) |
| `copytrader_jupiter_quote_cache_hits`      | Counter   | —                          | Redis price cache hit count                |

### Grafana dashboard

Import `grafana/copy-trader.json` for a pre-built dashboard covering trade activity, latency percentiles, stream health, and portfolio exposure.

---

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `stream disconnected, reconnecting...` every few seconds | Jetstream endpoint overloaded or network issue | Switch region, check OrbitFlare status page, verify firewall allows gRPC |
| Trades from aggregators (Axiom, Jupiter routes) never appear | Yellowstone disabled, only Jetstream running | Set `yellowstone.enabled: true` and configure `YELLOWSTONE_GRPC_ENDPOINT` |
| `simulation failed: InsufficientFunds` | Trader wallet is out of SOL | Top up wallet, reduce `max_trade_sol` |
| `simulation failed: SlippageExceeded` | Price moved between quote and simulation | Increase `slippage.default_bps`, enable Jito bundles |
| `trade_filtered: min_liquidity` on every token | Liquidity threshold too high | Lower `min_liquidity_sol` |
| `trade_filtered: cooldown` | Same token detected multiple times | Expected behavior; reduce `cooldown_per_token_secs` if too aggressive |
| High `stream_lag_slots` (>10) | Processing can't keep up | Increase `channel_buffer_size`, check CPU, reduce targets |
| Duplicate trades in journal | Redis was unavailable | Check Redis connectivity; dedup is best-effort if Redis is down |

---
