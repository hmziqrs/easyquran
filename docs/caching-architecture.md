# Caching Architecture — Current Snapshot

**Scope:** the two caching domains in this repo — (A) the **web offline cache stack** and (B) the **Rust API hot-database cache** for Quran translations. This is a *description of what is built today*, verified against code, so it can be diffed later to see what changed.

**Snapshot date:** 2026-08-08
**Commit:** `b17324d` (`master`)
**Method:** 10 parallel code-readers (one per subsystem) + a drift/gap critic cross-checked against `docs/quran.md`. Two load-bearing claims (the `/scripts` URL contract and the search-ETag digest) were re-verified by hand against `controller.rs:810` and `controller.rs:1163`.

> This doc is descriptive, not prescriptive. It mirrors `docs/quran.md` (the plan) where they agree and explicitly flags where the code differs from the plan. For the plan and its rationale, read `docs/quran.md`; for *what is running right now*, read this.

---

## 1. At a glance

Two domains, no shared cache between them. They meet only at the HTTP edge and at the `/quran` JSON contract.

```
                        BROWSER TAB
  ┌─────────────────────────────────────────────────────────────┐
  │ SvelteKit app                                               │
  │  ├─ routes (marketing SSG · app reader SSG/SSR · /t SSR)    │
  │  ├─ Service Worker ── Cache Storage (shell/pages/data/pack) │  ◀── Domain A: web offline
  │  ├─ Quran Web Worker ── OPFS + IndexedDB (sqlite-wasm)      │
  │  ├─ catalogue/manifest resolver (baked → api → fallback)    │
  │  └─ engagement prefetch (view-gated translation download)   │
  └─────────────────────────────────────────────────────────────┘
              │ same-origin /_quran/* gateway (allowlist + Range → R2)
              ▼
        Cloudflare R2 (immutable tanzil/ artifacts)            ─── publisher-owned source of truth

              │ /quran/* JSON (Arabic + translations)
              ▼
  ┌─────────────────────────────────────────────────────────────┐
  │ Axum API (quran_v1)                                         │  ◀── Domain B: rust hot-db
  │  ├─ Arabic corpus: boot-resident arena, never evicted       │
  │  ├─ TranslationPool: moka, single-flight, byte+count bound  │
  │  ├─ HTTP cache policy: weak ETag + per-family Cache-Control │
  │  └─ rate-limit store (in-mem L1 + SQLite L2) · /health      │
  └─────────────────────────────────────────────────────────────┘
```

### Cross-cutting invariants (hold across both domains)

1. **Quran databases are immutable.** Never written, never versioned. Sourced from Tanzil.net.
2. **Identity = id, never a hash.** `uthmani`, `simple-clean`, `en.sahih`, … OPFS keys, ETags, catalogue rows, specs are all id-keyed. SHA-256 over Quran data is *manual audit only* (`just quran-audit`, `pnpm audit:arabic`, `pnpm verify`) — never in build/boot/runtime/ETag/cache-key. Guarded by `tests/quran_v1.rs` + `web catalogue-sha-guard.test.ts`.
3. **Integrity at runtime is shape, not digest.** Boot asserts 6236 rows + tiling + ayah-keys; downloads size-check bytes only (`verifyBytes`), never hash.
4. **Arabic is always resident/pinned; only translations are evictable.** True on both sides (Rust boot-resident corpus; web pinned Arabic in OPFS).

---

## Domain A — Web offline caching

Six layers, each owning its own storage. No layer shares a cache with another; coordination is via message-passing (postMessage/BroadcastChannel) and shared IDB meta.

### A1. Service worker — app shell, pages, data, offline pack

`web/src/service-worker.ts` (the SW) + `web/src/lib/boot/service-worker.ts` (registration) + `web/src/lib/offline/*` (client-side counterparts).

**What it caches** (four Cache-Storage buckets + IDB meta):

| Cache name | Purpose | Keying | Eviction |
|---|---|---|---|
| `eq-app-${version}` | App shell + immutable build assets (`build`, `files`, `/`, `/app`, `quran-data.json`) | per-build; `version` = SvelteKit build id | atomic `caches.delete` on install failure; old `eq-app-*` pruned after handoff |
| `eq-pages-v1` | Rendered navigation HTML (cross-version) | full navigation URL | LRU trim past `PAGES_MAX=300` (IDB `recency` map) |
| `eq-data-v1` | SvelteKit `__data.json` (cross-version) | normalized (pathname + non-`x-sveltekit` query) | **none — unbounded** (SWR revalidate only) |
| `eq-pack-${packId}` | Offline route pack (all reader `__data.json` staged for full-offline read) | pack entry path, `ignoreSearch` | deleted on disable / incomplete reconcile / pack supersession |

IDB meta: db `easyquran-sw-meta` / store `meta` (keys: `installedVersion`, `activePack`, `ack:${clientId}`, `recency`, `maintenance` cursor). localStorage/sessionStorage: `easyquran.update.waiting`, `easyquran.reload-guard`, `easyquran.offline.pack`.

**Per-request strategies** (fetch router, GET only):

```
/_app/immutable/* ............... cache-first (eq-app)
navigate (HTML) ................. network-first, 3500ms abort
                                  fail → eq-app shell → eq-pages → eq-app SHELL_ROUTE → error
*__data.json .................... stale-while-revalidate (3500ms bg revalidate)
                                  miss → eq-pack-${activePack} fallback → error
IMMUTABLE build/files ........... cache-first
other same-origin ............... SWR
BYPASS → network: /quran/, /_quran/, r2.easyquran.fyi,
       /translations/, /firebase-config.js, /_app/version.json,
       /service-worker.js, /offline/manifest.json, /offline/pack.*.json
```

**Lifecycle (atomic update):**
- **Install** → `precache()` fetches each `PRECACHE` URL with `cache:'no-cache'`; any failure throws + `caches.delete(APP_CACHE)` (all-or-nothing). **Exception:** `SHELL_ROUTE` (`${base}/404.html`) fetch is wrapped in `.catch(()=>null)` with a 4s abort — a missing/failing shell is silently skipped and install proceeds *without* a fallback shell.
- **Activate** → `clients.claim()`; detects prior `eq-app-*`; on version change resets the maintenance cursor; if a prior existed, broadcasts `UPDATE_TAKEOVER`.
- **Handoff** → client posts `APP_READY` → SW records `ack:${clientId}` → `maybeFinalizeHandoff` prunes dead-client acks → only when *all live clients* ack the new version does `pruneOldAppCaches` delete old `eq-app-*`.
- **Update** → user clicks "Reload open tabs" (`UpdateToast`) → arms `reload-guard` (session), broadcasts `PREPARE_RELOAD`, posts `SKIP_WAITING` → new SW activates → `controllerchange` → every armed tab reloads. No auto-apply; a never-clicked update waits forever.
- **Maintenance** → `runMaintenance()` (single-flight via `maintenanceInFlight`): stages `pages → data → trim`, revalidates at `MAINTENANCE_CONCURRENCY=6`, trims `eq-pages` LRU past 300.

**Contract duplication:** the message types (`SKIP_WAITING`/`APP_READY`/`UPDATE_TAKEOVER`/`PREPARE_RELOAD`) and channel names are **shared** via the SW's relative import `./lib/offline/messages` (relative imports *do* work in this SW). Only `META_DB`/`META_STORE` (`service-worker.ts:21-22` vs `lib/offline/meta.ts:4-5`) and `normalizeDataKey` (`service-worker.ts:35-44` vs `lib/offline/keys.ts`, differing URL base) are genuinely duplicated inline — both are drift hazards.

**Constants:**
| Constant | Value | Where |
|---|---|---|
| `APP_CACHE` | `eq-app-${version}` | service-worker.ts:17 |
| `PAGES_CACHE` | `eq-pages-v1` | service-worker.ts:18 |
| `DATA_CACHE` | `eq-data-v1` | service-worker.ts:19 |
| `NAV_TIMEOUT_MS` | 3500 | service-worker.ts:25 |
| `PAGES_MAX` | 300 | service-worker.ts:26 |
| `MAINTENANCE_CONCURRENCY` | 6 | service-worker.ts:27 |
| shell precache abort | 4000 ms | service-worker.ts:155 |
| `version.pollInterval` | 5 min | vite.config.ts:19 |
| SW registration | `{scope:'/', updateViaCache:'none'}`, PROD-only | boot/service-worker.ts |

### A2. OPFS + SQLite-WASM offline engine (the Quran web worker)

`web/src/lib/workers/quran.worker.ts` (worker) + `worker-client.ts` (main-thread bridge) + `workers/opfs-cache.ts`, `opfs-retention.ts`, `download.ts`, `storage.ts`, `idb.ts`.

**Boots SQLite-WASM**, eagerly loads the two pinned Arabic DBs, serves read/search, lazily downloads translation dumps into OPFS keyed by `spec.id`, and prunes translations on LRU + size + TTL.

**Storage tiers:**

| Tier | Medium | Key | TTL / cap | Eviction |
|---|---|---|---|---|
| OPFS bytes (primary) | `FileSystemDirectoryHandle` | dir `easyquran/<spec.id>/`, file `<spec.id>.sqlite` | immutable | Arabic pinned; translations via prune |
| IDB bytes (fallback) | IndexedDB | db `easyquran-quran`/`artifacts`, key `${spec.id}:${spec.id}` | immutable | same prune pass |
| lastUsed meta | IndexedDB | db `easyquran-meta`/`lastUsed`, key `spec.id`, value `Date.now()` ms | — | cleared on eviction |
| in-mem open-DB LRU | `Map<sourceId,Database>` | sourceId | session | evict past `TRANSLATION_DB_CAP=4` (RAM only; does **not** touch OPFS) |
| in-mem Arabic | `Map<QuranSourceId,WorkerSourceState>` | uthmani / simple-clean | session | **never** (pinned) |

**Retention (`opfs-retention.ts`):** `pruneTranslations` runs single-flight on (a) worker init completion and (b) each successful translation download. `computeEvictions` drops pinned Arabic, sorts candidates by `lastUsed` asc, evicts until: remaining ≤ `CAP_COUNT=12` **and** total bytes ≤ `CAP_BYTES=128 MiB` **and** oldest is fresher than `TTL_MS=30d`. Caps count non-pinned candidates only; Arabic bytes sit outside both. No `navigator.storage.estimate()` awareness — caps are fixed regardless of actual quota.

**Integrity:** `verifyBytes` is size-only (`buf.byteLength !== spec.sizeBytes`), no sha. OPFS `put` is non-atomic (`createWritable`+write+close) — a mid-write crash leaves a corrupt file recovered only by the next read's size-check.

**Single-flight throughout:** `bootPromise` (init), `pruneInFlight` (prune), `pendingTranslationRunners` (per-id fetch), `idbConnections` (one IDBDatabase per `${dbName} ${storeName}`). Main-thread client: per-request `pending Map<seq>` with `DEFAULT_TIMEOUT_MS=30000`.

**Constants:** `TTL_MS=30d`, `CAP_COUNT=12`, `CAP_BYTES=128 MiB`, `TRANSLATION_DB_CAP=4`, `ROOT_DIR=easyquran`, `PINNED_ARABIC=[uthmani,simple-clean]`, `DEFAULT_TIMEOUT_MS=30000`, `IDB_VERSION=1`.

### A3. Catalogue + manifest + source-plan resolution

`catalogue.ts`, `manifest.ts`, `source-plan.ts`, `catalogue-store.svelte.ts`, `environment.ts`, `lib/data/translations.json`.

Resolves available sources at boot → produces a `DownloadableSpec`/`ArtifactSpec` per source. Identity is always the id; resolution falls through **baked → api → fallback**, never hard-failing.

**Resolution ladder:**

```
MANIFEST (Arabic, 2 frozen ids):
  !apiBase ──────────────────────────────► baked {scripts: QURAN.scripts, source:'baked'}
  else GET /scripts (3s):
    non-ok / decode-null / plan-mismatch / throw ─► baked
    pass ─────────────────────────────────► localize URLs → {scripts, source:'api'}

CATALOGUE (115 translations, 44 langs):
  SSR (resolveSourceCatalogue):  !apiBase ►baked · fresh cache? ►cached · empty/failed ►baked (uncached)
                                  module cache TTL 300s, single-flight pendingCatalogue
  Client (catalogueStore):       ready ►live · degraded(+30s backoff) ►stale baked · else fetch
                                  status machine idle→loading→ready|degraded
```

**Baked catalogue:** `lib/data/translations.json` — flat positional array, 115 rows × 8 fields (`TranslationField` 0..7: id, language, languageCode, direction, name, translator, filePath, sizeBytes), `TRANSLATION_FIELD_COUNT=8` strictly enforced. Re-parsed on *every* call (no memoization).

**`/scripts` is NOT dead.** The Rust `/scripts` builder emits `{public_url}/tanzil/arabic/{filename}` (`controller.rs:810`) — the publisher's layout, matching the baked manifest. The URL contract drift described in `docs/quran.md §1` is in the *done* section because it was fixed; when R2 serves the files, `/scripts` returns 2 verified artifacts and `resolveManifest` can flip `source` to `'api'`.

**Two independent, non-shared catalogue caches:** SSR (`catalogue.ts` module cache, TTL 300s) and client (`catalogueStore`, status machine). The client bypasses `resolveSourceCatalogue` entirely, so they can diverge. `resolveManifest` has neither cache nor single-flight (called once per boot, so cheap today).

**Constants:** `SOURCE_CATALOGUE_TTL_MS=300000`, `ERROR_BACKOFF_MS=30000`, `FETCH_TIMEOUT_MS=3000`, `ManifestSource='api'|'baked'`, `QuranSourceId={uthmani,simple-clean}`, `QURAN_R2_UPSTREAM_BASE=https://r2.easyquran.fyi`, `LOCAL_QURAN_ARTIFACT_BASE=QURAN_ARTIFACT_DELIVERY_BASE='/_quran'`.

### A4. Network resilience + engagement prefetch

`fetch.ts`, `api-client.ts`, `engagement.ts`, `track-view.svelte.ts`, `quran/worker-client.ts`, `workers/download.ts`, `lib/server/owner.ts`.

- **`fetchWithTimeout`** (`FETCH_TIMEOUT_MS=3000`): wraps `fetch` in an `AbortController`, composes the caller's external signal onto the inner controller. No retry, no backoff.
- **`quranApi`** (`api-client.ts`): `readSurah` → `GET /sources/{id}/surah/{n}`, `readRange` → `GET /sources/{id}/range?from=&to=`, `search` → `GET /search?q=&limit=&offset=`. Throws on `!res.ok`. No `Retry-After` honoring.
- **`withTranslationFallback`** (`worker-client.ts:108`): tier chain — `[no-worker & apiBase → api] → [cached → worker] → fire bg ensureTranslation → [apiBase → api] → [re-check cache → worker] → throw`. The API call is the network fallback when the worker DB is absent.
- **Engagement prefetch** (`engagement.ts`): after `VIEWS_BEFORE_PREFETCH=2` reader views of **any** source (Arabic views count toward the total — `bumpViews` runs before the Arabic early-return), `whenIdle` (`requestIdleCallback`, 5s cap) triggers `ensureTranslation` (translation sources only). Gated by `navigator.connection.saveData` / `effectiveType 2g|slow-2g`. **One retry max** per source per tab (`retried` Set; cleared only by test-only `__resetEngagementState`).
- **`downloadBytes`** (`workers/download.ts`): bare `fetch`, `Accept-Encoding: identity`. **No timeout, no AbortController, no retry** — the one fetch in the subsystem not wrapped by `fetchWithTimeout`; a hung CDN connection can block a fire-and-forget `ensureTranslation` indefinitely.

**Constants:** `FETCH_TIMEOUT_MS=3000`, `VIEWS_BEFORE_PREFETCH=2`, `whenIdle` cap 5000, `DEFAULT_TIMEOUT_MS=30000`, owner cache `CACHE_TTL_MS=3600000` (1h, caches fallback on failure too).

### A5. SSR disk-TTL HTML cache (translated pages only)

`web/src/lib/server/quran-disk-cache.ts` + `web/src/hooks.server.ts:102` + `web/src/routes/health/quran/+server.ts`.

`adapter-node` SSR HTML disk cache for **translated reader routes only** — the four route IDs whose `id` includes `/t/[lang]/[translator]`: `/app/[surah]/t/[lang]/[translator]`, `/app/[surah]/t/[lang]/[translator]/page/[localPage]`, `/app/t/[lang]/[translator]/page/[n]`, `/app/t/[lang]/[translator]/juz/[n]` (URLs like `/app/al-baqarah/t/en/sahih`, `/app/t/en/sahih/page/2`). The matcher is a route-id *substring* check (`id.includes("/t/[lang]/[translator]")`, `hooks.server.ts:13`), not a URL prefix, which is why the `/app/`-nested translation routes are caught. Arabic `/app/[surah]`, `/app/juz/[n]`, `/app/page/[n]` stay prerendered/SSG and produce a null cache key (never intercepted). `__data.json` is excluded (`isDataRequest`).

**Key:** filename `<sanitized(diskCacheKey)>.html`, where `diskCacheKey = [build-${appBuildId}, sourceId, kind, String(a)[, String(b)]]` joined `__`, sanitized `/[^a-z0-9.-]+/gi → '_'`. `KINDS=['surah','page','juz']`. Example: `build-<id>__en.sahih__surah__2__1`. The `build-<appBuildId>` prefix namespaces disposable HTML per web build (so cached markup never points at removed assets) — **not** Quran versioning; `sourceId` carries Quran identity.

**TTL/budget:** 7 days (`DEFAULT_TTL_MS=604800000`), 256 MiB LRU budget (`DEFAULT_BUDGET_BYTES=268435456`), orphan-`.tmp` reap > 1h. Env-overridable; compose defaults identical.

**Eviction:** `#prune()` runs only on `set()` — reaps expired + orphan `.tmp` + oldest-survivors-by-mtime until ≤ budget. No background TTL sweeper. Atomic write via tmp+`fs.rename`. `get()` `fs.utimes`-touches hits (LRU recency).

**Observability:** `Server-Timing: quran_ssr_cache;desc="hit"|desc="miss"` (quoted `desc` token) + `X-EasyQuran-Quran-Cache: hit|miss` on every cacheable translated GET (hit and miss). `GET /health/quran` (web route) returns `{ready, appBuildId, translatedPageCache: {entries, bytes, hits, misses, writes, evictions, errors}}`, `no-store`.

**Medium:** Docker named volume `web_quran_cache` at `/app/cache/quran-ssr` (prod); `process.cwd()/.cache/quran-ssr` (dev). Single web container in compose → single writer.

> Note: `GET /health/quran` (web) is a **different endpoint** from the Rust `GET /quran/health/ready` — the former reports the SSR disk cache, the latter reports the translation pool. Do not conflate.

### A6. `/_quran` artifact gateway

`web/src/routes/_quran/[...artifact]/+server.ts`.

Same-origin streaming proxy: lets the browser/worker fetch baked Arabic/translation SQLite from R2 without a cross-origin request. **Allowlist** (`ALLOWED_ARTIFACTS`, 117 keys built once at module load from `translations.json` + 2 Arabic) gates every request; unknown → 404. Forwards `Range` verbatim to `${QURAN_R2_UPSTREAM_BASE}/${artifact}`, passes upstream `content-length`/`content-range`/`accept-ranges` through, stamps `Cache-Control: public, max-age=31536000, immutable` + a weak id-based `ETag: W/"quran-artifact:<artifact>"`. `If-None-Match` match (non-Range) → 304 short-circuit (no upstream hop). Upstream non-OK/non-206 → 502; 5xx gets `no-store` via hooks. `accept-encoding: identity` forces uncompressed bytes. The SW explicitly bypasses `/_quran/`. No bytes rewritten or hashed in transit.

---

## Domain B — Rust hot-database caching

Four subsystems. Arabic is always resident; translations are on-demand; HTTP responses are edge-cacheable via headers (no server-side response store); rate-limiting is in-process with SQLite durability.

### B1. Arabic boot-resident corpus (never evicted)

`quran/loader.rs` + `quran/store.rs` + boot in `main.rs` (load call `:438`, `Arc` wrap `:448`, `AppState` field `:512`; `exit(1)` on failure `:455`).

Both Arabic scripts (`quran-uthmani.sqlite` = display, `quran-simple-clean.sqlite` = search corpus) load **once at boot** into a packed in-memory `Corpus { arena: Box<str>, offsets: Box<[u32]> }` — all verse text contiguous, O(1) `verse(g)` slice via `arena[offsets[g-1]..offsets[g]]`. Wrapped in `Arc<QuranStore>` in `AppState`, held for process lifetime.

**Boot is fail-fast.** `validate_rows` asserts `rows.len()==6236` and contiguous `1..=6236`; `build_meta` asserts 114 suras, ayah-key cross-check, per-sura row count, bismillah split `1/112/1`, range counts (juzs=30/pages=604/rukus=556/quarters=240/manzils=7), 15 contiguous sajdas, and full tiling (first `start_global==1`, last `end_global==6236`, no gap/overlap). Any violation → `Err` → `main.rs:455 std::process::exit(1)`.

**No SHA at boot.** Shape asserts only. The sqlite file is opened `read_only(true).immutable(true)`, queried once, `conn.close()` before boot returns — no DB handle survives startup (proven by `no_sqlite_access_after_startup`, `#[cfg(unix)]`). `Corpus::joined_for_digest` exists but is dead code in the binary — reserved for the manual `just quran-audit`.

**No fallback, by design:** one source file per script, no replica; any failure refuses boot rather than serving a partial corpus. `verse(g)` cannot miss post-boot.

**Constants:** `VERSE_COUNT=6236`, `SURA_COUNT=114`, `RESPONSE_CAP=300`, `etag_tag="quran-corpus"` (static `&'static str`).

### B2. Translation pool (moka, single-flight, byte+count bound)

`quran/translation_pool.rs` + wiring in `state.rs:57`, `main.rs:459-492`, bounds in `config/settings.rs:136-165`.

On-demand in-memory cache of translation `Corpus` (one per source id), served by `controller::source_surah` (`GET /quran/sources/{sourceId}/surah/{surah}`, handler `:1180`) and `source_range` (`GET /quran/sources/{sourceId}/range`, handler `:1222`) — the quran_v1 router is nested at `/quran` (`main.rs:694`). Completely separate from the Arabic resident store.

**Cold build:** `get_or_build(id)` → `moka::future::Cache::try_get_with(id, init)` — **single-flight**: N concurrent cold requests for the same id coalesce into one build (test: 16 concurrent → `builds==1`). The `init` closure acquires a `BUILD_CONCURRENCY=2` semaphore permit (gates cold builds only; cached hits skip it), then `load_translation_corpus` (sqlite `read_only+immutable`, open→scan→close per cold build — no connection pooling). **No `std::sync` guard held across `.await`** (tokio `Semaphore` permit is `Send`), so handler futures stay `Send`.

**Two-stage eviction (whichever trips first):**
1. **moka count ceiling** — `max_capacity = max_resident_translations.max(1)` (default 8; the `.max(1)` floor guards a misconfigured 0), `time_to_idle = idle_ttl` (default 1800s).
2. **byte ceiling** — enforced by a *separate serialized prune pass* (`enforce_byte_bound`, `prune_sem=1`): after each `get_or_build`, loop `run_pending_tasks → if resident_bytes > max → invalidate min-access-tick resident` until under bound. **There is no moka weigher** — the byte bound is after-the-fact, with transient overshoot bounded to ~2 concurrent builds.

The async eviction listener decrements `resident_bytes`, and for `cause != Replaced` drops the `residents`-map entry + bumps `evictions` + records a timestamp (60s rolling window → `evictions_per_minute`).

**Bounds (settings, env-overridable):**

| Bound | Default | Env |
|---|---|---|
| max resident translations | 8 | `QURAN_MAX_RESIDENT_TRANSLATIONS` |
| max resident bytes | 48 MiB | `QURAN_MAX_RESIDENT_TRANSLATION_BYTES` |
| idle TTL | 1800 s (30 min) | `QURAN_TRANSLATION_IDLE_TTL_SECS` |
| `BUILD_CONCURRENCY` | 2 | const (`translation_pool.rs:17`) |
| prune semaphore | 1 | const (`:70`) |

**Security:** `parse_id` whitelist membership *is* the path-traversal guard (`TranslationId::parse(s, &id_whitelist)`); unknown/traversal ids → `None` → 400, no path ever built from raw input. Catalogue + `id_whitelist` frozen at boot (no runtime reload).

**Stats** (`PoolStats`, via `/quran/health/ready`): `resident_count`, `resident_bytes`, `builds`, `lookups`, `hit_rate` (= `1 - builds/lookups`, or `1.0` when `lookups==0`), `evictions`, `evictions_per_minute`.

### B3. HTTP cache policy + weak ETag (stateless)

`modules/quran_v1/cache.rs` + `controller.rs` + `cors.rs`.

Stateless header-emission layer (no server-side response store). Every content handler picks a `Cache-Control` family and stamps a weak ETag via `cache::respond_cached[_with_etag]` — the sole exception is `/health/ready`, which builds its response manually with `NO_STORE` and **no** ETag (`controller.rs:1019-1024`).

```
weak_etag(tag, canonical_key) = W/"{tag}:{canonical_key}"
  Arabic tag   = "quran-corpus"        (static &'static str; store.rs:325-327)
  Translation  = "tanzil-{id}"         (controller.rs:54-56)
```

`If-None-Match` match → 304 (empty body, keeps ETag/Cache-Control/Vary). `canonical()` sorts query params so equivalent query orders collapse. `Vary: Accept-Encoding` on every 200/304.

| Family | Value | Used by |
|---|---|---|
| `ARABIC_CACHE` | `public, max-age=300, s-maxage=86400, stale-while-revalidate=604800, stale-if-error=604800` | surah/ayah/range/juz/page/ruku/hizb/manzil/sources(/scripts) |
| `SEARCH_CACHE` | `public, max-age=60, s-maxage=300` | `/search` only |
| `IMMUTABLE_CACHE` | `public, max-age=31536000, immutable` | `/random?date=` only |
| `NO_STORE` | `no-store` | `/health/ready`, partial `/scripts`/`/sources`, **all 5xx** (`error.rs:84`) |

**Search-router split** (`main.rs:672-690`, `mod.rs:57-59`): `/search` is mounted on its own `Router` so it gets a tighter `30/60s` rate limit *before* the merge, then the merged public router is wrapped by `600/60s` `PathKey::Fixed("quran-v1")` — so search is double-limited, and parameterized routes can't fan into separate buckets.

**CORS** (`cors.rs`): `allow_headers` includes `IF_NONE_MATCH`/`IF_MODIFIED_SINCE`/`CACHE_CONTROL`, `expose_headers` includes `ETAG`/`CACHE_CONTROL`/`RETRY_AFTER`, `max_age=86400`.

### B4. Rate-limit store + health/metrics

`crates/rux-request-gate/` (layer, store, abuse, ip) + `services/rate_limit_store.rs` (L2) + `controller.rs:985` (`/quran/health/ready`).

In-process (no Redis) **fixed-window per-IP** rate limiting + dual-threshold abuse limiter + one-shot dedup. Two tiers:

- **L1** — `InMemoryStore`: `Mutex<HashMap<String,Bucket>>` (limits) + `Mutex<HashMap<String,SystemTime>>` (dedup claims). Key = `ratelimit:{ip}:{path}`. Fixed window `now + 60s`; auto-resets on next `incr_expire` after expiry. `count` + `block` under one lock (race-safety, no test pinning it).
- **L2** — SQLite table `rate_limit_state(key PK, fixed_count, fixed_expires_at, block_until_at)`, same SeaORM connection as the app DB. `spawn_flush_task` every `FLUSH_INTERVAL_SECS=10` (`MissedTickBehavior::Skip`): `DELETE` expired + `UPSERT` live snapshots + `store.prune()`. Boot: `ensure_table → load → restore → spawn_flush_task`. L2 failures are non-fatal (warn); L1 keeps enforcing.

**Fail modes:** rate-limit store error → fail-**CLOSED** (503); abuse-check error → fail-CLOSED (503); dedup → fail-**OPEN** (allows the action). IP-resolution failure → request collapses into a shared `unknown` bucket.

**`/quran/health/ready`** (`controller.rs:985`, `no-store`): `HealthReady` exposing Arabic resident bytes, load/catalogue durations, and `TranslationPoolHealth` (pool stats + max bounds + idle TTL). `ready` is hardcoded `true`. (This is the Rust endpoint; the web `GET /health/quran` for the SSR disk cache is a separate surface — see A5.) No Prometheus/`/metrics` text endpoint — limiter counters leave only via OTLP push.

**Limits:**

| Scope | Limit | PathKey |
|---|---|---|
| all `/quran` (incl `/health/ready`) | 600 / 60s | `Fixed("quran-v1")` |
| `/quran/search` | 30 / 60s (plus the 600/60 above) | `Raw` |
| private (auth/post/comment/media/…) | 100/200/100/30/… per 60s | `Raw` |

---

## 2. Constants reference (quick-diff anchors)

### Web
| Value | Constant | Location |
|---|---|---|
| `eq-app-${version}` / `eq-pages-v1` / `eq-data-v1` | cache names | service-worker.ts:17-19 |
| 3500 / 300 / 6 / 4000 | `NAV_TIMEOUT_MS` / `PAGES_MAX` / `MAINTENANCE_CONCURRENCY` / shell abort | service-worker.ts:25-27,155 |
| 30 d / 12 / 128 MiB / 4 | OPFS `TTL_MS` / `CAP_COUNT` / `CAP_BYTES` / `TRANSLATION_DB_CAP` | opfs-retention.ts:8-10, quran.worker.ts:63 |
| 30000 | worker `DEFAULT_TIMEOUT_MS` | worker-client.ts:32 |
| 300000 / 30000 / 3000 | catalogue TTL / backoff / `FETCH_TIMEOUT_MS` | catalogue.ts:116, catalogue-store.svelte.ts:5, fetch.ts:1 |
| 2 | `VIEWS_BEFORE_PREFETCH` | engagement.ts:9 |
| 7 d / 256 MiB / 1 h | SSR TTL / budget / orphan-tmp | quran-disk-cache.ts:10-12 |
| `tanzil/arabic/<file>.sqlite`, `tanzil/translations/sqlite/<id>.sqlite` | R2 layout | environment.ts, gateway allowlist |

### Rust
| Value | Constant | Location |
|---|---|---|
| 6236 / 114 / 300 | `VERSE_COUNT` / `SURA_COUNT` / `RESPONSE_CAP` | store.rs:1-3 |
| `"quran-corpus"` / `"tanzil-{id}"` | ETag tags | store.rs:325-327, controller.rs:54-56 |
| 8 / 48 MiB / 1800 s / 2 / 1 | pool max-count / max-bytes / idle-TTL / `BUILD_CONCURRENCY` / prune-sem | settings.rs:160-162, translation_pool.rs:17,70 |
| `max-age=300, s-maxage=86400, swr=604800, sie=604800` | `ARABIC_CACHE` | cache.rs:5-6 |
| `max-age=60, s-maxage=300` | `SEARCH_CACHE` | cache.rs:8 |
| `max-age=31536000, immutable` | `IMMUTABLE_CACHE` | cache.rs:10 |
| 600 / 30 (per 60s) | quran family / search rate limits | main.rs:675,682-684 |
| 10 s / 24 h | `FLUSH_INTERVAL_SECS` / `MAX_ATTEMPT_WINDOW` | rate_limit_store.rs:17, store.rs:13 |

---

## 3. Drift vs `docs/quran.md` (verified)

Items where the code disagrees with the plan doc. Severity in parens.

1. **(high) SW contract duplication is over-stated.** `docs/quran.md §72` says the SW↔client contract is *duplicated* (SW-inline + `lib/offline/*`) because SvelteKit "forbids relative imports" in the SW. In reality the message types + channel names are **shared** via the SW's relative import `./lib/offline/messages` (relative imports work). Only `META_DB`/`META_STORE` (`service-worker.ts:21-22` vs `lib/offline/meta.ts:4-5`) and `normalizeDataKey` (`service-worker.ts:35-44` vs `lib/offline/keys.ts`) are genuinely duplicated — and *those two* are the real drift hazards the doc should point at instead.

2. **(med) "Install verifies the complete new cache" is false for the shell route.** `§72` claims atomic all-or-nothing install. `SHELL_ROUTE` (`${base}/404.html`) precache is `.catch(()=>null)` (service-worker.ts:159) — a missing/failing shell is silently skipped and the terminal navigation fallback (service-worker.ts:341-342) then returns `Response.error()` instead of an offline page.

3. **(med) The search ETag embeds a SHA-256 digest.** `§16` ("sha never names an ETag") and `§63` ("never a digest") are literally contradicted: `controller.rs:1163` computes `hex::encode(&sha2::Sha256::digest(norm_q.as_bytes())[..8])` and threads it into the search canonical_key. This is a digest over **user input** (the query string), *not* Quran data — so it honors the no-sha-on-Quran-*data* intent — but the doc's absolute wording is broader than the intent and should carve out this exception explicitly. *(Verified by hand, `controller.rs:1163-1170`.)*

4. **(low) Translation byte-bound is a separate prune pass, not a unified LRU.** `§3` table says "eviction: LRU, whichever bound trips first" as if both are inside moka. Moka enforces only the count ceiling (`max_capacity`); the byte ceiling is enforced by a serialized post-build `enforce_byte_bound` loop, with transient overshoot bounded by `BUILD_CONCURRENCY=2`.

### Rejected drift (false alarms caught by the critic)

- **`/scripts` is NOT a dead API.** The URL is already `{public_url}/tanzil/arabic/{filename}` (`controller.rs:810`, verified) — the publisher layout `docs/quran.md §1` prescribes, in the *done* section. When R2 serves the files, `/scripts` returns 2 verified artifacts and `resolveManifest` can return `source:'api'`.
- **`/health/quran` vs `/quran/health/ready` are two different endpoints** (web SSR disk-cache stats vs Rust pool stats). No drift.
- **`bench/README.md` "sampled"** describes the benchmark's data-collection side-channel, not server header emission. No drift.

---

## 4. Known gaps & weak spots (snapshot, not plan)

Curated from the readers + critic, all code-grounded. Listed so a future diff can show what got addressed.

**Web offline:**
- `eq-data-v1` (`__data.json`) cache is **unbounded** — no eviction but SWR. `PAGES_MAX=300` applies only to `eq-pages`. Long-lived installs with route churn accumulate entries until a version bump.
- No time-based TTL anywhere in the SW; freshness depends entirely on SWR + `runMaintenance` rescan. If the maintenance cursor stalls (e.g. IDB unavailable mid-loop → cursor never advances), stale `__data.json` persists until next version.
- `eq-pages` `recency` map is one IDB value rewritten wholesale on every touch/trim — read-modify-write with no cross-tab lock; concurrent tabs can lose recency updates (last-writer-wins).
- `skipWaiting` is gated entirely on a user click; no max-wait auto-activation.
- Offline fallback is the SvelteKit-generated `404.html`, not a dedicated "you are offline" page.
- Offline pack `packId == appVersion` → every deploy re-stages the full ~7 MiB / 1308-entry pack even if no route content changed.
- `downloadBytes` has **no timeout/abort/retry**; a hung CDN connection blocks fire-and-forget `ensureTranslation`.
- No exponential backoff/jitter anywhere; engagement retry is hard-capped at 1 per source per tab.
- `verifyBytes` is size-only; a byte-accurate corrupt download of exactly `spec.sizeBytes` passes. OPFS `put` is non-atomic (crash mid-write → corrupt file, recovered by next size-check).
- No quota awareness; OPFS caps are fixed constants regardless of `navigator.storage.estimate()`.
- `localizeDeliveryUrls` silently preserves a remote `downloadUrl` for a translation id with no baked match — a drifted `/sources` response could inject an arbitrary cross-origin URL (the `/_quran` gateway allowlist is the only backstop).

**Rust hot-db:**
- `hit_rate` = `1 - builds/lookups` conflates "no traffic" with "perfect" and uses builds (cold loads), not moka's hit/miss counters.
- Pool catalogue/whitelist frozen at boot; adding a translation needs a restart (acceptable under the immutable-DB rule, but undocumented as an ops constraint).
- No "byte-bound vs count-bound" breakdown in `PoolStats` — `evictions` aggregates both.
- `resident_bytes` (atomic) and moka `entry_count` are independently maintained; could diverge on a moka version change (byte-bound test guards the happy path only).
- `Vary` is hard-coded to only `Accept-Encoding`; Arabic varies by `?script` and translations by source — cross-script/source correctness rests entirely on the ETag differing per `canonical_key`. A CDN that ignores ETag and keys on URL+Vary could cross-serve the wrong script.
- Only `If-None-Match` is honored; `If-Modified-Since` is CORS-allowed but never read (dead exposed header).
- `respond_cached` and `respond_cached_with_etag` are ~95% duplicated.
- Rate-limit dedup claims map has **no periodic prune** — long-TTL distinct keys can accumulate unbounded if never released.
- L2 restore always sets `block_scope=Temp` (schema stores `block_until_at` but not scope) → Long abuse blocks downgrade to Temp on restart (correct expiry, wrong scope).
- Attempt-only buckets aren't snapshotted → in-progress abuse chains lost on restart.
- `/quran/health/ready` shares the 600/60s `quran-v1` bucket with real reader traffic.
- No HTTP `/metrics` text endpoint; limiter counters leave only via OTLP push.

---

## 5. How to use this doc later

- **To see what changed:** `git diff b17324d..HEAD -- docs/caching-architecture.md` shows doc edits; re-run the same audit (10 readers + critic) and diff the constants tables (§2) and the drift/gap lists (§3, §4) to see what shifted in code.
- **To verify a claim:** every constant carries a `file:line`; every subsystem section names its files. The two highest-stakes claims (`/scripts` URL, search-ETag digest) were hand-verified at `controller.rs:810` and `controller.rs:1163` against this commit.
- **What is explicitly NOT here:** the auth/session cache (SQLite sessions, outside `/quran`), FCM push delivery mechanics, the owner-profile SSR cache, and mobile clients (none in this repo). Those are adjacent but out of the two-domain scope.
