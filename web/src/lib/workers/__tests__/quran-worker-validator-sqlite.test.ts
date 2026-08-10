import { beforeAll, describe, expect, it, vi } from "vite-plus/test";
import init, { type Database, type Sqlite3Static } from "@sqlite.org/sqlite-wasm";
import { QuranSourceId } from "$lib/data/quran-types";
import { QURAN_ROW_COUNT } from "$lib/workers/opfs-cache";

// These tests exercise the REAL SQLite-opening validator path:
// assertStagedQuranBytes -> openReadOnly (sqlite3_deserialize) -> wasm query runner
// -> runOne(count) + runQuery(coordinates) -> assertStagedQuranContent.
// The sibling quran-worker-validator.test.ts only covers the pure post-query
// assertion logic; here a staged DB is built with the same wasm runtime the
// worker uses, serialized, and handed to the production validator function.
//
// The wasm sqlite runtime initializes successfully under vitest + happy-dom.
//
// quran.worker.ts statically imports resolveSourceProfile, so the mock must be
// hoisted before the worker first loads. Default to the real resolver; the
// wrong-profile test flips the override to a short row count.
const profileOverride = vi.hoisted(() => ({ value: null as { canonicalRowCount: number } | null }));
vi.mock("$lib/quran/view/source-profiles", async (importOriginal) => {
  const actual = await importOriginal<typeof import("$lib/quran/view/source-profiles")>();
  return {
    ...actual,
    resolveSourceProfile: (id: QuranSourceId) =>
      profileOverride.value ?? actual.resolveSourceProfile(id),
  };
});

interface BuildRow {
  index: number;
  sura: number;
  aya: number;
}

let builder: Sqlite3Static | null = null;

function serializeAfter(setup: (db: Database) => void): Uint8Array {
  const s = builder!;
  const db = new s.oo1.DB();
  setup(db);
  const capi = s.capi;
  const wasm = s.wasm;
  const pSize = wasm.alloc(8);
  const dataPtr = capi.sqlite3_serialize(db, "main", pSize, 0);
  const heap = wasm.heap8u();
  const size = Number(new DataView(heap.buffer, heap.byteOffset + pSize, 8).getBigInt64(0, true));
  const bytes = new Uint8Array(heap.buffer, heap.byteOffset + dataPtr, size).slice();
  db.close();
  wasm.dealloc(pSize);
  return bytes;
}

function serializeQuranDb(rows: readonly BuildRow[]): Uint8Array {
  return serializeAfter((db) => {
    db.exec('CREATE TABLE quran_text ("index" INTEGER, sura INTEGER, aya INTEGER, text TEXT)');
    const stmt = db.prepare('INSERT INTO quran_text VALUES (?,?,?,?)');
    try {
      for (const r of rows) {
        stmt.bind([r.index, r.sura, r.aya, ""]);
        stmt.step();
        stmt.reset();
      }
    } finally {
      stmt.finalize();
    }
  });
}

function serializeWrongSchemaDb(): Uint8Array {
  return serializeAfter((db) => {
    db.exec("CREATE TABLE meta (key TEXT, value TEXT)");
  });
}

function contiguousRows(count: number, startIndex = 1): BuildRow[] {
  const rows: BuildRow[] = [];
  for (let i = 0; i < count; i++) rows.push({ index: startIndex + i, sura: 1, aya: 1 });
  return rows;
}

type WorkerModule = typeof import("$lib/workers/quran.worker");
let worker: WorkerModule;
let validBytes: Uint8Array;

beforeAll(async () => {
  builder = await init();
  worker = await import("$lib/workers/quran.worker");
  await worker.__initValidatorRuntime();
  validBytes = serializeQuranDb(contiguousRows(QURAN_ROW_COUNT));
});

describe("assertStagedQuranBytes real SQLite-opening path", () => {
  it("accepts a valid staged Tanzil DB for an Arabic source", () => {
    expect(() => worker.assertStagedQuranBytes(validBytes, QuranSourceId.TanzilUthmani)).not.toThrow();
  });

  it("accepts a valid staged DB for a translation source (profile check skipped)", () => {
    expect(() => worker.assertStagedQuranBytes(validBytes, "en.sahih")).not.toThrow();
  });

  it("rejects a tampered DB with the wrong row count", () => {
    const bytes = serializeQuranDb(contiguousRows(QURAN_ROW_COUNT - 1));
    expect(() => worker.assertStagedQuranBytes(bytes, QuranSourceId.TanzilUthmani)).toThrow(
      /row count/,
    );
  });

  it("rejects a tampered DB with non-contiguous globalIndex coordinates", () => {
    const rows = contiguousRows(QURAN_ROW_COUNT - 1);
    rows.push({ index: QURAN_ROW_COUNT + 1, sura: 1, aya: 1 });
    const bytes = serializeQuranDb(rows);
    expect(() => worker.assertStagedQuranBytes(bytes, QuranSourceId.TanzilUthmani)).toThrow(
      /non-contiguous/,
    );
  });

  it("rejects a DB with the wrong schema (missing quran_text table)", () => {
    const bytes = serializeWrongSchemaDb();
    expect(() => worker.assertStagedQuranBytes(bytes, QuranSourceId.TanzilUthmani)).toThrow(
      /no such table/i,
    );
  });

  it("rejects an Arabic source whose profile row count disagrees", () => {
    profileOverride.value = { canonicalRowCount: QURAN_ROW_COUNT - 1 };
    try {
      expect(() =>
        worker.assertStagedQuranBytes(validBytes, QuranSourceId.TanzilUthmani),
      ).toThrow(/profile row count mismatch/);
    } finally {
      profileOverride.value = null;
    }
  });
});
