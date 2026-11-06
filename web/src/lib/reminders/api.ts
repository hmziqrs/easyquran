import { authClient } from "#lib/auth/auth-client.js";

import { decodeReminder, REMINDER_KIND, reminderSetBody, type ReminderSchedule } from "./schedule";

/**
 * Reminder routes (F9). Both are signed-in only. `null` means no schedule is stored.
 * A failure is reported as `ok: false` and never throws, so the settings row can
 * keep its last known state.
 */

export type ReminderLoad =
  | { readonly ok: true; readonly schedule: ReminderSchedule | null }
  | { readonly ok: false };

export type ReminderSave =
  | { readonly ok: true; readonly schedule: ReminderSchedule }
  | { readonly ok: false; readonly status: number };

export async function loadReminder(signal?: AbortSignal): Promise<ReminderLoad> {
  try {
    const res = await authClient.unsafeRequest("/notification/v1/reminder/get", {
      method: "POST",
      body: { kind: REMINDER_KIND },
      signal,
    });
    if (!res.ok) return { ok: false };
    return { ok: true, schedule: decodeReminder(res.data) };
  } catch {
    return { ok: false };
  }
}

export async function saveReminder(schedule: ReminderSchedule): Promise<ReminderSave> {
  try {
    const res = await authClient.unsafeRequest("/notification/v1/reminder/set", {
      method: "POST",
      body: reminderSetBody(schedule),
    });
    if (!res.ok) return { ok: false, status: res.status };
    const saved = decodeReminder(res.data);
    if (saved === null) return { ok: false, status: res.status };
    return { ok: true, schedule: saved };
  } catch {
    return { ok: false, status: 0 };
  }
}
