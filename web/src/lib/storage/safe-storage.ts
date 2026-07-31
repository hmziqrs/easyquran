/* ════════════════════════════════════════════════════════════════════════
   safe-storage.ts — reusable, policy-aware localStorage mechanics.

   These are *mechanics only*: SSR/browser guards, safe JSON read+write, a
   schema-version gate, and an opt-in cross-tab subscription with explicit
   teardown. They deliberately do NOT decide *when* or *how often* a domain
   persists — each store keeps its own scheduling and side effects (notes
   debounce, prefs/consent write immediately, notifications couples writes to
   token registration). Avoid wiring every domain through one configurable
   "god helper": the options become harder to follow than the code they
   replace.
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";

/**
 * Read and JSON-parse `key`. Returns `undefined` on SSR, when the key is
 * absent, or when the stored value fails to parse. The caller is expected to
 * run the result through a domain decoder — the raw `unknown` is intentionally
 * never trusted as typed data.
 */
export function readJSON(key: string): unknown {
  if (!browser) return undefined;
  try {
    const raw = localStorage.getItem(key);
    return raw == null ? undefined : JSON.parse(raw);
  } catch {
    return undefined;
  }
}

/**
 * JSON-serialize and write `value` to `key`. No-op on SSR; swallows quota /
 * private-mode / disabled-storage errors (persistence is best-effort and must
 * never break the reading experience).
 */
export function writeJSON(key: string, value: unknown): void {
  if (!browser) return;
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* storage may be unavailable (private mode, quota) — non-fatal */
  }
}

/** Remove `key` from localStorage. No-op on SSR or when absent. */
export function removeJSON(key: string): void {
  if (!browser) return;
  try {
    localStorage.removeItem(key);
  } catch {
    /* non-fatal */
  }
}

/**
 * Schema-version gate. Returns true when `raw` carries an EXPLICIT `v` that
 * does not equal `current` — i.e. a future, incompatible shape that must be
 * discarded wholesale so a schema change can never half-load into runtime
 * state. A blob with NO `v` is legacy data from before the field existed and
 * should be migrated forward: the domain's decoder validates every field, and
 * the next write re-stamps the current version.
 */
export function isFutureSchema(raw: unknown, current: number): boolean {
  if (typeof raw !== "object" || raw === null || !("v" in raw)) return false;
  return (raw as { v: unknown }).v !== current;
}

/**
 * Subscribe to cross-tab `storage` events for a specific key. The `storage`
 * event only fires in *other* tabs (never the writer's own), so this is the
 * right primitive for multi-tab re-sync without echo. Returns a teardown
 * function that detaches the listener — call it from the owning store's
 * dispose path so listeners never outlive their store. No-op on SSR (returns a
 * noop teardown).
 */
export function onStorageKey(key: string, handler: () => void): () => void {
  if (!browser) return () => {};
  const listener = (e: StorageEvent) => {
    if (e.key === key) handler();
  };
  window.addEventListener("storage", listener);
  return () => window.removeEventListener("storage", listener);
}

/**
 * Subscribe to `pagehide` (covers tab close, reload, and bfcache eviction on
 * modern browsers). Use it to flush any deferred writes (e.g. a debounced note
 * save) before the page unloads so the last keystroke is durable. Returns a
 * teardown function. No-op on SSR.
 */
export function onPageHide(handler: () => void): () => void {
  if (!browser) return () => {};
  window.addEventListener("pagehide", handler);
  return () => window.removeEventListener("pagehide", handler);
}
