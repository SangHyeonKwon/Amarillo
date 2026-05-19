# Real-time Follow Mode (S05)

The indexer can **follow the chain head** instead of processing a fixed block
range — M002's first slice.

## Usage

```bash
# fixed range (M001, unchanged)
cargo run -p indexer -- --from-block 18000000 --to-block 18001000

# follow the chain head continuously (Ctrl-C to stop)
cargo run -p indexer -- --follow
cargo run -p indexer -- --follow --poll-interval-secs 6 --confirmations 20
```

| Flag | Default | Meaning |
|------|---------|---------|
| `--follow` | off | Continuous head-follow instead of a fixed range. |
| `--poll-interval-secs` | 12 | Sleep between head polls. |
| `--confirmations` | 12 | Index only up to `head - N` (shallow-reorg cushion). |

`--from-block` is **optional** with `--follow` (resume point comes from the
`indexer_checkpoint`, not the CLI). `--to-block` is ignored with `--follow`.

## How it works

Loop: fetch chain head (`get_block_number`, retried with backoff) → compute the
next range via the pure `next_target(head, confirmations, checkpoint)` →
`index_range(..)` (reuses the M001 pipeline + per-chunk checkpointing) → sleep →
repeat. `Ctrl-C` finishes the in-flight chunk and stops cleanly.

- **No backfill in follow mode**: with no checkpoint, follow starts at the
  *safe tip* (`head - confirmations`), not genesis. Backfill is the fixed-range
  mode's job; run that first if you need history.
- **Confirmations lag** trades latency for reorg safety (D009). Deep-reorg
  detection/correction is **S06**, not S05.

## Limits / scope (see `.gsd/DECISIONS.md` D009)

- Polling, not `eth_subscribe` (→ S07).
- Only shallow reorgs are mitigated (via the confirmations lag). A reorg deeper
  than `--confirmations` is **not** corrected until S06.

## Verification

The follow loop needs a live `RPC_URL`, which CI may not have. So the
**decision logic is verified without RPC**:

```bash
cargo test -p indexer        # next_target unit tests (no RPC/DB)
```

Live smoke (manual, requires RPC + Postgres):

```bash
export DATABASE_URL=postgres://defi:defi@localhost:5432/defi_analytics
export RPC_URL=<an ethereum rpc endpoint>
cargo run -p indexer -- --follow --poll-interval-secs 6
# expect: "follow mode started", periodic "indexing new range" logs,
#         checkpoint advancing; Ctrl-C → "stopping follow loop"
```
