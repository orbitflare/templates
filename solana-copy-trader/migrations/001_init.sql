CREATE TABLE IF NOT EXISTS trades (
    id              BIGSERIAL PRIMARY KEY,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    target_wallet   TEXT NOT NULL,
    target_label    TEXT,
    target_tx_sig   TEXT NOT NULL,
    direction       TEXT NOT NULL CHECK (direction IN ('buy', 'sell')),
    dex             TEXT NOT NULL,
    input_mint      TEXT NOT NULL,
    output_mint     TEXT NOT NULL,
    target_amount   NUMERIC NOT NULL,
    our_amount_sol  NUMERIC NOT NULL,
    our_tx_sig      TEXT,
    status          TEXT NOT NULL CHECK (status IN (
                        'detected', 'filtered', 'simulated',
                        'submitted', 'confirmed', 'failed'
                    )),
    failure_reason  TEXT,
    slippage_bps    INTEGER,
    priority_fee    BIGINT,
    latency_ms      INTEGER,
    dry_run         BOOLEAN NOT NULL DEFAULT TRUE,

    UNIQUE(target_tx_sig)
);

CREATE INDEX IF NOT EXISTS idx_trades_target ON trades(target_wallet, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_trades_status ON trades(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_trades_mint   ON trades(output_mint, created_at DESC);
