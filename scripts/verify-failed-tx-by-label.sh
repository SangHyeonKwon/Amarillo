#!/usr/bin/env bash
# Verify GET /v1/analytics/failed-tx/by-label (S09 / M003):
#   - default call -> 200 with array<{label, address, total_failures, by_category}>
#   - bad `from` (non-RFC3339) -> 400 with { "error": ... }
#   - owner=<unknown> -> 200 with empty array (no tenancy match)
#
# Tolerates empty data on the default call — depending on the seed,
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
cleanup() { kill "$API_PID" 2>/dev/null || true; }
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

if [ "$fail" -eq 0 ]; then
  echo "ALL PASS"
  exit 0
else
  echo "FAILURES present"
  exit 1
fi
