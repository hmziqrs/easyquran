/* ════════════════════════════════════════════════════════════════════════
   storage.ts — GENERIC, worker-safe byte stores (OPFS + IndexedDB).

   A ByteStore is intentionally DUMB: it stores and retrieves raw bytes keyed by
   (version, key). No verification, no domain knowledge — that belongs to the
   caller (see cached.ts). No $lib/$env/$app, no Svelte, no DOM-only APIs, no
   Quran types. Only relative imports + web-standard APIs
   (navigator.storage / FileSystemDirectoryHandle, indexedDB).
   ════════════════════════════════════════════════════════════════════════ */

export interface ByteStore {
  get(version: string, key: string): Promise<Uint8Array<ArrayBuffer> | null>;
  put(version: string, key: string, bytes: Uint8Array<ArrayBuffer>): Promise<boolean>;
}

export function hasOpfs(): boolean {
  return typeof navigator !== "undefined" && !!navigator.storage?.getDirectory;
}

/**
 * OPFS-backed ByteStore. Layout: `<rootDir>/<version>/<key>` (key is the file
 * name). `get` returns null when the file or any parent directory is absent
 * (NotFoundError) and throws on other storage errors. `put` creates the
 * directory chain and the file via `createWritable`. The factory rejects if
 * OPFS is unavailable — guard with `hasOpfs()` before constructing.
 */
export function createOpfsStore(rootDir: string): ByteStore {
  if (!hasOpfs()) throw new Error("OPFS is not available in this environment");

  async function versionDir(version: string, create: boolean): Promise<FileSystemDirectoryHandle> {
    const root = await navigator.storage.getDirectory();
    const top = await root.getDirectoryHandle(rootDir, { create });
    return top.getDirectoryHandle(version, { create });
  }

  return {
    async get(version, key) {
      try {
        const dir = await versionDir(version, false);
        const fh = await dir.getFileHandle(key);
        const file = await fh.getFile();
        return new Uint8Array(await file.arrayBuffer());
      } catch (err) {
        // Missing file or parent directory ⇒ not present; other errors propagate.
        if (err instanceof DOMException && err.name === "NotFoundError") return null;
        throw err;
      }
    },
    async put(version, key, bytes) {
      const dir = await versionDir(version, true);
      const fh = await dir.getFileHandle(key, { create: true });
      const writable = await fh.createWritable();
      await writable.write(bytes);
      await writable.close();
      return true;
    },
  };
}

// One shared IDBDatabase per (dbName, storeName) for the process lifetime —
// opening a fresh IDBDatabase per call leaks connections. Never closed; the
// cached promise is reused by every get/put on that pair.
const idbConnections = new Map<string, Promise<IDBDatabase>>();

function openIdb(dbName: string, storeName: string): Promise<IDBDatabase> {
  const cacheKey = `${dbName} ${storeName}`;
  let p = idbConnections.get(cacheKey);
  if (!p) {
    p = new Promise((resolve, reject) => {
      const req = indexedDB.open(dbName, 1);
      req.onupgradeneeded = () => {
        if (!req.result.objectStoreNames.contains(storeName)) {
          req.result.createObjectStore(storeName);
        }
      };
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
    idbConnections.set(cacheKey, p);
  }
  return p;
}

/**
 * IndexedDB-backed ByteStore. Generalizes the old idb-cache: one shared
 * IDBDatabase per (dbName, storeName). Entries are stored as ArrayBuffer under
 * key `<version>:<key>`. Both get and put swallow connection/transaction
 * errors (get → null, put → false) so callers can treat storage as best-effort.
 */
export function createIdbStore(dbName: string, storeName: string): ByteStore {
  const idbKey = (version: string, key: string): string => `${version}:${key}`;
  return {
    async get(version, key) {
      try {
        const db = await openIdb(dbName, storeName);
        return await new Promise((resolve, reject) => {
          const tx = db.transaction(storeName, "readonly");
          const req = tx.objectStore(storeName).get(idbKey(version, key));
          req.onsuccess = () =>
            resolve(req.result instanceof ArrayBuffer ? new Uint8Array(req.result) : null);
          req.onerror = () => reject(req.error);
        });
      } catch {
        return null;
      }
    },
    async put(version, key, bytes) {
      try {
        const db = await openIdb(dbName, storeName);
        await new Promise<void>((resolve, reject) => {
          const tx = db.transaction(storeName, "readwrite");
          tx.objectStore(storeName).put(bytes.buffer, idbKey(version, key));
          tx.oncomplete = () => resolve();
          tx.onerror = () => reject(tx.error);
        });
        return true;
      } catch {
        return false;
      }
    },
  };
}
