import { type FormEvent, useState } from "react";

import { useApiKey } from "@/state/apiKey";

/**
 * Session-memory admin API key entry (S18 / M006 / D024).
 *
 * Two states:
 *
 * - **No key applied** — masked text input + "Apply" button. Submitting an
 *   empty or whitespace-only value is rejected client-side with a brief
 *   message; the {@link useApiKey} setter would clamp to `null` anyway, but
 *   surfacing the rejection inline is friendlier than a silent no-op.
 * - **Key applied** — the input collapses to a short status badge (length
 *   only, never the value) plus a "Clear" button. Clicking "Clear" wipes the
 *   key from both the React state and the `@/api/client` module slot
 *   (`apiKey.tsx` does the latter via `useEffect`).
 *
 * The component is **stateless across page reloads** by design (D024). No
 * `localStorage`, no `sessionStorage`, no URL param — refresh = re-enter. The
 * trade-off is friction for the operator; the gain is no persistent secret
 * surface accessible to XSS / DevTools / browser history.
 */
export function ApiKeyInput() {
  const { apiKey, setApiKey } = useApiKey();
  const [input, setInput] = useState("");
  const [error, setError] = useState<string | null>(null);

  function onApply(e: FormEvent) {
    e.preventDefault();
    const trimmed = input.trim();
    if (trimmed === "") {
      setError("API key must be non-empty.");
      return;
    }
    setError(null);
    setApiKey(trimmed);
    setInput("");
  }

  function onClear() {
    setApiKey(null);
    setError(null);
    setInput("");
  }

  return (
    <div className="card" style={{ marginBottom: 16 }}>
      <div className="card-head">
        <div>
          <div className="card-title">Admin API key</div>
          <div className="card-sub">
            Required for create / rotate / deactivate (S16/M006). GET endpoints
            stay public. Stored in <strong>session memory only</strong> — refresh
            clears it on purpose (D024). See{" "}
            <a
              href="https://github.com/SangHyeonKwon/defi-tx-indexer/blob/main/docs/api-failed-tx.md#authentication"
              target="_blank"
              rel="noreferrer"
            >
              docs/api-failed-tx.md#Authentication
            </a>
            .
          </div>
        </div>
      </div>
      {apiKey == null ? (
        <form
          onSubmit={onApply}
          className="toolbar"
          style={{ gap: 8, marginTop: 8, flexWrap: "wrap" }}
        >
          <input
            type="password"
            placeholder="Paste admin API key (32+ bytes recommended)"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            autoComplete="off"
            spellCheck={false}
            style={{ flex: "1 1 320px", minWidth: 240 }}
            aria-label="Admin API key"
          />
          <button className="btn" type="submit" disabled={input.trim() === ""}>
            Apply
          </button>
        </form>
      ) : (
        <div
          className="toolbar"
          style={{ gap: 12, marginTop: 8, alignItems: "center", flexWrap: "wrap" }}
        >
          <span
            className="badge"
            style={{ color: "#3ECF8E", borderColor: "#3ECF8E" }}
            aria-live="polite"
          >
            Key active ({apiKey.length} chars)
          </span>
          <span className="muted" style={{ fontSize: 12 }}>
            Write buttons enabled. Refresh the page to drop the key.
          </span>
          <button className="btn" type="button" onClick={onClear}>
            Clear
          </button>
        </div>
      )}
      {error && (
        <div
          role="alert"
          style={{
            marginTop: 10,
            color: "#F66061",
            background: "rgba(246,96,97,0.08)",
            border: "1px solid rgba(246,96,97,0.4)",
            padding: 10,
            borderRadius: 6,
            fontSize: 13,
          }}
        >
          {error}
        </div>
      )}
    </div>
  );
}
