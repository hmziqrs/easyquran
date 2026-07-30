/* ════════════════════════════════════════════════════════════════════════
   quran.worker.ts — the offline Quran engine (sqlite-wasm, deserialize).

   Opens the two cached Arabic databases as READ-ONLY in-memory DBs via
   `sqlite3_deserialize` (no "opfs" VFS, no SharedArrayBuffer, no COOP/COEP —
   the host is not crossOriginIsolated and must stay that way to keep FCM's
   cross-origin importScripts working). Bytes are durable in OPFS (opfs-cache)
   and re-verified against sizeBytes + sha256 on every load.

   Speaks the protocol in quran/protocol.ts over postMessage. Client-only: this
   file is a module Worker instantiated from worker-client.ts via
   `new Worker(new URL(...), { type: "module" })`; it must never be imported by
   prerender/server code.
   ════════════════════════════════════════════════════════════════════════ */

import init, { type Sqlite3Static, type Database } from "@sqlite.org/sqlite-wasm";
import { ensureArtifact } from "./opfs-cache";
import type { ArtifactSpec } from "$lib/data/quran-types";
import type { ResolvedManifest } from "../quran/manifest";
import type { WorkerOutbound, WorkerRequest, WorkerStatus } from "../quran/protocol";

/** Minimal worker scope (avoids DOM/webworker `self` lib clashes). */
interface WorkerCtx {
  postMessage(msg: WorkerOutbound): void;
  onmessage: ((e: MessageEvent<WorkerRequest>) => void) | null;
}
const ctx = self as unknown as WorkerCtx;

let sqlite3: Sqlite3Static | null = null;
/** The Uthmani read DB (display + SSG-parity reads). */
let uthmaniDb: Database | null = null;
/** Kept for the Phase 4 search corpus (simple-clean). Held as bytes so the
 *  in-memory DB can be opened on demand without a second persistent handle. */
let simpleCleanBytes: Uint8Array | null = null;
/** Retain the Uthmani bytes so their backing buffer is never GC'd while the
 *  deserialized DB references its heap copy. */
let uthmaniBytes: Uint8Array | null = null;
let ready = false;

function emit(msg: WorkerOutbound): void {
  ctx.postMessage(msg);
}
function status(s: WorkerStatus, detail?: string): void {
  emit({ type: "status", status: s, detail });
}

/** Open a read-only in-memory DB from raw SQLite bytes via deserialize. */
function openReadOnly(bytes: Uint8Array): Database {
  const db = new sqlite3!.oo1.DB(); // :memory:
  const READONLY = sqlite3!.capi.SQLITE_DESERIALIZE_READONLY;
  const FLAGS = sqlite3!.capi.SQLITE_DESERIALIZE_FREEONCLOSE | READONLY;
  // sqlite must own the buffer on its heap to free it on close:
  const ptr = sqlite3!.wasm.allocFromTypedArray(bytes);
  const rc = sqlite3!.capi.sqlite3_deserialize(
    db,
    "main",
    ptr,
    bytes.byteLength,
    bytes.byteLength,
    FLAGS,
  );
  if (rc !== sqlite3!.capi.SQLITE_OK) {
    sqlite3!.wasm.dealloc(ptr);
    throw new Error(`sqlite3_deserialize failed: rc=${rc}`);
  }
  return db;
}

/** Initialize the engine: load wasm, fetch+verify+cache both DBs, deserialize. */
async function initialize(manifest: ResolvedManifest): Promise<void> {
  status("init");
  // The bundler-friendly default resolves sqlite3.wasm via the package exports
  // map; Vite emits it as a hashed asset.
  sqlite3 = await init();

  const uthmaniSpec = manifest.scripts.find((s) => s.id === "uthmani") as ArtifactSpec | undefined;
  const simpleSpec = manifest.scripts.find((s) => s.id === "simple-clean") as ArtifactSpec | undefined;
  if (!uthmaniSpec || !simpleSpec) throw new Error("manifest missing a script spec");

  status("downloading", "uthmani");
  const u = await ensureArtifact(uthmaniSpec, manifest.contentVersion);
  uthmaniBytes = u.bytes;
  uthmaniDb = openReadOnly(u.bytes);

  status("downloading", "simple-clean");
  const s = await ensureArtifact(simpleSpec, manifest.contentVersion);
  simpleCleanBytes = s.bytes;

  ready = true;
  console.info(
    `[quran] offline engine ready (uthmani: ${u.store}, simple-clean: ${s.store}); ` +
      `surah 1 has ${readSurah(1).length} verses`,
  );
  status("ready");
}

/** Read one surah's verbatim Uthmani verses, in ayah order. */
function readSurah(num: number): string[] {
  if (!uthmaniDb) throw new Error("uthmani db not open");
  const stmt = uthmaniDb.prepare("SELECT text FROM quran_text WHERE sura = ? ORDER BY aya");
  stmt.bind([num]);
  const out: string[] = [];
  while (stmt.step()) out.push(stmt.get(0) as string);
  stmt.finalize();
  return out;
}

ctx.onmessage = async (e: MessageEvent<WorkerRequest>): Promise<void> => {
  const msg = e.data;
  const id = msg.id;
  const typeStr = (msg as { type: string }).type;
  try {
    if (msg.type === "init") {
      await initialize(msg.manifest);
      emit({ id, ok: true, result: null });
      emit({ type: "ready" });
      return;
    }
    if (!ready) {
      emit({ id, ok: false, error: "engine not ready" });
      return;
    }
    switch (msg.type) {
      case "readSurah":
        emit({ id, ok: true, result: readSurah(msg.num) });
        return;
      case "ping":
        emit({ id, ok: true, result: "pong" });
        return;
      default:
        emit({ id, ok: false, error: `unknown request ${typeStr}` });
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    if (typeStr === "init") {
      status("error", message);
      emit({ type: "fatal", error: message });
    }
    emit({ id, ok: false, error: message });
  }
};
