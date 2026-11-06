/**
 * Optional offline audio. Files live in the Cache API under one name, and a
 * device-local index records each file's size so the explicit budget in
 * `policy.ts` can be enforced without reading every cached blob.
 */
import { asNumber, asObject, readJSON, tryWriteJSON } from "#lib/storage/index.js";

import { OFFLINE_AUDIO_MAX_FILE_BYTES, admitOfflineBytes } from "./policy";

export const OFFLINE_AUDIO_CACHE = "easyquran-audio-v1";
export const OFFLINE_AUDIO_INDEX_KEY = "easyquran.audio.offlineIndex.v1";

/** url -> stored bytes. */
type OfflineIndex = Map<string, number>;

function readIndex(): OfflineIndex {
  const index: OfflineIndex = new Map();
  const raw = asObject(readJSON(OFFLINE_AUDIO_INDEX_KEY));
  if (!raw) return index;
  for (const [url, bytes] of Object.entries(raw)) {
    const size = asNumber(bytes, 1, OFFLINE_AUDIO_MAX_FILE_BYTES);
    if (size !== undefined) index.set(url, size);
  }
  return index;
}

function writeIndex(index: OfflineIndex): boolean {
  return tryWriteJSON(OFFLINE_AUDIO_INDEX_KEY, Object.fromEntries(index));
}

function hasCacheStorage(): boolean {
  return "caches" in globalThis;
}

export function offlineBytesUsed(): number {
  let total = 0;
  for (const bytes of readIndex().values()) total += bytes;
  return total;
}

export type CacheResult = "ok" | "limit" | "failed";

/**
 * Downloads one audio file into the offline cache. `limit` when the file would
 * break the budget (checked against the declared length before the body is read).
 */
export async function cacheAudioFile(url: string): Promise<CacheResult> {
  if (!hasCacheStorage()) return "failed";
  const index = readIndex();
  if (index.has(url)) return "ok";
  const used = offlineBytesUsed();
  try {
    const response = await fetch(url, { mode: "cors", credentials: "omit" });
    if (!response.ok) return "failed";
    const declared = Number(response.headers.get("content-length"));
    if (declared > 0 && !admitOfflineBytes(used, declared)) {
      void response.body?.cancel().catch(() => {});
      return "limit";
    }
    const blob = await response.blob();
    if (!admitOfflineBytes(used, blob.size)) return "limit";
    const cache = await caches.open(OFFLINE_AUDIO_CACHE);
    await cache.put(url, new Response(blob));
    index.set(url, blob.size);
    return writeIndex(index) ? "ok" : "failed";
  } catch {
    return "failed";
  }
}

/** Object URL for a cached file, or null when it is not cached. Caller revokes it. */
export async function cachedAudioObjectUrl(url: string): Promise<string | null> {
  if (!hasCacheStorage()) return null;
  if (!readIndex().has(url)) return null;
  try {
    const cache = await caches.open(OFFLINE_AUDIO_CACHE);
    const hit = await cache.match(url);
    if (!hit) return null;
    return URL.createObjectURL(await hit.blob());
  } catch {
    return null;
  }
}

export async function removeCachedAudioFiles(): Promise<void> {
  try {
    if (hasCacheStorage()) await caches.delete(OFFLINE_AUDIO_CACHE);
  } catch {
    // Storage blocked: the index is cleared below regardless.
  }
  writeIndex(new Map());
}
