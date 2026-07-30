# EasyQuran — Quran Content API

> Status: **plan**. Scope: the Axum Quran API and its data stores.
> Translations are a future extension. Web delivery is documented separately.

---

## 1. Fixed decisions

1. Quran and translation content is read-only.
2. Arabic API responses contain **verbatim source text**. The API does not
   normalize, split, trim, or otherwise transform an ayah.
3. Uthmani and simple-clean Arabic are loaded into memory at backend startup.
4. Translations will be converted into one read-only SQLite database per
   translation. Translation databases are not loaded wholesale into memory.
5. Complete Arabic and translation databases are immutable artifacts served
   directly from S3-compatible object storage.
6. Axum serves metadata, partial reads, search, SEO/build consumers, and
   clients without an offline database. It does not proxy complete database
   downloads.
7. Published database objects are versioned and never overwritten in place.
8. This plan does not define the web reader or browser storage.

---

## 2. Goals and non-goals

### Goals

- Add a public Quran API under `/quran/v1`.
- Fetch Arabic by surah, juz, Hafs/Madani page, ruku, hizb quarter,
  manzil, individual ayah, and global range.
- Search Arabic from the in-memory corpus.
- Keep Quran data independent from the mutable application database
  (`sea_db`).
- Expose direct database download URLs.
- Define the future translation pack and API contract without implementing it
  in the Arabic MVP.

### Non-goals

- Editing Quran or translation content.
- Storing Quran content in `easyquran.db`.
- Loading every translation into process memory.
- Searching every translation in one request.
- Accounts, bookmarks, notes, or reading-progress sync.
- Web delivery implementation.

---

## 3. Source data

| Asset | Path | Current shape |
|---|---|---|
| Uthmani Arabic | `db/quran/tanzil/arabic/quran-uthmani.sqlite` | `quran_text("index" PK, sura, aya, text)`, 6,236 rows |
| Simple-clean Arabic | `db/quran/tanzil/arabic/quran-simple-clean.sqlite` | same schema, 6,236 rows |
| Metadata | `db/quran/tanzil/quran-data.xml` | surah and navigation metadata |
| Translation sources | `db/quran/tanzil/translations/sql/*.sql` | 115 SQL dumps; not runtime-ready |
| Translation catalog | `db/quran/tanzil/translations/index.json` | translation names, languages, directions, and source metadata |

Current sizes:

- Uthmani SQLite: about 1.6 MB.
- Simple-clean SQLite: about 930 KB.
- Translation SQL sources: about 170 MiB total.

### 3.1 Global ayah invariant

`quran_text."index"` is the canonical global ayah index:

- contiguous from 1 through 6,236;
- unique;
- ordered by surah and ayah;
- equal to `sura.start + aya`, where XML `sura.start` is zero-based
  and `aya` is one-based.

Examples:

- surah 1 → 1–7;
- surah 2 → 8–293;
- surah 114, ayah 6 → 6,236.

The backend can therefore represent each Arabic script as one array and serve
navigation units as array slices.

### 3.2 Metadata

| Element | Count | API use |
|---|---:|---|
| `<sura>` | 114 | surah metadata |
| `<juz>` | 30 | juz ranges |
| `<page>` | 604 | Hafs/Madani page ranges |
| `<ruku>` | 556 | ruku ranges |
| `<quarter>` | 240 | hizb-quarter ranges |
| `<manzil>` | 7 | manzil ranges |
| `<sajda>` | 15 | sajda markers |

Juz, page, ruku, quarter, and manzil elements are start markers. One range
builder covers all five:

```text
startGlobal = this marker's global index
endGlobal   = next marker's startGlobal - 1
last end    = 6236
```

Each family must tile `[1, 6236]` without gaps or overlaps.

### 3.3 Verbatim text

The API returns the exact `text` field loaded from the requested source:

- 1:1 remains unchanged;
- 9:1 remains unchanged;
- embedded basmala text in other first ayahs remains unchanged;
- Uthmani differences in 95:1 and 97:1 remain unchanged;
- the mid-ayah basmala in 27:30 remains unchanged;
- Unicode normalization and code-point order remain unchanged.

`surah.bismillah` describes the source content for clients. It does not cause a
response transformation.

---

## 4. Architecture

```text
Arabic SQLite + XML
        │
        ├── S3/CDN whole-database downloads
        │
        └── backend startup ──► immutable in-memory QuranStore
                                      │
                                      └── Arabic API reads and search

Translation SQL sources (future)
        │
        └── build ──► one SQLite database per translation
                              │
                              ├── S3/CDN whole-database downloads
                              └── backend bounded disk cache
                                       │
                                       └── translation reads and search
```

### 4.1 Arabic startup

1. Open both Arabic SQLite files read-only.
2. Read all rows in global-index order.
3. Parse `quran-data.xml` and the committed slug table.
4. Build and validate metadata ranges.
5. Construct `QuranStore`.
6. Close the SQLite connections.

Arabic request handling performs no SQLite query. Uthmani is the default
response script; simple-clean is an alternate response script and the Arabic
search corpus.

### 4.2 Runtime model

```rust
struct VerseText {
    uthmani: Box<str>,
    simple_clean: Box<str>,
}

struct SuraMeta {
    index: u16,
    ayas: u16,
    start_global: u32,
    end_global: u32,
    revelation_order: u16,
    ruku_count: u16,
    place: Place,
    name_arabic: Box<str>,
    name_translit: Box<str>,
    name_english: Box<str>,
    slug: Box<str>,
    bismillah: Bismillah, // FirstAyah | None | EmbeddedPrefix
}

struct Range {
    index: u16,
    start_global: u32,
    end_global: u32,
    start_sura: u16,
    start_aya: u16,
}

struct QuranMeta {
    suras: [SuraMeta; 114],
    juzs: Box<[Range]>,
    pages: Box<[Range]>,
    rukus: Box<[Range]>,
    hizb_quarters: Box<[Range]>,
    manzils: Box<[Range]>,
    sajdas: Box<[Sajda]>,
}

struct QuranStore {
    // verses[global_index - 1]
    verses: Box<[VerseText]>,
    meta: QuranMeta,
    content_version: Box<str>,
    artifacts: ArabicArtifacts,
    translations: Option<TranslationRegistry>, // future
}
```

External Quran numbers are one-based. Array access must always convert
explicitly:

```text
verse offset = globalIndex - 1
sura offset  = suraNumber - 1
```

### 4.3 Slugs

Use one committed 114-entry `num → slug` table. At startup assert:

- exactly 114 entries;
- every surah number appears once;
- all slugs are unique;
- existing public slugs remain stable.

---

## 5. Database artifacts

### 5.1 Arabic artifacts

Expose whole-database download metadata from `/scripts`:

```json
{
  "id": "uthmani",
  "contentVersion": "2026-07-30",
  "sizeBytes": 1593344,
  "downloadUrl": "https://cdn.example/quran/arabic/uthmani/2026-07-30/quran-uthmani.sqlite"
}
```

Object keys are versioned:

```text
quran/arabic/<script>/<content-version>/<filename>.sqlite
```

### 5.2 Translation packs — future

Each source dump becomes one file:

```text
translations/packs/<translation-id>.sqlite
```

Minimum schema:

```sql
PRAGMA user_version = 1;

CREATE TABLE pack_meta (
    id              TEXT PRIMARY KEY,
    language        TEXT NOT NULL,
    language_code   TEXT NOT NULL,
    direction       TEXT NOT NULL,
    name            TEXT NOT NULL,
    translator      TEXT NOT NULL,
    content_version TEXT NOT NULL,
    source_url      TEXT
);

CREATE TABLE verses (
    global_index INTEGER PRIMARY KEY,
    sura         INTEGER NOT NULL,
    aya          INTEGER NOT NULL,
    text         TEXT NOT NULL,
    UNIQUE (sura, aya)
);

CREATE VIRTUAL TABLE verses_fts USING fts5(
    text,
    content = 'verses',
    content_rowid = 'global_index'
);
```

Build gates:

- exactly 6,236 verses;
- global indices contiguous from 1 through 6,236;
- `(sura, aya)` unique;
- every row maps to the Arabic corpus's global index;
- expected verse count for every surah;
- required metadata present;
- expected `user_version`;
- no WAL or SHM sidecars.

### 5.3 S3-compatible publication

Translation object keys:

```text
quran/translations/<id>/<content-version>/<id>.sqlite
```

Rules for every Arabic or translation database:

- upload the complete SQLite file;
- never overwrite a published object;
- use a new content version for a replacement;
- `Content-Type: application/vnd.sqlite3`;
- `Cache-Control: public, max-age=31536000, immutable`;
- public `GET` and `HEAD`;
- bucket CORS allows the intended clients.

The translation catalog contains:

```json
{
  "catalogVersion": "2026-07-30",
  "schemaVersion": 1,
  "translations": [
    {
      "id": "en.sahih",
      "language": "English",
      "languageCode": "en",
      "direction": "ltr",
      "name": "Saheeh International",
      "translator": "Saheeh International",
      "contentVersion": "2011-04-24",
      "sizeBytes": 0,
      "downloadUrl": "https://cdn.example/quran/translations/en.sahih/2011-04-24/en.sahih.sqlite"
    }
  ]
}
```

The mutable catalog uses a short cache policy, for example:

```text
Cache-Control: public, max-age=300, must-revalidate
```

### 5.4 Backend translation cache — future

When a request needs a translation:

1. Resolve the ID through the catalog.
2. Reuse its local immutable file if present.
3. Otherwise download it once to a temporary path.
4. Confirm it opens with the expected pack schema.
5. Atomically move it into the local cache.
6. Open one read-only connection, with a maximum of two if measurements
   justify it.

Use:

- a single-flight guard for cold downloads;
- a small LRU for open connections;
- a separate byte-bounded disk cache;
- `min_connections = 0`;
- a process-wide limit for cold downloads and pack queries;
- versioned local filenames that are never replaced while open.

Do not preload every pack. If an explicitly requested pack is unavailable,
return 503 rather than silently omitting it.

---

## 6. API surface

Public prefix: `/quran/v1`

### 6.1 Arabic and metadata

| Path | Returns |
|---|---|
| `GET /suras` | all surah metadata |
| `GET /suras/{sura}` | one surah |
| `GET /suras/{sura}/ayahs` | all ayahs in a surah |
| `GET /suras/{sura}/ayahs?from=10&to=20` | inclusive within-surah range |
| `GET /ayahs/{sura}/{aya}` | one ayah |
| `GET /ayahs/{verseKey}` | one ayah, for example `2:255` |
| `GET /ayahs?fromGlobal=1&toGlobal=7` | inclusive global range |
| `GET /juzs` | all juz metadata |
| `GET /juzs/{juz}` | one juz |
| `GET /juzs/{juz}/ayahs` | all ayahs in a juz |
| `GET /pages` | all Hafs/Madani page metadata |
| `GET /pages/{page}` | one page |
| `GET /pages/{page}/ayahs` | all ayahs on a page |
| `GET /rukus` | all ruku metadata |
| `GET /rukus/{ruku}/ayahs` | all ayahs in a ruku |
| `GET /hizb-quarters` | all 240 quarter ranges |
| `GET /hizb-quarters/{quarter}/ayahs` | all ayahs in a quarter |
| `GET /manzils` | all manzil metadata |
| `GET /manzils/{manzil}/ayahs` | all ayahs in a manzil |
| `GET /sajdas` | all sajda markers |
| `GET /scripts` | scripts, content version, and database download URLs |
| `GET /random?date=YYYY-MM-DD` | deterministic ayah for a UTC date |
| `GET /search?q=...&script=uthmani` | Arabic search |

`/hizb-quarters` is intentionally explicit: the XML contains 240 quarter
markers, not 60 complete hizb records.

Arabic text parameters:

- `script=uthmani|simple-clean`, default `uthmani`;
- `from` and `to` are inclusive one-based ayah numbers;
- `fromGlobal` and `toGlobal` are inclusive and bounded to `1..=6236`;
- unknown parameters are rejected.

### 6.2 Translation routes — future

Translation endpoints target one pack at a time:

| Path | Returns |
|---|---|
| `GET /translations` | catalog with direct download URLs |
| `GET /translations/{id}` | one translation's metadata |
| `GET /translations/{id}/suras/{sura}/ayahs` | translated surah |
| `GET /translations/{id}/ayahs/{sura}/{aya}` | translated ayah |
| `GET /translations/{id}/juzs/{juz}/ayahs` | translated juz |
| `GET /translations/{id}/pages/{page}/ayahs` | translated page |
| `GET /translations/{id}/search?q=...` | search one translation |

Arabic and translation responses share verse keys and global indices. Clients
can join them without a multi-pack API response.

### 6.3 Response types

```rust
struct Ayah {
    key: String,
    sura: u16,
    aya: u16,
    global_index: u32,
    text: String,             // exact selected-source text
    script: Script,
    sajda: Option<SajdaKind>,
}

struct AyahRange {
    range: RangeMeta,
    ayahs: Vec<Ayah>,
    content_version: String,
}

struct TranslationAyah {
    key: String,
    sura: u16,
    aya: u16,
    global_index: u32,
    translation_id: String,
    text: String,
    content_version: String,
}

struct Artifact {
    id: String,
    content_version: String,
    size_bytes: u64,
    download_url: String,
}
```

Wire fields use `camelCase`.

### 6.4 Errors

| Condition | Status |
|---|---:|
| unknown Quran identifier or translation | 404 |
| unknown script or invalid value | 400 |
| malformed range or query | 400 |
| requested translation pack unavailable | 503 |
| Arabic source invariant failure | backend not ready |

---

## 7. Search

### 7.1 Arabic MVP

At startup, build a normalized search value for every simple-clean ayah. A
search request:

1. normalizes `q`;
2. scans the 6,236 search values;
3. returns matching global indices;
4. renders text from the requested script, defaulting to Uthmani.

A bounded in-memory scan is sufficient for the MVP and keeps Arabic search out
of a mutable FTS database.

Normalization applies only to search values, never response text:

- remove combining marks;
- fold hamza-bearing alefs to bare alef;
- fold alef-maqsura to ya;
- optionally fold ta-marbuta to ha after search-quality testing.

Limits:

- minimum two Unicode scalar values after normalization;
- maximum 128 UTF-8 bytes;
- `limit` defaults to 20 and is capped at 50;
- search responses are not cached.

### 7.2 Translation search — future

Translation search opens one explicit pack from the local disk cache and
queries its `verses_fts` table.

The API never opens or scans all translation packs for one request.

---

## 8. Caching, CORS, and runtime

### 8.1 Versioning and API caching

`QURAN_CONTENT_VERSION` is an operator-managed release string. Change it when
an Arabic source, metadata XML, slug table, or API representation changes.

It is returned by `/scripts`, included in response envelopes, logged at
startup, and used in weak ETags:

```text
Cache-Control: public, max-age=86400, s-maxage=3600
ETag: W/"<content-version>:<canonical-resource-key>"
```

Support `If-None-Match`, `304 Not Modified`, and `HEAD`.

Search:

```text
Cache-Control: private, no-store
```

Each translation has its own independent content version.

### 8.2 Public router and CORS

The Quran API must be mounted on a separate public router branch:

- `Access-Control-Allow-Origin: *`;
- GET, HEAD, and OPTIONS only;
- no credentials;
- no session middleware;
- no CSRF middleware;
- shared request IDs, tracing, metrics, compression, and security headers.

The current application-wide credentialed CORS and session layers in
`main.rs` must be restructured; nesting routes differently inside `router.rs`
alone will not bypass them.

Apply:

- a generous coarse rate/concurrency limit to all origin routes;
- the existing search limit precedent to search routes;
- bounds checking before store access;
- catalog lookup for translation IDs rather than raw path construction.

### 8.3 Runtime configuration

```rust
struct QuranSettings {
    uthmani_path: PathBuf,
    simple_clean_path: PathBuf,
    metadata_xml_path: PathBuf,
    slug_table_path: PathBuf,
    content_version: String,
    uthmani_download_url: Url,
    simple_clean_download_url: Url,
}

struct TranslationSettings { // future
    catalog_url: Url,
    pack_cache_dir: PathBuf,
    pack_cache_max_bytes: u64,
    open_pack_cap: usize,
    pack_connections: u32, // default 1, maximum 2
    cold_download_concurrency: usize,
}
```

Add `pub quran: Arc<QuranStore>` to `AppState`.

The runtime image or deployment mount must contain both Arabic SQLite files,
the XML, and the slug table. They are startup inputs only. The future
translation cache directory is writable and may be ephemeral.

### 8.4 Health

Keep liveness separate from readiness. Quran readiness exposes:

```json
{
  "ready": true,
  "contentVersion": "2026-07-30",
  "verseCount": 6236,
  "suraCount": 114
}
```

A translation catalog or pack failure must not make Arabic unavailable.

### 8.5 Random ayah

`GET /random?date=YYYY-MM-DD` maps a UTC date to:

```text
(daysSinceEpoch % 6236) + 1
```

This is uniform over ayahs. Surahs with more ayahs appear more often.

---

## 9. Implementation phases

### Phase 0 — Arabic store

- Add settings and the slug table.
- Load both Arabic sources into memory.
- Parse XML and build every metadata range.
- Validate the source invariants.
- Add `Arc<QuranStore>` to `AppState`.
- Include or mount startup data in the backend deployment.

Exit: Quran readiness is true and all 6,236 source rows in both scripts are
available verbatim from memory.

### Phase 1 — Arabic API

- Add `modules/quran_v1/`.
- Add Arabic and metadata routes.
- Add validators and DTOs.
- Add the separate public router/CORS branch.
- Add cache headers, weak ETags, HEAD, conditional requests, and OpenAPI.

Exit: all range families return the correct ordered slice, with no session
cookie or credentialed CORS on Quran routes.

### Phase 2 — Arabic search

- Add normalized in-memory search values.
- Add `/search`, limits, rate limiting, metrics, and smoke tests.

Exit: Arabic reads and search perform no SQLite query after startup.

### Phase 3 — Translation artifacts

- Convert each source translation into its own SQLite pack.
- Add pack metadata, FTS, and build gates.
- Upload versioned immutable Arabic and translation databases.
- Publish the translation catalog.

Exit: complete databases download directly from object storage and published
keys are never overwritten.

### Phase 4 — Translation API

- Add the bounded disk cache and single-flight cold downloads.
- Add one-pack translation read and search routes.
- Add cold-cache, eviction, unavailable-pack, and concurrency tests.

Exit: translations are never loaded wholesale into Rust memory and no request
fans out across all packs.

---

## 10. Tests

### Arabic and metadata

- Both Arabic sources contain exactly 6,236 rows.
- Global indices are contiguous and map to every `(sura, aya)`.
- Metadata counts are 114 / 30 / 604 / 556 / 240 / 7 / 15.
- Every range family tiles `[1, 6236]`.
- The slug table contains 114 unique entries.

### Verbatim responses

- Every API `text` equals the selected source value.
- 1:1, 2:1, 9:1, 27:30, 95:1, and 97:1 remain unchanged.
- No response-text normalization occurs.

### HTTP

- Invalid identifiers return 404.
- Invalid scripts, ranges, dates, and queries return 400.
- Ranges are inclusive and globally ordered.
- Uthmani is the default script.
- Conditional GET and HEAD work.
- Public routes have wildcard CORS, no credentials, and no cookies.
- Search uses `private, no-store`.

### Translation future

- One SQLite pack is built per catalog entry.
- Every pack has 6,236 correctly mapped rows and expected metadata.
- Catalog URLs point to versioned objects.
- Concurrent cold requests download a pack once.
- Disk and open-connection limits are enforced.
- Translation failures do not affect Arabic.

---

## 11. Remaining API decisions

1. Final 114-entry slug spelling.
2. `QURAN_CONTENT_VERSION` naming convention.
3. Whether ta-marbuta folding improves Arabic search.
4. Future translation disk-cache and open-pack limits.
5. Whether the first translation release needs juz/page endpoints in addition
   to surah, ayah, and search.
