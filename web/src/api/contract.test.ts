import { describe, expect, it } from "vitest";

import {
  normalizeAddressParam,
  parseFailedTxEnvelope,
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
});
