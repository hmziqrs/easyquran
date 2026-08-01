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
