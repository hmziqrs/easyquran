/* ════════════════════════════════════════════════════════════════════════
   opfs-cache.ts — durable byte cache for the two Quran SQLite artifacts.

   The async OPFS API (isolation-free — no SharedArrayBuffer/COEP needed) stores
   the immutable files under an OPFS directory named by `contentVersion`, per
   docs/quran-web-delivery.md §6.2. Every read re-verifies sizeBytes + sha256, so
   a truncated/corrupt/evicted file simply triggers a redownload — the verify is
   the real guard, not a fragile temp+rename dance.

   Fallback chain: OPFS → IndexedDB (idb-cache) → per-session fetch (no
   durability, but the reader still works). All APIs here are Worker-safe.
   ════════════════════════════════════════════════════════════════════════ */

import type { ArtifactSpec } from "$lib/data/quran-types";
import { idbGet, idbSet } from "./idb-cache";

const ROOT_DIR = "easyquran";

/** An ArrayBuffer-backed byte view (what DOM write/digest APIs require). */
type Bytes = Uint8Array<ArrayBuffer>;

export type CacheStore = "opfs" | "idb" | "session";
export interface CachedArtifact {
  bytes: Bytes;
  store: CacheStore;
}

function hasOpfs(): boolean {
  return typeof navigator !== "undefined" && !!navigator.storage?.getDirectory;
}

async function sha256(data: BufferSource): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", data);
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/** Verify a downloaded buffer against the artifact's expected size + sha256. */
async function verify(buf: Bytes, spec: ArtifactSpec): Promise<Bytes> {
  if (buf.byteLength !== spec.sizeBytes) {
    throw new Error(`${spec.id}: size ${buf.byteLength} ≠ expected ${spec.sizeBytes}`);
  }
  if ((await sha256(buf)) !== spec.sha256) throw new Error(`${spec.id}: sha256 mismatch`);
  return buf;
}

/**
 * Fetch with identity encoding and verify size + sha256 before returning. The
 * body is STREAMED so bytes-received can be reported mid-flight for a progress
 * bar. The expected total is `spec.sizeBytes` (known up front) — `Content-Length`
 * is never read, so this introduces no cross-origin ExposeHeaders/CORS need.
 * `onProgress(loaded, total)` is invoked as bytes arrive (and once with 0 up
 * front); the full buffer is still assembled before hashing/writing, so memory
 * behavior and verification are unchanged from the previous arrayBuffer() path.
 */
async function fetchVerified(
  spec: ArtifactSpec,
  onProgress?: (loaded: number, total: number) => void,
): Promise<Bytes> {
  const res = await fetch(spec.downloadUrl, { headers: { "Accept-Encoding": "identity" } });
  if (!res.ok) throw new Error(`fetch ${spec.id}: HTTP ${res.status}`);
  const total = spec.sizeBytes;

  if (res.body) {
    const reader = res.body.getReader();
    try {
      const chunks: Uint8Array[] = [];
      let loaded = 0;
      onProgress?.(0, total);
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        if (value) {
          chunks.push(value);
          loaded += value.byteLength;
          onProgress?.(loaded, total);
        }
      }
      // Concatenate chunks into one buffer (same shape as the old arrayBuffer()).
      const buf = new Uint8Array(loaded);
      let off = 0;
      for (const c of chunks) {
        buf.set(c, off);
        off += c.byteLength;
      }
      return verify(buf, spec);
    } finally {
      // Release the body stream whether we finished, threw, or aborted.
      await reader.cancel().catch(() => {});
    }
  }

  // No body stream (e.g. opaque/unsupported response): buffer at once, no progress.
  return verify(new Uint8Array(await res.arrayBuffer()), spec);
}

/** Return the cached bytes for an artifact if present AND valid, else null. */
async function opfsReadValid(
  active: FileSystemDirectoryHandle,
  spec: ArtifactSpec,
): Promise<Bytes | null> {
  const name = `${spec.id}.sqlite`;
  try {
    const fh = await active.getFileHandle(name);
    const file = await fh.getFile();
    const bytes = new Uint8Array(await file.arrayBuffer());
    if (bytes.byteLength === spec.sizeBytes && (await sha256(bytes)) === spec.sha256) return bytes;
    await active.removeEntry(name).catch(() => {});
  } catch {
    /* not present */
  }
  return null;
}

/** Fetch, verify, and durably store an artifact under <contentVersion>/. */
async function opfsFetchAndStore(
  active: FileSystemDirectoryHandle,
  spec: ArtifactSpec,
  onProgress?: (loaded: number, total: number) => void,
): Promise<Bytes> {
  const bytes = await fetchVerified(spec, onProgress);
  const fh = await active.getFileHandle(`${spec.id}.sqlite`, { create: true });
  const writable = await fh.createWritable();
  await writable.write(bytes);
  await writable.close();
  return bytes;
}

/**
 * Ensure an artifact is available as bytes: OPFS (read or fetch+store), falling
 * back to IDB, then to a per-session fetch. Never throws on storage failure —
 * only on a genuinely unverifiable download. `onProgress` is forwarded only on
 * the OPFS fetch path (the durable, common case); cache hits and the per-session
 * fallback emit no progress.
 */
export async function ensureArtifact(
  spec: ArtifactSpec,
  contentVersion: string,
  onProgress?: (loaded: number, total: number) => void,
): Promise<CachedArtifact> {
  // 1. OPFS (read valid → else fetch+store)
  if (hasOpfs()) {
    try {
      const root = await navigator.storage.getDirectory();
      const eq = await root.getDirectoryHandle(ROOT_DIR, { create: true });
      const active = await eq.getDirectoryHandle(contentVersion, { create: true });
      const hit = await opfsReadValid(active, spec);
      if (hit) return { bytes: hit, store: "opfs" };
      const fetched = await opfsFetchAndStore(active, spec, onProgress);
      return { bytes: fetched, store: "opfs" };
    } catch (err) {
      console.warn(`[opfs-cache] OPFS unavailable for ${spec.id}, falling back:`, err);
    }
  }

  // 2. IDB
  const cached = await idbGet(contentVersion, spec.id);
  if (cached && cached.byteLength === spec.sizeBytes && (await sha256(cached)) === spec.sha256) {
    return { bytes: cached, store: "idb" };
  }

  // 3. per-session fetch (+ best-effort IDB persist)
  const bytes = await fetchVerified(spec);
  void idbSet(contentVersion, spec.id, bytes);
  return { bytes, store: "session" };
}
