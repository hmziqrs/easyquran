# EasyQuran — Quran content delivery to the web

> Status: **implemented**. This is the decision record for how Quran content
> reaches the browser — the constraints and the reasoning behind them, not a
> build plan. The phased rollout and its open questions are done and have been
> removed.
>
> Scope: a server-rendered, SEO-indexable Qur'an reader with a live Rust backend
> and an offline-capable Arabic reader. The browser caches the two existing
> Arabic databases unchanged: `quran-uthmani.sqlite` and
> `quran-simple-clean.sqlite`. **No additional Arabic SQLite database is built,
> generated, or maintained.**
>
> This document owns web delivery decisions. `quran-api.md` owns Rust runtime
> behavior and the shared HTTP contract. `quran-normalization.md` owns the
> canonical view and normalization rules.
>
> **§11 is superseded once translations ship:** it records today's live-only
> behavior, which the Service Worker still enforces. `quran-translations.md`
> replaces it with direct SQLite packs from R2.
>
> **Planned web-reader supersession:**
> [`quran-ssg-optimization-plan.md`](./quran-ssg-optimization-plan.md) replaces
> this record's full-Surah SSG and web-build XML parsing decisions only after
> that migration passes its acceptance checks. Until then, this document still
> describes the implemented system.

---

## 1. Requirements

Standing constraints. Cited elsewhere by number — keep the numbering stable.

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
   `cache: "no-store"` and are never written to OPFS or Cache Storage. (See the
   supersession note above.)
6. **SPA-fast after hydration.** Client navigation is prefetched and local once
   the Arabic files are ready.
7. **Preserve the reader boundary.** The `VerseKey` contract, pure routing
   helpers, and component-facing loaded-surah shape in
   `web/src/lib/data/quran.ts` remain stable.

Operational resources such as health, OpenAPI, and version metadata remain
online-only.

---

## 2. Why the existing databases are enough

Both files already have the schema needed for verse reads:

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

- `quran-uthmani.sqlite` is the display source — Axum, SSG, browser reading,
  copying, and SEO HTML.
- `quran-simple-clean.sqlite` is the search source and the optional
  simple-clean display script. Its smaller, undiacritated text is loaded into a
  small in-memory array and substring-scanned.
- `quran-data.xml` supplies surah metadata and navigation markers to the Rust
  backend. The web no longer reads it: two one-time, immutable compact JSON
  snapshots carry the web catalog and navigation data.
- `web/static/quran-meta/quran-catalog.json` owns URL slugs, mapped to the API's
  numeric surah identifier.

There is no need to merge the files, and no need to add FTS: the complete
simple-clean corpus is about 744 KB, small enough for a normalized substring
scan in both Rust and a browser Worker.

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

## 4. Source invariants and `contentVersion`

The web build and backend consume the checked-in source files directly. There
is no database builder and no generated SQLite artifact.

Invariants that must hold for every release:

- each database contains exactly 6,236 rows;
- global indices are contiguous from 1 through 6,236;
- both databases have identical ordered `(index, sura, aya)` keys;
- the verbatim text digests equal the golden constants in `quran-api.md` §3.3;
- XML counts are 114 surahs, 30 juzs, 604 pages, 556 rukus, 240 quarters,
  7 manzils, and 15 sajdas;
- each range family tiles `[1, 6236]` without a gap or overlap;
- Uthmani bismillah classification is exactly one first-ayah, 112 embedded
  prefixes, and one absent; the shadda variants at 95:1 and 97:1 are preserved;
- the CDN identity-encoded byte length and SHA-256 for each file match what
  `/quran/v1/scripts` advertises.

The databases deliberately have no internal EasyQuran version field. Their
enclosing `contentVersion` is computed by the backend from:

```text
BLAKE3(uthmani bytes || simple-clean bytes || XML bytes)[0..16 hex]
```

The browser uses that value as the OPFS directory name and validates each
download against its advertised `sizeBytes` and `sha256`. When `contentVersion`
changes, it downloads both current files into a new directory before promoting
it.

---

## 5. Server and SSG

Backend implementation is owned by `quran-api.md`. Rust reads both Arabic
databases and the XML at startup, builds an immutable in-memory store, and
closes its SQLite handles. HTTP requests do not query SQLite.

The two web-facing jobs:

- **SSG for SEO.** `+page.server.ts` reads `quran-uthmani.sqlite` directly
  through `node:sqlite` and prerenders all 114 `/app/<slug>` pages without a
  running backend during `vite build`. A server-only loader reads the compact
  catalog/navigation snapshots solely to select the route data; the full
  snapshots do not enter HTML, Svelte page data, or initial browser JavaScript.
- **Live JSON.** `/quran/v1` serves Arabic reads, range metadata, normalized
  substring search, and live translations. `/quran/v1/scripts` advertises the
  immutable CDN URLs, sizes, and checksums.

Crawlers do not depend on client WASM. Prerendered Uthmani HTML is the SEO and
first-paint surface; live/backend and OPFS data take over after hydration.

---

## 6. Browser data flow

**Engine.** `sqlite-wasm` runs inside a dedicated Web Worker because OPFS
synchronous access handles are Worker-only. The Worker opens both source
databases read-only — Uthmani for the reader, simple-clean when that script is
requested and to initialize search — keeping their existing `quran_text` schema.

**Persistence and version changes.**

1. Compare `/quran/v1/version.contentVersion` with the active OPFS directory.
2. If both files are present and valid there, open them.
3. Otherwise fetch `/quran/v1/scripts`, download both files directly from their
   CDN URLs with identity encoding, and stream them into a temporary directory.
4. Verify each final byte count and SHA-256.
5. Open both read-only and run a minimal schema/row-count check.
6. Promote the directory atomically.

Call `navigator.storage.persist()` after a user gesture. Persistence is
best-effort; eviction is handled as a cache miss and redownload.

**Lifecycle.** First paint uses prerendered HTML (no WASM). Before the cache is
warm, SPA reads and search fall back to the live API. WASM and the databases
load in the background. After caching, reads, navigation, and search run
locally. Offline, Arabic remains available and translations show an online-only
state.

Prerendered verses are serialized into `page.data` so hydration matches the DOM
byte-for-byte; the already-painted surah is not re-queried. `reader.svelte.ts`
keeps a per-open-surah synchronous verse cache so helpers like `verseText`,
`copyVerse`, `bookmarkList.text`, and `durationFor` work without a Worker
round-trip. Async navigation is guarded by a request/version token so a stale
response cannot overwrite the selected surah.

**API adapter.** The web owns slugs; Axum accepts numeric identifiers `1..=114`.
The adapter consumes the envelope's `data` field and the camelCase `/surahs` +
`/ayahs` wire (`surah`, `ayah`, `ayahCount`); the SQLite column names `sura` and
`aya` stay inside the backend/Worker boundary. It handles `404`, validation
`400`, `429`, and retryable `5xx` using the API's closed error shape — a failed
request must never become an empty surah.

---

## 7. Navigation metadata

`quran-data.xml` is metadata, not verse text. Both consumers derive the same
inclusive ranges from its start markers:

```text
startGlobal = current marker global index
endGlobal   = next marker global index - 1
last end    = 6236
```

Rust parses the XML into its in-memory store at startup. SvelteKit reads the two
checked-in compact snapshots through a server-only loader during prerender and
the browser fetches them only after first paint or explicit navigation intent.
This gives local juz, page, ruku, hizb-quarter, manzil, and sajda navigation
without touching either database or embedding corpus-wide metadata in the
initial bundle.

The browser computes the deterministic `/random` result locally using the same
frozen UTC-date constants as the API. The API's 300-ayah response cap suffices
for before-cache complete-surah fallback because the largest surah has 286 ayahs.

---

## 8. Search

One algorithm, two implementations — normalize every simple-clean verse with
`searchVersion` rules, substring match, return ascending `globalIndex` order.
Online, Rust builds the normalized array at startup; offline, the Worker reads
all 6,236 rows once and builds the same array.

`quran-normalization.md` owns the normalization rules themselves. What this
document fixes is the parity contract: both sides share fixtures and rules, and
enforce the same query length, `limit`, `offset`, phrase/subsequence semantics,
and ordering. A fixed query suite must return identical ordered verse keys
online and offline. Highlighting may differ internally but cannot change the
match set. A normalization change bumps `searchVersion` and ships new Rust and
web code; it never modifies or rebuilds a SQLite file.

No FTS extension, FTS table, token query language, or search database exists.

---

## 9. Offline and caching

- OPFS holds both existing Arabic SQLite files under the active
  `contentVersion`.
- One Service Worker at `/` caches the same-origin shell, HTML, JS/CSS, and
  WASM, and handles Firebase Messaging push natively (no `importScripts`).
- The Service Worker passes `/quran/v1/**` through and does not Cache-Storage
  cache either CDN SQLite file; OPFS is their sole browser-persistent copy.
- It excludes `/firebase-config.js` so FCM configuration cannot become stale.
- Translation routes, translation-bearing Arabic requests, and translation-pack
  URLs always bypass caches. Translation fetches also use `cache: "no-store"`.

The background cost is roughly 2.5 MB for the two databases plus the SQLite WASM
distribution. None of it blocks the prerendered first paint.

---

## 10. The `quran.ts` seam

`web/src/lib/data/quran.ts` is the single web-owned place for URL slugs and the
reader's synchronous contract. Catalog metadata is separate from the
component-facing loaded-surah value; the latter keeps its `verses` array,
populated from prerendered `page.data`, the per-surah cache, the Worker, or the
live API. Verse arrays are not authored inline in the catalog.

Arabic/transliterated names, place, revelation order, counts, bismillah
classification, ranges, and sajdas come from XML-derived build data.
`verseKey`, `parseKey`, `toArabicDigits`, `BISMILLAH`, `showsBismillah`,
`surahPath`, `slugFor`, and the adjacency helpers stay.

No separate slug JSON or generated slug module exists. The backend does not need
web slugs — its surah identifiers are numeric.

---

## 11. Translations (current behavior)

Superseded by `quran-translations.md` once that plan lands. Today:

- request only the active translation for the open content;
- render it alongside Arabic, then discard it;
- use `cache: "no-store"` and the Service Worker bypass rules;
- show Arabic-only mode while offline.

The API's downloadable translation packs are for other clients. This reader does
not request or store them. Translation search, if added, is backend-only.

---

## 12. Release/version handshake

1. Validate the three authoritative inputs and compute `contentVersion`.
2. Upload the two SQLite files unchanged to their never-overwritten CDN keys.
3. Deploy the API, which verifies both CDN objects before `/scripts` advertises
   them.
4. Build SSG from the same checked-in Uthmani database and XML. SSG does not
   independently calculate or declare a version.
5. At hydration, read `contentVersion` from `/quran/v1/version` and compare it
   with the active OPFS directory. On mismatch, keep the prerendered page
   internally consistent while both files download and validate, then switch as
   one unit.
6. Retain still-supported immutable objects so older web bundles can continue or
   safely use live JSON.
