/* ════════════════════════════════════════════════════════════════════════
   manifest.ts — resolve the Quran content manifest (scripts + versions).

   Tries the live /quran/v1 API (`/version` + `/scripts`) when `apiBase` is set;
   falls back to the BAKED constants in site.ts on any failure (network error,
   404, 5xx, timeout, or `apiBase` empty). Today the API is not live, so the
   baked path is what runs — and the reader works fully. When the API ships,
   `source` flips to "api" automatically.

   This module runs on the MAIN THREAD (it imports $env via site.ts); the worker
   receives the resolved manifest as a plain message, so no SvelteKit/$env code
   ever crosses into the worker bundle.
   ════════════════════════════════════════════════════════════════════════ */

import { QURAN } from "$lib/config/site";
import type { ArtifactSpec } from "$lib/data/quran-types";
import { decodeScriptsPayload, decodeVersionPayload } from "./wire";

export interface ResolvedManifest {
  contentVersion: string;
  searchVersion: string;
  scripts: readonly ArtifactSpec[];
  source: "api" | "baked";
}

const baked: ResolvedManifest = {
  contentVersion: QURAN.contentVersion,
  searchVersion: QURAN.searchVersion,
  scripts: QURAN.scripts,
  source: "baked",
};

/** Resolve the manifest, preferring the live API and degrading to baked. */
export async function resolveManifest(signal?: AbortSignal): Promise<ResolvedManifest> {
  if (!QURAN.apiBase) return baked;
  // Compose a 3s timeout with the caller's abort signal. Both are torn down in
  // the `finally` below so a fetch rejection or caller abort can never leak the
  // timer or leave an anonymous listener attached to the caller's signal.
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), 3000);
  const onAbort = () => ctrl.abort();
  signal?.addEventListener("abort", onAbort);
  try {
    const [vRes, sRes] = await Promise.all([
      fetch(`${QURAN.apiBase}/version`, {
        signal: ctrl.signal,
        headers: { accept: "application/json" },
      }),
      fetch(`${QURAN.apiBase}/scripts`, {
        signal: ctrl.signal,
        headers: { accept: "application/json" },
      }),
    ]);
    if (!vRes.ok || !sRes.ok) return baked;

    // /scripts + /version are untrusted API JSON. Both shapes (enveloped or
    // bare, per backend build) and every entry are validated by the shared wire
    // decoders — this module no longer hand-rolls the field-by-field rebuild.
    const sBody = await sRes.json();
    const scripts = decodeScriptsPayload(sBody);
    if (!scripts || scripts.length < 2) return baked;

    const vBody = await vRes.json();
    const version = decodeVersionPayload(vBody) ?? {
      contentVersion: null,
      searchVersion: null,
    };
    return {
      contentVersion: version.contentVersion ?? QURAN.contentVersion,
      searchVersion: version.searchVersion ?? QURAN.searchVersion,
      scripts,
      source: "api",
    };
  } catch {
    return baked;
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener("abort", onAbort);
  }
}
