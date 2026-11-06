import { browser } from "$app/env";

import { authClient } from "#lib/auth/auth-client.js";
import { deviceKind } from "#lib/sessions/device.js";
import { asNumber, asObject, asString, readJSON, writeJSON } from "#lib/storage/index.js";
import { newEntityId } from "#lib/sync/ids.js";

import { deviceIdFor, isMinorOwner, setDeviceIdFor } from "./identity";
import {
  type DeviceKeyStore,
  ensureDeviceKey,
  generateDeviceKey,
  idbKeyStore,
  repairDeviceKey,
  type StoredDeviceKey,
} from "./key-store";
import { base64UrlEncode, type PublicJwk, proofMessage } from "./keys";

/**
 * Account device registration (devices.md D2, D3). Sign-in and push-token refresh are
 * the only triggers. At most one register per device per 24 h, one tab at a time,
 * and a failure never blocks reading or sync: callers ignore the result.
 *
 * A register carries the browser key's public half and a signature over a session
 * nonce (D3). With no key available the register goes out with `key_store: none`.
 * On a 409 the client mints a new id and key and registers once more.
 *
 * Nothing here reaches analytics, rate-limit keys or logs. The device id, key and
 * signature go to `/device/v1/*` and nowhere else.
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
  key_store: "browser" | "none";
  public_key?: PublicJwk;
  nonce?: string;
  signature?: string;
  token?: string;
}

interface RegisteredRecord {
  at: number;
  token: string | null;
}

interface Identity {
  deviceId: string;
  key: StoredDeviceKey | null;
}

interface Attempt {
  ok: boolean;
  status: number;
  deviceId: string | null;
}

/** Closed set the server accepts. Anything else in the UA is "other". */
export function browserFamily(ua: string): BrowserFamily {
  if (/Edg\//u.test(ua)) return "edge";
  if (/Firefox\//u.test(ua)) return "firefox";
  if (/Chrome\/|CriOS\//u.test(ua)) return "chrome";
  if (/Safari\//u.test(ua)) return "safari";
  return "other";
}

/** The keyless body. The keyed body adds the public key, the nonce and the signature. */
export function registrationFor(
  owner: number,
  deviceId: string = deviceIdFor(String(owner)),
  token: string | null = null,
): DeviceRegistration {
  const ua = browser && "navigator" in globalThis ? navigator.userAgent : "";
  const body: DeviceRegistration = {
    expected_user_id: owner,
    device_id: deviceId,
    platform: "web",
    client: "web",
    form_factor: deviceKind(),
    browser: browserFamily(ua),
    key_store: "none",
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

let storeOverride: DeviceKeyStore | null = null;

/** Test seam: swap the key store. Production always uses IndexedDB. */
export function setDeviceKeyStore(store: DeviceKeyStore | null): void {
  storeOverride = store;
}

function keyStore(): DeviceKeyStore {
  return storeOverride ?? idbKeyStore();
}

/** Best effort. A browser that refuses persistence still gets a key for this session. */
async function requestPersistence(): Promise<void> {
  try {
    if ("storage" in navigator) await navigator.storage.persist();
  } catch {
    // Persistence is a hint to the browser, never a requirement.
  }
}

/** The stored key for this id, or a fresh one. A failure means no key, not a blocked register. */
async function resolveIdentity(owner: number): Promise<Identity> {
  const id = deviceIdFor(String(owner));
  void requestPersistence();
  try {
    const key = await ensureDeviceKey(keyStore(), String(owner), id);
    if (key.deviceId !== id) setDeviceIdFor(String(owner), key.deviceId);
    return { deviceId: key.deviceId, key };
  } catch (err) {
    console.warn("[device] no device key, registering without one:", err);
    return { deviceId: id, key: null };
  }
}

/** New id and new key, after the server refused the old id with a 409. */
async function mintIdentity(owner: number): Promise<Identity> {
  const id = newEntityId();
  setDeviceIdFor(String(owner), id);
  try {
    const key = await generateDeviceKey(id);
    await keyStore().put(String(owner), key);
    return { deviceId: id, key };
  } catch (err) {
    console.warn("[device] no device key, registering without one:", err);
    return { deviceId: id, key: null };
  }
}

interface Challenge {
  nonce: string;
  salt: string;
}

/** `POST /device/v1/challenge`. Null on any failure, and the register then waits. */
async function fetchChallenge(signal?: AbortSignal): Promise<Challenge | null> {
  const res = await authClient.unsafeRequest<unknown>("/device/v1/challenge", {
    method: "POST",
    body: {},
    signal,
  });
  const data = asObject(res.data);
  const nonce = asString(data?.nonce);
  const salt = asString(data?.salt);
  if (!res.ok || nonce === undefined || salt === undefined) return null;
  return { nonce, salt };
}

/** The body for this identity. Null when the key cannot sign a fresh challenge. */
async function bodyFor(
  owner: number,
  token: string | null,
  identity: Identity,
  signal?: AbortSignal,
): Promise<DeviceRegistration | null> {
  const base = registrationFor(owner, identity.deviceId, token);
  if (identity.key === null) return base;
  const challenge = await fetchChallenge(signal);
  if (challenge === null) return null;
  const signature = await crypto.subtle.sign(
    { name: "ECDSA", hash: "SHA-256" },
    identity.key.privateKey,
    proofMessage(challenge.nonce, identity.deviceId, owner),
  );
  return {
    ...base,
    key_store: "browser",
    public_key: identity.key.publicJwk,
    nonce: challenge.nonce,
    signature: base64UrlEncode(new Uint8Array(signature)),
  };
}

async function attempt(
  owner: number,
  token: string | null,
  identity: Identity,
  signal?: AbortSignal,
): Promise<Attempt> {
  const failed: Attempt = { ok: false, status: 0, deviceId: null };
  const body = await bodyFor(owner, token, identity, signal);
  if (body === null) return failed;
  try {
    const res = await authClient.unsafeRequest<unknown>("/device/v1/register", {
      method: "POST",
      body,
      signal,
    });
    const data = asObject(res.data);
    return { ok: res.ok, status: res.status, deviceId: asString(data?.device_id) ?? null };
  } catch (err) {
    console.warn("[device] register failed:", err);
    return failed;
  }
}

/**
 * One register, including the 409 retry. The server's id wins: a different id in the
 * reply is adopted locally, and the key is re-paired with it.
 */
async function registerOnce(
  owner: number,
  token: string | null,
  signal?: AbortSignal,
): Promise<boolean> {
  let identity = await resolveIdentity(owner);
  let result = await attempt(owner, token, identity, signal);
  if (result.status === 409) {
    identity = await mintIdentity(owner);
    result = await attempt(owner, token, identity, signal);
  }
  if (!result.ok) return false;
  // The server always echoes the id it kept. A reply without one means the id sent.
  const kept = result.deviceId ?? identity.deviceId;
  if (kept !== identity.deviceId) {
    setDeviceIdFor(String(owner), kept);
    if (identity.key !== null) {
      await repairDeviceKey(keyStore(), String(owner), identity.key, kept);
    }
  }
  writeRecord(owner, { at: Date.now(), token });
  return true;
}

/**
 * Register this device if due. Returns false only on a real failure. A tab that
 * finds another tab holding the lock returns true: that tab is registering now.
 * A minor account (A34) never registers: there is no device row and no push token,
 * so this is a no-op, not a failure.
 */
export async function ensureDeviceRegistered(
  owner: number,
  token: string | null = null,
  signal?: AbortSignal,
): Promise<boolean> {
  if (!browser) return false;
  if (isMinorOwner(String(owner))) return true;
  if (!registrationDue(owner, token)) return true;

  const run = async (): Promise<boolean> => {
    if (!registrationDue(owner, token)) return true;
    return registerOnce(owner, token, signal);
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
