import { browser } from "$app/environment";
import type { JsonValue } from "$lib/storage/decoders";

export function readJSON(key: string): JsonValue | undefined {
  if (!browser) return undefined;
  try {
    const raw = localStorage.getItem(key);
    return raw == null ? undefined : JSON.parse(raw);
  } catch {
    return undefined;
  }
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- callers persist heterogeneous app state; JSON.stringify inside this fn is the serializer at the boundary.
export function writeJSON(key: string, value: unknown): void {
  if (!browser) return;
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {}
}

export function removeJSON(key: string): void {
  if (!browser) return;
  try {
    localStorage.removeItem(key);
  } catch {}
}

export type StorageArea = "local" | "session";

/**
 * Reads a raw (non-JSON) value. Reaching for `localStorage` at all throws when
 * the browser blocks site data, so the access itself has to sit inside the try.
 */
export function readRaw(area: StorageArea, key: string): string | null {
  if (!browser) return null;
  try {
    return (area === "local" ? localStorage : sessionStorage).getItem(key);
  } catch {
    return null;
  }
}

export function writeRaw(area: StorageArea, key: string, value: string): void {
  if (!browser) return;
  try {
    (area === "local" ? localStorage : sessionStorage).setItem(key, value);
  } catch {}
}

export function removeRaw(area: StorageArea, key: string): void {
  if (!browser) return;
  try {
    (area === "local" ? localStorage : sessionStorage).removeItem(key);
  } catch {}
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is arbitrary parsed JSON from readJSON(); this predicate is the boundary check itself.
export function isFutureSchema(raw: unknown, current: number): boolean {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- typeof-object is the only discriminator for arbitrary parsed JSON before the "v" probe.
  if (typeof raw !== "object" || raw === null || !("v" in raw)) return false;
  return raw.v !== current;
}

export function onStorageKey(key: string, handler: () => void): () => void {
  if (!browser) return () => {};
  const listener = (e: StorageEvent) => {
    if (e.key === key) handler();
  };
  window.addEventListener("storage", listener);
  return () => window.removeEventListener("storage", listener);
}

export function onPageHide(handler: () => void): () => void {
  if (!browser) return () => {};
  window.addEventListener("pagehide", handler);
  return () => window.removeEventListener("pagehide", handler);
}
