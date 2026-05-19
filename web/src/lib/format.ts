/**
 * Display formatters. All numeric input is `Decimal` (number | string),
 * coerced through {@link toNumber} — see the precision note in `api/types.ts`.
 */

import type { Decimal, ErrorCategory } from "@/api/types";

/** Coerce a wire `Decimal` to a JS number; non-finite values become 0. */
export function toNumber(value: Decimal | null | undefined): number {
  const n =
    typeof value === "number"
      ? value
      : typeof value === "string"
        ? Number(value.trim())
        : Number(value);
  return Number.isFinite(n) ? n : 0;
}

const compact = new Intl.NumberFormat("en-US", {
  notation: "compact",
  maximumFractionDigits: 2,
});

const grouped = new Intl.NumberFormat("en-US", { maximumFractionDigits: 2 });

/** `1234567 → "1.23M"`. */
export function formatCompact(value: Decimal | null | undefined): string {
  return compact.format(toNumber(value));
}

/** `1234567 → "1,234,567"`. */
export function formatNumber(value: Decimal | null | undefined): string {
  return grouped.format(toNumber(value));
}

/** Compact USD, e.g. `"$1.23M"`. */
export function formatUsd(value: Decimal | null | undefined): string {
  return `$${formatCompact(value)}`;
}

/** `12.3456 → "12.35%"`. */
export function formatPct(value: Decimal | null | undefined): string {
  return `${toNumber(value).toFixed(2)}%`;
}

/** Fee tier in bps → percentage label, e.g. `3000 → "0.30%"`. */
export function feeTierLabel(bps: number): string {
  return `${(bps / 10000).toFixed(2)}%`;
}

/** `0x1234abcd…ef567890` (6 + 4 hex chars). */
export function shortAddress(address: string): string {
  if (!address || address.length <= 12) return address;
  return `${address.slice(0, 6)}…${address.slice(-4)}`;
}

const dateFmt = new Intl.DateTimeFormat("en-US", {
  year: "numeric",
  month: "short",
  day: "2-digit",
});

const dateTimeFmt = new Intl.DateTimeFormat("en-US", {
  month: "short",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
});

export function formatDate(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : dateFmt.format(d);
}

export function formatDateTime(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : dateTimeFmt.format(d);
}

/** Relative time, e.g. `"3d ago"`, `"just now"`. */
export function timeAgo(iso: string): string {
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return iso;
  const secs = Math.round((Date.now() - then) / 1000);
  if (secs < 60) return "just now";
  const mins = Math.round(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.round(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.round(hours / 24);
  if (days < 30) return `${days}d ago`;
  const months = Math.round(days / 30);
  if (months < 12) return `${months}mo ago`;
  return `${Math.round(months / 12)}y ago`;
}

const ERROR_LABELS: Record<ErrorCategory, string> = {
  INSUFFICIENT_BALANCE: "Insufficient balance",
  SLIPPAGE_EXCEEDED: "Slippage exceeded",
  DEADLINE_EXPIRED: "Deadline expired",
  UNAUTHORIZED: "Unauthorized",
  TRANSFER_FAILED: "Transfer failed",
  UNKNOWN: "Unknown",
};

export function errorCategoryLabel(category: ErrorCategory): string {
  return ERROR_LABELS[category] ?? category;
}

/** Stable color per error category for charts/legends. */
const ERROR_COLORS: Record<ErrorCategory, string> = {
  INSUFFICIENT_BALANCE: "#F66061",
  SLIPPAGE_EXCEEDED: "#F4BD50",
  DEADLINE_EXPIRED: "#E88957",
  UNAUTHORIZED: "#C981E6",
  TRANSFER_FAILED: "#E06D6E",
  UNKNOWN: "#888888",
};

export function errorCategoryColor(category: ErrorCategory): string {
  return ERROR_COLORS[category] ?? "#888888";
}

/** Categorical palette for generic charts. */
export const CHART_PALETTE = [
  "#3ECF8E",
  "#F4BD50",
  "#F66061",
  "#888888",
  "#7A7A7A",
  "#5F5F5F",
  "#4A4A4A",
];

/** ISO date `n` days ago at 00:00:00Z (for date-range defaults). */
export function isoDaysAgo(days: number): string {
  const d = new Date();
  d.setUTCDate(d.getUTCDate() - days);
  d.setUTCHours(0, 0, 0, 0);
  return d.toISOString();
}

/** Current instant as ISO string. */
export function isoNow(): string {
  return new Date().toISOString();
}

/** `<input type="date">` value (`YYYY-MM-DD`) from an ISO string. */
export function toDateInput(iso: string): string {
  return iso.slice(0, 10);
}

/** ISO datetime (UTC midnight) from an `<input type="date">` value. */
export function fromDateInput(value: string): string {
  return `${value}T00:00:00Z`;
}
