export interface ByteStore {
  get(tag: string, key: string): Promise<Uint8Array<ArrayBuffer> | null>;
  put(tag: string, key: string, bytes: Uint8Array<ArrayBuffer>): Promise<boolean>;
}

export function hasOpfs(): boolean {
  return typeof navigator !== "undefined" && !!navigator.storage?.getDirectory;
}

export function createOpfsStore(rootDir: string): ByteStore {
  if (!hasOpfs()) throw new Error("OPFS is not available in this environment");

  async function tagDir(tag: string, create: boolean): Promise<FileSystemDirectoryHandle> {
    const root = await navigator.storage.getDirectory();
    const top = await root.getDirectoryHandle(rootDir, { create });
    return top.getDirectoryHandle(tag, { create });
  }

  return {
    async get(tag, key) {
      try {
        const dir = await tagDir(tag, false);
        const fh = await dir.getFileHandle(key);
        const file = await fh.getFile();
        return new Uint8Array(await file.arrayBuffer());
      } catch (err) {
        if (err instanceof DOMException && err.name === "NotFoundError") return null;
        throw err;
      }
    },
    async put(tag, key, bytes) {
      const dir = await tagDir(tag, true);
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
  const idbKey = (tag: string, key: string): string => `${tag}:${key}`;
  return {
    async get(tag, key) {
      try {
        const db = await openIdb(dbName, storeName);
        return await new Promise((resolve, reject) => {
          const tx = db.transaction(storeName, "readonly");
          const req = tx.objectStore(storeName).get(idbKey(tag, key));
          req.onsuccess = () =>
            resolve(req.result instanceof ArrayBuffer ? new Uint8Array(req.result) : null);
          req.onerror = () => reject(req.error);
        });
      } catch {
        return null;
      }
    },
    async put(tag, key, bytes) {
      try {
        const db = await openIdb(dbName, storeName);
        await new Promise<void>((resolve, reject) => {
          const tx = db.transaction(storeName, "readwrite");
          tx.objectStore(storeName).put(bytes.buffer, idbKey(tag, key));
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
