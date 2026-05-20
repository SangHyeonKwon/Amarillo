import { useMemo } from "react";
import { Link, useSearchParams } from "react-router-dom";
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import {
  useFailedTxAnalysis,
  useFailedTxByLabel,
  useFailedTxDetail,
  useFailedTxList,
  useFailedTxTimeseries,
} from "@/api/hooks";
import {
  ERROR_CATEGORIES,
  type ErrorCategory,
  type FailedTransaction,
  type FailedTxAnalysis,
  type FailedTxByLabelPoint,
} from "@/api/types";
import { AsyncState } from "@/components/AsyncState";
import { type Column, DataTable } from "@/components/DataTable";
import { KpiCard } from "@/components/KpiCard";
import {
  axisLine,
  axisTick,
  numeric,
  tooltipContentStyle,
  tooltipItemStyle,
  tooltipLabelStyle,
} from "@/lib/chart";
import {
  errorCategoryColor,
  errorCategoryLabel,
  formatCompact,
  formatDate,
  formatDateTime,
  formatNumber,
  formatPct,
  timeAgo,
  toNumber,
} from "@/lib/format";
import { parseEnumFilter, parseIntFilter, useUrlFilters } from "@/lib/urlFilters";

const CATEGORY_FILTERS = ["ALL", ...ERROR_CATEGORIES] as const;
const SORT_FILTERS = ["failures", "share", "recent"] as const;
const WINDOW_FILTERS = ["7", "30", "90", "365", "all"] as const;

/** Per-cycle page size for the failed-tx list (S04 hardening: receiver-capped). */
const LIST_LIMIT = 20;

const columns: Column<FailedTxAnalysis>[] = [
  {
    header: "Error category",
    cell: (f) => (
      <span
        className="badge"
        style={{
          color: errorCategoryColor(f.error_category),
          borderColor: errorCategoryColor(f.error_category),
        }}
      >
        {errorCategoryLabel(f.error_category)}
      </span>
    ),
  },
  {
    header: "Failures",
    align: "right",
    cell: (f) => formatNumber(f.failure_count),
  },
  {
    header: "% of total",
    align: "right",
    cell: (f) => formatPct(f.pct_of_total),
  },
  {
    header: "Avg gas wasted",
    align: "right",
    cell: (f) => formatCompact(f.avg_gas_wasted),
  },
  {
    header: "Most recent",
    align: "right",
    cell: (f) => (
      <span className="muted" title={f.most_recent_failure}>
        {timeAgo(f.most_recent_failure)}
      </span>
    ),
  },
];

export function FailedTx() {
  const [searchParams] = useSearchParams();
  const [filters, setFilters] = useUrlFilters({
    category: "ALL",
    sort: "failures",
    window: "90",
    offset: "0",
    tx: "",
  });
  const selectedCategory = parseEnumFilter(
    filters.category,
    CATEGORY_FILTERS,
    "ALL",
  );
  const sortBy = parseEnumFilter(filters.sort, SORT_FILTERS, "failures");
  const windowFilter = parseEnumFilter(filters.window, WINDOW_FILTERS, "90");
  const windowDays = windowFilter === "all" ? null : parseIntFilter(windowFilter, 90, 1, 3650);
  const offset = parseIntFilter(filters.offset, 0, 0, 1_000_000);
  const selectedTx = filters.tx.trim().toLowerCase();

  const query = useFailedTxAnalysis();
  const data = query.data ?? [];

  // ── Real timeseries + filtered list + drill-down (FE-WIRE-T02) ──
  // Convert the lookback window to an RFC3339 range the new endpoints accept.
  // Empty `from`/`to` means "no bound" on the server.
  const { fromIso, toIso } = useMemo(() => {
    if (windowDays == null) return { fromIso: undefined, toIso: undefined };
    const now = Date.now();
    return {
      fromIso: new Date(now - windowDays * 24 * 60 * 60 * 1000).toISOString(),
      toIso: new Date(now).toISOString(),
    };
  }, [windowDays]);
  // Buckets follow the lookback: hour for ≤7d, day for ≤90d, week beyond.
  const interval = windowDays == null || windowDays > 90
    ? "week"
    : windowDays <= 7
      ? "hour"
      : "day";

  const categoryParam = selectedCategory === "ALL" ? undefined : selectedCategory;
  const listQuery = useFailedTxList({
    category: categoryParam,
    from: fromIso,
    to: toIso,
    limit: LIST_LIMIT,
    offset,
  });
  const listData = listQuery.data?.data ?? [];
  const listTotal = listQuery.data?.pagination.total ?? 0;
  const listCount = listQuery.data?.pagination.count ?? 0;

  const trendQuery = useFailedTxTimeseries({ interval, from: fromIso, to: toIso });
  const trendPoints = trendQuery.data ?? [];

  const byLabelQuery = useFailedTxByLabel({ from: fromIso, to: toIso, limit: 20 });
  const byLabelData = byLabelQuery.data ?? [];

  const detailQuery = useFailedTxDetail(selectedTx || undefined);
  const detail = detailQuery.data;

  // Pivot timeseries → wide rows for stacked area chart (one column per category).
  const trendChart = useMemo(() => {
    if (trendPoints.length === 0) return { rows: [], categories: [] as ErrorCategory[] };
    const buckets = new Set<string>();
    const cats = new Set<ErrorCategory>();
    for (const p of trendPoints) {
      buckets.add(p.bucket);
      cats.add(p.error_category);
    }
    const orderedBuckets = [...buckets].sort();
    const orderedCats = [...cats];
    const rows = orderedBuckets.map((bucket) => {
      const row: Record<string, string | number> = { bucket };
      for (const c of orderedCats) {
        row[c] = 0;
      }
      return row;
    });
    const rowByBucket = new Map<string, Record<string, string | number>>();
    rows.forEach((r) => rowByBucket.set(String(r.bucket), r));
    for (const p of trendPoints) {
      const r = rowByBucket.get(p.bucket);
      if (r) r[p.error_category] = p.failure_count;
    }
    return { rows, categories: orderedCats };
  }, [trendPoints]);

  const source = searchParams.get("source");
  const sourcePool = searchParams.get("pool");

  const byLabelColumns: Column<FailedTxByLabelPoint>[] = [
    {
      header: "Label",
      cell: (r) => <span>{r.label}</span>,
    },
    {
      header: "Address",
      cell: (r) => (
        <span className="mono" title={r.address}>
          {r.address.slice(0, 10)}…{r.address.slice(-6)}
        </span>
      ),
    },
    {
      header: "Total failures",
      align: "right",
      cell: (r) => formatCompact(r.total_failures),
    },
    {
      header: "Distribution",
      cell: (r) => (
        <span
          className="toolbar"
          style={{ flexWrap: "wrap", gap: 4, justifyContent: "flex-start" }}
        >
          {Object.entries(r.by_category)
            .sort(([, a], [, b]) => b - a)
            .slice(0, 4)
            .map(([cat, count]) => (
              <span
                key={cat}
                className="badge"
                style={{
                  color: errorCategoryColor(cat as ErrorCategory),
                  borderColor: errorCategoryColor(cat as ErrorCategory),
                  fontSize: 11,
                }}
              >
                {errorCategoryLabel(cat as ErrorCategory)} {count}
              </span>
            ))}
        </span>
      ),
    },
  ];

  const listColumns: Column<FailedTransaction>[] = [
    {
      header: "Tx hash",
      cell: (f) => (
        <span className="mono" title={f.tx_hash}>
          {f.tx_hash.slice(0, 10)}…{f.tx_hash.slice(-6)}
        </span>
      ),
    },
    {
      header: "Category",
      cell: (f) => (
        <span
          className="badge"
          style={{
            color: errorCategoryColor(f.error_category),
            borderColor: errorCategoryColor(f.error_category),
          }}
        >
          {errorCategoryLabel(f.error_category)}
        </span>
      ),
    },
    {
      header: "Revert reason",
      cell: (f) => (
        <span className="muted" title={f.revert_reason ?? "—"}>
          {f.revert_reason ?? "—"}
        </span>
      ),
    },
    {
      header: "Gas",
      align: "right",
      cell: (f) => formatCompact(f.gas_used),
    },
    {
      header: "When",
      align: "right",
      cell: (f) => (
        <span className="muted" title={f.timestamp}>
          {timeAgo(f.timestamp)}
        </span>
      ),
    },
  ];

  const { scoped, chart, total, topCategory, mostRecentLabel } = useMemo(() => {
    const cutoffMs =
      windowDays == null ? null : Date.now() - windowDays * 24 * 60 * 60 * 1000;

    const scopedRows = data.filter((row) => {
      if (selectedCategory !== "ALL" && row.error_category !== selectedCategory) {
        return false;
      }
      if (cutoffMs == null) return true;
      return new Date(row.most_recent_failure).getTime() >= cutoffMs;
    });

    const sorted = [...scopedRows];
    switch (sortBy) {
      case "share":
        sorted.sort((a, b) => toNumber(b.pct_of_total) - toNumber(a.pct_of_total));
        break;
      case "recent":
        sorted.sort(
          (a, b) =>
            new Date(b.most_recent_failure).getTime() -
            new Date(a.most_recent_failure).getTime(),
        );
        break;
      case "failures":
      default:
        sorted.sort((a, b) => b.failure_count - a.failure_count);
        break;
    }

    const mostRecent = sorted[0]?.most_recent_failure;
    return {
      chart: sorted.map((f) => ({
        key: f.error_category,
        name: errorCategoryLabel(f.error_category),
        count: f.failure_count,
        pct: toNumber(f.pct_of_total),
        color: errorCategoryColor(f.error_category),
      })),
      total: sorted.reduce((acc, f) => acc + f.failure_count, 0),
      scoped: sorted,
      topCategory: sorted[0]
        ? errorCategoryLabel(sorted[0].error_category)
        : "—",
      mostRecentLabel: mostRecent ? timeAgo(mostRecent) : "—",
    };
  }, [data, selectedCategory, sortBy, windowDays]);

  const sourceHint =
    source === "pool" && sourcePool
      ? `Focused from pool ${sourcePool}`
      : source === "traders"
        ? "Focused from trader leaderboard"
        : null;

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Failed transaction analysis</h1>
          <p>
            Revert reasons decoded from <span className="mono">debug_traceTransaction</span> —
            classified into error categories
          </p>
          {sourceHint && <p className="muted">{sourceHint}</p>}
        </div>
        <div className="toolbar">
          <div className="field">
            <label htmlFor="category">Category</label>
            <select
              id="category"
              value={selectedCategory}
              onChange={(e) => setFilters({ category: e.target.value, offset: "0" })}
            >
              <option value="ALL">All categories</option>
              {ERROR_CATEGORIES.map((category) => (
                <option key={category} value={category}>
                  {errorCategoryLabel(category)}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label htmlFor="sort">Sort</label>
            <select
              id="sort"
              value={sortBy}
              onChange={(e) => setFilters({ sort: e.target.value })}
            >
              <option value="failures">Failures</option>
              <option value="share">Share (%)</option>
              <option value="recent">Most recent</option>
            </select>
          </div>
          <div className="field">
            <label htmlFor="window">Lookback window</label>
            <select
              id="window"
              value={windowFilter}
              onChange={(e) => setFilters({ window: e.target.value, offset: "0" })}
            >
              <option value="7">Last 7 days</option>
              <option value="30">Last 30 days</option>
              <option value="90">Last 90 days</option>
              <option value="365">Last 365 days</option>
              <option value="all">All</option>
            </select>
          </div>
        </div>
      </div>

      <AsyncState
        isLoading={query.isLoading}
        isError={query.isError}
        error={query.error}
        isEmpty={scoped.length === 0}
        emptyLabel="No failed transactions analyzed yet."
      >
        <div className="grid kpi-grid">
          <KpiCard
            label="Total failures"
            icon="⚠"
            value={formatNumber(total)}
          />
          <KpiCard
            label="Error categories"
            icon="▤"
            value={scoped.length}
          />
          <KpiCard
            label="Top reason"
            icon="▲"
            value={<span style={{ fontSize: 20 }}>{topCategory}</span>}
          />
          <KpiCard
            label="Most recent signal"
            icon="◷"
            value={mostRecentLabel}
          />
        </div>

        <div
          className="grid"
          style={{
            gridTemplateColumns: "repeat(auto-fit, minmax(340px, 1fr))",
          }}
        >
          <div className="card">
            <div className="card-head">
              <div className="card-title">Failure share</div>
              <div className="card-sub">By error category</div>
            </div>
            <div className="chart-box">
              <ResponsiveContainer width="100%" height="100%">
                <PieChart>
                  <Pie
                    data={chart}
                    dataKey="count"
                    nameKey="name"
                    innerRadius={62}
                    outerRadius={98}
                    paddingAngle={2}
                    stroke="none"
                  >
                    {chart.map((d) => (
                      <Cell key={d.key} fill={d.color} />
                    ))}
                  </Pie>
                  <Tooltip
                    contentStyle={tooltipContentStyle}
                    itemStyle={tooltipItemStyle}
                    labelStyle={tooltipLabelStyle}
                    formatter={(v) => [formatNumber(numeric(v)), "Failures"]}
                  />
                </PieChart>
              </ResponsiveContainer>
            </div>
            <div className="legend">
              {chart.map((d) => (
                <span key={d.key} className="legend-item">
                  <span
                    className="legend-swatch"
                    style={{ background: d.color }}
                  />
                  {d.name}
                </span>
              ))}
            </div>
          </div>

          <div className="card">
            <div className="card-head">
              <div className="card-title">Share of total</div>
              <div className="card-sub">Percent per category</div>
            </div>
            <div className="chart-box">
              <ResponsiveContainer width="100%" height="100%">
                <BarChart
                  layout="vertical"
                  data={chart}
                  margin={{ top: 4, right: 16, bottom: 4, left: 8 }}
                >
                  <XAxis
                    type="number"
                    tick={axisTick}
                    axisLine={axisLine}
                    tickLine={false}
                    tickFormatter={(v) => `${numeric(v)}%`}
                  />
                  <YAxis
                    type="category"
                    dataKey="name"
                    tick={axisTick}
                    axisLine={false}
                    tickLine={false}
                    width={140}
                  />
                  <Tooltip
                    cursor={{ fill: "rgba(62,207,142,0.12)" }}
                    contentStyle={tooltipContentStyle}
                    itemStyle={tooltipItemStyle}
                    labelStyle={tooltipLabelStyle}
                    formatter={(v) => [`${numeric(v).toFixed(2)}%`, "Share"]}
                  />
                  <Bar dataKey="pct" radius={[0, 3, 3, 0]}>
                    {chart.map((d) => (
                      <Cell key={d.key} fill={d.color} />
                    ))}
                  </Bar>
                </BarChart>
              </ResponsiveContainer>
            </div>
          </div>
        </div>

        <div className="card">
          <div className="card-head">
            <div className="card-title">Failure trend</div>
            <div className="card-sub">
              Real timeseries (`/v1/analytics/failed-tx/timeseries`) — bucketed by{" "}
              <span className="mono">{interval}</span>, stacked by category
            </div>
          </div>
          <AsyncState
            isLoading={trendQuery.isLoading}
            isError={trendQuery.isError}
            error={trendQuery.error}
            isEmpty={trendChart.rows.length === 0}
            emptyLabel="No failures in the selected window."
          >
            <div className="chart-box">
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart
                  data={trendChart.rows}
                  margin={{ top: 8, right: 12, bottom: 0, left: 0 }}
                >
                  <CartesianGrid stroke="rgba(51,51,51,0.8)" vertical={false} />
                  <XAxis
                    dataKey="bucket"
                    tick={axisTick}
                    axisLine={axisLine}
                    tickLine={false}
                    tickFormatter={(v) => formatDate(String(v))}
                    minTickGap={26}
                  />
                  <YAxis
                    tick={axisTick}
                    axisLine={false}
                    tickLine={false}
                    width={48}
                    tickFormatter={(v) => formatCompact(numeric(v))}
                  />
                  <Tooltip
                    contentStyle={tooltipContentStyle}
                    itemStyle={tooltipItemStyle}
                    labelStyle={tooltipLabelStyle}
                    labelFormatter={(l) => formatDateTime(String(l))}
                    formatter={(v, key) => [
                      formatNumber(numeric(v)),
                      errorCategoryLabel(String(key) as ErrorCategory),
                    ]}
                  />
                  {trendChart.categories.map((cat) => (
                    <Area
                      key={cat}
                      type="monotone"
                      dataKey={cat}
                      stackId="categories"
                      stroke={errorCategoryColor(cat)}
                      fill={errorCategoryColor(cat)}
                      fillOpacity={0.55}
                    />
                  ))}
                </AreaChart>
              </ResponsiveContainer>
            </div>
            <div className="legend">
              {trendChart.categories.map((cat) => (
                <span key={cat} className="legend-item">
                  <span
                    className="legend-swatch"
                    style={{ background: errorCategoryColor(cat) }}
                  />
                  {errorCategoryLabel(cat)}
                </span>
              ))}
            </div>
          </AsyncState>
        </div>

        <div className="card">
          <div className="card-head">
            <div className="card-title">Breakdown</div>
            <div className="card-sub">
              The data Dune can't show — failure modes, not just successful swaps
            </div>
          </div>
          <DataTable
            columns={columns}
            rows={scoped}
            rowKey={(f) => f.error_category}
            caption="Failed transaction category breakdown"
            onRowClick={(row) => setFilters({ category: row.error_category, offset: "0" })}
          />
        </div>

        <div className="card">
          <div className="card-head">
            <div className="card-title">Failures by labeled contract</div>
            <div className="card-sub">
              `/v1/analytics/failed-tx/by-label` (S09 / M003) — joins live
              failures against our private `contract_label` store. Dune can't
              see the right side of this join.
            </div>
          </div>
          <AsyncState
            isLoading={byLabelQuery.isLoading}
            isError={byLabelQuery.isError}
            error={byLabelQuery.error}
            isEmpty={byLabelData.length === 0}
            emptyLabel="No label-joinable failures in this window (seed labels exist but no matching failed_transaction · transaction.to_addr pairs)."
          >
            <DataTable
              columns={byLabelColumns}
              rows={byLabelData}
              rowKey={(r) => r.address}
              caption="Failures by labeled contract"
            />
          </AsyncState>
        </div>

        <div className="card">
          <div className="card-head">
            <div className="card-title">Browse failed transactions</div>
            <div className="card-sub">
              `/v1/failed-tx` — filter inherits the toolbar (category + lookback).
              Row click loads the decoded revert + call_tree below.
            </div>
          </div>
          <AsyncState
            isLoading={listQuery.isLoading}
            isError={listQuery.isError}
            error={listQuery.error}
            isEmpty={listData.length === 0}
            emptyLabel="No failed transactions in this filter."
          >
            <DataTable
              columns={listColumns}
              rows={listData}
              rowKey={(f) => f.tx_hash}
              caption="Failed transactions (paginated, with accurate total)"
              onRowClick={(row) => setFilters({ tx: row.tx_hash })}
            />
            <div
              className="spread"
              style={{ marginTop: 12, alignItems: "center" }}
            >
              <span className="muted">
                Showing {listCount === 0 ? 0 : offset + 1}–{offset + listCount} of{" "}
                {formatNumber(listTotal)}
              </span>
              <span className="toolbar">
                <button
                  className="btn"
                  type="button"
                  disabled={offset === 0}
                  onClick={() =>
                    setFilters({ offset: String(Math.max(offset - LIST_LIMIT, 0)) })
                  }
                >
                  Prev
                </button>
                <button
                  className="btn"
                  type="button"
                  disabled={offset + listCount >= listTotal}
                  onClick={() => setFilters({ offset: String(offset + LIST_LIMIT) })}
                >
                  Next
                </button>
              </span>
            </div>
          </AsyncState>
        </div>

        {selectedTx && (
          <div className="card">
            <div className="card-head">
              <div className="card-title">Tx inspection</div>
              <div className="card-sub">
                <span className="mono">{selectedTx}</span> · `/v1/failed-tx/{`{tx_hash}`}`
              </div>
              <button
                className="btn"
                type="button"
                onClick={() => setFilters({ tx: "" })}
              >
                Close
              </button>
            </div>
            <AsyncState
              isLoading={detailQuery.isLoading}
              isError={detailQuery.isError}
              error={detailQuery.error}
              isEmpty={!detail}
              emptyLabel="No matching failed transaction (404)."
            >
              {detail && (
                <>
                  <div className="grid kpi-grid">
                    <KpiCard
                      label="Category"
                      icon="▤"
                      value={
                        <span
                          className="badge"
                          style={{
                            color: errorCategoryColor(detail.failed.error_category),
                            borderColor: errorCategoryColor(
                              detail.failed.error_category,
                            ),
                          }}
                        >
                          {errorCategoryLabel(detail.failed.error_category)}
                        </span>
                      }
                    />
                    <KpiCard
                      label="Revert reason"
                      icon="⚠"
                      value={detail.failed.revert_reason ?? "—"}
                    />
                    <KpiCard
                      label="Failing function"
                      icon="ƒ"
                      value={
                        detail.failing_function_decoded ? (
                          <span style={{ display: "inline-block", textAlign: "left" }}>
                            <div style={{ fontWeight: 600 }}>
                              {detail.failing_function_decoded.name}
                            </div>
                            <div
                              className="mono"
                              style={{ fontSize: 11, opacity: 0.7 }}
                            >
                              {detail.failing_function_decoded.signature}
                            </div>
                            {detail.failing_function_decoded.source && (
                              <span
                                className="badge"
                                style={{
                                  fontSize: 10,
                                  marginTop: 4,
                                  display: "inline-block",
                                }}
                              >
                                {detail.failing_function_decoded.source}
                              </span>
                            )}
                          </span>
                        ) : (
                          <span className="mono">
                            {detail.failed.failing_function ?? "—"}
                          </span>
                        )
                      }
                    />
                    <KpiCard
                      label="Gas used"
                      icon="⛽"
                      value={formatCompact(detail.failed.gas_used)}
                    />
                  </div>
                  {detail.call_tree_truncated && (
                    <div
                      className="badge"
                      style={{ marginTop: 8, color: "#F66061", borderColor: "#F66061" }}
                    >
                      call_tree truncated (S04 cap hit) — tail dropped
                    </div>
                  )}
                  {detail.root_cause ? (
                    <div
                      style={{
                        marginTop: 12,
                        padding: 10,
                        borderLeft: "3px solid #F66061",
                        background: "rgba(246,96,97,0.06)",
                        fontSize: 13,
                      }}
                    >
                      <div className="card-sub" style={{ marginBottom: 6 }}>
                        Root cause — first revert frame in{" "}
                        <span className="mono">trace_id ASC</span> (pre-order DFS)
                      </div>
                      <div
                        style={{
                          fontFamily: "var(--font-mono, monospace)",
                          fontSize: 12,
                          lineHeight: 1.6,
                        }}
                      >
                        <span className="muted">
                          [{detail.root_cause.trace_id}]
                        </span>{" "}
                        depth {detail.root_cause.call_depth}{" "}
                        {detail.root_cause.call_type}{" "}
                        <span className="mono">
                          {detail.root_cause.from_addr.slice(0, 10)}…
                        </span>
                        {" → "}
                        <span className="mono">
                          {detail.root_cause.to_addr
                            ? `${detail.root_cause.to_addr.slice(0, 10)}…`
                            : "—"}
                        </span>
                        {detail.root_cause.input && (
                          <>
                            {" · selector "}
                            <span className="mono">
                              {detail.root_cause.input.slice(0, 10)}
                            </span>
                          </>
                        )}
                        <div style={{ color: "#F66061", marginTop: 6 }}>
                          err: {detail.root_cause.error}
                        </div>
                      </div>
                    </div>
                  ) : (
                    <div
                      className="muted"
                      style={{ marginTop: 12, fontSize: 13 }}
                    >
                      Root cause: <span className="mono">null</span> — the
                      indexer recorded no per-frame error for this transaction
                      (silent default is intentionally not allowed; see docs).
                    </div>
                  )}
                  {detail.diagnosis ? (
                    <div
                      style={{
                        marginTop: 12,
                        padding: 10,
                        borderLeft: "3px solid #3ecf8e",
                        background: "rgba(62,207,142,0.07)",
                        fontSize: 13,
                      }}
                    >
                      <div className="card-sub" style={{ marginBottom: 6 }}>
                        Diagnosis — why it failed + what to try next
                      </div>
                      <div
                        style={{
                          marginBottom: detail.diagnosis.recommended_action
                            ? 6
                            : 0,
                        }}
                      >
                        {detail.diagnosis.message}
                      </div>
                      {detail.diagnosis.recommended_action && (
                        <div style={{ fontWeight: 600 }}>
                          ▶ {detail.diagnosis.recommended_action}
                        </div>
                      )}
                      {detail.diagnosis.source && (
                        <span
                          className="badge"
                          style={{
                            fontSize: 10,
                            marginTop: 6,
                            display: "inline-block",
                          }}
                        >
                          {detail.diagnosis.source}
                        </span>
                      )}
                    </div>
                  ) : (
                    <div
                      className="muted"
                      style={{ marginTop: 12, fontSize: 13 }}
                    >
                      Diagnosis: <span className="mono">null</span> — this
                      error_category isn't seeded yet (operators can extend
                      via `category_diagnosis`).
                    </div>
                  )}
                  <div
                    style={{
                      marginTop: 12,
                      fontFamily: "var(--font-mono, monospace)",
                      fontSize: 12,
                      lineHeight: 1.6,
                    }}
                  >
                    {detail.call_tree.map((frame) => (
                      <div
                        key={frame.trace_id}
                        style={{ paddingLeft: frame.call_depth * 16 }}
                      >
                        <span className="muted">[{frame.trace_id}]</span>{" "}
                        {frame.call_type}{" "}
                        <span className="mono">{frame.from_addr.slice(0, 10)}…</span>
                        {" → "}
                        <span className="mono">
                          {frame.to_addr ? `${frame.to_addr.slice(0, 10)}…` : "—"}
                        </span>{" "}
                        <span className="muted">
                          ({formatCompact(frame.gas_used)} gas)
                        </span>
                        {frame.error && (
                          <span
                            style={{ color: "#F66061", marginLeft: 8 }}
                          >
                            err: {frame.error}
                          </span>
                        )}
                      </div>
                    ))}
                  </div>
                </>
              )}
            </AsyncState>
          </div>
        )}

        <div className="card">
          <div className="card-head">
            <div className="card-title">Investigation drill-down</div>
            <div className="card-sub">Cross-check failed-tx signal with other lenses</div>
          </div>
          <div className="grid">
            <Link className="btn" to="/traders?source=failed-tx">
              Open top traders
            </Link>
            <Link className="btn" to="/pools?source=failed-tx">
              Open pools
            </Link>
            <button
              className="btn"
              type="button"
              onClick={() => setFilters({ category: "ALL", sort: "recent", offset: "0" })}
            >
              Focus recent anomalies
            </button>
          </div>
        </div>
      </AsyncState>
    </>
  );
}
