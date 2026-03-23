CREATE TABLE IF NOT EXISTS inner_instructions (
    id               BIGSERIAL PRIMARY KEY,
    signature        TEXT NOT NULL REFERENCES transactions(signature),
    instruction_idx  INTEGER NOT NULL,
    depth            INTEGER NOT NULL,
    program_id       TEXT NOT NULL,
    accounts         TEXT[] NOT NULL DEFAULT '{}',
    data             TEXT,
    indexed_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ii_signature ON inner_instructions(signature);
CREATE INDEX IF NOT EXISTS idx_ii_program_id ON inner_instructions(program_id);
CREATE INDEX IF NOT EXISTS idx_ii_depth ON inner_instructions(depth);
