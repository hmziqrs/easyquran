import { browser } from "$app/environment";

export function readJSON(key: string): unknown {
  if (!browser) return undefined;
  try {
    const raw = localStorage.getItem(key);
    return raw == null ? undefined : JSON.parse(raw);
  } catch {
    return undefined;
  }
}

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

export function isFutureSchema(raw: unknown, current: number): boolean {
  if (typeof raw !== "object" || raw === null || !("v" in raw)) return false;
  return (raw as { v: unknown }).v !== current;
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
