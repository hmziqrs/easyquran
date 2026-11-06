import { describe, expect, it } from "vite-plus/test";

import {
  clockFromMinutes,
  decodeReminder,
  inQuietHours,
  minutesFromClock,
  reminderSetBody,
  scheduleProblem,
  type ReminderSchedule,
} from "../schedule";

const base: ReminderSchedule = {
  enabled: true,
  timeZone: "Asia/Karachi",
  reminderMinute: 20 * 60,
  quietStartMinute: null,
  quietEndMinute: null,
};

describe("clock maths", () => {
  it("reads HH:MM and rejects anything else", () => {
    expect(minutesFromClock("20:30")).toBe(20 * 60 + 30);
    expect(minutesFromClock("00:00")).toBe(0);
    expect(minutesFromClock("24:00")).toBeNull();
    expect(minutesFromClock("12:60")).toBeNull();
    expect(minutesFromClock("7:05")).toBeNull();
    expect(minutesFromClock("")).toBeNull();
  });

  it("formats minutes back to HH:MM", () => {
    expect(clockFromMinutes(7 * 60 + 5)).toBe("07:05");
    expect(clockFromMinutes(0)).toBe("00:00");
    expect(clockFromMinutes(1440)).toBe("00:00");
  });
});

describe("quiet hours", () => {
  it("wraps midnight when the start is after the end", () => {
    expect(inQuietHours(23 * 60, 22 * 60, 7 * 60)).toBe(true);
    expect(inQuietHours(6 * 60, 22 * 60, 7 * 60)).toBe(true);
    expect(inQuietHours(8 * 60, 22 * 60, 7 * 60)).toBe(false);
  });

  it("treats equal or missing bounds as no quiet hours", () => {
    expect(inQuietHours(60, 60, 60)).toBe(false);
    expect(inQuietHours(60, null, null)).toBe(false);
  });
});

describe("scheduleProblem (mirrors the server check)", () => {
  it("accepts a schedule with no quiet hours", () => {
    expect(scheduleProblem(base)).toBeNull();
  });

  it("refuses a reminder time outside one day", () => {
    expect(scheduleProblem({ ...base, reminderMinute: 1440 })).toBe("bad_time");
    expect(scheduleProblem({ ...base, reminderMinute: -1 })).toBe("bad_time");
  });

  it("needs both quiet bounds or neither", () => {
    expect(scheduleProblem({ ...base, quietStartMinute: 60 })).toBe("quiet_incomplete");
    expect(scheduleProblem({ ...base, quietEndMinute: 60 })).toBe("quiet_incomplete");
  });

  it("refuses equal quiet bounds and out-of-range quiet bounds", () => {
    expect(scheduleProblem({ ...base, quietStartMinute: 60, quietEndMinute: 60 })).toBe(
      "quiet_same",
    );
    expect(scheduleProblem({ ...base, quietStartMinute: 1440, quietEndMinute: 60 })).toBe(
      "quiet_range",
    );
  });

  it("refuses a reminder time inside quiet hours", () => {
    expect(
      scheduleProblem({
        ...base,
        reminderMinute: 8 * 60,
        quietStartMinute: 7 * 60,
        quietEndMinute: 9 * 60,
      }),
    ).toBe("inside_quiet");
  });

  it("accepts a wrapped quiet window that does not hold the reminder", () => {
    expect(
      scheduleProblem({
        ...base,
        reminderMinute: 20 * 60,
        quietStartMinute: 22 * 60,
        quietEndMinute: 7 * 60,
      }),
    ).toBeNull();
  });
});

describe("decodeReminder", () => {
  it("decodes a stored schedule", () => {
    expect(
      decodeReminder({
        reminder: {
          kind: "goal",
          enabled: true,
          time_zone: "Asia/Karachi",
          reminder_minute: 1200,
          quiet_start_minute: 1320,
          quiet_end_minute: 420,
          updated_at: "2026-11-06T10:00:00+00:00",
        },
      }),
    ).toEqual({
      enabled: true,
      timeZone: "Asia/Karachi",
      reminderMinute: 1200,
      quietStartMinute: 1320,
      quietEndMinute: 420,
    });
  });

  it("returns null for no schedule and for malformed rows", () => {
    expect(decodeReminder({ reminder: null })).toBeNull();
    expect(decodeReminder({})).toBeNull();
    expect(decodeReminder({ reminder: { time_zone: "UTC", reminder_minute: 9999 } })).toBeNull();
    expect(decodeReminder({ reminder: { reminder_minute: 60 } })).toBeNull();
  });

  it("reads a missing or non-true enabled flag as off", () => {
    const row = decodeReminder({ reminder: { time_zone: "UTC", reminder_minute: 60 } });
    expect(row?.enabled).toBe(false);
  });
});

describe("reminderSetBody", () => {
  it("sends the goal kind and snake_case fields", () => {
    expect(reminderSetBody(base)).toEqual({
      kind: "goal",
      enabled: true,
      time_zone: "Asia/Karachi",
      reminder_minute: 1200,
      quiet_start_minute: null,
      quiet_end_minute: null,
    });
  });
});
