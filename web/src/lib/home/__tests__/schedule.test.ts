import { describe, expect, it } from "vite-plus/test";

import { parseTimeHHMM, SCHEDULE_PRESETS, slotStateFor, type ScheduleSlot } from "../schedule.js";

function slot(label: string, time: string): ScheduleSlot {
  return { id: label, label, time };
}

/** Local wall-clock ms at HH:MM on a fixed date — TZ-independent by construction. */
function at(hours: number, minutes: number): number {
  return new Date(2026, 0, 15, hours, minutes, 0, 0).getTime();
}

describe("parseTimeHHMM — strict 24h HH:MM", () => {
  it("accepts padded midnight-through-2359 strings", () => {
    expect(parseTimeHHMM("00:00")).toBe("00:00");
    expect(parseTimeHHMM("09:05")).toBe("09:05");
    expect(parseTimeHHMM("23:59")).toBe("23:59");
  });

  it("rejects anything else", () => {
    expect(parseTimeHHMM("24:00")).toBeUndefined();
    expect(parseTimeHHMM("7:30")).toBeUndefined();
    expect(parseTimeHHMM("12:60")).toBeUndefined();
    expect(parseTimeHHMM("12:3")).toBeUndefined();
    expect(parseTimeHHMM("")).toBeUndefined();
    expect(parseTimeHHMM("ab:cd")).toBeUndefined();
    expect(parseTimeHHMM("12:30 ")).toBeUndefined();
  });
});

describe("SCHEDULE_PRESETS", () => {
  it("carries the seven named slots with valid default times", () => {
    expect(SCHEDULE_PRESETS).toHaveLength(7);
    for (const preset of SCHEDULE_PRESETS) {
      expect(parseTimeHHMM(preset.time)).toBe(preset.time);
      expect(preset.label.length).toBeGreaterThan(0);
    }
  });
});

describe("slotStateFor — inclusive ±30-minute due window", () => {
  const NOON = slot("Dhuhr", "12:00");

  it("returns null for an empty schedule", () => {
    expect(slotStateFor([], at(12, 0))).toBeNull();
  });

  it("marks a slot due inside the window and exactly at the edges", () => {
    expect(slotStateFor([NOON], at(12, 0))?.state).toBe("due");
    expect(slotStateFor([NOON], at(11, 30))?.state).toBe("due"); // +30 edge
    expect(slotStateFor([NOON], at(12, 30))?.state).toBe("due"); // -30 edge
    expect(slotStateFor([NOON], at(11, 29))?.state).toBe("later");
    expect(slotStateFor([NOON], at(12, 31))?.state).toBe("later");
  });

  it("sees through midnight in both directions", () => {
    const late = slot("Isha", "23:50");
    const early = slot("Fajr", "00:10");
    expect(slotStateFor([late], at(0, 10))?.state).toBe("due"); // 20 past midnight
    expect(slotStateFor([early], at(23, 50))?.state).toBe("due"); // 20 before midnight
    expect(slotStateFor([late], at(0, 21))?.state).toBe("later"); // 31 past midnight
    expect(slotStateFor([early], at(0, 41))?.state).toBe("later"); // 31 after 00:10
  });

  it("points at the next slot across midnight", () => {
    const state = slotStateFor([slot("Isha", "23:50")], at(12, 0));
    if (state?.state !== "later") throw new Error("expected a later state");
    expect(state.slot.label).toBe("Isha");
    expect(state.minutesUntil).toBe(710);
  });

  it("prefers the soonest upcoming slot when none is due", () => {
    const state = slotStateFor([slot("Asr", "15:30"), slot("Isha", "19:30")], at(13, 0));
    if (state?.state !== "later") throw new Error("expected a later state");
    expect(state.slot.label).toBe("Asr");
    expect(state.minutesUntil).toBe(150);
  });

  it("prefers the chronologically earliest slot when several are due", () => {
    // 12:30 is thirty minutes past, 13:00 thirty ahead — earliest occurrence wins.
    const state = slotStateFor([slot("Later", "13:00"), slot("Earlier", "12:30")], at(13, 0));
    if (state?.state !== "due") throw new Error("expected a due state");
    expect(state.slot.label).toBe("Earlier");
  });

  it("counts a slot twenty-nine minutes past as due, thirty-one as not", () => {
    expect(slotStateFor([NOON], at(12, 29))?.state).toBe("due");
    expect(slotStateFor([NOON], at(12, 31))?.state).toBe("later");
  });
});
