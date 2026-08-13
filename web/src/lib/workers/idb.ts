import { idbError } from "./idb-error";
export const IDB_VERSION = 1;

const idbConnections = new Map<string, Promise<IDBDatabase>>();

export function openIdb(dbName: string, storeName: string): Promise<IDBDatabase> {
  const cacheKey = `${dbName} ${storeName}`;
  let p = idbConnections.get(cacheKey);
  if (!p) {
    p = new Promise<IDBDatabase>((resolve, reject) => {
      const req = indexedDB.open(dbName, IDB_VERSION);
      req.onupgradeneeded = () => {
        if (!req.result.objectStoreNames.contains(storeName)) {
          req.result.createObjectStore(storeName);
        }
      };
      req.onsuccess = () => {
        const db = req.result;
        // One-store-per-db invariant: a second store added to an existing db at
        // the pinned IDB_VERSION never fires onupgradeneeded, so a missing store
        // here is a hard error rather than a silent NotFound later.
        if (!db.objectStoreNames.contains(storeName)) {
          db.close();
          reject(
            idbError(
              new DOMException(`IDB store "${storeName}" missing in ${dbName}`, "NotFoundError"),
              "open",
            ),
          );
          return;
        }
        // Evict the cached connection if it is force-closed (origin eviction) or
        // a version change is requested elsewhere, so the next call reopens.
        db.onclose = () => {
          idbConnections.delete(cacheKey);
        };
        db.onversionchange = () => {
          db.close();
          idbConnections.delete(cacheKey);
        };
        resolve(db);
      };
      req.onerror = () => reject(idbError(req.error, "open"));
    });
    idbConnections.set(cacheKey, p);
    // A rejected open (e.g. missing store) must not latch the cache forever.
    void p.catch(() => {
      idbConnections.delete(cacheKey);
    });
  }
  return p;
}

export function runTxVoid(
  db: IDBDatabase,
  store: string,
  mode: IDBTransactionMode,
  fn: (objectStore: IDBObjectStore) => void,
): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    const tx = db.transaction(store, mode);
    fn(tx.objectStore(store));
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(idbError(tx.error, "transaction"));
    tx.onabort = () => reject(idbError(tx.error, "transaction abort"));
  });
}

export async function idbGet<T>(
  db: IDBDatabase,
  store: string,
  key: IDBValidKey,
): Promise<T | undefined> {
  return await new Promise<T | undefined>((resolve, reject) => {
    const tx = db.transaction(store, "readonly");
    const req = tx.objectStore(store).get(key);
    // SAFETY: IDB request results are `any`; records for these keys are written by
    // callers of idbPut with the same caller-named shape T that idbGet<T> declares.
    req.onsuccess = () => resolve(req.result as T | undefined);
    req.onerror = () => reject(idbError(req.error, "get"));
  });
}

export function idbPut(
  db: IDBDatabase,
  store: string,
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- generic IDB value sink: callers pass ArrayBuffer, number, and caller-named T values (storage.ts, opfs-retention.ts, offline/meta.ts, service-worker.ts); IndexedDB structured-clones any value
  value: unknown,
  key?: IDBValidKey,
): Promise<void> {
  return runTxVoid(db, store, "readwrite", (s) => {
    if (key === undefined) s.put(value);
    else s.put(value, key);
  });
}

export function idbDelete(db: IDBDatabase, store: string, key: IDBValidKey): Promise<void> {
  return runTxVoid(db, store, "readwrite", (s) => {
    s.delete(key);
  });
}

export async function idbScan<T>(
  db: IDBDatabase,
  store: string,
  prefix: string,
): Promise<Record<string, T>> {
  const out: Record<string, T> = {};
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction(store, "readonly");
    const request = tx.objectStore(store).openCursor();
    request.onsuccess = () => {
      const cursor = request.result;
      if (!cursor) {
        resolve();
        return;
      }
      const key = cursor.key;
      // eslint-disable-next-line anti-slop/no-runtime-typeof -- IDB cursor keys are polymorphic (IDBValidKey); this store writes only string keys, so this filters our records at the read boundary
      if (typeof key === "string" && key.startsWith(prefix)) {
        // SAFETY: cursor.value is `any` from IDB; records under these string keys were
        // written via idbPut with the caller-named shape T that idbScan<T> declares.
        out[key.slice(prefix.length)] = cursor.value as T;
      }
      cursor.continue();
    };
    request.onerror = () => reject(idbError(request.error, "request"));
  });
  return out;
}
