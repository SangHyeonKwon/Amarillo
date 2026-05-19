/**
 * Thin fetch wrapper around the defi-tx-indexer REST API.
 *
 * Resolves the base URL from `VITE_API_BASE_URL` (defaults to the
 * docker-compose published port), serializes query params, and normalizes
 * the `{ "error": string }` error body into a thrown {@link ApiError}.
 */

import type { ApiErrorBody } from "./types";

const BASE_URL: string = (
  import.meta.env.VITE_API_BASE_URL ?? "http://localhost:3000"
).replace(/\/+$/, "");

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

export { BASE_URL as API_BASE_URL };
