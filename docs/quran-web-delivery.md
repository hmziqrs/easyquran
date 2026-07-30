# EasyQuran — Quran content delivery to the web (design plan)

> Status: **plan** (not yet implemented). Scope: a server-rendered, SEO-indexable Qur'an reader
> whose front-end caches the **entire Qur'an database** and runs **every read feature offline** —
> search, jump-to-ayah, jump-to-juz, jump-to-page, ranges — against a local SQL engine. No runtime
> backend is required for reads after the one-time cache; the backend's role is server-side
> rendering (for SEO) and serving the database file.
>
> This document is **self-contained**.

---

## 1. Goals & requirements

1. **SEO / server-rendered.** Every `/app/<slug>` page ships as real HTML with the Uthmani verse
   text in the DOM — indexable by Google/Bing/social/AI crawlers without relying on their JS
   rendering queue, and best-possible LCP / Core Web Vitals.
2. **Permanent client cache of the full database.** The Qur'an is immutable, so the database is
   downloaded **once** and kept **forever** (no expiry, no re-sync, no invalidation logic).
3. **Full offline feature parity with the backend.** The offline front-end can do everything the
   backend can: **full-text search**, jump-to-ayah, jump-to-juz, jump-to-page, and range queries —
   all running locally against the cached database.
4. **SPA-fast after hydration.** Client-side navigation, no full page reloads, prefetched on hover.
5. **Contract preserved.** The `Surah` / `VerseKey` interface and pure helpers in
   `web/src/lib/data/quran.ts` stay; the `_reader` components are not edited for the core cutover.

---

## 2. The decision, and why

### Why a SQL engine in the browser (WASM SQLite)

The deciding requirement is **#3 — offline feature parity**, which includes both **full-text
search** *and* **relational queries** (juz/page ranges, ayah lookups). That combination rules out
the lighter options:

- **IndexedDB** — native and WASM-free, but it is a NoSQL key/value store with **no full-text
  search** and **no relational queries**. It cannot serve "jump to juz 5" or `MATCH`-style search
  without us rebuilding a query engine on top of it.
- **A JS search library** (MiniSearch/FlexSearch) — excellent for offline *text* search, but it
  cannot do the relational side (juz/page/range navigation, joins).
- **Web SQL** — deprecated (2010) and removed from all browsers (~2023); there is **no native SQL
  database in the browser** anymore.

WASM SQLite is the only option that delivers FTS **and** relational queries offline. It is a real
cost (a ~1 MB WASM blob, plus the database, held in memory), but it is **paid for** by requirement
#3 — and because the database is cached permanently and the reading path is server-rendered, the
cost lands off the critical path (see §7).

### Why one canonical database

There is exactly **one** `quran.sqlite`, built once from the Tanzil sources. The **backend opens it
read-only** (for SSR + serving the file); the **client downloads it once and queries it locally**.
One source of truth means no data duplication, no per-surah JSON chunks, and no drift between the
SEO HTML and the offline experience.

### Verified facts that shape the design

| Fact (verified on disk) | Consequence |
|---|---|
| `quran-data.xml` is **metadata only** — 114 sura rows + juz/page/ruku/sajda; **zero verse text**. | The build reads the XML for the catalog/ranges and a `.sqlite` for verse text. |
| Verse text lives in `db/quran/tanzil/arabic/quran-uthmani.sqlite` (1.6 MB, 6236 rows, `quran_text("index", sura, aya, text)`). | Primary content source for the canonical db. |
| `quran-simple-clean.sqlite` (908 KB) is the same verses **undiacriticated, regular alef**. | The right corpus to index for search (see §9). |
| The web **hardcodes 11 surahs** in `quran.ts` and **never fetches the backend**. | There is no live content path to disrupt. |
| The Rust backend (`rust/data/easyquran.db`) is a CMS clone with **no Qur'an tables** and **no Qur'an routes**. | The backend must gain a Qur'an module; reads are served from the new `quran.sqlite`, not `easyquran.db`. |
| Deploy is `@sveltejs/adapter-static` with `prerender = true`. | SSG is already the model; extending it to 114 surahs needs no infra change. |
| Node is **v24** → built-in `node:sqlite`. | The build reads the Tanzil `.sqlite` with **zero new dependencies**. |

---

## 3. Architecture

```
BUILD (once)
  Tanzil  ──► build-quran-sqlite.mjs (node:sqlite, zero deps)
    sources: quran-uthmani.sqlite  +  quran-simple-clean.sqlite  +  quran-data.xml
    output:  ONE canonical  quran.sqlite
               quran_text (verses)        surahs (114 meta)
               juzs / pages / rukus / sajdas (ranges)
               quran_fts (FTS5 over simple-clean)

SERVER (Rust backend)
  opens quran.sqlite read-only  ──►
    (a) SSR at build time  ──►  114 prerendered HTML pages (verses in DOM → SEO)
    (b) serves quran.sqlite to the client  (one-time download)
    (c) later: online search + future dynamic features

CLIENT
  first load:   prerendered HTML shows verses instantly (SEO + first paint)
                background: fetch sqlite-wasm (~1 MB / ~400 KB gz) + quran.sqlite (~2–3 MB)
                            → persist to OPFS (on-disk, permanent)
  second load+: wasm + db loaded from OPFS → instant, fully offline
                ALL features run locally: search (FTS), jump-to-ayah/juz/page, ranges

CACHING
  OPFS               the quran.sqlite itself (persistent, survives reload, never re-downloaded)
  navigator.storage  .persist() → mark storage non-evictable
  Service Worker     caches sqlite-wasm, the db file, prerendered HTML, app shell
```

| Layer | Technology | Job |
|---|---|---|
| SEO / first paint | SvelteKit **prerender** (SSG), sourcing from `quran.sqlite` | verses in the DOM for crawlers |
| Local query engine | **sqlite-wasm** (official; ships FTS5 + OPFS VFS) | all offline reads + search |
| Persistent storage | **OPFS** (on-disk) + `navigator.storage.persist()` | permanent db cache |
| Asset caching | Service Worker | wasm + db file + HTML + shell |

---

## 4. The canonical `quran.sqlite`

A single file, built by `web/scripts/build-quran-sqlite.mjs` (Node, `node:sqlite`, no native deps),
run before `vite build` via a `prebuild` hook. The same file is opened by the Rust backend and
served to the client.

**Schema:**

```sql
-- verses (Uthmani)
CREATE TABLE quran_text (
  "index" INTEGER PRIMARY KEY,        -- global ayah id, contiguous 1..6236
  sura INTEGER NOT NULL,              -- 1..114
  aya  INTEGER NOT NULL,              -- 1..N
  text TEXT NOT NULL
);
CREATE INDEX idx_quran_text_sura ON quran_text(sura, aya);

-- simple-clean text (undiacriticated, regular alef) — the search corpus
CREATE TABLE quran_simple (
  "index" INTEGER PRIMARY KEY,
  text TEXT NOT NULL
);

-- surah metadata
CREATE TABLE surahs (
  num INTEGER PRIMARY KEY,            -- 1..114
  slug TEXT UNIQUE NOT NULL,          -- deep-link slug (see Slugs)
  name TEXT NOT NULL,                 -- Arabic
  tname TEXT NOT NULL,                -- transliterated
  ename TEXT,                         -- English meaning
  place TEXT NOT NULL,                -- 'Meccan' | 'Medinan'
  ayah_count INTEGER NOT NULL,
  revelation_order INTEGER,
  rukus INTEGER
);

-- ranges (global-index bounds; tile [1,6236] with no gaps — assert at build)
CREATE TABLE juzs  (num INTEGER PRIMARY KEY, start_index INTEGER, end_index INTEGER,
                    start_sura INTEGER, start_aya INTEGER);
CREATE TABLE pages (num INTEGER PRIMARY KEY, start_index INTEGER, end_index INTEGER,
                    start_sura INTEGER, start_aya INTEGER);
-- rukus / sajdas analogous

-- offline full-text search over simple-clean
CREATE VIRTUAL TABLE quran_fts USING fts5(text, content='quran_simple', content_rowid='index');
```

**Build invariants (assert at build time, fail loud):**

- `COUNT(*) = 6236` for both `quran_text` and `quran_simple`; `"index"` contiguous 1..6236.
- The 30 juz ranges and 604 page ranges each **tile `[1, 6236]`** exactly (no gaps/overlaps).
- `quran_fts` row count == 6236.

**Bismillah handling (build-time).** Tanzil embeds the basmala as a **byte-identical prefix** on
ayah-1 of 112 surahs; surah 1 ayah 1 *is* the basmala; surah 9 has none. The web renders a separate
basmala header, so the prefix is **stripped when building `quran_text`** (for ayah 1, sura ∉ {1,9})
to avoid doubling it. One assertion: the prefix is present on exactly those 112 surahs.

**Slugs.** Ship a committed 114-entry `num→slug` map (`web/scripts/slugs.mjs`). The 11 already-live
slugs (`al-fatihah`, `al-baqarah`, `al-asr`, `al-fil`, `quraysh`, `al-kawthar`, `al-kafirun`,
`an-nasr`, `al-ikhlas`, `al-falaq`, `an-nas`) are a verbatim subset; the other 103 are authored
once and frozen. The XML `tname` (`Al-Baqara`) cannot be normalized to them.

---

## 5. Server / SSR for SEO

The Rust backend gains a Qur'an module that opens `quran.sqlite` read-only. Its content-read job is
**server-side rendering**, not a runtime read API for the SPA:

- **SSG (recommended).** SvelteKit prerenders all 114 `/app/<slug>` pages at build time. The
  `load` reads verse data from `quran.sqlite` (the build may read the file directly via `node:sqlite`
  or call the backend) and the verses are baked into the static HTML. Served from the CDN; the
  backend need not be running at runtime. This matches the current `adapter-static` deploy.
- **Request-time SSR (optional).** A live backend renders HTML per request (`adapter-node` /
  `adapter-cloudflare`). Heavier; only if dynamic server rendering is ever required.

The backend additionally **serves `quran.sqlite`** as a static asset (e.g. `/quran.sqlite` or via
the CDN/R2) so the client can download it once.

> Crawlers do not run the client WASM SQLite, so **the prerendered HTML is the SEO surface** — it
> must contain the verse text. The client WASM db is the SPA/offline experience layered on top.

---

## 6. Client — WASM SQLite on OPFS

**Engine:** the official **`sqlite-wasm`** build. It ships **FTS5** (so offline search works) and an
**OPFS VFS** (so the database persists on-disk across reloads without re-downloading or
re-initializing). This is the modern, supported way to run SQLite in the browser.

**Persistence:**

- Store `quran.sqlite` in **OPFS** (Origin Private File System) — true on-disk persistence.
- Call `navigator.storage.persist()` on first user gesture so the browser marks the storage
  **non-evictable** (satisfies "saved permanently").
- The WASM binary and the initial db file are fetched once and Service-Worker-cached.

**Lifecycle / UX:**

- **First visit:** prerendered HTML renders verses instantly (SEO + UX). In the background, the app
  fetches `sqlite-wasm` + `quran.sqlite`, writes the db to OPFS, and initializes the engine. Until
  the db is ready, client-side navigation/search **falls back to the backend** (online) or the
  prerendered HTML.
- **Second visit onward:** wasm + db load from OPFS — **instant, fully offline**, all features
  local.

**Feature surface (all offline, against the local db):**

- Open surah: `SELECT text FROM quran_text WHERE sura=? ORDER BY aya`.
- Jump to ayah: `WHERE sura=? AND aya=?`; deep-link `?verse=N` scrolls into view.
- Jump to juz / page: `WHERE "index" BETWEEN ?start AND ?end` (a PK range scan).
- Search: `WHERE quran_fts MATCH ?` (see §9).

**Async seam.** SQLite queries are async, so verse-text lookups become async after hydration. To
keep the synchronous contract for cheap things, **load the 114-row `surahs` table into memory once
the engine is ready** — `surahBySlug`, `surahByNum`, `adjacentSurahs`, `surahMeta` then stay
synchronous; only verse text, ranges, and search go through async queries. The `_reader` components
keep receiving a `Surah`-shaped prop.

---

## 7. Offline, caching, and the cost

- **OPFS** holds `quran.sqlite` permanently (the "immutable → cache forever" requirement).
- **Service Worker** cache-first: `sqlite-wasm`, `quran.sqlite`, the prerendered `/app/<slug>` HTML,
  and the app shell; stale-while-revalidate for JS/CSS/fonts.
- **The WASM cost is off the critical path:** first paint is the prerendered HTML (no WASM needed);
  the ~1 MB engine + db download in the background, once, then never again. Search/navigation that
  needs the db simply waits for (or falls back online until) the one-time cache completes.

**Two service workers at root scope.** The new content SW coexists with the classic
`web/static/firebase-messaging-sw.js`. Firebase owns its own registration, so the content SW
registration (in `+layout.svelte` `onMount`, browser-only, idempotent) must use a **separately
versioned cache name** so one never disturbs the other. Verify
`navigator.serviceWorker.getRegistrations()` shows two after a hard reload.

---

## 8. The seam — `web/src/lib/data/quran.ts`

`quran.ts` stays the contract + pure helpers. Changes are internal:

- The hardcoded 11-surah `SURAHS` array is replaced by the **in-memory 114-row metadata** loaded
  from the local db once ready (with a small bundled fallback catalog so the sidebar renders before
  the db loads). `Sidebar`, `adjacentSurahs`, `surahMeta`, and reader-store name lookups keep
  working synchronously.
- Add `ayahCount: number` to `Surah` (additive); `surahMeta` reads `s.ayahCount`.
- Verse text, juz/page, and search move behind async queries against the local db.
- **Untouched:** `verseKey`, `parseKey`, `toArabicDigits`, `BISMILLAH`, `showsBismillah`,
  `surahPath`, `slugFor`. The `_reader` components (`SurahReader`, `VerseRow`, `Sidebar`, `Results`)
  are not edited for the core cutover; `Results` becomes async only when search ships (§9).

---

## 9. Search

Offline FTS5 over the **simple-clean** corpus (`quran_fts`). Indexing simple-clean (not Uthmani) is
essential: Uthmani carries harakat and uses alef-wasla (`ٱ`, U+0671) which never matches regular
alef (`ا`, U+0627), so an FTS index over Uthmani has near-zero recall. Simple-clean is already
undiacriticated with regular alef, so tokenization works. The user query is normalized the same way
before `MATCH`. This remains **exact-token** matching (prefix/contains), not root/stem — acceptable
for v1; stemming can come later.

Online search (backend) is a future enhancement behind `PUBLIC_API_BASE_URL`, used as the
pre-cache fallback and for any ranked/advanced search.

---

## 10. Phased plan

Each phase ships independently; nothing breaks the current 11-surah site until the cutover.

**Phase 0 — Canonical `quran.sqlite` (no behavior change).**
`web/scripts/build-quran-sqlite.mjs` (+ `slugs.mjs`): reads the Tanzil sources, emits `quran.sqlite`
with all tables + FTS, asserts the invariants (6236 rows, juz/page tiling, bismillah prefix).
Add `"quran:db": "node scripts/build-quran-sqlite.mjs"` + `prebuild` hook.
*Exit:* `quran.sqlite` builds, opens read-only, all queries return correct data.

**Phase 1 — Backend Qur'an module + SSR.**
Rust opens `quran.sqlite` read-only; serves the file to the client; SvelteKit prerenders all 114
`/app/<slug>` pages from it (verses in HTML → SEO). Per-page `<title>`/description, JSON-LD,
sitemap, `precompress`.
*Exit:* 114 indexable HTML pages with full verse text; `quran.sqlite` downloadable.

**Phase 2 — Client WASM SQLite + core read parity.**
Wire `sqlite-wasm` + OPFS; persist with `navigator.storage.persist()`; hydrate the reader from the
local db (open surah, jump-to-ayah); fall back to backend/prerender before the db is cached. Swap
`quran.ts` internals (§8); one-line change in `[surah]/+page.svelte` to consume async data.
*Exit:* reader renders all 114 surahs from the local db; second visit is instant + offline.

**Phase 3 — Caching + SPA polish.**
Service Worker (cache wasm + db + HTML + shell); two-SW coexistence; `data-sveltekit-preload-data`;
confirm `getRegistrations()` shows two SWs.
*Exit:* offline navigation between opened surahs works; Lighthouse SEO pass.

**Phase 4 — Offline search + juz/page navigation.**
FTS5 search UI (async `Results.svelte`); jump-to-juz / jump-to-page controls backed by range
queries; query normalization.
*Exit:* offline search returns correct hits; juz/page browsing works offline.

**Phase 5 — Translations + online features.**
Add translation tables/FTS to `quran.sqlite`; translation picker; online search/sync behind
`PUBLIC_API_BASE_URL`.

---

## 11. Open questions / decisions

1. **SSG vs request-time SSR** — recommend **SSG** (build-time prerender, static CDN, backend not
   needed at runtime). Confirm.
2. **WASM engine** — recommend official **`sqlite-wasm`** (FTS5 + OPFS VFS in one). Alternatives:
   `WA-SQLite`, `absurd-sql` (IndexedDB-backed).
3. **OPFS browser support** — solid in Chrome/Edge (102+), Safari (17+), Firefox (111+). Fallback
   for older browsers: IndexedDB-backed engine, or online-only (backend).
4. **Before-cache fallback** — while the db downloads on first visit, SPA nav/search falls back to
   the backend (online) or prerendered HTML. Confirm acceptable.
5. **Slugs** — confirm the transliteration rule for the 103 new slugs (the 11 live ones are frozen).
6. **Commit `quran.sqlite` in repo vs generate in CI** — lean: generate in CI (and cache the
   artifact) to keep the repo small; commit only if reproducible-offline builds matter more.
7. **License** — Tanzil is **non-commercial**; fine while not monetized. Revisit the moment
   billing/accounts go live, and render a tanzil.net attribution in the reader footer.
