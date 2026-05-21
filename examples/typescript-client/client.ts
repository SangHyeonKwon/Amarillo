/**
 * Amarillo — minimal TypeScript example client.
 *
 * Self-contained: depends only on Node 18+ `fetch` and `node:crypto`. There
 * is no `package.json` — copy `client.ts` (and optionally `examples.ts`) into
 * your project, point a TypeScript build at them, and you're done (D017).
 *
 * Wire types and endpoint paths mirror `crates/api/src/routes/*.rs`. Field
 * shapes are documented next to each interface; the response contract is
 * additive (D004 / D014) — new fields don't break existing readers.
 */
import { createHmac, timingSafeEqual } from "node:crypto";

// ── Wire types (mirror `crates/db/src/models.rs` Serialize output) ────────

/** `failed_transaction.error_category` — SCREAMING_SNAKE_CASE on the wire. */
export type ErrorCategory =
  | "INSUFFICIENT_BALANCE"
  | "SLIPPAGE_EXCEEDED"
  | "DEADLINE_EXPIRED"
  | "UNAUTHORIZED"
  | "TRANSFER_FAILED"
  | "UNKNOWN";

/** Single `failed_transaction` row. */
export interface FailedTransaction {
  tx_hash: string;
  error_category: ErrorCategory;
  revert_reason: string | null;
  failing_function: string | null;
  gas_used: number;
  timestamp: string;
}

/** Flattened `trace_log` frame — pre-order DFS by `trace_id`. */
export interface TraceLog {
  tx_hash: string;
  call_depth: number;
  call_type: string;
  from_addr: string;
  to_addr: string | null;
  value: string;
  gas_used: number;
  input: string | null;
  output: string | null;
  error: string | null;
  trace_id: number;
}

/** S11 — 4-byte selector resolved against the self-owned ABI seed. */
export interface DecodedFunction {
  selector: string;
  name: string;
  signature: string;
  source: string | null;
}

/** S12 — category-level diagnosis: message + recommended_action. */
export interface Diagnosis {
  message: string;
  recommended_action: string | null;
  source: string | null;
}

/** `GET /v1/failed-tx/{tx_hash}` payload. */
export interface FailedTxDetail {
  failed: FailedTransaction;
  call_tree: TraceLog[];
  call_tree_truncated: boolean;
  /** S10 — first trace frame whose `error` is non-null; explicit null otherwise. */
  root_cause: TraceLog | null;
  /** S11 — name/signature for `failed.failing_function`; explicit null when unmapped. */
  failing_function_decoded: DecodedFunction | null;
  /** S12 — message + recommended_action for `failed.error_category`. */
  diagnosis: Diagnosis | null;
}

export interface PaginationMeta {
  limit: number;
  offset: number;
  count: number;
  total: number;
}

export interface FailedTxListResponse {
  data: FailedTransaction[];
  pagination: PaginationMeta;
}

export interface FailedTxTrendPoint {
  bucket: string;
  error_category: ErrorCategory;
  failure_count: number;
}

export interface FailedTxByLabelPoint {
  label: string;
  address: string;
  total_failures: number;
  by_category: Record<string, number>;
}

export interface AlertSubscription {
  subscription_id: number;
  error_category: ErrorCategory | null;
  to_addr: string | null;
  webhook_url: string;
  active: boolean;
  created_at: string;
}

/**
 * `signing_secret` is revealed exactly once — at creation or rotation. The
 * `GET /v1/alert-subscriptions` list endpoint never returns it.
 */
export interface AlertSubscriptionCreated extends AlertSubscription {
  signing_secret: string;
}

interface ApiErrorBody {
  error: string;
}

// ── Client ─────────────────────────────────────────────────────────────────

export interface FailedTxFilter {
  category?: ErrorCategory;
  from?: string;
  to?: string;
  limit?: number;
  offset?: number;
}

export interface FailedTxTrendFilter {
  interval?: "hour" | "day" | "week";
  from?: string;
  to?: string;
}

export interface ByLabelFilter {
  from?: string;
  to?: string;
  owner?: string;
  limit?: number;
}

export interface CreateAlertBody {
  webhook_url: string;
  error_category?: ErrorCategory;
  to_addr?: string;
}

/** HTTP error from the Amarillo API — `status` is the HTTP code. */
export class AmarilloError extends Error {
  constructor(
    public readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = "AmarilloError";
  }
}

/** Tiny fetch-based client. Drop-in: `new AmarilloClient("http://localhost:3000")`. */
export class AmarilloClient {
  private readonly baseUrl: string;

  constructor(baseUrl: string) {
    this.baseUrl = baseUrl.endsWith("/") ? baseUrl.slice(0, -1) : baseUrl;
  }

  private async request<T>(
    method: string,
    path: string,
    body?: unknown,
  ): Promise<T> {
    const headers: Record<string, string> = {};
    let bodyText: string | undefined;
    if (body !== undefined) {
      headers["Content-Type"] = "application/json";
      bodyText = JSON.stringify(body);
    }
    const res = await fetch(`${this.baseUrl}${path}`, {
      method,
      headers,
      body: bodyText,
    });
    if (!res.ok) {
      const text = await res.text();
      let msg = text;
      try {
        const j = JSON.parse(text) as ApiErrorBody;
        msg = j.error ?? text;
      } catch {
        // not JSON; surface raw text
      }
      throw new AmarilloError(res.status, msg);
    }
    if (res.status === 204) {
      return undefined as T;
    }
    return (await res.json()) as T;
  }

  /** `GET /v1/failed-tx/{tx_hash}` — single-tx diagnosis (root_cause + decoded + diagnosis). */
  async getFailedTx(txHash: string): Promise<FailedTxDetail> {
    const r = await this.request<{ data: FailedTxDetail }>(
      "GET",
      `/v1/failed-tx/${txHash}`,
    );
    return r.data;
  }

  /** `GET /v1/failed-tx` — filtered list with accurate total. */
  async listFailedTx(filter: FailedTxFilter = {}): Promise<FailedTxListResponse> {
    const qs = new URLSearchParams();
    if (filter.category) qs.set("category", filter.category);
    if (filter.from) qs.set("from", filter.from);
    if (filter.to) qs.set("to", filter.to);
    if (filter.limit != null) qs.set("limit", String(filter.limit));
    if (filter.offset != null) qs.set("offset", String(filter.offset));
    return this.request("GET", `/v1/failed-tx?${qs}`);
  }

  /** `GET /v1/analytics/failed-tx/timeseries` — bucketed trend by category. */
  async getFailedTxTimeseries(
    filter: FailedTxTrendFilter = {},
  ): Promise<FailedTxTrendPoint[]> {
    const qs = new URLSearchParams();
    if (filter.interval) qs.set("interval", filter.interval);
    if (filter.from) qs.set("from", filter.from);
    if (filter.to) qs.set("to", filter.to);
    const r = await this.request<{ data: FailedTxTrendPoint[] }>(
      "GET",
      `/v1/analytics/failed-tx/timeseries?${qs}`,
    );
    return r.data;
  }

  /** `GET /v1/analytics/failed-tx/by-label` — failures grouped by labeled contract. */
  async getFailedTxByLabel(
    filter: ByLabelFilter = {},
  ): Promise<FailedTxByLabelPoint[]> {
    const qs = new URLSearchParams();
    if (filter.from) qs.set("from", filter.from);
    if (filter.to) qs.set("to", filter.to);
    if (filter.owner) qs.set("owner", filter.owner);
    if (filter.limit != null) qs.set("limit", String(filter.limit));
    const r = await this.request<{ data: FailedTxByLabelPoint[] }>(
      "GET",
      `/v1/analytics/failed-tx/by-label?${qs}`,
    );
    return r.data;
  }

  /** `POST /v1/alert-subscriptions` — signing_secret is revealed exactly once. */
  async createAlertSubscription(
    body: CreateAlertBody,
  ): Promise<AlertSubscriptionCreated> {
    const r = await this.request<{ data: AlertSubscriptionCreated }>(
      "POST",
      `/v1/alert-subscriptions`,
      body,
    );
    return r.data;
  }

  /** `GET /v1/alert-subscriptions` — never returns signing_secret. */
  async listAlertSubscriptions(): Promise<AlertSubscription[]> {
    const r = await this.request<{ data: AlertSubscription[] }>(
      "GET",
      `/v1/alert-subscriptions`,
    );
    return r.data;
  }

  /** `DELETE /v1/alert-subscriptions/{id}` — soft-deactivate. */
  async deleteAlertSubscription(id: number): Promise<void> {
    await this.request<void>("DELETE", `/v1/alert-subscriptions/${id}`);
  }

  /** `POST /v1/alert-subscriptions/{id}/rotate-secret` — same one-time secret contract. */
  async rotateAlertSecret(id: number): Promise<AlertSubscriptionCreated> {
    const r = await this.request<{ data: AlertSubscriptionCreated }>(
      "POST",
      `/v1/alert-subscriptions/${id}/rotate-secret`,
    );
    return r.data;
  }
}

// ── Webhook receiver: HMAC-SHA256 signature verification ──────────────────

/**
 * Verifies the `X-Amarillo-Signature` header against the raw POST body.
 *
 * The dispatcher signs the **raw request body bytes** with HMAC-SHA256,
 * keyed by the **32 bytes obtained by hex-decoding** `signing_secret`
 * (the secret is 64 hex chars). The header value is `"sha256=<hex>"`.
 *
 * This mirrors `crates/indexer/src/alerts.rs::sign_payload` /
 * `post_signed`. **Use `timingSafeEqual`** (this function does) to avoid
 * leaking timing information to attackers.
 *
 * @param rawBody The raw request body **before** JSON parsing.
 * @param signatureHeader The exact value of the `X-Amarillo-Signature` header.
 * @param signingSecret The 64-hex-char secret stored at sub creation time.
 * @returns true if the signature is valid.
 */
export function verifyAlertSignature(
  rawBody: string | Uint8Array,
  signatureHeader: string | undefined,
  signingSecret: string,
): boolean {
  if (!signatureHeader) {
    return false;
  }
  // Header is "sha256=<hex>"; reject anything else.
  const prefix = "sha256=";
  if (!signatureHeader.startsWith(prefix)) {
    return false;
  }
  const sigHex = signatureHeader.slice(prefix.length);
  if (!/^[0-9a-fA-F]+$/.test(sigHex)) {
    return false;
  }
  if (!/^[0-9a-fA-F]{64}$/.test(signingSecret)) {
    return false;
  }
  const key = Buffer.from(signingSecret, "hex");
  const mac = createHmac("sha256", key);
  mac.update(rawBody);
  const expected = mac.digest();
  const actual = Buffer.from(sigHex, "hex");
  if (actual.length !== expected.length) {
    return false;
  }
  return timingSafeEqual(actual, expected);
}
