#!/usr/bin/env bash
# Verify GET /v1/failed-tx/{tx_hash}:
#   - a seeded failed hash      -> 200 with { data: { failed, call_tree } }
#   - an unknown hash           -> 404 with { error: ... }
#
# Requires a reachable Postgres with seed data. Default targets the
# docker-compose `postgres` service published on localhost:5432
# (run: `docker compose up -d` && `docker compose run --rm seed`).
# Builds and runs the api on a test port (default 3001), then tears it down.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

: "${DATABASE_URL:=postgres://defi:defi@localhost:5432/defi_analytics}"
PORT="${API_PORT:-3001}"
GOOD_HASH="${GOOD_HASH:-0xdead000000000000000000000000000000000000000000000000000000000001}"
BAD_HASH="0x0000000000000000000000000000000000000000000000000000000000000000"
export DATABASE_URL API_HOST=127.0.0.1 API_PORT="$PORT" RUST_LOG="${RUST_LOG:-warn}"

echo "building api..."
if ! cargo build -p api >/tmp/vftx-build.log 2>&1; then
  echo "FAIL — cargo build -p api failed:"; tail -20 /tmp/vftx-build.log; exit 1
fi

./target/debug/api >/tmp/verify-failed-tx-api.log 2>&1 &
API_PID=$!
cleanup() { kill "$API_PID" 2>/dev/null || true; }
trap cleanup EXIT

for _ in $(seq 1 60); do
  curl -fsS "http://127.0.0.1:$PORT/health" >/dev/null 2>&1 && break
  sleep 1
done

fail=0
gc=$(curl -s -o /tmp/vftx-good.json -w '%{http_code}' "http://127.0.0.1:$PORT/v1/failed-tx/$GOOD_HASH")
bc=$(curl -s -o /tmp/vftx-bad.json  -w '%{http_code}' "http://127.0.0.1:$PORT/v1/failed-tx/$BAD_HASH")

echo "GOOD ($GOOD_HASH): HTTP $gc"
if [ "$gc" = 200 ] && grep -q '"call_tree"' /tmp/vftx-good.json \
   && grep -q '"error_category"' /tmp/vftx-good.json; then
  echo "  PASS"
else
  echo "  FAIL — expected 200 with failed + call_tree"; cat /tmp/vftx-good.json; fail=1
fi

# Assert call_tree semantics, not just presence (regression guard for the
# pre-order/trace_id ordering — see queries.rs::list_trace_logs_by_tx).
if [ "$gc" = 200 ]; then
  node -e '
    const j = require("/tmp/vftx-good.json");
    const t = (j.data && j.data.call_tree) || [];
    if (!t.length) { console.log("  ORDER FAIL: empty call_tree"); process.exit(1); }
    if (t[0].call_depth !== 0) { console.log("  ORDER FAIL: root frame not first"); process.exit(1); }
    for (let i = 1; i < t.length; i++)
      if (t[i].trace_id <= t[i - 1].trace_id) {
        console.log("  ORDER FAIL: trace_id not strictly ascending at index " + i);
        process.exit(1);
      }
    console.log("  ORDER OK (pre-order: root first, trace_id strictly ascending)");
  ' || fail=1
fi

echo "BAD  ($BAD_HASH): HTTP $bc"
if [ "$bc" = 404 ] && grep -q '"error"' /tmp/vftx-bad.json; then
  echo "  PASS"
else
  echo '  FAIL — expected 404 {"error":...}'; cat /tmp/vftx-bad.json; fail=1
fi

# --- S04 L2: malformed hash -> 400 (not 404) ---
mc=$(curl -s -o /tmp/vftx-mal.json -w '%{http_code}' "http://127.0.0.1:$PORT/v1/failed-tx/0xnothex")
echo "MALFORMED (0xnothex): HTTP $mc"
if [ "$mc" = 400 ] && grep -q '"error"' /tmp/vftx-mal.json; then
  echo "  PASS"
else
  echo '  FAIL — expected 400 {"error":...}'; cat /tmp/vftx-mal.json; fail=1
fi

# --- S02: list endpoint (filter + accurate total + 400 on bad input) ---
lc=$(curl -s -o /tmp/vftx-list.json -w '%{http_code}' \
  "http://127.0.0.1:$PORT/v1/failed-tx?category=UNKNOWN&limit=2")
echo "LIST (?category=UNKNOWN&limit=2): HTTP $lc"
if [ "$lc" = 200 ]; then
  node -e '
    const j = require("/tmp/vftx-list.json");
    const d = j.data || [], p = j.pagination || {};
    if (typeof p.total !== "number") { console.log("  FAIL: pagination.total missing"); process.exit(1); }
    if (d.length > 2) { console.log("  FAIL: limit not applied"); process.exit(1); }
    if (d.length > p.total) { console.log("  FAIL: count > total"); process.exit(1); }
    if (!d.every(r => r.error_category === "Unknown")) { console.log("  FAIL: category filter leaked"); process.exit(1); }
    console.log("  PASS (total=" + p.total + ", returned=" + d.length + ")");
  ' || fail=1
else
  echo "  FAIL — expected 200"; cat /tmp/vftx-list.json; fail=1
fi

for q in "category=BOGUS" "from=not-a-date"; do
  hc=$(curl -s -o /tmp/vftx-400.json -w '%{http_code}' "http://127.0.0.1:$PORT/v1/failed-tx?$q")
  echo "LIST (?$q): HTTP $hc"
  if [ "$hc" = 400 ] && grep -q '"error"' /tmp/vftx-400.json; then
    echo "  PASS"
  else
    echo '  FAIL — expected 400 {"error":...}'; cat /tmp/vftx-400.json; fail=1
  fi
done

# --- S03: timeseries (bucketed trend + 400 on bad interval) ---
tc=$(curl -s -o /tmp/vftx-ts.json -w '%{http_code}' \
  "http://127.0.0.1:$PORT/v1/analytics/failed-tx/timeseries?interval=day")
echo "TIMESERIES (?interval=day): HTTP $tc"
if [ "$tc" = 200 ]; then
  node -e '
    const j = require("/tmp/vftx-ts.json");
    const d = j.data || [];
    if (!Array.isArray(d) || d.length < 1) { console.log("  FAIL: empty/non-array data"); process.exit(1); }
    for (const p of d)
      if (!("bucket" in p) || !("error_category" in p) || typeof p.failure_count !== "number") {
        console.log("  FAIL: unexpected point shape"); process.exit(1);
      }
    for (let i = 1; i < d.length; i++)
      if (d[i].bucket < d[i - 1].bucket) { console.log("  FAIL: buckets not ascending"); process.exit(1); }
    console.log("  PASS (points=" + d.length + ")");
  ' || fail=1
else
  echo "  FAIL — expected 200"; cat /tmp/vftx-ts.json; fail=1
fi

tbc=$(curl -s -o /tmp/vftx-tsbad.json -w '%{http_code}' \
  "http://127.0.0.1:$PORT/v1/analytics/failed-tx/timeseries?interval=bogus")
echo "TIMESERIES (?interval=bogus): HTTP $tbc"
if [ "$tbc" = 400 ] && grep -q '"error"' /tmp/vftx-tsbad.json; then
  echo "  PASS"
else
  echo '  FAIL — expected 400 {"error":...}'; cat /tmp/vftx-tsbad.json; fail=1
fi

[ "$fail" = 0 ] && echo "ALL PASS" || echo "FAILURES"
exit "$fail"
