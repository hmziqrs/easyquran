import { deleteDeviceKey } from "#lib/device/key-store.js";
import { removeJSON } from "#lib/storage/safe-storage.js";
import { destroyAllSyncRecords } from "#lib/sync/outbox.js";

/**
 * localStorage keys scoped to one account. Each is `<prefix><user id>`, written by the module
 * named in its comment. Device-only keys (reader preferences, home layout, notes kept while
 * signed out, bookmarks.local, reading.bookmark, Quran caches) are not listed and stay.
 */
export const ACCOUNT_KEY_PREFIXES: readonly string[] = [
  "easyquran.preferences.", // preferences/store.svelte.ts
  "easyquran.goals.", // goals/store.svelte.ts
  "easyquran.notes.", // stores/reader-persistence.svelte.ts
  "easyquran.sessions.", // sessions/store.svelte.ts
  "easyquran.bookmarks.user.", // bookmarks/store.svelte.ts
  "easyquran.bookmarks.last-folder.", // bookmarks/store.svelte.ts
  "easyquran.sync.clock.", // sync/clock.ts
  "easyquran.device.registered.", // device/register.ts
  "easyquran.device.id.", // device/identity.ts
  "easyquran.analytics.queue.", // analytics/reading-analytics.ts
];

/**
 * Removes every browser-side record of one account after self-service deletion (F7): the sync
 * outbox and dead letters, the account's device key, and its account-keyed localStorage. Each
 * step runs on its own, so one failing store does not keep the others. Logout's purge hook
 * quarantines the outbox instead; this path is destructive on purpose.
 */
export async function purgeAccountLocalData(userId: number): Promise<void> {
  const owner = String(userId);
  try {
    await destroyAllSyncRecords();
  } catch {
    // Storage can be blocked or full. The server already erased the account, so the failure
    // is logged by nothing here; leftovers stay until the browser clears site data.
  }
  try {
    await deleteDeviceKey(owner);
  } catch {
    // Same as above: best effort.
  }
  for (const prefix of ACCOUNT_KEY_PREFIXES) {
    removeJSON(`${prefix}${owner}`);
  }
}
