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
- **Confirmations lag** trades latency for reorg safety (D009).

## Observability (S07-T01)

Every loop iteration emits **one structured `tracing` summary line**
(`"follow cycle summary"`, INFO) with parseable fields — no log scraping
needed, no new dependency (reuses `tracing` + `chrono`):

| Field | Meaning |
|-------|---------|
| `cycle` | Loop iteration count (process-local, from 1). |
| `head` | Chain head from `get_block_number`. |
| `checkpoint` | Last processed block (`-1` if none yet). |
| `lag` | Indexing lag = `head − checkpoint` (0 if no checkpoint). |
| `indexed_this_cycle` | Blocks indexed in this iteration. |
| `blocks_total` | Cumulative blocks indexed since process start. |
| `reorgs_total` | Cumulative reorg count. |
| `last_reorg_depth` | Blocks rolled back in the most recent reorg (0 if none). |
| `last_poll` | UTC timestamp of this cycle. |

A reorg cycle instead emits two WARN lines (`"reorg detected — rolling
back"` then `"rolled back — re-indexing next cycle"`) carrying
`cycle`, `fork`, `depth` (rolled-back block count), `reorgs_total`.

These counters live in process memory only (not persisted) and are
**observation-only**: branch flow, timing, RPC/DB calls are unchanged
(no behavior/perf regression).

## Reorg detection & correction (S06)

Every poll, *before* indexing, the loop compares the most recent local block
hashes against the chain (`detect_fork` → pure `find_fork_point`). On a
mismatch it calls `rollback_from_block(fork)` — deletes `>= fork` across
block/tx/events/trace/failed in one transaction and rewinds the checkpoint —
then re-indexes on the next iteration.

- **Safety**: if any chain hash is unavailable (RPC error / block absent) the
  check yields *no fork* — never a destructive rollback on uncertain data.
- **Scan window** = `max(--confirmations, 64)` blocks. A reorg **deeper than
  the window is _not_ fully corrected**: the whole window mismatches,
  `find_fork_point` returns the window floor, and `rollback_from_block` only
  deletes `>= floor` — older blocks below the window that belong to the
  abandoned chain are **kept** (under-deletion → potential silent
  inconsistency). This is *not* a conservative over-delete. Correctness relies
  on the unstated assumption *reorg depth ≤ scan window*; on mainnet that holds
  in practice (PoS finality ≈ 64 blocks ≤ window, plus the confirmations lag),
  so practical risk is low — but it is **not unconditionally "safe"**. Dynamic
  window widening to the true common ancestor is S07-T03.

## Limits / scope (see `.gsd/DECISIONS.md` D009, D010)

- Polling, not `eth_subscribe` (→ S07).
- Reorgs **within** the scan window are detected & corrected exactly (S06).
  Reorgs **deeper than** the window are under-corrected (stale pre-window
  blocks retained → potential silent inconsistency) — low practical risk on
  mainnet given PoS finality, full fix is S07-T03. An earlier draft of this
  doc/D010/S06-SUMMARY mislabeled this as a safe "conservative over-delete";
  corrected 2026-05-20 (review R1).

## Verification

The follow loop needs a live `RPC_URL`, which CI may not have. So the
**decision logic is verified without RPC**:

```bash
cargo test -p indexer        # next_target + find_fork_point unit tests (no RPC/DB)
cargo test -p db -- --ignored # rollback_from_block idempotency (needs Postgres)
```

Live smoke (manual, requires RPC + Postgres):

```bash
export DATABASE_URL=postgres://defi:defi@localhost:5432/defi_analytics
export RPC_URL=<an ethereum rpc endpoint>
cargo run -p indexer -- --follow --poll-interval-secs 6
# expect: "follow mode started", periodic "indexing new range" logs,
#         checkpoint advancing; Ctrl-C → "stopping follow loop"
```
