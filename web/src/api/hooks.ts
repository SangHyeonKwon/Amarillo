/**
 * TanStack Query hooks — one per API endpoint.
 *
 * Single-resource endpoints unwrap the `{ data }` envelope via `select`.
 * Paginated endpoints return the full `PaginatedResponse` so pages can read
 * `pagination`. Query keys mirror the URL + params for predictable caching.
 */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  normalizeAddressParam,
  parseAlertSubscriptionCreatedEnvelope,
  parseAlertSubscriptionListEnvelope,
  parseBlockEnvelope,
  parseDailyVolumeEnvelope,
  parseFailedTxByLabelEnvelope,
  parseFailedTxDetailEnvelope,
  parseFailedTxEnvelope,
  parseFailedTxListEnvelope,
  parseFailedTxTimeseriesEnvelope,
  parseLatestBlockEnvelope,
  parsePoolEnvelope,
  parsePoolsEnvelope,
  parsePoolStatsEnvelope,
  parseSwapsEnvelope,
  parseTokensEnvelope,
  parseTradersEnvelope,
} from "./contract";
import { apiDelete, apiGet, apiPost } from "./client";
import type {
  CreateAlertSubscriptionBody,
  ErrorCategory,
  IsoDateTime,
  TimeBucket,
} from "./types";

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

// ── Failure-intelligence (M001) — per-tx detail / filtered list / timeseries ─

/**
 * Single failed-tx diagnosis: decoded revert reason + classified category +
 * flattened call-tree (pre-order DFS by `trace_id`). `tx_hash` is lowercased
 * before the request; the query stays disabled until it's provided.
 */
export function useFailedTxDetail(txHash: string | undefined) {
  const normalized = txHash?.trim().toLowerCase();
  return useQuery({
    queryKey: ["failed-tx", "detail", normalized],
    enabled: !!normalized,
    queryFn: ({ signal }) =>
      apiGet(
        `/v1/failed-tx/${normalized}`,
        undefined,
        signal,
        parseFailedTxDetailEnvelope,
      ),
    select: (r) => r.data,
    staleTime: STALE_TIME,
  });
}

export interface FailedTxListArgs {
  /** Filter by error category (SCREAMING_SNAKE_CASE on the wire). */
  category?: ErrorCategory;
  /** Inclusive lower bound, RFC3339 (e.g. `2024-09-01T00:00:00Z`). */
  from?: IsoDateTime;
  /** Inclusive upper bound, RFC3339. */
  to?: IsoDateTime;
  limit?: number;
  offset?: number;
}

/**
 * Failed-tx list with **filter-adjusted `total`** (D005). Returns the full
 * `TotalPaginatedResponse` so the page can render "N of TOTAL".
 */
export function useFailedTxList({
  category,
  from,
  to,
  limit = 20,
  offset = 0,
}: FailedTxListArgs = {}) {
  return useQuery({
    queryKey: ["failed-tx", "list", { category, from, to, limit, offset }],
    queryFn: ({ signal }) =>
      apiGet(
        "/v1/failed-tx",
        { category, from, to, limit, offset },
        signal,
        parseFailedTxListEnvelope,
      ),
    staleTime: STALE_TIME,
  });
}

export interface FailedTxTimeseriesArgs {
  /** Bucket size: `hour` / `day` (default) / `week`. */
  interval?: TimeBucket;
  from?: IsoDateTime;
  to?: IsoDateTime;
}

/** Failed-tx counts bucketed by interval and category (S03 timeseries). */
export function useFailedTxTimeseries({
  interval = "day",
  from,
  to,
}: FailedTxTimeseriesArgs = {}) {
  return useQuery({
    queryKey: ["analytics", "failed-tx", "timeseries", { interval, from, to }],
    queryFn: ({ signal }) =>
      apiGet(
        "/v1/analytics/failed-tx/timeseries",
        { interval, from, to },
        signal,
        parseFailedTxTimeseriesEnvelope,
      ),
    select: (r) => r.data,
    staleTime: STALE_TIME,
  });
}

export interface FailedTxByLabelArgs {
  from?: IsoDateTime;
  to?: IsoDateTime;
  /** Tenancy filter — empty/absent = match all labels. */
  owner?: string;
  limit?: number;
}

/**
 * Failed-tx distribution by labeled contract (S09 / M003) — the demo of
 * `failed_transaction × transaction × contract_label`. Returns one row per
 * (label, address) with `total_failures` and a `by_category` map.
 */
export function useFailedTxByLabel({
  from,
  to,
  owner,
  limit = 50,
}: FailedTxByLabelArgs = {}) {
  return useQuery({
    queryKey: ["analytics", "failed-tx", "by-label", { from, to, owner, limit }],
    queryFn: ({ signal }) =>
      apiGet(
        "/v1/analytics/failed-tx/by-label",
        { from, to, owner, limit },
        signal,
        parseFailedTxByLabelEnvelope,
      ),
    select: (r) => r.data,
    staleTime: STALE_TIME,
  });
}

// ── Alert subscriptions (S08 + HARDEN2) ─────────────────────────────

export interface AlertSubscriptionsListArgs {
  limit?: number;
}

/**
 * Active *and* inactive alert subscriptions, newest first. The list never
 * contains `signing_secret` — the backend serde-skips it and the parser does
 * not look it up. The secret is only available on the create/rotate mutation
 * response, **once**.
 */
export function useAlertSubscriptions({
  limit = 100,
}: AlertSubscriptionsListArgs = {}) {
  return useQuery({
    queryKey: ["alert-subscriptions", { limit }],
    queryFn: ({ signal }) =>
      apiGet(
        "/v1/alert-subscriptions",
        { limit },
        signal,
        parseAlertSubscriptionListEnvelope,
      ),
    select: (r) => r.data,
    staleTime: STALE_TIME,
  });
}

/**
 * Create a new subscription. The mutation response contains the **one-time
 * signing_secret**; the UI must surface it in a reveal modal and then call
 * `mutation.reset()` so the value isn't retained in any cache after the
 * modal closes.
 */
export function useCreateAlertSubscription() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateAlertSubscriptionBody) =>
      apiPost(
        "/v1/alert-subscriptions",
        body,
        undefined,
        parseAlertSubscriptionCreatedEnvelope,
      ),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["alert-subscriptions"] });
    },
  });
}

/**
 * Rotate an existing subscription's signing secret. Same one-time-reveal
 * contract as creation — surface in a modal, then reset.
 */
export function useRotateAlertSubscription() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (subscriptionId: number) =>
      apiPost(
        `/v1/alert-subscriptions/${subscriptionId}/rotate-secret`,
        undefined,
        undefined,
        parseAlertSubscriptionCreatedEnvelope,
      ),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["alert-subscriptions"] });
    },
  });
}

/** Soft-deactivate a subscription. Backend returns 204; we map to `void`. */
export function useDeactivateAlertSubscription() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (subscriptionId: number) =>
      apiDelete(`/v1/alert-subscriptions/${subscriptionId}`),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["alert-subscriptions"] });
    },
  });
}
