/**
 * Thin fetch wrapper around the defi-tx-indexer REST API.
 *
 * Resolves the base URL from `VITE_API_BASE_URL` (defaults to the
 * docker-compose published port), serializes query params, and normalizes
 * the `{ "error": string }` error body into a thrown {@link ApiError}.
 *
 * **Admin auth (S18 / M006).** A module-level slot holds the active admin
 * API key. {@link setAdminApiKey} mutates it; {@link apiPost} / {@link apiDelete}
 * read it on every call and attach `Authorization: Bearer <key>` when set.
 * `apiGet` is unauthenticated (M006/D021 — public reads stay embed-friendly).
 *
 * The slot is **memory-only**. The frontend wires it from `<ApiKeyProvider>`
 * (`@/state/apiKey`); refresh clears the React state and `useEffect` clears
 * the slot. D024 — no localStorage, no `NEXT_PUBLIC_*` baked into the bundle.
 */

import type { ApiErrorBody } from "./types";

const BASE_URL: string = (
  import.meta.env.VITE_API_BASE_URL ?? "http://localhost:3000"
).replace(/\/+$/, "");

/**
 * Active admin API key, or `null` when no key has been applied this session.
 * Read by `apiPost` / `apiDelete` on every call. Never persisted.
 */
let _apiKey: string | null = null;

/**
 * Replace the active admin API key. Pass `null` to clear (e.g. when the user
 * clicks "Clear" or the page navigates away). Empty / whitespace strings are
 * stored as `null` so a stray apply doesn't falsely activate write buttons.
 *
 * The frontend's `ApiKeyProvider` calls this from a `useEffect` — most callers
 * shouldn't reach for it directly.
 */
export function setAdminApiKey(key: string | null): void {
  if (key == null) {
    _apiKey = null;
    return;
  }
  const trimmed = key.trim();
  _apiKey = trimmed === "" ? null : trimmed;
}

/** Test-only — peek at the slot to verify auth wiring. Not exported as public API. */
export function _getAdminApiKeyForTests(): string | null {
  return _apiKey;
}

/** Error thrown for any non-2xx API response. */
export class ApiError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }
}

export type QueryParams = Record<
  string,
  string | number | boolean | null | undefined
>;
export type ResponseParser<T> = (body: unknown) => T;

function buildUrl(path: string, params?: QueryParams): string {
  const url = new URL(`${BASE_URL}${path}`);
  if (params) {
    for (const [key, value] of Object.entries(params)) {
      if (value !== undefined && value !== null) {
        url.searchParams.set(key, String(value));
      }
    }
  }
  return url.toString();
}

/**
 * Performs a GET request and optionally runs a parser/normalizer against the
 * JSON body before returning `T`.
 *
 * @throws {ApiError} when the response status is not 2xx.
 */
export async function apiGet<T>(
  path: string,
  params?: QueryParams,
  signal?: AbortSignal,
  parser?: ResponseParser<T>,
): Promise<T> {
  const res = await fetch(buildUrl(path, params), {
    headers: { Accept: "application/json" },
    signal,
  });

  if (!res.ok) {
    let message = res.statusText || `HTTP ${res.status}`;
    try {
      const body = (await res.json()) as ApiErrorBody;
      if (body?.error) message = body.error;
    } catch {
      // Non-JSON error body — fall back to status text.
    }
    throw new ApiError(res.status, message);
  }

  const body = (await res.json()) as unknown;
  return parser ? parser(body) : (body as T);
}

/**
 * Performs a POST request with an optional JSON body and parses the JSON
 * response. Pass `body = undefined` for endpoints that take no body
 * (e.g. `/rotate-secret`). `Content-Type: application/json` is only attached
 * when a body is sent.
 *
 * @throws {ApiError} on any non-2xx response.
 */
export async function apiPost<T>(
  path: string,
  body: unknown,
  signal?: AbortSignal,
  parser?: ResponseParser<T>,
): Promise<T> {
  const headers: Record<string, string> = { Accept: "application/json" };
  if (body !== undefined) {
    headers["Content-Type"] = "application/json";
  }
  // S18 — write routes are admin-only on the server (S16). Attach the key
  // if one is set; otherwise the server will respond with 401 and the page
  // banner surfaces that. Buttons should already be disabled when no key is
  // set, so reaching the server unauthenticated is a programmer/UI bug, not
  // an expected flow.
  if (_apiKey !== null) {
    headers["Authorization"] = `Bearer ${_apiKey}`;
  }
  const init: RequestInit = {
    method: "POST",
    headers,
    signal,
  };
  if (body !== undefined) {
    init.body = JSON.stringify(body);
  }
  const res = await fetch(`${BASE_URL}${path}`, init);

  if (!res.ok) {
    let message = res.statusText || `HTTP ${res.status}`;
    try {
      const errBody = (await res.json()) as ApiErrorBody;
      if (errBody?.error) message = errBody.error;
    } catch {
      // Non-JSON error body — fall back to status text.
    }
    throw new ApiError(res.status, message);
  }

  const respBody = (await res.json()) as unknown;
  return parser ? parser(respBody) : (respBody as T);
}

/**
 * Performs a DELETE request. The backend uses 204 No Content for successful
 * soft-deactivation; this helper expects no body and returns `void`.
 *
 * @throws {ApiError} on any non-2xx response.
 */
export async function apiDelete(path: string, signal?: AbortSignal): Promise<void> {
  const headers: Record<string, string> = { Accept: "application/json" };
  if (_apiKey !== null) {
    headers["Authorization"] = `Bearer ${_apiKey}`;
  }
  const res = await fetch(`${BASE_URL}${path}`, {
    method: "DELETE",
    headers,
    signal,
  });
  if (!res.ok) {
    let message = res.statusText || `HTTP ${res.status}`;
    try {
      const errBody = (await res.json()) as ApiErrorBody;
      if (errBody?.error) message = errBody.error;
    } catch {
      // Non-JSON error body.
    }
    throw new ApiError(res.status, message);
  }
}

export { BASE_URL as API_BASE_URL };
