# OrbitFlare gRPC Indexer

![OrbitFlare gRPC Indexer](assets/og.png)

A dual-stream Solana transaction indexer. It pulls from two sources at once -OrbitFlare's Jetstream for speed (decoded shreds, sub-second) and Yellowstone gRPC for completeness (full transactions with CPIs). Both write to the same Postgres database. When the same transaction arrives from both streams, the rows merge automatically.

The project ships as two separate binaries and an optional frontend:

- **orbit-grpc-indexer** -the indexer. Connects to both gRPC streams, filters, enriches, and writes to Postgres.
- **indexer-api-server** -the API. Reads from the same Postgres and serves REST endpoints, health checks, and a WebSocket live feed.
- **frontend/** -a Next.js dashboard for browsing indexed data in real time.

They share a database, not a process. Run them together or separately. Scale the API without touching the indexer.

## How it works

```mermaid
graph LR
    subgraph Indexer
        JS[Jetstream gRPC] --> F1[Filter]
        YS[Yellowstone gRPC] --> F2[Filter]
        F1 --> E1[Enrich]
        F2 --> E2[Enrich]
        E1 --> S1[Batch + Write]
        E2 --> S2[Batch + Write]
    end

    subgraph Database
        PG[(PostgreSQL)]
    end

    subgraph API Server
        AX[Axum]
        AX --> REST[REST API]
        AX --> WS[WebSocket]
        AX --> HP[Health]
    end

    S1 --> PG
    S2 --> PG
    PG --> AX
    PG --> NOTIFY --> WS
```

Jetstream writes first because it's faster. Yellowstone arrives later with CPI data (inner instructions, logs, fees) and enriches the existing row via `ON CONFLICT DO UPDATE`. The `source` column tracks where data came from -`jetstream`, `yellowstone`, or `both` for merged rows.

## Quickstart

You need Rust (1.85+), Docker (for Postgres), and Node.js (for the frontend).

**Install the OrbitFlare CLI and scaffold the project:**

```bash
cargo install orbitflare
orbitflare template --install orbit-grpc-indexer
cd orbit-grpc-indexer
```

**1. Start Postgres**

```bash
docker compose up -d
```

**2. Configure**

Copy `.env.example` to `.env` and fill in your endpoints:

```
JETSTREAM_URL=http://jp.jetstream.orbitflare.com
YELLOWSTONE_URL=http://fra.rpc.orbitflare.com:10000
DATABASE_URL=postgres://indexer:indexer@localhost:5432/jetstream_indexer
```

The `config.yml` file has tuning knobs -timeouts, batch sizes, reconnect settings, subscription filters.

**3. Run the indexer**

```bash
cargo run --bin orbit-grpc-indexer
```

It connects to both streams, runs database migrations, and starts indexing. You'll see metrics logged every 30 seconds.

**4. Run the API server**

```bash
cargo run --bin indexer-api-server
```

Starts on port 3000 by default. Override with `PORT=8080`.

**5. Run the frontend (optional)**

```bash
cd frontend
npm install
npm run dev
```

Opens on port 3001. Proxies API requests to port 3000.

## API

**Health**
```
GET /health
```

**List transactions** (cursor or offset pagination)
```
GET /api/v1/transactions?limit=25
GET /api/v1/transactions?limit=25&cursor=408331289_abc...
GET /api/v1/transactions?pagination=offset&offset=0&limit=25
GET /api/v1/transactions?source=jetstream&success=true&slot_min=408000000
```

**Single transaction** (includes inner instructions / CPIs)
```
GET /api/v1/transactions/:signature
```

**Transactions by account**
```
GET /api/v1/accounts/:address/transactions?limit=25
```

**WebSocket live feed**
```
ws://localhost:3000/ws/transactions
```

Receives JSON messages as transactions are indexed:
```json
{"signature":"abc...","slot":408331289,"source":"jetstream","success":true,"has_cpi_data":false}
```

The WebSocket is powered by Postgres LISTEN/NOTIFY -the indexer sends a notification after each write, the API server picks it up and broadcasts to connected clients.

## Project structure

```
orbit-grpc-indexer/
├── .env.example              env vars (URLs, database)
├── config.yml                tuning knobs (timeouts, batching, filters)
├── docker-compose.yml        local Postgres
├── Cargo.toml                workspace root
│
├── migrations/
│   ├── 0001_create_transactions.sql
│   ├── 0002_create_inner_instructions.sql
│   └── 0003_create_accounts_touched.sql
│
├── protos/
│   ├── jetstream.proto
│   ├── geyser.proto
│   └── solana-storage.proto
│
├── crates/
│   ├── proto/                proto compilation (tonic-prost-build)
│   ├── core/                 traits and shared types (no implementations)
│   ├── config/               YAML loader with dotenvy
│   ├── ingest/               Jetstream + Yellowstone stream clients
│   ├── filter/               program, account, composite filters
│   ├── db/                   SeaORM entities, Postgres sink, queries
│   └── indexer/              indexer binary
│
├── server/                   API server binary (Axum)
│   └── src/
│       ├── main.rs
│       ├── router.rs
│       ├── state.rs
│       ├── health.rs
│       ├── error.rs
│       ├── filter.rs
│       ├── response.rs
│       ├── handler/
│       └── pagination/
│
└── frontend/                 Next.js dashboard (optional)
    └── src/
        ├── app/
        ├── components/
        └── lib/
```

## Database

Three tables, each serving a different query pattern.

```mermaid
erDiagram
    transactions {
        text signature PK
        bigint slot
        timestamptz block_time
        bigint fee
        boolean success
        jsonb err
        integer num_instructions
        text[] accounts
        text[] log_messages
        boolean has_cpi_data
        text source
        jsonb raw
        timestamptz indexed_at
        timestamptz enriched_at
    }

    inner_instructions {
        bigserial id PK
        text signature FK
        integer instruction_idx
        integer depth
        text program_id
        text[] accounts
        text data
        timestamptz indexed_at
    }

    accounts_touched {
        bigserial id PK
        text account
        text signature FK
        bigint slot
        boolean is_signer
        boolean is_writable
        timestamptz indexed_at
    }

    transactions ||--o{ inner_instructions : "has"
    transactions ||--o{ accounts_touched : "has"
```

- **transactions** -one row per transaction. Jetstream inserts first, Yellowstone enriches via upsert. GIN index on the accounts array for containment queries.
- **inner_instructions** -CPI data from Yellowstone only. One row per inner instruction call.
- **accounts_touched** -denormalized lookup for "give me all transactions involving this account." Unique on (account, signature).

## Dual-stream merge

The upsert logic handles the merge automatically:

- `fee`, `err`, `raw` use COALESCE -Yellowstone fills what Jetstream left null
- `log_messages` only overwrites if the new array is non-empty
- `has_cpi_data` uses OR -once true, stays true
- `source` flips to `both` when a second stream writes the same signature
- `enriched_at` gets set when Yellowstone first enriches a Jetstream-only row

## Slot resume

On startup, the indexer queries `MAX(slot)` from the database and passes it as `from_slot` in the Yellowstone subscription. This means if you stop the indexer and restart it later, Yellowstone picks up from where it left off (as far as the gRPC server supports it). Jetstream doesn't support `from_slot` -it only delivers live data.

During a session, the last seen slot is tracked in memory. If the stream drops and reconnects, it resumes from the last slot it saw, not from the beginning.

## Trait-based architecture

Every boundary in the pipeline is a trait. Swap any part without touching the rest.

```mermaid
graph TD
    TS[TransactionStream] -->|"next()"| TF[TransactionFilter]
    TF -->|"filter()"| TE[TransactionEnricher]
    TE -->|"enrich()"| TSK[TransactionSink]
    TSK -->|"write_batch()"| DB[(Storage)]
```

| Want to change | Implement | Plug in |
|---|---|---|
| New data source | `TransactionStream` | `Pipeline::spawn_stream()` |
| Custom filtering | `TransactionFilter` | `CompositeFilter::add()` |
| Instruction decoding | `TransactionEnricher` | Replace `DefaultEnricher` |
| Different database | `TransactionSink` | Replace `PostgresSink` |
| Health checks | `HealthReporter` | Replace `SystemHealth` |

## Configuration

**Environment variables** (in `.env`):

| Variable | Required | Description |
|---|:---:|---|
| `JETSTREAM_URL` | Yes | Jetstream gRPC endpoint |
| `YELLOWSTONE_URL` | Yes | Yellowstone gRPC endpoint |
| `DATABASE_URL` | Yes | Postgres connection string |
| `PORT` | No | API server port (default 3000) |

**Config file** (`config.yml`): timeouts, reconnect backoff, subscription filters, batch sizes, pagination limits, log level. See the included `config.yml` for all options with their defaults.

## Subscription filters

Jetstream and Yellowstone have separate subscription filters in `config.yml`. These are server-side filters -they control what the gRPC server sends you.

```yaml
jetstream:
  transactions:
    account_required:
      - "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"

yellowstone:
  transactions:
    account_required:
      - "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"
```

There's also a local filter layer (`filters` section) that runs after receiving data -useful for things the server-side filters can't express.
