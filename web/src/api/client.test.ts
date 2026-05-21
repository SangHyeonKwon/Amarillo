/**
 * Unit tests for `@/api/client` — admin auth header wiring (S18 / M006).
 *
 * Mocks `fetch` to inspect the request headers actually emitted by `apiPost`
 * / `apiDelete` / `apiGet`, then asserts:
 *
 * - `apiPost` / `apiDelete` attach `Authorization: Bearer <key>` when a key
 *   is set via `setAdminApiKey`, and omit it when the slot is `null` or empty.
 * - `apiGet` *never* attaches the header — public reads stay embed-friendly
 *   (M006/D021/X policy).
 * - The slot is module-level mutable: setting `null` after a key wipes the
 *   header on the very next call.
 *
 * Tests share module state, so each test resets the slot in `beforeEach`.
 */
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  _getAdminApiKeyForTests,
  apiDelete,
  apiGet,
  apiPost,
  setAdminApiKey,
} from "./client";

function mockOkResponse(body: unknown = {}): Response {
  return {
    ok: true,
    status: 200,
    statusText: "OK",
    json: () => Promise.resolve(body),
  } as unknown as Response;
}

function mockNoContent(): Response {
  return {
    ok: true,
    status: 204,
    statusText: "No Content",
    json: () => Promise.resolve(null),
  } as unknown as Response;
}

/** Pull the headers off a fetch mock call as a plain record for easy lookup. */
function headersFromCall(mock: ReturnType<typeof vi.fn>): Record<string, string> {
  const [, init] = mock.mock.calls[0] ?? [];
  return (init?.headers ?? {}) as Record<string, string>;
}

describe("api/client — admin auth wiring (S18/M006)", () => {
  beforeEach(() => {
    setAdminApiKey(null);
    vi.restoreAllMocks();
  });

  it("setAdminApiKey stores trimmed non-empty strings", () => {
    setAdminApiKey("  key-with-spaces  ");
    expect(_getAdminApiKeyForTests()).toBe("key-with-spaces");
  });

  it("setAdminApiKey clamps empty / whitespace-only inputs to null", () => {
    setAdminApiKey("");
    expect(_getAdminApiKeyForTests()).toBeNull();
    setAdminApiKey("   ");
    expect(_getAdminApiKeyForTests()).toBeNull();
  });

  it("apiPost attaches Authorization: Bearer <key> when a key is set", async () => {
    setAdminApiKey("test-key-32-bytes");
    const fetchMock = vi.fn().mockResolvedValue(mockOkResponse({ data: {} }));
    vi.stubGlobal("fetch", fetchMock);

    await apiPost("/v1/alert-subscriptions", { webhook_url: "https://x" });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const headers = headersFromCall(fetchMock);
    expect(headers["Authorization"]).toBe("Bearer test-key-32-bytes");
    expect(headers["Content-Type"]).toBe("application/json");
  });

  it("apiPost omits Authorization when no key is set", async () => {
    const fetchMock = vi.fn().mockResolvedValue(mockOkResponse({ data: {} }));
    vi.stubGlobal("fetch", fetchMock);

    await apiPost("/v1/alert-subscriptions", { webhook_url: "https://x" });

    const headers = headersFromCall(fetchMock);
    expect(headers["Authorization"]).toBeUndefined();
  });

  it("apiDelete attaches Authorization: Bearer <key> when a key is set", async () => {
    setAdminApiKey("delete-key");
    const fetchMock = vi.fn().mockResolvedValue(mockNoContent());
    vi.stubGlobal("fetch", fetchMock);

    await apiDelete("/v1/alert-subscriptions/123");

    const headers = headersFromCall(fetchMock);
    expect(headers["Authorization"]).toBe("Bearer delete-key");
  });

  it("apiDelete omits Authorization when no key is set", async () => {
    const fetchMock = vi.fn().mockResolvedValue(mockNoContent());
    vi.stubGlobal("fetch", fetchMock);

    await apiDelete("/v1/alert-subscriptions/123");

    const headers = headersFromCall(fetchMock);
    expect(headers["Authorization"]).toBeUndefined();
  });

  it("apiGet never attaches Authorization (D021/X — public reads stay embed-friendly)", async () => {
    setAdminApiKey("should-not-leak-into-get");
    const fetchMock = vi.fn().mockResolvedValue(mockOkResponse({ data: {} }));
    vi.stubGlobal("fetch", fetchMock);

    await apiGet("/v1/failed-tx/0xabc");

    const headers = headersFromCall(fetchMock);
    expect(headers["Authorization"]).toBeUndefined();
  });

  it("setAdminApiKey(null) after a key wipes the header on the next call", async () => {
    setAdminApiKey("first-key");
    const post1 = vi.fn().mockResolvedValue(mockOkResponse({ data: {} }));
    vi.stubGlobal("fetch", post1);
    await apiPost("/v1/alert-subscriptions", {});
    expect(headersFromCall(post1)["Authorization"]).toBe("Bearer first-key");

    setAdminApiKey(null);
    const post2 = vi.fn().mockResolvedValue(mockOkResponse({ data: {} }));
    vi.stubGlobal("fetch", post2);
    await apiPost("/v1/alert-subscriptions", {});
    expect(headersFromCall(post2)["Authorization"]).toBeUndefined();
  });
});
