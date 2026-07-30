/* ════════════════════════════════════════════════════════════════════════
   worker-client.ts — main-thread proxy for the quran.worker.

   Owns the Worker lifecycle, request/response correlation, and lifecycle-event
   forwarding. The only place `new Worker(...)` is constructed, so the worker
   bundle stays out of the server/prerender graph. `start()` is called from a
   browser-only onMount with a resolved manifest (manifest.ts runs here, on the
   main thread — never inside the worker).
   ════════════════════════════════════════════════════════════════════════ */

import type { ResolvedManifest } from "./manifest";
import type { WorkerOutbound, WorkerRequest, WorkerStatus } from "./protocol";

let worker: Worker | null = null;
let seq = 0;
let isReady = false;
let startPromise: Promise<void> | null = null;
let readyPromise: Promise<void> | null = null;
let readyResolve: (() => void)[] = [];
let readyReject: ((e: Error) => void)[] = [];

const pending = new Map<number, { resolve: (v: unknown) => void; reject: (e: Error) => void }>();
const statusListeners = new Set<(s: WorkerStatus, detail?: string) => void>();

function handle(msg: WorkerOutbound): void {
  if ("id" in msg) {
    const p = pending.get(msg.id);
    if (!p) return;
    pending.delete(msg.id);
    msg.ok ? p.resolve(msg.result) : p.reject(new Error(msg.error));
    return;
  }
  if (msg.type === "status") {
    for (const cb of statusListeners) cb(msg.status, msg.detail);
  } else if (msg.type === "fatal") {
    const err = new Error(msg.error);
    readyReject.forEach((r) => r(err));
    readyReject = [];
  }
}

function request<T>(build: (id: number) => WorkerRequest): Promise<T> {
  if (!worker) return Promise.reject(new Error("quran worker not started"));
  const id = ++seq;
  return new Promise<T>((resolve, reject) => {
    pending.set(id, { resolve: resolve as (v: unknown) => void, reject });
    worker!.postMessage(build(id));
  });
}

export const quranWorker = {
  get ready(): boolean {
    return isReady;
  },
  /** Resolves once the engine has both DBs deserialized; rejects on fatal error. */
  get whenReady(): Promise<void> {
    if (isReady) return Promise.resolve();
    return readyPromise ?? Promise.reject(new Error("quran worker not started"));
  },
  /** Subscribe to lifecycle status (init/downloading/ready/error). Returns an unsub. */
  onStatus(cb: (s: WorkerStatus, detail?: string) => void): () => void {
    statusListeners.add(cb);
    return () => statusListeners.delete(cb);
  },

  /** Start the worker with a resolved manifest. Idempotent; safe to call once. */
  start(manifest: ResolvedManifest): Promise<void> {
    if (startPromise) return startPromise;
    startPromise = (async () => {
      worker = new Worker(new URL("../workers/quran.worker.ts", import.meta.url), {
        type: "module",
        name: "quran-db",
      });
      worker.addEventListener("message", (e: MessageEvent<WorkerOutbound>) => handle(e.data));
      readyPromise = new Promise<void>((res, rej) => {
        readyResolve.push(res);
        readyReject.push(rej);
      });
      try {
        await request<null>((id) => ({ id, type: "init", manifest }));
        isReady = true;
        readyResolve.forEach((r) => r());
        readyResolve = [];
      } catch (err) {
        const e = err instanceof Error ? err : new Error(String(err));
        readyReject.forEach((r) => r(e));
        readyReject = [];
        throw e;
      }
    })();
    return startPromise;
  },

  /** Read one surah's verbatim Uthmani verses from the local DB. */
  readSurah(num: number): Promise<string[]> {
    return request<string[]>((id) => ({ id, type: "readSurah", num }));
  },
};
