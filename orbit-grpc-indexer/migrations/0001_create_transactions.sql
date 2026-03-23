CREATE TABLE IF NOT EXISTS transactions (
    signature        TEXT PRIMARY KEY,
    slot             BIGINT NOT NULL,
    block_time       TIMESTAMPTZ,
    fee              BIGINT,
    success          BOOLEAN NOT NULL,
    err              JSONB,
    num_instructions INTEGER,
    accounts         TEXT[] NOT NULL DEFAULT '{}',
    log_messages     TEXT[] NOT NULL DEFAULT '{}',
    has_cpi_data     BOOLEAN NOT NULL DEFAULT FALSE,
    source           TEXT NOT NULL,
    raw              JSONB,
    indexed_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    enriched_at      TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_tx_slot ON transactions(slot);
CREATE INDEX IF NOT EXISTS idx_tx_block_time ON transactions(block_time);
CREATE INDEX IF NOT EXISTS idx_tx_success ON transactions(success);
CREATE INDEX IF NOT EXISTS idx_tx_source ON transactions(source);
CREATE INDEX IF NOT EXISTS idx_tx_accounts ON transactions USING GIN(accounts);
