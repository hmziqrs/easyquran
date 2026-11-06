import { authState } from "#lib/auth/auth-state.svelte.js";
import { readRaw, removeRaw, writeRaw } from "#lib/storage/index.js";
import { newEntityId } from "#lib/sync/ids.js";

/**
 * Per-account device id on this install. Synchronous because stamps are: reader
 * writes cannot wait on IndexedDB. Only sync stamps and the account device list
 * ever carry it, never analytics, rate-limit keys or logs.
 *
 * A minor account (A34) has no persisted device identity. Its id is a random value
 * held in memory for this page load only: never written to storage, never registered.
 * Stamps still carry a device tail, so the stamp format is unchanged.
 */

function keyFor(owner: string): string {
  return `easyquran.device.id.${owner}`;
}

/** Survives a blocked `localStorage` for the life of the page. */
const memory = new Map<string, string>();

/** Memory-only ids for minor accounts, one per owner for this page load. */
const ephemeral = new Map<string, string>();

/** True when the signed-in account is a declared minor and `owner` is that account. */
export function isMinorOwner(owner: string): boolean {
  const user = authState.user;
  return user !== null && user.is_minor === true && String(user.id) === owner;
}

export function deviceIdFor(owner: string): string {
  if (isMinorOwner(owner)) {
    const existing = ephemeral.get(owner) ?? crypto.randomUUID();
    ephemeral.set(owner, existing);
    return existing;
  }
  const key = keyFor(owner);
  const stored = readRaw("local", key);
  if (stored !== null && stored.length > 0) return stored;
  const minted = memory.get(key) ?? newEntityId();
  memory.set(key, minted);
  writeRaw("local", key, minted);
  return minted;
}

/** Adopt the id the server kept: rungs 2 and 3 hand back the row's id, not the one sent. */
export function setDeviceIdFor(owner: string, id: string): void {
  const key = keyFor(owner);
  memory.set(key, id);
  writeRaw("local", key, id);
}

/** Drop any id persisted before the account became a minor (A34). */
export function forgetDeviceIdFor(owner: string): void {
  const key = keyFor(owner);
  memory.delete(key);
  ephemeral.delete(owner);
  removeRaw("local", key);
}
