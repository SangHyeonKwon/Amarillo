#!/usr/bin/env bash
# Verify /v1/alert-subscriptions (S08):
#   POST valid           -> 201 with { data: { subscription_id, signing_secret, ... } }
#   POST unsafe webhook  -> 400 (SSRF guard)
#   POST bad category    -> 400
#   POST bad to_addr     -> 400
#   GET                  -> 200; created id present; NO signing_secret leak in list
#   DELETE existing      -> 204
#   DELETE same again    -> 404
#   DELETE nonexistent   -> 404
#
# Requires a reachable Postgres with the S08 migration applied. Default
# targets docker-compose `postgres` on localhost:5432. Builds and runs the
# api on a test port (default 3001), then tears it down.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

: "${DATABASE_URL:=postgres://defi:defi@localhost:5432/defi_analytics}"
PORT="${API_PORT:-3001}"
export DATABASE_URL API_HOST=127.0.0.1 API_PORT="$PORT" RUST_LOG="${RUST_LOG:-warn}"

echo "building api..."
if ! cargo build -p api >/tmp/valerts-build.log 2>&1; then
  echo "FAIL — cargo build -p api failed:"; tail -20 /tmp/valerts-build.log; exit 1
fi

./target/debug/api >/tmp/verify-alerts-api.log 2>&1 &
API_PID=$!

CREATED_ID=""
cleanup() {
  if [ -n "$CREATED_ID" ]; then
    curl -fsS -X DELETE "http://127.0.0.1:$PORT/v1/alert-subscriptions/$CREATED_ID" >/dev/null 2>&1 || true
  fi
  kill "$API_PID" 2>/dev/null || true
}
trap cleanup EXIT

for _ in $(seq 1 60); do
  curl -fsS "http://127.0.0.1:$PORT/health" >/dev/null 2>&1 && break
  sleep 1
done

fail=0
URL="http://127.0.0.1:$PORT/v1/alert-subscriptions"
SUFFIX=$(date +%s)
WEBHOOK="https://example.test/hook-$SUFFIX"
TO_ADDR="0x00000000000000000000000000000000000000aa"

# --- POST valid -> 201 ---
pc=$(curl -s -o /tmp/valerts-create.json -w '%{http_code}' -H 'Content-Type: application/json' \
  -X POST "$URL" \
  --data "{\"webhook_url\":\"$WEBHOOK\",\"error_category\":\"SLIPPAGE_EXCEEDED\",\"to_addr\":\"$TO_ADDR\"}")
echo "POST valid: HTTP $pc"
if [ "$pc" = 201 ]; then
  CREATED_ID=$(node -e '
    const j = require("/tmp/valerts-create.json");
    const d = j.data || {};
    if (!d.subscription_id || !d.signing_secret) { console.error("missing fields"); process.exit(1); }
    if (typeof d.signing_secret !== "string" || d.signing_secret.length !== 64) {
      console.error("signing_secret not 64-hex"); process.exit(1);
    }
    process.stdout.write(String(d.subscription_id));
  ') || fail=1
  echo "  PASS (subscription_id=$CREATED_ID, signing_secret revealed once)"
else
  echo "  FAIL — expected 201"; cat /tmp/valerts-create.json; fail=1
fi

# --- POST unsafe webhook (SSRF) -> 400 ---
for BAD_URL in 'http://example.com/x' 'https://127.0.0.1/x' 'https://10.0.0.1/x' \
              'https://169.254.169.254/meta' 'https://localhost/x'; do
  bc=$(curl -s -o /tmp/valerts-bad.json -w '%{http_code}' -H 'Content-Type: application/json' \
    -X POST "$URL" --data "{\"webhook_url\":\"$BAD_URL\"}")
  if [ "$bc" = 400 ] && grep -q '"error"' /tmp/valerts-bad.json; then
    echo "POST unsafe ($BAD_URL): HTTP 400 PASS"
  else
    echo "POST unsafe ($BAD_URL): HTTP $bc FAIL"; cat /tmp/valerts-bad.json; fail=1
  fi
done

# --- POST bad category -> 400 ---
cbc=$(curl -s -o /tmp/valerts-cat.json -w '%{http_code}' -H 'Content-Type: application/json' \
  -X POST "$URL" --data "{\"webhook_url\":\"https://example.test/y\",\"error_category\":\"bogus\"}")
echo "POST bad category: HTTP $cbc"
if [ "$cbc" = 400 ]; then echo "  PASS"; else echo "  FAIL"; cat /tmp/valerts-cat.json; fail=1; fi

# --- POST bad to_addr -> 400 ---
tbc=$(curl -s -o /tmp/valerts-addr.json -w '%{http_code}' -H 'Content-Type: application/json' \
  -X POST "$URL" --data "{\"webhook_url\":\"https://example.test/z\",\"to_addr\":\"0xnothex\"}")
echo "POST bad to_addr: HTTP $tbc"
if [ "$tbc" = 400 ]; then echo "  PASS"; else echo "  FAIL"; cat /tmp/valerts-addr.json; fail=1; fi

# --- GET list -> 200, contains id, NO signing_secret leak ---
gc=$(curl -s -o /tmp/valerts-list.json -w '%{http_code}' "$URL?limit=500")
echo "GET list: HTTP $gc"
if [ "$gc" = 200 ] && [ -n "$CREATED_ID" ]; then
  node -e "
    const j = require('/tmp/valerts-list.json');
    const arr = (j.data || []);
    if (!Array.isArray(arr)) { console.log('  FAIL: data not array'); process.exit(1); }
    const found = arr.find(s => s.subscription_id === Number($CREATED_ID));
    if (!found) { console.log('  FAIL: created id $CREATED_ID not in list'); process.exit(1); }
    for (const s of arr) {
      if ('signing_secret' in s) {
        console.log('  FAIL: signing_secret leaked in GET for subscription_id=' + s.subscription_id);
        process.exit(1);
      }
    }
    console.log('  PASS (id present, NO signing_secret leak across ' + arr.length + ' rows)');
  " || fail=1
else
  echo "  FAIL"; cat /tmp/valerts-list.json; fail=1
fi

# --- DELETE existing -> 204 ---
if [ -n "$CREATED_ID" ]; then
  dc=$(curl -s -o /dev/null -w '%{http_code}' -X DELETE "$URL/$CREATED_ID")
  echo "DELETE $CREATED_ID: HTTP $dc"
  if [ "$dc" = 204 ]; then echo "  PASS"; else echo "  FAIL"; fail=1; fi

  # --- DELETE same id again -> 404 (already inactive) ---
  d2=$(curl -s -o /tmp/valerts-d2.json -w '%{http_code}' -X DELETE "$URL/$CREATED_ID")
  echo "DELETE $CREATED_ID (again): HTTP $d2"
  if [ "$d2" = 404 ] && grep -q '"error"' /tmp/valerts-d2.json; then
    echo "  PASS"
    CREATED_ID=""  # already cleaned up — skip trap-cleanup
  else
    echo "  FAIL"; cat /tmp/valerts-d2.json; fail=1
  fi
fi

# --- DELETE nonexistent -> 404 ---
nc=$(curl -s -o /tmp/valerts-nx.json -w '%{http_code}' -X DELETE "$URL/999999999")
echo "DELETE 999999999: HTTP $nc"
if [ "$nc" = 404 ] && grep -q '"error"' /tmp/valerts-nx.json; then
  echo "  PASS"
else
  echo "  FAIL"; cat /tmp/valerts-nx.json; fail=1
fi

if [ "$fail" -eq 0 ]; then
  echo "ALL PASS"
  exit 0
else
  echo "FAILURES present"
  exit 1
fi
