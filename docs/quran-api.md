# EasyQuran — Quran Content API

> Status: **plan**. Scope: the Axum Quran API and its data stores.
> Translations are a future extension. Web delivery is documented separately.

---

## 1. Fixed decisions

1. Quran and translation content is read-only.
2. Arabic API responses contain **verbatim source text**. The API does not
   normalize, split, trim, or otherwise transform an ayah. This is enforced by
   a golden digest, not by convention — see §3.3.
3. Uthmani and simple-clean Arabic are loaded into memory at backend startup.
4. Translations will be converted into one read-only SQLite database per
   translation. Translation databases are not loaded wholesale into memory.
5. Complete Arabic and translation databases are immutable artifacts served
   directly from S3-compatible object storage.
6. Axum serves metadata, partial reads, search, SEO/build consumers, and
   clients without an offline database. It does not proxy complete database
   downloads.
7. The Quran corpus is **immutable and unversioned**. Published database objects
   are never overwritten in place; the integrity/cache identity is the pinned
   sha256 digest, not a version.
8. Backend and browser identify the same immutable source set by its
   **content-addressed digest** (`sourceDigests.uthmani` / `.simpleClean`), never
   a hand-typed value (§3.3, §8.1).
9. API responses and the two published Arabic databases carry byte-identical
   source ayah text (§3.3).
10. The existing `quran-uthmani.sqlite` and `quran-simple-clean.sqlite` files
    are also the browser download artifacts. No third Arabic SQLite database is
    built or published.
11. This plan does not define the web reader or browser storage.

---

## 2. Goals and non-goals

### Goals

- Add a public Quran API under `/quran`.
- Fetch Arabic by surah, juz, Hafs/Madani page, ruku, hizb quarter,
  manzil, individual ayah, and global range.
- Search Arabic from the in-memory corpus.
- Keep Quran data independent from the mutable application database
  (`sea_db`).
- Expose direct download URLs for the two existing Arabic databases without
  proxying their bytes.
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
| Translation sources | `db/quran/tanzil/translations/sql/*.sql` | 115 SQL dumps; MySQL/phpMyAdmin format, not runtime-ready |
| Translation catalog | `db/quran/tanzil/translations/index.json` | translation names, languages, directions, `lastUpdate`, `sha256`, and source metadata |

Also committed and **not** authoritative for the runtime: `arabic/sql/*.sql`
(the SQL form of the two Arabic databases) and `translations/index.min.json`.
The runtime reads only the two Arabic `.sqlite` files and the XML.

Current sizes, decimal throughout:

- Uthmani SQLite: 1,593,344 B (1.59 MB).
- Simple-clean SQLite: 929,792 B (0.93 MB).
- Translation SQL sources: 178,493,321 B (178.5 MB) across 115 files,
  averaging 1.54 MB, largest 10.55 MB (`ru.kuliev-alsaadi`).

### 3.1 Global ayah invariant

`quran_text."index"` is the canonical global ayah index:

- contiguous from 1 through 6,236;
- unique;
- ordered by surah and ayah;
- equal to `sura.start + aya`, where XML `sura.start` is zero-based
  and `aya` is one-based.

Examples:

- surah 1 → 1–7;
- surah 2 → 8–293 (286 ayahs; the span looks wrong by inspection and is correct);
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

Juz, page, ruku, quarter, and manzil elements are start markers carrying only
`index`, `sura`, and `aya`. One range builder covers all five:

```text
startGlobal = this marker's global index
endGlobal   = next marker's startGlobal - 1
last end    = 6236
```

Each family must tile `[1, 6236]` without gaps or overlaps. The builder also
depends on marker `index` order matching global-index order; assert it rather
than assuming it.

`<quarter>` has no hizb attribute, so the hizb number is not in the source. The
API derives and exposes it rather than making clients guess:

```text
hizb           = ((index - 1) / 4) + 1     // 1..60
quarterInHizb  = ((index - 1) % 4) + 1     // 1..4
```

### 3.3 Verbatim text

The API returns the exact `text` field loaded from the requested source:

- 1:1 remains unchanged;
- 9:1 remains unchanged;
- embedded basmala text in other first ayahs remains unchanged;
- Uthmani differences in 95:1 and 97:1 remain unchanged;
- the mid-ayah basmala in 27:30 remains unchanged;
- Unicode normalization and code-point order remain unchanged.

Measured basmala distribution in the source, which is what `Bismillah` describes:

| Case | Surahs | Count |
|---|---|---:|
| `FirstAyah` — ayah 1 *is* the basmala | 1 | 1 |
| `None` — no basmala | 9 | 1 |
| `EmbeddedPrefix` — ayah 1 begins `basmala + " "` | all others | 112 |

Within `EmbeddedPrefix`, 95:1 and 97:1 carry an extra U+0651 SHADDA
(`0628 0651 0650 0633…` versus the usual `0628 0650 0633…`). `Bismillah` does
not distinguish this, which is harmless because the enum is descriptive only.
27:30 contains the basmala mid-ayah, byte-identical to 1:1, and in simple-clean
is the only non-first ayah that does.

The source encodes the correct state per surah and that state is meaningful:
surah 9's omission is intentional, surah 1's basmala genuinely is ayah 1, and the
95:1/97:1 spelling is a real orthographic distinction. `surah.bismillah` reports
which case a surah is, for styling or display purposes. It is descriptive
metadata and never an instruction to alter text.

Ayah text is stored and served exactly as the source has it, everywhere, at every
stage.

#### Why normalization is a correctness issue, not a style rule

**5,782 of 6,236 Uthmani rows are not NFC-normalized.** NFC would rewrite them,
most often by consuming U+0653 ARABIC MADDAH ABOVE (1,738 occurrences) and
U+0627 ALEF (156). Simple-clean is NFC- and NFKC-stable. Several common layers
normalize silently — some JSON and database drivers, some text pipelines, some
search indexers — so a dependency upgrade can corrupt thousands of ayahs while
every rendered page still looks correct.

Enforce it with a golden digest over the loaded corpus, computed as
`sha256` of all 6,236 texts joined by `\n` in global-index order:

```text
uthmani       32cc746d817cad9fd4366c7597bfceb177e7649233616c0a80309074b2eb99ee   (1,359,434 B)
simple-clean  375934722ccbfab0d97754df464deac0dcffe962dc0632cc1ce5c6ca25dcea67   (  743,921 B)
```

Verified: NFC-normalizing the Uthmani corpus changes its digest to
`6ee54875c37e4d88…`, so this test detects the failure. A per-row equality test
against the same source cannot — if the *loader* is what normalizes, both sides
of that comparison are wrong identically and it passes.

The digest is asserted at startup (§4.1) and in CI (§10).

---

## 4. Architecture

```text
Existing Arabic SQLite files + XML
        │
        ├── backend startup ──► immutable in-memory QuranStore
        │                             │
        │                             └── Arabic API reads and substring search
        │
        └── publish the same two SQLite files to S3/CDN
                                      │
                                      └── browser downloads them directly

Translation SQL sources (future)
        │
        └── build ──► one SQLite database per translation
                              │
                              ├── S3/CDN whole-database downloads
                              └── TranslationService (bounded disk cache)
                                       │
                                       └── translation reads and search
```

`QuranStore` is immutable. The translation layer is mutable and therefore a
sibling of it on `AppState`, not a field inside it (§4.4).

### 4.1 Arabic startup

The store is built **before the runtime serves traffic** — either in a plain
`fn main` that constructs it and then enters the Tokio runtime, or via
`tokio::task::spawn_blocking`. The SQLite and XML file work below
must never run inside a request handler or behind a lazy `OnceCell`.

1. Open both Arabic SQLite files read-only, as `file:<path>?mode=ro&immutable=1`.
2. Read all rows in global-index order.
3. Parse `quran-data.xml`.
4. Build and validate metadata ranges.
5. Compute the source digests (§3.3, §8.1).
6. Construct `QuranStore` and close the SQLite connections.

`immutable=1` is safe for this startup read because deployed source files are
not changed in place while the short-lived connections are open; it also
suppresses lock and `-shm` activity on a read-only mount.

Cost: 12,472 row reads plus a 77,234 B XML parse — roughly 50–150 ms in
release, up to ~1 s in debug or on a cold page cache. Size container
`startupProbe` / `initialDelaySeconds` accordingly.

**Failure policy: fail fast.** Any missing or corrupt source, XML failure,
tiling-invariant failure or digest mismatch logs the
specific invariant and exits non-zero. A partially loaded or invariant-violating
Arabic store is worse than an unavailable one, because it would serve wrong
scripture silently; a failing image is also a louder signal than a degraded
process that passes liveness. This matches the existing eight fail-closed boot
checks in this codebase. There is no "Arabic not ready" served state.

Arabic request handling performs no SQLite query. Uthmani is the default
response script; simple-clean is an alternate response script and the Arabic
search corpus.

### 4.2 Runtime model

Text is stored as per-script arenas with offset tables, not as one boxed string
per verse per script. The reason is scan locality: §7.1 reads only the
normalized corpus, and an interleaved layout would stride 32 B through a 200 KB
slice to collect pointers into 2 MB of scattered heap. Arenas also cut 18,708
allocations (and ~299 KB of allocator overhead) down to 6.

```rust
struct Corpus {
    arena: Box<str>,
    offsets: Box<[u32]>, // len 6237; verse g is arena[offsets[g-1]..offsets[g]]
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
    bismillah: Bismillah, // FirstAyah | None | EmbeddedPrefix
}

/// `K` is a zero-sized family marker (`Juz`, `Page`, `Ruku`, `HizbQuarter`,
/// `Manzil`) so that `pages[juz - 1]` is a compile error.
struct Range<K> {
    index: u16,
    start_global: u32,
    end_global: u32,
    start_sura: u16,
    start_aya: u16,
    end_sura: u16,
    end_aya: u16,
    _family: PhantomData<K>,
}

struct Sajda {
    index: u16,
    sura: u16,
    aya: u16,
    global_index: u32,
    kind: SajdaKind, // Obligatory | Recommended
}

struct QuranMeta {
    suras: [SuraMeta; 114], // 114 × 88 B ≈ 10 KB, safe to construct by value;
                            // anything per-ayah must be boxed
    juzs: Box<[Range<Juz>]>,
    pages: Box<[Range<Page>]>,
    rukus: Box<[Range<Ruku>]>,
    hizb_quarters: Box<[Range<HizbQuarter>]>,
    manzils: Box<[Range<Manzil>]>,
    sajdas: Box<[Sajda]>,
}

/// Normalized simple-clean corpus for §7.1. ~725 KB; one contiguous scan.
struct SearchIndex {
    arena: Box<str>,
    offsets: Box<[u32]>,      // len 6237
    /// normalized scalar index -> source scalar index, per script,
    /// so hits can be highlighted in the rendered script (§7.1).
    map_uthmani: Box<[u32]>,
    map_simple_clean: Box<[u32]>,
}

struct QuranStore {
    uthmani: Corpus,
    simple_clean: Corpus,
    meta: QuranMeta,
    search: SearchIndex,
    source_digests: SourceDigests,
}
```

`QuranStore` holds content only. Artifact URLs are derived from object-storage
settings and joined into `/scripts` by its handler, so a CDN hostname change is
not a Quran content change.

External Quran numbers are one-based. Array access must always convert
explicitly:

```text
verse offset = globalIndex - 1
sura offset  = suraNumber - 1
```

### 4.3 Surah identifiers and web slugs

The public API accepts numeric surah identifiers `1..=114`. URL slugs are a web
routing concern and remain in the existing `web/src/lib/data/quran.ts` contract;
the web adapter resolves a slug to a number before calling Axum. The backend
therefore needs no slug file, and a web-only URL spelling change does not change
Quran API content or its digest.

### 4.4 Translation service — future

The translation layer needs mutable state and so cannot live inside
`Arc<QuranStore>`. It is a sibling field on `AppState`:

```rust
pub struct TranslationService {
    catalog: ArcSwap<Catalog>,
    open: Mutex<LruCache<PackId, Arc<PackHandle>>>,
    disk_bytes_live: AtomicU64,
    disk_bytes_pending_unlink: AtomicU64,
    inflight: Mutex<HashMap<PackId, Weak<OnceCell<Result<Arc<PackHandle>>>>>>,
    downloads: Arc<Semaphore>,
    http: reqwest::Client, // the shared AppState client, not a second pool
}
```

Rule: **no `std::sync` guard may be held across an `.await`.**
`std::sync::MutexGuard` is `!Send`, and axum handler futures must be `Send`, so
locking around a download does not compile. Compute-then-drop, or use
`tokio::sync::Mutex`. `moka::future::Cache::try_get_with` provides correct
single-flight natively and is preferred over hand-rolling it.

---

## 5. Database artifacts

### 5.1 Arabic artifacts

Expose the two existing databases from `/scripts`. These exact files are used
by the backend at startup and downloaded by the web reader:

```json
{
  "data": {
    "scripts": [
      {
        "id": "uthmani",
        "sizeBytes": 1593344,
        "sha256": "581cc5405831bc072fccd8db55cd1db72c5c5440c39bd975edbf03447efecf53",
        "downloadUrl": "https://cdn.example/quran/arabic/uthmani/quran-uthmani.sqlite"
      },
      {
        "id": "simple-clean",
        "sizeBytes": 929792,
        "sha256": "a0c52760d6660ac5be1de5c76bb10df7a839a3e8a87ecb0e636fe2ed45b2e4a3",
        "downloadUrl": "https://cdn.example/quran/arabic/simple-clean/quran-simple-clean.sqlite"
      }
    ]
  }
}
```

Object keys are immutable and content-addressed; there is no version path
segment — the artifact `sha256` is the identity:

```text
quran/arabic/<script>/<filename>.sqlite
```

At boot, `HEAD` both URLs once with `Accept-Encoding: identity`.
Refuse to advertise a link that 404s or whose identity-encoded `Content-Length`
does not equal `sizeBytes`. Axum advertises the URLs but never proxies their
bytes.

The existing databases have only `quran_text` and deliberately remain
byte-for-byte source artifacts; they do not need a `meta` table. The web client
stores them under an OPFS directory keyed by the artifact `sha256` and verifies
each file against `/scripts.sizeBytes` and `/scripts.sha256`.

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
    schema_kind     TEXT NOT NULL CHECK (schema_kind = 'translation-pack'),
    language        TEXT NOT NULL,
    language_code   TEXT NOT NULL,
    direction       TEXT NOT NULL,
    name            TEXT NOT NULL,
    translator      TEXT NOT NULL,
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

`PRAGMA user_version` is file-local, not a project-wide schema number.
`pack_meta.schema_kind` prevents consumers from opening the wrong translation
artifact type. These future translation packs are unrelated to the Arabic
browser/search design and are not required for the Arabic MVP.

Build gates:

- exactly 6,236 verses;
- global indices contiguous from 1 through 6,236;
- `(sura, aya)` unique;
- every row maps to the Arabic corpus's global index;
- expected verse count for every surah;
- **no empty or whitespace-only text** (`length(trim(text)) > 0`);
- required metadata present;
- expected `user_version`;
- `sizeBytes` and `sha256` populated and non-zero;
- `PRAGMA integrity_check` is `ok`;
- no WAL or SHM sidecars.

The empty-text gate is not hypothetical: four rows in the current sources are
empty and pass every other gate — `sq.mehdiu` (2539, 5636), `fa.safavi` (5797),
`ku.asan` (6207).

**Journal-mode mechanism.** The gate above states an outcome; the builder
achieves it by running `PRAGMA journal_mode=DELETE;` then `VACUUM;` before
closing, and the gate asserts header bytes 18 and 19 are both `1` and that no
`-wal`/`-shm` sibling exists. This is load-bearing: a WAL-mode SQLite file
cannot be opened *even for reading* from a read-only directory, because SQLite
must create the `-shm` file. WAL is the ambient default in this repo — the app
database runs it deliberately — so a rebuilt artifact would otherwise fail to
open on a read-only mount. Add `*.db-wal` and `*.db-shm` to `.gitignore` in the
same pass.

Built pack sizes, measured across a full conversion of all 115 dumps
(`translations/sqlite/`, built by `scripts/sql-to-sqlite.ts`): total
**194,895,872 B (194.9 MB)**, averaging 1,694,746 B, largest 13,029,376 B
(`ru.kuliev-alsaadi`). That is ~1.09× the SQL sources, and the five largest are
13.0 / 5.8 / 3.9 / 3.7 / 3.7 MB.

### 5.3 S3-compatible publication

Translation object keys:

```text
quran/translations/<id>/<sha256>/<id>.sqlite
```

Rules for every Arabic or translation database:

- upload the complete SQLite file;
- never overwrite a published object — enforced with a conditional write
  (`If-None-Match: *`) plus a bucket policy denying unconditional `PutObject`,
  with bucket versioning as a backstop;
- for translation packs (future), use a new applicable pack version for a
  replacement; Arabic objects are immutable at a stable, digest-addressed key
  and are never replaced;
- `Content-Type: application/vnd.sqlite3`;
- no `Content-Encoding`: publish the exact SQLite bytes with identity encoding
  so `Content-Length`, byte ranges, `sizeBytes`, and `sha256` all describe the
  same representation. A separately named compressed artifact can be added
  later only with an explicit decompression/checksum contract;
- `Cache-Control: public, max-age=31536000, immutable, no-transform`;
- public `GET` and `HEAD`, with `Accept-Ranges: bytes`;
- bucket CORS as specified below.

Bucket CORS, written out because offline-capable clients may fetch these
directly and need headers *exposed* for progress UI and resumable downloads:

```text
AllowedOrigins:  the explicit production and preview origins (not "*")
AllowedMethods:  GET, HEAD
AllowedHeaders:  Range, If-None-Match, Cache-Control, Pragma
ExposeHeaders:   Content-Length, Content-Range, Accept-Ranges, ETag
MaxAgeSeconds:   86400
```

Without `ExposeHeaders`, JS cannot read `Content-Length` and there is no
progress bar; without `Range` allowed and `Accept-Ranges` exposed, a resumed
download is impossible.

Publishing translation packs does not require the EasyQuran web reader to use
them. That reader deliberately keeps translations live-only and never persists
translation packs; the direct-download surface exists for other clients.

**Retention.** Immutable objects accumulate ~195 MB per full translation
republish, and offline clients pin a version indefinitely by design, so an
age-based lifecycle rule would delete exactly the objects that must be kept.
Retain the N most recent versions per artifact **plus any version referenced by
a still-supported `catalogVersion`**, implemented as a tag-based policy. The
support window is an open decision (§11.2).

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
      "sizeBytes": 1048576,
      "sha256": "…",
      "downloadUrl": "https://cdn.example/quran/translations/en.sahih/<sha256>/en.sahih.sqlite"
    }
  ]
}
```

`sizeBytes` and `sha256` are required and non-zero. They are the only defense
against a truncated download (§5.4).

The mutable catalog uses a short cache policy, for example:

```text
Cache-Control: public, max-age=300, must-revalidate
```

### 5.4 Backend translation cache — future

When a request needs a translation:

1. Resolve the ID through the catalog. Unknown ID → 404, without touching disk.
2. Reuse its local immutable file if present, cloning the `Arc<PackHandle>`.
3. Otherwise download it once into `<pack_cache_dir>/.incoming/<id>-<version>.part`,
   hashing the body incrementally while streaming.
4. Reject on `sha256` or length mismatch, delete the temp file, and return 502.
   Then confirm it opens with the expected pack schema and `user_version`.
5. `rename` it into the cache directory and `fsync` the directory FD.
6. Open a small read-only pool over it and insert into the LRU.

**Integrity is checked by digest, not by schema probe.** A truncated body still
has a valid header, `pack_meta`, and `user_version`, because those live in the
first pages — precisely the part a schema probe reads and a truncated tail is
not. Such a file would be promoted under a never-replaced name and then serve
`SQLITE_CORRUPT`, or for FTS5 *silently return fewer hits*, permanently. On the
13.0 MB Russian pack this is the likely failure mode. S3 ETag is not a usable
checksum for multipart uploads. `PRAGMA quick_check` is the fallback only where
no digest is available.

**The temp file must live inside `pack_cache_dir`.** `rename(2)` is atomic only
within a filesystem; from `/tmp` (tmpfs or overlayfs) to a mounted cache
directory it returns `EXDEV` and fails 100% of the time, and a copy fallback
reintroduces the torn-file window the atomic move exists to prevent. §8.3 makes
a separately mounted, possibly ephemeral cache directory the expected
deployment. Reject at boot if `.incoming` and the cache directory report
different `st_dev`. Sweep orphaned `*.part` files at boot.

**Cold downloads are detached from the request.** The download runs in a
`tokio::spawn` owned by the single-flight entry; handlers await a notification
with a short request budget (1–2 s) and on expiry return 503 with
`Retry-After: 5` while the warm continues. If the requesting handler owned the
future instead, a client disconnect or a tower timeout would cancel the download
**for every follower** waiting on that entry. The 13.0 MB pack on a 2 Mbit/s
path is ~52 s, so an unbudgeted download would pin a request for over a minute.

Connection and eviction model:

- `pack_connections = 4` (or `min(4, num_cpus)`). Read-only SQLite readers do
  not contend, so capping at one serializes reads the storage engine does not
  require: a pathological FTS5 query at ~100 ms drops a pack to ~10 rps with
  full head-of-line blocking.
- `acquire_timeout = 250ms`, shedding with 503 rather than queueing. sqlx's
  30 s default would pile requests up for half a minute.
- `min_connections = 1` for packs currently in the LRU, so the FD and page cache
  stay warm and pinned. `min_connections = 0` would let sqlx close the last
  connection out from under the "is open" invariant below.
- `idle_timeout` and `max_lifetime` disabled — the file is immutable, so there
  is no staleness risk.
- Memory is bounded by `open_pack_cap × pack_connections × cache_size`. SQLite
  defaults to ~2 MB of page cache per connection, so pin `PRAGMA cache_size`
  explicitly and default `open_pack_cap = 8`. At 8 × 4 × 2 MB that is 64 MB; a
  careless cap of 115 would exceed the entire disk budget.

**Eviction safety needs a refcount, not an assertion.** Handlers hold
`Arc<PackHandle>` for the duration of a query; the evictor may only remove
entries whose `Arc::strong_count() == 1`, and must `pool.close().await` —
awaiting FD release — *before* `remove_file`. On Linux `unlink` of an open file
succeeds and the inode survives until the last FD closes, so without this the
failure is silent budget overrun rather than an error: up to
`open_pack_cap × pack_size` (8 × 13.0 MB = 104 MB) past `pack_cache_max_bytes`,
and ENOSPC on a volume sized to the configured limit. Track
`disk_bytes_live` and `disk_bytes_pending_unlink` and admit new downloads
against their **sum**; if no candidate is unpinned, reject with 503 +
`Retry-After` rather than exceeding the budget.

**Warm the cache; do not rely on an ephemeral one.** A `emptyDir` cache means
every pod restart re-downloads up to ~195 MB, and a fleet-wide deploy produces a
thundering herd on the popular packs exactly when p99 is most visible. Prefer a
persistent volume — 195 MB is trivial to provision — or at minimum a configured
`warm_packs` list prefetched at boot in a background task; the top five packs
average 6.0 MB, so ~30 MB of prefetch removes nearly all user-visible cold
starts.

Do not preload every pack. See §6.4 for the status of each failure cause.

---

## 6. API surface

Public prefix: `/quran`

### 6.1 Arabic and metadata

Every navigation family has the same three-route shape, so no family is a
special case:

| Path | Returns |
|---|---|
| `GET /surahs` | all surah metadata |
| `GET /surahs/{surah}` | one surah |
| `GET /surahs/{surah}/ayahs` | ayahs in a surah |
| `GET /sources/{sourceId}/surah/{surah}` | one source's full surah text + normalization (web `readSurah`) |
| `GET /sources/{sourceId}/range?from=&to=` | global-index ayah window ≤300 + per-surah normalizations (web `readRangeText`) |
| `GET /ayahs/{surah}/{ayah}` | one ayah |
| `GET /ayahs?keys=2:255,1:1` | up to 50 arbitrary ayahs, order preserved |
| `GET /ayahs?fromGlobal=1&toGlobal=7` | inclusive global range |
| `GET /juzs`, `GET /juzs/{juz}`, `GET /juzs/{juz}/ayahs` | juz metadata and ayahs |
| `GET /pages`, `GET /pages/{page}`, `GET /pages/{page}/ayahs` | Hafs/Madani pages |
| `GET /rukus`, `GET /rukus/{ruku}`, `GET /rukus/{ruku}/ayahs` | rukus |
| `GET /hizb-quarters`, `GET /hizb-quarters/{quarter}`, `GET /hizb-quarters/{quarter}/ayahs` | all 240 quarter ranges |
| `GET /manzils`, `GET /manzils/{manzil}`, `GET /manzils/{manzil}/ayahs` | manzils |
| `GET /sajdas`, `GET /sajdas/{sajda}` | sajda markers |
| `GET /scripts` | scripts and database download URLs (digest-addressed) |
| `GET /random` | deterministic ayah for a UTC date |
| `GET /search?q=…` | Arabic search |
| `GET /health/ready` | readiness (§8.4), `Cache-Control: no-store` |
| `GET /openapi.json` | OpenAPI document |

`/hizb-quarters` is intentionally explicit: the XML contains 240 quarter
markers, not 60 complete hizb records. Each response carries `hizb` and
`quarterInHizb` (§3.2) so clients need no undocumented arithmetic.

`GET /ayahs/{verseKey}` is **not** a separate resource. A single-segment
`2:255` form is accepted only as an alias and answers
`308 Permanent Redirect` to `/quran/ayahs/2/255`; a segment not matching
`^[1-9][0-9]{0,2}:[1-9][0-9]{0,2}$` returns 400. Two URLs for one resource would
otherwise produce two ETags and two CDN entries for identical bytes.

Parameter matrix:

- `script=uthmani|simple-clean`, default `uthmani`, accepted on every route that
  returns `text`;
- `from` and `to` are inclusive one-based ordinals **within the addressed unit**,
  accepted on every `…/ayahs` route; they are validated against that unit's
  length and never clamped;
- `fromGlobal` and `toGlobal` are inclusive, bounded to `1..=6236`, and required
  together;
- `cursor` and `limit` paginate (see below);
- `{surah}` accepts only `1..=114`; the web layer resolves URL slugs before
  calling the API;
- unknown parameters are rejected with 400, via
  `#[serde(deny_unknown_fields)]` on each query struct — plain serde ignores
  them silently.

**Response size is capped at 300 ayahs.** `1..=6236` bounds each endpoint but
not the span, so without a cap `?fromGlobal=1&toGlobal=6236` legally returns the
entire Qur'an in one document. A request whose computed span exceeds the cap
returns 400 `range_too_large` with the cap in the body. Units that can exceed it
— especially manzils and global ranges — are
paginated with `?cursor=<globalIndex>&limit=<=300>`, and `RangeMeta` carries
`nextCursor`.

Every complete surah fits under the cap; the largest has 286 ayahs, so the web
reader's before-cache full-surah fallback never needs pagination.

### 6.2 Translation routes — future

Rather than enumerate a narrower list: **every §6.1 route that returns ayah text
has a `/translations/{id}` mirror with an identical path shape, identical
parameters, and identical validation**, except that `script` does not apply and
`/random` is Arabic-only. `/scripts` has no mirror; `/translations` is its
analogue.

| Path | Returns |
|---|---|
| `GET /translations` | catalog with direct download URLs |
| `GET /translations/{id}` | one translation's metadata |
| `GET /translations/{id}/…` | the §6.1 surface, per the mirror rule above |

Arabic and translation responses share verse keys and global indices. Clients
can join them without a multi-pack API response. Arabic ayah routes also accept
`?translations=<id>` for a single pack, adding a `translations` array per ayah,
so the common reader view (Arabic plus one translation) is one round trip rather
than two.

The EasyQuran web reader may use either live JSON shape, but it sends
translation-bearing requests with client cache bypass and its Service Worker
never stores them. It does not consume the translation-pack download URLs.
That product policy does not remove the public pack-download capability for
other clients.

### 6.3 Response types

Every Quran content, metadata, search, and artifact-metadata response uses one
envelope. Readiness and the OpenAPI document are operational exceptions:

```rust
struct Envelope<T> {
    data: T,
}
```
struct RangeMeta {
    kind: RangeKind, // surah | juz | page | ruku | hizb-quarter | manzil | global
    index: Option<u16>,        // None for kind = global
    hizb: Option<u8>,          // hizb-quarter only
    quarter_in_hizb: Option<u8>,
    start_global: u32,
    end_global: u32,
    first: VerseKey,
    last: VerseKey,
    count: u32,                // ayahs returned
    total: u32,                // ayahs in the unit, before from/to/cursor
    script: Script,
    next_cursor: Option<u32>,
}

struct Ayah {
    key: String,
    surah: u16,
    ayah: u16,
    global_index: u32,
    text: String,             // exact selected-source text
    sajda: Option<SajdaKind>, // omitted when absent, never null
    // navigation position, free from the in-memory ranges:
    juz: u16,
    page: u16,
    ruku: u16,
    hizb_quarter: u16,
    manzil: u8,
}

struct AyahRange {
    range: RangeMeta,
    ayahs: Vec<Ayah>,
}

struct TranslationRange {
    translation_id: String,
    range: RangeMeta,
    ayahs: Vec<TranslationVerse>, // key, surah, ayah, global_index, text
}

struct SearchResponse {
    query: String,
    total: u32,
    limit: u16,
    offset: u32,
    results: Vec<SearchHit>,
}

struct SearchHit {
    kind: SearchHitKind, // REQUIRED discriminator; "ayah" today ("opener" deferred, not shipped)
    ayah: Ayah,
    /// UTF-16 code-unit offsets into `ayah.text`, for a JavaScript consumer.
    highlights: Vec<Highlight>, // { start: u32, end: u32 }
}

struct Artifact {
    id: String,
    size_bytes: u64,
    sha256: String,
    download_url: String,
}

// `GET /sources/{sourceId}/surah/{surah}` — web `readSurah` shape.
struct SourceSurahText {
    source_id: String,           // "uthmani" | "simple-clean"
    script: String,
    verses: Vec<String>,
    normalization: SurahNormalization,
}

struct SurahNormalization {
    surah: u16,
    source_id: String,
    script: String,
    source_profile: String,
    packaging: OpenerPackaging,   // numbered-ayah | embedded-prefix | chapter-flag | separate-row | absent
    opener_kind: OpenerKind,
    opener_text: Option<String>,
    opener_end_scalar: u32,
    body_start_scalar: u32,
}

// `GET /sources/{sourceId}/range?from=&to=` — web `readRangeText` shape.
struct RangeText {
    ayahs: Vec<LeanAyah>,         // { key, surah, ayah, global_index, text }
    normalizations: Vec<SurahNormalization>,
}

```

`script` lives on `RangeMeta`, not on each `Ayah`, since it is constant across a
response. The `sourceId` is one of `uthmani` or `simple-clean`; the `readSurah`
and `range` endpoints return the web worker's exact shapes (raw text plus
per-surah normalization so the client can split basmala openers identically
online and offline).

Serialization, which must be stated because the defaults are wrong here:

```rust
#[serde(rename_all = "camelCase")]   // structs
#[serde(rename_all = "kebab-case")]  // enums
```

`Script` = `"uthmani" | "simple-clean"` — under default casing this would
serialize as `"SimpleClean"` and not round-trip with the query value.
`SajdaKind` = `"recommended" | "obligatory"`. `Place` = `"meccan" | "medinan"`.
`Bismillah` = `"first-ayah" | "none" | "embedded-prefix"` — note that a default
derive would emit the string `"None"`, which readers confuse with JSON `null`.
`Ayah.sajda` uses `skip_serializing_if` and is omitted when absent.

These are the API's first camelCase success bodies; existing modules serialize
snake_case, so this is a deliberate divergence.

Wire naming is settled as `/surahs` + `/ayahs`, with `surah`, `ayah`, and
`ayahCount` in success bodies. Source SQLite columns and XML elements retain
their upstream `sura` / `aya` spelling internally; adapters must not leak those
names onto the wire.

### 6.4 Errors

All errors share one body:

```json
{ "error": { "code": "range_too_large", "message": "…",
             "detail": { "max": 300, "requested": 6236 } } }
```

`Content-Type: application/json; charset=utf-8`, with a closed `code` list.

| Condition | Status |
|---|---:|
| unknown Quran identifier or translation ID | 404 |
| unknown route | 404 |
| unknown script or invalid value | 400 |
| malformed range, or range out of bounds | 400 |
| unknown query parameter | 400 |
| malformed or non-existent date | 400 |
| range span exceeds the cap | 400 |
| method not GET/HEAD/OPTIONS | 405 + `Allow` |
| rate limit exceeded | 429 + `Retry-After` |
| translation catalog unreachable | 503 + `Retry-After` |
| pack download in progress | 503 + `Retry-After: 5` |
| pack capacity rejection | 503 + `Retry-After` |
| pack fetch from object storage failed | 502 |
| pack failed digest or integrity check | 500 |

Dates are strict ISO 8601 calendar dates: `2026-2-3` and `2026-02-30` are both
400. Every 5xx carries `Cache-Control: no-store` so a CDN cannot pin a failure.
The four pack failure causes are deliberately separated — a client must be able
to tell "retry shortly" from "this will never work" from "page someone" — and
each emits a distinct metric label (§8.4). Note that the existing rate limiter
also fails closed with 503; keep its code distinct.

There is no "backend not ready" status. An Arabic invariant failure exits at
boot (§4.1) and is never observed by a client.

---

## 7. Search

### 7.1 Arabic MVP

At startup, build the normalized `SearchIndex` (§4.2) over simple-clean. A
search request:

1. normalizes `q` with the **same function** used to build the corpus;
2. scans the normalized arena in one contiguous pass;
3. terminates early once `limit` hits are collected;
4. renders text from the requested script, defaulting to Uthmani, and maps hit
   offsets back into that script for highlighting.

**Match semantics**: a substring match of the normalized query against the
normalized ayah value, after collapsing whitespace runs in both. A query
containing spaces is one phrase, not independent terms. No boolean operators,
wildcards, or stemming in the MVP. Substring matching across word boundaries is
deliberate — Arabic clitics (`و`, `ال`, `ب`, `ل`) attach to the following word.

Results are ordered by ascending `globalIndex`; ordering is stable and
documented so paging is consistent. `total` reports the full match count.
Relevance ranking is out of scope for the MVP.

Highlighting requires the offset maps in `SearchIndex`: matching happens on
normalized simple-clean while rendering happens in the requested script, and
those strings differ in both length and code points (alef-wasla U+0671 versus
alef U+0627, plus all harakat), so a raw normalized offset is meaningless in the
response text. Offsets are returned as UTF-16 code units because the consumer is
JavaScript.

Normalization applies only to search values, never response text:

- remove combining marks;
- fold hamza-bearing alefs to bare alef;
- fold alef-maqsura to ya;
- optionally fold ta-marbuta to ha (§11.1 — resolve before Phase 2 ships, not
  after).

Corpus and query normalization implement one shared specification and must pass
the same fixtures in Rust and Node. A divergence is a correctness bug, not a
tuning knob. The frozen rule set has no version tag — it is itself immutable,
like the corpus. Changing the rule set ships with updated backend/web code and
invalidates search ETags (the digest is the cache identity). It does not alter
or require rebuilding the existing SQLite files.

Limits:

- minimum 3 and maximum 64 Unicode scalar values after normalization, both in
  the same unit; outside that range is 400. Two scalars under substring
  semantics (`ال`, `من`) matches a large fraction of the corpus and returns
  "the first 20 ayahs of the Qur'an" at the highest possible cost;
- `limit` defaults to 20, capped at 50; `offset` defaults to 0, capped at 500.

The scan covers ~700 KB of normalized text — roughly 0.1–0.5 ms with a
`memchr`-based search, acceptable inline in an async handler. If it ever exceeds
~1 ms, move it to `spawn_blocking`.

### 7.2 Translation search — future

Translation search opens one explicit pack from the local disk cache and
queries its `verses_fts` table.

The API never opens or scans all translation packs for one request.

### 7.3 Offline client parity

The web reader downloads the existing simple-clean database, reads its 6,236
rows into a small in-memory normalized array, and uses the same substring
semantics, limits, ordering, and frozen normalization fixtures as the
backend. No FTS table or derived SQLite database is required.

A fixed query suite must return identical ordered verse keys online and
offline. Highlight rendering may be implemented independently, but it must not
change which verses match.

---

## 8. Caching, CORS, and runtime

### 8.1 Integrity, identity, and API caching

The Quran corpus is immutable, so it has no version. Its identity is the
**pinned sha256 digest** of each source set — `sourceDigests.uthmani` and
`sourceDigests.simpleClean` — surfaced on `/health/ready`. There is no
`contentVersion`, `searchVersion`, `apiVersion`, `/version` endpoint, or
version-derived URL segment anywhere in the API.

| Identity | Covers | Source of truth |
|---|---|---|
| `sourceDigests` (sha256) | the two Arabic SQLite corpora | computed at boot; pinned, never hand-typed |
| artifact `sha256` | each published SQLite file | `/scripts` envelope |
| `PRAGMA user_version` | one SQLite file's schema (file-local) | that file's schema; checked at open |
| per-pack/catalog identity | one translation / the catalog (future) | translation system, separate from the Quran corpus |

The digest is **derived, not declared**: at boot compute `sha256` of each
corpus's 6,236 texts joined by `\n` in global-index order (§3.3) and assert it
against the pinned constant. Boot fails on mismatch; the digest is never sent
to clients as a version. The web client does not recompute it — it reads
`/scripts.sha256` and `/health/ready.sourceDigests`, keys its OPFS directory by
the artifact `sha256`, and redownloads a file only if that digest differs.

An operator-typed version string would join the backend's source inputs and the
published objects with nothing but discipline, and both failure modes are
silent: forget to bump it and every cached response plus every `If-None-Match`
returns 304 for changed content; bump it spuriously and `/scripts` advertises a
`downloadUrl` for a key that was never uploaded. A content-addressed digest
makes same-day republishes structurally distinct and removes the bump-it-by-hand
failure mode entirely.

The two existing Arabic databases remain verbatim and carry no internal
version marker. Their identity is carried by their sha256 in the `/scripts`
envelope and by the immutable object key (`quran/arabic/<script>/<filename>.sqlite`).

Cache headers:

```text
Cache-Control: public, max-age=300, s-maxage=86400,
               stale-while-revalidate=604800, stale-if-error=604800
ETag: W/"<source-digest>:<canonical-resource-key>"
Vary: Accept-Encoding
```

The short browser TTL with a long shared TTL is deliberate and the inverse of
the obvious arrangement: browser caches cannot be purged, CDN caches can, so the
unpurgeable layer gets the short life. (If the digest is later moved into the
URL path, serve `max-age=31536000, immutable` instead and give the digest-free
alias the policy above.)

`<canonical-resource-key>` is the resource kind plus its normalized identifiers
plus **every** accepted query parameter in fixed alphabetical order, with
defaults made explicit — e.g. `surahs/2/ayahs?from=1&script=uthmani&to=286`.
Verse keys normalize to `surah/ayah`. Omitting `script` here
would let `?script=simple-clean` and `?script=uthmani` collide on one ETag, and
a shared CDN would then serve the wrong script. Translation ETags (future) are
`W/"<arabic-source-digest>+<pack-sha256>:<key>"`.

The ETag is weak because the shared compression layer varies the encoded bytes
for an unchanged representation, which is legal for weak validators and illegal
for strong ones. API JSON routes never serve byte ranges; the immutable SQLite
objects in §5.3 keep object-storage strong ETags.

Support `If-None-Match`, `304 Not Modified`, and `HEAD`. A 304 repeats `ETag`,
`Cache-Control`, and `Vary`; a `Vary` mismatch between 200 and 304 corrupts
shared caches.

Search is cacheable — it is a pure function of
`(sourceDigest, normalized q, script, limit, offset)`:

```text
Cache-Control: public, max-age=60, s-maxage=300
ETag: W/"<source-digest>:search?limit=20&offset=0&q=<normalized-q>&script=uthmani"
```

`no-store` here would force every repeat of a popular query to re-scan the
corpus at the origin, which is the load §8.2 rate-limits against; and `private`
would distinguish nothing on a router with no user.

### 8.2 Public router and CORS

The Quran API is mounted as a **sibling router branch**, merged after the
existing branch has been layered:

```rust
let private = router::router(state.clone())
    .layer(/* existing chain, unchanged */)
    .layer(RouteBlockerLayer::new(state.clone()));

let public = modules::quran_v1::routes()
    .layer(/* security headers, request id, metrics, trace, compression */)
    .layer(rate_limit::rate_limit_layer(&state, 600, 60))
    .layer(middleware::from_fn(middlewares::client_ip::resolve_client_ip))
    .layer(settings.http.ip_source.clone().into_extension())
    .layer(public_cors_layer());

let app = Router::new().merge(private).nest("/quran", public).with_state(state);
```

Nesting inside `router.rs` cannot escape the existing layers, because `main.rs`
wraps the router returned by `router::router(state)` wholesale and
`Router::layer` rewrites every route present at that moment. But **no existing
layer needs restructuring**: `.layer()` binds to the routes present when it is
called, and `Router::merge` preserves each side's already-applied layers, so a
sibling branch is sufficient. Earlier drafts of this plan called for
restructuring the application-wide CORS and session layers; that overstates the
work and puts live authentication at risk for no benefit.

The public branch must have:

- `Access-Control-Allow-Origin: *`;
- GET, HEAD, and OPTIONS only;
- `Access-Control-Allow-Headers: If-None-Match, If-Modified-Since, Accept, Accept-Encoding, Cache-Control, Pragma`;
- `Access-Control-Expose-Headers: ETag, Cache-Control, Retry-After`;
- no credentials, no session middleware, no CSRF middleware;
- shared request IDs, tracing, metrics, compression, and security headers.

The two CORS header lines are not optional detail. Browser JS cannot read a
response header absent from `Access-Control-Expose-Headers`, and a cross-origin
`If-None-Match` fails preflight unless the header is allowed — so without them
the entire conditional-GET design silently fails in a browser, which is the
primary consumer.

Three layers currently in the global stack must **not** apply to this branch:

- `middlewares::cors::origin_guard` — it rejects any request whose `Origin` is
  not in a hardcoded allowlist, returning `OperationNotAllowed`. It is an
  allowlist *enforcer*, not a header-setter, so it fails closed no matter what
  `CorsLayer` advertises, and a wildcard-CORS public API is impossible while it
  is in the path.
- `RouteBlockerLayer` — outside `development` it runs an upsert *and* a select
  against `sea_db` per request, whose pool is `max_connections(1)`. Every Quran
  request would serialize on one connection and perform a write, contradicting
  the Phase 2 exit criterion and §2's independence goal.
- `csrf_guard` — GETs are already method-exempt, so this is cleanliness rather
  than function.

Two layers must be **re-applied**, because they live on the existing branch only:

- `resolve_client_ip` and the `ip_source` extension. The rate limiter keys on the
  `ClientIp` extension and falls back to the literal string `"unknown"` when it
  is absent, so without these every caller on Earth shares one bucket and the
  first 600 requests per minute rate-limit everyone. This exact bug has been
  fixed here once before.
- `security_headers`, `request_id`, `http_metrics`, and `TraceLayer`, which are
  applied inside `router.rs` rather than `main.rs`. Extract the stack into a
  helper so both branches share one definition. (`request_id` and
  `http_metrics` are currently applied twice, in both files; do not make it a
  third.)

`http_metrics` must switch to `MatchedPath`. It currently labels on the raw
path, and the Quran surface is 6,236 ayahs + 604 pages + 556 rukus + 240
quarters + 114 surahs ≈ 7,750 label values across three instruments — a metrics
cardinality incident. Using `MatchedPath` also improves the existing routes.

Rate limiting: apply a coarse per-IP limit to all public routes and a tighter
one to search. The existing `/search/v1` limit of 30/min is a *mechanism*
precedent, not a value precedent — it was calibrated for a CSRF-gated POST that
a caller must first mint a token for, whereas `GET /quran/search` is
method-exempt from CSRF. Note also that the existing limiter keys per path, so
"coarse across all Quran routes" needs a path-independent key strategy.

Also apply bounds checking before store access, and catalog lookup for
translation IDs rather than raw path construction.

### 8.3 Runtime configuration

Follow the existing `Settings` convention: a sub-struct with an inherent
`from_env()`, composed as a field on `Settings`, reading through the
`config::env` helpers, with mandatory values `expect`ed at the call site
carrying an operator-actionable message. Note that every existing setting is a
`String` — there is no `PathBuf` or `Url` precedent — and `Settings` deliberately
does not derive `Debug`.

```rust
struct QuranSettings {
    uthmani_path: PathBuf,          // QURAN_UTHMANI_PATH
    simple_clean_path: PathBuf,     // QURAN_SIMPLE_CLEAN_PATH
    metadata_xml_path: PathBuf,     // QURAN_METADATA_XML_PATH
}

struct TranslationSettings { // future
    catalog_url: Url,
    pack_cache_dir: PathBuf,
    pack_cache_max_bytes: u64,
    open_pack_cap: usize,              // default 8
    pack_connections: u32,             // default 4
    pack_cache_size_kib: u32,          // PRAGMA cache_size, pinned
    pack_acquire_timeout: Duration,    // default 250ms
    cold_download_concurrency: usize,
    cold_download_total_timeout: Duration,  // default 120s
    cold_download_stall_timeout: Duration,  // default 15s with no bytes
    max_download_bytes: u64,           // catalog sizeBytes + slack
    warm_packs: Vec<String>,
}
```

Download URLs are **not** hand-entered settings. Arabic and translation URLs
are derived from `settings.object_storage.public_url` and the bucket; Arabic
keys are stable and digest-addressed (`quran/arabic/<script>/<filename>.sqlite`),
translation keys (future) carry their pack identity.

`max_download_bytes` bounds a broken or hostile upstream; without it a
misbehaving endpoint can fill the cache volume.

Add `pub quran: Arc<QuranStore>` and `pub translations: Option<Arc<TranslationService>>`
to `AppState`. `AppState` is `Clone` with all-cheap-clone fields and exactly one
construction site, so this is mechanical.

Dependencies to add: `sqlx` as a **direct** dependency with the `sqlite` feature
(it is already in the graph transitively via SeaORM), an XML parser, and
`unicode-normalization` and `moka` as direct dependencies.

Do **not** add `rusqlite`. Its `libsqlite3-sys` and sqlx's both declare
`links = "sqlite3"`; a version mismatch is a hard cargo link-collision error,
and `deny.toml`'s `multiple-versions = "warn"` will not catch it. Prefer
`roxmltree` for the XML: pure Rust, DOM-shaped, ideal for a 77 KB file read once
at boot. Separately, `deny.toml` currently ignores two quick-xml advisories for
a crate that is not in this workspace's graph at all — inherited from ruxlog.
Delete those stale ignores, or a genuinely vulnerable quick-xml entering the
graph would be silently suppressed.

New error codes live in `ruxlog-types`, a closed enum whose `status_code()` map
is locked by a totality snapshot test and which feeds TS/Dart codegen. Reuse the
generic codes (`RecordNotFound`, `InvalidInput`, `ServiceUnavailable`) where they
fit; a `QRN_*` namespace means touching a shared crate and downstream codegen.

The runtime image or deployment mount must contain both existing Arabic SQLite
files and the XML. All three are startup inputs and may be mounted read-only.
The future translation cache directory is writable and should be persistent
(§5.4).

### 8.4 Health

Keep liveness separate from readiness. `GET /quran/health/ready`:

```json
{
  "ready": true,
  "sourceDigests": { "uthmani": "32cc746d…", "simpleClean": "37593472…" },
  "verseCount": 6236,
  "surahCount": 114,
  "translations": {
    "catalogVersion": "2026-07-30",
    "catalogAgeSeconds": 42,
    "catalogLastError": null,
    "packsCached": 3,
    "packBytesUsed": 20025344,
    "packBytesMax": 536870912,
    "openPacks": 2,
    "coldDownloadsInFlight": 0,
    "lastDownloadError": null
  }
}
```

The `translations` block never affects the top-level `ready`. Arabic is either
loaded or the process exited (§4.1), so `ready` is effectively constant after
boot and this endpoint is primarily a digest/observability surface.

Without those translation fields the guarantee "a translation failure must not
make Arabic unavailable" is **unobservable**: an operator whose catalog fetch has
failed for a week, or whose disk cache is thrashing, sees a green `ready: true`
and 503s only in client logs. `sourceDigests` lets an operator diff the image
against the bucket.

Metrics: `quran_pack_download_total{id,result}`,
`quran_pack_download_seconds`, `quran_pack_cache_bytes`,
`quran_pack_evictions_total`, `quran_pack_open_connections`,
`quran_catalog_refresh_failures_total`,
`quran_translation_requests_total{id,status}`, `quran_sqlite_queries_total`.
Alert on sustained 503 rate and on eviction rate (thrash).

### 8.5 Ayah of the day

`GET /random?date=YYYY-MM-DD` maps a UTC date to an ayah:

```text
ayah = ((daysSinceEpoch * K + C) % 6236) + 1     // K = 1103, C = 4177
```

`6236 = 2² × 1559` with 1559 prime, so any `K` coprime to 6,236 gives a
full-period cycle: every ayah appears exactly once per 6,236 days. `K` and `C`
are frozen constants — changing them changes the answer for every past date.

A plain `(daysSinceEpoch % 6236) + 1` also has the full-period property but is a
sequential march through the mushaf: tomorrow is always today + 1, so it walks
2:1 through 2:286 for 286 consecutive days and the next value is trivially
predictable. The multiplier destroys that adjacency at no cost.

The mapping is uniform over ayahs, so surahs with more ayahs appear more often.

`date` is optional and defaults to the current UTC date. It is always a UTC
calendar date, never a timestamp; a client wanting local-day semantics computes
its own local date and passes it. The resolved date is echoed in the response so
a client can detect a stale cache.

Caching, which cannot use the §8.1 blanket policy:

- `date` omitted → `Cache-Control: public, max-age=<seconds until next 00:00 UTC>`;
- `date` supplied → `Cache-Control: public, max-age=31536000, immutable`.

Under the blanket `max-age`, a client fetching at 23:59 UTC would hold
yesterday's ayah for most of the following day.

---

## 9. Implementation phases

### Phase 0 — Arabic store

- Add settings for the two existing SQLite files and the XML.
- Load both Arabic sources into memory as arenas.
- Parse XML and build every metadata range.
- Validate the source invariants and assert the golden digests.
- Add `Arc<QuranStore>` to `AppState`.
- Include or mount startup data in the backend deployment.
- Upload the two Arabic artifacts to immutable digest-addressed keys — 2.5 MB of bytes
  with no build pipeline needed, so there is no reason to defer it to Phase 3.

Exit: all 6,236 source rows in both scripts are available verbatim from memory,
and the golden digests match the constants in §3.3.

### Phase 1a — Router split

- Add the sibling public router branch with wildcard CORS.
- Re-apply `resolve_client_ip`, the `ip_source` extension, and the shared
  observability stack to it.
- Exclude `origin_guard`, `RouteBlockerLayer`, `csrf_guard`, and session layers.
- Switch `http_metrics` to `MatchedPath`.

Exit, both halves required:

1. Table-driven over the router's enumerated Quran routes — every route returns
   no `Set-Cookie`, `Access-Control-Allow-Origin: *`, and no
   `Access-Control-Allow-Credentials`; a Quran route missing from the table
   fails the test.
2. A regression suite over the existing authenticated routes asserts a session
   cookie is still issued, origin-restricted CORS still applies, and a revoked
   session is still rejected.

The second half is not optional. This step touches live authentication for every
existing route, and a mistake here silently logs out every user or drops
revoked-session enforcement.

### Phase 1b — Arabic API

- Add `modules/quran_v1/` with routes, validators, and DTOs.
- Freeze `/surahs` + `/ayahs`, numeric surah identifiers, and the
  `surah`/`ayah` wire fields as specified in §6.
- Add the response-size cap and cursor pagination.
- Add cache headers, weak ETags, HEAD, and conditional requests.
- Expose the two existing database URLs and checksums from `/scripts`; never
  proxy their bytes.

Exit: every range family returns the correct ordered slice; no response exceeds
the cap; a cross-origin conditional GET completes preflight and the client can
read `ETag`.

### Phase 1c — OpenAPI

- Add the first `#[utoipa::path]` annotations and register schemas in `docs.rs`.

`docs.rs` is currently 11 lines with an empty `paths()` list and there are zero
annotations anywhere in the tree, so there is no convention to follow — this is
greenfield and is separated for that reason.

### Phase 2 — Arabic search

- Add the normalized `SearchIndex` with offset maps.
- Add `/search`, limits, rate limiting, metrics, and smoke tests.
- Resolve the ta-marbuta question (§11.1) before shipping.
- Freeze the normalization fixtures; use the same substring semantics online and
  offline.

Exit: search performs no SQLite query, per the assertion in §10.

### Phase 3 — Translation artifacts

- Convert each source translation into its own SQLite pack.
- Add pack metadata, FTS, and all build gates including empty-text and digests.
- Upload versioned immutable translation databases.
- Publish the translation catalog with `sizeBytes` and `sha256`.

Exit: complete databases download directly from object storage; re-publishing an
existing key fails.

### Phase 4 — Translation API

- Add `TranslationService`, the bounded disk cache, and detached single-flight
  cold downloads.
- Add one-pack translation read and search routes.
- Add cold-cache, eviction, unavailable-pack, integrity-failure, and
  concurrency tests.

Exit: per the assertions in §10 — no pack is loaded wholesale into memory and no
request opens more than one pack.

---

## 10. Tests

Each exit criterion below is stated as something a test can actually observe.

### Arabic and metadata

- Both Arabic sources contain exactly 6,236 rows.
- Global indices are contiguous and map to every `(sura, aya)`.
- Metadata counts are 114 / 30 / 604 / 556 / 240 / 7 / 15.
- Every range family tiles `[1, 6236]`, and marker `index` order matches global
  order.
- `hizb` and `quarterInHizb` derivation covers 1..60 and 1..4 exactly.

### Verbatim responses

- **Golden digest**: `sha256` over all 6,236 texts joined by `\n` equals the
  constants in §3.3, for both scripts. This is the load-bearing test — a per-row
  comparison against the same source cannot detect a normalizing *loader*,
  because both sides would be wrong identically.
- Every API `text` equals the selected source value.
- 1:1, 2:1, 9:1, 27:30, 95:1, and 97:1 remain unchanged.
- The `Bismillah` split is exactly 1 / 112 / 1.
- No response-text normalization occurs.

### HTTP

- Invalid identifiers return 404; invalid scripts, ranges, dates, and queries
  return 400; unknown query parameters return 400.
- A non-GET/HEAD/OPTIONS method returns 405 with `Allow`.
- Ranges are inclusive and globally ordered; `from > to` and out-of-bounds are
  400 and never clamped.
- A whole-Qur'an global range is rejected or paginated; no single response
  exceeds 300 ayahs.
- Uthmani is the default script; two requests differing only in `script` have
  different ETags.
- Conditional GET and HEAD work, including on `/search`; a 304 repeats `ETag`,
  `Cache-Control`, and `Vary`.
- A cross-origin conditional GET completes preflight and the JS client can read
  `ETag`.
- Public routes have wildcard CORS, no credentials, and no cookies — table-driven
  over the enumerated routes.
- `/ayahs/2:255` returns 308 to `/ayahs/2/255`; `/ayahs/abc` returns 400.
- `/random` over 6,236 consecutive dates yields a permutation of `1..=6236`, and
  no two consecutive dates differ by 1.
- `/scripts` omits any artifact whose URL failed the boot `HEAD` check or whose
  identity-encoded `Content-Length` differs from `sizeBytes`.
- `/scripts` exposes exactly the existing Uthmani and simple-clean databases,
  and Axum never serves their bytes.

### No-SQLite-after-startup

- After the store is built, `chmod 000` the Arabic sources (or unmount the data
  directory) and assert every Quran route still returns byte-identical bodies.
- `quran_sqlite_queries_total` is unchanged across a request-exercising
  integration test.
- A CI check asserts `modules/quran_v1/` contains no reference to `sea_db`,
  `sqlx`, or any SQLite type; `load_quran_store()` owns its connections locally
  so the absence is enforced structurally.

### Search

- A fixed query set returns identical ordered verse keys online and offline.
- Rust and web normalization pass the same frozen fixture corpus.
- Highlight offsets fall on valid UTF-16 boundaries in the response script.
- A 2-scalar query returns 400; early termination caps allocations at `limit`.
- A normalization fixture change is a correctness change that ships with updated
  backend and web code; there is no version tag to bump.

### Translation future

- One SQLite pack is built per catalog entry.
- Every pack has 6,236 correctly mapped rows, no empty text, and expected
  metadata.
- Catalog URLs point to versioned objects; `sizeBytes` and `sha256` are non-zero.
- A truncated download is rejected and leaves no file in the cache.
- Concurrent cold requests download a pack once, and cancelling one requester
  does not abort the download for the others.
- `.incoming` and the cache directory are on the same device.
- Disk and open-connection limits are enforced; the evictor never unlinks a
  pinned pack, and accounting includes pending-unlink bytes.
- With `mmap_size=0` and `cache_size` pinned, querying the 13.0 MB pack keeps
  the process RSS delta under 8 MB.
- A single-pack request opens exactly one pack file.
- Each of the four pack failure causes returns its distinct status, and every
  503 carries `Retry-After`.
- Re-publishing an existing object key exits non-zero.
- Translation failures do not affect Arabic, and a degraded translation layer is
  visible in `/health/ready`.

---

## 11. Remaining API decisions

1. Whether ta-marbuta folding improves Arabic search (resolve before Phase 2).
2. Retention window for superseded immutable objects (§5.3).
3. Future translation disk-cache sizing and `warm_packs` selection.
