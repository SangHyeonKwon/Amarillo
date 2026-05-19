# API — Failure Intelligence (M001)

> The edge Dune does not provide out of the box (see `.gsd/PROJECT.md`):
> per-transaction failure **diagnosis**, a filtered **list** with accurate
> totals, and a category **trend** feed. Three endpoints, consistent envelope,
> embeddable. Runnable example & smoke: `./scripts/verify-failed-tx.sh`.

## `GET /v1/failed-tx/{tx_hash}`

Returns the decoded/classified failure record for a single transaction plus its
flattened call trace.

### Path parameters

| Param | Type | Description |
|-------|------|-------------|
| `tx_hash` | string | Transaction hash (`0x…`, 66 chars). |

### Response `200`

`ApiResponse<FailedTxDetail>` — same envelope as every other endpoint:

```jsonc
{
  "data": {
    "failed": {
      "tx_hash": "0xdead…0001",
      "error_category": "Unknown",        // ErrorCategory enum
      "revert_reason": null,              // decoded Error(string)/Panic, if any
      "failing_function": null,           // 4-byte selector of failing call
      "gas_used": 45000,
      "timestamp": "2023-09-01T12:00:00Z"
    },
    "call_tree": [                         // trace_log frames, pre-order (trace_id asc)
      {
        "tx_hash": "0xdead…0001",
        "call_depth": 0,
        "call_type": "CALL",
        "from_addr": "0x5555…5555",
        "to_addr": "0xE592…1564",         // Uniswap V3 Router
        "value": "0",
        "gas_used": 35000,
        "input": "0x414bf389",            // exactInputSingle selector
        "output": null,
        "error": "Too little received",   // ← revert text at the call site
        "trace_id": 4
      }
      // … deeper frames
    ],
    "call_tree_truncated": false           // true if capped at MAX 2000 frames
  }
}
```

`call_tree` is ordered by `trace_id ASC` — i.e. **pre-order DFS**, the exact
order the indexer flattened the call tree when inserting (`trace_id` is a
BIGSERIAL that preserves that order). This is the correct linear tree order;
sorting by `call_depth` first would interleave sibling subtrees and make the
tree unreconstructable. Frames are flattened, not nested — nested
reconstruction is a future slice (see `.gsd/DECISIONS.md` D004); use
`call_depth` to indent/rebuild.

`call_tree` is capped at **2000** frames; `call_tree_truncated: true` signals a
partial response (defense against pathologically large traces).

### Response `400` / `404`

A syntactically invalid `tx_hash` (not `0x` + 64 hex) is a **client error →
400**. A well-formed hash with no failure record is **404**:

```json
{ "error": "invalid tx_hash (expected 0x + 64 hex): 0xnothex" }   // 400
{ "error": "failed transaction 0x0000…0000" }                     // 404
```

### Why this beats the aggregate view

`/v1/analytics/failed-tx` (the existing `vw_failed_tx_analysis`) only reports
counts per `error_category`. In the example above the aggregate would bucket
this tx as **Unknown**, yet the per-tx call tree exposes the real cause —
`"Too little received"` (a slippage failure) at the router call. The
diagnosis endpoint is strictly more informative than the rollup.

## `GET /v1/failed-tx`

Filtered, paginated list of failed transactions with an accurate `total`
(the drill-down's list side; S02 of M001).

### Query parameters (all optional)

| Param | Type | Description |
|-------|------|-------------|
| `category` | string | `ErrorCategory` filter, SCREAMING_SNAKE_CASE (e.g. `SLIPPAGE_EXCEEDED`). |
| `from` | string | Lower bound on `timestamp`, RFC 3339. |
| `to` | string | Upper bound on `timestamp`, RFC 3339. |
| `limit` | int | Page size, default 20, clamped 1–100. |
| `offset` | int | Skip count, default 0. |

### Response `200`

`TotalPaginatedResponse<FailedTransaction>` — note `pagination.total` is the
full filtered count, independent of `limit`/`offset` (this is the bit Dune
dashboards don't expose as an embeddable contract):

```jsonc
{
  "data": [
    { "tx_hash": "0xdead…0001", "error_category": "Unknown",
      "revert_reason": null, "failing_function": null,
      "gas_used": 45000, "timestamp": "2023-09-01T12:00:00Z" }
  ],
  "pagination": { "limit": 20, "offset": 0, "count": 1, "total": 3 }
}
```

The existing `PaginatedResponse` (no `total`) is left unchanged for contract
compatibility; this endpoint uses the additive `TotalPaginatedResponse`
(see `.gsd/DECISIONS.md` D005).

### Response `400`

Client input errors are `400`, **not** `404` (the request is well-formed-but-invalid,
no resource lookup happened):

```json
{ "error": "invalid `category`: BOGUS" }
```

— e.g. an unknown `category` value or a non-RFC3339 `from`/`to`.

## `GET /v1/analytics/failed-tx/timeseries`

Failure counts bucketed by time × error category — the trend feed for charts
(S03 of M001).

### Query parameters

| Param | Type | Description |
|-------|------|-------------|
| `interval` | string | Bucket unit: `hour` \| `day` \| `week`. Default `day`. **Whitelist only.** |
| `from` | string | Lower bound on `timestamp`, RFC 3339 (optional). |
| `to` | string | Upper bound on `timestamp`, RFC 3339 (optional). |

`interval` is a closed enum mapped to a fixed `date_trunc` literal **and** passed
as a bound parameter — never string-interpolated (SQL-injection safe; see
`.gsd/S03-PLAN.md` and KNOWLEDGE).

### Response `200`

`ApiResponse<FailedTxTrendPoint[]>`, ordered by `bucket ASC, error_category ASC`:

```jsonc
{
  "data": [
    { "bucket": "2023-09-01T00:00:00Z", "error_category": "Unknown", "failure_count": 3 }
  ]
}
```

The per-category counts reconcile with `/v1/failed-tx`'s `total` for the same
filter window (asserted by the db integration test).

### Response `400`

Unknown `interval`, or non-RFC3339 `from`/`to`:

```json
{ "error": "invalid `interval` (hour|day|week): bogus" }
```

## Verify

```bash
docker compose up -d
docker compose run --rm seed

# HTTP layer: end-to-end via the running api (200 seeded / order / 404)
./scripts/verify-failed-tx.sh

# DB layer: query + invariants (incl. trace pre-order regression guard)
cargo test -p db -- --ignored
```

Two complementary layers: the script exercises the live HTTP endpoint; the
`cargo test -p db -- --ignored` integration tests exercise the db queries
directly (and assert the call-tree pre-order invariant). Override the script
fixture with `GOOD_HASH=… DATABASE_URL=… API_PORT=… ./scripts/verify-failed-tx.sh`.

> **Note on `:3000`** — `verify-failed-tx.sh` builds and runs the API **locally
> on port 3001** against the compose Postgres, so it does not depend on the
> compose `api` service. The `docker compose` `api` (port **3000**) serves the
> image *as previously built*; to expose the M001 endpoints there too, rebuild:
> `docker compose up -d --build api`.
