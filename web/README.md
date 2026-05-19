# defi-tx-indexer · web

Analytics dashboard for the [defi-tx-indexer](../README.md) REST API.

A read-only single-page app that visualizes indexed Uniswap V3 activity —
including **failed-transaction analysis decoded from tx traces**, the data
point that distinguishes this project from generic on-chain SQL platforms.

## Stack

| Layer | Technology |
|-------|-----------|
| Build | Vite 5 |
| UI | React 18 + TypeScript (strict) |
| Data | TanStack Query v5 (against the REST API) |
| Charts | Recharts |
| Routing | React Router v6 |
| Styling | Plain CSS (dark theme, CSS variables) — no UI framework |

The app is purely a client of the REST API; it adds **no** backend code and
keeps the API contract mirrored in `src/api/types.ts`.

## Pages

| Route | Purpose |
|-------|---------|
| `/` | Overview — KPIs, daily swap volume, recent swaps |
| `/pools` | Paginated pool list → pool detail (stats, volume, swaps) |
| `/failed-tx` | ⭐ Failed-tx analysis with category/sort/lookback URL filters + time-axis signal |
| `/traders` | Top traders with URL-synced `limit`, `label`, `sort` filters |

## Frontend architecture

- **Contract layer**: `src/api/contract.ts` performs runtime parsing/normalization
  for all API responses (`decimal`, enum variants, addresses, timestamps).
- **Data ownership**: `src/api/hooks.ts` owns server-state fetching/cache keys;
  pages consume typed normalized data only.
- **Filter state**: `src/lib/urlFilters.ts` keeps analytics filters in URL query
  params for reproducible, shareable views.
- **UI boundaries**: page modules own feature composition; reusable primitives
  stay in `src/components` / `src/lib`.

### URL filter conventions

- `failed-tx`: `category`, `sort`, `window`
- `traders`: `limit`, `label`, `sort`
- `pool-detail`: `from`, `to`, `swaps`
- default values are omitted from URL for compact links.

## Develop

```bash
cd web
npm install
cp .env.example .env          # set VITE_API_BASE_URL (default http://localhost:3000)
npm run dev                   # http://localhost:5173
```

The API must be running and reachable from the browser. With the repo's
`docker compose up -d`, the default `http://localhost:3000` works as-is.

```bash
npm run typecheck             # tsc --noEmit (strict)
npm run build                 # typecheck + production build to dist/
npm run preview               # serve the production build locally
```

## Docker

Built and served by the repo-root `docker-compose.yml` as the `web` service
(nginx on port **8080**). `VITE_API_BASE_URL` is baked at image build time via
a build arg — it must point at a URL the browser can reach (not a
docker-internal hostname), so it defaults to `http://localhost:3000`.
`VITE_APP_BASE_PATH` controls browser/router base path (`/` by default).

```bash
docker compose up -d          # postgres + api + web
# dashboard → http://localhost:8080
```

## API contract

`src/api/types.ts` mirrors `crates/db/src/models.rs` and the response
envelopes in `crates/api/src/response.rs`, while `src/api/contract.ts`
enforces runtime parsing. If the API changes, update both. `BigDecimal` values
are normalized as decimal strings at the contract boundary; display formatting
still coerces to `number`, so chart-level precision is intentionally
approximate.

## API extension candidates (insight roadmap)

- Failed transaction raw event list endpoint with `category`, `from`, `to`,
  `limit`, `offset`.
- Failed transaction detail endpoint by `tx_hash` (decoded reason + context).
- Analytics endpoints with explicit `total` for pagination metadata.
