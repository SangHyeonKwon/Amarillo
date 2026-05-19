import { useMemo } from "react";
import { useNavigate } from "react-router-dom";
import {
  Area,
  AreaChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import {
  useDailyVolume,
  useFailedTxAnalysis,
  useLatestBlock,
  usePools,
  useSwaps,
} from "@/api/hooks";
import { API_BASE_URL } from "@/api/client";
import type { SwapEvent } from "@/api/types";
import { AddressBadge } from "@/components/AddressBadge";
import { AsyncState } from "@/components/AsyncState";
import { type Column, DataTable } from "@/components/DataTable";
import { KpiCard } from "@/components/KpiCard";
import { WorkspacePanel } from "@/components/WorkspacePanel";
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
import { formatCompact, formatDate, formatNumber, timeAgo } from "@/lib/format";

const swapColumns: Column<SwapEvent>[] = [
  {
    header: "Time",
    cell: (s) => <span title={s.timestamp}>{timeAgo(s.timestamp)}</span>,
  },
  { header: "Pool", cell: (s) => <AddressBadge address={s.pool_address} /> },
  { header: "Tx", cell: (s) => <AddressBadge address={s.tx_hash} /> },
  {
    header: "Amount In",
    align: "right",
    cell: (s) => formatCompact(s.amount_in),
  },
  {
    header: "Amount Out",
    align: "right",
    cell: (s) => formatCompact(s.amount_out),
  },
];

export function Overview() {
  const navigate = useNavigate();
  const latest = useLatestBlock();
  const volume = useDailyVolume({ limit: 100 });
  const failed = useFailedTxAnalysis();
  const pools = usePools({ limit: 100 });
  const swaps = useSwaps({ limit: 8 });

  const series = useMemo(() => {
    const rows = volume.data?.data ?? [];
    const byDate = new Map<string, number>();
    for (const r of rows) {
      byDate.set(r.swap_date, (byDate.get(r.swap_date) ?? 0) + r.swap_count);
    }
    return [...byDate.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([date, swapCount]) => ({ date, swapCount }));
  }, [volume.data]);

  const totalSwaps = series.reduce((acc, d) => acc + d.swapCount, 0);
  const totalFailures = (failed.data ?? []).reduce(
    (acc, f) => acc + f.failure_count,
    0,
  );

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Overview</h1>
          <p>Live snapshot of indexed Uniswap V3 activity</p>
        </div>
      </div>

      <div className="grid kpi-grid">
        <KpiCard
          label="Latest block"
          icon="◫"
          loading={latest.isLoading}
          value={latest.data != null ? formatNumber(latest.data) : "—"}
          hint={latest.isError ? "API unreachable" : "Last indexed"}
        />
        <KpiCard
          label="Swaps (recent window)"
          icon="⇄"
          loading={volume.isLoading}
          value={formatCompact(totalSwaps)}
          hint="Across daily-volume view"
        />
        <KpiCard
          label="Failed transactions"
          icon="⚠"
          loading={failed.isLoading}
          value={formatCompact(totalFailures)}
          hint="Decoded from tx traces"
        />
        <KpiCard
          label="Pools indexed"
          icon="◇"
          loading={pools.isLoading}
          value={formatNumber(pools.data?.pagination.count ?? 0)}
          hint="First page"
        />
      </div>

      <WorkspacePanel
        apiBaseUrl={API_BASE_URL}
        latestBlock={latest.data}
        poolCount={pools.data?.pagination.count ?? 0}
        failedCount={totalFailures}
        previewRows={volume.data?.data ?? []}
        recentSwaps={swaps.data?.data ?? []}
        isApiOnline={latest.isSuccess}
      />

      <div className="card">
        <div className="card-head">
          <div className="card-title">Daily swap volume</div>
          <div className="card-sub">Sum of swaps per day, all pools</div>
        </div>
        <AsyncState
          isLoading={volume.isLoading}
          isError={volume.isError}
          error={volume.error}
          isEmpty={series.length === 0}
          emptyLabel="No swap volume in the indexed range yet."
        >
          <div className="chart-box">
            <ResponsiveContainer width="100%" height="100%">
              <AreaChart
                data={series}
                margin={{ top: 8, right: 12, bottom: 0, left: 0 }}
              >
                <defs>
                  <linearGradient id="vol" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor={chartBrand} stopOpacity={0.42} />
                    <stop offset="100%" stopColor={chartBrand} stopOpacity={0} />
                  </linearGradient>
                </defs>
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
                  contentStyle={tooltipContentStyle}
                  itemStyle={tooltipItemStyle}
                  labelStyle={tooltipLabelStyle}
                  labelFormatter={(l) => formatDate(String(l))}
                  formatter={(v) => [formatNumber(numeric(v)), "Swaps"]}
                />
                <Area
                  type="monotone"
                  dataKey="swapCount"
                  stroke={chartBrand}
                  strokeWidth={2}
                  fill="url(#vol)"
                />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        </AsyncState>
      </div>

      <div className="card">
        <div className="card-head">
          <div className="card-title">Recent swaps</div>
          <div className="card-sub">Latest decoded Swap events</div>
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
            caption="Recent swap events"
            onRowClick={(s) => navigate(`/pools/${s.pool_address}`)}
          />
        </AsyncState>
      </div>
    </>
  );
}
