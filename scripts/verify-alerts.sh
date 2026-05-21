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
: "${AMARILLO_ADMIN_API_KEY:?required (S16/M006) — set in your env or .env. The api server fails to boot without it, and POST/DELETE/rotate-secret all require it (S17 — see docs/api-failed-tx.md#Authentication).}"
PORT="${API_PORT:-3001}"
export DATABASE_URL AMARILLO_ADMIN_API_KEY API_HOST=127.0.0.1 API_PORT="$PORT" RUST_LOG="${RUST_LOG:-warn}"

# S17 — admin/write Authorization header (D021/D022). Applied to all POST/DELETE/
# rotate-secret curl invocations below. GET endpoints (list) stay unauthenticated.
AUTH="Authorization: Bearer ${AMARILLO_ADMIN_API_KEY}"

echo "building api..."
if ! cargo build -p api >/tmp/valerts-build.log 2>&1; then
  echo "FAIL — cargo build -p api failed:"; tail -20 /tmp/valerts-build.log; exit 1
fi

./target/debug/api >/tmp/verify-alerts-api.log 2>&1 &
API_PID=$!

CREATED_ID=""
INITIAL_SECRET=""
ROTATED_SECRET=""
RATE_ID=""
cleanup() {
  if [ -n "$CREATED_ID" ]; then
    curl -fsS -H "$AUTH" -X DELETE "http://127.0.0.1:$PORT/v1/alert-subscriptions/$CREATED_ID" >/dev/null 2>&1 || true
  fi
  if [ -n "$RATE_ID" ]; then
    curl -fsS -H "$AUTH" -X DELETE "http://127.0.0.1:$PORT/v1/alert-subscriptions/$RATE_ID" >/dev/null 2>&1 || true
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
  -H "$AUTH" -X POST "$URL" \
  --data "{\"webhook_url\":\"$WEBHOOK\",\"error_category\":\"SLIPPAGE_EXCEEDED\",\"to_addr\":\"$TO_ADDR\"}")
echo "POST valid: HTTP $pc"
if [ "$pc" = 201 ]; then
  read -r CREATED_ID INITIAL_SECRET <<<"$(node -e '
    const j = require("/tmp/valerts-create.json");
    const d = j.data || {};
    if (!d.subscription_id || !d.signing_secret) { console.error("missing fields"); process.exit(1); }
    if (typeof d.signing_secret !== "string" || d.signing_secret.length !== 64) {
      console.error("signing_secret not 64-hex"); process.exit(1);
    }
    process.stdout.write(String(d.subscription_id) + " " + d.signing_secret);
  ')" || fail=1
  echo "  PASS (subscription_id=$CREATED_ID, signing_secret revealed once)"
else
  echo "  FAIL — expected 201"; cat /tmp/valerts-create.json; fail=1
fi

# --- POST unsafe webhook (SSRF) -> 400 ---
for BAD_URL in 'http://example.com/x' 'https://127.0.0.1/x' 'https://10.0.0.1/x' \
              'https://169.254.169.254/meta' 'https://localhost/x'; do
  bc=$(curl -s -o /tmp/valerts-bad.json -w '%{http_code}' -H 'Content-Type: application/json' \
    -H "$AUTH" -X POST "$URL" --data "{\"webhook_url\":\"$BAD_URL\"}")
  if [ "$bc" = 400 ] && grep -q '"error"' /tmp/valerts-bad.json; then
    echo "POST unsafe ($BAD_URL): HTTP 400 PASS"
  else
    echo "POST unsafe ($BAD_URL): HTTP $bc FAIL"; cat /tmp/valerts-bad.json; fail=1
  fi
done

# --- POST bad category -> 400 ---
cbc=$(curl -s -o /tmp/valerts-cat.json -w '%{http_code}' -H 'Content-Type: application/json' \
  -H "$AUTH" -X POST "$URL" --data "{\"webhook_url\":\"https://example.test/y\",\"error_category\":\"bogus\"}")
echo "POST bad category: HTTP $cbc"
if [ "$cbc" = 400 ]; then echo "  PASS"; else echo "  FAIL"; cat /tmp/valerts-cat.json; fail=1; fi

# --- POST bad to_addr -> 400 ---
tbc=$(curl -s -o /tmp/valerts-addr.json -w '%{http_code}' -H 'Content-Type: application/json' \
  -H "$AUTH" -X POST "$URL" --data "{\"webhook_url\":\"https://example.test/z\",\"to_addr\":\"0xnothex\"}")
echo "POST bad to_addr: HTTP $tbc"
if [ "$tbc" = 400 ]; then echo "  PASS"; else echo "  FAIL"; cat /tmp/valerts-addr.json; fail=1; fi

# --- S14/M005: rate_threshold scenarios ---
RATE_WEBHOOK="https://example.test/rate-$SUFFIX"

# POST rate_threshold valid -> 201 with sub_type + rate fields
rpc=$(curl -s -o /tmp/valerts-rate.json -w '%{http_code}' -H 'Content-Type: application/json' \
  -H "$AUTH" -X POST "$URL" \
  --data "{\"webhook_url\":\"$RATE_WEBHOOK\",\"sub_type\":\"rate_threshold\",\"threshold_count\":5,\"threshold_window_secs\":60,\"debounce_secs\":300}")
echo "POST rate valid: HTTP $rpc"
if [ "$rpc" = 201 ]; then
  RATE_ID=$(node -e '
    const d = (require("/tmp/valerts-rate.json").data) || {};
    if (d.sub_type !== "rate_threshold") { console.error("sub_type missing/wrong"); process.exit(1); }
    if (d.threshold_count !== 5 || d.threshold_window_secs !== 60 || d.debounce_secs !== 300) {
      console.error("rate fields mismatch"); process.exit(1);
    }
    if (typeof d.signing_secret !== "string" || d.signing_secret.length !== 64) {
      console.error("signing_secret not 64-hex"); process.exit(1);
    }
    process.stdout.write(String(d.subscription_id));
  ') || fail=1
  echo "  PASS (rate sub id=$RATE_ID, threshold=5/60s/debounce=300s)"
else
  echo "  FAIL — expected 201"; cat /tmp/valerts-rate.json; fail=1
fi

# POST rate_threshold without required fields -> 400
for BAD_RATE in \
  '{"webhook_url":"https://example.test/r1","sub_type":"rate_threshold"}' \
  '{"webhook_url":"https://example.test/r2","sub_type":"rate_threshold","threshold_count":5}' \
  '{"webhook_url":"https://example.test/r3","sub_type":"rate_threshold","threshold_count":0,"threshold_window_secs":60,"debounce_secs":0}' \
  '{"webhook_url":"https://example.test/r4","sub_type":"rate_threshold","threshold_count":5,"threshold_window_secs":-1,"debounce_secs":0}'; do
  rbc=$(curl -s -o /tmp/valerts-rbad.json -w '%{http_code}' -H 'Content-Type: application/json' \
    -H "$AUTH" -X POST "$URL" --data "$BAD_RATE")
  if [ "$rbc" = 400 ] && grep -q '"error"' /tmp/valerts-rbad.json; then
    echo "POST bad rate ($(echo "$BAD_RATE" | cut -c1-60)…): HTTP 400 PASS"
  else
    echo "POST bad rate: HTTP $rbc FAIL"; cat /tmp/valerts-rbad.json; fail=1
  fi
done

# POST per_event with rate fields -> 400 (mixed combination)
mc=$(curl -s -o /tmp/valerts-mix.json -w '%{http_code}' -H 'Content-Type: application/json' \
  -H "$AUTH" -X POST "$URL" \
  --data '{"webhook_url":"https://example.test/mix","sub_type":"per_event","threshold_count":5,"threshold_window_secs":60,"debounce_secs":0}')
echo "POST per_event + rate fields: HTTP $mc"
if [ "$mc" = 400 ] && grep -q '"error"' /tmp/valerts-mix.json; then
  echo "  PASS"
else
  echo "  FAIL"; cat /tmp/valerts-mix.json; fail=1
fi

# POST sub_type=bogus -> 400
bsc=$(curl -s -o /tmp/valerts-bsub.json -w '%{http_code}' -H 'Content-Type: application/json' \
  -H "$AUTH" -X POST "$URL" \
  --data '{"webhook_url":"https://example.test/bsub","sub_type":"bogus"}')
echo "POST sub_type=bogus: HTTP $bsc"
if [ "$bsc" = 400 ] && grep -q '"error"' /tmp/valerts-bsub.json; then
  echo "  PASS"
else
  echo "  FAIL"; cat /tmp/valerts-bsub.json; fail=1
fi

# GET list includes rate sub with rate fields visible (sub_type/threshold_*)
if [ -n "$RATE_ID" ]; then
  rgc=$(curl -s -o /tmp/valerts-rlist.json -w '%{http_code}' "$URL?limit=500")
  if [ "$rgc" = 200 ]; then
    node -e "
      const arr = (require('/tmp/valerts-rlist.json').data) || [];
      const r = arr.find(s => s.subscription_id === Number($RATE_ID));
      if (!r) { console.log('  FAIL: rate sub $RATE_ID not in list'); process.exit(1); }
      if (r.sub_type !== 'rate_threshold') { console.log('  FAIL: sub_type missing/wrong on list'); process.exit(1); }
      if (r.threshold_count !== 5 || r.threshold_window_secs !== 60 || r.debounce_secs !== 300) {
        console.log('  FAIL: rate fields not preserved on list'); process.exit(1);
      }
      if ('signing_secret' in r) { console.log('  FAIL: signing_secret leak'); process.exit(1); }
      console.log('  PASS (rate fields visible on GET, no secret leak)');
    " || fail=1
  else
    echo "  FAIL GET after rate POST: HTTP $rgc"; fail=1
  fi
fi

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

# --- ROTATE secret (HARDEN2-T02): 200 with NEW signing_secret ---
if [ -n "$CREATED_ID" ]; then
  rc=$(curl -s -o /tmp/valerts-rot.json -w '%{http_code}' -H "$AUTH" -X POST "$URL/$CREATED_ID/rotate-secret")
  echo "ROTATE $CREATED_ID: HTTP $rc"
  if [ "$rc" = 200 ]; then
    ROTATED_SECRET=$(node -e "
      const j = require('/tmp/valerts-rot.json');
      const d = j.data || {};
      if (typeof d.signing_secret !== 'string' || d.signing_secret.length !== 64) {
        console.error('rotated signing_secret not 64-hex'); process.exit(1);
      }
      if (d.signing_secret === '$INITIAL_SECRET') {
        console.error('rotated secret equals initial (no rotation happened)'); process.exit(1);
      }
      if (d.subscription_id !== Number($CREATED_ID)) {
        console.error('subscription_id changed unexpectedly'); process.exit(1);
      }
      process.stdout.write(d.signing_secret);
    ") || fail=1
    echo "  PASS (new secret differs from initial)"

    # GET list still must not leak signing_secret
    grc=$(curl -s -o /tmp/valerts-list2.json -w '%{http_code}' "$URL?limit=500")
    if [ "$grc" = 200 ]; then
      node -e "
        const j = require('/tmp/valerts-list2.json');
        for (const s of (j.data || [])) {
          if ('signing_secret' in s) {
            console.log('  FAIL: signing_secret leaked in GET after rotate'); process.exit(1);
          }
        }
        console.log('  PASS (no signing_secret leak in GET after rotate)');
      " || fail=1
    else
      echo "  FAIL (GET after rotate: HTTP $grc)"; fail=1
    fi
  else
    echo "  FAIL — expected 200"; cat /tmp/valerts-rot.json; fail=1
  fi
fi

# --- ROTATE nonexistent -> 404 ---
rnc=$(curl -s -o /tmp/valerts-rnx.json -w '%{http_code}' -H "$AUTH" -X POST "$URL/999999999/rotate-secret")
echo "ROTATE 999999999: HTTP $rnc"
if [ "$rnc" = 404 ] && grep -q '"error"' /tmp/valerts-rnx.json; then
  echo "  PASS"
else
  echo "  FAIL"; cat /tmp/valerts-rnx.json; fail=1
fi

# --- DELETE existing -> 204 ---
if [ -n "$CREATED_ID" ]; then
  dc=$(curl -s -o /dev/null -w '%{http_code}' -H "$AUTH" -X DELETE "$URL/$CREATED_ID")
  echo "DELETE $CREATED_ID: HTTP $dc"
  if [ "$dc" = 204 ]; then echo "  PASS"; else echo "  FAIL"; fail=1; fi

  # --- ROTATE inactive (just deactivated) -> 404 ---
  ric=$(curl -s -o /tmp/valerts-rinact.json -w '%{http_code}' -H "$AUTH" -X POST "$URL/$CREATED_ID/rotate-secret")
  echo "ROTATE $CREATED_ID (inactive): HTTP $ric"
  if [ "$ric" = 404 ] && grep -q '"error"' /tmp/valerts-rinact.json; then
    echo "  PASS"
  else
    echo "  FAIL"; cat /tmp/valerts-rinact.json; fail=1
  fi

  # --- DELETE same id again -> 404 (already inactive) ---
  d2=$(curl -s -o /tmp/valerts-d2.json -w '%{http_code}' -H "$AUTH" -X DELETE "$URL/$CREATED_ID")
  echo "DELETE $CREATED_ID (again): HTTP $d2"
  if [ "$d2" = 404 ] && grep -q '"error"' /tmp/valerts-d2.json; then
    echo "  PASS"
    CREATED_ID=""  # already cleaned up — skip trap-cleanup
  else
    echo "  FAIL"; cat /tmp/valerts-d2.json; fail=1
  fi
fi

# --- DELETE nonexistent -> 404 ---
nc=$(curl -s -o /tmp/valerts-nx.json -w '%{http_code}' -H "$AUTH" -X DELETE "$URL/999999999")
echo "DELETE 999999999: HTTP $nc"
if [ "$nc" = 404 ] && grep -q '"error"' /tmp/valerts-nx.json; then
  echo "  PASS"
else
  echo "  FAIL"; cat /tmp/valerts-nx.json; fail=1
fi

# --- AUTH (S17): missing header -> 401 (info-leak 방지로 단일 응답) ---
nac=$(curl -s -o /tmp/valerts-noauth.json -w '%{http_code}' -H 'Content-Type: application/json' \
  -X POST "$URL" --data '{"webhook_url":"https://example.test/noauth"}')
echo "POST no auth header: HTTP $nac"
if [ "$nac" = 401 ] && grep -q '"error":"unauthorized"' /tmp/valerts-noauth.json; then
  echo "  PASS"
else
  echo '  FAIL — expected 401 {"error":"unauthorized"}'; cat /tmp/valerts-noauth.json; fail=1
fi

# --- AUTH (S17): wrong key -> 401 (same response as missing — info-leak 방지) ---
wac=$(curl -s -o /tmp/valerts-wauth.json -w '%{http_code}' -H 'Content-Type: application/json' \
  -H "Authorization: Bearer wrong-key-xxxxxxxxxx" \
  -X POST "$URL" --data '{"webhook_url":"https://example.test/wrongkey"}')
echo "POST wrong key: HTTP $wac"
if [ "$wac" = 401 ] && grep -q '"error":"unauthorized"' /tmp/valerts-wauth.json; then
  echo "  PASS"
else
  echo '  FAIL — expected 401 {"error":"unauthorized"}'; cat /tmp/valerts-wauth.json; fail=1
fi

if [ "$fail" -eq 0 ]; then
  echo "ALL PASS"
  exit 0
else
  echo "FAILURES present"
  exit 1
fi
