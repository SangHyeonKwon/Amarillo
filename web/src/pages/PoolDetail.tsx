import { useMemo } from "react";
import { Link, useParams } from "react-router-dom";
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import {
  useDailyVolume,
  usePool,
  usePoolStats,
  useSwaps,
} from "@/api/hooks";
import type { SwapEvent } from "@/api/types";
import { AddressBadge } from "@/components/AddressBadge";
import { AsyncState } from "@/components/AsyncState";
import { type Column, DataTable } from "@/components/DataTable";
import { KpiCard } from "@/components/KpiCard";
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
  feeTierLabel,
  formatCompact,
  formatDate,
  formatNumber,
  fromDateInput,
  timeAgo,
  toDateInput,
} from "@/lib/format";
import { parseIntFilter, useUrlFilters } from "@/lib/urlFilters";

// Uniswap V3 launched mid-2021; this lower bound safely covers any seed data.
const DEFAULT_FROM = "2019-01-01";
const DEFAULT_TO = toDateInput(new Date().toISOString());

const swapColumns: Column<SwapEvent>[] = [
  {
    header: "Time",
    cell: (s) => <span title={s.timestamp}>{timeAgo(s.timestamp)}</span>,
  },
  { header: "Tx", cell: (s) => <AddressBadge address={s.tx_hash} /> },
  { header: "Sender", cell: (s) => <AddressBadge address={s.sender} /> },
  { header: "In", align: "right", cell: (s) => formatCompact(s.amount_in) },
  { header: "Out", align: "right", cell: (s) => formatCompact(s.amount_out) },
];

export function PoolDetail() {
  const { address } = useParams<{ address: string }>();
  const [filters, setFilters] = useUrlFilters({
    from: DEFAULT_FROM,
    to: DEFAULT_TO,
    swaps: "10",
  });
  const from = filters.from;
  const to = filters.to;
  const swapLimit = parseIntFilter(filters.swaps, 10, 5, 100);

  const pool = usePool(address);
  const stats = usePoolStats(
    address,
    fromDateInput(from),
    fromDateInput(to),
  );
  const volume = useDailyVolume({ poolAddress: address, limit: 100 });
  const swaps = useSwaps({ poolAddress: address, limit: swapLimit });

  const series = useMemo(
    () =>
      (volume.data?.data ?? [])
        .map((r) => ({ date: r.swap_date, swapCount: r.swap_count }))
        .sort((a, b) => a.date.localeCompare(b.date)),
    [volume.data],
  );

  return (
    <>
      <div>
        <Link to="/pools" className="back-link">
          ← Pools
        </Link>
        <div className="page-head">
          <div>
            <h1>{pool.data?.pair_name ?? "Pool"}</h1>
            <p>
              {address && <AddressBadge address={address} full />}
              {pool.data && (
                <span className="badge badge--fee" style={{ marginLeft: 10 }}>
                  {feeTierLabel(pool.data.fee_tier)}
                </span>
              )}
              {address && (
                <>
                  {" "}
                  ·{" "}
                  <Link
                    className="inline-link"
                    to={`/failed-tx?source=pool&pool=${address}`}
                  >
                    Inspect failed-tx context
                  </Link>
                </>
              )}
            </p>
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-head">
          <div className="card-title">Pool stats</div>
          <div className="toolbar">
            <div className="field">
              <label htmlFor="from">From</label>
              <input
                id="from"
                type="date"
                value={from}
                max={to}
                onChange={(e) => {
                  const nextFrom = e.target.value;
                  setFilters({
                    from: nextFrom,
                    ...(nextFrom > to ? { to: nextFrom } : {}),
                  });
                }}
              />
            </div>
            <div className="field">
              <label htmlFor="to">To</label>
              <input
                id="to"
                type="date"
                value={to}
                min={from}
                onChange={(e) => {
                  const nextTo = e.target.value;
                  setFilters({
                    to: nextTo,
                    ...(nextTo < from ? { from: nextTo } : {}),
                  });
                }}
              />
            </div>
            <div className="field">
              <label htmlFor="swaps">Recent swaps</label>
              <select
                id="swaps"
                value={String(swapLimit)}
                onChange={(e) => setFilters({ swaps: e.target.value })}
              >
                <option value="10">10</option>
                <option value="20">20</option>
                <option value="50">50</option>
                <option value="100">100</option>
              </select>
            </div>
            <div className="field field--hint">
              <label>Shareable filters</label>
              <span className="muted">Date range and swap limit are URL-synced.</span>
            </div>
            <div className="field">
              <label htmlFor="reset-filters">Reset</label>
              <button
                id="reset-filters"
                className="btn"
                type="button"
                onClick={() =>
                  setFilters({
                    from: DEFAULT_FROM,
                    to: DEFAULT_TO,
                    swaps: "10",
                  })
                }
              >
                Reset filters
              </button>
            </div>
          </div>
        </div>
        <AsyncState
          isLoading={stats.isLoading}
          isError={stats.isError}
          error={stats.error}
        >
          <div className="grid kpi-grid">
            <KpiCard
              label="Swaps"
              value={formatNumber(stats.data?.swap_count ?? 0)}
            />
            <KpiCard
              label="Unique traders"
              value={formatNumber(stats.data?.unique_traders ?? 0)}
            />
            <KpiCard
              label="Volume in"
              value={formatCompact(stats.data?.total_volume_in ?? 0)}
            />
            <KpiCard
              label="Avg trade size"
              value={formatCompact(stats.data?.avg_trade_size ?? 0)}
            />
            <KpiCard
              label="Liquidity events"
              value={formatNumber(stats.data?.liquidity_events ?? 0)}
            />
            <KpiCard
              label="Est. fees"
              value={formatCompact(stats.data?.estimated_fees ?? 0)}
            />
          </div>
        </AsyncState>
      </div>

      <div className="card">
        <div className="card-head">
          <div className="card-title">Daily swap volume</div>
          <div className="card-sub">This pool</div>
        </div>
        <AsyncState
          isLoading={volume.isLoading}
          isError={volume.isError}
          error={volume.error}
          isEmpty={series.length === 0}
        >
          <div className="chart-box">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart
                data={series}
                margin={{ top: 8, right: 12, bottom: 0, left: 0 }}
              >
                <CartesianGrid stroke={gridStroke} vertical={false} />
                <XAxis
                  dataKey="date"
                  tick={axisTick}
                  axisLine={axisLine}
                  tickLine={false}
                  tickFormatter={formatDate}
                  minTickGap={28}
                />
                <YAxis
                  tick={axisTick}
                  axisLine={false}
                  tickLine={false}
                  width={48}
                  tickFormatter={(v) => formatCompact(numeric(v))}
                />
                <Tooltip
                  cursor={{ fill: "rgba(62,207,142,0.12)" }}
                  contentStyle={tooltipContentStyle}
                  itemStyle={tooltipItemStyle}
                  labelStyle={tooltipLabelStyle}
                  labelFormatter={(l) => formatDate(String(l))}
                  formatter={(v) => [formatNumber(numeric(v)), "Swaps"]}
                />
                <Bar dataKey="swapCount" fill={chartBrand} radius={[3, 3, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </div>
        </AsyncState>
      </div>

      <div className="card">
        <div className="card-head">
          <div className="card-title">Recent swaps</div>
        </div>
        <AsyncState
          isLoading={swaps.isLoading}
          isError={swaps.isError}
          error={swaps.error}
          isEmpty={(swaps.data?.data.length ?? 0) === 0}
        >
          <DataTable
            columns={swapColumns}
            rows={swaps.data?.data ?? []}
            rowKey={(s) => s.event_id}
            caption="Recent swaps for selected pool"
          />
        </AsyncState>
      </div>
    </>
  );
}
