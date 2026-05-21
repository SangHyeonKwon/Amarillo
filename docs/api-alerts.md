# Alert Subscriptions API (S08)

REST endpoints for **failure-pattern alert subscriptions**. The dispatcher
(`indexer --dispatch-alerts`) reads from the same tables and delivers an
HMAC-signed webhook **exactly once** per matching failed transaction.

> Pipeline: M002 indexer adds a failed tx → matcher (SQL) finds active
> subscriptions whose `(error_category, to_addr)` matches and not yet
> delivered → dispatcher POSTs the signed body → idempotent `alert_delivery`
> upsert.

## Endpoints

### `POST /v1/alert-subscriptions` — create a subscription

Body (all optional fields default to "any"):

```json
{
  "webhook_url":     "https://example.com/alerts",
  "error_category": "SLIPPAGE_EXCEEDED",
  "to_addr":         "0x00000000000000000000000000000000000000aa"
}
```

- `webhook_url` (required, https-only): outbound URL the dispatcher will
  POST to. **400** if it fails the SSRF guard (`db::validators::
  webhook_url_is_safe`): non-https / loopback / RFC1918 private /
  link-local (incl. `169.254.169.254` cloud metadata) / `0.0.0.0` /
  multicast / broadcast / IPv6 ULA `fc00::/7` / IPv6 link-local `fe80::/10`
  / `localhost` / `*.localhost` / `*.local`. Also **400** if length
  exceeds 2048.
- `error_category` (optional): `INSUFFICIENT_BALANCE` | `SLIPPAGE_EXCEEDED`
  | `DEADLINE_EXPIRED` | `UNAUTHORIZED` | `TRANSFER_FAILED` | `UNKNOWN`.
  Single source = `ErrorCategory::from_str` (S02 pattern). Other values
  → **400**.
- `to_addr` (optional): `0x` + 40 hex; normalized to lowercase. Other
  shapes → **400**.

Response **201**:

```json
{
  "data": {
    "subscription_id": 42,
    "error_category":  "SlippageExceeded",
    "to_addr":         "0x00000000000000000000000000000000000000aa",
    "webhook_url":     "https://example.com/alerts",
    "signing_secret":  "<64 hex chars>",
    "active":          true,
    "created_at":      "2026-05-20T11:00:00Z"
  }
}
```

**`signing_secret` is revealed *once* here.** It is **never** returned by
`GET` and never appears in logs (the model has `#[serde(skip_serializing)]`
on the field). Persist it on the receiving side immediately — the only
recovery for a lost secret is to create a new subscription.

#### `sub_type='rate_threshold'` — rate-aggregation mode (S14 / M005)

For the bot-operator persona — per-event alerts are noisy when sporadic
failures are normal operation. `rate_threshold` fires a single webhook
only when the matching failure count crosses a configured threshold inside
a sliding window, then debounces (silences the same sub) for
`debounce_secs` before the next firing is possible.

Body:

```json
{
  "webhook_url":           "https://example.com/bot-alerts",
  "error_category":        "SLIPPAGE_EXCEEDED",
  "to_addr":               "0xmybotaddress00000000000000000000000000aa",
  "sub_type":              "rate_threshold",
  "threshold_count":       10,
  "threshold_window_secs": 300,
  "debounce_secs":         600
}
```

- `sub_type` — `"per_event"` (default, S08) or `"rate_threshold"` (S14).
  Anything else → **400**.
- `threshold_count` (rate, required) — minimum match count in the window
  to fire. Must be `> 0`.
- `threshold_window_secs` (rate, required) — sliding window length in
  seconds. Must be `> 0`.
- `debounce_secs` (rate, required) — silence period after a delivery
  attempt. Must be `>= 0`.

The API rejects mismatched bodies with **400**:

- `sub_type='per_event'` carrying any rate field
- `sub_type='rate_threshold'` missing any rate field, or a non-positive
  `threshold_count` / `threshold_window_secs`, or a negative `debounce_secs`

Response **201** is the per-event envelope plus the four rate-mode fields
(`sub_type`, `threshold_count`, `threshold_window_secs`, `debounce_secs`).
When the dispatcher fires, the POST body has a different shape — keyed by
`sub_type` so a single receiver can branch on it:

```json
{
  "subscription_id":       42,
  "sub_type":              "rate_threshold",
  "match_count":           14,
  "threshold_count":       10,
  "threshold_window_secs": 300
}
```

The `X-Amarillo-Signature: sha256=<hex>` header and verification flow
(HMAC-SHA256 of the raw body, keyed by the hex-decoded 32-byte secret)
are **identical** to per-event — switch on `sub_type` *after* verifying
the signature.

**Debounce semantics (race-safe, not strictly exactly-once).** Two
workers may match the same sub in the same instant and *both* fire one
delivery before either writes its `alert_rate_dispatch` row. From that
point on the most-recent `dispatched_at` starts the debounce window, so
permanent duplication is impossible. If your receiver needs strict
idempotency, dedupe on the `subscription_id + match_count` pair (or the
delivery timestamp). Self-imposed scope (D018): rate *ratio* / trend
analytics is a separate slice (S14.1 sketch).

### Verifying the signature on the receiver

The string in the response is a **64-character lowercase hex** encoding of
a **32-byte secret**. The dispatcher uses the **32 raw bytes** (hex-
*decoded*) as the HMAC key, not the 64 ASCII hex characters. Receivers
must hex-decode before HMAC-ing or signatures will not match.

Read `X-Amarillo-Signature: sha256=<hex>`, hex-decode the stored
`signing_secret` to 32 bytes, then `HMAC-SHA256(key_bytes,
raw_request_body_bytes)`, and compare **in constant time** against the
header's hex value.

Node (≥18, no extra deps):

```js
import { createHmac, timingSafeEqual } from "node:crypto";

const key = Buffer.from(STORED_HEX_SECRET, "hex"); // 32 bytes
const got = req.header("X-Amarillo-Signature").replace(/^sha256=/, "");
const want = createHmac("sha256", key).update(rawBody).digest("hex");
if (got.length !== want.length || !timingSafeEqual(Buffer.from(got), Buffer.from(want))) {
  throw new Error("invalid signature");
}
```

Python (stdlib only):

```py
import hmac, hashlib
key = bytes.fromhex(STORED_HEX_SECRET)                 # 32 bytes
got = request.headers["X-Amarillo-Signature"].removeprefix("sha256=")
want = hmac.new(key, raw_body, hashlib.sha256).hexdigest()
if not hmac.compare_digest(got, want):
    raise ValueError("invalid signature")
```

### `GET /v1/alert-subscriptions?limit=…` — list

Returns all subscriptions (active and inactive), newest first
(`subscription_id DESC`). `limit` clamped to `[1, 500]` (default 100).
`signing_secret` is **omitted** from list responses.

```json
{ "data": [ { "subscription_id": 42, "active": true, ... }, ... ] }
```

### `DELETE /v1/alert-subscriptions/{id}` — soft-deactivate

- **204** on success (subscription now inactive — dispatcher excludes it).
- **404** if `id` doesn't exist *or* is already inactive (idempotent
  semantics: second DELETE doesn't 204, it 404s).

Hard delete is intentionally **not** exposed at the HTTP layer — the
`alert_delivery` history is preserved for auditing (`db::queries::
delete_alert_subscription` exists but is internal/admin-only).

### `POST /v1/alert-subscriptions/{id}/rotate-secret` — rotate signing secret

Issues a fresh 256-bit signing secret for an **active** subscription
(HARDEN2-T02). Same one-time-reveal contract as creation: the new
`signing_secret` is returned in the response and never again. The
subscription id, webhook url, and filter remain unchanged; only the
HMAC key changes. The next webhook the dispatcher sends will be signed
with the new key.

```bash
curl -fsS -X POST "$BASE/v1/alert-subscriptions/$ID/rotate-secret"
```

Response **200**:

```json
{
  "data": {
    "subscription_id": 42,
    "error_category":  "SlippageExceeded",
    "to_addr":         "0x00…aa",
    "webhook_url":     "https://example.com/alerts",
    "signing_secret":  "<new 64-hex chars>",
    "active":          true,
    "created_at":      "2026-05-20T11:00:00Z"
  }
}
```

- **404** if the subscription doesn't exist *or* is inactive (rotating
  an already-deactivated subscription is intentionally disallowed —
  rotating implies "keep using this subscription with a new key",
  which contradicts soft-deletion).
- The DB query is idempotent — calling rotate twice with the same
  generated secret would land the same row state — but the API layer
  generates a fresh secret per call, so each invocation actually
  invalidates the previous one. Use it once on suspected compromise,
  push the new secret to the receiver immediately.

## Dispatcher (`indexer --dispatch-alerts`)

Separate mode of the indexer binary (D012 outbox pattern: failure
isolation from the indexing/reorg loop). Reads `DATABASE_URL` only — no
RPC. One sweep per `--poll-interval-secs`:

```bash
DATABASE_URL=postgres://… cargo run -p indexer -- --dispatch-alerts \
  --poll-interval-secs 12
```

For each pending `(active subscription × matching failed_tx)` row not yet
in `alert_delivery` with `status='delivered'`:

1. Re-check `webhook_url_is_safe` (defense in depth; if a row predates a
   policy tightening, the guard wins and the delivery is recorded as
   `failed`).
2. Sign the body `{"subscription_id":…,"tx_hash":"…"}` with HMAC-SHA256.
3. POST with `X-Amarillo-Signature: sha256=<hex>`, timeout 10s,
   redirects disabled.
4. `record_alert_delivery` upsert (PK `(subscription_id, tx_hash)`):
   - success → `status='delivered'`, `delivered_at=NOW()` (the anti-join
     excludes this row from all subsequent sweeps).
   - failure → `status='failed'`, `attempts+=1`, `last_error=…` (the
     anti-join keeps the row pending so it retries next sweep).

`Ctrl-C` stops between sweeps cleanly.

**Operational constraints (honest):**

- **Multi-dispatcher safe (HARDEN-T02)**. Before POSTing, the dispatcher
  calls `try_claim_alert_match` — an atomic `INSERT … ON CONFLICT DO
  UPDATE WHERE` that flips the row to `status='claimed'` only if it was
  previously `failed` or a stale `claimed` (>= `CLAIM_STALE_AFTER_SECS`
  = 60s; the `alert_delivery` PK plus the WHERE clause guarantee
  exactly one worker observes a successful claim for any given
  `(sub, tx)`). Workers that lose the race skip and increment
  `claim_skipped`. A crashed worker's `claimed` row is automatically
  re-claimable after 60 s. `find_pending_alert_matches`'s anti-join
  shares the same staleness window so the two functions stay in lock-
  step — no SELECT-then-POST race window left.
- **Bounded parallelism (HARDEN-T03/M2)**: `dispatch_once` now runs up
  to `MAX_CONCURRENT_POSTS` (= 10) POSTs concurrently via
  `tokio::task::JoinSet` — prime-N + drain-refill keeps the count at
  the cap. Worst-case sweep time drops from `batch × timeout` (~17 min
  at 100×10s) to roughly `batch / N × timeout` (~1.7 min), bounded by
  receiver responsiveness. A panicking task is logged and counted as
  `failed`; the cycle continues with the rest (alerts are
  best-effort).
- **Ctrl-C granularity**: still honored between cycles (existing
  `tokio::select!` at the wait phase). Cancellation *inside* a running
  POST is bounded by `REQUEST_TIMEOUT_SECS` (= 10 s).

## Verification

Pure logic (no network/DB):

```bash
cargo test -p db --lib   # validators::tests (SSRF guard, 9 cases)
cargo test -p indexer    # HMAC vector + payload determinism, etc.
```

DB integration (Postgres required):

```bash
cargo test -p db -- --ignored   # alert_match_is_idempotent_and_scoped (anti-join, retry, deactivate)
```

HTTP acceptance (local api on `:3001`, docker Postgres):

```bash
bash scripts/verify-alerts.sh
# expect: POST 201 + signing_secret once; SSRF/format → 400 (5 cases);
#         GET 200, NO signing_secret leak; DELETE 204 then 404; nonexistent 404.
```

Live webhook delivery needs a real receiver and is **not** in the
automated gate (D009~D012 verification-constraint pattern). Manual smoke:
register a subscription pointing at `https://webhook.site/…` (or any
HTTPS endpoint that accepts POST), run `--dispatch-alerts`, observe the
signed request on the receiver.

## Security posture (honest)

- **Signing**: HMAC-SHA256, per-subscription random 256-bit key. RFC-4231
  test vector pinned in unit tests. Verify on receiver in constant time.
- **SSRF**: scheme + IP-class + name-suffix guard (see endpoint above for
  full list). Redirects are disabled at the HTTP client to block the
  obvious one-hop bypass.
- **DNS-time SSRF (HARDEN3 / D020)**: the dispatcher injects a custom
  `dns_resolver` (`SafeDnsResolver`) into reqwest. The OS resolver runs
  on a blocking task, and **every resolved IP** is fed back through the
  same `db::validators::ip_is_safe` policy that screens URL literals at
  parse time. Result: an attacker domain that returns a public IP at
  subscription time but **rebinds to `127.0.0.1` (or any unsafe IP) at
  connect time** is rejected before the TCP handshake — the resolver
  returns `Err`, reqwest never opens the socket. Same policy, two
  enforcement points, single source of truth.
- **Residual** (after HARDEN3): the OS stub resolver's **response
  cache** is outside our reach — if a kernel-level resolver (nscd /
  systemd-resolved) hands us a stale cached IP that was poisoned
  between our look-ups, we cannot see that. Full closure requires
  doing DNS over UDP ourselves (hickory-dns-style), which is parked in
  BACKLOG until first user demand. Practical risk on the indexer's own
  network: low; not zero.
- **Secrets**: `signing_secret` is revealed *once* on creation **and
  on rotation** (HARDEN2-T02), never in list responses, never logged.
  `WS_URL` is env-only and also never logged (S07-T02 carryover).
- **`last_error` URL masking (HARDEN2-T01)**: reqwest error strings
  often include the failing URL / resolved IP / internal diagnostics,
  and they get persisted to `alert_delivery.last_error` (visible via
  any future admin-list endpoint). Before writing, `redact_urls`
  replaces every `http(s)://…` (up to the next whitespace) with
  `<redacted-url>`. The remaining 500-character cap (from HARDEN-T03/L1)
  bounds total length. Trade-off: this is whitespace-terminated, so a
  noisy receiver embedding URL fragments inside quotes/parens may
  over-redact slightly — preferred over under-redaction.
