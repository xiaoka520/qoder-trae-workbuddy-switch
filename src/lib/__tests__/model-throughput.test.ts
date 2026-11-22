import { expect, it } from "vitest";

import type { GatewayUsageDim, GatewayUsageDimDay, OutputThroughput } from "@/lib/types";
import { averageTokensPerSecond, dimTotals, mergeDims, modelKey } from "@/lib/usage-ledger";

function day(key: string, throughput?: OutputThroughput): GatewayUsageDimDay {
  return {
    key,
    truncated: false,
    partial: false,
    keysPartial: false,
    accounts: [],
    endpoints: [],
    keys: [],
    models: [{
      model: "m",
      input: 6000,
      output: 600,
      total: 6600,
      requests: 1,
      failed: 0,
      credit: 0,
      throughput,
    }],
  };
}

const dim = (...days: GatewayUsageDimDay[]): GatewayUsageDim => ({
  date: "2026-11-22",
  retentionDays: 90,
  days,
});

it("weights measured requests by duration across days and scopes", () => {
  const first = dim(
    day("2026-11-20"),
    day("2026-11-21", { outputTokens: 100, durationMs: 1_000 }),
    day("2026-11-22", { outputTokens: 17, durationMs: 500 }),
  );
  const second = dim(day("2026-11-21", { outputTokens: 150, durationMs: 4_000 }));
  const window = { kind: "custom" as const, from: "2026-11-20", to: "2026-11-21" };
  const [model] = dimTotals(mergeDims([first, second]), window, modelKey);
  expect(model.tokens).toBe(19_800);
  expect(averageTokensPerSecond(model.throughput)).toBe(50);
  expect(averageTokensPerSecond(dimTotals(first, window, modelKey)[0].throughput)).toBe(100);
});

it("returns an unavailable rate for a model with legacy buckets only", () => {
  const window = { kind: "custom" as const, from: "2026-11-22", to: "2026-11-22" };
  const [model] = dimTotals(dim(day("2026-11-22")), window, modelKey);
  expect(averageTokensPerSecond(model.throughput)).toBeNull();
});
