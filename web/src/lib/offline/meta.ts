import { browser } from "$app/environment";
import { idbDelete, idbGet, idbPut, openIdb } from "$lib/workers/idb";

const META_DB = "easyquran-sw-meta";
const META_STORE = "meta";

export interface ActivePack {
  packId: string;
  entries: number;
  bytes: number;
  savedAt: number;
}

export async function metaGet<T>(key: string): Promise<T | undefined> {
  if (!browser) return undefined;
  try {
    return await idbGet<T>(await openIdb(META_DB, META_STORE), META_STORE, key);
  } catch (err) {
    console.warn("[offline] metaGet failed:", err);
    return undefined;
  }
}

export async function metaSet(key: string, value: unknown): Promise<void> {
  if (!browser) return;
  try {
    await idbPut(await openIdb(META_DB, META_STORE), META_STORE, value, key);
  } catch (err) {
    console.warn("[offline] metaSet failed:", err);
  }
}

export async function metaDel(key: string): Promise<void> {
  if (!browser) return;
  try {
    await idbDelete(await openIdb(META_DB, META_STORE), META_STORE, key);
  } catch (err) {
    console.warn("[offline] metaDel failed:", err);
  }
}

export async function getActivePack(): Promise<ActivePack | null> {
  const value = await metaGet<ActivePack>("activePack");
  return value && typeof value.packId === "string" ? value : null;
}

export async function setActivePack(pack: ActivePack): Promise<void> {
  await metaSet("activePack", pack);
}

export async function clearActivePack(): Promise<void> {
  await metaDel("activePack");
}

export async function recordAck(clientId: string, version: string): Promise<void> {
  const acks = (await metaGet<Record<string, string>>("acks")) ?? {};
  acks[clientId] = version;
  await metaSet("acks", acks);
}

export async function cleanAcks(live: string[]): Promise<void> {
  const acks = (await metaGet<Record<string, string>>("acks")) ?? {};
  const liveSet = new Set(live);
  const next: Record<string, string> = {};
  for (const [id, version] of Object.entries(acks)) if (liveSet.has(id)) next[id] = version;
  await metaSet("acks", next);
}

export async function getMaintenanceCursor(): Promise<string | null> {
  const meta = (await metaGet<{ cursor: string | null }>("maintenance")) ?? { cursor: null };
  return meta.cursor ?? null;
}

export async function setMaintenanceCursor(cursor: string | null): Promise<void> {
  await metaSet("maintenance", { cursor });
}

export async function getRecency(): Promise<Record<string, number>> {
  return (await metaGet<Record<string, number>>("recency")) ?? {};
}

export async function setRecency(recency: Record<string, number>): Promise<void> {
  await metaSet("recency", recency);
}
