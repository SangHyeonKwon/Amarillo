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

- **Run a single dispatcher.** `find_pending_alert_matches` (SELECT) and
  `record_alert_delivery` (UPSERT) are *not* atomic; two dispatchers can
  pick the same `(subscription, tx)` and both POST before either records
  a delivered row. `alert_delivery`'s composite PK still prevents
  duplicate rows, but the webhook fires twice. Worker-claim semantics
  (e.g. `SELECT … FOR UPDATE SKIP LOCKED`) are backlog — until then,
  deploy exactly one `--dispatch-alerts` process.
- **Per-cycle POSTs are sequential.** Batch 100 × 10 s timeout ⇒ a fully
  pending sweep can take ~17 min before the next sleep, and `Ctrl-C` is
  only honored between sweeps. Bounded parallelism (`buffer_unordered`)
  is backlog; current behaviour fits "small N of subscribers".

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
- **Residual**: DNS-time IP rebinding (an attacker domain that resolves
  to a private IP at connect time) is **not** caught by name-based
  rejection alone — connect-time IP check is backlog. Practical risk on
  the indexer's own network: low; not zero.
- **Secrets**: `signing_secret` is revealed *once* on creation, never in
  list responses, never logged. `WS_URL` is env-only and also never
  logged (S07-T02 carryover).
