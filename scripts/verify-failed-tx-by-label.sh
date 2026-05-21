#!/usr/bin/env bash
# Verify GET /v1/analytics/failed-tx/by-label (S09 / M003) AND
# POST/DELETE /v1/contract-labels admin endpoints (S15 / M005):
#   - default call           -> 200 with array<{label, address, total_failures, by_category}>
#   - bad `from` (non-RFC3339) -> 400 with { "error": ... }
#   - owner=<unknown>        -> 200 with empty array (no tenancy match)
#   - POST new label         -> 201 with the row (address lowercased)
#   - POST same address      -> 201 with overwritten label (UPSERT semantics)
#   - POST invalid address   -> 400
#   - POST empty label       -> 400
#   - DELETE existing        -> 204
#   - DELETE same again      -> 404
#
# Tolerates empty data on the default by-label call — depending on the seed,
# label-joinable failed_transactions may or may not exist; the contract
# (shape + status codes) is what we pin.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

: "${DATABASE_URL:=postgres://defi:defi@localhost:5432/defi_analytics}"
PORT="${API_PORT:-3001}"
export DATABASE_URL API_HOST=127.0.0.1 API_PORT="$PORT" RUST_LOG="${RUST_LOG:-warn}"

echo "building api..."
if ! cargo build -p api >/tmp/vlbl-build.log 2>&1; then
  echo "FAIL — cargo build -p api failed:"; tail -20 /tmp/vlbl-build.log; exit 1
fi

./target/debug/api >/tmp/verify-by-label-api.log 2>&1 &
API_PID=$!
ADMIN_ADDR=""
cleanup() {
  if [ -n "$ADMIN_ADDR" ]; then
    curl -fsS -X DELETE "http://127.0.0.1:$PORT/v1/contract-labels/$ADMIN_ADDR" >/dev/null 2>&1 || true
  fi
  kill "$API_PID" 2>/dev/null || true
}
trap cleanup EXIT

for _ in $(seq 1 60); do
  curl -fsS "http://127.0.0.1:$PORT/health" >/dev/null 2>&1 && break
  sleep 1
done

fail=0
URL="http://127.0.0.1:$PORT/v1/analytics/failed-tx/by-label"

# --- default: 200 + array shape ---
dc=$(curl -s -o /tmp/vlbl-default.json -w '%{http_code}' "$URL?limit=20")
echo "GET default: HTTP $dc"
if [ "$dc" = 200 ]; then
  node -e '
    const j = require("/tmp/vlbl-default.json");
    if (!j || !Array.isArray(j.data)) {
      console.log("  FAIL: data not array"); process.exit(1);
    }
    for (const row of j.data) {
      if (typeof row.label !== "string"
          || typeof row.address !== "string"
          || typeof row.total_failures !== "number"
          || typeof row.by_category !== "object"
          || row.by_category === null) {
        console.log("  FAIL: row shape", JSON.stringify(row).slice(0, 120));
        process.exit(1);
      }
      // address must be lowercased 0x + 40 hex (contract_label invariant).
      if (!/^0x[0-9a-f]{40}$/.test(row.address)) {
        console.log("  FAIL: address not lowercased 0x + 40 hex:", row.address);
        process.exit(1);
      }
      // by_category sum must equal total_failures (pivot invariant).
      const sum = Object.values(row.by_category).reduce((a, b) => a + b, 0);
      if (sum !== row.total_failures) {
        console.log("  FAIL: by_category sum", sum, "!= total_failures", row.total_failures);
        process.exit(1);
      }
    }
    console.log("  PASS (", j.data.length, "rows, shape + invariants OK)");
  ' || fail=1
else
  echo "  FAIL — expected 200"; cat /tmp/vlbl-default.json; fail=1
fi

# --- bad `from` -> 400 ---
bc=$(curl -s -o /tmp/vlbl-bad.json -w '%{http_code}' "$URL?from=not-a-time")
echo "GET bad from: HTTP $bc"
if [ "$bc" = 400 ] && grep -q '"error"' /tmp/vlbl-bad.json; then
  echo "  PASS"
else
  echo '  FAIL — expected 400 {"error":...}'; cat /tmp/vlbl-bad.json; fail=1
fi

# --- owner=<unknown> -> 200 + empty data ---
oc=$(curl -s -o /tmp/vlbl-owner.json -w '%{http_code}' "$URL?owner=nobody-test-1234567")
echo "GET owner=nobody: HTTP $oc"
if [ "$oc" = 200 ]; then
  node -e '
    const j = require("/tmp/vlbl-owner.json");
    if (!Array.isArray(j.data) || j.data.length !== 0) {
      console.log("  FAIL: expected empty array, got", JSON.stringify(j.data).slice(0, 120));
      process.exit(1);
    }
    console.log("  PASS (empty array)");
  ' || fail=1
else
  echo "  FAIL — expected 200"; cat /tmp/vlbl-owner.json; fail=1
fi

# --- S15 / M005: contract-labels admin endpoints ---
ADMIN_URL="http://127.0.0.1:$PORT/v1/contract-labels"
ADMIN_ADDR="0xfeed000000000000000000000000000000000515"

# POST create -> 201, address lowercased + label/owner round-trip
ac=$(curl -s -o /tmp/vlbl-admin-create.json -w '%{http_code}' -H 'Content-Type: application/json' \
  -X POST "$ADMIN_URL" \
  --data "{\"address\":\"$ADMIN_ADDR\",\"label\":\"Admin Test Bot\",\"owner_id\":\"verify-script\"}")
echo "POST create: HTTP $ac"
if [ "$ac" = 201 ]; then
  node -e "
    const d = (require('/tmp/vlbl-admin-create.json').data) || {};
    if (d.address !== '$ADMIN_ADDR') { console.log('  FAIL: address mismatch (lowercased?):', d.address); process.exit(1); }
    if (d.label !== 'Admin Test Bot') { console.log('  FAIL: label mismatch:', d.label); process.exit(1); }
    if (d.owner_id !== 'verify-script') { console.log('  FAIL: owner_id mismatch:', d.owner_id); process.exit(1); }
    console.log('  PASS (address=' + d.address.slice(0, 10) + '… label=\"' + d.label + '\")');
  " || fail=1
else
  echo "  FAIL — expected 201"; cat /tmp/vlbl-admin-create.json; fail=1
fi

# POST upsert (same address, new label) -> 201 with overwritten label
uc=$(curl -s -o /tmp/vlbl-admin-upsert.json -w '%{http_code}' -H 'Content-Type: application/json' \
  -X POST "$ADMIN_URL" \
  --data "{\"address\":\"$ADMIN_ADDR\",\"label\":\"Renamed Bot\",\"owner_id\":\"verify-script\"}")
echo "POST upsert: HTTP $uc"
if [ "$uc" = 201 ]; then
  node -e "
    const d = (require('/tmp/vlbl-admin-upsert.json').data) || {};
    if (d.label !== 'Renamed Bot') {
      console.log('  FAIL: UPSERT did not overwrite label, got:', d.label); process.exit(1);
    }
    console.log('  PASS (label overwritten to \"' + d.label + '\")');
  " || fail=1
else
  echo "  FAIL — expected 201"; cat /tmp/vlbl-admin-upsert.json; fail=1
fi

# POST invalid address -> 400
ic=$(curl -s -o /tmp/vlbl-admin-bad.json -w '%{http_code}' -H 'Content-Type: application/json' \
  -X POST "$ADMIN_URL" \
  --data '{"address":"0xnothex","label":"Bad"}')
echo "POST bad address: HTTP $ic"
if [ "$ic" = 400 ] && grep -q '"error"' /tmp/vlbl-admin-bad.json; then
  echo "  PASS"
else
  echo "  FAIL"; cat /tmp/vlbl-admin-bad.json; fail=1
fi

# POST empty label -> 400
ec=$(curl -s -o /tmp/vlbl-admin-emptylabel.json -w '%{http_code}' -H 'Content-Type: application/json' \
  -X POST "$ADMIN_URL" \
  --data "{\"address\":\"0xaaaa000000000000000000000000000000000aaa\",\"label\":\"\"}")
echo "POST empty label: HTTP $ec"
if [ "$ec" = 400 ] && grep -q '"error"' /tmp/vlbl-admin-emptylabel.json; then
  echo "  PASS"
else
  echo "  FAIL"; cat /tmp/vlbl-admin-emptylabel.json; fail=1
fi

# DELETE existing -> 204
dc=$(curl -s -o /dev/null -w '%{http_code}' -X DELETE "$ADMIN_URL/$ADMIN_ADDR")
echo "DELETE existing: HTTP $dc"
if [ "$dc" = 204 ]; then
  echo "  PASS"
  ADMIN_ADDR=""  # already cleaned up
else
  echo "  FAIL — expected 204"; fail=1
fi

# DELETE again -> 404
dc2=$(curl -s -o /tmp/vlbl-admin-del2.json -w '%{http_code}' -X DELETE "$ADMIN_URL/0xfeed000000000000000000000000000000000515")
echo "DELETE same again: HTTP $dc2"
if [ "$dc2" = 404 ] && grep -q '"error"' /tmp/vlbl-admin-del2.json; then
  echo "  PASS"
else
  echo "  FAIL"; cat /tmp/vlbl-admin-del2.json; fail=1
fi

# DELETE invalid address -> 400
dbc=$(curl -s -o /tmp/vlbl-admin-delbad.json -w '%{http_code}' -X DELETE "$ADMIN_URL/0xnothex")
echo "DELETE bad address: HTTP $dbc"
if [ "$dbc" = 400 ] && grep -q '"error"' /tmp/vlbl-admin-delbad.json; then
  echo "  PASS"
else
  echo "  FAIL"; cat /tmp/vlbl-admin-delbad.json; fail=1
fi

if [ "$fail" -eq 0 ]; then
  echo "ALL PASS"
  exit 0
else
  echo "FAILURES present"
  exit 1
fi
