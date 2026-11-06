import { idbGet, idbPut, openIdb } from "#lib/workers/idb.js";

import type { PublicJwk } from "./keys";

/**
 * The browser's device key (devices.md D3, row "Web browser tab"): one non-extractable
 * P-256 private key per account per browser profile, in IndexedDB. The id lives in
 * `localStorage` (see `identity.ts`). A record pairs the two, so a mismatch is a lost
 * key or a lost id, never a silent pairing.
 */

const DB_NAME = "easyquran-device";
const STORE = "keys";

export interface StoredDeviceKey {
  deviceId: string;
  privateKey: CryptoKey;
  publicJwk: PublicJwk;
}

export interface DeviceKeyStore {
  get(owner: string): Promise<StoredDeviceKey | undefined>;
  /** Write only if the owner has no record. Returns whatever record ends up stored. */
  putIfAbsent(owner: string, record: StoredDeviceKey): Promise<StoredDeviceKey>;
  put(owner: string, record: StoredDeviceKey): Promise<void>;
}

/** IndexedDB transactions serialise across tabs, so the read and the write sit in one. */
export function idbKeyStore(): DeviceKeyStore {
  return {
    async get(owner) {
      return idbGet<StoredDeviceKey>(await openIdb(DB_NAME, STORE), STORE, owner);
    },
    async putIfAbsent(owner, record) {
      const db = await openIdb(DB_NAME, STORE);
      return new Promise<StoredDeviceKey>((resolve, reject) => {
        const tx = db.transaction(STORE, "readwrite");
        const store = tx.objectStore(STORE);
        let winner: StoredDeviceKey = record;
        const probe = store.get(owner);
        probe.onsuccess = () => {
          // IDB hands back `any`; the store only ever holds records written by this module.
          const existing: StoredDeviceKey | undefined = probe.result;
          if (existing === undefined) store.put(record, owner);
          else winner = existing;
        };
        tx.oncomplete = () => resolve(winner);
        tx.onerror = () => reject(tx.error);
        tx.onabort = () => reject(tx.error);
      });
    },
    async put(owner, record) {
      await idbPut(await openIdb(DB_NAME, STORE), STORE, record, owner);
    },
  };
}

/** Same contract as the IndexedDB store, for tests and for environments without it. */
export function memoryKeyStore(): DeviceKeyStore {
  const records = new Map<string, StoredDeviceKey>();
  return {
    get: async (owner) => records.get(owner),
    putIfAbsent: async (owner, record) => {
      const existing = records.get(owner);
      if (existing !== undefined) return existing;
      records.set(owner, record);
      return record;
    },
    put: async (owner, record) => {
      records.set(owner, record);
    },
  };
}

/** A new P-256 key. The private half never leaves the browser; the public half is exportable. */
export async function generateDeviceKey(deviceId: string): Promise<StoredDeviceKey> {
  const pair = await crypto.subtle.generateKey({ name: "ECDSA", namedCurve: "P-256" }, false, [
    "sign",
    "verify",
  ]);
  const jwk = await crypto.subtle.exportKey("jwk", pair.publicKey);
  if (jwk.x === undefined || jwk.y === undefined) throw new Error("device key export has no point");
  return {
    deviceId,
    privateKey: pair.privateKey,
    publicJwk: { kty: "EC", crv: "P-256", x: jwk.x, y: jwk.y },
  };
}

/**
 * The stored key for this owner and id, or a fresh one. A record for another id
 * counts as lost and is replaced. When two tabs race a first sign-in, both come back
 * with the record that won the add-if-absent.
 */
export async function ensureDeviceKey(
  store: DeviceKeyStore,
  owner: string,
  deviceId: string,
): Promise<StoredDeviceKey> {
  const existing = await store.get(owner);
  if (existing !== undefined && existing.deviceId === deviceId) return existing;
  const fresh = await generateDeviceKey(deviceId);
  if (existing === undefined) return store.putIfAbsent(owner, fresh);
  await store.put(owner, fresh);
  return fresh;
}

/** Re-pair a stored key with the id the server gave back (rung 2 or 3). */
export async function repairDeviceKey(
  store: DeviceKeyStore,
  owner: string,
  key: StoredDeviceKey,
  deviceId: string,
): Promise<StoredDeviceKey> {
  const next: StoredDeviceKey = { ...key, deviceId };
  await store.put(owner, next);
  return next;
}
