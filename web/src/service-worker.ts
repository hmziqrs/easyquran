/// <reference types="@sveltejs/kit" />
/// <reference lib="webworker" />

// Kit 3 removed `$service-worker`: `version` now comes from `$app/env`, the
// Vite-generated file list (`build`) is `$app/manifest`'s `immutable`, the
// static-dir file list (`files`) is its `assets`, and `base`-prefixed paths
// are built with `resolve()` from `$app/paths`.
import { version } from "$app/env";
import { assets, immutable } from "$app/manifest";
import { resolve } from "$app/paths";

import { PUSH_TITLE, PUSH_BODY, PUSH_TAG } from "./lib/firebase/push-privacy";
import { normalizeDataKey } from "./lib/offline/keys";
import {
  SKIP_WAITING,
  APP_READY,
  UPDATE_TAKEOVER,
  PURGE_USER_CACHES,
  PURGE_ACK,
  STORAGE_STATS,
  STORAGE_STATS_ACK,
  VERSION_QUERY,
  VERSION_RESULT,
  SW_BROADCAST_CHANNEL,
  type ClientToSwMessage,
  type StorageLayerStats,
  type SwToClientMessage,
} from "./lib/offline/messages";
import { openIdb, idbGet, idbPut, idbDelete, idbScan } from "./lib/workers/idb";

// SAFETY: this file is the service-worker entry (compiled with the webworker
// lib reference above), so `self` is the worker global at runtime; the DOM lib
// also declares `self`, forcing a widen to cross the incompatible declarations.
const selfAny = self as unknown;
// SAFETY: carried over from `selfAny` above — same global object, re-branded
// with the worker scope the rest of this module uses.
const sw = selfAny as ServiceWorkerGlobalScope;

const APP_CACHE = `eq-app-${version}`;
export const PAGES_CACHE = "eq-pages-v1";
export const DATA_CACHE = "eq-data-v1";

const META_DB = "easyquran-sw-meta";
const META_STORE = "meta";

// I7 grace prune: pruning old eq-app-* caches is deferred behind a persisted
// deadline instead of firing the moment the last client acks, so a routine
// SKIP_WAITING can never yank precached chunks out from under tabs still
// running the previous version's lazy-loaded assets. The deadline lives in
// the IDB meta store (not a timer — the SW is idle-killed and loses timers),
// and is re-checked on every wake event.
export const PRUNE_GRACE_MS = 10 * 60 * 1000;
export const PRUNE_DEADLINE_KEY = "pruneDeadline";

// SAFETY: kit 3 `$app/paths` has no `base` export; resolve()'s pathname branch
// (non-leading-slash argument) prefixes the base path without a route lookup.
// `quran-meta/…` is a build artifact outside the generated `Path` union, so
// resolve is re-branded to the plain pathname signature.
const resolveAny = resolve as unknown;
// SAFETY: carried over from `resolveAny` above — same function value, only
// re-branded to the string-pathname signature the precache lists need.
const resolvePath = resolveAny as (pathname: string) => string;
// Prerendered app shells. Each one is served offline ONLY at its own URL: a
// SvelteKit 3 page hydrates the route baked into its HTML (node ids + params
// in the inline start script), not the one in the address bar, so answering an
// unrelated URL with a shell would render the wrong page under that URL. An
// uncached URL gets offlineFallback() instead, which links back to these.
// The landing (`/`, the manifest start_url) and the surah index are critical:
// install fails without them. The /ar twins are best-effort.
const SHELL_HOME = resolvePath("");
const SHELL_SURAH = resolvePath("surah");
const CRITICAL_SHELLS: readonly string[] = [SHELL_HOME, SHELL_SURAH];
const OPTIONAL_SHELLS: readonly string[] = [resolvePath("ar"), resolvePath("ar/surah")];
const SHELLS = new Set<string>([...CRITICAL_SHELLS, ...OPTIONAL_SHELLS]);
// Kit 3's runtime public-env module. Every page's inline boot script runs
// `import("./_app/env.js").then(() => import(start) ...)`, so without a cached
// copy an offline page never hydrates. Served `no-cache` by server.ts (a
// runtime value, not a build artifact) -> network-first with cache fallback.
const ENV_MODULE = resolvePath("_app/env.js");
const QURAN_DATA = resolvePath("quran-meta/quran-data.json");
const NAV_TIMEOUT_MS = 3500;
export const PAGES_MAX = 300;
export const DATA_MAX = 400;
export const DATA_BUDGET_BYTES = 32 * 1024 * 1024;
const MAINTENANCE_CONCURRENCY = 6;
// Install-time fetch fan-out. Unbounded Promise.all over ~900 files starved
// mobile connections and one slow/failed file rejected the whole install.
const PRECACHE_CONCURRENCY = 6;
const PRECACHE_ATTEMPTS = 2;

// Kit 3 removed `serviceWorker.files` — the precache filter kit 2 carried in
// svelte.config.js lives here now: dotfiles (.DS_Store et al.) are refused by
// this server, while _headers/_redirects/robots.txt/og.png and the quran-meta/
// catalogue are not client precache material (quran-data.json is re-added
// deliberately below). `qurancom/` is the dev-only quran.com reference mirror
// (13 MB, read only by the 404-in-production /design/qurancom specimen) and
// `icons/oauth/` holds provider-upload artwork the app never renders.
const PRECACHE_EXCLUDED_NAMES = new Set(["_headers", "_redirects", "robots.txt", "og.png"]);
const PRECACHE_EXCLUDED_PREFIXES: readonly string[] = ["quran-meta/", "qurancom/", "icons/oauth/"];

function isPrecacheExcluded(path: string): boolean {
  const rel = path.replace(/^\/+/, "");
  if (PRECACHE_EXCLUDED_NAMES.has(rel)) return true;
  if (PRECACHE_EXCLUDED_PREFIXES.some((prefix) => rel.startsWith(prefix))) return true;
  return rel.split("/").some((segment) => segment.startsWith("."));
}

// kit 2's `$service-worker.build` + `.files` become `manifest.immutable`
// (Vite output) + `manifest.assets` (static dir); both are `{ path }` objects.
// Kit 3 emits those paths WITHOUT a leading slash ("favicon.png") while the
// fetch handler matches slash-prefixed `url.pathname`, so every entry runs
// through resolvePath()'s pathname branch (it prefixes "/" plus the base
// under a non-empty base) — otherwise IMMUTABLE.has(url.pathname) never
// matches and static assets fall through to SWR on every request.
const PRECACHE_FILES: string[] = [...immutable, ...assets]
  .filter((entry) => !isPrecacheExcluded(entry.path))
  .map((entry) => resolvePath(entry.path.replace(/^\/+/, "")));

const IMMUTABLE = new Set(PRECACHE_FILES);

// Single home for the cache-key normalizer is #lib/offline/keys (with its own
// test); re-exported so the route-level service-worker tests keep pinning the
// SW's key contract through this module.
export { normalizeDataKey };

export function isPending(res: Response): boolean {
  return res.headers.get("x-eq-translation-pending") === "1";
}

export function isCacheable(res: Response): boolean {
  const cc = res.headers.get("cache-control");
  if (cc && (/no-store/i.test(cc) || /private/i.test(cc))) return false;
  if (isPending(res)) return false;
  return true;
}

function db(): Promise<IDBDatabase> {
  return openIdb(META_DB, META_STORE);
}

async function metaGet<T>(key: string): Promise<T | undefined> {
  try {
    return await idbGet<T>(await db(), META_STORE, key);
  } catch {
    return undefined;
  }
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- values are heterogeneous IndexedDB records (ack version strings, maintenance cursor, recency maps); the IDB value slot is structurally untyped by design
async function metaSet(key: string, value: unknown): Promise<void> {
  try {
    await idbPut(await db(), META_STORE, value, key);
  } catch {}
}

async function metaDel(key: string): Promise<void> {
  try {
    await idbDelete(await db(), META_STORE, key);
  } catch {}
}

async function metaScan<T>(prefix: string): Promise<Record<string, T>> {
  try {
    return await idbScan<T>(await db(), META_STORE, prefix);
  } catch {
    return {};
  }
}

interface DataMetaEntry {
  key: string;
  lastUsed: number;
  sizeBytes: number;
}

const DATA_META_PREFIX = "data:";

function dataMetaId(key: string): string {
  return `${DATA_META_PREFIX}${key}`;
}

async function measureResponseSize(res: Response): Promise<number> {
  // Measure the decompressed body, never trust Content-Length: prerendered
  // __data.json is served precompressed, so the header reflects the encoded
  // (or absent) size while the cache stores the full decoded body.
  try {
    return (await res.clone().arrayBuffer()).byteLength;
  } catch {
    return 0;
  }
}

let dataMetaLock: Promise<void> = Promise.resolve();
function serializeDataMeta(fn: () => Promise<void>): Promise<void> {
  const next = dataMetaLock.then(fn, fn);
  dataMetaLock = next.catch(() => undefined);
  return next;
}

let recencyLock: Promise<void> = Promise.resolve();
function serializeRecency(fn: () => Promise<void>): Promise<void> {
  const next = recencyLock.then(fn, fn);
  recencyLock = next.catch(() => undefined);
  return next;
}

async function rawDataMetaDel(key: string): Promise<void> {
  try {
    await idbDelete(await db(), META_STORE, dataMetaId(key));
  } catch {}
}

async function rawDataMetaSet(entry: DataMetaEntry): Promise<void> {
  try {
    await idbPut(await db(), META_STORE, entry, dataMetaId(entry.key));
  } catch {}
}

async function rawDataMetaGet(key: string): Promise<DataMetaEntry | undefined> {
  try {
    return await idbGet<DataMetaEntry>(await db(), META_STORE, dataMetaId(key));
  } catch {
    return undefined;
  }
}

async function rawDataMetaScan(): Promise<Map<string, DataMetaEntry>> {
  const obj = await metaScan<DataMetaEntry>(DATA_META_PREFIX);
  return new Map(Object.entries(obj));
}

export async function recordDataEntry(key: string, res: Response): Promise<void> {
  const sizeBytes = await measureResponseSize(res);
  const lastUsed = Date.now();
  return serializeDataMeta(() => rawDataMetaSet({ key, lastUsed, sizeBytes }));
}

export function touchDataMeta(key: string): Promise<void> {
  return serializeDataMeta(async () => {
    const existing = await rawDataMetaGet(key);
    if (!existing) return;
    await rawDataMetaSet({ key, lastUsed: Date.now(), sizeBytes: existing.sizeBytes });
  });
}

export function deleteDataMeta(key: string): Promise<void> {
  return serializeDataMeta(() => rawDataMetaDel(key));
}

export function purgeAllDataMeta(): Promise<void> {
  return serializeDataMeta(async () => {
    const entries = await rawDataMetaScan();
    await Promise.all([...entries.keys()].map((key) => rawDataMetaDel(key)));
  });
}

export async function purgeUserCachesHandler(port?: MessagePort): Promise<void> {
  try {
    await caches.delete(PAGES_CACHE);
    await caches.delete(DATA_CACHE);
    await purgeAllDataMeta();
    await serializeRecency(() => metaSet("recency", {}));
  } catch {}
  if (port) {
    try {
      port.postMessage({ type: PURGE_ACK } satisfies SwToClientMessage);
    } catch {}
  }
}

const EMPTY_LAYER_STATS: StorageLayerStats = Object.freeze({ entries: 0, bytes: 0 });

async function measurePagesCache(): Promise<StorageLayerStats> {
  const pages = await caches.open(PAGES_CACHE);
  const reqs = [...(await pages.keys())];
  const sizes = reqs.map(() => 0);
  await pool(reqs, MAINTENANCE_CONCURRENCY, async (req, index) => {
    const res = await pages.match(req).catch(() => undefined);
    if (!res) return;
    sizes[index] = await measureResponseSize(res);
  });
  let bytes = 0;
  for (const size of sizes) bytes += size;
  return { entries: reqs.length, bytes };
}

async function measureDataCache(): Promise<StorageLayerStats> {
  const meta = await rawDataMetaScan();
  const data = await caches.open(DATA_CACHE);
  const cacheKeys = new Set<string>();
  for (const req of await data.keys()) cacheKeys.add(normalizeDataKey(req.url));
  let bytes = 0;
  let entries = 0;
  for (const [key, entry] of meta) {
    if (!cacheKeys.has(key)) continue;
    bytes += entry.sizeBytes;
    entries += 1;
  }
  return { entries, bytes };
}

export async function computeStorageStats(): Promise<{
  pages: StorageLayerStats;
  data: StorageLayerStats;
}> {
  const [pages, data] = await Promise.all([
    measurePagesCache().catch(() => EMPTY_LAYER_STATS),
    measureDataCache().catch(() => EMPTY_LAYER_STATS),
  ]);
  return { pages, data };
}

export async function storageStatsHandler(port?: MessagePort): Promise<void> {
  const stats = await computeStorageStats();
  if (!port) return;
  try {
    port.postMessage({
      type: STORAGE_STATS_ACK,
      pages: stats.pages,
      data: stats.data,
    } satisfies SwToClientMessage);
  } catch {}
}

export async function enforceDataBoundsInner(): Promise<void> {
  const data = await caches.open(DATA_CACHE);
  const meta = await rawDataMetaScan();
  const cacheReqs = await data.keys();
  const cacheKeys = new Set<string>();
  for (const req of cacheReqs) cacheKeys.add(normalizeDataKey(req.url));

  for (const key of meta.keys()) {
    if (!cacheKeys.has(key)) {
      await rawDataMetaDel(key);
      meta.delete(key);
    }
  }
  const missingMeta: string[] = [];
  for (const key of cacheKeys) if (!meta.has(key)) missingMeta.push(key);

  const entries = [...meta.values()].sort((a, b) => a.lastUsed - b.lastUsed);
  let totalBytes = 0;
  for (const e of entries) totalBytes += e.sizeBytes;
  let count = cacheKeys.size;

  for (const entry of entries) {
    if (count <= DATA_MAX && totalBytes <= DATA_BUDGET_BYTES) break;
    const deleted = await data.delete(new Request(entry.key)).catch(() => false);
    await rawDataMetaDel(entry.key);
    if (deleted) count--;
    totalBytes -= entry.sizeBytes;
  }
  for (const key of missingMeta) {
    if (count <= DATA_MAX) break;
    const deleted = await data.delete(new Request(key)).catch(() => false);
    if (deleted) count--;
  }
}

export function enforceDataBounds(): Promise<void> {
  return serializeDataMeta(enforceDataBoundsInner);
}

export async function readDataMeta(key: string): Promise<DataMetaEntry | undefined> {
  return rawDataMetaGet(key);
}

export async function scanDataMeta(): Promise<Map<string, DataMetaEntry>> {
  return rawDataMetaScan();
}

let broadcastChannel: BroadcastChannel | null = null;
function getBroadcast(): BroadcastChannel | null {
  if (broadcastChannel) return broadcastChannel;
  try {
    broadcastChannel = new BroadcastChannel(SW_BROADCAST_CHANNEL);
  } catch {
    broadcastChannel = null;
  }
  return broadcastChannel;
}

async function pool<T>(
  items: T[],
  limit: number,
  worker: (item: T, index: number) => Promise<void>,
): Promise<void> {
  let next = 0;
  async function run(): Promise<void> {
    while (true) {
      const i = next;
      next++;
      if (i >= items.length) return;
      await worker(items[i]!, i);
    }
  }
  const runners: Promise<void>[] = [];
  for (let k = 0; k < Math.min(limit, items.length); k++) runners.push(run());
  await Promise.all(runners);
}

sw.addEventListener("install", (event) => {
  event.waitUntil(precache());
});

// Two tiers. CRITICAL (install rejects without it): the landing + surah-index
// shells, every same-origin module/stylesheet their HTML references to boot
// (modulepreload links, the inline start script's imports, env.js), and
// quran-data.json. BEST-EFFORT: the rest of the manifest, fetched with bounded
// concurrency; a failed file is skipped and filled later by the runtime
// cache-first / SWR handlers. A failed install keeps what it wrote: a retry of
// the same version reuses the hashed /_app/immutable/ entries, and any other
// version's prune removes the cache if this version never activates.
async function precache(): Promise<void> {
  const cache = await caches.open(APP_CACHE);
  const critical = new Set<string>([QURAN_DATA]);
  for (const shell of CRITICAL_SHELLS) {
    const html = await precacheShell(cache, shell);
    for (const ref of bootReferences(html, shell)) critical.add(ref);
  }
  await pool([...critical], PRECACHE_CONCURRENCY, (url) => precacheOne(cache, url));

  const rest = [...PRECACHE_FILES, ...OPTIONAL_SHELLS].filter((url) => !critical.has(url));
  let failed = 0;
  await pool(rest, PRECACHE_CONCURRENCY, async (url) => {
    try {
      await precacheOne(cache, url);
    } catch {
      failed++;
    }
  });
  if (failed > 0) console.warn(`[sw] precache: ${failed} optional file(s) deferred to runtime`);
}

async function precacheFetch(url: string): Promise<Response> {
  let lastError: Error | null = null;
  for (let attempt = 0; attempt < PRECACHE_ATTEMPTS; attempt++) {
    try {
      const res = await fetch(url, { cache: "no-cache", credentials: "same-origin" });
      if (res.ok && res.type !== "opaqueredirect") return res;
      lastError = new Error(`precache ${url} failed: ${res.status}`);
    } catch (err) {
      lastError = err instanceof Error ? err : new Error(`precache ${url} failed`);
    }
  }
  throw lastError ?? new Error(`precache ${url} failed`);
}

const IMMUTABLE_DIR = resolvePath("_app/immutable/");

async function precacheOne(cache: Cache, url: string): Promise<void> {
  // Hashed build output never changes under one URL: keep a copy an earlier
  // install attempt of this version wrote, or reuse a previous version's copy,
  // instead of re-downloading every chunk on every update.
  if (url.startsWith(IMMUTABLE_DIR)) {
    if (await cache.match(url)) return;
    const prior = await matchOtherAppCaches(url);
    if (prior) {
      await cache.put(url, prior);
      return;
    }
  }
  const res = await precacheFetch(url);
  await cache.put(url, await storable(res));
}

// Returns the shell's HTML so its boot references can be promoted to critical.
async function precacheShell(cache: Cache, url: string): Promise<string> {
  const res = await precacheFetch(url);
  let html = "";
  try {
    html = await res.clone().text();
  } catch {}
  await cache.put(url, await storable(res));
  return html;
}

const BOOT_REF_PATTERN = /(?:href|src)="([^"]+)"|import\(\s*"([^"]+)"\s*\)/g;

// Same-origin build files (and env.js) a shell's HTML loads to hydrate.
function bootReferences(html: string, shellUrl: string): string[] {
  const base = new URL(shellUrl, sw.location.origin);
  const out: string[] = [];
  for (const match of html.matchAll(BOOT_REF_PATTERN)) {
    const raw = match[1] ?? match[2];
    if (!raw) continue;
    let resolved: URL;
    try {
      resolved = new URL(raw, base);
    } catch {
      continue;
    }
    if (resolved.origin !== sw.location.origin) continue;
    const path = resolved.pathname;
    if (path === ENV_MODULE || IMMUTABLE.has(path)) out.push(path);
  }
  return out;
}

// A cached entry must be usable for ANY later request to its URL:
// - `redirected` responses cannot answer a navigation (the browser turns them
//   into a network error), so the body is re-wrapped in a fresh Response;
// - `Vary` (server.ts / hooks.server.ts append `Vary: Accept` to HTML that has
//   a .md twin, `/` included) makes the Cache API compare the stored request's
//   Accept with the lookup's, so a precache entry fetched with `*/*` never
//   matched a `text/html` navigation. Precache entries are single-representation
//   by construction, so the header is dropped.
// Rebuilt bodies are already decoded by fetch(), so encoding/length go too.
async function storable(res: Response): Promise<Response> {
  if (!res.redirected && !res.headers.has("vary")) return res;
  const headers = new Headers(res.headers);
  headers.delete("vary");
  headers.delete("content-encoding");
  headers.delete("content-length");
  return new Response(await res.blob(), {
    status: res.status,
    statusText: res.statusText,
    headers,
  });
}

// Older eq-app-* caches survive the I7 grace window precisely so tabs still on
// the previous build can load its lazy chunks; this is the lookup that serves
// them (cacheFirstApp) and the install-time reuse source (precacheOne).
async function matchOtherAppCaches(url: string): Promise<Response | undefined> {
  const names = (await caches.keys()).filter((k) => k.startsWith("eq-app-") && k !== APP_CACHE);
  for (const name of names) {
    const hit = await (await caches.open(name)).match(url, { ignoreVary: true });
    if (hit) return hit;
  }
  return undefined;
}

sw.addEventListener("activate", (event) => {
  event.waitUntil(activate());
});

async function activate(): Promise<void> {
  await sw.clients.claim();
  const keys = await caches.keys();
  const priorExisted = keys.some((k) => k.startsWith("eq-app-") && k !== APP_CACHE);
  const prev = await metaGet<string>("installedVersion");
  if (prev && prev !== version) {
    await setCursor(null);
  }
  await metaSet("installedVersion", version);
  await purgeLegacyReaderPaths();
  void enforceDataBounds();
  // Wake event: a deadline left over by a previous worker (or an earlier
  // all-ack of this version) may already have expired by activation time.
  await maybePruneAfterGrace();
  if (priorExisted) announceTakeover();
}

function isLegacyReaderPath(url: string): boolean {
  const pathname = new URL(url, sw.location.origin).pathname;
  // Scheme A: every `/en/**`, `/app/**` and `/ar/app/**` spelling is dead (the
  // hooks.server 308 map owns them). PAGES/DATA caches and their metadata
  // self-clean those entries on activate; unprefixed and /ar shapes are live.
  return (
    pathname === "/en" ||
    pathname.startsWith("/en/") ||
    pathname === "/app" ||
    pathname.startsWith("/app/") ||
    pathname === "/ar/app" ||
    pathname.startsWith("/ar/app/")
  );
}

async function purgeLegacyReaderPaths(): Promise<void> {
  for (const cacheName of [PAGES_CACHE, DATA_CACHE]) {
    const cache = await caches.open(cacheName);
    const requests = await cache.keys();
    await Promise.all(
      requests
        .filter((request) => isLegacyReaderPath(request.url))
        .map((request) => cache.delete(request)),
    );
  }
  const dataMeta = await rawDataMetaScan();
  await Promise.all(
    [...dataMeta.keys()].filter((url) => isLegacyReaderPath(url)).map((key) => rawDataMetaDel(key)),
  );
  await serializeRecency(async () => {
    const recency = (await metaGet<Record<string, number>>("recency")) || {};
    let changed = false;
    for (const key of Object.keys(recency)) {
      if (isLegacyReaderPath(key)) {
        delete recency[key];
        changed = true;
      }
    }
    if (changed) await metaSet("recency", recency);
  });
}

function announceTakeover(): void {
  const message: SwToClientMessage = { type: UPDATE_TAKEOVER, version };
  void sw.clients
    .matchAll({ includeUncontrolled: true })
    .then((clients) => {
      for (const client of clients) client.postMessage(message);
    })
    .catch(() => {});
  const channel = getBroadcast();
  if (channel) {
    try {
      channel.postMessage(message);
    } catch {}
  }
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- m is event.data from postMessage, an opaque runtime boundary; this guard is the parse
function isClientToSw(m: unknown): m is ClientToSwMessage {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- discriminating an opaque postMessage payload; no parse seam exists at a bare MessageEvent
  if (!m || typeof m !== "object") return false;
  // SAFETY: m is narrowed to a non-null object by the guard above; the cast
  // only widens the property read for the string check below.
  const type = (m as { type?: unknown }).type;
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- narrowing the untyped payload field to string; no schema boundary exists in the SW message path
  return typeof type === "string";
}

function isClient(source: Client | ServiceWorker | MessagePort | null): source is Client {
  return !!source && "id" in source;
}

// VERSION_QUERY RPC: reply with this worker's own build version on the
// MessageChannel port the caller transferred (same transport as
// PURGE_USER_CACHES / STORAGE_STATS). Valid for any worker state — active or
// waiting — because the answer is the version of the script actually running.
export function versionQueryHandler(port?: MessagePort): void {
  if (!port) return;
  try {
    port.postMessage({ type: VERSION_RESULT, version } satisfies SwToClientMessage);
  } catch {}
}

sw.addEventListener("message", (event) => {
  if (!isClientToSw(event.data)) return;
  const data = event.data;
  switch (data.type) {
    case SKIP_WAITING:
      void sw.skipWaiting();
      void maybePruneAfterGrace();
      return;
    case APP_READY:
      void onAppReady(isClient(event.source) ? event.source : null);
      return;
    case VERSION_QUERY:
      versionQueryHandler(event.ports[0]);
      void maybePruneAfterGrace();
      return;
    case PURGE_USER_CACHES:
      void purgeUserCachesHandler(event.ports[0]);
      void maybePruneAfterGrace();
      return;
    case STORAGE_STATS:
      void storageStatsHandler(event.ports[0]);
      void maybePruneAfterGrace();
      return;
  }
});

async function onAppReady(source: Client | null): Promise<void> {
  if (source) {
    await metaSet(`ack:${source.id}`, version);
  }
  await maybeFinalizeHandoff();
  // Wake event: on this same APP_READY the all-ack path may have just reset
  // the deadline (then this no-ops), or an inherited deadline may have
  // expired while the ack set is already complete for this version.
  await maybePruneAfterGrace();
  void runMaintenance();
}

// All live clients have acked the CURRENT version: (re)arm the grace deadline
// instead of pruning immediately (I7). Every new all-ack overwrites the
// deadline, which is what makes a late-arriving straggler reset the window.
export async function maybeFinalizeHandoff(): Promise<void> {
  const live = await sw.clients.matchAll({ includeUncontrolled: false });
  const liveIds = new Set(live.map((c) => c.id));
  const acks = await metaScan<string>("ack:");
  await Promise.all(
    Object.keys(acks)
      .filter((id) => !liveIds.has(id))
      .map((id) => metaDel(`ack:${id}`)),
  );
  if (live.length === 0) return;
  const allAcked = live.every((c) => acks[c.id] === version);
  if (!allAcked) return;
  await metaSet(PRUNE_DEADLINE_KEY, Date.now() + PRUNE_GRACE_MS);
}

// Event-driven grace-prune check (I7): runs on every SW wake (activate, any
// handled message). Prunes only when the persisted deadline has passed AND
// every live client STILL acks the CURRENT SW's own version — the exact
// maybeFinalizeHandoff predicate, version-bound so an inherited stale
// deadline from a superseded worker can never prune while this version's
// acks are incomplete. Zero clients = vacuously safe (nobody can be holding
// old-chunk references), so the predicate is allowed to hold on an empty
// live set.
export async function maybePruneAfterGrace(): Promise<void> {
  const deadline = await metaGet<number>(PRUNE_DEADLINE_KEY);
  if (deadline === undefined) return;
  if (Date.now() < deadline) return;
  // A newer worker installing or waiting has its own eq-app-<next> cache, which
  // pruneOldAppCaches() would see as "old" and delete mid-install, leaving it to
  // activate with an empty shell. Keep the deadline; the next wake re-checks.
  const reg = sw.registration;
  if (reg && (reg.installing || reg.waiting)) return;
  const live = await sw.clients.matchAll({ includeUncontrolled: false });
  const acks = await metaScan<string>("ack:");
  const allAcked = live.every((c) => acks[c.id] === version);
  if (!allAcked) return;
  await pruneOldAppCaches();
  await metaDel(PRUNE_DEADLINE_KEY);
}

async function pruneOldAppCaches(): Promise<void> {
  const keys = await caches.keys();
  await Promise.all(
    keys.filter((k) => k.startsWith("eq-app-") && k !== APP_CACHE).map((k) => caches.delete(k)),
  );
}

sw.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET") return;
  const url = new URL(req.url);

  if (url.pathname === "/quran" || url.pathname.startsWith("/quran/")) return;
  if (url.pathname === "/_quran" || url.pathname.startsWith("/_quran/")) return;
  if (url.hostname.endsWith("r2.easyquran.fyi")) return;
  if (url.pathname === "/firebase-config.js") return;
  if (url.pathname.includes("/translations/")) return;
  if (url.origin === sw.location.origin && url.pathname.startsWith("/api/")) return;

  if (
    url.pathname === "/_app/version.json" ||
    url.pathname === "/service-worker.js" ||
    url.pathname === "/offline/manifest.json" ||
    (url.pathname.startsWith("/offline/pack.") && url.pathname.endsWith(".json"))
  ) {
    return;
  }

  if (url.pathname.startsWith("/_app/immutable/")) {
    event.respondWith(cacheFirstApp(req));
    return;
  }

  if (url.origin === sw.location.origin && url.pathname === ENV_MODULE) {
    event.respondWith(networkFirstApp(req, url.pathname));
    return;
  }

  if (req.mode === "navigate") {
    event.respondWith(handleNavigation(req));
    return;
  }

  if (url.origin === sw.location.origin && url.pathname.endsWith("/__data.json")) {
    event.respondWith(handleData(req));
    return;
  }

  if (url.origin === sw.location.origin && IMMUTABLE.has(url.pathname)) {
    event.respondWith(cacheFirstApp(req));
    return;
  }

  if (url.origin === sw.location.origin) {
    event.respondWith(swrApp(req));
  }
});

// Build output and static files have exactly one representation per URL, so
// lookups ignore Vary (precache keys carry no request headers; a page's
// script/font/image request does). Misses fall back to older eq-app-* caches
// (I7 grace: a tab on the previous build still loads its lazy chunks).
async function cacheFirstApp(req: Request): Promise<Response> {
  const cache = await caches.open(APP_CACHE);
  const hit = await cache.match(req, { ignoreVary: true });
  if (hit) return hit;
  try {
    const res = await fetch(req);
    if (res && res.ok && isCacheable(res)) {
      void cache.put(req, res.clone()).catch(() => {});
    }
    return res;
  } catch {
    const prior = await matchOtherAppCaches(req.url).catch(() => undefined);
    return prior ?? Response.error();
  }
}

// Runtime (non-hashed) app module: fresh when the network answers within the
// navigation budget, the cached copy otherwise, then one unbounded retry.
async function networkFirstApp(req: Request, key: string): Promise<Response> {
  const cache = await caches.open(APP_CACHE);
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), NAV_TIMEOUT_MS);
  try {
    const res = await fetch(req, { signal: controller.signal });
    if (res.ok && isCacheable(res)) {
      await cache.put(key, await storable(res.clone())).catch(() => {});
    }
    return res;
  } catch {
    // timed out or offline: fall through to the cache
  } finally {
    clearTimeout(timeout);
  }
  const hit =
    (await cache.match(key, { ignoreVary: true })) ??
    (await matchOtherAppCaches(key).catch(() => undefined));
  if (hit) return hit;
  try {
    return await fetch(req);
  } catch {
    return Response.error();
  }
}

// Navigation lookups use header-less Requests: a navigation carries
// `Accept: text/html,...`, and the Cache API compares that against the stored
// request whenever the stored response has `Vary: Accept`. Every navigation
// cache entry (precache shells, PAGES_CACHE) is stored under a header-less
// Request too, so the two always agree, while a negotiated non-navigation
// entry (e.g. markdown fetched under the same URL into the app cache by
// swrApp) is never handed to a navigation.
async function cachedShell(pathname: string): Promise<Response | undefined> {
  if (!SHELLS.has(pathname)) return undefined;
  const app = await caches.open(APP_CACHE);
  const hit = await app.match(new Request(pathname));
  if (hit) return hit;
  // A shell from the previous build still boots: its chunks are in that
  // build's eq-app-* cache, served by cacheFirstApp's fallback.
  return matchOtherAppCaches(pathname);
}

async function cachedPage(key: string, pathname: string): Promise<Response | undefined> {
  const pages = await caches.open(PAGES_CACHE);
  // Exact key first, then the bare path: an offline reload carrying a query the
  // page was never saved with (?source=pwa, utm_*) still finds the page.
  const candidates = key === pathname ? [key] : [key, pathname];
  for (const candidate of candidates) {
    const hit = await pages.match(new Request(candidate));
    if (!hit) continue;
    if (isCacheable(hit)) return hit;
    await pages.delete(new Request(candidate)).catch(() => {});
  }
  return undefined;
}

const OFFLINE_COPY = {
  en: {
    lang: "en",
    dir: "ltr",
    title: "You're offline",
    body: "This page isn't saved on this device yet. Pages you've opened before still work offline.",
    home: "Home",
    surahs: "Surah list",
    homeHref: SHELL_HOME,
    surahsHref: SHELL_SURAH,
  },
  ar: {
    lang: "ar",
    dir: "rtl",
    title: "أنت غير متصل",
    body: "هذه الصفحة غير محفوظة على هذا الجهاز بعد. الصفحات التي فتحتها من قبل تعمل دون اتصال.",
    home: "الرئيسية",
    surahs: "قائمة السور",
    homeHref: resolvePath("ar"),
    surahsHref: resolvePath("ar/surah"),
  },
} as const;

// Last resort for a navigation nothing cached can answer while the network is
// unreachable. Never a prerendered shell (see SHELLS): it would hydrate the
// wrong route under this URL. Self-contained (no app chunks), links to the
// precached shells, and reloads itself when connectivity returns.
export function offlineFallback(pathname: string): Response {
  const arabic = pathname === "/ar" || pathname.startsWith("/ar/");
  const copy = arabic ? OFFLINE_COPY.ar : OFFLINE_COPY.en;
  const html = `<!doctype html><html lang="${copy.lang}" dir="${copy.dir}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="light dark"><title>${copy.title}</title><style>body{font-family:system-ui,sans-serif;margin:0;min-height:100vh;display:grid;place-items:center;padding:16px;box-sizing:border-box}main{max-width:28rem;text-align:center}a{margin:0 .5rem}</style></head><body><main><h1>${copy.title}</h1><p>${copy.body}</p><p><a href="${copy.homeHref}">${copy.home}</a><a href="${copy.surahsHref}">${copy.surahs}</a></p></main><script>addEventListener("online",()=>location.reload())</script></body></html>`;
  return new Response(html, {
    status: 503,
    headers: { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" },
  });
}

export async function handleNavigation(req: Request): Promise<Response> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), NAV_TIMEOUT_MS);
  let network: Response | null = null;
  try {
    network = await fetch(req, { signal: controller.signal });
  } catch {
    network = null;
  } finally {
    clearTimeout(timeout);
  }

  const key = normalizeDataKey(req.url);

  if (network && network.ok && network.type !== "opaque" && isCacheable(network)) {
    await storePage(key, network);
    await touchRecency(req.url);
    return network;
  }
  if (network) return network;

  // Offline (or over the NAV_TIMEOUT_MS budget): this URL's own prerendered
  // shell, then this URL's saved page. Never another URL's HTML.
  const pathname = new URL(req.url).pathname;
  const cached = (await cachedShell(pathname)) ?? (await cachedPage(key, pathname));
  if (cached) {
    await touchRecency(req.url);
    return cached;
  }

  // Every cache missed and the NAV_TIMEOUT_MS budget already killed one fetch.
  // Retry once unbounded so a slow-but-online request (dev compiles a cold
  // route in seconds) still succeeds; an offline request rejects immediately
  // and gets the self-contained offline page instead of a browser error.
  try {
    const retry = await fetch(req);
    if (retry.ok && retry.type !== "opaque" && isCacheable(retry)) {
      await storePage(key, retry);
      await touchRecency(req.url);
    }
    return retry;
  } catch {
    return offlineFallback(pathname);
  }
}

async function storePage(key: string, res: Response): Promise<void> {
  try {
    const pages = await caches.open(PAGES_CACHE);
    await pages.put(new Request(key), await storable(res.clone()));
  } catch {}
}

export async function handleData(req: Request): Promise<Response> {
  const key = normalizeDataKey(req.url);
  const data = await caches.open(DATA_CACHE);
  let hit = await data.match(key);

  if (hit && !isCacheable(hit)) {
    await data.delete(new Request(key)).catch(() => {});
    await deleteDataMeta(key);
    hit = undefined;
  }

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), NAV_TIMEOUT_MS);
  const revalidate = fetch(req, { signal: controller.signal })
    .then(async (res) => {
      if (!res || !res.ok || !isCacheable(res)) return res;
      try {
        await data.put(new Request(key), res.clone());
        void recordDataEntry(key, res);
        void enforceDataBounds();
      } catch {
        await deleteDataMeta(key);
      }
      return res;
    })
    .catch(() => null)
    .finally(() => clearTimeout(timeout));

  if (hit) {
    void revalidate;
    void touchDataMeta(key);
    return hit;
  }

  const fresh = await revalidate;
  if (fresh && fresh.ok) return fresh;

  const activePack = await metaGet<{ packId?: string } | null>("activePack");
  const packId = activePack?.packId;
  if (packId) {
    const packName = `eq-pack-${packId}`;
    if (await caches.has(packName)) {
      const packHit = await caches.match(new Request(key), {
        cacheName: packName,
      });
      if (packHit) return packHit;
    }
  }
  // Same rationale as the handleNavigation retry: the revalidate timeout must
  // not downgrade a slow or erroring __data.json into an opaque error
  // Response; pass the real response (status included) through instead.
  try {
    return await fetch(req);
  } catch {
    return Response.error();
  }
}

async function swrApp(req: Request): Promise<Response> {
  const cache = await caches.open(APP_CACHE);
  const hit = await cache.match(req);
  const network = fetch(req)
    .then(async (res) => {
      if (res && res.ok && isCacheable(res)) {
        await cache.put(req, res.clone()).catch(() => {});
      }
      return res;
    })
    .catch(() => null);
  if (hit) {
    void network;
    return hit;
  }
  const fresh = await network;
  return fresh || Response.error();
}

async function touchRecency(rawUrl: string): Promise<void> {
  const key = normalizeDataKey(rawUrl);
  return serializeRecency(async () => {
    const recency = (await metaGet<Record<string, number>>("recency")) || {};
    recency[key] = Date.now();
    await metaSet("recency", recency);
  });
}

let maintenanceInFlight = false;

type MaintenanceCursor =
  | { stage: "pages"; offset: number }
  | { stage: "data"; offset: number }
  | { stage: "trim" }
  | { stage: "done" }
  | null;

async function setCursor(cursor: MaintenanceCursor): Promise<void> {
  await metaSet("maintenance", { cursor });
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the maintenance record read back from IndexedDB, which returns it untyped; this fn is the parse
function normalizeCursor(raw: unknown): MaintenanceCursor {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- opaque IDB record; the runtime object check is the only available discriminator
  if (raw && typeof raw === "object") {
    // SAFETY: raw is a non-null object by the guard above; the cast only
    // exposes the persisted fields for validation below.
    const c = raw as { stage?: unknown; offset?: unknown };
    if (c.stage === "trim") return { stage: "trim" };
    if (c.stage === "done") return { stage: "done" };
    // eslint-disable-next-line anti-slop/no-runtime-typeof -- validating the persisted offset field; no schema/parser seam exists in the SW
    if ((c.stage === "pages" || c.stage === "data") && typeof c.offset === "number") {
      return { stage: c.stage, offset: c.offset };
    }
  }
  return null;
}

async function runMaintenance(): Promise<void> {
  if (maintenanceInFlight) return;
  maintenanceInFlight = true;
  try {
    const meta = await metaGet<{ cursor: unknown }>("maintenance");
    let cursor: MaintenanceCursor = normalizeCursor(meta?.cursor);
    let successes = 0;
    let attempted = 0;
    for (;;) {
      if (cursor === null) {
        const r = await revalidateCache(PAGES_CACHE, 0, "pages");
        successes += r.successes;
        attempted += r.attempted;
        cursor = { stage: "data", offset: 0 };
        await setCursor(cursor);
        continue;
      }
      switch (cursor.stage) {
        case "pages": {
          const r = await revalidateCache(PAGES_CACHE, cursor.offset, "pages");
          successes += r.successes;
          attempted += r.attempted;
          cursor = { stage: "data", offset: 0 };
          await setCursor(cursor);
          continue;
        }
        case "data": {
          const r = await revalidateCache(DATA_CACHE, cursor.offset, "data");
          successes += r.successes;
          attempted += r.attempted;
          cursor = { stage: "trim" };
          await setCursor(cursor);
          continue;
        }
        case "trim": {
          await trimPages();
          await enforceDataBounds();
          if (attempted > 0 && successes === 0) {
            await setCursor(null);
            return;
          }
          cursor = { stage: "done" };
          await setCursor(cursor);
          continue;
        }
        case "done": {
          return;
        }
        default: {
          const _: never = cursor;
          void _;
          return;
        }
      }
    }
  } finally {
    maintenanceInFlight = false;
  }
}

async function revalidateCache(
  cacheName: string,
  start: number,
  stage: "pages" | "data",
): Promise<{ successes: number; attempted: number }> {
  const cache = await caches.open(cacheName);
  const all = await cache.keys();
  const slice = all.slice(start).map((req, i) => ({ req, idx: start + i }));
  const done = new Set<number>();
  let contiguous = start;
  let successes = 0;
  await pool(slice, MAINTENANCE_CONCURRENCY, async (item) => {
    let ok = false;
    try {
      const res = await fetch(item.req, { cache: "no-store" });
      if (res && res.ok && isCacheable(res)) {
        await cache.put(item.req, res.clone());
        if (stage === "data") void recordDataEntry(normalizeDataKey(item.req.url), res);
        ok = true;
      }
    } catch {
      // keep stale on failure
    }
    if (ok) successes++;
    done.add(item.idx);
    while (done.has(contiguous)) contiguous++;
    await setCursor({ stage, offset: contiguous });
  });
  return { successes, attempted: slice.length };
}

async function trimPages(): Promise<void> {
  const cache = await caches.open(PAGES_CACHE);
  const reqs = await cache.keys();
  if (reqs.length <= PAGES_MAX) return;
  await serializeRecency(async () => {
    const recency = (await metaGet<Record<string, number>>("recency")) || {};
    const scored = reqs.map((req) => ({
      req,
      last: recency[normalizeDataKey(req.url)] ?? 0,
    }));
    scored.sort((a, b) => a.last - b.last);
    const evict = scored.slice(0, reqs.length - PAGES_MAX);
    const nextRecency = { ...recency };
    for (const { req } of evict) {
      await cache.delete(req).catch(() => {});
      const key = normalizeDataKey(req.url);
      delete nextRecency[key];
    }
    await metaSet("recency", nextRecency);
  });
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the url field of an untrusted push JSON payload; this fn validates it before use
function safeTarget(raw: unknown): string {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- untrusted push payload field; the runtime string check precedes URL parsing
  if (typeof raw !== "string" || raw.length === 0) return "/";
  try {
    const u = new URL(raw, sw.location.origin);
    if (u.origin !== sw.location.origin) return "/";
    return `${u.pathname}${u.search}${u.hash}`;
  } catch {
    return "/";
  }
}

sw.addEventListener("push", (event) => {
  event.waitUntil(
    sw.registration.showNotification(PUSH_TITLE, {
      body: PUSH_BODY,
      icon: "/icons/icon-192.png",
      badge: "/icons/badge-96.png",
      tag: PUSH_TAG,
      data: { url: "/account" },
    }),
  );
});

sw.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const target: string = safeTarget(event.notification?.data?.url);
  event.waitUntil(
    (async () => {
      const all = await sw.clients.matchAll({ type: "window", includeUncontrolled: true });
      for (const client of all) {
        await client.focus();
        try {
          await client.navigate(target);
        } catch {
          void 0;
        }
        return;
      }
      await sw.clients.openWindow(target);
    })(),
  );
});
