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
: "${AMARILLO_ADMIN_API_KEY:?required (S16/M006) — set in your env or .env. The api server fails to boot without it.}"
PORT="${API_PORT:-3001}"
GOOD_HASH="${GOOD_HASH:-0xdead000000000000000000000000000000000000000000000000000000000001}"
BAD_HASH="0x0000000000000000000000000000000000000000000000000000000000000000"
# Auth note (S17): this script hits only **public GET** endpoints, so no
# Authorization header is attached to any curl. The admin key is still
# required because ApiConfig::from_env refuses to boot without it (D023).
export DATABASE_URL AMARILLO_ADMIN_API_KEY API_HOST=127.0.0.1 API_PORT="$PORT" RUST_LOG="${RUST_LOG:-warn}"

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

# Assert root_cause semantics (S10 / M004) — must be either null or an object
# whose trace_id equals the first error frame in call_tree (self-consistency).
if [ "$gc" = 200 ]; then
  node -e '
    const j = require("/tmp/vftx-good.json");
    const d = j.data || {};
    if (!Object.prototype.hasOwnProperty.call(d, "root_cause")) {
      console.log("  ROOT FAIL: root_cause field missing"); process.exit(1);
    }
    const rc = d.root_cause;
    if (rc === null) {
      console.log("  ROOT OK (null — indexer recorded no per-frame error)");
    } else if (typeof rc !== "object") {
      console.log("  ROOT FAIL: root_cause must be null or object, got " + typeof rc);
      process.exit(1);
    } else {
      if (rc.error == null) {
        console.log("  ROOT FAIL: root_cause.error must be non-null by definition");
        process.exit(1);
      }
      const t = d.call_tree || [];
      const first = t.find(f => f.error != null);
      if (!first) {
        console.log("  ROOT FAIL: root_cause present but no error frame in call_tree");
        process.exit(1);
      }
      if (first.trace_id !== rc.trace_id) {
        console.log("  ROOT FAIL: root_cause.trace_id=" + rc.trace_id +
                    " but first call_tree error frame trace_id=" + first.trace_id);
        process.exit(1);
      }
      console.log("  ROOT OK (trace_id=" + rc.trace_id + " matches first error frame in call_tree)");
    }
  ' || fail=1
fi

# Assert failing_function_decoded semantics (S11 / M004) — must be either null
# or an object whose selector equals data.failed.failing_function (lowercased)
# and whose name/signature are non-empty.
if [ "$gc" = 200 ]; then
  node -e '
    const j = require("/tmp/vftx-good.json");
    const d = j.data || {};
    if (!Object.prototype.hasOwnProperty.call(d, "failing_function_decoded")) {
      console.log("  DECODED FAIL: failing_function_decoded field missing");
      process.exit(1);
    }
    const fd = d.failing_function_decoded;
    if (fd === null) {
      console.log("  DECODED OK (null — selector absent or not in self-owned ABI seed)");
    } else if (typeof fd !== "object") {
      console.log("  DECODED FAIL: must be null or object, got " + typeof fd);
      process.exit(1);
    } else {
      const fnSelector = d.failed && d.failed.failing_function;
      if (!fnSelector) {
        console.log("  DECODED FAIL: decoded object present but failed.failing_function is null");
        process.exit(1);
      }
      if (fd.selector !== fnSelector.toLowerCase()) {
        console.log("  DECODED FAIL: selector mismatch " + fd.selector + " vs " + fnSelector);
        process.exit(1);
      }
      if (fd.selector !== fd.selector.toLowerCase()) {
        console.log("  DECODED FAIL: selector must be lowercased"); process.exit(1);
      }
      if (typeof fd.name !== "string" || !fd.name.length) {
        console.log("  DECODED FAIL: name must be non-empty string"); process.exit(1);
      }
      if (typeof fd.signature !== "string" || !fd.signature.length) {
        console.log("  DECODED FAIL: signature must be non-empty string"); process.exit(1);
      }
      // S11.1 — args is either null (decode skipped / failed) or an array of
      // { type: string, value: any }. Each top-level type token from the
      // signature must show up positionally in `args[i].type` when present.
      if (!Object.prototype.hasOwnProperty.call(fd, "args")) {
        console.log("  DECODED FAIL: args field missing (S11.1 — must be null or array, not absent)");
        process.exit(1);
      }
      if (fd.args !== null) {
        if (!Array.isArray(fd.args)) {
          console.log("  DECODED FAIL: args must be null or array, got " + typeof fd.args);
          process.exit(1);
        }
        for (const a of fd.args) {
          if (typeof a !== "object" || a === null) {
            console.log("  DECODED FAIL: arg element must be object"); process.exit(1);
          }
          if (typeof a.type !== "string" || !a.type.length) {
            console.log("  DECODED FAIL: arg.type must be non-empty string"); process.exit(1);
          }
          if (!("value" in a)) {
            console.log("  DECODED FAIL: arg.value missing"); process.exit(1);
          }
        }
      }
      console.log("  DECODED OK (" + fd.name + " :: " + fd.signature +
        (fd.args === null ? "; args=null" : "; args=" + fd.args.length + " typed") + ")");
    }
  ' || fail=1
fi

# Assert root_cause_decoded semantics (S11.1) — null or DecodedFunction object
# whose selector equals the first 4 bytes of root_cause.input (lowercased).
# Args are validated with the same shape rules as failing_function_decoded.
if [ "$gc" = 200 ]; then
  node -e '
    const j = require("/tmp/vftx-good.json");
    const d = j.data || {};
    if (!Object.prototype.hasOwnProperty.call(d, "root_cause_decoded")) {
      console.log("  ROOT_DECODED FAIL: field missing (S11.1 — must be null or object, not absent)");
      process.exit(1);
    }
    const rd = d.root_cause_decoded;
    if (rd === null) {
      console.log("  ROOT_DECODED OK (null — root_cause/input absent or selector unseeded)");
    } else if (typeof rd !== "object") {
      console.log("  ROOT_DECODED FAIL: must be null or object, got " + typeof rd);
      process.exit(1);
    } else {
      const rcInput = d.root_cause && d.root_cause.input;
      if (typeof rcInput !== "string" || rcInput.length < 10) {
        console.log("  ROOT_DECODED FAIL: object present but root_cause.input is missing/too short");
        process.exit(1);
      }
      // First 4 bytes of input (8 hex chars after 0x) must match selector.
      const expected = "0x" + rcInput.replace(/^0x/, "").slice(0, 8).toLowerCase();
      if (rd.selector !== expected) {
        console.log("  ROOT_DECODED FAIL: selector " + rd.selector + " vs expected " + expected);
        process.exit(1);
      }
      if (typeof rd.name !== "string" || !rd.name.length ||
          typeof rd.signature !== "string" || !rd.signature.length) {
        console.log("  ROOT_DECODED FAIL: name/signature must be non-empty");
        process.exit(1);
      }
      if (!Object.prototype.hasOwnProperty.call(rd, "args")) {
        console.log("  ROOT_DECODED FAIL: args field missing"); process.exit(1);
      }
      if (rd.args !== null && !Array.isArray(rd.args)) {
        console.log("  ROOT_DECODED FAIL: args must be null or array"); process.exit(1);
      }
      console.log("  ROOT_DECODED OK (" + rd.name + " :: " + rd.signature + ")");
    }
  ' || fail=1
fi

# Assert diagnosis semantics (S12 / M004) — must be either null or an object.
# For seeded categories (the 6 ErrorCategory variants in SCREAMING_SNAKE), it
# must be non-null with a non-empty message (the migration seed guarantees it).
if [ "$gc" = 200 ]; then
  node -e '
    const SEEDED = new Set([
      // 기존 6 카테고리
      "INSUFFICIENT_BALANCE", "SLIPPAGE_EXCEEDED", "DEADLINE_EXPIRED",
      "UNAUTHORIZED", "TRANSFER_FAILED", "UNKNOWN",
      // S12.1 신규 4 세부 카테고리 — 마이그레이션 20240109의 시드 행이 보장
      "INSUFFICIENT_ALLOWANCE", "SLIPPAGE_AMOUNT_OUT",
      "SLIPPAGE_AMOUNT_IN", "SLIPPAGE_PRICE_IMPACT",
    ]);
    // Serde may emit the variant name ("Unknown") or the wire form
    // ("UNKNOWN") depending on enum tagging. Canonicalize for the check.
    const canon = (v) => {
      if (typeof v === "string") {
        return v.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toUpperCase();
      }
      if (v && typeof v === "object") {
        const k = Object.keys(v)[0];
        return k ? canon(k) : "";
      }
      return "";
    };
    const j = require("/tmp/vftx-good.json");
    const d = j.data || {};
    if (!Object.prototype.hasOwnProperty.call(d, "diagnosis")) {
      console.log("  DIAG FAIL: diagnosis field missing"); process.exit(1);
    }
    const dg = d.diagnosis;
    if (dg !== null && typeof dg !== "object") {
      console.log("  DIAG FAIL: must be null or object, got " + typeof dg);
      process.exit(1);
    }
    if (dg !== null) {
      if (typeof dg.message !== "string" || !dg.message.length) {
        console.log("  DIAG FAIL: message must be non-empty string"); process.exit(1);
      }
      // recommended_action and source: string or null (no other types).
      for (const k of ["recommended_action", "source"]) {
        if (dg[k] !== null && typeof dg[k] !== "string") {
          console.log("  DIAG FAIL: " + k + " must be string or null"); process.exit(1);
        }
      }
    }
    const cat = canon(d.failed && d.failed.error_category);
    if (SEEDED.has(cat) && dg === null) {
      console.log("  DIAG FAIL: seeded category " + cat + " must yield non-null diagnosis");
      process.exit(1);
    }
    console.log("  DIAG OK (" + (dg ? "msg=\"" + dg.message.slice(0, 40) + "…\"" : "null") + ")");
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
