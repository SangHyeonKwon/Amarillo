import { type FormEvent, useEffect, useState } from "react";

import { ApiError } from "@/api/client";
import {
  useAlertSubscriptions,
  useCreateAlertSubscription,
  useDeactivateAlertSubscription,
  useRotateAlertSubscription,
} from "@/api/hooks";
import {
  type AlertSubscription,
  type AlertSubscriptionCreated,
  type AlertSubType,
  type CreateAlertSubscriptionBody,
  ERROR_CATEGORIES,
  type ErrorCategory,
} from "@/api/types";
import { AsyncState } from "@/components/AsyncState";
import { type Column, DataTable } from "@/components/DataTable";
import { errorCategoryColor, errorCategoryLabel, timeAgo } from "@/lib/format";

const ADDR_RE = /^0x[0-9a-fA-F]{40}$/;

function truncMid(s: string, head: number, tail: number): string {
  return s.length <= head + tail + 1 ? s : `${s.slice(0, head)}…${s.slice(-tail)}`;
}

type FormErrorOk = { ok: true; body: CreateAlertSubscriptionBody };
type FormErrorBad = { ok: false; error: string };

/**
 * `/alerts` — alert subscription lifecycle (S08 + HARDEN2 API surface).
 *
 * Security척추: `signing_secret`은 백엔드가 생성·회전 응답에서 *딱 1회* 노출
 * 한다. 이 페이지는 그 시점에 모달로 평문 표시 + 복사 + 닫으면 즉시 React state
 * 에서 폐기하고 mutation cache까지 `reset()`한다. 절대 URL/쿼리캐시/로그에 안 남김.
 */
export function Alerts() {
  const list = useAlertSubscriptions({ limit: 200 });
  const create = useCreateAlertSubscription();
  const rotate = useRotateAlertSubscription();
  const deactivate = useDeactivateAlertSubscription();

  // ── form state ─────────────────────────────────────────────
  const [webhookUrl, setWebhookUrl] = useState("");
  const [category, setCategory] = useState<ErrorCategory | "ALL">("ALL");
  const [toAddr, setToAddr] = useState("");
  // S14 / M005 — rate-threshold extension. Empty strings while in per_event mode.
  const [subType, setSubType] = useState<AlertSubType>("per_event");
  const [thresholdCount, setThresholdCount] = useState("");
  const [thresholdWindowSecs, setThresholdWindowSecs] = useState("");
  const [debounceSecs, setDebounceSecs] = useState("");
  const [formError, setFormError] = useState<string | null>(null);

  // ── one-time secret reveal (lives only here, never persisted) ──
  const [revealed, setRevealed] = useState<AlertSubscriptionCreated | null>(null);
  const [copyStatus, setCopyStatus] = useState<"idle" | "ok" | "err">("idle");

  function closeRevealedModal() {
    setRevealed(null);
    setCopyStatus("idle");
    // 메모리상 mutation 결과도 같이 제거 — 시크릿이 react-query 캐시에 남지 않게.
    create.reset();
    rotate.reset();
  }

  // ESC 닫기
  useEffect(() => {
    if (!revealed) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") closeRevealedModal();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [revealed]);

  function validate(): FormErrorOk | FormErrorBad {
    const url = webhookUrl.trim();
    if (!url) {
      return { ok: false, error: "webhook_url is required." };
    }
    if (!url.toLowerCase().startsWith("https://")) {
      return { ok: false, error: "webhook_url must start with https:// (server-rejected otherwise)." };
    }
    const lower = toAddr.trim().toLowerCase();
    if (lower !== "" && !ADDR_RE.test(lower)) {
      return { ok: false, error: "to_addr must be 0x + 40 hex characters (or empty)." };
    }
    const body: CreateAlertSubscriptionBody = {
      webhook_url: url,
      ...(category !== "ALL" ? { error_category: category } : {}),
      ...(lower ? { to_addr: lower } : {}),
    };
    if (subType === "rate_threshold") {
      const tc = Number.parseInt(thresholdCount, 10);
      const tw = Number.parseInt(thresholdWindowSecs, 10);
      const db = Number.parseInt(debounceSecs, 10);
      if (!Number.isInteger(tc) || tc <= 0) {
        return { ok: false, error: "threshold_count must be a positive integer." };
      }
      if (!Number.isInteger(tw) || tw <= 0) {
        return { ok: false, error: "threshold_window_secs must be a positive integer." };
      }
      if (!Number.isInteger(db) || db < 0) {
        return { ok: false, error: "debounce_secs must be a non-negative integer." };
      }
      body.sub_type = "rate_threshold";
      body.threshold_count = tc;
      body.threshold_window_secs = tw;
      body.debounce_secs = db;
    }
    return { ok: true, body };
  }

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    setFormError(null);
    const v = validate();
    if (!v.ok) {
      setFormError(v.error);
      return;
    }
    try {
      const resp = await create.mutateAsync(v.body);
      setRevealed(resp.data);
      setWebhookUrl("");
      setCategory("ALL");
      setToAddr("");
      setSubType("per_event");
      setThresholdCount("");
      setThresholdWindowSecs("");
      setDebounceSecs("");
    } catch (err) {
      setFormError(err instanceof ApiError ? err.message : String(err));
    }
  }

  async function onRotate(id: number) {
    if (
      !window.confirm(
        `Rotate signing secret for subscription #${id}? The current secret will stop working immediately.`,
      )
    )
      return;
    try {
      const resp = await rotate.mutateAsync(id);
      setRevealed(resp.data);
    } catch (err) {
      window.alert(err instanceof ApiError ? err.message : String(err));
    }
  }

  async function onDeactivate(id: number) {
    if (
      !window.confirm(
        `Deactivate subscription #${id}? Soft-delete only — the alert_delivery history is preserved.`,
      )
    )
      return;
    try {
      await deactivate.mutateAsync(id);
    } catch (err) {
      window.alert(err instanceof ApiError ? err.message : String(err));
    }
  }

  async function copySecret() {
    if (!revealed) return;
    if (!navigator.clipboard) {
      setCopyStatus("err");
      return;
    }
    try {
      await navigator.clipboard.writeText(revealed.signing_secret);
      setCopyStatus("ok");
    } catch {
      setCopyStatus("err");
    }
  }

  const subs = list.data ?? [];
  const active = subs.filter((s) => s.active).length;
  const inactive = subs.length - active;

  const columns: Column<AlertSubscription>[] = [
    {
      header: "ID",
      cell: (s) => <span className="mono">#{s.subscription_id}</span>,
    },
    {
      header: "Status",
      cell: (s) => (
        <span
          className="badge"
          style={{
            color: s.active ? "#3ECF8E" : "#888",
            borderColor: s.active ? "#3ECF8E" : "#444",
          }}
        >
          {s.active ? "Active" : "Inactive"}
        </span>
      ),
    },
    {
      header: "Category",
      cell: (s) =>
        s.error_category ? (
          <span
            className="badge"
            style={{
              color: errorCategoryColor(s.error_category),
              borderColor: errorCategoryColor(s.error_category),
            }}
          >
            {errorCategoryLabel(s.error_category)}
          </span>
        ) : (
          <span className="muted">Any</span>
        ),
    },
    {
      header: "To address",
      cell: (s) =>
        s.to_addr ? (
          <span className="mono" title={s.to_addr}>
            {truncMid(s.to_addr, 6, 6)}
          </span>
        ) : (
          <span className="muted">Any</span>
        ),
    },
    {
      header: "Mode",
      cell: (s) =>
        s.sub_type === "rate_threshold" ? (
          <span style={{ display: "inline-block", textAlign: "left" }}>
            <span
              className="badge"
              style={{
                color: "#3ECF8E",
                borderColor: "#3ECF8E",
                fontSize: 11,
              }}
            >
              Rate ≥ {s.threshold_count} / {s.threshold_window_secs}s
            </span>
            <div className="muted" style={{ fontSize: 11, marginTop: 2 }}>
              debounce {s.debounce_secs}s
            </div>
          </span>
        ) : (
          <span className="muted">Per event</span>
        ),
    },
    {
      header: "Webhook",
      cell: (s) => (
        <span className="mono" title={s.webhook_url}>
          {truncMid(s.webhook_url, 28, 6)}
        </span>
      ),
    },
    {
      header: "Created",
      align: "right",
      cell: (s) => (
        <span className="muted" title={s.created_at}>
          {timeAgo(s.created_at)}
        </span>
      ),
    },
    {
      header: "Actions",
      align: "right",
      cell: (s) => (
        <span className="toolbar">
          <button
            className="btn"
            type="button"
            disabled={!s.active || rotate.isPending}
            onClick={() => onRotate(s.subscription_id)}
          >
            Rotate
          </button>
          <button
            className="btn"
            type="button"
            disabled={!s.active || deactivate.isPending}
            onClick={() => onDeactivate(s.subscription_id)}
          >
            Deactivate
          </button>
        </span>
      ),
    },
  ];

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Alert subscriptions</h1>
          <p>
            Failure-pattern webhooks (S08 + HARDEN2) — POST{" "}
            <span className="mono">/v1/alert-subscriptions</span>, rotation, soft-deactivate.
          </p>
        </div>
        <div className="toolbar">
          <span className="muted">
            {active} active · {inactive} inactive
          </span>
        </div>
      </div>

      <div className="card">
        <div className="card-head">
          <div className="card-title">Create subscription</div>
          <div className="card-sub">
            Server enforces HTTPS-only and SSRF guard — invalid URLs return 400 with
            the reason. Save the signing secret immediately when shown.
          </div>
        </div>
        <form onSubmit={onSubmit} className="grid" style={{ gap: 12 }}>
          <div className="field">
            <label htmlFor="webhook-url">Webhook URL *</label>
            <input
              id="webhook-url"
              type="url"
              required
              placeholder="https://example.com/alerts"
              value={webhookUrl}
              onChange={(e) => setWebhookUrl(e.target.value)}
              maxLength={2048}
            />
          </div>
          <div className="field">
            <label htmlFor="category">Error category</label>
            <select
              id="category"
              value={category}
              onChange={(e) => setCategory(e.target.value as ErrorCategory | "ALL")}
            >
              <option value="ALL">Any category</option>
              {ERROR_CATEGORIES.map((c) => (
                <option key={c} value={c}>
                  {errorCategoryLabel(c)}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label htmlFor="to-addr">to_addr (optional)</label>
            <input
              id="to-addr"
              type="text"
              placeholder="0x… (40 hex; leave blank for any)"
              value={toAddr}
              onChange={(e) => setToAddr(e.target.value)}
              maxLength={42}
            />
          </div>
          <div className="field">
            <label>Subscription mode (S14)</label>
            <div className="toolbar" style={{ gap: 16, flexWrap: "wrap" }}>
              <label style={{ display: "inline-flex", gap: 6 }}>
                <input
                  type="radio"
                  name="sub_type"
                  value="per_event"
                  checked={subType === "per_event"}
                  onChange={() => setSubType("per_event")}
                />
                Per event (1 match = 1 webhook)
              </label>
              <label style={{ display: "inline-flex", gap: 6 }}>
                <input
                  type="radio"
                  name="sub_type"
                  value="rate_threshold"
                  checked={subType === "rate_threshold"}
                  onChange={() => setSubType("rate_threshold")}
                />
                Rate threshold (count ≥ N in window, then debounce)
              </label>
            </div>
          </div>
          {subType === "rate_threshold" && (
            <div
              className="grid"
              style={{
                gap: 8,
                gridTemplateColumns: "repeat(3, minmax(0, 1fr))",
                borderLeft: "3px solid #3ECF8E",
                paddingLeft: 12,
              }}
            >
              <div className="field">
                <label htmlFor="threshold-count">threshold_count *</label>
                <input
                  id="threshold-count"
                  type="number"
                  min={1}
                  placeholder="e.g. 10"
                  value={thresholdCount}
                  onChange={(e) => setThresholdCount(e.target.value)}
                />
              </div>
              <div className="field">
                <label htmlFor="threshold-window">window_secs *</label>
                <input
                  id="threshold-window"
                  type="number"
                  min={1}
                  placeholder="e.g. 300"
                  value={thresholdWindowSecs}
                  onChange={(e) => setThresholdWindowSecs(e.target.value)}
                />
              </div>
              <div className="field">
                <label htmlFor="debounce">debounce_secs *</label>
                <input
                  id="debounce"
                  type="number"
                  min={0}
                  placeholder="e.g. 600"
                  value={debounceSecs}
                  onChange={(e) => setDebounceSecs(e.target.value)}
                />
              </div>
            </div>
          )}
          {formError && (
            <div
              role="alert"
              style={{
                color: "#F66061",
                background: "rgba(246,96,97,0.08)",
                border: "1px solid rgba(246,96,97,0.4)",
                padding: 10,
                borderRadius: 6,
                fontSize: 13,
              }}
            >
              {formError}
            </div>
          )}
          <div>
            <button className="btn" type="submit" disabled={create.isPending}>
              {create.isPending ? "Creating…" : "Create subscription"}
            </button>
          </div>
        </form>
      </div>

      <div className="card">
        <div className="card-head">
          <div className="card-title">Subscriptions</div>
          <div className="card-sub">
            Active and inactive (newest first). Rotate or deactivate per row.
          </div>
        </div>
        <AsyncState
          isLoading={list.isLoading}
          isError={list.isError}
          error={list.error}
          isEmpty={subs.length === 0}
          emptyLabel="No alert subscriptions yet. Create one above."
        >
          <DataTable
            columns={columns}
            rows={subs}
            rowKey={(s) => s.subscription_id}
            caption="Alert subscriptions"
          />
        </AsyncState>
      </div>

      {revealed && (
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="secret-modal-title"
          style={{
            position: "fixed",
            inset: 0,
            background: "rgba(0,0,0,0.6)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            zIndex: 1000,
          }}
          onClick={(e) => {
            if (e.target === e.currentTarget) closeRevealedModal();
          }}
        >
          <div
            className="card"
            style={{ maxWidth: 640, width: "100%", margin: 16 }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="card-head">
              <div>
                <div className="card-title" id="secret-modal-title">
                  Signing secret — shown only this time
                </div>
                <div className="card-sub">
                  Subscription #{revealed.subscription_id} · save this value now; the
                  server cannot reveal it again.
                </div>
              </div>
            </div>
            <div
              style={{
                marginTop: 8,
                padding: 12,
                background: "rgba(255,255,255,0.04)",
                border: "1px solid rgba(255,255,255,0.08)",
                borderRadius: 6,
              }}
            >
              <code
                className="mono"
                style={{
                  wordBreak: "break-all",
                  display: "block",
                  fontSize: 13,
                  lineHeight: 1.6,
                }}
              >
                {revealed.signing_secret}
              </code>
            </div>
            <div
              className="spread"
              style={{ marginTop: 12, alignItems: "center", gap: 8 }}
            >
              <span className="muted" style={{ fontSize: 12 }}>
                webhook:{" "}
                <span className="mono" title={revealed.webhook_url}>
                  {truncMid(revealed.webhook_url, 32, 8)}
                </span>
              </span>
              <span className="toolbar">
                <button className="btn" type="button" onClick={copySecret}>
                  {copyStatus === "ok"
                    ? "Copied ✓"
                    : copyStatus === "err"
                      ? "Copy failed (select & copy manually)"
                      : "Copy"}
                </button>
                <button className="btn" type="button" onClick={closeRevealedModal}>
                  Close
                </button>
              </span>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
