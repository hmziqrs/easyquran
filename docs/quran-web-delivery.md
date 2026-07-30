# EasyQuran — Quran content delivery to the web (design plan)

> Status: **plan** (not yet implemented). Scope: a server-rendered, SEO-indexable Qur'an reader
> with a **live Rust backend** serving content/search/translations, plus a front-end that **caches
> the Arabic database permanently** and runs **full offline parity** (search, jump-to-ayah/juz/page,
> ranges) against a local SQL engine. **Translations are live-only** (never cached, never offline).
> This document is **self-contained**.
>
> Decisions settled in review: **SSG + live Rust backend** for the MVP; **Arabic-only permanent
> cache** (translations loaded live); **one combined Service Worker at `/`**.

---

## 1. Goals & requirements

1. **SEO / server-rendered.** Every `/app/<slug>` page ships as real HTML with the Uthmani verse
   text in the DOM — indexable without relying on crawler JS execution, best-possible LCP.
2. **Permanent client cache of the Arabic database.** The Qur'an text is immutable, so the Arabic db
   is downloaded **once** and kept **forever** (no expiry, no periodic re-sync).

   This is safe only because the *text* is frozen — but the **artifact** is not: `quran-api.md` §8.1
   bumps `contentVersion` when the metadata XML, the slug table, or the derived schema changes, and
   any of those produces a new database. So "no invalidation" means *no polling*, not *no version
   check*. The db therefore carries a `meta` table with its `contentVersion`, and the client compares
   it against `GET /quran/v1/version` on load (cheap, `max-age=300`); a mismatch re-downloads. A
   cached database with no readable version marker cannot know it is stale, which is the one failure
   mode this requirement must not have.
3. **Full offline feature parity for Arabic.** Offline, the front-end does everything the backend
   can for Arabic: **full-text search**, jump-to-ayah, jump-to-juz, jump-to-page, ranges — all
   against the cached db.

   One documented exception: **search results are not identical online and offline.** This db uses
   FTS5 token matching; the backend MVP uses a substring scan over a normalized corpus
   (`quran-api.md` §7.3). Feature parity holds; result parity does not, and a user whose client
   switches sources mid-session can see results change. Accepted for now — see `quran-api.md` §11.6.
4. **Translations are live-only.** Translations are fetched from the backend on demand and are
   **never cached or available offline**. (Deliberate product decision.)
5. **SPA-fast after hydration.** Client-side navigation, prefetched on hover.
6. **Contract preserved.** The `Surah` / `VerseKey` interface and pure helpers in
   `web/src/lib/data/quran.ts` stay; the `_reader` components are not edited for the core cutover.

---

## 2. The decision, and why

### Why a SQL engine in the browser (WASM SQLite)

Requirement #3 — **offline parity**, which needs **full-text search *and* relational queries**
(juz/page ranges, ayah lookups) together — rules out the lighter options:

- **IndexedDB** — native, WASM-free, but **no FTS, no relational queries**. Cannot serve "jump to
  juz 5" or `MATCH`-style search without rebuilding a query engine on top of it.
- **A JS search library** (MiniSearch/FlexSearch) — great for offline *text* search, but **no
  relational side** (juz/page/range, joins).
- **Web SQL** — deprecated (2010), removed everywhere (~2023); **no native SQL db in browsers**.

WASM SQLite is the only option giving FTS **and** relational queries offline. It costs a ~1 MB WASM
blob + the db in memory, but that's paid for by #3 and lands off the critical path (§6).

### Why a live backend *and* a client cache

- **Backend (live):** the source of truth for content/search/**translations**, and the before-cache
  fallback on first visit. Required by #4 (translations live) and by online search.
- **Client OPFS cache (Arabic only):** delivers #2 (permanent) and #3 (offline parity). Once cached,
  Arabic reads/searches are local and instant; offline works.
- **SSG prerender:** delivers #1 (SEO) and first paint.

### Verified facts that shape the design

| Fact (verified on disk) | Consequence |
|---|---|
| `quran-data.xml` is **metadata only** (114 sura rows + juz/page/ruku/sajda; **no verse text**). | Build reads the XML for catalog/ranges and a `.sqlite` for verse text. |
| Verse text in `db/quran/tanzil/arabic/quran-uthmani.sqlite` (1.6 MB, 6236 rows, `quran_text("index",sura,aya,text)`). | Primary content source for the canonical db. |
| `quran-simple-clean.sqlite` (908 KB) = same verses **undiacriticated, regular alef**. | The FTS/search corpus (§9). **Uthmani carries the basmala prefix on 110 surahs; simple-clean on 112** (9 absent in both; 95 & 97 absent in Uthmani only). |
| Web **hardcodes 11 surahs** in `quran.ts`, **never fetches backend**; `/app` is currently `noindex`, excluded from sitemap, no per-page meta. | No live path to disrupt; SEO (#1) needs an explicit policy flip + per-surah meta (Phase 1). |
| Rust backend is a CMS clone with **no Qur'an tables/routes**. | Backend gains a Qur'an module reading the new `quran.sqlite` (not `easyquran.db`). |
| Deploy is `@sveltejs/adapter-static` + `prerender=true`; Node **v24** (built-in `node:sqlite`, FTS5 verified). | SSG is the SEO mechanism; build reads the db with zero deps. |

---

## 3. Architecture

```
BUILD (once)
  Tanzil ──► build-quran-sqlite.mjs (node:sqlite, zero deps, --experimental-sqlite)
    sources: quran-uthmani.sqlite + quran-simple-clean.sqlite + quran-data.xml
    output:  ONE canonical quran.sqlite  (ARABIC ONLY — no translations)
               quran_text (Uthmani)   quran_simple (simple-clean)   surahs (114 meta)
               juzs / pages / rukus / sajdas (ranges)   quran_fts (FTS5 over simple-clean)

SERVER (Rust backend — LIVE in MVP)
  opens quran.sqlite read-only ──►
    (a) SSG: build prerenders 114 /app/<slug> HTML (verses in DOM → SEO) [reads db directly]
    (b) LIVE JSON: surah/ayah content, FTS search, and translations (translations never cached)
    (c) serves quran.sqlite to the client (one-time download)

CLIENT
  first paint : prerendered HTML shows verses (SEO + UX)
  first visit : before the Arabic db caches, SPA nav/search fall back to the live backend JSON
                background: fetch sqlite-wasm (~1 MB) + quran.sqlite (~2–3 MB) → persist to OPFS
  subsequent  : Arabic reads/search/juz/page run locally on the cached db → instant, OFFLINE
  translations: always fetched LIVE from the backend, never cached, never offline

CACHING
  OPFS              the Arabic quran.sqlite (persistent; best-effort — versioned re-download on miss)
  Service Worker    ONE combined SW at / : content cache + importScripts(Firebase Messaging)
                    excludes /firebase-messaging-sw.js + /firebase-config.js from its cache
```

| Layer | Technology | Job |
|---|---|---|
| SEO / first paint | SvelteKit **prerender** (SSG), reading `quran.sqlite` at build | verses in DOM for crawlers |
| Live content/search/translations | **Rust backend** (JSON) | runtime source of truth + before-cache fallback + translations |
| Local query engine | **sqlite-wasm** (FTS5 + OPFS VFS) in a Web Worker | offline Arabic read/search/ranges |
| Persistent storage | **OPFS** + `navigator.storage.persist()` (+ versioned re-download) | permanent Arabic db cache |
| Asset caching | **one combined Service Worker at `/`** | wasm + db + HTML + shell; relays FCM |

---

## 4. The canonical `quran.sqlite` (Arabic only)

Built by `web/scripts/build-quran-sqlite.mjs` (Node, `node:sqlite`, no native deps), run before
`vite build` via a `prebuild` hook. Run with `NODE_OPTIONS=--experimental-sqlite` (defensively;
`node:sqlite` is still flagged on some v24 builds). The same file is opened by the Rust backend and
served to the client. **Translations are not in this file** — they're served live from the backend.

**Schema:**

```sql
PRAGMA user_version = 1;   -- schema shape; checked at open

-- so a cached db can tell whether it is stale (§1.2)
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
-- rows: content_version, schema_version, built_at, uthmani_digest, simple_clean_digest

CREATE TABLE quran_text  ("index" INTEGER PRIMARY KEY, sura INTEGER, aya INTEGER, text TEXT);
CREATE INDEX idx_quran_text_sura ON quran_text(sura, aya);
CREATE TABLE quran_simple ("index" INTEGER PRIMARY KEY, text TEXT);
-- derived search index (normalization only); quran_text/quran_simple stay verbatim
CREATE TABLE quran_search ("index" INTEGER PRIMARY KEY, text TEXT);
CREATE TABLE surahs (num INTEGER PRIMARY KEY, slug TEXT UNIQUE NOT NULL, name TEXT, tname TEXT,
                     ename TEXT, place TEXT, ayah_count INTEGER, revelation_order INTEGER, rukus INTEGER,
                     bismillah TEXT NOT NULL CHECK (bismillah IN ('first-ayah','none','embedded-prefix')));
CREATE TABLE juzs  (num INTEGER PRIMARY KEY, start_index INTEGER, end_index INTEGER, start_sura INTEGER, start_aya INTEGER);
CREATE TABLE pages (num INTEGER PRIMARY KEY, start_index INTEGER, end_index INTEGER, start_sura INTEGER, start_aya INTEGER);
-- rukus / sajdas analogous
CREATE VIRTUAL TABLE quran_fts USING fts5(text, content='quran_search', content_rowid='index');
```

**Build invariants (assert at build time, fail loud):**

- `COUNT(*) = 6236` for `quran_text`, `quran_simple`, `quran_search`, and `quran_fts`; `"index"`
  contiguous 1..6236.
- **Verbatim digests match the API's golden constants** (`quran-api.md` §3.3): `sha256` over all
  6,236 `quran_text` / `quran_simple` values joined by `\n` equals `32cc746d…` / `375934722…`. This
  is what makes "one source of truth, no drift between the SEO HTML and the offline experience"
  checkable rather than aspirational — and it catches a silent NFC normalization by `node:sqlite`,
  which would otherwise alter 5,782 of 6,236 Uthmani rows invisibly.
- `meta.content_version` is present and equals the version the backend reports.
- **FTS must be populated explicitly** — external-content FTS does **not** auto-fill; `MATCH` returns
  `[]` until rebuilt. Mandatory: `INSERT INTO quran_fts(quran_fts) VALUES('rebuild')`, then assert
  `count(*) = 6236`. (Forgetting this ships a silently-empty search.)
- The 30 juz ranges and 604 page ranges each **tile `[1, 6236]`** exactly. The XML gives `(sura,aya)`
  anchors and 0-based `sura/@start`; derive `start_index = suras[sura].start_global + (aya-1)` and
  `end_index = next.start_index - 1` (last → 6236); assert no gaps/overlaps.

**Bismillah handling (verified against the files).** Only **surah 9** is genuinely basmala-free —
this is **intentional** (At-Tawbah is the one surah without a basmala; never add one); surah 1's
basmala *is* ayah 1 (keep it). The other **112 surahs carry a basmala prefix on ayah 1** —
but it is **not byte-identical**: 110 use the standard Uthmani `بِسْمِ ٱللَّهِ…`, while **surahs 95 &
97 use a `بِّسْمِ…` variant** (a shadda on the first beh), so an exact `startsWith('بِسْمِ…')` match
**misses them** (verified: Uthmani ayah-1 = 114 rows → 111 `بِسْمِ`, 2 `بِّسْمِ`, 1 none).

Every table stores the source bytes exactly as they are. `surahs.bismillah`
(`first-ayah` | `none` | `embedded-prefix`) records which case each surah is, for display purposes
only. Assert at build time that exactly **112** rows classify as `embedded-prefix`, using the
diacritic-insensitive match so 95/97 are not missed — a data-integrity check on the source.

**Search** indexes `quran_search`, a derived column carrying only the normalization search itself
requires (harakat removed, alef/ya folding), matching `quran-api.md` §7.1. `quran_text` and
`quran_simple` are untouched.

**Slugs.** A committed 114-entry `num→slug` map (`web/scripts/slugs.mjs`) is the single source for
both the `surahs.slug` column **and** `entries()` in `[surah]/+page.ts` (which must stay synchronous
and feed the prerender). The 11 live slugs are a verbatim subset; the other 103 are hand-authored
once and frozen (the XML `tname` cannot be normalized to the live slugs).

---

## 5. Server — live backend + SSG for SEO

The Rust backend gains a Qur'an module opening `quran.sqlite` read-only. Two jobs:

- **SSG (build-time, for SEO).** `+page.server.ts` `load` reads `quran.sqlite` directly via
  `node:sqlite` (backend need not be running during `vite build`) → verses baked into the 114
  prerendered `/app/<slug>` HTML files. **SEO policy flip (Phase 1 exit):** `/app` moves from
  `noindex` → indexable; add per-surah `<title>`/description/canonical + a Qur'an JSON-LD node
  (derived from the `surahs` row, not hardcoded), extend `entries()` + sitemap to all 114, enable
  adapter `precompress`.
- **LIVE JSON (runtime).** Endpoints serve: surah/ayah content, **FTS search** (over simple-clean),
  and **translations** (live, never cached). Used as the before-cache SPA-nav fallback on first
  visit, for online search, and for all translation loading.

> Crawlers don't run client WASM, so **prerendered HTML is the SEO surface**. The live backend + the
> client OPFS cache are the SPA/offline experience layered on top.

---

## 6. Client — WASM SQLite on OPFS (Arabic) + live translations

**Engine:** official **`sqlite-wasm`** (ships FTS5 + OPFS VFS). OPFS sync access handles are only
available **inside a Web Worker**, so the engine runs in a dedicated Worker driven via the sqlite-wasm
**Promiser** (async-to-main-thread) API.

**Persistence (Arabic db):**

- Store `quran.sqlite` in **OPFS**; call `navigator.storage.persist()` on first gesture.
- OPFS "permanent" is **best-effort** (eviction under pressure is still possible; `persist()` is
  near-no-op on Safari). So: keep a **version pragma** in the db; on load, if OPFS is missing/stale,
  transparently **re-download** from the backend. The backend-served file is the canonical fallback.

**Lifecycle / UX:**

- **First paint:** prerendered HTML (no WASM needed).
- **Before cache (first visit):** SPA navigation/search fall back to the **live backend JSON**.
  Background: fetch `sqlite-wasm` + `quran.sqlite`, write to OPFS, init the Worker.
- **After cache:** Arabic reads/search/juz/page run **locally → instant, offline**.
- **Translations:** always `fetch` from the backend on demand; never written to OPFS; unavailable
  offline. (Loading state + graceful "online only" treatment.)

**The sync/async seam (no hydration flash):**

- **Serialize the prerendered verses into client state** (`page.data`) so the first client render
  matches the DOM byte-for-byte — **never re-query the already-painted surah** from WASM.
- Keep a **per-open-surah sync verse cache in `reader.svelte.ts`**, populated when the page load
  resolves, so the store's synchronous getters (`verseText`, `copyVerse`, `bookmarkList.text`,
  `durationFor`) keep working with no WASM round-trip and no component edits.
- The WASM db (and backend) serve **navigation to *other* surahs + search + juz/page/ranges** only.
  Async loads are guarded by a **version token** (captured at call time) so a stale response from a
  rapid back/forward can't overwrite the current surah. (`$derived` can't be async — fill `$state`
  via a guarded `$effect`.)

**Feature surface (offline, Arabic, against the local db):**

- Open surah: `SELECT text FROM quran_text WHERE sura=? ORDER BY aya`.
- Jump to ayah: `WHERE sura=? AND aya=?`; `?verse=N` scrolls into view.
- Jump to juz/page: `WHERE "index" BETWEEN ?start AND ?end`.
- Search: `WHERE quran_fts MATCH ?` (§9).

---

## 7. Offline, caching, and the cost

- **OPFS** holds the Arabic `quran.sqlite` permanently (best-effort; versioned re-download on miss).
- **One combined Service Worker at `/`** — the single controller. It does content caching **and**
  `importScripts` the Firebase Messaging compat SDK (what `firebase-messaging-sw.js` already does).
  It must **network-first / exclude** `/firebase-messaging-sw.js` and `/firebase-config.js` so it
  can't stale-cache the FCM worker. After reload, assert `navigator.serviceWorker.controller` is
  this SW (not merely that a registration exists). Fix the stale comment at
  `web/src/lib/firebase/messaging.ts:11` (references a nonexistent `src/service-worker.ts`).
- **Translations are explicitly not cached** — offline = Arabic only.
- **WASM cost is off the critical path:** first paint is prerendered HTML; the ~1 MB engine + ~2–3 MB
  db download in the background, once, then never again.

---

## 8. The seam — `web/src/lib/data/quran.ts`

`quran.ts` stays the contract + pure helpers. Changes are internal:

- The hardcoded 11-surah `SURAHS` array is replaced by the **in-memory 114-row metadata** (bundled
  from `slugs.mjs` + the `surahs` table) so `Sidebar`, `adjacentSurahs`, `surahMeta`, and the
  reader-store name lookups stay **synchronous**, and `entries()` stays sync/build-time.
- Add `ayahCount: number` to `Surah` (additive); `surahMeta` reads `s.ayahCount`.
- Verse text reaches components via **prerender data + the per-surah sync cache** (§6), not via the
  old inline array. Search/juz/page/translations are async (WASM/backend).
- **Untouched:** `verseKey`, `parseKey`, `toArabicDigits`, `BISMILLAH`, `showsBismillah`,
  `surahPath`, `slugFor`, and the `_reader` components for the core cutover (`Results` becomes async
  only when search ships).

---

## 9. Search

**Offline (after cache):** FTS5 `MATCH` over the **simple-clean** corpus in the local db. Indexing
simple-clean (not Uthmani) is essential — Uthmani's harakat + alef-wasla (`ٱ`) give near-zero recall.
Normalize the query the same way before `MATCH`. Exact-token/prefix only (no stemming) for v1.

**Online (before cache, or as source of truth):** the backend runs the same FTS5 over the same
`quran_simple`/`quran_fts`. Same query shape, same normalization — the client just chooses local-db
(offline/cached) vs backend (online/before-cache).

---

## 10. Translations — live only

Translations are fetched from the backend on demand and **never cached or available offline**:

- The reader requests the active translation's text for the open surah from the backend; it is
  rendered alongside the Arabic and discarded (not written to OPFS).
- Offline, the reader shows **Arabic only**.
- Per-language FTS search over translations, if ever wanted, is a backend feature (Phase 5) — never
  offline.

---

## 11. Phased plan

Each phase ships independently; nothing breaks the current 11-surah site until the cutover.

**Phase 0 — Canonical `quran.sqlite` (Arabic, no behavior change).**
`build-quran-sqlite.mjs` (+ `slugs.mjs`): emits `quran.sqlite` with all tables + FTS (with the
mandatory `rebuild`), asserts invariants (6236 rows, juz/page tiling, verbatim digests, 112 embedded-prefix rows,
absent-set logged). `prebuild` hook + `--experimental-sqlite`.
*Phase-0 exit checklist:* juz/page ranges tile `[1,6236]` verified; bismillah classification asserted (112 rows);
FTS `rebuild` enforced + asserted; `--experimental-sqlite` set; `quran.sqlite` exists (committed or
CI-built); 114-slug map frozen and feeding `entries()`.

**Phase 1 — Backend Qur'an module + SEO.**
Rust opens `quran.sqlite` read-only; live JSON for content/search; SvelteKit prerenders all 114
`/app/<slug>` from the db. SEO flip: `noindex`→index, per-surah title/description/JSON-LD, sitemap,
`precompress`.
*Exit:* 114 indexable HTML pages with verse text; content + search endpoints live.

**Phase 2 — Client WASM SQLite (Arabic) + core read parity.**
Wire `sqlite-wasm` Worker + OPFS + versioned re-download + `persist()`; serialize prerender verses
into client state (no flash); per-surah sync cache in the store; before-cache fallback to backend
JSON; one-line `+page.svelte` change to consume `data`; swap `quran.ts` internals (§8).
*Exit:* reader renders all 114 surahs; second visit is instant + offline (Arabic).

**Phase 3 — One combined Service Worker + SPA polish.**
Combined SW at `/` (content cache + `importScripts` FCM); exclude FCM routes; assert controller;
`data-sveltekit-preload-data`.
*Exit:* offline Arabic navigation works; FCM still delivers.

**Phase 4 — Offline search + juz/page navigation.**
FTS5 search UI (async `Results.svelte`, query normalization); jump-to-juz / jump-to-page from range
queries; online↔offline search handoff.
*Exit:* offline search returns correct hits; juz/page browsing offline.

**Phase 5 — Live translations.**
Backend translation endpoints; reader fetches the active translation live (never cached); RTL.

---

## 12. Open items / to confirm

1. **Commit `quran.sqlite` in-repo vs generate in CI** — sources are already tracked, so the saving
   is ~2–3 MB. Lean: commit for reproducible, decoupled builds.
2. **Author the 103 new slugs + 114 display names** (`slugs.mjs`) — hand-authored in the live style,
   11 frozen as a subset. (~30 min; the frozen map feeds `entries()`.)
3. **node:sqlite flag** — set `NODE_OPTIONS=--experimental-sqlite` defensively; pin Node ≥24.
4. **sqlite-wasm distribution variant** (EH vs ESM, shared memory) + a wasm/`quran.sqlite` size
   budget in CI — pick in Phase 2 once the real sizes are measured.
