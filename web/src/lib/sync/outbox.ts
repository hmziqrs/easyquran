import { browser } from "$app/environment";
import { asNumber, asObject, asString } from "$lib/storage";
import { idbError } from "$lib/workers/idb-error";
import { runTxVoid } from "$lib/workers/idb";
import type { SyncMutation } from "./types";

const SYNC_DB = "easyquran-sync";
const OUTBOX_STORE = "outbox";
const BY_DOMAIN_INDEX = "by_domain";
/**
 * v2 (2026-08): the outbox moved from caller-composed `${domain}:${padded seq}`
 * string keys to storage-allocated autoincrement keys + a `by_domain` index —
 * per-tab in-memory seq counters collided across tabs and `put` silently
 * overwrote queued rows. The bump recreates the store; queued mutations in
 * existing (dev-only) databases are dropped exactly once.
 */
const SYNC_DB_VERSION = 2;

function newMutationId(): string {
  // `in` (not typeof): randomUUID is missing on insecure origins (LAN http dev).
  if ("randomUUID" in crypto) return crypto.randomUUID();
  return `sync-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 12)}`;
}

/** A mutation before the storage assigns its durable `seq` (insertion key). */
export type SyncMutationDraft<P = unknown> = Omit<SyncMutation<P>, "seq">;

/**
 * Boundary decoder: a record read back from IndexedDB is unproven until parsed.
 * `seq` is the primary key the storage allocated at write time — it never comes
 * from the record body.
 */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is an arbitrary IDB record of uncertain provenance; this decoder IS the parser that validates it field by field.
function decodeMutation(raw: unknown, seq: number): SyncMutation | null {
  const obj = asObject(raw);
  if (!obj) return null;
  const id = asString(obj.id);
  const domain = asString(obj.domain);
  const queuedAt = asNumber(obj.queuedAt, 0, Number.POSITIVE_INFINITY);
  if (id === undefined || domain === undefined || queuedAt === undefined) {
    return null;
  }
  return { id, domain, seq, payload: obj.payload, queuedAt };
}

function compareMutations(a: SyncMutation, b: SyncMutation): number {
  return a.seq - b.seq;
}

/**
 * Pluggable durable storage behind the outbox. Two implementations: IndexedDB
 * (production) and an in-memory map (tests, and any environment without
 * IndexedDB). Both allocate the insertion key (`seq`) storage-side — IDB via
 * the store's own autoincrement generator, the memory impl via a counter scoped
 * to the storage instance — so key allocation is unique across every outbox
 * (and every tab) sharing the storage.
 */
export interface QueueStorage {
  readonly kind: "idb" | "memory";
  /** Up to `limit` mutations of `domain`, insertion-key ascending. */
  read(domain: string, limit: number): Promise<SyncMutation[]>;
  readAll(): Promise<SyncMutation[]>;
  /** Durable before the promise resolves; resolves the stored record with its allocated seq. */
  write<P>(record: SyncMutationDraft<P>): Promise<SyncMutation<P>>;
  /** Durable removal by insertion keys. */
  erase(seqs: number[]): Promise<void>;
  /** Durable removal of every queued mutation of `domain`; resolves the removed count. */
  clear(domain: string): Promise<number>;
}

export function memoryQueueStorage(): QueueStorage {
  const rows = new Map<number, SyncMutation>();
  // Storage-scoped key generator: mirrors IDB autoincrement — unique per
  // storage (not per Outbox instance) and never rewound by removals.
  let nextKey = 1;
  const readInto = (domain: string | null, limit: number): SyncMutation[] => {
    const matched: SyncMutation[] = [];
    for (const mutation of rows.values()) {
      if (domain === null || mutation.domain === domain) matched.push(mutation);
    }
    matched.sort(compareMutations);
    return matched.slice(0, limit);
  };
  return {
    kind: "memory",
    async read(domain, limit) {
      return readInto(domain, limit);
    },
    async readAll() {
      return readInto(null, Number.POSITIVE_INFINITY);
    },
    async write<P>(record: SyncMutationDraft<P>) {
      const stored: SyncMutation<P> = { ...record, seq: nextKey };
      rows.set(nextKey, stored);
      nextKey += 1;
      return stored;
    },
    async erase(seqs) {
      for (const seq of seqs) rows.delete(seq);
    },
    async clear(domain) {
      const seqs = [...rows.values()].filter((m) => m.domain === domain).map((m) => m.seq);
      for (const seq of seqs) rows.delete(seq);
      return seqs.length;
    },
  };
}

/**
 * Cursor read over the sync connection: walks the `by_domain` index (or the
 * whole store for `domain === null`) in primary-key — i.e. insertion — order,
 * stopping after `limit` decoded rows.
 */
function readRange(db: IDBDatabase, domain: string | null, limit: number): Promise<SyncMutation[]> {
  return new Promise<SyncMutation[]>((resolve, reject) => {
    const out: SyncMutation[] = [];
    const tx = db.transaction(OUTBOX_STORE, "readonly");
    const store = tx.objectStore(OUTBOX_STORE);
    const request =
      domain === null
        ? store.openCursor()
        : store.index(BY_DOMAIN_INDEX).openCursor(IDBKeyRange.only(domain));
    request.onsuccess = () => {
      const cursor = request.result;
      if (cursor === null || out.length >= limit) {
        resolve(out);
        return;
      }
      const seq = asNumber(cursor.primaryKey, 1, Number.MAX_SAFE_INTEGER);
      const decoded = seq === undefined ? null : decodeMutation(cursor.value, seq);
      if (decoded !== null) out.push(decoded);
      cursor.continue();
    };
    request.onerror = () => reject(idbError(request.error, "outbox cursor"));
  });
}

/** Put without a key so IDB's atomic autoincrement generator allocates it; resolves the stored mutation with `seq` filled. */
function putAutoKey<P>(db: IDBDatabase, record: SyncMutationDraft<P>): Promise<SyncMutation<P>> {
  return new Promise<SyncMutation<P>>((resolve, reject) => {
    const tx = db.transaction(OUTBOX_STORE, "readwrite");
    const request = tx.objectStore(OUTBOX_STORE).put(record);
    request.onsuccess = () => {
      const seq = asNumber(request.result, 1, Number.MAX_SAFE_INTEGER);
      if (seq === undefined) {
        reject(
          idbError(
            new DOMException("outbox key generator returned a non-integer key", "DataError"),
            "outbox put",
          ),
        );
        return;
      }
      resolve({ ...record, seq });
    };
    tx.onerror = () => reject(idbError(tx.error, "outbox put"));
    tx.onabort = () => reject(idbError(tx.error, "outbox put abort"));
  });
}

/**
 * Own open path (not $lib/workers/idb openIdb): the sync DB pins its own
 * version and its upgrade recreates the store with `autoIncrement` + a
 * `by_domain` index — shapes the shared one-store-per-db helper never creates.
 * The connection is cached per idbQueueStorage() instance; a close or version
 * change elsewhere evicts it so the next call reopens.
 */
export function idbQueueStorage(): QueueStorage {
  let connection: Promise<IDBDatabase> | null = null;
  const db = (): Promise<IDBDatabase> => {
    connection ??= new Promise<IDBDatabase>((resolve, reject) => {
      const req = indexedDB.open(SYNC_DB, SYNC_DB_VERSION);
      req.onupgradeneeded = () => {
        const upgrading = req.result;
        if (upgrading.objectStoreNames.contains(OUTBOX_STORE)) {
          upgrading.deleteObjectStore(OUTBOX_STORE);
        }
        const store = upgrading.createObjectStore(OUTBOX_STORE, { autoIncrement: true });
        store.createIndex(BY_DOMAIN_INDEX, "domain");
      };
      req.onsuccess = () => {
        const opened = req.result;
        opened.onclose = () => {
          connection = null;
        };
        opened.onversionchange = () => {
          opened.close();
          connection = null;
        };
        resolve(opened);
      };
      req.onerror = () => reject(idbError(req.error, "sync open"));
    });
    void connection.catch(() => {
      connection = null;
    });
    return connection;
  };
  return {
    kind: "idb",
    async read(domain, limit) {
      return await readRange(await db(), domain, limit);
    },
    async readAll() {
      return await readRange(await db(), null, Number.POSITIVE_INFINITY);
    },
    async write<P>(record: SyncMutationDraft<P>) {
      return await putAutoKey(await db(), record);
    },
    async erase(seqs) {
      if (seqs.length === 0) return;
      await runTxVoid(await db(), OUTBOX_STORE, "readwrite", (store) => {
        for (const seq of seqs) store.delete(seq);
      });
    },
    async clear(domain) {
      const opened = await db();
      const rows = await readRange(opened, domain, Number.POSITIVE_INFINITY);
      const seqs = rows.map((mutation) => mutation.seq);
      if (seqs.length > 0) {
        await runTxVoid(opened, OUTBOX_STORE, "readwrite", (store) => {
          for (const seq of seqs) store.delete(seq);
        });
      }
      return rows.length;
    },
  };
}

function defaultQueueStorage(): QueueStorage {
  // `in` (not typeof): feature-detects the global without tripping on undeclared access.
  if (browser && "indexedDB" in globalThis) return idbQueueStorage();
  return memoryQueueStorage();
}

/**
 * Durable FIFO mutation queue. `enqueue` persists before it resolves; the
 * storage allocates the insertion key (`seq`) atomically, so keys are unique
 * even across concurrent enqueues or multiple outbox instances (tabs) over one
 * storage. All ops are serialized through one promise chain so enqueue order ==
 * insertion order == FIFO replay order per domain.
 */
export class Outbox {
  readonly storage: QueueStorage;
  #chain: Promise<unknown> = Promise.resolve();

  constructor(storage: QueueStorage) {
    this.storage = storage;
  }

  #serialized<T>(op: () => Promise<T>): Promise<T> {
    const run = this.#chain.then(op, op);
    this.#chain = run.then(
      () => undefined,
      () => undefined,
    );
    return run;
  }

  async enqueue<P>(domain: string, payload: P): Promise<SyncMutation<P>> {
    return await this.#serialized(() =>
      this.storage.write({ id: newMutationId(), domain, payload, queuedAt: Date.now() }),
    );
  }

  /** Next up to `limit` mutations of `domain` in FIFO order, without removing them. */
  async take(domain: string, limit: number): Promise<SyncMutation[]> {
    return await this.#serialized(() => this.storage.read(domain, limit));
  }

  /** Remove successfully synced mutations by id; FIFO order of the rest is preserved. */
  async remove(domain: string, ids: string[]): Promise<void> {
    if (ids.length === 0) return;
    await this.#serialized(async () => {
      const idSet = new Set(ids);
      const queued = await this.storage.read(domain, Number.POSITIVE_INFINITY);
      const seqs = queued.filter((mutation) => idSet.has(mutation.id)).map((m) => m.seq);
      if (seqs.length > 0) await this.storage.erase(seqs);
    });
  }

  /** Durable wipe of every queued mutation of `domain` (e.g. logout); resolves the removed count. */
  async clear(domain: string): Promise<number> {
    return await this.#serialized(() => this.storage.clear(domain));
  }

  async count(domain: string): Promise<number> {
    return (await this.storage.read(domain, Number.POSITIVE_INFINITY)).length;
  }

  async all(): Promise<SyncMutation[]> {
    return await this.#serialized(() => this.storage.readAll());
  }
}

export function createOutbox(storage?: QueueStorage): Outbox {
  return new Outbox(storage ?? defaultQueueStorage());
}
