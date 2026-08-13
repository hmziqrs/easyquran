import { idbDelete, idbGet, idbPut, openIdb, runTxVoid } from "$lib/workers/idb";
import { afterEach, beforeEach, describe, expect, it } from "vite-plus/test";

interface FakeTx {
  aborted: boolean;
  error: DOMException | null;
  oncomplete: ((ev: unknown) => void) | null;
  onerror: ((ev: unknown) => void) | null;
  onabort: ((ev: unknown) => void) | null;
  objectStore(name: string): FakeStore;
  abort(): void;
}

interface FakeStore {
  data: Map<unknown, unknown>;
  transaction: FakeTx;
  get(key: unknown): { result: unknown; onsuccess: ((ev: unknown) => void) | null };
  put(
    value: unknown,
    key?: unknown,
  ): { result: unknown; onsuccess: ((ev: unknown) => void) | null };
  delete(key: unknown): { result: unknown; onsuccess: ((ev: unknown) => void) | null };
}

interface FakeDB {
  objectStoreNames: { contains(name: string): boolean };
  stores: Map<string, FakeStore>;
  createObjectStore(name: string): FakeStore;
  transaction(name: string, mode: IDBTransactionMode): FakeTx;
}

function installFakeIndexedDB(): {
  dbByName: Map<string, FakeDB>;
} {
  const dbByName = new Map<string, FakeDB>();

  function makeStore(): FakeStore {
    const data = new Map<unknown, unknown>();
    function makeReq(result: unknown): {
      result: unknown;
      onsuccess: ((ev: unknown) => void) | null;
    } {
      const req = {
        result,
        onsuccess: null as ((ev: unknown) => void) | null,
      };
      queueMicrotask(() => req.onsuccess?.(req));
      return req;
    }
    const store = {
      data,
      transaction: null as unknown as FakeTx,
      get: (key: unknown) => makeReq(data.get(key)),
      put: (value: unknown, key?: unknown) => {
        if (key !== undefined) data.set(key, value);
        return makeReq(key ?? null);
      },
      delete: (key: unknown) => {
        data.delete(key);
        return makeReq(undefined);
      },
    } as FakeStore;
    return store;
  }

  function makeTx(db: FakeDB, name: string): FakeTx {
    const store = db.stores.get(name) ?? makeStore();
    db.stores.set(name, store);
    const tx: FakeTx = {
      aborted: false,
      error: null,
      oncomplete: null,
      onerror: null,
      onabort: null,
      objectStore: () => store,
      abort: () => {
        tx.aborted = true;
        tx.error = new DOMException("aborted", "AbortError");
        queueMicrotask(() => tx.onabort?.(tx));
      },
    };
    store.transaction = tx;
    queueMicrotask(() => {
      if (!tx.aborted) {
        tx.oncomplete?.(tx);
      }
    });
    return tx;
  }

  function makeDB(): FakeDB {
    const db: FakeDB = {
      objectStoreNames: { contains: (name) => db.stores.has(name) },
      stores: new Map(),
      createObjectStore: (name) => {
        const s = makeStore();
        db.stores.set(name, s);
        return s;
      },
      transaction: (name) => makeTx(db, name),
    };
    return db;
  }

  const idb = {
    open(name: string, _version: number) {
      let db = dbByName.get(name);
      const isNew = !db;
      if (!db) {
        db = makeDB();
        dbByName.set(name, db);
      }
      const req = {
        result: db,
        error: null as DOMException | null,
        onsuccess: null as ((ev: unknown) => void) | null,
        onerror: null as ((ev: unknown) => void) | null,
        onupgradeneeded: null as ((ev: unknown) => void) | null,
      };
      queueMicrotask(() => {
        if (isNew) req.onupgradeneeded?.(req);
        req.onsuccess?.(req);
      });
      return req;
    },
  };

  (globalThis as { indexedDB: unknown }).indexedDB = idb;
  return { dbByName };
}

describe("idb helpers", () => {
  beforeEach(() => {
    installFakeIndexedDB();
  });

  afterEach(() => {
    delete (globalThis as { indexedDB?: unknown }).indexedDB;
  });

  it("openIdb reuses one connection promise per (db, store) pair", async () => {
    const a = openIdb("easyquran-sw-meta", "meta");
    const b = openIdb("easyquran-sw-meta", "meta");
    expect(a).toBe(b);
    const dbA = await a;
    const dbB = await b;
    expect(dbA).toBe(dbB);
  });

  it("openIdb caches separate connections per (db, store) pair", async () => {
    const meta = await openIdb("easyquran-sw-meta", "meta");
    const recency = await openIdb("easyquran-sw-meta", "recency");
    expect(meta).not.toBe(recency);
    expect((meta.objectStoreNames as { contains(n: string): boolean }).contains("meta")).toBe(true);
    expect((recency.objectStoreNames as { contains(n: string): boolean }).contains("recency")).toBe(
      true,
    );
  });

  it("runTxVoid rejects when the transaction is aborted", async () => {
    const db = await openIdb("easyquran-sw-meta", "meta");
    await expect(
      runTxVoid(db, "meta", "readwrite", (store) => {
        store.transaction.abort();
      }),
    ).rejects.toThrow(/aborted/i);
  });

  it("round-trips idbPut -> idbGet -> idbDelete -> idbGet(undefined)", async () => {
    const db = await openIdb("easyquran-sw-meta", "meta");
    await idbPut(db, "meta", { hello: "world" }, "k1");
    const got = await idbGet<{ hello: string }>(db, "meta", "k1");
    expect(got).toEqual({ hello: "world" });
    await idbDelete(db, "meta", "k1");
    const after = await idbGet(db, "meta", "k1");
    expect(after).toBeUndefined();
  });

  it("idbPut without an explicit key stores the value under an autogenerated key", async () => {
    const db = await openIdb("easyquran-sw-meta", "meta");
    await expect(idbPut(db, "meta", { x: 1 })).resolves.toBeUndefined();
  });
});

describe("pointer-style records", () => {
  beforeEach(() => {
    installFakeIndexedDB();
  });

  afterEach(() => {
    delete (globalThis as { indexedDB?: unknown }).indexedDB;
  });

  it("round-trips an active-pointer record under its sourceId key", async () => {
    const db = await openIdb("easyquran-pointers", "opfsPointers");
    const pointer = { sourceId: "en.sahih", activeFile: "en.sahih.sqlite" };
    await idbPut(db, "opfsPointers", pointer, "en.sahih");
    const got = await idbGet<{ sourceId: string; activeFile: string }>(
      db,
      "opfsPointers",
      "en.sahih",
    );
    expect(got).toEqual(pointer);
    await idbDelete(db, "opfsPointers", "en.sahih");
    expect(await idbGet(db, "opfsPointers", "en.sahih")).toBeUndefined();
  });

  it("keeps the pointer db isolated from the recency db", async () => {
    const pointerDb = await openIdb("easyquran-pointers", "opfsPointers");
    const metaDb = await openIdb("easyquran-meta", "lastUsed");
    expect(pointerDb).not.toBe(metaDb);
    await idbPut(pointerDb, "opfsPointers", { sourceId: "x", activeFile: "x.sqlite" }, "x");
    await idbPut(metaDb, "lastUsed", 123, "x");
    expect(
      (pointerDb.objectStoreNames as { contains(n: string): boolean }).contains("opfsPointers"),
    ).toBe(true);
    expect((metaDb.objectStoreNames as { contains(n: string): boolean }).contains("lastUsed")).toBe(
      true,
    );
    expect(
      (pointerDb.objectStoreNames as { contains(n: string): boolean }).contains("lastUsed"),
    ).toBe(false);
  });
});
