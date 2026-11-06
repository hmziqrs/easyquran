import { describe, expect, it } from "vite-plus/test";

import {
  GOAL_LIMIT,
  QURAN_AYAH_TOTAL,
  addDays,
  calendarDays,
  creditedMinutes,
  daysBetween,
  decodeGoals,
  draftError,
  goalFromDraft,
  goalProgress,
  goalTarget,
  presetDraft,
  streakOf,
  windowEnd,
  type ReadingDay,
  type ReadingGoal,
} from "../goal-progress";

const ID = "11111111-1111-4111-8111-111111111111";
const TODAY = "2026-11-06";

function minutesGoal(over: Partial<ReadingGoal> = {}): ReadingGoal {
  return {
    id: ID,
    kind: "daily_minutes",
    dailyMinutes: 15,
    startDay: "2026-11-01",
    endDay: "2026-11-10",
    pausedFrom: null,
    pausedDays: 0,
    ...over,
  };
}

function finishGoal(over: Partial<ReadingGoal> = {}): ReadingGoal {
  return {
    id: ID,
    kind: "finish_quran",
    dailyMinutes: null,
    startDay: "2026-11-01",
    endDay: "2026-11-30",
    pausedFrom: null,
    pausedDays: 0,
    ...over,
  };
}

function day(localDay: string, ayahsRead: number, ms: number): ReadingDay {
  return { localDay, ayahsRead, ms };
}

describe("day arithmetic", () => {
  it("adds and counts whole days across month, year and DST-sensitive dates", () => {
    expect(addDays("2026-10-31", 1)).toBe("2026-11-01");
    expect(addDays("2026-12-31", 1)).toBe("2027-01-01");
    expect(addDays("2026-03-28", 1)).toBe("2026-03-29");
    expect(daysBetween("2026-03-28", "2026-03-30")).toBe(2);
    expect(daysBetween(TODAY, "2026-11-01")).toBe(-5);
  });

  it("builds the calendar oldest first, ending today", () => {
    const cells = calendarDays(TODAY, 1);
    expect(cells).toHaveLength(7);
    expect(cells[0]).toBe("2026-10-31");
    expect(cells.at(-1)).toBe(TODAY);
  });
});

describe("credit and targets", () => {
  it("counts a verse read as at least one minute, so fast readers are not penalised", () => {
    // 30 verses in 2 minutes: time alone says 2, the verse floor says 30.
    expect(creditedMinutes(day(TODAY, 30, 120_000))).toBe(30);
    // A long slow session counts its real minutes.
    expect(creditedMinutes(day(TODAY, 2, 25 * 60_000))).toBe(25);
  });

  it("targets: minutes over the base span, verses for a finish goal", () => {
    expect(goalTarget(minutesGoal())).toBe(15 * 10);
    expect(goalTarget(finishGoal())).toBe(QURAN_AYAH_TOTAL);
  });
});

describe("goalProgress", () => {
  it("a fast reader meets a minutes goal on verse reads alone", () => {
    const goal = minutesGoal({ startDay: TODAY, endDay: addDays(TODAY, 2) });
    // Target 45 minutes over three days. 45 verses in fast sessions = 45 credited minutes.
    const result = goalProgress(goal, [day(TODAY, 45, 4 * 60_000)], TODAY);
    expect(result).toEqual({ done: 45, target: 45, state: "completed" });
  });

  it("sums only days inside the window, read on any device", () => {
    const goal = minutesGoal();
    const reads = [
      day("2026-10-31", 9, 60 * 60_000), // before the window: ignored
      day("2026-11-02", 5, 10 * 60_000),
      day("2026-11-05", 4, 5 * 60_000),
    ];
    expect(goalProgress(goal, reads, TODAY)).toEqual({ done: 15, target: 150, state: "active" });
  });

  it("a finish goal counts verse reads against 6,236", () => {
    const goal = finishGoal();
    const reads = [day("2026-11-02", 300, 0), day("2026-11-03", 200, 0)];
    expect(goalProgress(goal, reads, TODAY)).toEqual({
      done: 500,
      target: QURAN_AYAH_TOTAL,
      state: "active",
    });
  });

  it("reports ended when the window has passed without the target", () => {
    const goal = minutesGoal();
    expect(goalProgress(goal, [], "2026-11-11").state).toBe("ended");
  });

  it("paused days never use up the window; resume keeps the same end", () => {
    const running = minutesGoal({ startDay: "2026-11-01", endDay: "2026-11-05" });
    // Paused on Nov 3, still paused on Nov 6: the window now ends Nov 8, not Nov 5.
    const paused = minutesGoal({
      startDay: "2026-11-01",
      endDay: "2026-11-05",
      pausedFrom: "2026-11-03",
    });
    expect(windowEnd(running, TODAY)).toBe("2026-11-05");
    expect(windowEnd(paused, TODAY)).toBe("2026-11-08");
    expect(goalProgress(paused, [], TODAY).state).toBe("paused");
    // Resumed on Nov 6 after three paused days: the same end, stored instead of computed.
    const resumed = minutesGoal({
      startDay: "2026-11-01",
      endDay: "2026-11-05",
      pausedFrom: null,
      pausedDays: 3,
    });
    expect(windowEnd(resumed, TODAY)).toBe("2026-11-08");
    expect(windowEnd(paused, TODAY)).toBe(windowEnd(resumed, TODAY));
  });
});

describe("streakOf", () => {
  it("counts consecutive read days back from today", () => {
    const reads = [day(TODAY, 1, 0), day("2026-11-05", 2, 0), day("2026-11-04", 1, 0)];
    expect(streakOf(reads, TODAY)).toBe(3);
  });

  it("does not break a streak before today's reading starts", () => {
    const reads = [day("2026-11-05", 2, 0), day("2026-11-04", 1, 0)];
    expect(streakOf(reads, TODAY)).toBe(2);
  });

  it("stops at the first gap, and ignores zero-read rows", () => {
    const reads = [day(TODAY, 1, 0), day("2026-11-04", 1, 0), day("2026-11-03", 0, 0)];
    expect(streakOf(reads, TODAY)).toBe(1);
  });

  it("is zero with no reads", () => {
    expect(streakOf([], TODAY)).toBe(0);
  });

  it("counts a local day as the reader's day, whatever the zone, with no DST shift", () => {
    // Reads logged on local days that straddle a spring-forward night still form one streak.
    const reads = [day("2026-03-29", 1, 0), day("2026-03-28", 1, 0), day("2026-03-27", 1, 0)];
    expect(streakOf(reads, "2026-03-29")).toBe(3);
  });
});

describe("drafts and presets", () => {
  it("the presets are 15 minutes for 30 days, and finish in 30 days or a year", () => {
    expect(presetDraft("minutes_30", TODAY)).toEqual({
      kind: "daily_minutes",
      dailyMinutes: 15,
      startDay: TODAY,
      endDay: "2026-12-05",
    });
    expect(presetDraft("finish_30", TODAY).endDay).toBe("2026-12-05");
    expect(presetDraft("finish_365", TODAY).endDay).toBe("2027-11-05");
  });

  it("rejects bad dates, a window over 400 days, minutes outside 1 to 600, and a full list", () => {
    const base = {
      kind: "daily_minutes" as const,
      dailyMinutes: 15,
      startDay: TODAY,
      endDay: TODAY,
    };
    expect(draftError({ ...base, endDay: "2026-11-05" }, 0)).toBe("dates");
    expect(draftError({ ...base, startDay: "2026-02-30" }, 0)).toBe("dates");
    expect(draftError({ ...base, endDay: addDays(TODAY, 400) }, 0)).toBe("span");
    expect(draftError({ ...base, endDay: addDays(TODAY, 399) }, 0)).toBeNull();
    expect(draftError({ ...base, dailyMinutes: 0 }, 0)).toBe("minutes");
    expect(draftError({ ...base, dailyMinutes: 601 }, 0)).toBe("minutes");
    expect(draftError({ ...base, dailyMinutes: 2.5 }, 0)).toBe("minutes");
    expect(draftError({ ...base, dailyMinutes: Number.NaN }, 0)).toBe("minutes");
    expect(draftError(base, GOAL_LIMIT)).toBe("limit");
  });

  it("a finish goal needs no minutes", () => {
    const draft = {
      kind: "finish_quran" as const,
      dailyMinutes: null,
      startDay: TODAY,
      endDay: TODAY,
    };
    expect(draftError(draft, 0)).toBeNull();
    expect(goalFromDraft(draft, ID).dailyMinutes).toBeNull();
  });
});

describe("decodeGoals", () => {
  it("keeps valid rows, drops bad ones, de-duplicates and caps the list", () => {
    const good = { ...minutesGoal() };
    const rows = [
      good,
      { ...good, id: "short" },
      { ...good, kind: "mystery" },
      { ...good, dailyMinutes: 9999 },
      { ...good, endDay: "2026-10-01" },
      { ...good, pausedFrom: "not-a-day" },
      { ...good, pausedDays: -1 },
      "nope",
    ];
    expect(decodeGoals({ goals: rows })).toEqual([good]);
    const many = Array.from({ length: 12 }, (_, i) => ({
      ...good,
      id: `${String(i).padStart(8, "0")}-1111-4111-8111-111111111111`,
    }));
    expect(decodeGoals({ goals: many })).toHaveLength(GOAL_LIMIT);
  });

  it("returns nothing for anything that is not a goal document", () => {
    expect(decodeGoals(null)).toEqual([]);
    expect(decodeGoals([])).toEqual([]);
    expect(decodeGoals({ goals: "x" })).toEqual([]);
  });
});
