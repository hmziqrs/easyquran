/* ════════════════════════════════════════════════════════════════════════
   opfs-cache.ts — THIN Quran adapter over generic download/storage/cache
   primitives.

   Maps an ArtifactSpec to a DownloadSpec and wires the OPFS→IDB→session
   fallback chain for the two Arabic SQLite artifacts, exactly preserving the
   behavior quran.worker.ts depends on:
     • OPFS read re-verifies size + sha256 (a corrupt cached file ⇒ redownload).
     • onProgress is forwarded ONLY on the durable (OPFS) fetch path.
     • Never throws on storage failure — only on an unverifiable download.

   Public API (ensureArtifact / CacheStore / CachedArtifact) is unchanged; the
   worker file is untouched. The generic primitives live in download.ts /
   storage.ts / cached.ts and contain no Quran-specific knowledge.

   The async OPFS API (isolation-free — no SharedArrayBuffer/COEP needed) stores
   the immutable files under an OPFS directory named by `contentVersion`, per
   docs/quran-web-delivery.md §6.2. IDB is the fallback when OPFS is unavailable
   (Safari <17, private mode, quota denial). All APIs here are Worker-safe.
   ════════════════════════════════════════════════════════════════════════ */

import type { ArtifactSpec } from "$lib/data/quran-types";
import { downloadBytes, verifyBytes, type DownloadSpec, type ProgressFn } from "./download";
import { createIdbStore, createOpfsStore, hasOpfs } from "./storage";
import { ensureCached } from "./cached";

const ROOT_DIR = "easyquran";
const QURAN_DB = "easyquran-quran";
const QURAN_STORE = "artifacts";

export type CacheStore = "opfs" | "idb" | "session";
export interface CachedArtifact {
  bytes: Uint8Array<ArrayBuffer>;
  store: CacheStore;
}

function toDownloadSpec(spec: ArtifactSpec): DownloadSpec {
  return {
    url: spec.downloadUrl,
    sizeBytes: spec.sizeBytes,
    sha256: spec.sha256,
    label: spec.id,
  };
}

export async function ensureArtifact(
  spec: ArtifactSpec,
  contentVersion: string,
  onProgress?: (loaded: number, total: number) => void,
): Promise<CachedArtifact> {
  const dl = toDownloadSpec(spec);
  // OPFS keys files as `<id>.sqlite`; IDB keys entries as just `<id>` — each
  // store owns its own key convention, so the two are kept distinct here to
  // match the on-disk / in-DB layout of the previous implementation.
  const opfsKey = `${spec.id}.sqlite`;
  const idbKey = spec.id;
  const progress: ProgressFn | undefined = onProgress
    ? (p) => onProgress(p.loaded, p.total)
    : undefined;

  if (hasOpfs()) {
    try {
      const opfs = createOpfsStore(ROOT_DIR);
      const cached = await opfs.get(contentVersion, opfsKey);
      if (cached) {
        try {
          await verifyBytes(cached, dl);
          return { bytes: cached, store: "opfs" };
        } catch {
          /* corrupt cached file ⇒ fall through to a fresh download+store */
        }
      }
      const bytes = await downloadBytes(dl, progress);
      await opfs.put(contentVersion, opfsKey, bytes);
      return { bytes, store: "opfs" };
    } catch (err) {
      console.warn(`[opfs-cache] OPFS unavailable for ${spec.id}, falling back:`, err);
    }
  }

  const idb = createIdbStore(QURAN_DB, QURAN_STORE);
  const r = await ensureCached(dl, { store: idb, version: contentVersion, key: idbKey });
  return { bytes: r.bytes, store: r.from === "store" ? "idb" : "session" };
}
