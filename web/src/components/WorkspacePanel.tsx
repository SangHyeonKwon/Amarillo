import { useMemo, useState } from "react";

import type { DailySwapVolume, SwapEvent } from "@/api/types";
import { formatCompact, formatDate, formatNumber, timeAgo } from "@/lib/format";

type PanelTab = "sql" | "logs" | "auth";

interface WorkspacePanelProps {
  apiBaseUrl: string;
  latestBlock?: number | null;
  poolCount: number;
  failedCount: number;
  previewRows: DailySwapVolume[];
  recentSwaps: SwapEvent[];
  isApiOnline: boolean;
}

const SQL_QUERY = `SELECT pair_name, swap_count, total_amount_in
FROM vw_daily_swap_volume
ORDER BY swap_date DESC
LIMIT 5;`;

/**
 * Supabase-style control panel with SQL/logs/auth tabs.
 */
export function WorkspacePanel({
  apiBaseUrl,
  latestBlock,
  poolCount,
  failedCount,
  previewRows,
  recentSwaps,
  isApiOnline,
}: WorkspacePanelProps) {
  const [tab, setTab] = useState<PanelTab>("sql");

  const logs = useMemo(() => {
    const base = [
      {
        level: isApiOnline ? "INFO" : "ERROR",
        ts: new Date().toISOString(),
        message: isApiOnline
          ? "API connection healthy"
          : "API connection is currently unreachable",
      },
      {
        level: "INFO",
        ts: new Date().toISOString(),
        message: `Indexed pools snapshot: ${poolCount}`,
      },
      {
        level: "WARN",
        ts: new Date().toISOString(),
        message: `Failed tx categories in view: ${failedCount}`,
      },
    ];

    const fromSwaps = recentSwaps.slice(0, 4).map((swap) => ({
      level: "DEBUG",
      ts: swap.timestamp,
      message: `Decoded swap tx ${swap.tx_hash.slice(0, 12)}...`,
    }));

    return [...base, ...fromSwaps];
  }, [failedCount, isApiOnline, poolCount, recentSwaps]);

  return (
    <section className="workspace-panel card">
      <div className="workspace-head">
        <div>
          <div className="card-title">Workspace</div>
          <div className="card-sub">SQL, logs, and access context</div>
        </div>
        <div className="tabs" role="tablist" aria-label="Workspace tabs">
          <button
            type="button"
            role="tab"
            aria-selected={tab === "sql"}
            className={tab === "sql" ? "tab active" : "tab"}
            onClick={() => setTab("sql")}
          >
            SQL
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === "logs"}
            className={tab === "logs" ? "tab active" : "tab"}
            onClick={() => setTab("logs")}
          >
            Logs
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === "auth"}
            className={tab === "auth" ? "tab active" : "tab"}
            onClick={() => setTab("auth")}
          >
            Auth
          </button>
        </div>
      </div>

      {tab === "sql" && (
        <div className="workspace-body">
          <div className="sql-editor-wrap">
            <div className="sql-editor-head">
              <span className="mono muted">Query runner (preview)</span>
              <button type="button" className="btn btn--brand">
                Run query
              </button>
            </div>
            <pre className="sql-editor" aria-label="SQL query preview">
              <code>
                <span className="sql-token-keyword">SELECT</span>{" "}
                <span className="sql-token-field">pair_name</span>,{" "}
                <span className="sql-token-field">swap_count</span>,{" "}
                <span className="sql-token-field">total_amount_in</span>
                {"\n"}
                <span className="sql-token-keyword">FROM</span>{" "}
                <span className="sql-token-table">vw_daily_swap_volume</span>
                {"\n"}
                <span className="sql-token-keyword">ORDER BY</span>{" "}
                <span className="sql-token-field">swap_date</span>{" "}
                <span className="sql-token-keyword">DESC</span>
                {"\n"}
                <span className="sql-token-keyword">LIMIT</span>{" "}
                <span className="sql-token-number">5</span>;
              </code>
            </pre>
          </div>

          <div className="sql-result">
            <div className="mono muted" style={{ marginBottom: 8 }}>
              Result preview
            </div>
            <table className="tbl sql-result-table">
              <thead>
                <tr>
                  <th>Date</th>
                  <th>Pair</th>
                  <th className="num">Swaps</th>
                  <th className="num">Total in</th>
                </tr>
              </thead>
              <tbody>
                {previewRows.slice(0, 5).map((row, idx) => (
                  <tr key={`${row.pool_address}-${row.swap_date}-${idx}`}>
                    <td>{formatDate(row.swap_date)}</td>
                    <td>{row.pair_name}</td>
                    <td className="num">{formatNumber(row.swap_count)}</td>
                    <td className="num">{formatCompact(row.total_amount_in)}</td>
                  </tr>
                ))}
                {previewRows.length === 0 && (
                  <tr>
                    <td colSpan={4} className="muted">
                      No preview rows.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {tab === "logs" && (
        <div className="workspace-body">
          <div className="log-list">
            {logs.map((entry, idx) => (
              <div key={`${entry.level}-${entry.ts}-${idx}`} className="log-row">
                <span className={`log-level log-level--${entry.level.toLowerCase()}`}>
                  {entry.level}
                </span>
                <span className="log-msg">{entry.message}</span>
                <span className="log-time mono">{timeAgo(entry.ts)}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {tab === "auth" && (
        <div className="workspace-body">
          <div className="auth-grid">
            <div className="auth-card">
              <div className="auth-label">Connection</div>
              <div className="auth-value">
                {isApiOnline ? "Connected" : "Disconnected"}
              </div>
              <div className="auth-note mono">{apiBaseUrl}</div>
            </div>
            <div className="auth-card">
              <div className="auth-label">Access mode</div>
              <div className="auth-value">Public read-only</div>
              <div className="auth-note">No session required for dashboards</div>
            </div>
            <div className="auth-card">
              <div className="auth-label">Indexed context</div>
              <div className="auth-value">
                Block {latestBlock != null ? formatNumber(latestBlock) : "-"}
              </div>
              <div className="auth-note">Use as baseline before auth rollout</div>
            </div>
          </div>
        </div>
      )}

      <details className="workspace-meta">
        <summary className="mono">Raw query text</summary>
        <pre className="mono muted">{SQL_QUERY}</pre>
      </details>
    </section>
  );
}
