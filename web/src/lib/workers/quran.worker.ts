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
import type { ArtifactSpec, Ayah } from "$lib/data/quran-types";
import type { ResolvedManifest } from "../quran/manifest";
import type { WorkerOutbound, WorkerRequest, WorkerStatus } from "../quran/protocol";
import {
  DEFAULT_LIMIT,
  DEFAULT_OFFSET,
  MAX_LIMIT,
  MAX_OFFSET,
  isEligibleQuery,
  normalizeArabic,
  type SearchHit,
  type SearchOpts,
  type SearchResponse,
} from "../quran/search/normalize";

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

/** Normalized search corpus over simple-clean + Uthmani display text by index.
 *  Built lazily on the first search request. */
interface CorpusRow {
  index: number;
  sura: number;
  aya: number;
  norm: string;
}
let corpus: CorpusRow[] | null = null;
let uthmaniByIndex: Map<number, string> | null = null;

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

/** Read an inclusive global-index range of ayahs (juz / page / arbitrary). */
function readRange(from: number, to: number): Ayah[] {
  if (!uthmaniDb) throw new Error("uthmani db not open");
  const rows: [number, number, number, string][] = [];
  uthmaniDb.exec({
    sql: 'SELECT "index", sura, aya, text FROM quran_text WHERE "index" BETWEEN ? AND ? ORDER BY "index"',
    bind: [from, to],
    rowMode: "array",
    resultRows: rows,
  });
  return rows.map(([index, sura, aya, text]) => ({
    key: `${sura}:${aya}`,
    surah: sura,
    ayah: aya,
    globalIndex: index,
    text,
  }));
}

/** Build the normalized search corpus from simple-clean + a Uthmani text map. */
function ensureSearchCorpus(): void {
  if (corpus && uthmaniByIndex) return;
  if (!simpleCleanBytes || !uthmaniDb) throw new Error("search corpus sources unavailable");
  const sc = openReadOnly(simpleCleanBytes);
  const scRows: [number, number, number, string][] = [];
  sc.exec({
    sql: 'SELECT "index", sura, aya, text FROM quran_text ORDER BY "index"',
    rowMode: "array",
    resultRows: scRows,
  });
  corpus = scRows.map((r) => ({
    index: r[0],
    sura: r[1],
    aya: r[2],
    norm: normalizeArabic(r[3]),
  }));
  const uRows: [number, string][] = [];
  uthmaniDb.exec({
    sql: 'SELECT "index", text FROM quran_text ORDER BY "index"',
    rowMode: "array",
    resultRows: uRows,
  });
  uthmaniByIndex = new Map(uRows);
}

/** Substring search over the normalized simple-clean corpus (docs §7). */
function search(query: string, opts: SearchOpts = {}): SearchResponse {
  ensureSearchCorpus();
  const norm = normalizeArabic(query);
  const limit = Math.min(opts.limit ?? DEFAULT_LIMIT, MAX_LIMIT);
  const offset = Math.min(opts.offset ?? DEFAULT_OFFSET, MAX_OFFSET);
  if (!isEligibleQuery(norm)) {
    return { query, total: 0, limit, offset, results: [], source: "worker" };
  }
  const results: SearchHit[] = [];
  let total = 0;
  for (const row of corpus!) {
    if (row.norm.includes(norm)) {
      total++;
      if (total > offset && results.length < limit) {
        results.push({
          key: `${row.sura}:${row.aya}`,
          surah: row.sura,
          ayah: row.aya,
          globalIndex: row.index,
          text: uthmaniByIndex!.get(row.index) ?? "",
        });
      }
    }
  }
  return { query, total, limit, offset, results, source: "worker" };
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
      case "readRange":
        emit({ id, ok: true, result: readRange(msg.from, msg.to) });
        return;
      case "search":
        emit({ id, ok: true, result: search(msg.query, msg.opts) });
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
