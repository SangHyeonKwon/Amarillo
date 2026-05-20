-- Contract labels — off-chain private mapping `address` → human-readable name
-- (S09 / M003 "on-chain × off-chain join example"). Each consumer can have
-- their own labels (owner_id != NULL); the seed below is the public/demo set.
-- Idempotent migration; re-running it does not duplicate seed rows.

BEGIN;

CREATE TABLE IF NOT EXISTS contract_label (
    address    TEXT        PRIMARY KEY,
    label      TEXT        NOT NULL,
    owner_id   TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE  contract_label IS 'Off-chain address → human label (S09 / M003)';
COMMENT ON COLUMN contract_label.address  IS 'Lowercased 0x + 40 hex (matches `transaction.to_addr`)';
COMMENT ON COLUMN contract_label.label    IS 'Human-readable label, e.g. "Uniswap V3 SwapRouter"';
COMMENT ON COLUMN contract_label.owner_id IS 'Tenancy hint — NULL = global/public label';

CREATE INDEX IF NOT EXISTS idx_contract_label_owner
    ON contract_label (owner_id)
    WHERE owner_id IS NOT NULL;

-- Seed: well-known Uniswap V3 contracts (lowercased).
INSERT INTO contract_label (address, label, owner_id)
VALUES
    ('0xe592427a0aece92de3edee1f18e0157c05861564', 'Uniswap V3 SwapRouter', NULL),
    ('0x1f98431c8ad98523631ae4a59f267346ea31f984', 'Uniswap V3 Factory', NULL)
ON CONFLICT (address) DO NOTHING;

-- Auto-seed labels from the `pool` table — each existing pool gets a
-- "<pair_name> (pool)" label. LOWER() is defensive against any mixed-case
-- pool_address; `ON CONFLICT DO NOTHING` keeps the migration idempotent.
INSERT INTO contract_label (address, label, owner_id)
SELECT LOWER(pool_address), pair_name || ' (pool)', NULL
FROM pool
ON CONFLICT (address) DO NOTHING;

COMMIT;
