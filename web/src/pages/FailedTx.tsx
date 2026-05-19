import { useMemo } from "react";
import { Link, useSearchParams } from "react-router-dom";
import {
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

import { useFailedTxAnalysis } from "@/api/hooks";
import { ERROR_CATEGORIES, type FailedTxAnalysis } from "@/api/types";
import { AsyncState } from "@/components/AsyncState";
import { type Column, DataTable } from "@/components/DataTable";
import { KpiCard } from "@/components/KpiCard";
import {
  axisLine,
  axisTick,
  chartWarning,
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
  });
  const selectedCategory = parseEnumFilter(
    filters.category,
    CATEGORY_FILTERS,
    "ALL",
  );
  const sortBy = parseEnumFilter(filters.sort, SORT_FILTERS, "failures");
  const windowFilter = parseEnumFilter(filters.window, WINDOW_FILTERS, "90");
  const windowDays = windowFilter === "all" ? null : parseIntFilter(windowFilter, 90, 1, 3650);

  const query = useFailedTxAnalysis();
  const data = query.data ?? [];
  const source = searchParams.get("source");
  const sourcePool = searchParams.get("pool");

  const { scoped, chart, timeline, total, topCategory, mostRecentLabel } = useMemo(() => {
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

    const timelineByDate = new Map<
      string,
      { date: string; categoriesTouched: number; failures: number }
    >();
    for (const row of sorted) {
      const date = row.most_recent_failure.slice(0, 10);
      const curr = timelineByDate.get(date);
      if (curr) {
        curr.categoriesTouched += 1;
        curr.failures += row.failure_count;
      } else {
        timelineByDate.set(date, {
          date,
          categoriesTouched: 1,
          failures: row.failure_count,
        });
      }
    }

    const timelineRows = [...timelineByDate.values()].sort((a, b) =>
      a.date.localeCompare(b.date),
    );

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
      timeline: timelineRows,
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
              onChange={(e) => setFilters({ category: e.target.value })}
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
              onChange={(e) => setFilters({ window: e.target.value })}
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
            <div className="card-title">Time-axis of recent failures</div>
            <div className="card-sub">Grouped by most-recent failure day per category</div>
          </div>
          <div className="chart-box">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart
                data={timeline}
                margin={{ top: 8, right: 12, bottom: 0, left: 0 }}
              >
                <CartesianGrid stroke="rgba(51,51,51,0.8)" vertical={false} />
                <XAxis
                  dataKey="date"
                  tick={axisTick}
                  axisLine={axisLine}
                  tickLine={false}
                  tickFormatter={formatDate}
                  minTickGap={26}
                />
                <YAxis
                  yAxisId="failures"
                  tick={axisTick}
                  axisLine={false}
                  tickLine={false}
                  width={48}
                  tickFormatter={(v) => formatCompact(numeric(v))}
                />
                <YAxis
                  yAxisId="categories"
                  orientation="right"
                  tick={axisTick}
                  axisLine={false}
                  tickLine={false}
                  width={38}
                />
                <Tooltip
                  contentStyle={tooltipContentStyle}
                  itemStyle={tooltipItemStyle}
                  labelStyle={tooltipLabelStyle}
                  labelFormatter={(l) => formatDate(String(l))}
                  formatter={(v, key) => [
                    key === "categoriesTouched"
                      ? formatNumber(numeric(v))
                      : formatCompact(numeric(v)),
                    key === "categoriesTouched" ? "Categories touched" : "Failures",
                  ]}
                />
                <Bar
                  yAxisId="failures"
                  dataKey="failures"
                  fill="#F66061"
                  radius={[3, 3, 0, 0]}
                />
                <Bar
                  yAxisId="categories"
                  dataKey="categoriesTouched"
                  fill={chartWarning}
                  radius={[3, 3, 0, 0]}
                />
              </BarChart>
            </ResponsiveContainer>
          </div>
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
            onRowClick={(row) => setFilters({ category: row.error_category })}
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
                onClick={() => setFilters({ category: "ALL", sort: "recent" })}
              >
                Focus recent anomalies
              </button>
            </div>
          </div>
          <div className="card">
            <div className="card-head">
              <div className="card-title">Sample signals</div>
              <div className="card-sub">
                Latest category snapshots for triage (raw failed-tx events need API extension)
              </div>
            </div>
            <div className="sample-signals">
              {scoped.slice(0, 5).map((row) => (
                <div key={row.error_category} className="spread">
                  <span>{errorCategoryLabel(row.error_category)}</span>
                  <span className="muted">{formatDateTime(row.most_recent_failure)}</span>
                </div>
              ))}
              {scoped.length === 0 && <span className="muted">No category snapshots.</span>}
            </div>
            <p className="muted" style={{ marginTop: 12 }}>
              Proposed API additions: failed-tx raw list with filters (`category`, `from`, `to`,
              `limit`, `offset`) and tx-level detail endpoint.
            </p>
          </div>
        </div>
      </AsyncState>
    </>
  );
}
