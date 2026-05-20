/**
 * TypeScript mirror of the REST API contract.
 *
 * These types mirror the serialized shapes of `crates/db/src/models.rs`
 * (view-backed models) and the response envelopes in
 * `crates/api/src/response.rs` / `crates/api/src/error.rs`. The backend is
 * the source of truth — keep this file in sync if the API changes.
 *
 * `BigDecimal` values are expected as decimal strings on the wire (serde), but
 * the UI tolerates either string/number and normalizes through `api/contract`.
 * Display formatting then coerces through `toNumber()` where approximation is
 * acceptable.
 */

export type Decimal = number | string;
/** RFC 3339 / ISO 8601 timestamp, e.g. `2023-09-01T00:00:00Z`. */
export type IsoDateTime = string;
/** Calendar date, e.g. `2023-09-01`. */
export type IsoDate = string;

/** `FailedTransaction.error_category` — SCREAMING_SNAKE_CASE on the wire. */
export type ErrorCategory =
  | "INSUFFICIENT_BALANCE"
  | "SLIPPAGE_EXCEEDED"
  | "DEADLINE_EXPIRED"
  | "UNAUTHORIZED"
  | "TRANSFER_FAILED"
  | "UNKNOWN";

export const ERROR_CATEGORIES: ErrorCategory[] = [
  "INSUFFICIENT_BALANCE",
  "SLIPPAGE_EXCEEDED",
  "DEADLINE_EXPIRED",
  "UNAUTHORIZED",
  "TRANSFER_FAILED",
  "UNKNOWN",
];

// ── Table models ────────────────────────────────────────────────

export interface Block {
  block_number: number;
  timestamp: IsoDateTime;
  gas_used: number;
}

export interface Token {
  token_address: string;
  symbol: string;
  name: string;
  decimals: number;
}

export interface Pool {
  pool_address: string;
  pair_name: string;
  token0_address: string;
  token1_address: string;
  fee_tier: number;
  created_at: IsoDateTime;
}

export interface SwapEvent {
  pool_address: string;
  tx_hash: string;
  sender: string;
  recipient: string;
  amount0: Decimal;
  amount1: Decimal;
  amount_in: Decimal;
  amount_out: Decimal;
  sqrt_price_x96: Decimal;
  liquidity: Decimal;
  tick: number;
  log_index: number;
  timestamp: IsoDateTime;
  event_id: number;
}

// ── View-backed models (API response shapes) ────────────────────

export interface DailySwapVolume {
  pool_address: string;
  pair_name: string;
  swap_date: IsoDate;
  swap_count: number;
  total_amount_in: Decimal;
  total_amount_out: Decimal;
}

export interface TopTrader {
  user_address: string;
  label: string | null;
  total_swaps: number;
  total_volume_usd: Decimal;
  volume_rank: number;
}

export interface FailedTxAnalysis {
  error_category: ErrorCategory;
  failure_count: number;
  avg_gas_wasted: Decimal;
  pct_of_total: Decimal;
  most_recent_failure: IsoDateTime;
}

export interface PoolStats {
  pair_name: string;
  swap_count: number;
  unique_traders: number;
  total_volume_in: Decimal;
  avg_trade_size: Decimal;
  liquidity_events: number;
  estimated_fees: Decimal;
}

// ── Response envelopes ──────────────────────────────────────────

/** `crates/api/src/response.rs` — single-resource wrapper. */
export interface ApiResponse<T> {
  data: T;
}

export interface PaginationInfo {
  limit: number;
  offset: number;
  /** Count of rows in this page, not total rows across all pages. */
  count: number;
}

/** `crates/api/src/response.rs` — paginated list wrapper. */
export interface PaginatedResponse<T> {
  data: T[];
  pagination: PaginationInfo;
}

/** `crates/api/src/error.rs` — error body is `{ "error": string }`. */
export interface ApiErrorBody {
  error: string;
}
