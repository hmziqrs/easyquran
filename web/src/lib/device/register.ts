import { browser } from "$app/env";

import { authClient } from "#lib/auth/auth-client.js";
import { deviceKind } from "#lib/sessions/device.js";
import { asNumber, asObject, asString, readJSON, writeJSON } from "#lib/storage/index.js";

import { deviceIdFor } from "./identity";

/**
 * Account device registration (devices.md D2). Sign-in and push-token refresh are
 * the only triggers. At most one register per device per 24 h, one tab at a time,
 * and a failure never blocks reading or sync: callers ignore the result.
 *
 * Nothing here reaches analytics, rate-limit keys or logs. The device id is sent
 * to `/device/v1/register` and nowhere else.
 */

export const REGISTER_INTERVAL_MS = 24 * 60 * 60 * 1000;

const LOCK_NAME = "easyquran.device.register";
const KEY_PREFIX = "easyquran.device.registered.";

export type BrowserFamily = "chrome" | "firefox" | "safari" | "edge" | "other";

export interface DeviceRegistration {
  expected_user_id: number;
  device_id: string;
  platform: "web";
  client: "web";
  form_factor: "phone" | "tablet" | "desktop";
  browser: BrowserFamily;
  token?: string;
}

interface RegisteredRecord {
  at: number;
  token: string | null;
}

/** Closed set the server accepts. Anything else in the UA is "other". */
export function browserFamily(ua: string): BrowserFamily {
  if (/Edg\//u.test(ua)) return "edge";
  if (/Firefox\//u.test(ua)) return "firefox";
  if (/Chrome\/|CriOS\//u.test(ua)) return "chrome";
  if (/Safari\//u.test(ua)) return "safari";
  return "other";
}

export function registrationFor(owner: number, token: string | null = null): DeviceRegistration {
  const ua = browser && "navigator" in globalThis ? navigator.userAgent : "";
  const body: DeviceRegistration = {
    expected_user_id: owner,
    device_id: deviceIdFor(String(owner)),
    platform: "web",
    client: "web",
    form_factor: deviceKind(),
    browser: browserFamily(ua),
  };
  if (token !== null) body.token = token;
  return body;
}

function keyFor(owner: number): string {
  return `${KEY_PREFIX}${owner}`;
}

function readRecord(owner: number): RegisteredRecord | null {
  const record = asObject(readJSON(keyFor(owner)));
  if (record === undefined) return null;
  const at = asNumber(record.at, 0, Number.MAX_SAFE_INTEGER);
  if (at === undefined) return null;
  return { at, token: asString(record.token) ?? null };
}

function writeRecord(owner: number, record: RegisteredRecord): void {
  writeJSON(keyFor(owner), record);
}

/** Due when never registered, when the push token changed, or after 24 h. */
export function registrationDue(owner: number, token: string | null, now = Date.now()): boolean {
  const last = readRecord(owner);
  if (last === null) return true;
  if (token !== null && token !== last.token) return true;
  return now - last.at >= REGISTER_INTERVAL_MS;
}

async function postRegistration(body: DeviceRegistration, signal?: AbortSignal): Promise<boolean> {
  try {
    const res = await authClient.unsafeRequest("/device/v1/register", {
      method: "POST",
      body,
      signal,
    });
    return res.ok;
  } catch (err) {
    console.warn("[device] register failed:", err);
    return false;
  }
}

/**
 * Register this device if due. Returns false only on a real failure. A tab that
 * finds another tab holding the lock returns true: that tab is registering now.
 */
export async function ensureDeviceRegistered(
  owner: number,
  token: string | null = null,
  signal?: AbortSignal,
): Promise<boolean> {
  if (!browser) return false;
  if (!registrationDue(owner, token)) return true;

  const run = async (): Promise<boolean> => {
    if (!registrationDue(owner, token)) return true;
    const ok = await postRegistration(registrationFor(owner, token), signal);
    if (ok) writeRecord(owner, { at: Date.now(), token });
    return ok;
  };

  const locks: LockManager | null = "locks" in navigator ? navigator.locks : null;
  if (locks === null) return run();
  return locks.request(LOCK_NAME, { ifAvailable: true }, (lock) => (lock === null ? true : run()));
}

/** A removed push token must not look registered: the next call posts again. */
export function forgetRegisteredToken(owner: number): void {
  writeRecord(owner, { at: 0, token: null });
}

/** Post-sign-in trigger. Deferred so the auth state holds the new user first. */
export function scheduleDeviceRegistration(readOwner: () => number | null): void {
  setTimeout(() => {
    const owner = readOwner();
    if (owner !== null) void ensureDeviceRegistered(owner).catch(() => false);
  }, 0);
}
