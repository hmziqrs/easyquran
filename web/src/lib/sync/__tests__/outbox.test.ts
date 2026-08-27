import { createOutbox, idbQueueStorage, memoryQueueStorage, type Outbox, type SyncMutationDraft } from "$lib/sync/outbox";
import type { SyncMutation } from "$lib/sync/types";
import { afterEach, beforeEach, describe, expect, it } from "vite-plus/test";

async function seed(outbox: Outbox, domain: string, count: number): Promise<SyncMutation[]> {
  const queued: SyncMutation[] = [];
  for (let i = 0; i < count; i += 1) queued.push(await outbox.enqueue(domain, `p${i}`));
  return queued;
}

describe("Outbox (memory backend)", () => {
  it("walks FIFO order across take batches as entries are removed", async () => {
    const outbox = createOutbox(memoryQueueStorage());
    const queued = await seed(outbox, "bookmarks", 5);

    expect((await outbox.take("bookmarks", 2)).map((m) => m.payload)).toEqual(["p0", "p1"]);
    // take is a peek: without removal the same head comes back (failed rounds re-take it).
    expect((await outbox.take("bookmarks", 2)).map((m) => m.payload)).toEqual(["p0", "p1"]);

    await outbox.remove("bookmarks", queued.slice(0, 2).map((m) => m.id));
    expect((await outbox.take("bookmarks", 2)).map((m) => m.payload)).toEqual(["p2", "p3"]);

    await outbox.remove("bookmarks", queued.slice(2, 4).map((m) => m.id));
    expect((await outbox.take("bookmarks", 2)).map((m) => m.payload)).toEqual(["p4"]);

    await outbox.remove("bookmarks", [queued[4]!.id]);
    expect(await outbox.take("bookmarks", 2)).toEqual([]);
  });

  it("keeps order when removing mid-queue entries", async () => {
    const outbox = createOutbox(memoryQueueStorage());
    const queued = await seed(outbox, "bookmarks", 3);
    const middle = queued[1]!;

    await outbox.remove("bookmarks", [middle.id]);

    expect((await outbox.take("bookmarks", 10)).map((m) => m.payload)).toEqual(["p0", "p2"]);
    expect(await outbox.count("bookmarks")).toBe(2);
  });

  it("isolates domains in count/all/take and orders FIFO by insertion", async () => {
    const outbox = createOutbox(memoryQueueStorage());
    await seed(outbox, "bookmarks", 2);
    await seed(outbox, "notes", 1);
    await seed(outbox, "bookmarks", 1);

    expect(await outbox.count("bookmarks")).toBe(3);
    expect(await outbox.count("notes")).toBe(1);
    expect((await outbox.all()).map((m) => `${m.domain}:${String(m.payload)}:${m.seq}`)).toEqual([
      "bookmarks:p0:1",
      "bookmarks:p1:2",
      "notes:p0:3",
      "bookmarks:p0:4",
    ]);
    expect((await outbox.take("notes", 10)).map((m) => m.seq)).toEqual([3]);
  });

  it("allocates storage-scoped keys that are never reused after removal", async () => {
    const outbox = createOutbox(memoryQueueStorage());
    const first = await seed(outbox, "bookmarks", 2);
    const notesFirst = await outbox.enqueue("notes", "n0");

    // Keys come from one storage-scoped generator (like IDB autoincrement), so
    // they are unique across domains and never reused.
    expect(first.map((m) => m.seq)).toEqual([1, 2]);
    expect(notesFirst.seq).toBe(3);

    await outbox.remove("bookmarks", [first[0]!.id]);
    const next = await outbox.enqueue("bookmarks", "p2");
    expect(next.seq).toBe(4);
  });

  it("keeps key allocation unique under two outbox instances over one storage", async () => {
    const storage = memoryQueueStorage();
    const tabA = createOutbox(storage);
    const tabB = createOutbox(storage);

    const interleaved = await Promise.all([
      tabA.enqueue("bookmarks", "a0"),
      tabB.enqueue("bookmarks", "b0"),
      tabA.enqueue("bookmarks", "a1"),
      tabB.enqueue("bookmarks", "b1"),
    ]);

    const seqs = interleaved.map((m) => m.seq).sort((a, b) => a - b);
    expect(seqs).toEqual([1, 2, 3, 4]);
    expect((await tabA.take("bookmarks", 10)).map((m) => m.seq)).toEqual([1, 2, 3, 4]);
  });

  it("reads back an enqueued mutation immediately (durability before return)", async () => {
    const storage = memoryQueueStorage();
    const writer = createOutbox(storage);
    await writer.enqueue("bookmarks", "x");

    const reader = createOutbox(storage);
    expect(await reader.count("bookmarks")).toBe(1);
    expect((await reader.take("bookmarks", 10)).map((m) => m.payload)).toEqual(["x"]);
  });

  it("serializes concurrent enqueues into unique keys in FIFO order", async () => {
    const outbox = createOutbox(memoryQueueStorage());
    const mutations = await Promise.all(
      Array.from({ length: 20 }, (_, i) => outbox.enqueue("bookmarks", i)),
    );

    const seqs = mutations.map((m) => m.seq).sort((a, b) => a - b);
    expect(seqs).toEqual(Array.from({ length: 20 }, (_, i) => i + 1));
    expect((await outbox.take("bookmarks", 20)).map((m) => m.payload)).toEqual(
      Array.from({ length: 20 }, (_, i) => i),
    );
  });

  it("clear(domain) empties only that domain and reports the removed count", async () => {
    const outbox = createOutbox(memoryQueueStorage());
    await seed(outbox, "bookmarks", 2);
    await seed(outbox, "notes", 1);

    expect(await outbox.clear("bookmarks")).toBe(2);
    expect(await outbox.count("bookmarks")).toBe(0);
    expect(await outbox.count("notes")).toBe(1);
    expect(await outbox.clear("bookmarks")).toBe(0);
  });

  it("falls back to the memory backend outside the browser", async () => {
    const outbox = createOutbox();
    expect(outbox.storage.kind).toBe("memory");
    await outbox.enqueue("bookmarks", "x");
    expect(await outbox.count("bookmarks")).toBe(1);
  });
});

// --- Minimal indexedDB fake: exactly the IDBFactory subset the outbox uses
// --- (open with version + upgrade, tx, autoincrement put, index cursors,
// --- delete, store introspection for the shape assertions below).

interface FakeReq<T> {
  result: T;
  onsuccess: ((ev: FakeReq<T>) => void) | null;
}

interface FakeCursor {
  key: string | number;
  primaryKey: number;
  value: SyncMutationDraft;
  delete(): void;
  continue(): void;
}

interface FakeIndex {
  openCursor(range?: { readonly lower: string; readonly upper: string }): FakeReq<FakeCursor | null>;
}

interface FakeStore {
  autoIncrement: boolean;
  indexNames: readonly string[];
  data: Map<number, SyncMutationDraft>;
  openCursor(): FakeReq<FakeCursor | null>;
  put(value: SyncMutationDraft, key?: number): FakeReq<number>;
  delete(key: number): FakeReq<undefined>;
  index(name: string): FakeIndex;
  createIndex(name: string, keyPath: "domain"): FakeIndex;
}

interface FakeTx {
  objectStore(name: string): FakeStore;
  oncomplete: (() => void) | null;
  onerror: (() => void) | null;
  onabort: (() => void) | null;
}

interface FakeDB {
  version: number;
  stores: Map<string, FakeStore>;
  /** One entry per transaction, in open order: the mode the caller requested. */
  readonly txLog: string[];
  objectStoreNames: { contains(name: string): boolean };
  createObjectStore(name: string, options?: { autoIncrement?: boolean }): FakeStore;
  deleteObjectStore(name: string): void;
  transaction(name: string, mode?: IDBTransactionMode): FakeTx;
}

interface FakeOpenRequest {
  result: FakeDB;
  error: DOMException | null;
  onsuccess: ((ev: FakeOpenRequest) => void) | null;
  onerror: ((ev: FakeOpenRequest) => void) | null;
  onupgradeneeded: ((ev: FakeOpenRequest) => void) | null;
}

function installFakeIndexedDB(): Map<string, FakeDB> {
  const dbByName = new Map<string, FakeDB>();

  function makeReq<T>(result: T): FakeReq<T> {
    const req: FakeReq<T> = { result, onsuccess: null };
    queueMicrotask(() => req.onsuccess?.(req));
    return req;
  }

  function makeStore(autoIncrement: boolean): FakeStore {
    const data = new Map<number, SyncMutationDraft>();
    const indexes = new Map<string, FakeIndex>();
    const nextKey = (): number => {
      let max = 0;
      for (const key of data.keys()) if (key > max) max = key;
      return max + 1;
    };
    const cursorReq = (
      keyOf: (key: number, record: SyncMutationDraft) => string | number,
      matches?: (key: number, record: SyncMutationDraft) => boolean,
    ): FakeReq<FakeCursor | null> => {
      const req: FakeReq<FakeCursor | null> = { result: null, onsuccess: null };
      const rows = [...data.entries()]
        .filter(([key, record]) => matches?.(key, record) ?? true)
        .sort((a, b) => a[0] - b[0]);
      let index = 0;
      const advance = (): void => {
        if (index >= rows.length) {
          req.result = null;
        } else {
          const [key, record] = rows[index]!;
          req.result = {
            key: keyOf(key, record),
            primaryKey: key,
            value: record,
            delete: () => {
              data.delete(key);
            },
            continue: () => {
              index += 1;
              advance();
            },
          };
        }
        queueMicrotask(() => req.onsuccess?.(req));
      };
      advance();
      return req;
    };
    const openStoreCursor = (): FakeReq<FakeCursor | null> => cursorReq((key) => key);
    const makeIndex = (): FakeIndex => ({
      openCursor(range) {
        return cursorReq(
          (_key, record) => record.domain,
          (_key, record) => range === undefined || record.domain === range.lower,
        );
      },
    });
    const store: FakeStore = {
      autoIncrement,
      get indexNames() {
        return [...indexes.keys()];
      },
      data,
      openCursor: () => openStoreCursor(),
      put: (value, key) => {
        const allocated = key ?? nextKey();
        data.set(allocated, value);
        return makeReq(allocated);
      },
      delete: (key) => {
        data.delete(key);
        return makeReq(undefined);
      },
      index: (name) => {
        const existing = indexes.get(name);
        if (existing === undefined) throw new Error(`fake idb: no index "${name}"`);
        return existing;
      },
      createIndex: (name, _keyPath) => {
        const created = makeIndex();
        indexes.set(name, created);
        return created;
      },
    };
    return store;
  }

  function makeDB(version: number): FakeDB {
    const stores = new Map<string, FakeStore>();
    const txLog: string[] = [];
    return {
      version,
      stores,
      txLog,
      objectStoreNames: { contains: (name) => stores.has(name) },
      createObjectStore: (name, options) => {
        const store = makeStore(options?.autoIncrement === true);
        stores.set(name, store);
        return store;
      },
      deleteObjectStore: (name) => {
        stores.delete(name);
      },
      transaction: (name, mode) => {
        const store = stores.get(name);
        if (!store) throw new Error(`fake idb: no store "${name}"`);
        txLog.push(mode ?? "readonly");
        const tx: FakeTx = {
          objectStore: () => store,
          oncomplete: null,
          onerror: null,
          onabort: null,
        };
        // Macrotask, not microtask: cursor-walk requests schedule microtask
        // onsuccess chains; oncomplete must land after the whole walk so a
        // clear() counting deletions via the cursor resolves with the count.
        setTimeout(() => tx.oncomplete?.(), 0);
        return tx;
      },
    };
  }

  const idb = {
    open(name: string, version: number): FakeOpenRequest {
      const existing = dbByName.get(name);
      const upgrade = existing === undefined || version > existing.version;
      const db = existing ?? makeDB(version);
      if (upgrade) db.version = version;
      if (existing === undefined) dbByName.set(name, db);
      const req: FakeOpenRequest = {
        result: db,
        error: null,
        onsuccess: null,
        onerror: null,
        onupgradeneeded: null,
      };
      queueMicrotask(() => {
        if (upgrade) req.onupgradeneeded?.(req);
        req.onsuccess?.(req);
      });
      return req;
    },
  };

  // SAFETY: test seam — globalThis is widened to plain indexedDB/IDBKeyRange slots so this in-file fake (exactly the IDBFactory subset the outbox reads) can replace the real globals for the test run.
  const globals = globalThis as { indexedDB: unknown; IDBKeyRange: unknown };
  globals.indexedDB = idb;
  globals.IDBKeyRange = {
    only: (value: string) => ({ lower: value, upper: value }),
  };
  return dbByName;
}

describe("Outbox (idb backend)", () => {
  let dbs: Map<string, FakeDB>;

  beforeEach(() => {
    dbs = installFakeIndexedDB();
  });

  afterEach(() => {
    // SAFETY: teardown of the test seam — globalThis is widened to the optional indexedDB/IDBKeyRange slots; delete is a no-op when no fake was installed.
    const globals = globalThis as { indexedDB?: unknown; IDBKeyRange?: unknown };
    delete globals.indexedDB;
    delete globals.IDBKeyRange;
  });

  it("creates the store with autoincrement keys and a by_domain index at v2", async () => {
    const outbox = createOutbox(idbQueueStorage());
    await outbox.enqueue("bookmarks", "x");

    const db = dbs.get("easyquran-sync");
    expect(db).toBeDefined();
    expect(db!.version).toBe(2);
    const store = db!.stores.get("outbox")!;
    expect(store.autoIncrement).toBe(true);
    expect(store.indexNames).toContain("by_domain");
  });

  it("stores FIFO by insertion, isolates domains, and keeps keys unique across instances", async () => {
    const tabA = createOutbox(idbQueueStorage());
    const tabB = createOutbox(idbQueueStorage());

    const interleaved = await Promise.all([
      tabA.enqueue("bookmarks", "p0"),
      tabB.enqueue("bookmarks", "p1"),
      tabA.enqueue("notes", "n0"),
      tabB.enqueue("bookmarks", "p2"),
    ]);

    const seqs = interleaved.map((m) => m.seq).sort((a, b) => a - b);
    expect(seqs).toEqual([1, 2, 3, 4]);

    expect((await tabA.take("bookmarks", 10)).map((m) => m.payload)).toEqual(["p0", "p1", "p2"]);
    expect(await tabB.count("notes")).toBe(1);

    const firstBatch = await tabA.take("bookmarks", 1);
    await tabA.remove("bookmarks", [firstBatch[0]!.id]);
    expect((await tabB.take("bookmarks", 10)).map((m) => m.seq)).toEqual([2, 4]);
    expect((await tabA.all()).length).toBe(3);
  });

  it("clear(domain) removes only that domain's rows", async () => {
    const outbox = createOutbox(idbQueueStorage());
    await seed(outbox, "bookmarks", 2);
    await seed(outbox, "notes", 1);

    const removed = await outbox.clear("bookmarks");
    expect(removed).toBe(2);
    expect(await outbox.count("bookmarks")).toBe(0);
    expect(await outbox.count("notes")).toBe(1);
  });

  it("clear(domain) wipes in one readwrite transaction (no read-then-delete window)", async () => {
    const outbox = createOutbox(idbQueueStorage());
    await seed(outbox, "bookmarks", 2);
    await seed(outbox, "notes", 1);
    const db = dbs.get("easyquran-sync")!;

    db.txLog.length = 0;
    const removed = await outbox.clear("bookmarks");

    // Exactly one transaction, and it is the write itself: a cross-tab enqueue
    // between a separate read and delete (the old shape) cannot reappear.
    expect(removed).toBe(2);
    expect(db.txLog).toEqual(["readwrite"]);
    expect(await outbox.count("bookmarks")).toBe(0);
    expect(await outbox.count("notes")).toBe(1);
  });
});
