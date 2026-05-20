import type { ReactNode } from "react";

import { ApiError } from "@/api/client";

interface AsyncStateProps {
  isLoading: boolean;
  isError: boolean;
  error?: unknown;
  /** Render the empty state instead of children when true. */
  isEmpty?: boolean;
  emptyLabel?: string;
  children: ReactNode;
}

function errorMessage(error: unknown): string {
  if (error instanceof ApiError) {
    return error.status === 404
      ? "Not found — the indexer may not have data for this resource yet."
      : `API error (${error.status}): ${error.message}`;
  }
  if (error instanceof Error) {
    return `${error.message}. Is the API running at the configured base URL?`;
  }
  return "Unexpected error.";
}

/**
 * Renders loading / error / empty states for a query, or `children` once
 * data is ready. Keeps every page's fetch UX consistent.
 */
export function AsyncState({
  isLoading,
  isError,
  error,
  isEmpty,
  emptyLabel = "No data yet.",
  children,
}: AsyncStateProps) {
  if (isLoading) {
    return (
      <div className="state">
        <div className="spinner" />
        <span>Loading…</span>
      </div>
    );
  }

  if (isError) {
    return (
      <div className="state state--error">
        <span className="state-icon">⚠</span>
        <span>{errorMessage(error)}</span>
      </div>
    );
  }

  if (isEmpty) {
    return (
      <div className="state">
        <span className="state-icon">∅</span>
        <span>{emptyLabel}</span>
      </div>
    );
  }

  return <>{children}</>;
}
