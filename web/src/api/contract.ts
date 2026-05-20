import type {
  AlertSubscription,
  AlertSubscriptionCreated,
  ApiResponse,
  Block,
  DailySwapVolume,
  Decimal,
  ErrorCategory,
  FailedTransaction,
  FailedTxAnalysis,
  FailedTxDetail,
  FailedTxTrendPoint,
  PaginatedResponse,
  PaginationInfo,
  PaginationMeta,
  Pool,
  PoolStats,
  SwapEvent,
  Token,
  TopTrader,
  TotalPaginatedResponse,
  TraceLog,
} from "./types";
import { ERROR_CATEGORIES } from "./types";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function readRecord(value: unknown, path: string): Record<string, unknown> {
  if (!isRecord(value)) {
    throw new Error(`Invalid contract at ${path}: expected object.`);
  }
  return value;
}

function readString(value: unknown, path: string): string {
  if (typeof value !== "string") {
    throw new Error(`Invalid contract at ${path}: expected string.`);
  }
  return value;
}

function readOptionalString(value: unknown, path: string): string | null {
  if (value === null) return null;
  return readString(value, path);
}

function readBoolean(value: unknown, path: string): boolean {
  if (typeof value !== "boolean") {
    throw new Error(`Invalid contract at ${path}: expected boolean.`);
  }
  return value;
}

function readNumber(value: unknown, path: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`Invalid contract at ${path}: expected finite number.`);
  }
  return value;
}

function readInteger(value: unknown, path: string): number {
  const n = readNumber(value, path);
  if (!Number.isInteger(n)) {
    throw new Error(`Invalid contract at ${path}: expected integer number.`);
  }
  return n;
}

function readDecimal(value: unknown, path: string): Decimal {
  if (typeof value === "string") return value;
  if (typeof value === "number" && Number.isFinite(value)) return String(value);
  throw new Error(`Invalid contract at ${path}: expected decimal string/number.`);
}

function readIsoDateTime(value: unknown, path: string): string {
  const iso = readString(value, path);
  if (Number.isNaN(Date.parse(iso))) {
    throw new Error(`Invalid contract at ${path}: invalid ISO datetime.`);
  }
  return iso;
}

function readIsoDate(value: unknown, path: string): string {
  const date = readString(value, path);
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) {
    throw new Error(`Invalid contract at ${path}: invalid ISO date.`);
  }
  return date;
}

function canonicalizeCategoryToken(token: string): string {
  return token
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .replace(/[^A-Za-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .toUpperCase();
}

export function normalizeErrorCategory(value: unknown, path: string): ErrorCategory {
  if (typeof value === "string") {
    const normalized = canonicalizeCategoryToken(value);
    if (ERROR_CATEGORIES.includes(normalized as ErrorCategory)) {
      return normalized as ErrorCategory;
    }
    return "UNKNOWN";
  }

  // Serde unit enums can appear as {"VariantName": null}. Accept that too.
  if (isRecord(value)) {
    const keys = Object.keys(value);
    if (keys.length === 1) {
      return normalizeErrorCategory(keys[0], path);
    }
  }

  throw new Error(`Invalid contract at ${path}: expected error category.`);
}

export function normalizeAddressParam(value: string): string {
  return value.trim().toLowerCase();
}

function readAddress(value: unknown, path: string): string {
  return normalizeAddressParam(readString(value, path));
}

function parsePaginationInfo(value: unknown, path: string): PaginationInfo {
  const obj = readRecord(value, path);
  return {
    limit: readInteger(obj.limit, `${path}.limit`),
    offset: readInteger(obj.offset, `${path}.offset`),
    count: readInteger(obj.count, `${path}.count`),
  };
}

export function parseApiResponse<T>(
  value: unknown,
  parseData: (data: unknown, path: string) => T,
): ApiResponse<T> {
  const obj = readRecord(value, "response");
  return {
    data: parseData(obj.data, "response.data"),
  };
}

export function parsePaginatedResponse<T>(
  value: unknown,
  parseItem: (item: unknown, path: string) => T,
): PaginatedResponse<T> {
  const obj = readRecord(value, "response");
  const rawData = obj.data;
  if (!Array.isArray(rawData)) {
    throw new Error("Invalid contract at response.data: expected array.");
  }
  return {
    data: rawData.map((item, idx) => parseItem(item, `response.data[${idx}]`)),
    pagination: parsePaginationInfo(obj.pagination, "response.pagination"),
  };
}

function parsePaginationMeta(value: unknown, path: string): PaginationMeta {
  const obj = readRecord(value, path);
  return {
    limit: readInteger(obj.limit, `${path}.limit`),
    offset: readInteger(obj.offset, `${path}.offset`),
    count: readInteger(obj.count, `${path}.count`),
    total: readInteger(obj.total, `${path}.total`),
  };
}

export function parseTotalPaginatedResponse<T>(
  value: unknown,
  parseItem: (item: unknown, path: string) => T,
): TotalPaginatedResponse<T> {
  const obj = readRecord(value, "response");
  const rawData = obj.data;
  if (!Array.isArray(rawData)) {
    throw new Error("Invalid contract at response.data: expected array.");
  }
  return {
    data: rawData.map((item, idx) => parseItem(item, `response.data[${idx}]`)),
    pagination: parsePaginationMeta(obj.pagination, "response.pagination"),
  };
}

function parseBlock(value: unknown, path: string): Block {
  const obj = readRecord(value, path);
  return {
    block_number: readInteger(obj.block_number, `${path}.block_number`),
    timestamp: readIsoDateTime(obj.timestamp, `${path}.timestamp`),
    gas_used: readInteger(obj.gas_used, `${path}.gas_used`),
  };
}

function parsePool(value: unknown, path: string): Pool {
  const obj = readRecord(value, path);
  return {
    pool_address: readAddress(obj.pool_address, `${path}.pool_address`),
    pair_name: readString(obj.pair_name, `${path}.pair_name`),
    token0_address: readAddress(obj.token0_address, `${path}.token0_address`),
    token1_address: readAddress(obj.token1_address, `${path}.token1_address`),
    fee_tier: readInteger(obj.fee_tier, `${path}.fee_tier`),
    created_at: readIsoDateTime(obj.created_at, `${path}.created_at`),
  };
}

function parseToken(value: unknown, path: string): Token {
  const obj = readRecord(value, path);
  return {
    token_address: readAddress(obj.token_address, `${path}.token_address`),
    symbol: readString(obj.symbol, `${path}.symbol`),
    name: readString(obj.name, `${path}.name`),
    decimals: readInteger(obj.decimals, `${path}.decimals`),
  };
}

function parseSwapEvent(value: unknown, path: string): SwapEvent {
  const obj = readRecord(value, path);
  return {
    pool_address: readAddress(obj.pool_address, `${path}.pool_address`),
    tx_hash: readAddress(obj.tx_hash, `${path}.tx_hash`),
    sender: readAddress(obj.sender, `${path}.sender`),
    recipient: readAddress(obj.recipient, `${path}.recipient`),
    amount0: readDecimal(obj.amount0, `${path}.amount0`),
    amount1: readDecimal(obj.amount1, `${path}.amount1`),
    amount_in: readDecimal(obj.amount_in, `${path}.amount_in`),
    amount_out: readDecimal(obj.amount_out, `${path}.amount_out`),
    sqrt_price_x96: readDecimal(obj.sqrt_price_x96, `${path}.sqrt_price_x96`),
    liquidity: readDecimal(obj.liquidity, `${path}.liquidity`),
    tick: readInteger(obj.tick, `${path}.tick`),
    log_index: readInteger(obj.log_index, `${path}.log_index`),
    timestamp: readIsoDateTime(obj.timestamp, `${path}.timestamp`),
    event_id: readInteger(obj.event_id, `${path}.event_id`),
  };
}

function parsePoolStats(value: unknown, path: string): PoolStats {
  const obj = readRecord(value, path);
  return {
    pair_name: readString(obj.pair_name, `${path}.pair_name`),
    swap_count: readInteger(obj.swap_count, `${path}.swap_count`),
    unique_traders: readInteger(obj.unique_traders, `${path}.unique_traders`),
    total_volume_in: readDecimal(obj.total_volume_in, `${path}.total_volume_in`),
    avg_trade_size: readDecimal(obj.avg_trade_size, `${path}.avg_trade_size`),
    liquidity_events: readInteger(obj.liquidity_events, `${path}.liquidity_events`),
    estimated_fees: readDecimal(obj.estimated_fees, `${path}.estimated_fees`),
  };
}

function parseDailySwapVolume(value: unknown, path: string): DailySwapVolume {
  const obj = readRecord(value, path);
  return {
    pool_address: readAddress(obj.pool_address, `${path}.pool_address`),
    pair_name: readString(obj.pair_name, `${path}.pair_name`),
    swap_date: readIsoDate(obj.swap_date, `${path}.swap_date`),
    swap_count: readInteger(obj.swap_count, `${path}.swap_count`),
    total_amount_in: readDecimal(obj.total_amount_in, `${path}.total_amount_in`),
    total_amount_out: readDecimal(obj.total_amount_out, `${path}.total_amount_out`),
  };
}

function parseTopTrader(value: unknown, path: string): TopTrader {
  const obj = readRecord(value, path);
  return {
    user_address: readAddress(obj.user_address, `${path}.user_address`),
    label: readOptionalString(obj.label, `${path}.label`),
    total_swaps: readInteger(obj.total_swaps, `${path}.total_swaps`),
    total_volume_usd: readDecimal(obj.total_volume_usd, `${path}.total_volume_usd`),
    volume_rank: readInteger(obj.volume_rank, `${path}.volume_rank`),
  };
}

function parseFailedTxAnalysis(value: unknown, path: string): FailedTxAnalysis {
  const obj = readRecord(value, path);
  return {
    error_category: normalizeErrorCategory(
      obj.error_category,
      `${path}.error_category`,
    ),
    failure_count: readInteger(obj.failure_count, `${path}.failure_count`),
    avg_gas_wasted: readDecimal(obj.avg_gas_wasted, `${path}.avg_gas_wasted`),
    pct_of_total: readDecimal(obj.pct_of_total, `${path}.pct_of_total`),
    most_recent_failure: readIsoDateTime(
      obj.most_recent_failure,
      `${path}.most_recent_failure`,
    ),
  };
}

export function parseLatestBlockEnvelope(value: unknown): ApiResponse<number | null> {
  return parseApiResponse(value, (data, path) =>
    data === null ? null : readInteger(data, path),
  );
}

export function parseBlockEnvelope(value: unknown): ApiResponse<Block> {
  return parseApiResponse(value, parseBlock);
}

export function parsePoolsEnvelope(value: unknown): PaginatedResponse<Pool> {
  return parsePaginatedResponse(value, parsePool);
}

export function parsePoolEnvelope(value: unknown): ApiResponse<Pool> {
  return parseApiResponse(value, parsePool);
}

export function parsePoolStatsEnvelope(value: unknown): ApiResponse<PoolStats> {
  return parseApiResponse(value, parsePoolStats);
}

export function parseTokensEnvelope(value: unknown): PaginatedResponse<Token> {
  return parsePaginatedResponse(value, parseToken);
}

export function parseSwapsEnvelope(value: unknown): PaginatedResponse<SwapEvent> {
  return parsePaginatedResponse(value, parseSwapEvent);
}

export function parseTradersEnvelope(value: unknown): PaginatedResponse<TopTrader> {
  return parsePaginatedResponse(value, parseTopTrader);
}

export function parseDailyVolumeEnvelope(
  value: unknown,
): PaginatedResponse<DailySwapVolume> {
  return parsePaginatedResponse(value, parseDailySwapVolume);
}

export function parseFailedTxEnvelope(value: unknown): ApiResponse<FailedTxAnalysis[]> {
  return parseApiResponse(value, (data, path) => {
    if (!Array.isArray(data)) {
      throw new Error(`Invalid contract at ${path}: expected array.`);
    }
    return data.map((item, idx) => parseFailedTxAnalysis(item, `${path}[${idx}]`));
  });
}

// ── Failure-intelligence (detail / filtered list / timeseries, M001) ────────

function parseFailedTransaction(value: unknown, path: string): FailedTransaction {
  const obj = readRecord(value, path);
  return {
    tx_hash: readString(obj.tx_hash, `${path}.tx_hash`),
    error_category: normalizeErrorCategory(
      obj.error_category,
      `${path}.error_category`,
    ),
    revert_reason: readOptionalString(obj.revert_reason, `${path}.revert_reason`),
    failing_function: readOptionalString(
      obj.failing_function,
      `${path}.failing_function`,
    ),
    gas_used: readInteger(obj.gas_used, `${path}.gas_used`),
    timestamp: readIsoDateTime(obj.timestamp, `${path}.timestamp`),
  };
}

function parseTraceLog(value: unknown, path: string): TraceLog {
  const obj = readRecord(value, path);
  return {
    tx_hash: readString(obj.tx_hash, `${path}.tx_hash`),
    call_depth: readInteger(obj.call_depth, `${path}.call_depth`),
    call_type: readString(obj.call_type, `${path}.call_type`),
    from_addr: readString(obj.from_addr, `${path}.from_addr`),
    to_addr: readOptionalString(obj.to_addr, `${path}.to_addr`),
    value: readDecimal(obj.value, `${path}.value`),
    gas_used: readInteger(obj.gas_used, `${path}.gas_used`),
    input: readOptionalString(obj.input, `${path}.input`),
    output: readOptionalString(obj.output, `${path}.output`),
    error: readOptionalString(obj.error, `${path}.error`),
    trace_id: readInteger(obj.trace_id, `${path}.trace_id`),
  };
}

function parseFailedTxDetail(value: unknown, path: string): FailedTxDetail {
  const obj = readRecord(value, path);
  if (!Array.isArray(obj.call_tree)) {
    throw new Error(`Invalid contract at ${path}.call_tree: expected array.`);
  }
  if (typeof obj.call_tree_truncated !== "boolean") {
    throw new Error(
      `Invalid contract at ${path}.call_tree_truncated: expected boolean.`,
    );
  }
  return {
    failed: parseFailedTransaction(obj.failed, `${path}.failed`),
    call_tree: obj.call_tree.map((item, idx) =>
      parseTraceLog(item, `${path}.call_tree[${idx}]`),
    ),
    call_tree_truncated: obj.call_tree_truncated,
  };
}

function parseFailedTxTrendPoint(value: unknown, path: string): FailedTxTrendPoint {
  const obj = readRecord(value, path);
  return {
    bucket: readIsoDateTime(obj.bucket, `${path}.bucket`),
    error_category: normalizeErrorCategory(
      obj.error_category,
      `${path}.error_category`,
    ),
    failure_count: readInteger(obj.failure_count, `${path}.failure_count`),
  };
}

export function parseFailedTxDetailEnvelope(
  value: unknown,
): ApiResponse<FailedTxDetail> {
  return parseApiResponse(value, parseFailedTxDetail);
}

export function parseFailedTxListEnvelope(
  value: unknown,
): TotalPaginatedResponse<FailedTransaction> {
  return parseTotalPaginatedResponse(value, parseFailedTransaction);
}

export function parseFailedTxTimeseriesEnvelope(
  value: unknown,
): ApiResponse<FailedTxTrendPoint[]> {
  return parseApiResponse(value, (data, path) => {
    if (!Array.isArray(data)) {
      throw new Error(`Invalid contract at ${path}: expected array.`);
    }
    return data.map((item, idx) =>
      parseFailedTxTrendPoint(item, `${path}[${idx}]`),
    );
  });
}

// ── Alert subscriptions (S08 + HARDEN2) ─────────────────────────────

function parseAlertSubscription(value: unknown, path: string): AlertSubscription {
  const obj = readRecord(value, path);
  const cat = obj.error_category;
  return {
    subscription_id: readInteger(obj.subscription_id, `${path}.subscription_id`),
    error_category:
      cat == null
        ? null
        : normalizeErrorCategory(cat, `${path}.error_category`),
    to_addr: readOptionalString(obj.to_addr, `${path}.to_addr`),
    webhook_url: readString(obj.webhook_url, `${path}.webhook_url`),
    active: readBoolean(obj.active, `${path}.active`),
    created_at: readIsoDateTime(obj.created_at, `${path}.created_at`),
  };
}

function parseAlertSubscriptionCreated(
  value: unknown,
  path: string,
): AlertSubscriptionCreated {
  const obj = readRecord(value, path);
  const cat = obj.error_category;
  return {
    subscription_id: readInteger(obj.subscription_id, `${path}.subscription_id`),
    error_category:
      cat == null
        ? null
        : normalizeErrorCategory(cat, `${path}.error_category`),
    to_addr: readOptionalString(obj.to_addr, `${path}.to_addr`),
    webhook_url: readString(obj.webhook_url, `${path}.webhook_url`),
    signing_secret: readString(obj.signing_secret, `${path}.signing_secret`),
    active: readBoolean(obj.active, `${path}.active`),
    created_at: readIsoDateTime(obj.created_at, `${path}.created_at`),
  };
}

export function parseAlertSubscriptionListEnvelope(
  value: unknown,
): ApiResponse<AlertSubscription[]> {
  return parseApiResponse(value, (data, path) => {
    if (!Array.isArray(data)) {
      throw new Error(`Invalid contract at ${path}: expected array.`);
    }
    return data.map((item, idx) =>
      parseAlertSubscription(item, `${path}[${idx}]`),
    );
  });
}

export function parseAlertSubscriptionCreatedEnvelope(
  value: unknown,
): ApiResponse<AlertSubscriptionCreated> {
  return parseApiResponse(value, parseAlertSubscriptionCreated);
}
