# EasyQuran — Quran translations implementation plan

> Status: **plan**. Prepared 2026-08-01.
>
> Scope: add selectable Quran translations directly to the SvelteKit reader by
> downloading the existing per-translation SQLite artifacts from Cloudflare R2.
> This is Quran-content translation, not localization of the EasyQuran UI.
>
> This plan requires no translation service and no changes to the existing
> Arabic content API. It supersedes the translation-service notes in
> `quran-api.md` and `quran-web-delivery.md`; those documents should be aligned
> during Phase 0.

---

## 1. Outcome

Readers can choose one approved translation, download it directly from R2, and
see it paired with Arabic ayahs in surah, juz, and page views. Once downloaded,
the selected translation works offline alongside the existing Arabic reader.

The feature reuses what the repository already has:

- one complete SQLite file per translation;
- a browser SQLite-WASM Worker;
- verified byte downloads with progress reporting;
- OPFS with IndexedDB/session fallback;
- canonical global ayah coordinates;
- the composed Svelte 5 reader store.

The implementation preserves these invariants:

- Arabic remains byte-identical, prerendered, searchable, and independently
  offline-capable.
- Translation text remains byte-identical to its Tanzil source.
- A translation download or query failure never blocks Arabic rendering,
  navigation, or search.
- Only catalog-approved translation IDs and versioned R2 objects are opened.
- Tanzil and translator attribution remains visible while a translation is
  active.

The first release supports one installed/active translation at a time. Multiple
parallel translations, translation search, app UI localization, tafsir, and
translated SEO pages are follow-up work.

---

## 2. Architecture

```text
Tanzil SQL sources
    │
    └── existing fidelity-aware SQLite builder
          └── one quran_text database per translation
                    │
                    ├── deterministic size + SHA-256 + contentVersion
                    ├── versioned immutable R2 object
                    └── approved web catalog entry
                                      │
                                      ▼
                              browser Quran Worker
                                ├── download + progress
                                ├── size/SHA verification
                                ├── OPFS / IDB fallback
                                ├── read-only sqlite-wasm
                                └── range queries by global index
                                      │
                                      ▼
                               Svelte reader store
                                └── VerseKey → translation text
```

Translation files never pass through an application server. The browser talks
to the configured R2/CDN origin directly.

Only the active translation database is opened in SQLite-WASM. At most one
translation is retained persistently. Switching sources downloads and validates
the replacement before the previous pack is removed, so temporary storage may
briefly hold two packs.

---

## 3. Current state

### 3.1 Translation data

`db/quran/tanzil/translations/` already contains:

- 115 translations across 44 languages;
- 28 RTL entries across 8 RTL languages;
- 6,236 indexed rows per translation;
- authoritative SQL dumps in `sql/`;
- derived SQLite files in `sqlite/`;
- `index.json` and `index.min.json` catalogs;
- mirror, verification, conversion, catalog, and R2 publishing scripts.

Every SQLite file currently uses the schema the Worker already understands:

```sql
CREATE TABLE quran_text (
    "index" INTEGER PRIMARY KEY,
    sura    INTEGER NOT NULL DEFAULT 0,
    aya     INTEGER NOT NULL DEFAULT 0,
    text    TEXT NOT NULL
);

CREATE INDEX idx_quran_text_sura_aya ON quran_text (sura, aya);
```

The 115 SQLite files total about 195 MB. The average is about 1.7 MB and the
largest is about 13 MB, making one-pack-at-a-time browser downloads practical.

Four upstream rows are empty and must remain unmodified:

| Translation | Verse | Global index |
|---|---:|---:|
| `sq.mehdiu` | 21:56 | 2539 |
| `sq.mehdiu` | 77:14 | 5636 |
| `fa.safavi` | 80:39 | 5797 |
| `ku.asan` | 108:3 | 6207 |

`translation-empty-verses.md` records the source evidence. These rows are
known source exceptions, not text to repair or reject.

### 3.2 Web reader

The reader already:

- prerenders all 114 surahs from Uthmani SQLite;
- renders real juz and page routes;
- downloads and verifies Arabic SQLite artifacts;
- uses SQLite-WASM in `quran.worker.ts`;
- stores Arabic artifacts in OPFS, with IndexedDB/session fallback;
- exposes download progress through the Quran worker client;
- maps all content through canonical global ayah coordinates;
- composes reader behavior through focused `reader.svelte.ts` facets;
- bypasses `/translations/` paths in the Service Worker.

No translation data or translation UI is wired into the reader yet.

### 3.3 Gaps to resolve

1. Current translation objects use flat keys such as
   `tanzil/translations/sqlite/en.sahih.sqlite`; `--force` can replace them.
2. `index.min.json` lacks SQLite size, SQLite SHA-256, content version, direct
   versioned URL, source link, and terms link.
3. Translation files are built with fidelity checks but the current verifier
   primarily validates SQL sources, not every final SQLite artifact.
4. The Worker protocol accepts only Arabic initialization, surah reads, search,
   and ping operations.
5. Byte stores support `get` and `put`, but not deletion/pruning for replacing a
   downloaded translation.
6. No production translation allowlist or default catalog ordering is approved.

---

## 4. Fixed product decisions

1. **Arabic-only is the default.** A reader explicitly selects a translation.
2. **One active translation.** The first release never displays or retains two
   translation sources simultaneously.
3. **Direct SQLite delivery.** The browser downloads the selected versioned
   SQLite artifact from R2 and queries it in the existing Quran Worker.
4. **Offline after download.** The installed translation is stored in OPFS when
   available, IndexedDB as a fallback, or session memory as a final fallback.
5. **One persistent slot.** The most recently installed translation is retained.
   A replacement is verified and opened before the previous pack is removed.
6. **Explicit storage UX.** The picker shows download size/progress, installed
   state, offline availability, and a “Remove download” action.
7. **Ayah-by-Ayah presentation.** Selecting a translation switches the reader
   to Ayah-by-Ayah mode. Continuous Arabic reading remains Arabic-only.
8. **Plain text only.** Translation strings render as text nodes, never HTML or
   Markdown. Source punctuation and Unicode are preserved.
9. **Exact-source blanks.** Empty or whitespace-only source text produces a
   neutral “Translation unavailable for this ayah” line. Stored bytes remain
   unchanged.
10. **Fail the complete range safely.** Missing, duplicated, shifted, or
    malformed translation rows are never partially paired with Arabic.
11. **Production is allowlisted.** All 115 packs are built and verified, but the
    public picker contains only sources approved for quality, attribution,
    licensing, and launch.
12. **No translation search in the first release.** Arabic search behavior is
    unchanged.
13. **No translated SSR/SEO in the first release.** Translation rendering starts
    after hydration from the selected local/downloaded pack.

---

## 5. Reader experience

### 5.1 Translation picker

Add a “Translation” control to the shared reader header. It opens an accessible
sheet/dialog containing:

- “Arabic only” as the first radio option;
- a filter over language, translation name, and translator;
- entries grouped by language;
- translation name as the primary label and translator as supporting text;
- download size and installed/offline badge;
- active download progress and cancellation;
- a removal action for the installed source;
- Tanzil attribution, source link, and applicable terms link.

The runtime catalog loads when the picker first opens or when a saved selection
needs validation. It may remain in memory for the current app session.

Selecting an uninstalled translation starts the download. Keep the picker open
or show an equivalent persistent progress surface until the pack is ready,
cancelled, or fails. Do not activate a partially downloaded pack.

Selecting an installed translation activates it immediately. Selecting Arabic
only disables translation rendering but may retain the one installed pack for
quick offline re-enablement. “Remove download” deletes it and clears the
selection if it is active.

### 5.2 Verse presentation

In Ayah-by-Ayah mode, each `VerseRow` renders:

```text
verse reference + actions
Arabic ayah
translation text
```

The translation line:

- sets `lang` from the catalog language code;
- sets `dir="rtl"` or `dir="ltr"` from the catalog;
- aligns according to its own direction without changing the Arabic line;
- uses a visually distinct but quiet typography treatment;
- reserves a small minimum block while loading to limit layout shift;
- does not repeat translator attribution on every ayah.

The reader header/card shows the active translation name and translator once.
Tanzil attribution remains discoverable beside the selected source or in the
picker.

### 5.3 State changes

- Selecting an uninstalled source clears any old translation lines, downloads
  and validates it, opens it read-only, then atomically makes it active.
- Switching sources aborts a previous in-flight download/query. A monotonic
  token prevents late results from winning.
- Route navigation requests the active translation for the new global range.
- Selecting Arabic only clears displayed lines and persists `null` without
  necessarily deleting the installed pack.
- Going offline leaves Arabic untouched. An installed translation continues to
  work; an uninstalled selection shows the download-required state.
- A retry action repeats only the failed translation operation.
- When a newer content version is advertised, the installed version keeps
  working until the replacement downloads, verifies, and opens successfully.

### 5.4 Existing reader actions

Bookmarks, notes, last-read position, and Arabic font sizing remain unchanged.
Copy and share remain Arabic-only in the first release so enabling a display
preference does not silently change exported content. Including translation can
be added later as an explicit, attributed action.

---

## 6. Artifact and catalog plan

### 6.1 SQLite format

Continue using one standalone SQLite file per translation and the current
`quran_text` schema. Do not add FTS or a second verse representation for the
first release.

The builder must finish each database with:

```sql
PRAGMA journal_mode = DELETE;
VACUUM;
```

Close the database before hashing. There must be no `-wal` or `-shm` sidecar.
The web opens the verified bytes read-only through `sqlite3_deserialize`, just
as it does for Arabic sources.

No internal content-version table is required: the SHA-256 in the catalog and
versioned R2 key identifies the exact complete file. Schema validation protects
against opening a different SQLite artifact type.

### 6.2 Build gates

The data pipeline must prove for all 115 translations:

- the ID is unique and matches the filename;
- the source SQL exists and parses cleanly;
- SQLite contains exactly 6,236 rows;
- global indices are contiguous `1..=6236`;
- `(sura, aya)` is unique and matches the canonical Arabic coordinate table;
- every SQLite text value equals its parsed SQL source value exactly;
- the only blank rows are the four exact exceptions in §3.1;
- required language/name/translator/direction metadata is present;
- `quran_text` has the expected columns, index, and constraints;
- `PRAGMA integrity_check` returns `ok`;
- the SQLite header is compatible with read-only rollback-journal use;
- no WAL/SHM/temp sidecar exists;
- final `sizeBytes` and SHA-256 are non-zero;
- rebuilding unchanged input produces the same logical text digest, while any
  final-file byte difference naturally produces a different content version.

A new or removed blank row fails the build as source drift. It requires review
and an explicit fixture update; it is never auto-corrected.

### 6.3 Content and catalog versions

For each final closed SQLite file:

```text
sha256         = SHA-256(exact SQLite bytes)
contentVersion = first 16 hexadecimal characters of sha256
```

The rich SQL catalog remains provenance. Generate a separate web delivery
catalog containing only approved sources:

```jsonc
{
  "schemaVersion": 1,
  "catalogVersion": "<sha256-prefix-of-canonical-catalog>",
  "source": "Tanzil.net",
  "sourceUrl": "https://tanzil.net/trans/",
  "termsUrl": "https://tanzil.net/docs/terms_of_use",
  "translations": [
    {
      "id": "en.sahih",
      "language": "English",
      "languageCode": "en",
      "direction": "ltr",
      "name": "Saheeh International",
      "translator": "Saheeh International",
      "sourceLastUpdate": "April 24, 2011",
      "contentVersion": "0123456789abcdef",
      "sizeBytes": 1234567,
      "sha256": "<full SQLite SHA-256>",
      "downloadUrl": "https://cdn.example/tanzil/translations/sqlite/en.sahih/0123456789abcdef/en.sahih.sqlite"
    }
  ]
}
```

Sort entries by an explicit product order, with ID as the deterministic
tie-breaker. Compute `catalogVersion` from canonical catalog content excluding
the `catalogVersion` field.

The catalog ID is the only lookup key accepted by the web. `downloadUrl` must
resolve to the configured CDN origin and expected translation/version path.

### 6.4 Publication

Publish with versioned keys:

```text
tanzil/translations/catalog.v1.json
tanzil/translations/sqlite/<id>/<contentVersion>/<id>.sqlite
```

SQLite object rules:

- `Content-Type: application/vnd.sqlite3`;
- identity encoding;
- `Cache-Control: public, max-age=31536000, immutable, no-transform`;
- conditional create with `If-None-Match: *`;
- no `--force` path capable of replacing an existing versioned key;
- public `GET` and `HEAD`;
- `Accept-Ranges: bytes`, even though the first release downloads whole files.

Catalog rules:

- `Content-Type: application/json; charset=utf-8`;
- `Cache-Control: public, max-age=300, must-revalidate`;
- upload only after every referenced SQLite object exists and its `HEAD`
  `Content-Length` matches;
- replacing the mutable catalog is allowed.

Bucket CORS must allow the explicit production and preview web origins, `GET`
and `HEAD`, and expose `Content-Length`, `Accept-Ranges`, `ETag`, and
`Last-Modified`.

After publishing, fully download and hash at least one LTR pack, one RTL pack,
the largest pack, and every pack containing a known blank row.

Keep the existing flat SQLite keys during migration, but new catalog entries
must advertise only versioned keys. Remove legacy objects only under a separate
retention task after confirming no supported client references them.

---

## 7. Browser storage and Worker design

### 7.1 Catalog trust boundary

Configure the catalog URL from the existing Quran R2 base:

```text
<QURAN_R2_BASE>/tanzil/translations/catalog.v1.json
```

The configured URL establishes the only accepted artifact origin. The catalog
and all download metadata are untrusted and decoded field-by-field.

Validation includes:

- supported schema version;
- unique ID with `^[a-z]{2,3}[.][a-z0-9-]+$`;
- non-empty language/name/translator metadata;
- known `ltr | rtl` direction;
- 16-hex content version and 64-hex SHA-256;
- sensible positive byte size capped above the known largest pack;
- HTTPS URL on the configured CDN origin;
- exact `/tanzil/translations/sqlite/<id>/<version>/<id>.sqlite` path;
- no credentials, query, fragment, encoded separator, or traversal segment.

An invalid catalog falls back to Arabic-only. A single invalid entry may be
excluded while valid entries remain available, but the diagnostic must identify
the rejected ID without logging sensitive URLs.

### 7.2 Storage layout and lifecycle

Use a translation-specific byte store so Arabic version cleanup cannot remove
translation files accidentally:

```text
OPFS root: easyquran-translations/
version:   <id>-<contentVersion>/
key:       <id>.sqlite
```

Add `delete(version, key)` and bounded cleanup support to the shared `ByteStore`
contract for both OPFS and IndexedDB implementations.

Rules:

1. Look for the selected exact `(id, contentVersion)` locally.
2. Read the bytes and verify size/SHA before every open. A corrupt local value
   is deleted and treated as a cache miss.
3. If missing and online, download into memory with progress and a strict byte
   ceiling, then verify before `put`.
4. Re-read or otherwise confirm the stored bytes before declaring the pack
   installed; an interrupted store is detected on the next verification.
5. Open and validate SQLite read-only.
6. Mark the new pack installed/active only after validation and a successful
   range smoke query.
7. Close the old translation database, then delete its old versioned bytes.

Steady state stores at most one translation. During a replacement, two verified
packs may coexist briefly. Session-memory fallback does not promise offline
availability and is labeled accordingly.

Use the existing best-effort `navigator.storage.persist()` call. Browser
eviction remains possible; eviction is handled as “download required,” never as
an empty translation.

### 7.3 Download behavior

Extend the existing download helper rather than creating another fetch stack:

- `credentials: "omit"`;
- `redirect: "error"`;
- `Accept-Encoding: identity`;
- bounded timeout and abort signal;
- reject `Content-Length` when present and different from the catalog;
- stream progress and stop immediately if bytes exceed the advertised size;
- verify final size and SHA-256 before persistence/open;
- clear progress and partial memory on abort or failure.

The current helper buffers chunks and then joins them. This is acceptable for
the measured 13 MB maximum, but the implementation must cap total bytes before
allocation and include the largest pack in performance testing.

### 7.4 Worker protocol

Extend `quran/protocol.ts` with explicitly tagged translation operations:

```ts
type TranslationContext = {
  kind: "surah" | "juz" | "page";
  index: number;
  startGlobal: number;
  endGlobal: number;
  keys: VerseKey[];
};

type TranslationWorkerRequest =
  | { type: "translationInstall"; spec: TranslationArtifactSpec }
  | { type: "translationActivate"; spec: TranslationArtifactSpec }
  | { type: "translationRead"; context: TranslationContext }
  | { type: "translationRemove" }
  | { type: "translationStatus" };
```

Worker events distinguish Arabic progress from translation progress by using a
tagged artifact kind, not by overloading `DownloadProgress.script` with an
arbitrary translation ID.

The Worker maintains one optional translation state:

```ts
interface ActiveTranslation {
  spec: TranslationArtifactSpec;
  bytes: Uint8Array;
  database: Database;
  runner: QuranQueryRunner;
  store: "opfs" | "idb" | "session";
}
```

On activate:

- verify bytes and artifact identity;
- open with `sqlite3_deserialize` read-only;
- validate `quran_text` schema and index;
- assert 6,236 rows and boundary coordinates;
- run a known-row query;
- only then replace the active state and close the old database.

On range read:

```sql
SELECT "index", sura, aya, text
FROM quran_text
WHERE "index" BETWEEN ? AND ?
ORDER BY "index";
```

Decode every row into `{ key, surah, ayah, globalIndex, text }` and verify the
complete expected key sequence from the route context before posting it back.
The main thread never receives or handles the full database bytes.

### 7.5 Concurrency and stale work

Worker requests already carry correlation IDs. Add a separate monotonic
translation generation that increments on selection, removal, and navigation.

- A late install may finish its byte verification, but cannot activate after a
  newer selection.
- A late range read cannot populate a different route context.
- Removal waits for the active SQLite handle to close before deleting bytes.
- Multiple tabs may read the same immutable OPFS file, but each tab owns its own
  in-memory SQLite handle and selection state.
- A cross-tab removal that makes local bytes disappear affects the next open;
  an already deserialized in-memory database remains valid for the current tab.

---

## 8. Svelte reader implementation

### 8.1 Domain types and decoders

Add translation-specific types rather than extending Arabic `QuranSourceId`:

```ts
type TranslationDirection = "ltr" | "rtl";

interface TranslationCatalogEntry {
  id: string;
  language: string;
  languageCode: string;
  direction: TranslationDirection;
  name: string;
  translator: string;
  contentVersion: string;
  sizeBytes: number;
  sha256: string;
  downloadUrl: string;
}

interface TranslationAyah {
  key: VerseKey;
  surah: number;
  ayah: number;
  globalIndex: number;
  text: string;
}
```

Catalog JSON and Worker responses are both untrusted boundaries. Rebuild values
field-by-field and verify all ayah coordinates using the existing canonical
coordinate table. A malformed Worker range fails closed rather than rendering
the valid-looking prefix.

### 8.2 Reader translation facet

Add `reader-translations.svelte.ts` to the current composed reader store. It
owns:

- `selectedTranslationId: string | null`;
- catalog state (`idle | loading | ready | error`);
- install state (`not-installed | downloading | installed | error`);
- range state (`idle | loading | ready | offline | error`);
- current download progress;
- installed/active translation metadata;
- a reactive `SvelteMap<VerseKey, string>` for the current range;
- current translation context;
- monotonic selection/navigation token.

Expose focused `ReaderApi` methods:

```ts
loadTranslationCatalog(): Promise<void>;
setTranslation(id: string | null): Promise<void>;
setTranslationContext(context: TranslationContext): void;
retryTranslation(): Promise<void>;
removeInstalledTranslation(): Promise<void>;
translationFor(key: VerseKey): string | undefined;
```

Keep decoded ranges as ordinary arrays and replace the current translation map
only after the whole response validates. Do not make large response arrays
deeply reactive or progressively mutate verse rows during a query.

Persist only `selectedTranslationId` in `easyquran.reader`. This is a backward-
compatible optional field, so keep reader schema version 1. Existing field-wise
decoding safely ignores unknown fields; old blobs default to `null`.

Installed artifact identity comes from the Worker/byte store, not localStorage.
A persisted ID is validated against the live catalog and installed bytes before
activation.

### 8.3 Route integration

Each reader route supplies one exact context:

```text
/app/[surah]       → { kind: "surah", index, startGlobal, endGlobal, keys }
/app/juz/[n]       → { kind: "juz", index, startGlobal, endGlobal, keys }
/app/page/[n]      → { kind: "page", index, startGlobal, endGlobal, keys }
```

Set the context when page data changes. Keep Worker calls out of `VerseRow`; it
consumes synchronous store/prop state only. The same navigation-token pattern
already used for Arabic Worker refreshes guards translation results.

`VerseRow` receives or derives a small translation-line value containing text,
language, direction, and status. `SurahReader` and `RangeReader` retain ownership
of layout and stable keyed ayah lists.

### 8.4 Loading, offline, and error states

- Catalog loading: picker skeleton, Arabic-only reader remains usable.
- Downloading: progress with translated source name, size, cancel action, and
  no partial verse rendering.
- Range loading: subdued per-row skeletons after a short delay.
- Ready: render all verified rows by `VerseKey`.
- Known blank: render the neutral unavailable label.
- Offline with installed pack: read normally from the Worker.
- Offline without installed pack: show one download-required range notice.
- Network/storage/integrity failure: show one range-level message and retry;
  retain Arabic and any previously active valid pack.
- Invalid saved ID: clear selection, persist `null`, and announce that the
  source is unavailable.
- Storage eviction: change installed status to not-installed and offer download;
  never interpret it as a valid empty result.

Use one `aria-live="polite"` range-level status for selection, download, and
error changes. Individual ayahs are not live regions.

### 8.5 Service Worker and privacy

The Service Worker already bypasses paths containing `/translations/`. Keep the
rule and add regression coverage proving catalogs and SQLite packs never enter
Cache Storage. OPFS/IndexedDB byte storage is owned only by the Quran Worker.

Translation selection is a local display preference and needs no auth or remote
persistence. If analytics are later added, record only coarse source/language
selection after consent, never verse text or reading position.

---

## 9. Security, integrity, and licensing

- Treat catalog JSON, CDN headers, SQLite bytes, Worker messages, OPFS/IDB
  values, and persisted selection as untrusted.
- Pin downloads to the configured HTTPS CDN origin and exact versioned prefix.
- Reject redirects, credentials, unexpected queries/fragments, traversal, and
  encoded path separators.
- Bound catalog bytes, SQLite bytes, download time, response arrays, and Worker
  memory.
- Verify exact size and SHA-256 before persistence and before every open.
- Open SQLite from verified bytes as read-only.
- Validate schema, row count, boundary keys, and complete requested ranges.
- Render translation text as plain text only.
- Keep the current CSP and add only the necessary CDN origin to `connect-src`.
- Preserve Tanzil copyright/source headers in mirrored inputs and catalogs.
- Show Tanzil attribution/backlink and translator name while a source is active.
- Complete a production-use review of Tanzil terms before publishing the
  approved catalog. The repository records a non-commercial-use constraint and
  an attribution/backlink requirement for redistributing more than three
  translations.

Licensing review is a release gate, not a post-launch cleanup task.

---

## 10. Testing strategy

### 10.1 Data pipeline

- audit all 115 SQL sources and SQLite outputs;
- exact source-to-SQLite text equality;
- coordinate, count, schema, journal-mode, and integrity gates;
- exact known-empty fixture and failure on drift;
- deterministic approved catalog ordering/version;
- correct versioned URLs, sizes, and SHA-256 values;
- publisher refuses an existing immutable key;
- public `HEAD` and sampled full-download verification.

### 10.2 Worker and storage tests

- strict catalog/artifact decoding and URL-origin/path validation;
- OPFS, IndexedDB, and session install paths;
- local size/hash mismatch causes deletion and redownload;
- network size/hash mismatch never persists or opens;
- schema, row-count, boundary-coordinate, and integrity probes;
- surah, juz, and page range queries return exact ordered keys;
- known empty text returns exactly as stored;
- install/activate/read/remove protocol responses;
- one persistent slot and old-pack removal after successful replacement;
- failed replacement retains the previous valid pack;
- selection/navigation races and stale generation guards;
- cancellation clears progress and does not activate partial bytes;
- storage eviction behaves as a cache miss;
- active handles close before removal;
- largest-pack download/open/query stays within memory and long-task budgets.

### 10.3 Svelte tests

- v1 persistence with absent, valid, invalid, and cross-tab selected IDs;
- catalog, install, download, ready, offline, and error state transitions;
- picker filtering, grouping, keyboard navigation, labels, and focus return;
- progress, cancellation, retry, installed badge, and remove action;
- automatic switch to Ayah-by-Ayah mode;
- LTR and RTL `lang`/`dir` rendering;
- known-empty placeholder without changing source text;
- route changes and rapid source switches never mix rows;
- Arabic-only snapshots remain unchanged;
- Service Worker bypass for catalog and pack URLs.

Run `svelte-autofixer` on every changed `.svelte` file, then run formatting,
type checks, unit tests, and a production web build.

### 10.4 End-to-end acceptance

Test at least one LTR source, one RTL source, the largest source, and one
known-empty source:

1. Open a prerendered surah and select an uninstalled translation.
2. Observe accurate progress, then complete activation and rendering.
3. Navigate next/previous, then to a juz and page.
4. Reload offline and verify Arabic plus the installed translation still work.
5. Switch rapidly between two sources and verify no mixed rows or stale
   activation.
6. Fail a replacement download and verify the previous installed pack survives.
7. Remove the installed source and verify offline translation becomes
   unavailable while Arabic remains functional.
8. Corrupt cached bytes and verify they are rejected and redownloaded.
9. Verify attribution, keyboard behavior, screen-reader status, mobile layout,
   and RTL alignment.

### 10.5 Performance budgets

- Catalog: target below 50 KB gzip for the approved launch set.
- Download: never buffer beyond advertised size; hard cap 16 MB unless the
  verified dataset changes.
- Persistent translation storage: one pack, currently at most about 13 MB.
- Temporary replacement storage: at most two packs, currently about 26 MB.
- SQLite memory: one active translation database in addition to Arabic sources.
- Translation range query: under 20 ms for a 286-ayah surah on a representative
  mid-range phone after open.
- Translation work must not delay Arabic first paint or initial hydration.

---

## 11. Implementation phases

### Phase 0 — freeze policy and align documents

- Remove translation-service architecture from `quran-api.md`.
- Replace live translation request notes in `quran-web-delivery.md` with direct
  versioned SQLite download/offline behavior.
- Align `translation-empty-verses.md` with the neutral placeholder decision.
- Approve the production allowlist, ordering, attribution wording, and licensing
  review.
- Freeze catalog, versioning, storage-slot, Worker, and empty-row contracts.

Exit: no plan requires a translation application service and an approved launch
catalog exists.

### Phase 1 — harden and publish translation artifacts

- Extend verification to every final SQLite file.
- Generate SQLite size, SHA-256, contentVersion, and approved web catalog.
- Change publication to conditional versioned keys.
- Configure CORS and object headers.
- Publish to staging and verify representative full downloads.

Exit: every approved catalog object downloads directly, matches size/SHA, opens
read-only, and cannot be overwritten at its existing key.

### Phase 2 — translation storage and Worker protocol

- Extend `ByteStore` with delete/prune behavior.
- Add translation-specific OPFS/IDB/session caching.
- Add download cancellation, maximum-byte enforcement, and tagged progress.
- Add Worker install, activate, read, status, and remove operations.
- Add schema/row/coordinate validation and single-active-handle lifecycle.

Exit: Worker tests can install an approved pack, read exact surah/juz/page
ranges offline, replace it safely, and remove it.

### Phase 3 — reader state and picker

- Add translation types, catalog decoder/client, reader facet, route contexts,
  and compatible selection persistence.
- Add the accessible searchable/grouped picker, size/progress, installed badge,
  cancellation, removal, and attribution.
- Complete state, storage, race, and Service Worker regression tests.

Exit: a reader can select, download, activate, disable, re-enable, replace, and
remove a translation without stale state.

### Phase 4 — verse presentation and resilience

- Add translation lines to `VerseRow` and source/status UI to reader surfaces.
- Implement LTR/RTL, loading, blank, offline, error, update, and retry states.
- Complete component, accessibility, performance, and end-to-end tests.

Exit: the acceptance flow in §10.4 passes and Arabic-only snapshots remain
unchanged.

### Phase 5 — controlled rollout

- Publish approved production packs and catalog.
- Deploy the web behind a public feature flag.
- Enable a small cohort and observe CDN errors, integrity failures, storage
  failures, memory, long tasks, and Arabic performance.
- Expand only after the observation window passes.
- Add sources later through reviewed catalog updates, not reader code changes.

Exit: delivery, storage, performance, attribution, and Arabic-regression gates
remain healthy through rollout.

---

## 12. Expected file changes

### 12.1 Data pipeline

- `db/quran/tanzil/translations/scripts/sql-to-sqlite.ts`
- `db/quran/tanzil/translations/scripts/verify.ts`
- `db/quran/tanzil/translations/scripts/catalog.ts`
- `db/quran/tanzil/translations/scripts/upload-sqlite.ts`
- `db/quran/tanzil/translations/scripts/lib.ts`
- `db/quran/tanzil/translations/package.json`
- new approved-ID and known-empty fixtures
- generated `db/quran/tanzil/translations/catalog.v1.json`

### 12.2 Web data and Worker

- `web/src/lib/config/site.ts`
- new `web/src/lib/quran/translations/types.ts`
- new `web/src/lib/quran/translations/wire.ts`
- new `web/src/lib/quran/translations/catalog.ts`
- `web/src/lib/quran/protocol.ts`
- `web/src/lib/quran/worker-client.ts`
- `web/src/lib/workers/download.ts`
- `web/src/lib/workers/storage.ts`
- `web/src/lib/workers/opfs-cache.ts` or a translation-specific sibling
- `web/src/lib/workers/quran.worker.ts`
- focused catalog, Worker, storage, integrity, and race tests

### 12.3 Reader

- new `web/src/lib/stores/reader-translations.svelte.ts`
- `web/src/lib/stores/reader-core.svelte.ts`
- `web/src/lib/stores/reader-persistence.svelte.ts`
- `web/src/lib/stores/reader.svelte.ts`
- new picker/status components under the reader directory
- `ReaderShell.svelte`, `SurahReader.svelte`, `RangeReader.svelte`, and
  `VerseRow.svelte`
- surah, juz, and page route components for translation contexts
- `web/static/sw.js` regression coverage; behavior remains bypass-only
- CSP/environment configuration for the catalog/CDN origin
- reader persistence, component, accessibility, and end-to-end tests

### 12.4 Documentation

- `docs/quran-api.md`: remove future translation-service sections
- `docs/quran-web-delivery.md`: document direct SQLite download and offline use
- `docs/translation-empty-verses.md`: record the chosen UI behavior
- `db/quran/tanzil/translations/README.md`: document versioned catalog/publishing
- `.env.example` and deployment docs: catalog URL and web feature flag

Do not modify Arabic SQLite, Arabic source-view normalization, Arabic search, or
Arabic content routes unless a failing regression proves it necessary.

---

## 13. Release and rollback

Release order is versioned SQLite objects → mutable catalog → web flag.

Rollback:

1. Disable the web translation flag; Arabic behavior returns immediately.
2. Restore the previous catalog if a new entry is invalid. Its versioned SQLite
   object remains immutable and recoverable.
3. Never overwrite or delete a bad pack in place. Publish corrected bytes under
   a new content version and catalog version.
4. Leave older objects available for at least the supported web-build window
   before a separate retention task considers removal.

Client rollback requires no data migration: an installed older pack remains
readable, and disabling the feature leaves it inert until the user removes it
or storage is evicted.

No rollback step touches Arabic artifacts, bookmarks, notes, or reading
progress.

---

## 14. Definition of done

Translations are complete for the first release when:

- all 115 SQLite packs pass source fidelity and coordinate validation;
- approved packs have immutable versioned URLs, exact sizes, and SHA-256;
- the public catalog is deterministic, allowlisted, attributed, and validated;
- a corrupt/truncated/wrong pack can never persist as installed or render;
- the Worker can install, query, replace, reopen offline, and remove one pack;
- a user can select, switch, disable, persist, retry, and remove a translation
  across surah, juz, and page views;
- LTR, RTL, largest-pack, and known-empty cases render accessibly;
- no translation response uses an application translation service;
- translation bytes live only in the Quran byte store/session memory, never
  Cache Storage or app-state localStorage;
- Arabic prerendering, offline reading, search, copy/share, annotations, and
  navigation remain unchanged;
- attribution and production-use review are signed off;
- staged rollout and rollback have both been exercised.

---

## 15. Deferred decisions

1. **Production allowlist and ordering** — required in Phase 0. Use
   `en.sahih` for development only until review is complete.
2. **Public default translation** — this plan fixes Arabic-only for the first
   release; revisit locale-based defaults after measuring opt-in.
3. **More than one installed pack** — increase only with an explicit storage
   manager and quota/eviction UX.
4. **Copy/share with translation** — requires an explicit control and
   attribution format.
5. **Translation search** — may scan one installed pack or require a versioned
   index; it needs its own normalization and highlighting contract.
6. **Multiple simultaneous translations** — requires a new layout, memory, and
   attribution design.
7. **Background/preselected downloads** — do not spend bandwidth or storage
   before a user selects a source.
8. **Translated SEO pages** — requires stable URL, language, canonical, and
   `hreflang` policy and is not implied by client-side translation rendering.
