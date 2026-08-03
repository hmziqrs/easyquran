export interface ByteStore {
  get(version: string, key: string): Promise<Uint8Array<ArrayBuffer> | null>;
  put(version: string, key: string, bytes: Uint8Array<ArrayBuffer>): Promise<boolean>;
}

export function hasOpfs(): boolean {
  return typeof navigator !== "undefined" && !!navigator.storage?.getDirectory;
}

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

export async function pruneOpfs(rootDir: string, keepVersion: string): Promise<void> {
  if (!hasOpfs()) return;
  const root = await navigator.storage.getDirectory();
  const top = await root.getDirectoryHandle(rootDir);
  for await (const [name] of top.entries()) {
    if (name === keepVersion) continue;
    try {
      await top.removeEntry(name, { recursive: true });
    } catch (err) {
      console.warn(`[storage] pruneOpfs: failed to remove ${name}`, err);
    }
  }
}

export async function pruneIdb(dbName: string, storeName: string, keepVersion: string): Promise<void> {
  try {
    const db = await openIdb(dbName, storeName);
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(storeName, "readwrite");
      const store = tx.objectStore(storeName);
      const req = store.openCursor();
      req.onsuccess = () => {
        const cursor = req.result;
        if (!cursor) return;
        const key = cursor.key;
        if (typeof key === "string" && !key.startsWith(`${keepVersion}:`)) {
          cursor.delete();
        }
        cursor.continue();
      };
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } catch (err) {
    console.warn(`[storage] pruneIdb: failed`, err);
  }
}
