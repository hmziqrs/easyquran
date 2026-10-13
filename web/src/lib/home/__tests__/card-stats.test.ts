import { describe, expect, it } from "vite-plus/test";

import { cardStats, dayKeyOf } from "../card-stats.js";

/** Local wall-clock ms — TZ-independent by construction (no UTC subtraction). */
function at(year: number, month: number, day: number, hours = 12, minutes = 0): number {
  return new Date(year, month - 1, day, hours, minutes, 0, 0).getTime();
}

/** Fixed "now": local 2026-10-13 18:00. */
const NOW = at(2026, 10, 13, 18, 0);

describe("dayKeyOf — device-local calendar day", () => {
  it("formats local year/month/day zero-padded", () => {
    expect(dayKeyOf(NOW)).toBe("2026-10-13");
    expect(dayKeyOf(at(2026, 1, 5))).toBe("2026-01-05");
  });

  it("follows the local day boundary, not UTC", () => {
    // 23:30 local vs 00:30 local — same UTC instant pair would straddle in some
    // zones; both must land on their own local day key.
    expect(dayKeyOf(at(2026, 10, 13, 23, 30))).toBe("2026-10-13");
    expect(dayKeyOf(at(2026, 10, 14, 0, 30))).toBe("2026-10-14");
  });
});

describe("cardStats — empty and legacy cards", () => {
  it("returns an all-zero strip for a card without events", () => {
    const stats = cardStats({ count: 0, events: undefined }, NOW);
    expect(stats.total).toBe(0);
    expect(stats.today).toBe(0);
    expect(stats.last7).toBe(0);
    expect(stats.last30).toBe(0);
    expect(stats.currentStreak).toBe(0);
    expect(stats.bestStreak).toBe(0);
    expect(stats.lastRecitedAt).toBeNull();
    expect(stats.activity30).toHaveLength(30);
    expect(stats.activity30.every((day) => day.count === 0)).toBe(true);
  });

  it("keeps count as the total for legacy cards written before the ring", () => {
    const stats = cardStats({ count: 42, events: undefined }, NOW);
    expect(stats.total).toBe(42);
    expect(stats.today).toBe(0);
    expect(stats.lastRecitedAt).toBeNull();
  });

  it("spans the strip 30 local days ending today, oldest first", () => {
    const stats = cardStats({ count: 0, events: undefined }, NOW);
    // Sept has 30 days: 2026-09-14 + 29 steps lands on 2026-10-13.
    expect(stats.activity30[0]?.day).toBe("2026-09-14");
    expect(stats.activity30[29]?.day).toBe("2026-10-13");
  });
});

describe("cardStats — calendar-day windows", () => {
  it("counts events inside the local-day windows ending today", () => {
    const events = [
      at(2026, 10, 13, 9, 0), // today
      at(2026, 10, 12, 22, 0), // yesterday
      at(2026, 10, 6, 8, 0), // 7 days back — inside last30, outside last7
      at(2026, 9, 13, 10, 0), // 30 days back — outside last30 (window starts Sep 14)
      at(2026, 9, 12, 10, 0), // 31 days back
    ];
    const stats = cardStats({ count: events.length, events }, NOW);
    expect(stats.today).toBe(1);
    expect(stats.last7).toBe(2);
    expect(stats.last30).toBe(3);
  });

  it("buckets multiple same-day events onto one strip day", () => {
    const events = [at(2026, 10, 12, 6, 0), at(2026, 10, 12, 13, 0), at(2026, 10, 12, 21, 0)];
    const stats = cardStats({ count: 3, events }, NOW);
    expect(stats.last7).toBe(3);
    const stripDay = stats.activity30.find((day) => day.day === "2026-10-12");
    expect(stripDay?.count).toBe(3);
  });

  it("splits events 23h apart across midnight into two days", () => {
    // Calendar-day keys (never 24h gaps): 23:30 and next-day 00:30 are two days
    // — the same mechanism that keeps DST-short/long days honest.
    const events = [at(2026, 10, 12, 23, 30), at(2026, 10, 13, 0, 30)];
    const stats = cardStats({ count: 2, events }, NOW);
    expect(stats.today).toBe(1);
    expect(stats.last7).toBe(2);
  });
});

describe("cardStats — streaks", () => {
  it("counts the current streak only over consecutive days up to today", () => {
    const events = [
      at(2026, 10, 8), // run of three
      at(2026, 10, 9),
      at(2026, 10, 10),
      at(2026, 10, 13), // gap on the 11th/12th — today stands alone
    ];
    const stats = cardStats({ count: events.length, events }, NOW);
    expect(stats.currentStreak).toBe(1);
    expect(stats.bestStreak).toBe(3);
  });

  it("grants the grace day: yesterday keeps a streak alive, two days ago does not", () => {
    const alive = cardStats(
      { count: 3, events: [at(2026, 10, 10), at(2026, 10, 11), at(2026, 10, 12)] },
      NOW,
    );
    expect(alive.currentStreak).toBe(3);
    const dead = cardStats({ count: 1, events: [at(2026, 10, 11)] }, NOW);
    expect(dead.currentStreak).toBe(0);
    expect(dead.bestStreak).toBe(1);
  });

  it("walks streaks across month boundaries via day-string compare", () => {
    const events = [at(2026, 1, 30), at(2026, 1, 31), at(2026, 2, 1)];
    const stats = cardStats({ count: 3, events }, at(2026, 2, 1, 9, 0));
    expect(stats.currentStreak).toBe(3);
    expect(stats.bestStreak).toBe(3);
  });

  it("keeps the best streak when the current one reset", () => {
    const events = [
      at(2026, 9, 1),
      at(2026, 9, 2),
      at(2026, 9, 3),
      at(2026, 9, 4),
      at(2026, 9, 5),
      at(2026, 10, 12),
      at(2026, 10, 13),
    ];
    const stats = cardStats({ count: events.length, events }, NOW);
    expect(stats.bestStreak).toBe(5);
    expect(stats.currentStreak).toBe(2);
  });
});

describe("cardStats — last recited", () => {
  it("reports the newest event even from an unsorted ring", () => {
    const first = at(2026, 10, 13, 8, 0);
    const second = at(2026, 10, 13, 17, 30);
    const stats = cardStats({ count: 2, events: [second, first] }, NOW);
    expect(stats.lastRecitedAt).toBe(second);
  });
});
