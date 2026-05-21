/**
 * Three runnable scenarios against the Amarillo API. Run with:
 *
 *   npx tsx examples.ts http://localhost:3000
 *
 * (or compile with `tsc` and `node examples.js http://localhost:3000`).
 *
 * Each `demoXxx` is independent — comment out the calls in `main` to skip.
 *
 * The alert-subscription scenario (#2) hits write/admin endpoints that require
 * the admin API key. Set `AMARILLO_ADMIN_API_KEY` in your environment to run
 * it — otherwise it's skipped with a notice (S16/M006).
 */
import { createHmac } from "node:crypto";

import {
  AmarilloClient,
  AmarilloError,
  type FailedTxDetail,
  verifyAlertSignature,
} from "./client.ts";

const KNOWN_FAILED_TX =
  "0xdead000000000000000000000000000000000000000000000000000000000001";

// ── 1) Single-tx diagnosis: root_cause + decoded + diagnosis ─────────────

async function demoSingleDiagnosis(client: AmarilloClient): Promise<void> {
  console.log("\n=== 1. Single-tx diagnosis ===");
  let detail: FailedTxDetail;
  try {
    detail = await client.getFailedTx(KNOWN_FAILED_TX);
  } catch (err) {
    if (err instanceof AmarilloError && err.status === 404) {
      console.log(
        `  ${KNOWN_FAILED_TX} not seeded in this DB — try GOOD_HASH from scripts/verify-failed-tx.sh`,
      );
      return;
    }
    throw err;
  }
  console.log(`  tx_hash:        ${detail.failed.tx_hash}`);
  console.log(`  error_category: ${detail.failed.error_category}`);
  console.log(`  revert_reason:  ${detail.failed.revert_reason ?? "—"}`);
  if (detail.failing_function_decoded) {
    const d = detail.failing_function_decoded;
    console.log(`  failing fn:     ${d.name}  (${d.signature})`);
  } else {
    console.log(`  failing fn:     ${detail.failed.failing_function ?? "—"} (not in seed)`);
  }
  if (detail.root_cause) {
    console.log(
      `  root_cause:     trace_id=${detail.root_cause.trace_id} depth=${detail.root_cause.call_depth} err="${detail.root_cause.error}"`,
    );
  } else {
    console.log(`  root_cause:     null (indexer recorded no per-frame error)`);
  }
  if (detail.diagnosis) {
    console.log(`  diagnosis:      ${detail.diagnosis.message}`);
    if (detail.diagnosis.recommended_action) {
      console.log(`    → action:     ${detail.diagnosis.recommended_action}`);
    }
  } else {
    console.log(`  diagnosis:      null (category not seeded)`);
  }
  console.log(`  call_tree:      ${detail.call_tree.length} frames` +
    (detail.call_tree_truncated ? " (truncated)" : ""));
}

// ── 2) Alert subscription + webhook HMAC verification ───────────────────

async function demoAlertSubscription(client: AmarilloClient): Promise<void> {
  console.log("\n=== 2. Alert subscription + HMAC verification ===");
  // Create — signing_secret revealed exactly once. Drop it from memory
  // immediately after wiring it into your webhook receiver.
  const created = await client.createAlertSubscription({
    webhook_url: "https://example.com/amarillo-webhook",
    error_category: "SLIPPAGE_EXCEEDED",
  });
  console.log(`  subscription_id: ${created.subscription_id}`);
  console.log(`  signing_secret:  ${created.signing_secret.slice(0, 8)}…(hidden, 64 hex chars)`);
  const secret = created.signing_secret;

  // Demonstrate signature verification (without an actual incoming request).
  // The dispatcher hex-decodes the secret to 32 bytes and HMAC-SHA256s the
  // raw body — the receiver must do the same. See client.ts.
  const fakeBody = '{"tx_hash":"0xabc...","category":"SLIPPAGE_EXCEEDED"}';
  const key = Buffer.from(secret, "hex");
  const sigHex = createHmac("sha256", key).update(fakeBody).digest("hex");
  const headerValue = `sha256=${sigHex}`;

  const ok = verifyAlertSignature(fakeBody, headerValue, secret);
  const bad = verifyAlertSignature(fakeBody, "sha256=00", secret);
  console.log(`  verifyAlertSignature(valid):   ${ok}   (expected: true)`);
  console.log(`  verifyAlertSignature(invalid): ${bad}  (expected: false)`);

  // Cleanup — deactivate the demo subscription (operators would keep theirs).
  await client.deleteAlertSubscription(created.subscription_id);
  console.log(`  cleanup: deleted subscription_id=${created.subscription_id}`);
}

// ── 3) Failures by labeled contract ──────────────────────────────────────

async function demoByLabel(client: AmarilloClient): Promise<void> {
  console.log("\n=== 3. Failures by labeled contract ===");
  const rows = await client.getFailedTxByLabel({ limit: 10 });
  if (rows.length === 0) {
    console.log(`  (no label-joinable failures — seed labels via INSERT INTO contract_label …)`);
    return;
  }
  for (const r of rows) {
    const topCats = Object.entries(r.by_category)
      .sort(([, a], [, b]) => b - a)
      .slice(0, 3)
      .map(([cat, n]) => `${cat}=${n}`)
      .join(", ");
    console.log(`  ${r.label}  (${r.address.slice(0, 10)}…)  total=${r.total_failures}  [${topCats}]`);
  }
}

// ── Entry point ──────────────────────────────────────────────────────────

async function main(): Promise<void> {
  const baseUrl = process.argv[2];
  if (!baseUrl) {
    console.error("usage: tsx examples.ts <amarillo-base-url>   (e.g. http://localhost:3000)");
    process.exit(2);
  }
  const apiKey = process.env.AMARILLO_ADMIN_API_KEY;
  if (!apiKey) {
    console.log(
      "  note: AMARILLO_ADMIN_API_KEY not set — write/admin demo (#2) will be skipped.\n" +
      "        Set it in your env to run that scenario (S16/M006).",
    );
  }
  const client = new AmarilloClient(baseUrl, { apiKey });
  await demoSingleDiagnosis(client);
  if (apiKey) {
    await demoAlertSubscription(client);
  }
  await demoByLabel(client);
}

main().catch((err) => {
  console.error("FAILED:", err);
  process.exit(1);
});
