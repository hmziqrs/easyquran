import { promises as fs } from "node:fs";
import { sumBy } from "es-toolkit";
import path from "node:path";
import { version as appBuildId } from "$app/environment";

const KINDS = ["surah", "page", "juz"] as const;
export type DiskCacheKind = (typeof KINDS)[number];

const HTML_EXT = ".html";
const TMP_SUFFIX = ".tmp";
const DEFAULT_TTL_MS = 7 * 24 * 60 * 60 * 1000;
const DEFAULT_BUDGET_BYTES = 256 * 1024 * 1024;
const ORPHAN_TMP_AGE_MS = 60 * 60 * 1000;

export interface QuranDiskCacheOptions {
  directory: string;
  ttlMs: number;
  budgetBytes: number;
  now?: () => number;
}

export interface QuranDiskCacheStats {
  hits: number;
  misses: number;
  writes: number;
  evictions: number;
  errors: number;
  entries: number;
  bytes: number;
}

function envInt(name: string, fallback: number): number {
  const raw = process.env[name];
  if (raw === undefined || raw === "") return fallback;
  const n = Number(raw);
  return Number.isFinite(n) && n > 0 ? Math.floor(n) : fallback;
}

const TTL_MS = envInt("QURAN_SSR_CACHE_TTL_MS", DEFAULT_TTL_MS);
const BUDGET_BYTES = envInt("QURAN_SSR_CACHE_BUDGET_BYTES", DEFAULT_BUDGET_BYTES);
const CACHE_DIR =
  process.env.QURAN_SSR_CACHE_DIR ?? path.resolve(process.cwd(), ".cache/quran-ssr");

function sanitizeComponent(value: string): string {
  const cleaned = value.replace(/[^a-z0-9.-]+/gi, "_");
  if (cleaned === "" || cleaned === "." || cleaned === "..") return "_";
  return cleaned;
}

export function diskCacheKey(sourceId: string, kind: DiskCacheKind, a: number, b?: number): string {
  // This namespaces disposable HTML by the web build so cached markup never points at assets
  // removed by a later deploy. Quran source identity remains `sourceId`; this is not data versioning.
  const parts = [`build-${appBuildId}`, sourceId, kind, String(a)];
  if (b !== undefined) parts.push(String(b));
  return parts.map((part) => sanitizeComponent(part)).join("__");
}

export class QuranDiskCache {
  readonly #directory: string;
  readonly #ttlMs: number;
  readonly #budgetBytes: number;
  readonly #now: () => number;
  #hits = 0;
  #misses = 0;
  #writes = 0;
  #evictions = 0;
  #errors = 0;
  #tmpSequence = 0;

  constructor(options: QuranDiskCacheOptions) {
    this.#directory = options.directory;
    this.#ttlMs = options.ttlMs;
    this.#budgetBytes = options.budgetBytes;
    this.#now = options.now ?? Date.now;
  }

  #entryPath(key: string): string {
    return path.join(this.#directory, `${sanitizeComponent(key)}${HTML_EXT}`);
  }

  async get(key: string): Promise<string | null> {
    const file = this.#entryPath(key);
    const stat = await fs.stat(file).catch(() => null);
    if (!stat) {
      this.#misses += 1;
      return null;
    }
    const now = this.#now();
    if (stat.mtimeMs + this.#ttlMs < now) {
      const removed = await fs.rm(file, { force: true }).then(
        () => true,
        () => {
          this.#errors += 1;
          return false;
        },
      );
      if (removed) this.#evictions += 1;
      this.#misses += 1;
      return null;
    }
    const html = await fs.readFile(file, "utf8").catch(() => null);
    if (html === null) {
      this.#errors += 1;
      this.#misses += 1;
      return null;
    }
    const touched = new Date(now);
    await fs.utimes(file, touched, touched).catch(() => {
      this.#errors += 1;
    });
    this.#hits += 1;
    return html;
  }

  async set(key: string, html: string): Promise<void> {
    await fs.mkdir(this.#directory, { recursive: true });
    const final = this.#entryPath(key);
    this.#tmpSequence += 1;
    const tmp = path.join(
      this.#directory,
      `.${sanitizeComponent(key)}.${process.pid}.${this.#tmpSequence}${TMP_SUFFIX}`,
    );
    try {
      await fs.writeFile(tmp, html, "utf8");
      await fs.rename(tmp, final);
      const written = new Date(this.#now());
      await fs.utimes(final, written, written);
      this.#writes += 1;
      await this.#prune();
    } catch (error) {
      this.#errors += 1;
      await fs.rm(tmp, { force: true }).catch(() => {});
      throw error;
    }
  }

  async stats(): Promise<QuranDiskCacheStats> {
    const entries = await this.#entries();
    return {
      hits: this.#hits,
      misses: this.#misses,
      writes: this.#writes,
      evictions: this.#evictions,
      errors: this.#errors,
      entries: entries.length,
      bytes: sumBy(entries, (entry) => entry.size),
    };
  }

  async #entries(): Promise<{ name: string; mtime: number; size: number }[]> {
    const names = await fs.readdir(this.#directory).catch(() => [] as string[]);
    const entries: { name: string; mtime: number; size: number }[] = [];
    for (const name of names) {
      if (!name.endsWith(HTML_EXT)) continue;
      const stat = await fs.stat(path.join(this.#directory, name)).catch(() => null);
      if (stat) entries.push({ name, mtime: stat.mtimeMs, size: stat.size });
    }
    return entries;
  }

  async #prune(): Promise<void> {
    const names = await fs.readdir(this.#directory).catch(() => [] as string[]);
    const now = this.#now();
    const survivors: { name: string; mtime: number; size: number }[] = [];
    let total = 0;
    for (const name of names) {
      const file = path.join(this.#directory, name);
      if (name.endsWith(TMP_SUFFIX)) {
        const stat = await fs.stat(file).catch(() => null);
        if (stat && now - stat.mtimeMs > ORPHAN_TMP_AGE_MS) {
          await fs.rm(file, { force: true }).catch(() => {
            this.#errors += 1;
          });
        }
        continue;
      }
      if (!name.endsWith(HTML_EXT)) continue;
      const stat = await fs.stat(file).catch(() => null);
      if (!stat) continue;
      if (stat.mtimeMs + this.#ttlMs < now) {
        const removed = await fs.rm(file, { force: true }).then(
          () => true,
          () => {
            this.#errors += 1;
            return false;
          },
        );
        if (removed) this.#evictions += 1;
        continue;
      }
      survivors.push({ name, mtime: stat.mtimeMs, size: stat.size });
      total += stat.size;
    }
    if (total <= this.#budgetBytes) return;
    survivors.sort((a, b) => a.mtime - b.mtime);
    for (const entry of survivors) {
      if (total <= this.#budgetBytes) break;
      const removed = await fs.rm(path.join(this.#directory, entry.name), { force: true }).then(
        () => true,
        () => {
          this.#errors += 1;
          return false;
        },
      );
      if (removed) {
        this.#evictions += 1;
        total -= entry.size;
      }
    }
  }
}

const defaultCache = new QuranDiskCache({
  directory: CACHE_DIR,
  ttlMs: TTL_MS,
  budgetBytes: BUDGET_BYTES,
});

export const getCachedHtml = (key: string): Promise<string | null> => defaultCache.get(key);
export const setCachedHtml = (key: string, html: string): Promise<void> =>
  defaultCache.set(key, html);
export const getQuranDiskCacheStats = (): Promise<QuranDiskCacheStats> => defaultCache.stats();
