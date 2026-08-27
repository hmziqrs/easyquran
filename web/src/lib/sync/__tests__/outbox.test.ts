import { createOutbox, idbQueueStorage, memoryQueueStorage, type Outbox } from "$lib/sync/outbox";
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

  it("isolates domains in count/all/take", async () => {
    const outbox = createOutbox(memoryQueueStorage());
    await seed(outbox, "bookmarks", 2);
    await seed(outbox, "notes", 1);

    expect(await outbox.count("bookmarks")).toBe(2);
    expect(await outbox.count("notes")).toBe(1);
    expect((await outbox.all()).length).toBe(3);
    expect((await outbox.take("notes", 10)).map((m) => m.seq)).toEqual([1]);
  });

  it("allocates monotonic per-domain seqs that are never reused after removal", async () => {
    const outbox = createOutbox(memoryQueueStorage());
    const first = await seed(outbox, "bookmarks", 2);
    const notesFirst = await outbox.enqueue("notes", "n0");

    expect(first.map((m) => m.seq)).toEqual([1, 2]);
    expect(notesFirst.seq).toBe(1);

    await outbox.remove("bookmarks", [first[0]!.id]);
    const next = await outbox.enqueue("bookmarks", "p2");
    expect(next.seq).toBe(3);
  });

  it("reads back an enqueued mutation immediately (durability before return)", async () => {
    const storage = memoryQueueStorage();
    const writer = createOutbox(storage);
    await writer.enqueue("bookmarks", "x");

    const reader = createOutbox(storage);
    expect(await reader.count("bookmarks")).toBe(1);
    expect((await reader.take("bookmarks", 10)).map((m) => m.payload)).toEqual(["x"]);
  });

  it("serializes concurrent enqueues into unique monotonic seqs", async () => {
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

  it("falls back to the memory backend outside the browser", async () => {
    const outbox = createOutbox();
    expect(outbox.storage.kind).toBe("memory");
    await outbox.enqueue("bookmarks", "x");
    expect(await outbox.count("bookmarks")).toBe(1);
  });

  it("rejects domain names containing the key separator", async () => {
    const outbox = createOutbox(memoryQueueStorage());
    await expect(outbox.enqueue("bad:domain", "x")).rejects.toThrow(/must not contain/);
  });
});

// --- Minimal indexedDB fake: exactly the IDBFactory subset idb.ts + the
// --- outbox's cursor read use (open, tx, put/get/delete, openCursor).

interface FakeReq<T> {
  result: T;
  onsuccess: ((ev: FakeReq<T>) => void) | null;
}

interface FakeCursor {
  key: string;
  value: unknown;
  continue(): void;
}

interface FakeStore {
  data: Map<string, unknown>;
  get(key: string): FakeReq<unknown>;
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- fake of IDBObjectStore.put; value is the opaque structured-clone payload idbPut passes through (its own param is `unknown`).
  put(value: unknown, key: string): FakeReq<string>;
  delete(key: string): FakeReq<undefined>;
  openCursor(): FakeReq<FakeCursor | null>;
}

interface FakeTx {
  objectStore(name: string): FakeStore;
  oncomplete: (() => void) | null;
  onerror: (() => void) | null;
  onabort: (() => void) | null;
}

interface FakeDB {
  objectStoreNames: { contains(name: string): boolean };
  createObjectStore(name: string): FakeStore;
  transaction(name: string): FakeTx;
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

  function makeStore(): FakeStore {
    const data = new Map<string, unknown>();
    return {
      data,
      get: (key) => makeReq(data.get(key)),
      put: (value, key) => {
        data.set(key, value);
        return makeReq(key);
      },
      delete: (key) => {
        data.delete(key);
        return makeReq(undefined);
      },
      openCursor: () => {
        // No makeReq here: only advance() may schedule onsuccess callbacks, one per cursor step.
        const req: FakeReq<FakeCursor | null> = { result: null, onsuccess: null };
        const keys = [...data.keys()].sort();
        let index = 0;
        const advance = (): void => {
          if (index >= keys.length) {
            req.result = null;
          } else {
            const key = keys[index]!;
            req.result = {
              key,
              value: data.get(key),
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
      },
    };
  }

  function makeDB(): FakeDB {
    const stores = new Map<string, FakeStore>();
    return {
      objectStoreNames: { contains: (name) => stores.has(name) },
      createObjectStore: (name) => {
        const store = makeStore();
        stores.set(name, store);
        return store;
      },
      transaction: (name) => {
        const store = stores.get(name);
        if (!store) throw new Error(`fake idb: no store "${name}"`);
        const tx: FakeTx = {
          objectStore: () => store,
          oncomplete: null,
          onerror: null,
          onabort: null,
        };
        queueMicrotask(() => tx.oncomplete?.());
        return tx;
      },
    };
  }

  const idb = {
    open(name: string, _version: number): FakeOpenRequest {
      const existing = dbByName.get(name);
      const db = existing ?? makeDB();
      const isNew = existing === undefined;
      if (isNew) dbByName.set(name, db);
      const req: FakeOpenRequest = {
        result: db,
        error: null,
        onsuccess: null,
        onerror: null,
        onupgradeneeded: null,
      };
      queueMicrotask(() => {
        if (isNew) req.onupgradeneeded?.(req);
        req.onsuccess?.(req);
      });
      return req;
    },
  };

  // SAFETY: test seam — globalThis is widened to a plain indexedDB slot so this in-file fake (exactly the IDBFactory subset the outbox reads) can replace the real factory for the test run.
  (globalThis as { indexedDB: unknown }).indexedDB = idb;
  return dbByName;
}

describe("Outbox (idb backend)", () => {
  beforeEach(() => {
    installFakeIndexedDB();
  });

  afterEach(() => {
    // SAFETY: teardown of the test seam — globalThis is widened to the optional indexedDB slot; delete is a no-op when no fake was installed.
    delete (globalThis as { indexedDB?: unknown }).indexedDB;
  });

  it("stores FIFO via padded keys, isolates domains, and survives a second outbox instance", async () => {
    const outbox = createOutbox(idbQueueStorage());
    await seed(outbox, "bookmarks", 3);
    await outbox.enqueue("notes", "n0");

    expect((await outbox.take("bookmarks", 10)).map((m) => m.payload)).toEqual(["p0", "p1", "p2"]);
    expect(await outbox.count("notes")).toBe(1);

    const reopened = createOutbox(idbQueueStorage());
    expect((await reopened.take("bookmarks", 10)).length).toBe(3);

    const firstBatch = await outbox.take("bookmarks", 1);
    await outbox.remove("bookmarks", [firstBatch[0]!.id]);
    expect((await reopened.take("bookmarks", 10)).map((m) => m.seq)).toEqual([2, 3]);
    expect((await outbox.all()).length).toBe(3);
  });
});
