import { describe, expect, it } from "vite-plus/test";

import {
  OfflinePackBudget,
  PACK_BURST,
  PACK_BYTES_PER_DAY,
  PACK_DOWNLOADS_PER_DAY,
} from "#lib/server/offline-pack-budget.js";

const DAY = 86_400_000;
const MINUTE = 60_000;
const SIZE = 1024;
// Start mid-day so burst windows never straddle midnight in these tests.
const T0 = 10 * DAY + 12 * 3_600_000;

describe("OfflinePackBudget", () => {
  it("blocks the 21st download from one address in a day", () => {
    const budget = new OfflinePackBudget();
    // Spread across minutes so the burst limit never trips first.
    for (let i = 0; i < PACK_DOWNLOADS_PER_DAY; i += 1) {
      expect(budget.charge("203.0.113.1", SIZE, T0 + i * 2 * MINUTE)).toBeNull();
    }
    const blocked = budget.charge("203.0.113.1", SIZE, T0 + 60 * MINUTE);
    expect(blocked?.reason).toBe("daily-downloads");
    expect(blocked?.retryAfterSecs).toBeGreaterThan(0);
  });

  it("enforces the burst of five per minute", () => {
    const budget = new OfflinePackBudget();
    for (let i = 0; i < PACK_BURST; i += 1) {
      expect(budget.charge("203.0.113.2", SIZE, T0)).toBeNull();
    }
    expect(budget.charge("203.0.113.2", SIZE, T0 + 1000)?.reason).toBe("burst");
    expect(budget.charge("203.0.113.2", SIZE, T0 + MINUTE)).toBeNull();
  });

  it("enforces the 100 MB daily byte budget", () => {
    const budget = new OfflinePackBudget();
    const big = PACK_BYTES_PER_DAY / 2;
    expect(budget.charge("203.0.113.3", big, T0)).toBeNull();
    expect(budget.charge("203.0.113.3", big, T0 + MINUTE)).toBeNull();
    expect(budget.charge("203.0.113.3", 1, T0 + 2 * MINUTE)?.reason).toBe("daily-bytes");
  });

  it("keeps addresses independent", () => {
    const budget = new OfflinePackBudget();
    for (let i = 0; i < PACK_BURST; i += 1) budget.charge("203.0.113.4", SIZE, T0);
    expect(budget.charge("203.0.113.4", SIZE, T0)).not.toBeNull();
    expect(budget.charge("203.0.113.5", SIZE, T0)).toBeNull();
  });

  it("resets the daily allowance at the next UTC day", () => {
    const budget = new OfflinePackBudget();
    for (let i = 0; i < PACK_DOWNLOADS_PER_DAY; i += 1) {
      budget.charge("203.0.113.6", SIZE, T0 + i * 2 * MINUTE);
    }
    expect(budget.charge("203.0.113.6", SIZE, T0 + 60 * MINUTE)).not.toBeNull();
    const nextDay = Math.floor(T0 / DAY + 1) * DAY + 1000;
    expect(budget.charge("203.0.113.6", SIZE, nextDay)).toBeNull();
  });
});
