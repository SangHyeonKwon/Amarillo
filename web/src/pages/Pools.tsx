import { useNavigate, useSearchParams } from "react-router-dom";

import { usePools } from "@/api/hooks";
import type { Pool } from "@/api/types";
import { AddressBadge } from "@/components/AddressBadge";
import { AsyncState } from "@/components/AsyncState";
import { type Column, DataTable } from "@/components/DataTable";
import { Pagination } from "@/components/Pagination";
import { feeTierLabel, formatDate } from "@/lib/format";
import { parseIntFilter, useUrlFilters } from "@/lib/urlFilters";

const LIMIT = 20;

const columns: Column<Pool>[] = [
  {
    header: "Pair",
    cell: (p) => <strong>{p.pair_name}</strong>,
  },
  {
    header: "Fee",
    cell: (p) => <span className="badge badge--fee">{feeTierLabel(p.fee_tier)}</span>,
  },
  { header: "Pool", cell: (p) => <AddressBadge address={p.pool_address} /> },
  { header: "Token 0", cell: (p) => <AddressBadge address={p.token0_address} /> },
  { header: "Token 1", cell: (p) => <AddressBadge address={p.token1_address} /> },
  {
    header: "Created",
    align: "right",
    cell: (p) => <span className="muted">{formatDate(p.created_at)}</span>,
  },
];

export function Pools() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const [filters, setFilters] = useUrlFilters({ offset: "0" });
  const offset = parseIntFilter(filters.offset, 0, 0, 100_000);
  const source = searchParams.get("source");
  const query = usePools({ limit: LIMIT, offset });
  const rows = query.data?.data ?? [];

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Pools</h1>
          <p>Indexed Uniswap V3 liquidity pools — select a row for stats</p>
          {source ? <p className="muted">Entry context: {source}</p> : null}
        </div>
      </div>

      <div className="card">
        <AsyncState
          isLoading={query.isLoading}
          isError={query.isError}
          error={query.error}
          isEmpty={rows.length === 0}
          emptyLabel="No pools indexed yet."
        >
          <DataTable
            columns={columns}
            rows={rows}
            rowKey={(p) => p.pool_address}
            caption="Uniswap pools"
            onRowClick={(p) => navigate(`/pools/${p.pool_address}`)}
          />
          <Pagination
            offset={offset}
            limit={LIMIT}
            count={rows.length}
            onChange={(nextOffset) => setFilters({ offset: String(nextOffset) })}
          />
        </AsyncState>
      </div>
    </>
  );
}
