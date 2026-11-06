import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: false, dev: false, building: false }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "",
  PUBLIC_FCM_VAPID_KEY: "",
  PUBLIC_QURAN_API_BASE: "",
  PUBLIC_ENV: "local",
}));

import { dailyGlobalIndex, secondsUntilUtcMidnight, utcDateKey } from "#lib/home/daily-ayah.js";

describe("dailyGlobalIndex (mirrors quran_v1 random_global)", () => {
  // Vectors computed from the Rust formula: ((days * 1103 + 4177) mod 6236) + 1.
  it.each([
    ["1970-01-01", 4178],
    ["1970-01-02", 5281],
    ["2026-11-06", 939],
    ["2026-11-07", 2042],
  ])("maps %s to global ayah %i", (date, expected) => {
    expect(dailyGlobalIndex(date)).toBe(expected);
  });

  it("stays inside the 6236-verse range over many days, including before 1970", () => {
    for (let day = -3000; day < 30000; day += 7) {
      const index = dailyGlobalIndex(utcDateKey(new Date(Date.UTC(1970, 0, 1) + day * 86_400_000)));
      expect(index).toBeGreaterThanOrEqual(1);
      expect(index).toBeLessThanOrEqual(6236);
    }
  });

  it("gives consecutive days different verses", () => {
    expect(dailyGlobalIndex("2026-11-06")).not.toBe(dailyGlobalIndex("2026-11-07"));
  });
});

describe("utcDateKey", () => {
  it("names the UTC day, so 23:59 UTC and 00:00 UTC split on the boundary", () => {
    expect(utcDateKey(new Date("2026-11-06T23:59:59Z"))).toBe("2026-11-06");
    expect(utcDateKey(new Date("2026-11-07T00:00:00Z"))).toBe("2026-11-07");
  });
});

describe("secondsUntilUtcMidnight", () => {
  it("counts down to the next 00:00 UTC", () => {
    expect(secondsUntilUtcMidnight(new Date("2026-11-06T23:59:30Z"))).toBe(30);
    expect(secondsUntilUtcMidnight(new Date("2026-11-06T12:00:00Z"))).toBe(43_200);
  });

  it("is a full day at exactly midnight and never below one second", () => {
    expect(secondsUntilUtcMidnight(new Date("2026-11-06T00:00:00Z"))).toBe(86_400);
    expect(secondsUntilUtcMidnight(new Date("2026-11-06T23:59:59.500Z"))).toBe(1);
  });
});
