/* ════════════════════════════════════════════════════════════════════════
   idb-cache.ts — IndexedDB blob fallback for the Quran SQLite artifacts.

   Used when OPFS is unavailable (Safari <17, private mode, quota denial). Tiny
   key→ArrayBuffer store keyed `<contentVersion>:<scriptId>`. Worker-context
   only (no DOM); indexedDB + the promise wrappers are all Worker-safe.
   ════════════════════════════════════════════════════════════════════════ */

const DB_NAME = "easyquran-quran";
const STORE = "artifacts";
const key = (cv: string, id: string): string => `${cv}:${id}`;

// One shared connection for the worker's lifetime — opening a fresh
// IDBDatabase per call leaks connections. Never closed; cached promise is
// reused by open()/idbGet/idbSet.
let dbPromise: Promise<IDBDatabase> | null = null;

function open(): Promise<IDBDatabase> {
  if (dbPromise) return dbPromise;
  dbPromise = new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => {
      if (!req.result.objectStoreNames.contains(STORE)) req.result.createObjectStore(STORE);
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
  return dbPromise;
}

export async function idbGet(cv: string, id: string): Promise<Uint8Array<ArrayBuffer> | null> {
  try {
    const db = await open();
    return await new Promise((resolve, reject) => {
      const tx = db.transaction(STORE, "readonly");
      const req = tx.objectStore(STORE).get(key(cv, id));
      req.onsuccess = () =>
        resolve(req.result instanceof ArrayBuffer ? new Uint8Array(req.result) : null);
      req.onerror = () => reject(req.error);
    });
  } catch {
    return null;
  }
}

export async function idbSet(cv: string, id: string, bytes: Uint8Array<ArrayBuffer>): Promise<boolean> {
  try {
    const db = await open();
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(STORE, "readwrite");
      tx.objectStore(STORE).put(bytes.buffer, key(cv, id));
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
    return true;
  } catch {
    return false;
  }
}
