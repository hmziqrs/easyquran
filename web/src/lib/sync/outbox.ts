import { browser } from "$app/environment";
import { asNumber, asObject, asString } from "$lib/storage";
import { idbError } from "$lib/workers/idb-error";
import { idbPut, openIdb, runTxVoid } from "$lib/workers/idb";
import type { SyncMutation } from "./types";

const SYNC_DB = "easyquran-sync";
const OUTBOX_STORE = "outbox";
/** Key width: `${domain}:${zero-padded seq}` sorts lexicographically in seq order. */
const SEQ_KEY_WIDTH = 12;

function mutationKey(domain: string, seq: number): string {
  return `${domain}:${String(seq).padStart(SEQ_KEY_WIDTH, "0")}`;
}

function newMutationId(): string {
  // `in` (not typeof): randomUUID is missing on insecure origins (LAN http dev).
  if ("randomUUID" in crypto) return crypto.randomUUID();
  return `sync-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 12)}`;
}

/** Boundary decoder: a record read back from IndexedDB is unproven until parsed. */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is an arbitrary IDB record of uncertain provenance; this decoder IS the parser that validates it field by field.
function decodeMutation(raw: unknown): SyncMutation | null {
  const obj = asObject(raw);
  if (!obj) return null;
  const id = asString(obj.id);
  const domain = asString(obj.domain);
  const seq = asNumber(obj.seq, 1, Number.MAX_SAFE_INTEGER);
  const queuedAt = asNumber(obj.queuedAt, 0, Number.POSITIVE_INFINITY);
  if (id === undefined || domain === undefined || seq === undefined || queuedAt === undefined) {
    return null;
  }
  return { id, domain, seq, payload: obj.payload, queuedAt };
}

function compareMutations(a: SyncMutation, b: SyncMutation): number {
  if (a.domain !== b.domain) return a.domain < b.domain ? -1 : 1;
  return a.seq - b.seq;
}

/**
 * Pluggable durable storage behind the outbox. Two implementations: IndexedDB
 * (production, via the shared $lib/workers/idb connection cache) and an
 * in-memory map (tests, and any environment without IndexedDB).
 */
export interface QueueStorage {
  readonly kind: "idb" | "memory";
  /** Up to `limit` mutations of `domain`, seq-ascending. */
  read(domain: string, limit: number): Promise<SyncMutation[]>;
  readAll(): Promise<SyncMutation[]>;
  /** Durable before the returned promise resolves. */
  write(mutation: SyncMutation): Promise<void>;
  erase(domain: string, seqs: number[]): Promise<void>;
}

export function memoryQueueStorage(): QueueStorage {
  const rows = new Map<string, SyncMutation>();
  return {
    kind: "memory",
    async read(domain, limit) {
      const matched: SyncMutation[] = [];
      for (const mutation of rows.values()) {
        if (mutation.domain === domain) matched.push(mutation);
      }
      matched.sort(compareMutations);
      return matched.slice(0, limit);
    },
    async readAll() {
      return [...rows.values()].sort(compareMutations);
    },
    async write(mutation) {
      rows.set(mutationKey(mutation.domain, mutation.seq), mutation);
    },
    async erase(domain, seqs) {
      for (const seq of seqs) rows.delete(mutationKey(domain, seq));
    },
  };
}

/**
 * Cursor read over the shared openIdb connection: collects mutations whose key
 * starts with `prefix` in key (== seq) order, stopping after `limit`.
 * idbScan scans one prefix at a time with no limit, so this local variant uses
 * the same connection cache rather than forking a second idb module.
 */
function readRange(db: IDBDatabase, prefix: string, limit: number): Promise<SyncMutation[]> {
  return new Promise<SyncMutation[]>((resolve, reject) => {
    const out: SyncMutation[] = [];
    const tx = db.transaction(OUTBOX_STORE, "readonly");
    const request = tx.objectStore(OUTBOX_STORE).openCursor();
    request.onsuccess = () => {
      const cursor = request.result;
      if (!cursor) {
        resolve(out);
        return;
      }
      const key = cursor.key;
      // eslint-disable-next-line anti-slop/no-runtime-typeof -- IDB cursor keys are polymorphic (IDBValidKey); this store writes only `${domain}:${padded seq}` string keys, so this filters our records at the read boundary.
      if (typeof key === "string" && key.startsWith(prefix)) {
        const decoded = decodeMutation(cursor.value);
        if (decoded) out.push(decoded);
        if (out.length >= limit) {
          resolve(out);
          return;
        }
      }
      cursor.continue();
    };
    request.onerror = () => reject(idbError(request.error, "outbox cursor"));
  });
}

export function idbQueueStorage(): QueueStorage {
  let connection: Promise<IDBDatabase> | null = null;
  const db = (): Promise<IDBDatabase> => {
    connection ??= openIdb(SYNC_DB, OUTBOX_STORE);
    return connection;
  };
  return {
    kind: "idb",
    async read(domain, limit) {
      return await readRange(await db(), `${domain}:`, limit);
    },
    async readAll() {
      return await readRange(await db(), "", Number.POSITIVE_INFINITY);
    },
    async write(mutation) {
      await idbPut(
        await db(),
        OUTBOX_STORE,
        mutation,
        mutationKey(mutation.domain, mutation.seq),
      );
    },
    async erase(domain, seqs) {
      await runTxVoid(await db(), OUTBOX_STORE, "readwrite", (store) => {
        for (const seq of seqs) store.delete(mutationKey(domain, seq));
      });
    },
  };
}

function defaultQueueStorage(): QueueStorage {
  // `in` (not typeof): feature-detects the global without tripping on undeclared access.
  if (browser && "indexedDB" in globalThis) return idbQueueStorage();
  return memoryQueueStorage();
}

/**
 * Durable FIFO mutation queue. `enqueue` persists before it resolves and
 * allocates a per-domain monotonic seq; all writes are serialized through one
 * promise chain so seq order == insertion order even under concurrent enqueues.
 *
 * Contract: domain names must not contain ":" (it is the key-path separator),
 * and one Outbox instance per storage per tab (cross-tab seq allocation is not
 * coordinated; IDB `put` would silently overwrite a colliding key).
 */
export class Outbox {
  readonly storage: QueueStorage;
  #lastSeq = new Map<string, number>();
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

  async #nextSeq(domain: string): Promise<number> {
    const cached = this.#lastSeq.get(domain);
    if (cached !== undefined) return cached + 1;
    let max = 0;
    for (const mutation of await this.storage.readAll()) {
      if (mutation.domain === domain && mutation.seq > max) max = mutation.seq;
    }
    this.#lastSeq.set(domain, max);
    return max + 1;
  }

  async enqueue<P>(domain: string, payload: P): Promise<SyncMutation<P>> {
    if (domain.includes(":")) throw new Error(`sync domain "${domain}" must not contain ":"`);
    return await this.#serialized(async () => {
      const seq = await this.#nextSeq(domain);
      this.#lastSeq.set(domain, seq);
      const mutation: SyncMutation<P> = {
        id: newMutationId(),
        domain,
        seq,
        payload,
        queuedAt: Date.now(),
      };
      await this.storage.write(mutation);
      return mutation;
    });
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
      if (seqs.length > 0) await this.storage.erase(domain, seqs);
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
