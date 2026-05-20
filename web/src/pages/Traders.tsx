import { useMemo } from "react";
import { Link } from "react-router-dom";
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import { useTopTraders } from "@/api/hooks";
import type { TopTrader } from "@/api/types";
import { AddressBadge } from "@/components/AddressBadge";
import { AsyncState } from "@/components/AsyncState";
import { type Column, DataTable } from "@/components/DataTable";
import {
  axisLine,
  axisTick,
  chartBrand,
  gridStroke,
  numeric,
  tooltipContentStyle,
  tooltipItemStyle,
  tooltipLabelStyle,
} from "@/lib/chart";
import {
  formatCompact,
  formatNumber,
  formatUsd,
  shortAddress,
  toNumber,
} from "@/lib/format";
import { parseEnumFilter, parseIntFilter, useUrlFilters } from "@/lib/urlFilters";

const LIMIT = 25;
const LABEL_FILTERS = ["all", "whale", "bot", "retail"] as const;
const SORT_FILTERS = ["volume", "swaps", "rank"] as const;

function labelClass(label: string | null): string {
  switch (label?.toLowerCase()) {
    case "whale":
      return "badge badge--whale";
    case "bot":
      return "badge badge--bot";
    case "retail":
      return "badge badge--retail";
    default:
      return "badge";
  }
}

const columns: Column<TopTrader>[] = [
  {
    header: "#",
    cell: (t) => (
      <span className={`rank ${t.volume_rank === 1 ? "rank--1" : ""}`}>
        {t.volume_rank}
      </span>
    ),
  },
  {
    header: "Trader",
    cell: (t) => <AddressBadge address={t.user_address} />,
  },
  {
    header: "Label",
    cell: (t) =>
      t.label ? (
        <span className={labelClass(t.label)}>{t.label}</span>
      ) : (
        <span className="faint">—</span>
      ),
  },
  {
    header: "Swaps",
    align: "right",
    cell: (t) => formatNumber(t.total_swaps),
  },
  {
    header: "Volume (USD)",
    align: "right",
    cell: (t) => formatUsd(t.total_volume_usd),
  },
];

export function Traders() {
  const [filters, setFilters] = useUrlFilters({
    limit: String(LIMIT),
    label: "all",
    sort: "volume",
  });
  const limit = parseIntFilter(filters.limit, LIMIT, 10, 100);
  const label = parseEnumFilter(filters.label, LABEL_FILTERS, "all");
  const sort = parseEnumFilter(filters.sort, SORT_FILTERS, "volume");

  const query = useTopTraders(limit);
  const rows = query.data?.data ?? [];

  const filteredRows = useMemo(() => {
    const next =
      label === "all"
        ? [...rows]
        : rows.filter((row) => row.label?.toLowerCase() === label);

    switch (sort) {
      case "swaps":
        next.sort((a, b) => b.total_swaps - a.total_swaps);
        break;
      case "rank":
        next.sort((a, b) => a.volume_rank - b.volume_rank);
        break;
      case "volume":
      default:
        next.sort((a, b) => toNumber(b.total_volume_usd) - toNumber(a.total_volume_usd));
        break;
    }

    return next;
  }, [rows, label, sort]);

  const chart = useMemo(
    () =>
      [...filteredRows]
        .slice(0, 10)
        .map((t) => ({
          name: shortAddress(t.user_address),
          volume: toNumber(t.total_volume_usd),
        })),
    [filteredRows],
  );

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Top traders</h1>
          <p>
            Ranked by total USD volume (vw_top_traders) ·{" "}
            <Link className="inline-link" to="/failed-tx?source=traders">
              compare against failed-tx signals
            </Link>
          </p>
        </div>
        <div className="toolbar">
          <div className="field">
            <label htmlFor="limit">Limit</label>
            <select
              id="limit"
              value={String(limit)}
              onChange={(e) => setFilters({ limit: e.target.value })}
            >
              <option value="10">10</option>
              <option value="25">25</option>
              <option value="50">50</option>
              <option value="100">100</option>
            </select>
          </div>
          <div className="field">
            <label htmlFor="label">Label</label>
            <select
              id="label"
              value={label}
              onChange={(e) => setFilters({ label: e.target.value })}
            >
              <option value="all">All</option>
              <option value="whale">Whale</option>
              <option value="bot">Bot</option>
              <option value="retail">Retail</option>
            </select>
          </div>
          <div className="field">
            <label htmlFor="sort">Sort</label>
            <select
              id="sort"
              value={sort}
              onChange={(e) => setFilters({ sort: e.target.value })}
            >
              <option value="volume">Volume</option>
              <option value="swaps">Swaps</option>
              <option value="rank">Rank</option>
            </select>
          </div>
        </div>
      </div>

      <AsyncState
        isLoading={query.isLoading}
        isError={query.isError}
        error={query.error}
        isEmpty={filteredRows.length === 0}
        emptyLabel="No trader activity indexed yet."
      >
        <div className="card">
          <div className="card-head">
            <div className="card-title">Top 10 by volume</div>
            <div className="card-sub">USD</div>
          </div>
          <div className="chart-box">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart
                data={chart}
                margin={{ top: 8, right: 12, bottom: 0, left: 0 }}
              >
                <CartesianGrid stroke={gridStroke} vertical={false} />
                <XAxis
                  dataKey="name"
                  tick={axisTick}
                  axisLine={axisLine}
                  tickLine={false}
                  interval={0}
                  angle={-30}
                  textAnchor="end"
                  height={56}
                />
                <YAxis
                  tick={axisTick}
                  axisLine={false}
                  tickLine={false}
                  width={52}
                  tickFormatter={(v) => `$${formatCompact(numeric(v))}`}
                />
                <Tooltip
                  cursor={{ fill: "rgba(62,207,142,0.12)" }}
                  contentStyle={tooltipContentStyle}
                  itemStyle={tooltipItemStyle}
                  labelStyle={tooltipLabelStyle}
                  formatter={(v) => [`$${formatNumber(numeric(v))}`, "Volume"]}
                />
                <Bar dataKey="volume" fill={chartBrand} radius={[3, 3, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </div>
        </div>

        <div className="card">
          <div className="card-head">
            <div className="card-title">Ranking</div>
            <div className="card-sub">Top {limit} (URL-synced filters)</div>
          </div>
          <DataTable
            columns={columns}
            rows={filteredRows}
            rowKey={(t) => t.user_address}
            caption="Top trader ranking"
          />
        </div>
      </AsyncState>
    </>
  );
}
