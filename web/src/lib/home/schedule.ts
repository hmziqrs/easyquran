/**
 * Per-card recite schedule: named clock-time slots the user recites at. No
 * geolocation and no prayer-time astronomy — fixed, fully user-editable `HH:MM`
 * times only. Pure helpers shared by the cards store decoder and the card
 * form/detail surfaces; every function takes an explicit `nowMs` so callers and
 * tests stay deterministic.
 */

export interface ScheduleSlot {
  readonly id: string;
  readonly label: string;
  /** 24-hour clock time as strict `HH:MM`. */
  readonly time: string;
}

export interface SchedulePreset {
  readonly label: string;
  readonly time: string;
}

/** Named starting points for the schedule editor; times stay editable. */
export const SCHEDULE_PRESETS: readonly SchedulePreset[] = [
  { label: "Fajr", time: "05:00" },
  { label: "Dhuhr", time: "12:30" },
  { label: "Asr", time: "15:30" },
  { label: "Maghrib", time: "18:00" },
  { label: "Isha", time: "19:30" },
  { label: "Morning", time: "07:00" },
  { label: "Evening", time: "17:30" },
];

const TIME_HHMM = /^([01]\d|2[0-3]):[0-5]\d$/u;
const MINUTES_PER_DAY = 24 * 60;
/** A slot is "due" inside an inclusive ±30-minute window around its time. */
const DUE_WINDOW_MIN = 30;

/** Parses a strict `HH:MM` 24-hour clock string; undefined for anything else. */
export function parseTimeHHMM(raw: string): string | undefined {
  return TIME_HHMM.test(raw) ? raw : undefined;
}

export type SlotState =
  | { readonly state: "due"; readonly slot: ScheduleSlot }
  | { readonly state: "later"; readonly slot: ScheduleSlot; readonly minutesUntil: number };

function minutesOfDay(time: string): number {
  const hours = Number(time.slice(0, 2));
  const minutes = Number(time.slice(3, 5));
  return hours * 60 + minutes;
}

/** Forward minutes until the slot's next occurrence (0..1439), wrapping midnight. */
function minutesUntilSlot(slotMinutes: number, nowMinutes: number): number {
  const forward = slotMinutes - nowMinutes;
  return forward >= 0 ? forward : forward + MINUTES_PER_DAY;
}

/**
 * Due badge state for a card's schedule: a slot inside the inclusive ±30-minute
 * window wins (the chronologically earliest occurrence when several overlap, so
 * a slot thirty minutes past reads as due just like one thirty minutes ahead);
 * otherwise the soonest upcoming slot with its minutes-until hint, which may
 * wrap midnight. null when the schedule is empty.
 */
export function slotStateFor(schedule: readonly ScheduleSlot[], nowMs: number): SlotState | null {
  if (schedule.length === 0) return null;
  const now = new Date(nowMs);
  const nowMinutes = now.getHours() * 60 + now.getMinutes();
  let due: { readonly slot: ScheduleSlot; readonly offsetMinutes: number } | undefined;
  let next: { readonly slot: ScheduleSlot; readonly minutesUntil: number } | undefined;
  for (const slot of schedule) {
    const until = minutesUntilSlot(minutesOfDay(slot.time), nowMinutes);
    // Signed occurrence offset from now: negative for a slot already past.
    const offset = until <= DUE_WINDOW_MIN ? until : until - MINUTES_PER_DAY;
    if (offset >= -DUE_WINDOW_MIN) {
      if (due === undefined || offset < due.offsetMinutes) {
        due = { slot, offsetMinutes: offset };
      }
      continue;
    }
    if (next === undefined || until < next.minutesUntil) next = { slot, minutesUntil: until };
  }
  if (due !== undefined) return { state: "due", slot: due.slot };
  if (next !== undefined) {
    return { state: "later", slot: next.slot, minutesUntil: next.minutesUntil };
  }
  return null;
}
