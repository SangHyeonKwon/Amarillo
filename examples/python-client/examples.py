#!/usr/bin/env python3
"""
Three runnable scenarios against the Amarillo API. Run with:

    python3 examples.py http://localhost:3000

Each ``demo_xxx`` is independent — comment out the calls in ``main`` to skip.

The alert-subscription scenario (#2) hits write/admin endpoints that require
the admin API key. Set ``AMARILLO_ADMIN_API_KEY`` in your environment to run
it — otherwise it's skipped with a notice (S16/M006).
"""
import hashlib
import hmac
import os
import sys

from client import AmarilloClient, AmarilloError, verify_alert_signature

KNOWN_FAILED_TX = "0xdead000000000000000000000000000000000000000000000000000000000001"


def demo_single_diagnosis(client: AmarilloClient) -> None:
    """Scenario 1: single-tx diagnosis (root_cause + decoded + diagnosis)."""
    print("\n=== 1. Single-tx diagnosis ===")
    try:
        detail = client.get_failed_tx(KNOWN_FAILED_TX)
    except AmarilloError as err:
        if err.status == 404:
            print(
                f"  {KNOWN_FAILED_TX} not seeded — try GOOD_HASH from "
                "scripts/verify-failed-tx.sh"
            )
            return
        raise

    f = detail.failed
    print(f"  tx_hash:        {f.tx_hash}")
    print(f"  error_category: {f.error_category}")
    print(f"  revert_reason:  {f.revert_reason or '—'}")

    if detail.failing_function_decoded is not None:
        d = detail.failing_function_decoded
        print(f"  failing fn:     {d.name}  ({d.signature})")
    else:
        print(f"  failing fn:     {f.failing_function or '—'} (not in seed)")

    if detail.root_cause is not None:
        r = detail.root_cause
        print(
            f"  root_cause:     trace_id={r.trace_id} depth={r.call_depth} "
            f"err={r.error!r}"
        )
    else:
        print("  root_cause:     null (indexer recorded no per-frame error)")

    if detail.diagnosis is not None:
        dg = detail.diagnosis
        print(f"  diagnosis:      {dg.message}")
        if dg.recommended_action:
            print(f"    → action:     {dg.recommended_action}")
    else:
        print("  diagnosis:      null (category not seeded)")

    suffix = " (truncated)" if detail.call_tree_truncated else ""
    print(f"  call_tree:      {len(detail.call_tree)} frames{suffix}")


def demo_alert_subscription(client: AmarilloClient) -> None:
    """Scenario 2: alert subscription + HMAC verification."""
    print("\n=== 2. Alert subscription + HMAC verification ===")
    created = client.create_alert_subscription(
        webhook_url="https://example.com/amarillo-webhook",
        error_category="SLIPPAGE_EXCEEDED",
    )
    print(f"  subscription_id: {created.subscription_id}")
    print(
        f"  signing_secret:  {created.signing_secret[:8]}…"
        "(hidden, 64 hex chars)"
    )

    # Demonstrate signature verification without an actual incoming request.
    # The dispatcher hex-decodes the secret to 32 bytes and HMAC-SHA256s the
    # raw body — the receiver must do the same.
    secret = created.signing_secret
    fake_body = b'{"tx_hash":"0xabc...","category":"SLIPPAGE_EXCEEDED"}'
    key = bytes.fromhex(secret)
    sig_hex = hmac.new(key, fake_body, hashlib.sha256).hexdigest()
    header_value = f"sha256={sig_hex}"

    ok = verify_alert_signature(fake_body, header_value, secret)
    bad = verify_alert_signature(fake_body, "sha256=00", secret)
    print(f"  verify_alert_signature(valid):   {ok}   (expected: True)")
    print(f"  verify_alert_signature(invalid): {bad}  (expected: False)")

    # Cleanup — operators would keep theirs.
    client.delete_alert_subscription(created.subscription_id)
    print(f"  cleanup: deleted subscription_id={created.subscription_id}")


def demo_by_label(client: AmarilloClient) -> None:
    """Scenario 3: failures by labeled contract (S09)."""
    print("\n=== 3. Failures by labeled contract ===")
    rows = client.get_failed_tx_by_label(limit=10)
    if not rows:
        print(
            "  (no label-joinable failures — seed labels via "
            "INSERT INTO contract_label …)"
        )
        return
    for r in rows:
        top_cats = sorted(r.by_category.items(), key=lambda kv: kv[1], reverse=True)[:3]
        top_str = ", ".join(f"{k}={v}" for k, v in top_cats)
        print(
            f"  {r.label}  ({r.address[:10]}…)  "
            f"total={r.total_failures}  [{top_str}]"
        )


def main() -> int:
    if len(sys.argv) < 2:
        print(
            "usage: python3 examples.py <amarillo-base-url>   "
            "(e.g. http://localhost:3000)",
            file=sys.stderr,
        )
        return 2
    base_url = sys.argv[1]
    api_key = os.environ.get("AMARILLO_ADMIN_API_KEY")
    if not api_key:
        print(
            "  note: AMARILLO_ADMIN_API_KEY not set — write/admin demo (#2) will be skipped.\n"
            "        Set it in your env to run that scenario (S16/M006)."
        )
    client = AmarilloClient(base_url, api_key=api_key)
    demo_single_diagnosis(client)
    if api_key:
        demo_alert_subscription(client)
    demo_by_label(client)
    return 0


if __name__ == "__main__":
    sys.exit(main())
