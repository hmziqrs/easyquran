# EasyQuran — Quran content delivery to the web (design plan)

> Status: **plan** (not yet implemented). Scope: a server-rendered, SEO-indexable Qur'an reader
> with a **live Rust backend** serving content/search/translations, plus a front-end that **caches
> a derived Arabic database permanently** and runs the reader-facing Arabic surface offline against
> a local SQL engine. **Translations are live-only for this web reader** (never persisted, never
> available offline).
> This document owns web delivery decisions; `quran-api.md` remains authoritative for Rust runtime
> behavior and the shared wire/artifact contract.
>
> Decisions settled in review: **SSG + live Rust backend** for the MVP; **Arabic-only permanent
> cache** (translations loaded live); **one combined Service Worker at `/`**.

---

## 1. Goals & requirements

1. **SEO / server-rendered.** Every `/app/<slug>` page ships as real HTML with the Uthmani verse
   text in the DOM — indexable without relying on crawler JS execution, best-possible LCP.
2. **Permanent client cache of the Arabic database.** The Qur'an text is immutable, so the Arabic db
   is downloaded **once** and kept **forever** (no expiry, no periodic re-sync).

   This is safe only because every published artifact is immutable — but a new content, schema,
   normalizer, or builder version produces a new artifact. So "no invalidation" means *no polling*,
   not *no version check*. The db carries its full offline version tuple in `meta`; the client
   compares `meta.artifact_version` with
   `GET /quran/v1/version → data.offlineDatabase.artifactVersion` on load (cheap,
   `max-age=300`). A mismatch re-downloads the new URL advertised by `/scripts`. Comparing only
   `contentVersion` is insufficient because an offline schema or search-index change need not alter
   Quran source bytes.
3. **Offline parity for the reader-facing Arabic surface.** Offline, the front-end supports surah
   and ayah reads, arbitrary ayah batches/global ranges, juz/page/ruku/hizb-quarter/manzil ranges,
   sajda markers, deterministic ayah-of-the-day, both Arabic scripts, and search. Operational API
   resources such as health, OpenAPI, version, and artifact manifests are naturally online-only.

   One documented exception: **search results are not identical online and offline.** This db uses
   FTS5 token matching; the backend MVP uses a substring scan over a normalized corpus
   (`quran-api.md` §7.3). Feature parity holds; result parity does not, and a user whose client
   switches sources mid-session can see results change. Accepted for now — see `quran-api.md` §7.3.
4. **Translations are live-only.** Translations are fetched from the backend on demand and are
   sent with `cache: "no-store"`, bypass the Service Worker, and are never written to Cache Storage
   or OPFS. The public API may still publish translation packs for other clients; this web reader
   does not consume them.
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
| `quran-data.xml` is **metadata only** (114 sura rows + all navigation markers; **no verse text**). | Build reads the XML for catalog/ranges and a `.sqlite` for verse text. |
| Verse text in `db/quran/tanzil/arabic/quran-uthmani.sqlite` (1.6 MB, 6236 rows, `quran_text("index",sura,aya,text)`). | Primary content source for the derived offline db. |
| `quran-simple-clean.sqlite` (908 KB) = same verses **undiacriticated, regular alef**. | The FTS/search corpus (§9). Both scripts contain embedded prefixes for 112 surahs; Uthmani has 110 standard spellings plus the shadda variants at 95:1 and 97:1. Only surah 9 is prefix-free. |
| Web **hardcodes 11 surahs** in `quran.ts`, **never fetches backend**; `/app` is currently `noindex`, excluded from sitemap, no per-page meta. | No live path to disrupt; SEO (#1) needs an explicit policy flip + per-surah meta (Phase 1). |
| Rust backend is a CMS clone with **no Qur'an tables/routes**. | The API plan adds a module that loads the two raw Arabic databases + XML + canonical slugs into memory. It never opens the combined browser database. |
| Deploy is `@sveltejs/adapter-static` + `prerender=true`; Node **v24** (built-in `node:sqlite`, FTS5 verified). | SSG is the SEO mechanism; build reads the db with zero deps. |

---

## 3. Architecture

```
SHARED INPUTS
  Tanzil Arabic sqlite files + quran-data.xml + db/quran/slugs.json
  db/quran/content-manifest.json fixes input SHA-256 values + contentVersion

WEB/OFFLINE BUILD (once)
  shared inputs ──► build-quran-sqlite.mjs (node:sqlite, --experimental-sqlite)
    output:  ONE derived quran.sqlite  (ARABIC ONLY — no translations)
               quran_text (Uthmani)   quran_simple (simple-clean)   surahs (114 meta)
               all 5 range families + sajdas             quran_fts (FTS5 over simple-clean)
             + offline-artifact manifest (version tuple, size, sha256, CDN URL)
  SSG reads quran.sqlite directly; publisher uploads it to an immutable CDN key

SERVER (Rust backend — LIVE in MVP)
  opens the 2 raw Arabic sqlite files + XML + slugs at startup, builds an in-memory store,
  closes SQLite, and then ──►
    (a) LIVE JSON: content + normalized substring search + live translations
    (b) /scripts and /version advertise the offline manifest
    (c) never opens or proxies the combined quran.sqlite

CLIENT
  first paint : prerendered HTML shows verses (SEO + UX)
  first visit : before the Arabic db caches, SPA nav/search fall back to the live backend JSON
                background: fetch sqlite-wasm + quran.sqlite directly from CDN → persist to OPFS
  subsequent  : reader-facing Arabic reads/search/navigation run locally → instant, OFFLINE
  translations: always fetched LIVE with cache bypass, never persisted, never offline

CACHING
  OPFS              the Arabic quran.sqlite (persistent; best-effort — versioned re-download on miss)
  Service Worker    ONE combined SW at / : content cache + importScripts(Firebase Messaging)
                    excludes FCM config, offline DB downloads, and translation-bearing requests
```

| Layer | Technology | Job |
|---|---|---|
| SEO / first paint | SvelteKit **prerender** (SSG), reading `quran.sqlite` at build | verses in DOM for crawlers |
| Live content/search/translations | **Rust backend** (in-memory JSON) | runtime source of truth + before-cache fallback + substring search + translations |
| Local query engine | **sqlite-wasm** (FTS5 + OPFS VFS) in a Web Worker | offline Arabic read/search/ranges |
| Persistent storage | **OPFS** + `navigator.storage.persist()` (+ versioned re-download) | permanent Arabic db cache |
| Artifact delivery | **S3/CDN**, advertised by Axum | immutable combined database; Axum never proxies it |
| Asset caching | **one combined Service Worker at `/`** | wasm + HTML + shell; relays FCM; does not duplicate the OPFS database |

---

## 4. The derived offline `quran.sqlite` (Arabic only)

Built by `web/scripts/build-quran-sqlite.mjs` (Node, `node:sqlite`, no native deps), run before
`vite build` via a `prebuild` hook. Run with `NODE_OPTIONS=--experimental-sqlite` (defensively;
`node:sqlite` is still flagged on some v24 builds). SSG reads this file directly and the publisher
uploads it to the immutable CDN key from its generated manifest. Axum reads that manifest but never
opens or proxies the database. **Translations are not in this file** — they're served live from the
backend.

The source of truth remains the two raw Arabic databases, the XML, and
`db/quran/slugs.json`. Before building, Node verifies their SHA-256 values against
`db/quran/content-manifest.json` and copies its canonical `contentVersion`.

**Schema:**

```sql
PRAGMA user_version = 1;   -- offline schema version; checked at open

-- so a cached db can identify both its content and derived representation
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
-- rows: schema_kind='easyquran-offline', content_version, artifact_version,
--       schema_version, search_version, builder_version,
--       uthmani_digest, simple_clean_digest

CREATE TABLE quran_text  ("index" INTEGER PRIMARY KEY, sura INTEGER, aya INTEGER, text TEXT);
CREATE INDEX idx_quran_text_sura ON quran_text(sura, aya);
CREATE TABLE quran_simple ("index" INTEGER PRIMARY KEY, text TEXT);
-- derived search index (normalization only); quran_text/quran_simple stay verbatim
CREATE TABLE quran_search ("index" INTEGER PRIMARY KEY, text TEXT);
CREATE TABLE surahs (num INTEGER PRIMARY KEY, slug TEXT UNIQUE NOT NULL, name TEXT, tname TEXT,
                     ename TEXT, place TEXT, ayah_count INTEGER, revelation_order INTEGER, rukus INTEGER,
                     bismillah TEXT NOT NULL CHECK (bismillah IN ('first-ayah','none','embedded-prefix')));
CREATE TABLE juzs (
    num INTEGER PRIMARY KEY, start_index INTEGER, end_index INTEGER,
    start_sura INTEGER, start_aya INTEGER, end_sura INTEGER, end_aya INTEGER
);
CREATE TABLE pages (
    num INTEGER PRIMARY KEY, start_index INTEGER, end_index INTEGER,
    start_sura INTEGER, start_aya INTEGER, end_sura INTEGER, end_aya INTEGER
);
CREATE TABLE rukus (
    num INTEGER PRIMARY KEY, start_index INTEGER, end_index INTEGER,
    start_sura INTEGER, start_aya INTEGER, end_sura INTEGER, end_aya INTEGER
);
CREATE TABLE hizb_quarters (
    num INTEGER PRIMARY KEY, hizb INTEGER, quarter_in_hizb INTEGER,
    start_index INTEGER, end_index INTEGER,
    start_sura INTEGER, start_aya INTEGER, end_sura INTEGER, end_aya INTEGER
);
CREATE TABLE manzils (
    num INTEGER PRIMARY KEY, start_index INTEGER, end_index INTEGER,
    start_sura INTEGER, start_aya INTEGER, end_sura INTEGER, end_aya INTEGER
);
CREATE TABLE sajdas (
    num INTEGER PRIMARY KEY, global_index INTEGER UNIQUE,
    sura INTEGER, aya INTEGER, kind TEXT
);
CREATE VIRTUAL TABLE quran_fts USING fts5(text, content='quran_search', content_rowid='index');
```

`PRAGMA user_version = 1` does not collide with translation pack schema version
1 because SQLite versions are file-local and
`meta.schema_kind` rejects the wrong artifact type.

**Build invariants (assert at build time, fail loud):**

- The four input SHA-256 values match `db/quran/content-manifest.json`; copy its
  `contentVersion` rather than independently reimplementing or hand-typing the
  backend's BLAKE3 result.
- `COUNT(*) = 6236` for `quran_text`, `quran_simple`, `quran_search`, and `quran_fts`; `"index"`
  contiguous 1..6236.
- **Verbatim digests match the API's golden constants** (`quran-api.md` §3.3): `sha256` over all
  6,236 `quran_text` / `quran_simple` values joined by `\n` equals `32cc746d…` / `375934722…`. This
  is what makes "one source of truth, no drift between the SEO HTML and the offline experience"
  checkable rather than aspirational — and it catches a silent NFC normalization by `node:sqlite`,
  which would otherwise alter 5,782 of 6,236 Uthmani rows invisibly.
- `meta` contains the exact content/artifact/schema/search/builder tuple from the
  generated offline manifest. `artifactVersion` follows the formula in
  `quran-api.md` §5.1; the client compares this value, not only `contentVersion`.
- **FTS must be populated explicitly** — external-content FTS does **not** auto-fill; `MATCH` returns
  `[]` until rebuilt. Mandatory: `INSERT INTO quran_fts(quran_fts) VALUES('rebuild')`, then assert
  `count(*) = 6236`. (Forgetting this ships a silently-empty search.)
- All five range families have the API counts 30 / 604 / 556 / 240 / 7 and each
  **tiles `[1, 6236]`** exactly; there are 15 correctly mapped sajda markers. The XML gives `(sura,aya)`
  anchors and 0-based `sura/@start`; derive `start_index = suras[sura].start_global + (aya-1)` and
  `end_index = next.start_index - 1` (last → 6236); assert no gaps/overlaps.
- Build in global-index order, set `journal_mode=DELETE`, run `VACUUM` and
  `integrity_check`, emit no WAL/SHM sidecars, and store no build timestamp. Two
  clean builds for the same version tuple must have the same final SHA-256.
- Generate the artifact manifest only after the database closes; record its
  identity-encoded `sizeBytes`, final-byte `sha256`, immutable CDN URL, and full
  version tuple.

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
requires (harakat removed, alef/ya folding), matching the versioned fixtures and `searchVersion`
from `quran-api.md` §7.1. `quran_text` and `quran_simple` are untouched.

**Slugs.** The committed 114-entry `db/quran/slugs.json` is the single authored source for the
`surahs.slug` column, the backend, and `entries()` in `[surah]/+page.ts`. If synchronous web code
needs `web/scripts/slugs.mjs`, generate it from the JSON and fail CI when it differs. The 11 live
slugs are a verbatim subset; the other 103 are hand-authored once and frozen (the XML `tname`
cannot be normalized to the live slugs).

---

## 5. Server — live backend + SSG for SEO

Backend implementation is owned by `quran-api.md`: Rust loads the two raw
Arabic databases, XML, and canonical slug JSON at startup, constructs an
immutable in-memory store, and closes SQLite before serving traffic. It never
opens the derived offline database. The two delivery jobs are:

- **SSG (build-time, for SEO).** `+page.server.ts` `load` reads `quran.sqlite` directly via
  `node:sqlite` (backend need not be running during `vite build`) → verses baked into the 114
  prerendered `/app/<slug>` HTML files. **SEO policy flip (Phase 1 exit):** `/app` moves from
  `noindex` → indexable; add per-surah `<title>`/description/canonical + a Qur'an JSON-LD node
  (derived from the `surahs` row, not hardcoded), extend `entries()` + sitemap to all 114, enable
  adapter `precompress`.
- **LIVE JSON (runtime).** `/quran/v1` serves surah/ayah content from memory,
  normalized **substring search**, and translations. It also advertises the
  derived database manifest through `/scripts` and `/version`; the browser
  downloads the database bytes directly from the manifest's CDN URL.

> Crawlers don't run client WASM, so **prerendered HTML is the SEO surface**. The live backend + the
> client OPFS cache are the SPA/offline experience layered on top. Reading the
> derived database at SSG time is intentional: the build verifies the same
> source digests and content manifest as the backend, without requiring a live
> API during `vite build`.

---

## 6. Client — WASM SQLite on OPFS (Arabic) + live translations

**Engine:** official **`sqlite-wasm`** (ships FTS5 + OPFS VFS). OPFS sync access handles are only
available **inside a Web Worker**, so the engine runs in a dedicated Worker driven via the sqlite-wasm
**Promiser** (async-to-main-thread) API.

**Persistence (Arabic db):**

- Store `quran.sqlite` in **OPFS**; call `navigator.storage.persist()` on first gesture.
- OPFS "permanent" is **best-effort** (eviction under pressure is still possible; `persist()` is
  near-no-op on Safari). On load, validate `PRAGMA user_version`, `meta.schema_kind`, and
  `meta.artifact_version` against `/quran/v1/version`. If OPFS is missing, corrupt, wrong-kind, or
  stale, fetch `/quran/v1/scripts`, then download the advertised immutable URL **directly from the
  CDN** with HTTP-cache bypass and write it to OPFS. Validate identity `Content-Length`/`sizeBytes`
  and final-byte SHA-256 before promotion. Axum never serves the database body.

**Lifecycle / UX:**

- **First paint:** prerendered HTML (no WASM needed).
- **Before cache (first visit):** SPA navigation/search fall back to the **live backend JSON**.
  Background: fetch `sqlite-wasm` plus the CDN database, verify it, write it to OPFS, and initialize
  the Worker.
- **After cache:** reader-facing Arabic reads/search/navigation run **locally → instant, offline**.
- **Translations:** always `fetch` from the backend on demand with `{ cache: "no-store" }`; never
  written to OPFS or Cache Storage; unavailable offline. (Loading state + graceful "online only"
  treatment.)

**The sync/async seam (no hydration flash):**

- **Serialize the prerendered verses into client state** (`page.data`) so the first client render
  matches the DOM byte-for-byte — **never re-query the already-painted surah** from WASM.
- Keep a **per-open-surah sync verse cache in `reader.svelte.ts`**, populated when the page load
  resolves, so the store's synchronous getters (`verseText`, `copyVerse`, `bookmarkList.text`,
  `durationFor`) keep working with no WASM round-trip and no component edits.
- The WASM db (and backend) serve **navigation to other surahs, search, and reader-facing Arabic
  ranges/markers** only.
  Async loads are guarded by a **version token** (captured at call time) so a stale response from a
  rapid back/forward can't overwrite the current surah. (`$derived` can't be async — fill `$state`
  via a guarded `$effect`.)

**Live API adapter.** The fallback client consumes the API envelope's `data`
field and the settled camelCase `/surahs` + `/ayahs` wire (`surah`, `ayah`,
`ayahCount`). SQL's upstream `sura` / `aya` column names stay inside the Worker.
The adapter handles `404`, validation `400`, `429`, and retryable `5xx` bodies
using the API's closed error shape; it never treats a failed response as an
empty surah.

**Feature surface (offline, Arabic, against the local db):**

- Open surah and choose script: query `quran_text` or `quran_simple` in ayah order.
- Jump to ayah, arbitrary verse-key batch, and inclusive global ranges.
- Jump to juz, page, ruku, hizb-quarter, or manzil using the corresponding
  range table; expose the same first/last/global-index metadata as the API.
- Resolve sajda markers locally.
- Compute `/random` locally with the API's frozen UTC-date constants.
- Search: `WHERE quran_fts MATCH ?` (§9).

The 300-ayah HTTP cap does not complicate the first-visit reader fallback:
every complete surah fits, with a maximum of 286 ayahs.

---

## 7. Offline, caching, and the cost

- **OPFS** holds the Arabic `quran.sqlite` permanently (best-effort; versioned re-download on miss).
- **One combined Service Worker at `/`** — the single controller. It does content caching **and**
  `importScripts` the Firebase Messaging compat SDK (what `firebase-messaging-sw.js` already does).
  It must **network-first / exclude** `/firebase-messaging-sw.js` and `/firebase-config.js` so it
  can't stale-cache the FCM worker. After reload, assert `navigator.serviceWorker.controller` is
  this SW (not merely that a registration exists). Fix the stale comment at
  `web/src/lib/firebase/messaging.ts:11` (references a nonexistent `src/service-worker.ts`).
- Cache only the same-origin shell, HTML, JS/CSS, and WASM. Pass all
  `/quran/v1/**` requests through without Cache Storage, and never cache the CDN
  `quran.sqlite`; OPFS is its sole persistent copy.
- **Translations are explicitly not cached** — bypass
  `/quran/v1/translations/**`, any Arabic request carrying `translations=`, and
  translation-pack CDN URLs. Translation fetches also set `cache: "no-store"`.
  Offline therefore means Arabic only.
- **WASM cost is off the critical path:** first paint is prerendered HTML; the ~1 MB engine + ~2–3 MB
  db download in the background, once, then never again.

---

## 8. The seam — `web/src/lib/data/quran.ts`

`quran.ts` stays the contract + pure helpers. Changes are internal:

- The hardcoded 11-surah `SURAHS` array is replaced by the **in-memory 114-row metadata** (bundled
  from canonical `db/quran/slugs.json`—optionally through a generated `slugs.mjs`—plus the
  `surahs` table) so `Sidebar`, `adjacentSurahs`, `surahMeta`, and the reader-store name lookups stay
  **synchronous**, and `entries()` stays sync/build-time.
- Add `ayahCount: number` to `Surah` (additive); `surahMeta` reads `s.ayahCount`.
- Verse text reaches components via **prerender data + the per-surah sync cache** (§6), not via the
  old inline array. Search/navigation/translations are async (WASM/backend).
- **Untouched:** `verseKey`, `parseKey`, `toArabicDigits`, `BISMILLAH`, `showsBismillah`,
  `surahPath`, `slugFor`, and the `_reader` components for the core cutover (`Results` becomes async
  only when search ships).

---

## 9. Search

**Offline (after cache):** FTS5 `MATCH` over the **simple-clean** corpus in the local db. Indexing
simple-clean (not Uthmani) is essential — Uthmani's harakat + alef-wasla (`ٱ`) give near-zero recall.
Normalize the query with the same versioned fixtures as the API before `MATCH`. The Worker escapes
FTS operators and deliberately exposes exact-token/prefix semantics only (no arbitrary FTS syntax or
stemming) for v1.

**Online (before cache):** the backend performs the API plan's normalized **substring scan** over
its in-memory simple-clean corpus. It does not query FTS5 or open the offline database.

The UI applies the API's normalized-query length and result limits to both
sources, orders results by `globalIndex`, and adapts each source into one view
model. API results include UTF-16 highlight offsets; offline results may return
an empty highlight list because FTS offsets into normalized simple-clean cannot
be applied directly to verbatim Uthmani text. Switching sources may change the
result set because substring and token/prefix semantics are intentionally
different. `searchVersion` guarantees shared normalization, not identical match
semantics.

---

## 10. Translations — live only

Translations are fetched from the backend on demand and **never persisted or available offline in
the EasyQuran web reader**:

- The reader requests the active translation's text for the open surah from the backend; it is
  rendered alongside the Arabic and discarded. Requests use `cache: "no-store"` and the Service
  Worker bypass rules in §7.
- Offline, the reader shows **Arabic only**.
- The API's downloadable translation packs serve other clients. This reader neither requests nor
  stores them.
- Per-language FTS search over translations, if ever wanted, is a backend feature (Phase 5) — never
  offline.

---

## 11. Phased plan

Each phase ships independently; nothing breaks the current 11-surah site until the cutover.

**Phase 0 — Derived offline `quran.sqlite` (Arabic, no behavior change).**
`build-quran-sqlite.mjs` consumes the shared content manifest and canonical slug JSON; emits all
offline tables + populated FTS; proves the five range families, 15 sajdas, verbatim digests,
bismillah classification, schema kind/version, deterministic rebuild, and integrity; then generates
and publishes the offline artifact manifest and immutable CDN object.
*Phase-0 exit checklist:* all build gates in §4 pass; `--experimental-sqlite` is set; the CDN HEAD
matches identity `sizeBytes`; `db/quran/slugs.json` feeds generated web metadata and `entries()`; the
API can validate and advertise the manifest without opening the database.

**Phase 1 — API contract integration + SEO.**
Backend implementation is tracked exclusively by `quran-api.md`; this web phase consumes its frozen
`/surahs` + `/ayahs` OpenAPI contract and `/scripts` manifest. SvelteKit prerenders all 114
`/app/<slug>` pages from the derived database. SEO flip: `noindex`→index, per-surah
title/description/JSON-LD, sitemap, `precompress`.
*Exit:* 114 indexable HTML pages contain verbatim verse text; the generated/mock client and live API
agree on envelopes, wire names, errors, and artifact metadata.

**Phase 2 — Client WASM SQLite (Arabic) + core read parity.**
Wire `sqlite-wasm` Worker + OPFS + manifest/SHA-verified CDN download + `persist()`; validate the
full artifact/schema/search tuple; serialize prerender verses
into client state (no flash); per-surah sync cache in the store; before-cache fallback to backend
JSON; one-line `+page.svelte` change to consume `data`; swap `quran.ts` internals (§8).
*Exit:* reader renders all 114 surahs; second visit is instant + offline (Arabic).

**Phase 3 — One combined Service Worker + SPA polish.**
Combined SW at `/` (shell cache + `importScripts` FCM); exclude FCM routes, all API requests, the
offline database, and translation-bearing traffic; assert controller; `data-sveltekit-preload-data`.
*Exit:* offline Arabic navigation works; FCM still delivers.

**Phase 4 — Offline search + complete Arabic navigation.**
FTS5 search UI (async `Results.svelte`, versioned normalization and source adapter); add all
juz/page/ruku/hizb-quarter/manzil/sajda/batch/global-range surfaces and the deterministic random
ayah; implement the documented online-substring ↔ offline-FTS handoff.
*Exit:* the complete reader-facing Arabic surface works offline and the fixed divergence query set
matches `quran-api.md` §7.3.

**Phase 5 — Live translations.**
After API Phase 4 freezes the translation DTOs, the reader fetches the active translation live with
cache bypass, never requests pack downloads, and handles offline Arabic-only mode; RTL.

---

## 12. Release/version handshake

1. Build and test the deterministic database, upload its never-overwritten CDN object, and generate
   the final manifest.
2. Deploy the API with that manifest. Boot verifies source/content/search versions and the CDN HEAD
   before `/scripts` advertises it.
3. Build and deploy SSG from the same database and manifest; embed `contentVersion` and
   `artifactVersion` in page data.
4. At hydration, compare the embedded, API, and OPFS versions. Never open an unsupported
   `schemaVersion`. During a rolling-deploy mismatch, keep the already-painted surah internally
   consistent, use one selected source/version for subsequent navigation, and request a reload once
   the matching web build is available rather than mixing verse sets.
5. Retain supported old immutable artifacts so clients on an older web bundle can continue or fall
   back to live JSON safely.

---

## 13. Open items / to confirm

1. **Author the 103 new slugs + 114 display names** in `db/quran/slugs.json` — hand-authored in the
   live style, with the existing 11 frozen. Generate any MJS/TS form from it.
2. **sqlite-wasm distribution variant** (EH vs ESM, shared memory) + a wasm/`quran.sqlite` size
   budget in CI — pick in Phase 2 once the real sizes are measured.

Settled: generate and publish `quran.sqlite` in CI rather than committing it; pin Node ≥24 and set
`NODE_OPTIONS=--experimental-sqlite` defensively.
