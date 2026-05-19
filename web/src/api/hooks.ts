/**
 * TanStack Query hooks — one per API endpoint.
 *
 * Single-resource endpoints unwrap the `{ data }` envelope via `select`.
 * Paginated endpoints return the full `PaginatedResponse` so pages can read
 * `pagination`. Query keys mirror the URL + params for predictable caching.
 */

import { useQuery } from "@tanstack/react-query";

import {
  normalizeAddressParam,
  parseBlockEnvelope,
  parseDailyVolumeEnvelope,
  parseFailedTxEnvelope,
  parseLatestBlockEnvelope,
  parsePoolEnvelope,
  parsePoolsEnvelope,
  parsePoolStatsEnvelope,
  parseSwapsEnvelope,
  parseTokensEnvelope,
  parseTradersEnvelope,
} from "./contract";
import { apiGet } from "./client";

const STALE_TIME = 30_000;

export interface ListArgs {
  limit?: number;
  offset?: number;
}

/** Latest indexed block number (`null` if nothing indexed yet). */
export function useLatestBlock() {
  return useQuery({
    queryKey: ["blocks", "latest"],
    queryFn: ({ signal }) =>
      apiGet("/v1/blocks/latest", undefined, signal, parseLatestBlockEnvelope),
    select: (r) => r.data,
    staleTime: STALE_TIME,
  });
}

/** Single block by number. */
export function useBlock(blockNumber: number | undefined) {
  return useQuery({
    queryKey: ["blocks", blockNumber],
    enabled: blockNumber != null,
    queryFn: ({ signal }) =>
      apiGet(`/v1/blocks/${blockNumber}`, undefined, signal, parseBlockEnvelope),
    select: (r) => r.data,
    staleTime: STALE_TIME,
  });
}

/** Paginated pool list. */
export function usePools({ limit = 20, offset = 0 }: ListArgs = {}) {
  return useQuery({
    queryKey: ["pools", { limit, offset }],
    queryFn: ({ signal }) =>
      apiGet("/v1/pools", { limit, offset }, signal, parsePoolsEnvelope),
    staleTime: STALE_TIME,
  });
}

/** Single pool by address. */
export function usePool(address: string | undefined) {
  const normalizedAddress = address ? normalizeAddressParam(address) : undefined;

  return useQuery({
    queryKey: ["pools", normalizedAddress],
    enabled: !!normalizedAddress,
    queryFn: ({ signal }) =>
      apiGet(
        `/v1/pools/${normalizedAddress}`,
        undefined,
        signal,
        parsePoolEnvelope,
      ),
    select: (r) => r.data,
    staleTime: STALE_TIME,
  });
}

/**
 * Pool aggregate stats for a date range.
 *
 * The endpoint requires `from_date` and `to_date` (ISO 8601) — the query
 * stays disabled until both are provided.
 */
export function usePoolStats(
  address: string | undefined,
  fromDate: string,
  toDate: string,
) {
  const normalizedAddress = address ? normalizeAddressParam(address) : undefined;

  return useQuery({
    queryKey: ["pools", normalizedAddress, "stats", { fromDate, toDate }],
    enabled: !!normalizedAddress && !!fromDate && !!toDate,
    queryFn: ({ signal }) =>
      apiGet(
        `/v1/pools/${normalizedAddress}/stats`,
        { from_date: fromDate, to_date: toDate },
        signal,
        parsePoolStatsEnvelope,
      ),
    select: (r) => r.data,
    staleTime: STALE_TIME,
  });
}

/** Paginated token list. */
export function useTokens({ limit = 20, offset = 0 }: ListArgs = {}) {
  return useQuery({
    queryKey: ["tokens", { limit, offset }],
    queryFn: ({ signal }) =>
      apiGet("/v1/tokens", { limit, offset }, signal, parseTokensEnvelope),
    staleTime: STALE_TIME,
  });
}

export interface SwapsArgs extends ListArgs {
  poolAddress?: string;
}

/** Paginated swap events, optionally filtered by pool. */
export function useSwaps({ poolAddress, limit = 20, offset = 0 }: SwapsArgs = {}) {
  const normalizedPoolAddress = poolAddress
    ? normalizeAddressParam(poolAddress)
    : undefined;

  return useQuery({
    queryKey: ["swaps", { poolAddress: normalizedPoolAddress, limit, offset }],
    queryFn: ({ signal }) =>
      apiGet(
        "/v1/swaps",
        { pool_address: normalizedPoolAddress, limit, offset },
        signal,
        parseSwapsEnvelope,
      ),
    staleTime: STALE_TIME,
  });
}

/** Top traders by volume (limit default 10, max 100; offset always 0). */
export function useTopTraders(limit = 10) {
  return useQuery({
    queryKey: ["traders", "top", { limit }],
    queryFn: ({ signal }) =>
      apiGet("/v1/traders/top", { limit }, signal, parseTradersEnvelope),
    staleTime: STALE_TIME,
  });
}

export interface DailyVolumeArgs extends ListArgs {
  poolAddress?: string;
}

/** Daily swap volume (vw_daily_swap_volume), optionally filtered by pool. */
export function useDailyVolume({
  poolAddress,
  limit = 60,
  offset = 0,
}: DailyVolumeArgs = {}) {
  const normalizedPoolAddress = poolAddress
    ? normalizeAddressParam(poolAddress)
    : undefined;

  return useQuery({
    queryKey: [
      "analytics",
      "daily-volume",
      { poolAddress: normalizedPoolAddress, limit, offset },
    ],
    queryFn: ({ signal }) =>
      apiGet(
        "/v1/analytics/daily-volume",
        { pool_address: normalizedPoolAddress, limit, offset },
        signal,
        parseDailyVolumeEnvelope,
      ),
    staleTime: STALE_TIME,
  });
}

/** Failed-tx breakdown by error category (vw_failed_tx_analysis). */
export function useFailedTxAnalysis() {
  return useQuery({
    queryKey: ["analytics", "failed-tx"],
    queryFn: ({ signal }) =>
      apiGet("/v1/analytics/failed-tx", undefined, signal, parseFailedTxEnvelope),
    select: (r) => r.data,
    staleTime: STALE_TIME,
  });
}
