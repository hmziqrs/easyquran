import { browser } from "$app/env";

import { asNumber, asObject, asString } from "#lib/storage/index.js";
import { idbError } from "#lib/workers/idb-error.js";
import { runTxVoid } from "#lib/workers/idb.js";

import type { SyncMutation } from "./types";

const SYNC_DB = "easyquran-sync";
const OUTBOX_STORE = "outbox";
const DEAD_STORE = "dead";
const BY_DOMAIN_INDEX = "by_domain";
/**
 * v2 (2026-08): autoincrement keys + `by_domain` index (string-key collision fix).
 * v3 (2026-10): adds the `dead` dead-letter store (EC-29). Upgrades NEVER
 * delete the outbox store anymore — queued rows survive every future bump
 * (EC-55); the only recreation case is a pre-v2 shape (no `by_domain` index).
 */
const SYNC_DB_VERSION = 3;

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
  /** True when a resilient wrapper degraded the storage to memory at runtime (EC-25). */
  readonly degraded?: boolean;
  /** Up to `limit` mutations of `domain`, insertion-key ascending. */
  read(domain: string, limit: number): Promise<SyncMutation[]>;
  readAll(): Promise<SyncMutation[]>;
  /** Durable before the promise resolves; resolves the stored record with its allocated seq. */
  write<P>(record: SyncMutationDraft<P>): Promise<SyncMutation<P>>;
  /** Durable removal by insertion keys. */
  erase(seqs: number[]): Promise<void>;
  /** Durable removal of every queued mutation of `domain`; resolves the removed count. */
  clear(domain: string): Promise<number>;
  /** Move a record into the dead-letter store (EC-29); durable before resolving. */
  deadWrite<P>(record: SyncMutationDraft<P>): Promise<void>;
}

export function memoryQueueStorage(): QueueStorage {
  const rows = new Map<number, SyncMutation>();
  const deadRows = new Map<number, SyncMutation>();
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
    async deadWrite<P>(record: SyncMutationDraft<P>) {
      deadRows.set(nextKey, { ...record, seq: nextKey } as SyncMutation);
      nextKey += 1;
    },
  };
}

/**
 * Cursor read over the sync connection: walks the `by_domain` index (or the
 * whole store for `domain === null`) in primary-key — i.e. insertion — order,
 * stopping after `limit` decoded rows. A row that fails `decodeMutation` is
 * corrupt durable state: without a purge it is immortal (skipped by every
 * read, never erased), so it is erased best-effort on sight (EC-26).
 */
function readRange(
  db: IDBDatabase,
  storeName: string,
  domain: string | null,
  limit: number,
): Promise<SyncMutation[]> {
  return new Promise<SyncMutation[]>((resolve, reject) => {
    const out: SyncMutation[] = [];
    const corrupt: number[] = [];
    const tx = db.transaction(storeName, "readonly");
    const store = tx.objectStore(storeName);
    const request =
      domain === null
        ? store.openCursor()
        : store.index(BY_DOMAIN_INDEX).openCursor(IDBKeyRange.only(domain));
    request.onsuccess = () => {
      const cursor = request.result;
      if (cursor === null || out.length >= limit) {
        resolve(out);
        if (corrupt.length > 0) {
          void runTxVoid(db, storeName, "readwrite", (purge) => {
            for (const seq of corrupt) purge.delete(seq);
          }).catch(() => undefined);
        }
        return;
      }
      const seq = asNumber(cursor.primaryKey, 1, Number.MAX_SAFE_INTEGER);
      const decoded = seq === undefined ? null : decodeMutation(cursor.value, seq);
      if (decoded !== null) out.push(decoded);
      else if (seq !== undefined) corrupt.push(seq);
      cursor.continue();
    };
    request.onerror = () => reject(idbError(request.error, "outbox cursor"));
  });
}

/** Put without a key so IDB's atomic autoincrement generator allocates it; resolves the stored mutation with `seq` filled. */
function putAutoKey<P>(
  db: IDBDatabase,
  storeName: string,
  record: SyncMutationDraft<P>,
): Promise<SyncMutation<P>> {
  return new Promise<SyncMutation<P>>((resolve, reject) => {
    const tx = db.transaction(storeName, "readwrite");
    const request = tx.objectStore(storeName).put(record);
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

function hasDomainIndex(store: IDBObjectStore): boolean {
  // Array.from, not .contains: the in-file test double hands back a plain array.
  return Array.from(store.indexNames).includes(BY_DOMAIN_INDEX);
}

/**
 * Idempotent upgrade step (EC-55): an existing outbox store is kept as is —
 * its queued rows must survive every bump — only gaining the `by_domain`
 * index if a pre-v2 database lacks it. Any incompatible pre-v2 rows then fail
 * `decodeMutation` on first read and are purged by the EC-26 path.
 */
function ensureOutboxStore(upgrading: IDBDatabase, upgradeTx: IDBTransaction | null): void {
  if (upgradeTx === null) throw new Error("sync upgrade without a versionchange transaction");
  if (upgrading.objectStoreNames.contains(OUTBOX_STORE)) {
    const store = upgradeTx.objectStore(OUTBOX_STORE);
    if (!hasDomainIndex(store)) store.createIndex(BY_DOMAIN_INDEX, "domain");
    return;
  }
  const store = upgrading.createObjectStore(OUTBOX_STORE, { autoIncrement: true });
  store.createIndex(BY_DOMAIN_INDEX, "domain");
}

/**
 * Own open path (not #lib/workers/idb.js openIdb): the sync DB pins its own
 * version and its upgrade creates the outbox/dead stores with `autoIncrement` +
 * `by_domain` indexes — shapes the shared one-store-per-db helper never
 * creates. The connection is cached per idbQueueStorage() instance; a close or
 * version change elsewhere evicts it so the next call reopens.
 */
export function idbQueueStorage(): QueueStorage {
  let connection: Promise<IDBDatabase> | null = null;
  const db = (): Promise<IDBDatabase> => {
    connection ??= new Promise<IDBDatabase>((resolve, reject) => {
      const req = indexedDB.open(SYNC_DB, SYNC_DB_VERSION);
      req.onupgradeneeded = () => {
        ensureOutboxStore(req.result, req.transaction);
        if (!req.result.objectStoreNames.contains(DEAD_STORE)) {
          const dead = req.result.createObjectStore(DEAD_STORE, { autoIncrement: true });
          dead.createIndex(BY_DOMAIN_INDEX, "domain");
        }
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
      return await readRange(await db(), OUTBOX_STORE, domain, limit);
    },
    async readAll() {
      return await readRange(await db(), OUTBOX_STORE, null, Number.POSITIVE_INFINITY);
    },
    async write<P>(record: SyncMutationDraft<P>) {
      return await putAutoKey(await db(), OUTBOX_STORE, record);
    },
    async deadWrite<P>(record: SyncMutationDraft<P>) {
      await putAutoKey(await db(), DEAD_STORE, record);
    },
    async erase(seqs) {
      if (seqs.length === 0) return;
      await runTxVoid(await db(), OUTBOX_STORE, "readwrite", (store) => {
        for (const seq of seqs) store.delete(seq);
      });
    },
    async clear(domain) {
      const opened = await db();
      // One readwrite tx over the whole by_domain range: a cross-tab enqueue
      // cannot slip between a read tx and a delete tx — IDB serializes any
      // concurrent store write against this transaction, so the wipe is atomic.
      return await new Promise<number>((resolve, reject) => {
        let removed = 0;
        const tx = opened.transaction(OUTBOX_STORE, "readwrite");
        const request = tx
          .objectStore(OUTBOX_STORE)
          .index(BY_DOMAIN_INDEX)
          .openCursor(IDBKeyRange.only(domain));
        request.onsuccess = () => {
          const cursor = request.result;
          if (cursor === null) return; // resolution rides on tx.oncomplete
          cursor.delete();
          removed += 1;
          cursor.continue();
        };
        tx.oncomplete = () => resolve(removed);
        tx.onerror = () => reject(idbError(tx.error, "outbox clear"));
        tx.onabort = () => reject(idbError(tx.error, "outbox clear abort"));
      });
    },
  };
}

/**
 * Runtime degradation wrapper (EC-25): private mode / blocked storage can make
 * `indexedDB.open` reject long after construction. The first failing op flips
 * the wrapper to in-memory storage permanently — the retried op then succeeds
 * instead of rejecting forever — and `degraded` lets the surface warn that
 * changes may not survive a reload.
 */
export function resilientQueueStorage(
  primary: QueueStorage,
  fallback: () => QueueStorage = memoryQueueStorage,
): QueueStorage {
  let memory: QueueStorage | null = null;
  const attempt = async <T>(op: (storage: QueueStorage) => Promise<T>): Promise<T> => {
    if (memory !== null) return await op(memory);
    try {
      return await op(primary);
    } catch {
      memory = fallback();
      return await op(memory);
    }
  };
  return {
    get kind() {
      return (memory ?? primary).kind;
    },
    get degraded() {
      return memory !== null;
    },
    read: (domain, limit) => attempt((storage) => storage.read(domain, limit)),
    readAll: () => attempt((storage) => storage.readAll()),
    write: <P>(record: SyncMutationDraft<P>) => attempt((storage) => storage.write(record)),
    deadWrite: <P>(record: SyncMutationDraft<P>) => attempt((storage) => storage.deadWrite(record)),
    erase: (seqs) => attempt((storage) => storage.erase(seqs)),
    clear: (domain) => attempt((storage) => storage.clear(domain)),
  };
}

function defaultQueueStorage(): QueueStorage {
  // `in` (not typeof): feature-detects the global without tripping on undeclared access.
  if (browser && "indexedDB" in globalThis) return resilientQueueStorage(idbQueueStorage());
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

  /** True once the durable backend degraded to memory at runtime (EC-25). */
  get degraded(): boolean {
    return this.storage.degraded === true;
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

  /**
   * Move one mutation from the live queue to the dead-letter store (EC-29):
   * payload + queuedAt preserved, durable before the original row is erased.
   * Dead letters are never replayed — the surfaced count (`status.deadLettered`)
   * plus `lastError` carry the failure to the UI; an inspection/clear API is a
   * follow-up seam.
   */
  async deadLetter<P>(domain: string, mutation: SyncMutation<P>): Promise<void> {
    await this.#serialized(async () => {
      await this.storage.deadWrite({
        id: mutation.id,
        domain,
        payload: mutation.payload,
        queuedAt: mutation.queuedAt,
      });
      await this.storage.erase([mutation.seq]);
    });
  }

  /**
   * Re-queue every mutation of `from` under `to` — payload and FIFO order
   * preserved, fresh insertion keys — then durably clear `from`. Resolves the
   * moved count. The merge review's explicit attribution path for retained
   * pre-partition entries (EC-48); never called by the engine unprompted.
   */
  async reassign(from: string, to: string): Promise<number> {
    return await this.#serialized(async () => {
      const queued = await this.storage.read(from, Number.POSITIVE_INFINITY);
      for (const mutation of queued) {
        await this.storage.write({
          id: mutation.id,
          domain: to,
          payload: mutation.payload,
          queuedAt: mutation.queuedAt,
        });
      }
      if (queued.length > 0) await this.storage.erase(queued.map((m) => m.seq));
      return queued.length;
    });
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
