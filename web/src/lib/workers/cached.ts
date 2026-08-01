/* ════════════════════════════════════════════════════════════════════════
   cached.ts — GENERIC, worker-safe cache-or-download helper (single store).

   Tries a ByteStore first (re-verifying bytes when the spec carries a size or
   sha), and falls back to a fresh download, best-effort persisting the result.
   NEVER throws for storage or verification failure — only an unverifiable
   download can throw (from downloadBytes). No $lib/$env/$app, no Svelte, no
   DOM-only APIs, no Quran types. Relative imports only.
   ════════════════════════════════════════════════════════════════════════ */

import { downloadBytes, verifyBytes } from "./download";
import type { DownloadSpec, ProgressFn } from "./download";
import type { ByteStore } from "./storage";

export interface EnsureOptions {
  store?: ByteStore;
  version?: string;
  key?: string;
  onProgress?: ProgressFn;
}

export interface EnsureResult {
  bytes: Uint8Array<ArrayBuffer>;
  from: "store" | "download";
}

/**
 * Ensure `spec` is available as bytes: read+verify from the store if all of
 * (store, version, key) are given and the entry is present and valid; otherwise
 * download (forwarding `onProgress`) and best-effort persist. Storage and
 * verify-on-read failures never escape — they just trigger a download.
 */
export async function ensureCached(spec: DownloadSpec, opts: EnsureOptions): Promise<EnsureResult> {
  const { store, version, key, onProgress } = opts;

  if (store && version && key) {
    try {
      const cached = await store.get(version, key);
      if (cached) {
        if (spec.sizeBytes !== undefined || spec.sha256 !== undefined) {
          await verifyBytes(cached, spec);
        }
        return { bytes: cached, from: "store" };
      }
    } catch {
      // Storage error or verify mismatch: never throw — fall through.
    }
  }

  const bytes = await downloadBytes(spec, onProgress);
  if (store && version && key) {
    try {
      await store.put(version, key, bytes);
    } catch {
      /* best-effort persist */
    }
  }
  return { bytes, from: "download" };
}
