CREATE TABLE IF NOT EXISTS accounts_touched (
    id               BIGSERIAL PRIMARY KEY,
    account          TEXT NOT NULL,
    signature        TEXT NOT NULL REFERENCES transactions(signature),
    slot             BIGINT NOT NULL,
    is_signer        BOOLEAN NOT NULL DEFAULT FALSE,
    is_writable      BOOLEAN NOT NULL DEFAULT FALSE,
    indexed_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(account, signature)
);

CREATE INDEX IF NOT EXISTS idx_at_account ON accounts_touched(account);
CREATE INDEX IF NOT EXISTS idx_at_slot ON accounts_touched(slot);
CREATE INDEX IF NOT EXISTS idx_at_signer ON accounts_touched(is_signer) WHERE is_signer = TRUE;
