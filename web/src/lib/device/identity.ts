import { readRaw, writeRaw } from "#lib/storage/index.js";
import { newEntityId } from "#lib/sync/ids.js";

/**
 * Per-account device id on this install. Synchronous because stamps are: reader
 * writes cannot wait on IndexedDB. Only sync stamps and the account device list
 * ever carry it, never analytics, rate-limit keys or logs.
 */

function keyFor(owner: string): string {
  return `easyquran.device.id.${owner}`;
}

/** Survives a blocked `localStorage` for the life of the page. */
const memory = new Map<string, string>();

export function deviceIdFor(owner: string): string {
  const key = keyFor(owner);
  const stored = readRaw("local", key);
  if (stored !== null && stored.length > 0) return stored;
  const minted = memory.get(key) ?? newEntityId();
  memory.set(key, minted);
  writeRaw("local", key, minted);
  return minted;
}
