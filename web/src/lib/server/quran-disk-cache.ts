import { promises as fs } from "node:fs";
import path from "node:path";

const KINDS = ["surah", "page", "juz"] as const;
export type DiskCacheKind = (typeof KINDS)[number];

const HTML_EXT = ".html";
const TMP_SUFFIX = ".tmp";
const DEFAULT_TTL_MS = 7 * 24 * 60 * 60 * 1000;
const DEFAULT_BUDGET_BYTES = 256 * 1024 * 1024;
const ORPHAN_TMP_AGE_MS = 60 * 60 * 1000;

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

export function diskCacheKey(
  sourceId: string,
  kind: DiskCacheKind,
  a: number,
  b?: number,
): string {
  const parts = [sourceId, kind, String(a)];
  if (b !== undefined) parts.push(String(b));
  return parts.map(sanitizeComponent).join("__");
}

export function htmlCacheKey(pathname: string): string {
  return sanitizeComponent(`p${pathname}`);
}

function entryPath(key: string): string {
  return path.join(CACHE_DIR, `${key}${HTML_EXT}`);
}

export async function getCachedHtml(key: string): Promise<string | null> {
  const file = entryPath(key);
  const stat = await fs.stat(file).catch(() => null);
  if (!stat) return null;
  if (stat.mtimeMs + TTL_MS < Date.now()) {
    await fs.rm(file, { force: true }).catch(() => {});
    return null;
  }
  const html = await fs.readFile(file, "utf8").catch(() => null);
  if (html === null) return null;
  await fs.utimes(file, new Date(), new Date()).catch(() => {});
  return html;
}

export async function setCachedHtml(key: string, html: string): Promise<void> {
  await fs.mkdir(CACHE_DIR, { recursive: true });
  const final = entryPath(key);
  const tmp = path.join(
    CACHE_DIR,
    `.${sanitizeComponent(key)}.${process.pid}.${Date.now()}${TMP_SUFFIX}`,
  );
  await fs.writeFile(tmp, html, "utf8");
  await fs.rename(tmp, final);
  await prune();
}

async function prune(): Promise<void> {
  let names: string[];
  try {
    names = await fs.readdir(CACHE_DIR);
  } catch {
    return;
  }
  const now = Date.now();
  const survivors: { name: string; mtime: number; size: number }[] = [];
  let total = 0;
  for (const name of names) {
    const file = path.join(CACHE_DIR, name);
    if (name.endsWith(TMP_SUFFIX)) {
      const stat = await fs.stat(file).catch(() => null);
      if (stat && now - stat.mtimeMs > ORPHAN_TMP_AGE_MS) {
        await fs.rm(file, { force: true }).catch(() => {});
      }
      continue;
    }
    if (!name.endsWith(HTML_EXT)) continue;
    const stat = await fs.stat(file).catch(() => null);
    if (!stat) continue;
    if (stat.mtimeMs + TTL_MS < now) {
      await fs.rm(file, { force: true }).catch(() => {});
      continue;
    }
    survivors.push({ name, mtime: stat.mtimeMs, size: stat.size });
    total += stat.size;
  }
  if (total <= BUDGET_BYTES) return;
  survivors.sort((a, b) => a.mtime - b.mtime);
  for (const entry of survivors) {
    if (total <= BUDGET_BYTES) break;
    await fs.rm(path.join(CACHE_DIR, entry.name), { force: true }).catch(() => {});
    total -= entry.size;
  }
}
