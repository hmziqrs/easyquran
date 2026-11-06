import { asNumber, asObject, asString } from "#lib/storage/decoders.js";

/**
 * Goal reminder schedule (F9). The account stores one row per kind on the server
 * (`/notification/v1/reminder/*`); this module holds the pure parts: clock maths,
 * the same checks the server runs, and the wire decoder. Times are minutes after
 * local midnight in `timeZone`.
 */

export const REMINDER_KIND = "goal" as const;
export const MINUTES_PER_DAY = 24 * 60;
export const DEFAULT_REMINDER_MINUTE = 20 * 60;

export interface ReminderSchedule {
  readonly enabled: boolean;
  readonly timeZone: string;
  readonly reminderMinute: number;
  readonly quietStartMinute: number | null;
  readonly quietEndMinute: number | null;
}

export type ReminderProblem =
  | "bad_time"
  | "quiet_incomplete"
  | "quiet_same"
  | "quiet_range"
  | "inside_quiet";

/** `HH:MM` from an `<input type="time">` to minutes after midnight, or null. */
export function minutesFromClock(value: string): number | null {
  const match = /^(\d{2}):(\d{2})$/u.exec(value);
  if (match === null) return null;
  const hours = Number(match[1]);
  const minutes = Number(match[2]);
  if (hours > 23 || minutes > 59) return null;
  return hours * 60 + minutes;
}

/** Minutes after midnight to `HH:MM`, for an `<input type="time">`. */
export function clockFromMinutes(minute: number): string {
  const safe = ((Math.trunc(minute) % MINUTES_PER_DAY) + MINUTES_PER_DAY) % MINUTES_PER_DAY;
  const hh = String(Math.floor(safe / 60)).padStart(2, "0");
  const mm = String(safe % 60).padStart(2, "0");
  return `${hh}:${mm}`;
}

/** Quiet hours cover `[start, end)`. A start after the end wraps midnight. */
export function inQuietHours(minute: number, start: number | null, end: number | null): boolean {
  if (start === null || end === null || start === end) return false;
  if (start < end) return minute >= start && minute < end;
  return minute >= start || minute < end;
}

/** The first problem with a schedule, or null. Mirrors `reminders::schedule_problem`. */
export function scheduleProblem(schedule: ReminderSchedule): ReminderProblem | null {
  const { reminderMinute, quietStartMinute, quietEndMinute } = schedule;
  if (
    !Number.isInteger(reminderMinute) ||
    reminderMinute < 0 ||
    reminderMinute >= MINUTES_PER_DAY
  ) {
    return "bad_time";
  }
  if (quietStartMinute === null && quietEndMinute === null) return null;
  if (quietStartMinute === null || quietEndMinute === null) return "quiet_incomplete";
  const outOfRange = [quietStartMinute, quietEndMinute].some(
    (m) => !Number.isInteger(m) || m < 0 || m >= MINUTES_PER_DAY,
  );
  if (outOfRange) return "quiet_range";
  if (quietStartMinute === quietEndMinute) return "quiet_same";
  if (inQuietHours(reminderMinute, quietStartMinute, quietEndMinute)) return "inside_quiet";
  return null;
}

/** The device's IANA zone, or UTC when the platform does not report one. */
export function deviceTimeZone(): string {
  try {
    const zone = Intl.DateTimeFormat().resolvedOptions().timeZone;
    return zone.length > 0 ? zone : "UTC";
  } catch {
    return "UTC";
  }
}

/** Decodes the `reminder` field of a reminder response. Null when there is no schedule or it is malformed. */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed reminder response body; this decoder is its boundary parser.
export function decodeReminder(raw: unknown): ReminderSchedule | null {
  const row = asObject(asObject(raw)?.reminder);
  if (row === undefined) return null;
  const timeZone = asString(row.time_zone);
  const reminderMinute = asNumber(row.reminder_minute, 0, MINUTES_PER_DAY - 1);
  if (timeZone === undefined || reminderMinute === undefined) return null;
  return {
    enabled: row.enabled === true,
    timeZone,
    reminderMinute,
    quietStartMinute: asNumber(row.quiet_start_minute, 0, MINUTES_PER_DAY - 1) ?? null,
    quietEndMinute: asNumber(row.quiet_end_minute, 0, MINUTES_PER_DAY - 1) ?? null,
  };
}

/** The body the set route takes. */
export interface ReminderSetBody {
  readonly kind: typeof REMINDER_KIND;
  readonly enabled: boolean;
  readonly time_zone: string;
  readonly reminder_minute: number;
  readonly quiet_start_minute: number | null;
  readonly quiet_end_minute: number | null;
}

export function reminderSetBody(schedule: ReminderSchedule): ReminderSetBody {
  return {
    kind: REMINDER_KIND,
    enabled: schedule.enabled,
    time_zone: schedule.timeZone,
    reminder_minute: schedule.reminderMinute,
    quiet_start_minute: schedule.quietStartMinute,
    quiet_end_minute: schedule.quietEndMinute,
  };
}
