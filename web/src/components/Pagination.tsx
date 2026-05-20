interface PaginationProps {
  offset: number;
  limit: number;
  /** Number of items returned for the current page. */
  count: number;
  onChange: (offset: number) => void;
}

/**
 * Offset/limit pager. The API returns no total count, so "next" is enabled
 * whenever a full page came back (a heuristic — there may be a final empty
 * page, which the empty state then handles gracefully).
 */
export function Pagination({ offset, limit, count, onChange }: PaginationProps) {
  const page = Math.floor(offset / limit) + 1;
  const hasPrev = offset > 0;
  const hasNext = count >= limit;

  if (!hasPrev && !hasNext) return null;

  return (
    <div className="pager">
      <span>
        Page {page} · {count} shown
      </span>
      <button
        className="btn"
        disabled={!hasPrev}
        onClick={() => onChange(Math.max(0, offset - limit))}
      >
        ← Prev
      </button>
      <button
        className="btn"
        disabled={!hasNext}
        onClick={() => onChange(offset + limit)}
      >
        Next →
      </button>
    </div>
  );
}
