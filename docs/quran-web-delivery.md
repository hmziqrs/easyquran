# EasyQuran — Quran content delivery to the web (design plan)

> Status: **plan** (not yet implemented). Scope: a server-rendered,
> SEO-indexable Qur'an reader with a live Rust backend and an offline-capable
> Arabic reader.
>
> The browser caches the two existing Arabic databases unchanged:
> `quran-uthmani.sqlite` and `quran-simple-clean.sqlite`. Translations are
> live-only for this web reader. **No additional Arabic SQLite database is
> built, generated, or maintained.**
>
> This document owns web delivery decisions. `quran-api.md` owns Rust runtime
> behavior and the shared HTTP contract.

---

## 1. Goals and requirements

1. **SEO / server-rendered.** Every `/app/<slug>` page ships as real HTML with
   the Uthmani verse text in the DOM.
2. **Reuse the existing Arabic databases.**
   `quran-uthmani.sqlite` and `quran-simple-clean.sqlite` are downloaded from
   the immutable CDN URLs advertised by `/quran/v1/scripts` and stored in an
   OPFS directory named by the backend's `contentVersion`. The files are not
   altered and require no `meta` table.
3. **Offline Arabic-reader parity.** Surah and ayah reads, arbitrary ayah
   batches and global ranges, juz/page/ruku/hizb-quarter/manzil navigation,
   sajda markers, deterministic ayah-of-the-day, both Arabic scripts, and
   Arabic search work offline after the two databases are cached.
4. **The same search behavior online and offline.** Both sides normalize and
   substring-scan the 6,236 simple-clean verses. No FTS table or search database
   is required.
5. **Translations are live-only.** Translation requests use
   `cache: "no-store"` and are never written to OPFS or Cache Storage. The
   public API may publish translation packs for other clients; this web reader
   does not download them.
6. **SPA-fast after hydration.** Client navigation is prefetched and local once
   the Arabic files are ready.
7. **Preserve the reader boundary.** The `VerseKey` contract, pure routing
   helpers, and component-facing loaded-surah shape in
   `web/src/lib/data/quran.ts` remain stable.

Operational resources such as health, OpenAPI, and version metadata remain
online-only.

---

## 2. Why the existing databases are enough

Both existing files already have the schema needed for verse reads:

```sql
CREATE TABLE quran_text (
    "index" INTEGER PRIMARY KEY,
    sura    INTEGER NOT NULL DEFAULT 0,
    aya     INTEGER NOT NULL DEFAULT 0,
    text    TEXT NOT NULL
);

CREATE INDEX idx_quran_text_sura_aya ON quran_text (sura, aya);
```

Both contain 6,236 rows with the same contiguous global indices and the same
`(sura, aya)` keys.

- `quran-uthmani.sqlite` is the display source. Its text is used by Axum, SSG,
  browser reading, copying, and SEO HTML.
- `quran-simple-clean.sqlite` is the search source and the optional
  simple-clean display script. Its smaller, undiacritated text is loaded into a
  small in-memory array and substring-scanned.
- `quran-data.xml` supplies surah metadata and the navigation markers. The
  backend parses it at startup; the web build parses it into the JavaScript
  bundle. It is not another runtime database.
- `web/src/lib/data/quran.ts` owns URL slugs. The web maps a slug to the API's
  numeric surah identifier.

There is no need to merge the files. There is also no need to add FTS to either
file: the complete simple-clean text corpus is about 744 KB, so a normalized
substring scan is small enough for both Rust and a browser Worker.

---

## 3. Architecture

```text
EXISTING, AUTHORITATIVE INPUTS
  db/quran/tanzil/arabic/quran-uthmani.sqlite
  db/quran/tanzil/arabic/quran-simple-clean.sqlite
  db/quran/tanzil/quran-data.xml

BACKEND
  opens both SQLite files + XML at startup
    └── builds an immutable in-memory store
          ├── verbatim Arabic reads
          ├── normalized simple-clean substring search
          └── metadata/range reads
  closes SQLite before serving
  /scripts advertises the same two existing files on the CDN

SSG
  opens quran-uthmani.sqlite directly with node:sqlite
  parses XML and uses quran.ts slug metadata
    └── prerenders 114 SEO pages

BROWSER
  first paint ──► prerendered Uthmani HTML
  before cache ──► live backend fallback
  background   ──► download the same two SQLite files from CDN to OPFS
  after cache  ──► local reads + local substring search
  translations ──► live backend only
```

| Layer | Technology | Job |
|---|---|---|
| SEO / first paint | SvelteKit SSG + `node:sqlite` | Read the existing Uthmani database and emit verse HTML |
| Live content and search | Rust backend, in memory | Runtime source and before-cache fallback |
| Local Arabic reads | `sqlite-wasm` in a Web Worker | Open the two existing databases unchanged |
| Local Arabic search | Worker in-memory array | Normalize and substring-scan simple-clean rows |
| Navigation metadata | XML parsed at backend startup and web build | Surah/range/sajda metadata without another database |
| Persistent storage | OPFS | Keep both existing databases under `contentVersion` |
| Artifact delivery | S3/CDN, advertised by Axum | Serve the two existing immutable files; Axum does not proxy them |
| Asset caching | One Service Worker at `/` | Cache the app shell and WASM; leave SQLite to OPFS |

---

## 4. Source use and validation

The web build and backend consume the checked-in source files directly. There
is no database builder and no generated SQLite artifact.

Build and CI assert:

- each database contains exactly 6,236 rows;
- global indices are contiguous from 1 through 6,236;
- both databases have identical ordered `(index, sura, aya)` keys;
- the verbatim text digests equal the golden constants in
  `quran-api.md` §3.3;
- XML counts are 114 surahs, 30 juzs, 604 pages, 556 rukus, 240 quarters,
  7 manzils, and 15 sajdas;
- each range family tiles `[1, 6236]` without a gap or overlap;
- Uthmani bismillah classification is exactly one first-ayah, 112 embedded
  prefixes, and one absent; the shadda variants at 95:1 and 97:1 are preserved;
- the CDN identity-encoded byte length and SHA-256 for each file match what
  `/quran/v1/scripts` advertises.

The existing databases deliberately have no internal EasyQuran version field.
Their enclosing `contentVersion` is computed by the backend from:

```text
BLAKE3(uthmani bytes || simple-clean bytes || XML bytes)[0..16 hex]
```

The browser uses that value as the OPFS directory name and validates each
download with its advertised `sizeBytes` and `sha256`. When `contentVersion`
changes, it downloads both current source files into a new directory before
promoting it.

---

## 5. Server and SSG

Backend implementation is owned by `quran-api.md`. Rust reads both existing
Arabic databases and the XML at startup, builds an immutable in-memory store,
and closes its SQLite handles. HTTP requests do not query SQLite.

The two web-facing jobs are:

- **SSG for SEO.** `+page.server.ts` reads
  `db/quran/tanzil/arabic/quran-uthmani.sqlite` directly through
  `node:sqlite`. It prerenders all 114 `/app/<slug>` pages without requiring a
  running backend during `vite build`. The web build parses `quran-data.xml`
  for metadata and uses the 114 slug mappings in the existing
  `web/src/lib/data/quran.ts`.
- **Live JSON.** `/quran/v1` serves Arabic reads, range metadata, normalized
  substring search, and live translations. `/quran/v1/scripts` advertises the
  immutable CDN URLs, sizes, and checksums for the two existing Arabic files.

The SEO cutover removes `/app` from `noindex`, adds per-surah title,
description, canonical URL, and JSON-LD, includes all 114 pages in `entries()`
and the sitemap, and enables adapter precompression.

Crawlers do not depend on client WASM. Prerendered Uthmani HTML is the SEO and
first-paint surface; live/backend and OPFS data take over after hydration.

---

## 6. Browser data flow

### 6.1 SQLite engine

Use `sqlite-wasm` inside a dedicated Web Worker because OPFS synchronous access
handles are Worker-only. The Worker opens the two source databases read-only:

- Uthmani for the normal reader;
- simple-clean when that script is requested and to initialize search.

The databases retain their existing `quran_text` schema. Queries stay simple:

```sql
-- one surah
SELECT "index", sura, aya, text
FROM quran_text
WHERE sura = ?
ORDER BY aya;

-- search corpus initialization, from simple-clean
SELECT "index", sura, aya, text
FROM quran_text
ORDER BY "index";
```

### 6.2 Persistence and version changes

1. Compare the current `/quran/v1/version.contentVersion` with the active OPFS
   directory.
2. If both files are present and valid in that directory, open them.
3. Otherwise fetch `/quran/v1/scripts`, download both files directly from their
   CDN URLs with identity encoding, and stream them into a temporary OPFS
   directory.
4. Verify each final byte count and SHA-256.
5. Open both databases read-only and run a minimal schema/row-count check.
6. Promote the directory atomically for subsequent opens.

Call `navigator.storage.persist()` after a user gesture. Persistence remains
best-effort; eviction is handled as a cache miss and redownload.

### 6.3 Lifecycle and UX

- **First paint:** use prerendered Uthmani HTML; no WASM is needed.
- **Before cache:** SPA reads and search fall back to the live API.
- **Background:** load WASM and download/validate the two databases.
- **After cache:** Arabic reads, metadata navigation, and search run locally.
- **Offline:** Arabic remains available; translations show an online-only
  state.

Serialize the prerendered verses into `page.data` so hydration matches the DOM
byte-for-byte. Do not immediately re-query the already-painted surah.

Keep a per-open-surah synchronous verse cache in `reader.svelte.ts`. This lets
existing synchronous helpers such as `verseText`, `copyVerse`,
`bookmarkList.text`, and `durationFor` continue to work without a Worker
round-trip. Guard async navigation with a request/version token so an older
response cannot overwrite the currently selected surah.

### 6.4 API adapter

The web owns slugs; Axum accepts numeric surah identifiers `1..=114`.
`quran.ts` resolves `/app/<slug>` to a number before the adapter calls the API.

The adapter consumes the API envelope's `data` field and the settled camelCase
`/surahs` + `/ayahs` wire (`surah`, `ayah`, `ayahCount`). The upstream SQLite
column names `sura` and `aya` stay inside the backend/Worker boundary.

It handles `404`, validation `400`, `429`, and retryable `5xx` responses using
the API's closed error shape; a failed request must never become an empty surah.

---

## 7. Navigation metadata

`quran-data.xml` is metadata, not verse text. Both consumers derive the same
inclusive ranges from its start markers:

```text
startGlobal = current marker global index
endGlobal   = next marker global index - 1
last end    = 6236
```

- Rust parses the XML into its in-memory store at startup.
- The SvelteKit build parses the same XML and includes the small range and
  sajda arrays in the web bundle.

This provides local juz, page, ruku, hizb-quarter, manzil, and sajda navigation
without modifying either database and without adding a third file for the user
to maintain.

The browser computes the deterministic `/random` result locally using the same
frozen UTC-date constants as the API.

The API's 300-ayah response cap is sufficient for before-cache complete-surah
fallback because the largest surah has 286 ayahs.

---

## 8. Search

There is one search algorithm with two implementations:

```text
simple-clean quran_text rows
  ──► normalize every verse with searchVersion rules
  ──► substring match the normalized query
  ──► return ascending globalIndex order
```

- **Online:** Rust builds the normalized array at backend startup and scans it
  in memory.
- **Offline:** the Worker reads all 6,236 rows from the cached
  `quran-simple-clean.sqlite` once, builds the same normalized array, and scans
  it in memory.

Both sides use the same fixtures and rules: remove combining marks, fold
hamza-bearing alefs, fold alef-maqsura to ya, apply the settled ta-marbuta
decision, and collapse whitespace. Both enforce the same query length,
`limit`, `offset`, phrase/subsequence semantics, and ascending-global-index
ordering.

A fixed query suite must return identical ordered verse keys online and
offline. Highlight calculation may differ internally, but cannot change the
match set. A normalization change bumps `searchVersion` and ships updated Rust
and web code; it does not modify or rebuild either SQLite file.

No FTS extension, FTS table, token query language, or search database is part of
this plan.

---

## 9. Offline and caching

- OPFS holds both existing Arabic SQLite files under the active
  `contentVersion`.
- One Service Worker at `/` caches the same-origin shell, HTML, JS/CSS, and
  WASM and imports the Firebase Messaging compatibility worker.
- The Service Worker passes `/quran/v1/**` through and does not Cache-Storage
  cache either CDN SQLite file; OPFS is their sole browser-persistent copy.
- It network-first/excludes `/firebase-messaging-sw.js` and
  `/firebase-config.js` so FCM configuration cannot become stale.
- Translation routes, translation-bearing Arabic requests, and translation-pack
  URLs always bypass caches. Translation fetches also use
  `cache: "no-store"`.

The background cost is approximately 2.5 MB for the two existing databases plus
the selected SQLite WASM distribution. None of it blocks the prerendered first
paint.

---

## 10. The `quran.ts` seam

`web/src/lib/data/quran.ts` remains the single web-owned place for URL slugs and
the reader's synchronous contract.

- Split the current catalog metadata from the component-facing loaded-surah
  value, and expand the 11 authored catalog entries to all 114 while preserving
  existing slugs.
- Add `ayahCount: number` to catalog metadata.
- Stop authoring inline verse arrays in the catalog at the data cutover.
  The component-facing loaded-surah value keeps its `verses` array, populated
  from prerendered `page.data`, the per-surah cache, the Worker, or the live
  API.
- Use XML-derived build data for Arabic/transliterated names, place,
  revelation order, counts, bismillah classification, ranges, and sajdas.
- Keep `verseKey`, `parseKey`, `toArabicDigits`, `BISMILLAH`,
  `showsBismillah`, `surahPath`, `slugFor`, and adjacency helpers.

No separate slug JSON or generated slug module is introduced. The backend does
not need web slugs because its surah identifiers are numeric.

---

## 11. Translations

Translations are fetched from the backend on demand and never persisted by the
EasyQuran web reader:

- request only the active translation for the open content;
- render it alongside Arabic, then discard it;
- use `cache: "no-store"` and the Service Worker bypass rules;
- show Arabic-only mode while offline.

The API's downloadable translation packs are for other clients. This web reader
does not request or store them. Translation search, if added, is backend-only.

---

## 12. Phased implementation

### Phase 0 — validate and publish the existing sources

- Add CI assertions from §4.
- Expand the existing `quran.ts` metadata to all 114 surahs.
- Parse XML during the web build for navigation metadata.
- Upload the two existing SQLite files unchanged to immutable,
  `contentVersion`-keyed CDN paths.
- Configure the API to advertise their exact sizes and checksums.

Exit: the checked-in files, backend inputs, SSG inputs, and CDN bytes have
matching digests. No database build step exists.

### Phase 1 — API integration and SEO

- Implement/consume the frozen `/surahs` + `/ayahs` numeric API contract.
- Read the existing Uthmani database directly during SSG.
- Prerender and index all 114 pages with per-surah metadata and sitemap entries.

Exit: all 114 HTML pages contain verbatim Uthmani verse text and the web adapter
matches the API envelopes, wire names, and errors.

### Phase 2 — browser SQLite and core offline reads

- Add the `sqlite-wasm` Worker and OPFS directory lifecycle.
- Download and validate the two existing database files from `/scripts` URLs.
- Wire prerender state, the per-surah synchronous cache, and live fallback.

Exit: all 114 surahs render from local Uthmani/simple-clean data on a subsequent
offline visit.

### Phase 3 — Service Worker and SPA polish

- Combine shell caching and FCM support in one root Service Worker.
- Exclude API, SQLite, FCM configuration, and translation traffic as specified
  in §9.
- Enable navigation preloading/prefetch behavior.

Exit: offline Arabic navigation works and FCM still delivers.

### Phase 4 — matching search and full navigation

- Initialize the in-memory search corpus from simple-clean in the Worker.
- Implement the shared normalization fixtures and substring search.
- Add all range/sajda/batch/global-range surfaces and deterministic random ayah.

Exit: the fixed query suite returns identical ordered keys online and offline,
and the complete reader-facing Arabic surface works offline.

### Phase 5 — live translations

- Consume the frozen translation DTOs.
- Fetch only the selected translation with cache bypass.
- Handle offline Arabic-only mode and RTL/LTR presentation.

---

## 13. Release/version handshake

1. Validate the three authoritative inputs and compute `contentVersion`.
2. Upload the two existing SQLite files unchanged to their never-overwritten
   CDN keys.
3. Deploy the API, which verifies both CDN objects before `/scripts` advertises
   them.
4. Build SSG from the same checked-in Uthmani database and XML. SSG does not
   independently calculate or declare a version.
5. At hydration, read `contentVersion` from `/quran/v1/version` and compare it
   with the active OPFS directory. On mismatch, keep the prerendered page
   internally consistent while both current files download and validate, then
   switch as one unit.
6. Retain still-supported immutable objects so older web bundles can continue
   or safely use live JSON.

---

## 14. Remaining web decisions

1. Author the remaining 103 URL slugs in the existing
   `web/src/lib/data/quran.ts`, preserving the 11 current slugs.
2. Choose the `sqlite-wasm` distribution variant and measure its final transfer
   size during Phase 2.

Settled: use the existing `quran-uthmani.sqlite` and
`quran-simple-clean.sqlite` unchanged in the backend and browser. Do not create
or publish another Arabic SQLite file.
