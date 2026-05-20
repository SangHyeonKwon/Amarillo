import { describe, expect, it } from "vitest";

import {
  normalizeAddressParam,
  parseFailedTxDetailEnvelope,
  parseFailedTxEnvelope,
  parseFailedTxListEnvelope,
  parseFailedTxTimeseriesEnvelope,
  parseLatestBlockEnvelope,
  parsePoolsEnvelope,
} from "@/api/contract";

describe("api/contract", () => {
  it("normalizes lowercase address params", () => {
    expect(normalizeAddressParam(" 0xAbCd ")).toBe("0xabcd");
  });

  it("parses latest block envelope with nullable data", () => {
    expect(parseLatestBlockEnvelope({ data: null }).data).toBeNull();
    expect(parseLatestBlockEnvelope({ data: 123 }).data).toBe(123);
  });

  it("parses failed-tx envelope with enum variants and decimal normalization", () => {
    const parsed = parseFailedTxEnvelope({
      data: [
        {
          error_category: "INSUFFICIENT_BALANCE",
          failure_count: 12,
          avg_gas_wasted: "123.45",
          pct_of_total: 33.3,
          most_recent_failure: "2025-01-01T00:00:00Z",
        },
        {
          error_category: { SlippageExceeded: null },
          failure_count: 3,
          avg_gas_wasted: 75,
          pct_of_total: "10.5",
          most_recent_failure: "2025-01-02T00:00:00Z",
        },
      ],
    });

    expect(parsed.data[0].error_category).toBe("INSUFFICIENT_BALANCE");
    expect(parsed.data[0].pct_of_total).toBe("33.3");
    expect(parsed.data[1].error_category).toBe("SLIPPAGE_EXCEEDED");
    expect(parsed.data[1].avg_gas_wasted).toBe("75");
  });

  it("parses paginated pools and normalizes addresses", () => {
    const parsed = parsePoolsEnvelope({
      data: [
        {
          pool_address: "0xABCD",
          pair_name: "WETH/USDC",
          token0_address: "0xAAAA",
          token1_address: "0xBBBB",
          fee_tier: 3000,
          created_at: "2025-01-01T00:00:00Z",
        },
      ],
      pagination: {
        limit: 20,
        offset: 0,
        count: 1,
      },
    });

    expect(parsed.data[0].pool_address).toBe("0xabcd");
    expect(parsed.pagination.count).toBe(1);
  });

  // ── FE-WIRE-T01: failure-intelligence parsers ─────────────────

  it("parses single failed-tx detail with call_tree + truncated flag", () => {
    const parsed = parseFailedTxDetailEnvelope({
      data: {
        failed: {
          tx_hash: "0xdead",
          // serde default = PascalCase variant; parser must canonicalize.
          error_category: "SlippageExceeded",
          revert_reason: "Too little received",
          failing_function: "0xa9059cbb",
          gas_used: 21000,
          timestamp: "2025-01-01T00:00:00Z",
        },
        call_tree: [
          {
            tx_hash: "0xdead",
            call_depth: 0,
            call_type: "CALL",
            from_addr: "0x01",
            to_addr: "0x02",
            value: "0",
            gas_used: 21000,
            input: null,
            output: null,
            error: null,
            trace_id: 1,
          },
          {
            tx_hash: "0xdead",
            call_depth: 1,
            call_type: "STATICCALL",
            from_addr: "0x02",
            to_addr: null,
            value: "0",
            gas_used: 500,
            input: "0xdeadbeef",
            output: null,
            error: "Too little received",
            trace_id: 2,
          },
        ],
        call_tree_truncated: true,
      },
    });

    expect(parsed.data.failed.error_category).toBe("SLIPPAGE_EXCEEDED");
    expect(parsed.data.failed.revert_reason).toBe("Too little received");
    expect(parsed.data.call_tree).toHaveLength(2);
    // Pre-order DFS invariant: trace_id strictly ascending.
    expect(parsed.data.call_tree[0].trace_id).toBe(1);
    expect(parsed.data.call_tree[1].trace_id).toBe(2);
    expect(parsed.data.call_tree[1].to_addr).toBeNull();
    expect(parsed.data.call_tree_truncated).toBe(true);
  });

  it("parses failed-tx list with TotalPaginatedResponse (filter-adjusted total)", () => {
    const parsed = parseFailedTxListEnvelope({
      data: [
        {
          tx_hash: "0xa1",
          // SCREAMING_SNAKE wire form also accepted (D002 asymmetry).
          error_category: "INSUFFICIENT_BALANCE",
          revert_reason: null,
          failing_function: null,
          gas_used: 30000,
          timestamp: "2025-01-01T00:00:00Z",
        },
        {
          tx_hash: "0xa2",
          error_category: { Unknown: null },
          revert_reason: null,
          failing_function: null,
          gas_used: 21000,
          timestamp: "2025-01-02T00:00:00Z",
        },
      ],
      pagination: { limit: 20, offset: 0, count: 2, total: 42 },
    });

    expect(parsed.data).toHaveLength(2);
    expect(parsed.data[0].error_category).toBe("INSUFFICIENT_BALANCE");
    expect(parsed.data[1].error_category).toBe("UNKNOWN");
    expect(parsed.pagination.total).toBe(42);
    expect(parsed.pagination.count).toBe(2);
  });

  it("parses failed-tx timeseries with bucket + category + count", () => {
    const parsed = parseFailedTxTimeseriesEnvelope({
      data: [
        {
          bucket: "2025-01-01T00:00:00Z",
          error_category: "Unknown",
          failure_count: 5,
        },
        {
          bucket: "2025-01-02T00:00:00Z",
          error_category: "InsufficientBalance",
          failure_count: 3,
        },
      ],
    });

    expect(parsed.data).toHaveLength(2);
    expect(parsed.data[0].error_category).toBe("UNKNOWN");
    expect(parsed.data[1].error_category).toBe("INSUFFICIENT_BALANCE");
    expect(parsed.data[1].failure_count).toBe(3);
  });

  it("failed-tx detail throws on malformed shape", () => {
    // missing call_tree_truncated
    expect(() =>
      parseFailedTxDetailEnvelope({
        data: {
          failed: {
            tx_hash: "0x1",
            error_category: "Unknown",
            revert_reason: null,
            failing_function: null,
            gas_used: 1,
            timestamp: "2025-01-01T00:00:00Z",
          },
          call_tree: [],
        },
      }),
    ).toThrow(/call_tree_truncated/);

    // call_tree not an array
    expect(() =>
      parseFailedTxDetailEnvelope({
        data: {
          failed: {
            tx_hash: "0x1",
            error_category: "Unknown",
            revert_reason: null,
            failing_function: null,
            gas_used: 1,
            timestamp: "2025-01-01T00:00:00Z",
          },
          call_tree: "not-array",
          call_tree_truncated: false,
        },
      }),
    ).toThrow(/call_tree/);
  });
});
