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
    "call_tree_truncated": false,          // true if capped at MAX 2000 frames
    "root_cause": {                        // first call_tree frame with error ≠ null, or null (S10 / M004)
      "tx_hash": "0xdead…0001",
      "call_depth": 0,
      "call_type": "CALL",
      "from_addr": "0x5555…5555",
      "to_addr": "0xE592…1564",
      "value": "0",
      "gas_used": 35000,
      "input": "0x414bf389",
      "output": null,
      "error": "Too little received",
      "trace_id": 4
    }
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

`root_cause` is a *single* `trace_log` frame — the earliest entry in `call_tree`
whose `error` is non-null, i.e. **where the revert actually originated**
(pre-order DFS order). It is a redundant projection of `call_tree` provided
for the common "where did the failure come from?" question, so clients don't
have to scan the array. **`null` is explicit, not absence** — it signals
that the indexer recorded no per-frame error for this transaction (silent
default is intentionally not allowed; see `.gsd/DECISIONS.md` D014).

Per-frame trace analysis is what Dune can't expose out of the box — Dune's
query model has no notion of `trace_log.error` because traces aren't part
of its public dataset. `root_cause` makes that asymmetry directly visible
to embedding products (S10 of M004).

`failing_function_decoded` resolves the 4-byte `failing_function` selector
(e.g. `0xa9059cbb`) against our self-owned `function_signature` ABI seed
table into a human-readable shape:

```jsonc
"failing_function_decoded": {              // S11 / M004 — null if no match
  "selector":  "0xa9059cbb",               // always lowercased
  "name":      "transfer",
  "signature": "transfer(address,uint256)",
  "source":    "erc20"                     // 'erc20' | 'uniswap-v3-router' | ...
}
```

`null` is explicit (D014) — it means either `failing_function` itself was
`null` *or* the selector isn't in our seed. The contract guarantees:

- `selector === data.failed.failing_function.toLowerCase()` (self-consistency).
- `name` and `signature` are non-empty strings.

Why self-owned ABI seed and not 4byte.directory? Two reasons (D015, D008
spirit):

1. **No runtime third-party dependency.** Production calls don't hit an
   external endpoint mid-request; the seed lives entirely in our migrations.
2. **Curated quality.** Public selector databases are full of garbage from
   typos and collisions. We seed only what we've verified against EIP-20
   and the Uniswap V3 ABI; an operator extending the seed is a deliberate
   `INSERT INTO function_signature ... ON CONFLICT DO NOTHING`.

ABI **args** decoding (turning the input bytes into typed values) is
deliberately *out of scope* for this slice — that's a separate slice (S11.1
sketch) because it needs the full ABI type system (address / uint / dynamic
bytes / nested tuples). Name + signature alone is already the high-value
gain for the dApp developer persona.

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

## `GET /v1/analytics/failed-tx/by-label` — Failures by labeled contract (S09 / M003)

Joins on-chain failure data (`failed_transaction × transaction`) with the
**off-chain** `contract_label` table (a private mapping `address → human
label` that we store ourselves) to expose **failure distribution per labeled
contract**. This is the M003 "on-chain × private-data join" example —
exactly the kind of question Dune can't answer because Dune has no access to
your private label store.

Query parameters (all optional):

| Param   | Type        | Default | Meaning |
|---------|-------------|---------|---------|
| `from`  | RFC3339     | none    | Inclusive lower time bound (else: any) |
| `to`    | RFC3339     | none    | Inclusive upper time bound (else: any) |
| `owner` | text        | none    | Tenancy filter — empty/absent = all labels; otherwise must equal `contract_label.owner_id` |
| `limit` | integer     | 50      | Clamped 1..=200; rows are `total_failures` DESC |

Response: `ApiResponse<FailedTxByLabelPoint[]>` where each row is

```json
{
  "label": "Uniswap V3 SwapRouter",
  "address": "0xe592427a0aece92de3edee1f18e0157c05861564",
  "total_failures": 47,
  "by_category": { "SLIPPAGE_EXCEEDED": 31, "UNKNOWN": 16 }
}
```

Pivot invariant: `sum(by_category) === total_failures` (verified by
`scripts/verify-failed-tx-by-label.sh`). `address` is always lowercased
0x + 40 hex (the `contract_label` PK convention).

Errors:

- non-RFC3339 `from`/`to` → **400** `{ "error": "invalid `from` …" }`
- unknown `owner` (no matching tenant) → **200** with `{ "data": [] }`
- empty result (no labels or no matching failures) → **200** with `{ "data": [] }`

### Where labels come from

The migration `migrations/20240105000001_add_contract_label.sql` seeds:

- The Uniswap V3 SwapRouter + Factory addresses (global, `owner_id IS NULL`).
- One label per existing `pool` row (`"<pair_name> (pool)"`).

Operators add their own labels via the (admin-only) `insert_contract_label`
DB function; an authenticated HTTP surface is intentionally **out of scope**
for the demo (D013, D008 spirit).

### Why this is the moat

Dune queries every dataset that's *publicly indexed*. Your label set is
*consumer-specific*: which contracts you deployed, which counterparties you
care about, which user IDs your KYC system already knows. The endpoint above
shows a single concrete instance (contract labels); the same pattern fits
bot-operator self-labels or exchange KYC mapping by swapping the off-chain
table — same join, different private side.

## Verify

```bash
docker compose up -d
docker compose run --rm seed

# HTTP layer: end-to-end via the running api (200 seeded / order / 404)
./scripts/verify-failed-tx.sh

# By-label endpoint: shape + invariants + 400 + empty tenant (S09 / M003)
./scripts/verify-failed-tx-by-label.sh    # set API_PORT=… if 3001 is taken

# DB layer: query + invariants (incl. trace pre-order regression guard
# + label aggregate / owner filter / future-window emptiness)
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
