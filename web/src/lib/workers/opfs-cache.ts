import type { ArtifactSpec } from "$lib/data/quran-types";
import { downloadBytes, verifyBytes, type DownloadSpec, type ProgressFn } from "./download";
import { createIdbStore, createOpfsStore, hasOpfs } from "./storage";
import { ensureCached } from "./cached";
import { stampLastUsed } from "./opfs-retention";

export const ROOT_DIR = "easyquran";
export const QURAN_DB = "easyquran-quran";
export const QURAN_STORE = "artifacts";

export type CacheStore = "opfs" | "idb" | "session";
export interface CachedArtifact {
  bytes: Uint8Array<ArrayBuffer>;
  store: CacheStore;
  downloaded: boolean;
}
export interface CachedArtifactInfo {
  readonly id: string;
  readonly store: CacheStore;
  readonly tag: string;
  readonly sizeBytes: number;
}

let artifactsDbPromise: Promise<IDBDatabase> | null = null;
function openArtifactsIdb(): Promise<IDBDatabase> {
  if (!artifactsDbPromise) {
    artifactsDbPromise = new Promise<IDBDatabase>((resolve, reject) => {
      const req = indexedDB.open(QURAN_DB, 1);
      req.onupgradeneeded = () => {
        const db = req.result;
        if (!db.objectStoreNames.contains(QURAN_STORE)) db.createObjectStore(QURAN_STORE);
      };
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
  }
  return artifactsDbPromise;
}

function toDownloadSpec(spec: ArtifactSpec): DownloadSpec {
  return {
    url: spec.downloadUrl,
    sizeBytes: spec.sizeBytes,
    label: spec.id,
  };
}

export async function ensureArtifact(
  spec: ArtifactSpec,
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
      const cached = await opfs.get(spec.id, opfsKey);
      if (cached) {
        try {
          await verifyBytes(cached, dl);
          void stampLastUsed(spec.id);
          return { bytes: cached, store: "opfs", downloaded: false };
        } catch {}
      }
      const bytes = await downloadBytes(dl, progress);
      await opfs.put(spec.id, opfsKey, bytes);
      await stampLastUsed(spec.id);
      return { bytes, store: "opfs", downloaded: true };
    } catch (err) {
      console.warn(`[opfs-cache] OPFS unavailable for ${spec.id}, falling back:`, err);
    }
  }

  const idb = createIdbStore(QURAN_DB, QURAN_STORE);
  const r = await ensureCached(dl, { store: idb, tag: spec.id, key: idbKey });
  const downloaded = r.from === "download";
  if (downloaded) await stampLastUsed(spec.id);
  else void stampLastUsed(spec.id);
  return { bytes: r.bytes, store: r.from === "store" ? "idb" : "session", downloaded };
}

async function listOpfsArtifacts(): Promise<CachedArtifactInfo[]> {
  const out: CachedArtifactInfo[] = [];
  if (!hasOpfs()) return out;
  let top: FileSystemDirectoryHandle;
  try {
    const root = await navigator.storage.getDirectory();
    top = await root.getDirectoryHandle(ROOT_DIR, { create: false });
  } catch {
    return out;
  }
  try {
    for await (const tagName of top.keys()) {
      let tagDir: FileSystemDirectoryHandle;
      try {
        tagDir = await top.getDirectoryHandle(tagName, { create: false });
      } catch {
        continue;
      }
      for await (const fileName of tagDir.keys()) {
        if (!fileName.endsWith(".sqlite")) continue;
        const id = fileName.slice(0, -".sqlite".length);
        let sizeBytes = 0;
        try {
          const fh = await tagDir.getFileHandle(fileName);
          sizeBytes = (await fh.getFile()).size;
        } catch {}
        out.push({ id, store: "opfs", tag: tagName, sizeBytes });
      }
    }
  } catch (err) {
    console.warn("[opfs-cache] OPFS listing failed:", err);
  }
  return out;
}

async function listIdbArtifacts(): Promise<CachedArtifactInfo[]> {
  const out: CachedArtifactInfo[] = [];
  try {
    const db = await openArtifactsIdb();
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(QURAN_STORE, "readonly");
      const req = tx.objectStore(QURAN_STORE).openCursor();
      req.onsuccess = () => {
        const cur = req.result;
        if (!cur) return;
        const compound = typeof cur.key === "string" ? cur.key : "";
        const sep = compound.indexOf(":");
        if (sep > 0) {
          const tag = compound.slice(0, sep);
          const id = compound.slice(sep + 1);
          const v = cur.value;
          const sizeBytes = v instanceof ArrayBuffer ? v.byteLength : 0;
          out.push({ id, store: "idb", tag, sizeBytes });
        }
        cur.continue();
      };
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } catch (err) {
    console.warn("[opfs-cache] IDB listing failed:", err);
  }
  return out;
}

export async function listCachedArtifacts(): Promise<CachedArtifactInfo[]> {
  const [opfs, idb] = await Promise.all([listOpfsArtifacts(), listIdbArtifacts()]);
  const byId = new Map<string, CachedArtifactInfo>();
  for (const info of idb) byId.set(info.id, info);
  for (const info of opfs) byId.set(info.id, info);
  return [...byId.values()];
}

export async function deleteCachedArtifact(id: string, tag: string): Promise<void> {
  if (hasOpfs()) {
    try {
      const root = await navigator.storage.getDirectory();
      const top = await root.getDirectoryHandle(ROOT_DIR, { create: false });
      const tagDir = await top.getDirectoryHandle(tag, { create: false });
      await tagDir.removeEntry(`${id}.sqlite`);
    } catch {}
  }
  try {
    const db = await openArtifactsIdb();
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(QURAN_STORE, "readwrite");
      tx.objectStore(QURAN_STORE).delete(`${tag}:${id}`);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } catch {}
}
