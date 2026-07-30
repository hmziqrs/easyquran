/**
 * lib.ts — shared logic for the tanzil-translation data pipeline.
 *
 * Used by fetch.ts (mirror + index), catalog.ts (trimmed index), verify.ts
 * (integrity), sql-to-sqlite.ts (build artifacts), and upload-sqlite.ts (push
 * the artifacts + catalog to Cloudflare R2).
 */

import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

// ── paths ────────────────────────────────────────────────────────────────
const __dirname = path.dirname(fileURLToPath(import.meta.url));
/** .../db/quran/tanzil/translations */
export const ROOT = path.resolve(__dirname, "..");
/** repo root (translations/ ↑ tanzil ↑ quran ↑ db ↑ root) */
export const REPO_ROOT = path.resolve(ROOT, "..", "..", "..", "..");
export const SQLDIR = path.join(ROOT, "sql");
/** derived SQLite artifacts — one `<id>.sqlite` per translation (see sql-to-sqlite.ts) */
export const SQLITEDIR = path.join(ROOT, "sqlite");
export const INDEX = path.join(ROOT, "index.json");
export const INDEX_MIN = path.join(ROOT, "index.min.json");
export const PAGE_CACHE = path.join(ROOT, ".trans_page.html");
export const DEFAULT_WEB_TARGET = path.resolve(REPO_ROOT, "web/src/lib/data/translations.json");

// ── constants ────────────────────────────────────────────────────────────
export const BASE = "https://tanzil.net/trans";
export const UA = "easyquran-data/1.0 (+https://easyquran.app; tanzil mirror)";
/** language codes (tanzil-id prefix) whose script is right-to-left */
export const RTL_CODES = new Set(["ar", "fa", "ur", "ps", "sd", "ug", "ku", "dv"]);

// ── types ────────────────────────────────────────────────────────────────
export interface PageItem {
  id: string;
  language: string;
  name: string;
  translator: string;
}

export interface Translation {
  id: string;
  language: string;
  languageCode: string;
  direction: "rtl" | "ltr";
  name: string;
  nameNative?: string;
  translator: string;
  lastUpdate?: string;
  ayaCount?: number;
  file: { sql: string; sizeBytes?: number; sha256?: string };
  urls: { tanzil: string; download: string; browse: string; changelog: string };
  languageMismatch?: { page: string; sql: string };
}

export interface IndexFile {
  source: string;
  sourceUrl: string;
  format: string;
  license: string;
  note: string;
  count: number;
  translations: Translation[];
}

/** minimal, web-facing entry — what index.min.json holds */
export interface MinTranslation {
  id: string;
  language: string;
  languageCode: string;
  direction: "rtl" | "ltr";
  name: string;
  translator: string;
  /**
   * Artifact path, relative to the catalog's own location. Points at the
   * per-translation SQLite build (never the raw MySQL dump — those stay in the
   * repo and are not published). Placeholder until the derived delivery
   * databases in docs/quran-web-delivery.md land.
   */
  file: string;
}

// ── small utilities ───────────────────────────────────────────────────────
export const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

export function log(msg: string): void {
  console.log(msg);
}

/** Run an async worker over items with bounded concurrency, preserving order. */
export async function pPool<T, R>(
  items: T[],
  worker: (item: T, index: number) => Promise<R>,
  concurrency = 6,
  onResult?: (result: R, item: T, index: number) => void,
): Promise<R[]> {
  const results: R[] = new Array(items.length);
  let cursor = 0;
  const run = async (): Promise<void> => {
    while (cursor < items.length) {
      const i = cursor++;
      results[i] = await worker(items[i]!, i);
      onResult?.(results[i]!, items[i]!, i);
    }
  };
  await Promise.all(Array.from({ length: Math.min(concurrency, items.length) }, run));
  return results;
}

const NAMED_ENTITIES: Record<string, string> = {
  amp: "&",
  quot: '"',
  apos: "'",
  lt: "<",
  gt: ">",
  nbsp: " ",
};

export function decodeEntities(s: string): string {
  return s.replace(/&(#x[0-9a-fA-F]+|#\d+|[a-zA-Z]+);/g, (match, ent: string) => {
    if (ent.startsWith("#")) {
      const cp = ent[1] === "x" || ent[1] === "X" ? parseInt(ent.slice(2), 16) : parseInt(ent.slice(1), 10);
      return Number.isNaN(cp) ? match : String.fromCodePoint(cp);
    }
    return ent in NAMED_ENTITIES ? NAMED_ENTITIES[ent]! : match;
  });
}

function stripTags(s: string): string {
  return decodeEntities(s.replace(/<[^>]+>/g, "")).trim();
}

export async function readJson<T>(file: string): Promise<T> {
  return JSON.parse(await readFile(file, "utf-8")) as T;
}

export async function writeJson(file: string, data: unknown): Promise<void> {
  await writeFile(file, JSON.stringify(data, null, 2) + "\n", "utf-8");
}

function stripUndef<T extends object>(o: T): T {
  const rec = o as Record<string, unknown>;
  for (const k of Object.keys(rec)) if (rec[k] === undefined) delete rec[k];
  return o;
}

// ── discover the translation list from the /trans/ page ────────────────────
export async function fetchPage(): Promise<string> {
  if (existsSync(PAGE_CACHE)) return readFileSync(PAGE_CACHE, "utf-8");
  const res = await fetch(`${BASE}/`, { headers: { "user-agent": UA } });
  if (!res.ok) throw new Error(`fetching /trans/ failed: ${res.status}`);
  const html = await res.text();
  await writeFile(PAGE_CACHE, html, "utf-8");
  return html;
}

export function parsePage(html: string): PageItem[] {
  const rows = html.match(/<tr[^>]*>[\s\S]*?<\/tr>/g) ?? [];
  const out: PageItem[] = [];
  const seen = new Set<string>();
  for (const row of rows) {
    const cells = row.split(/<td[^>]*>/).slice(1); // drop the chunk before the first <td>
    if (cells.length < 4) continue;
    const language = stripTags(cells[0]!);
    const name = stripTags(cells[1]!);
    const translator = stripTags(cells[2]!);
    const m = cells[3]!.match(/href="\/trans\/([a-z]{2,4}\.[a-z0-9-]+)"/);
    if (!m) continue;
    const id = m[1]!;
    if (seen.has(id)) continue;
    seen.add(id);
    out.push({
      id,
      language,
      name: name.replace(/\s*[*†]+\s*$/, "").trim(),
      translator: translator.replace(/\s*[*†]+\s*/g, "").trim(),
    });
  }
  return out;
}

// ── parse a downloaded SQL dump ────────────────────────────────────────────
const HEADER_RE = /^#\s*(Name|Translator|Language|ID|Last Update|Source)\s*:\s*(.+)$/;
const ROW_RE = /\(\d+,\s*\d+,\s*\d+,\s*'/g;

export interface ParsedSql {
  header: Record<string, string>;
  ayaCount: number;
  sizeBytes: number;
  sha256: string;
}

/** Decode + scan a dump. Throws on invalid UTF-8 (TextDecoder fatal). */
export function parseSql(buf: Buffer): ParsedSql {
  const text = new TextDecoder("utf-8", { fatal: true }).decode(buf);
  const header: Record<string, string> = {};
  for (const line of text.split(/\r?\n/)) {
    const m = line.match(HEADER_RE);
    if (m) header[m[1]!] = m[2]!.trim();
  }
  const ayaCount = (text.match(ROW_RE) ?? []).length;
  return {
    header,
    ayaCount,
    sizeBytes: buf.length,
    sha256: createHash("sha256").update(buf).digest("hex"),
  };
}

// ── build the full + minimal indexes ───────────────────────────────────────
export function buildIndex(items: PageItem[]): IndexFile {
  const translations: Translation[] = items.map((it) => {
    const code = it.id.split(".")[0]!;
    const sqlPath = path.join(SQLDIR, `${it.id}.sql`);
    const parsed = existsSync(sqlPath) ? parseSql(readFileSync(sqlPath)) : null;
    const hdr = parsed?.header ?? {};
    const sqlLang = hdr.Language ?? "";
    const mismatch = !!(sqlLang && it.language && sqlLang.toLowerCase() !== it.language.toLowerCase());

    return stripUndef<Translation>({
      id: it.id,
      language: it.language,
      languageCode: code,
      direction: RTL_CODES.has(code) ? "rtl" : "ltr",
      name: hdr.Name ?? it.name,
      nameNative: it.name !== hdr.Name ? it.name : undefined,
      translator: hdr.Translator ?? it.translator,
      lastUpdate: hdr["Last Update"],
      ayaCount: parsed?.ayaCount,
      file: stripUndef({
        sql: `sql/${it.id}.sql`,
        sizeBytes: parsed?.sizeBytes,
        sha256: parsed?.sha256,
      }),
      urls: {
        tanzil: `${BASE}/${it.id}`,
        download: `${BASE}/${it.id}?type=sql`,
        browse: `https://tanzil.net/#trans/${it.id}/1:1`,
        changelog: `${BASE}/log/${it.id}`,
      },
      ...(mismatch ? { languageMismatch: { page: it.language, sql: sqlLang } } : {}),
    });
  });

  return {
    source: "Tanzil.net",
    sourceUrl: `${BASE}/`,
    format: "sql (MySQL phpMyAdmin dump)",
    license:
      "Tanzil Terms of Use — non-commercial use; attribution + backlink to tanzil.net required when redistributing more than three translations.",
    note: "name = SQL-header name (authoritative); nameNative = display name from the /trans/ page (often the original script).",
    count: translations.length,
    translations,
  };
}

export function minFromIndex(index: IndexFile): MinTranslation[] {
  return index.translations.map((t) => ({
    id: t.id,
    language: t.language,
    languageCode: t.languageCode,
    direction: t.direction,
    name: t.name,
    translator: t.translator,
    file: `sqlite/${t.id}.sqlite`,
  }));
}

/** Where the web app picks up the catalog. Override with WEB_TRANSLATIONS_PATH. */
function webTarget(): string {
  const env = process.env.WEB_TRANSLATIONS_PATH;
  if (env) return path.isAbsolute(env) ? env : path.resolve(ROOT, env);
  return DEFAULT_WEB_TARGET;
}

/** Write index.min.json and (unless web:false) copy it into the web app. */
export async function writeCatalog(index: IndexFile, opts: { web?: boolean } = {}): Promise<void> {
  await writeJson(INDEX_MIN, minFromIndex(index));
  if (opts.web === false) return;
  const target = webTarget();
  await mkdir(path.dirname(target), { recursive: true });
  await writeFile(target, await readFile(INDEX_MIN, "utf-8"), "utf-8");
  log(`  catalog → ${path.relative(REPO_ROOT, target)}`);
}

// ── download one SQL dump (retries + politeness) ───────────────────────────
export async function downloadSql(id: string, force: boolean): Promise<"ok" | "cached" | "fail"> {
  await mkdir(SQLDIR, { recursive: true });
  const dest = path.join(SQLDIR, `${id}.sql`);
  if (existsSync(dest) && !force && statSync(dest).size > 1000) return "cached";

  const url = `${BASE}/${id}?type=sql`;
  for (let attempt = 1; attempt <= 4; attempt++) {
    try {
      const res = await fetch(url, { headers: { "user-agent": UA }, signal: AbortSignal.timeout(90_000) });
      const buf = Buffer.from(await res.arrayBuffer());
      const head = buf.subarray(0, 400).toString("latin1").toLowerCase();
      if (!res.ok || buf.length < 1000 || head.includes("file not found") || head.includes("<!doctype html")) {
        throw new Error(`bad response ${res.status} (${buf.length} bytes)`);
      }
      await writeFile(dest, buf);
      await sleep(200); // be polite to tanzil
      return "ok";
    } catch {
      if (attempt === 4) return "fail";
      await sleep(1500 * attempt);
    }
  }
  return "fail";
}

/** Every downloaded sql file (basename, e.g. "en.sahih.sql"). */
export function listSqlFiles(): string[] {
  return readdirSync(SQLDIR)
    .filter((f) => f.endsWith(".sql"))
    .sort();
}

/** Every built sqlite file (basename, e.g. "en.sahih.sqlite"). */
export function listSqliteFiles(): string[] {
  return readdirSync(SQLITEDIR)
    .filter((f) => f.endsWith(".sqlite"))
    .sort();
}
