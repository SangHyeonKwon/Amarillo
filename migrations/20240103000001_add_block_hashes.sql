-- Add block/parent hashes to enable reorg detection (M002/S06).
-- Existing rows predate this and stay NULL (no backfill); follow-mode and
-- future indexing populate them. Detection only needs hashes going forward.

BEGIN;

ALTER TABLE block ADD COLUMN IF NOT EXISTS block_hash  TEXT;
ALTER TABLE block ADD COLUMN IF NOT EXISTS parent_hash TEXT;

COMMENT ON COLUMN block.block_hash  IS 'Block hash — reorg detection (NULL for pre-S06 rows)';
COMMENT ON COLUMN block.parent_hash IS 'Parent block hash — fork-point detection (NULL for pre-S06 rows)';

COMMIT;
