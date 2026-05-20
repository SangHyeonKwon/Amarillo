import { useCallback, useMemo } from "react";
import { useSearchParams } from "react-router-dom";

type StringFilterState = Record<string, string>;

export function parseIntFilter(
  raw: string | undefined,
  fallback: number,
  min: number,
  max: number,
): number {
  if (!raw) return fallback;
  const n = Number(raw);
  if (!Number.isFinite(n)) return fallback;
  const i = Math.trunc(n);
  if (i < min) return min;
  if (i > max) return max;
  return i;
}

export function parseEnumFilter<T extends string>(
  raw: string | undefined,
  allowed: readonly T[],
  fallback: T,
): T {
  if (!raw) return fallback;
  return allowed.includes(raw as T) ? (raw as T) : fallback;
}

/**
 * URL query state helper used by analytics pages so filters are shareable.
 *
 * Empty values are omitted from the URL to keep query strings compact.
 */
export function useUrlFilters<T extends StringFilterState>(
  defaults: T,
): readonly [T, (patch: Partial<T>) => void] {
  const [searchParams, setSearchParams] = useSearchParams();

  const filters = useMemo(() => {
    const next = {} as T;
    for (const [key, fallback] of Object.entries(defaults)) {
      const raw = searchParams.get(key);
      next[key as keyof T] = (raw ?? fallback) as T[keyof T];
    }
    return next;
  }, [defaults, searchParams]);

  const updateFilters = useCallback(
    (patch: Partial<T>) => {
      setSearchParams((prev) => {
        const next = new URLSearchParams(prev);
        for (const [key, value] of Object.entries(patch)) {
          if (value == null || value === "" || value === defaults[key]) {
            next.delete(key);
          } else {
            next.set(key, value);
          }
        }
        return next;
      });
    },
    [defaults, setSearchParams],
  );

  return [filters, updateFilters] as const;
}
