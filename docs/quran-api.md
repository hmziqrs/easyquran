# EasyQuran — Quran Content API (design plan)

> Status: **plan** (not yet implemented). Scope: MVP serves **Arabic text** by surah /
> juz / page / single ayah. **Translations** are a designed-but-deferred extension —
> the `?translations=` param and dynamic pack loader are specified here so they slot in
> later without an API redesign.
>
> Aligns with `ROADMAP.md` → *Rust backend → Quran content API (surahs, ayahs, translations)*.
>
> Every claim in §2 marked **(verified)** was checked against the files on disk on
> 2026-07-30; the check commands live in §14 so they can be re-run when the data changes.

---

## 1. Goals & scope

- Read-only Quran content API on the existing Axum backend (`rust/backend/api`).
- Fetch by **surah**, by **juz**, by **page** (Madani mushaf), and **individual ayah**
  (by `sura/aya` and by verse-key `2:255`).
- Serve **Arabic** (Uthmani + simple-clean) from the data we already have — **no rebuild**.
- Wire the web reader (`web/`) off the hardcoded sample surahs onto this API. The
  `Surah` / `VerseKey` *type* contract is preserved; four call sites that assume
  `Surah.verses` is populated do change (§11.1) and the rendered script changes (§11.0).
- Leave a clean seam to add **translation packs** later (one `.sqlite` per translation,
  loaded dynamically).

### License note (non-commercial)

Tanzil's terms are **non-commercial use; attribution + tanzil.net backlink required when
redistributing more than three translations.** EasyQuran is not monetized today, so this is
fine for the Arabic text and any translations we ship now. **Revisit the moment billing /
accounts / paid features go live** — at that point either confirm continued non-commercial
use in writing, obtain a commercial arrangement with Tanzil, or switch translations to a
permissively-licensed dataset. Surface attribution (tanzil.net backlink) in the reader
regardless.

---

## 2. Source data (verified on disk)

| Asset | Path | Shape |
|---|---|---|
| Arabic — Uthmani | `db/quran/tanzil/arabic/quran-uthmani.sqlite` (1.6 MB) | `quran_text("index" PK, sura, aya, text)`, idx `idx_quran_text_sura_aya(sura,aya)`, 6236 rows |
| Arabic — simple-clean | `db/quran/tanzil/arabic/quran-simple-clean.sqlite` (929 KB) | identical schema |
| Metadata | `db/quran/tanzil/quran-data.xml` (77 KB) | see the element table below |
| Translations (future) | `db/quran/tanzil/translations/sql/*.sql` (115 dumps) | `index, sura, aya, text` each; catalog in `translations/index.json` |

**Load-bearing invariant (verified):** `quran_text."index"` is the **global ayah index**
1..6236, contiguous, zero gaps (`count = max = 6236`). It equals
`sura.start(0-based, from XML) + aya(1-based)`: surah 1 → 1..7, surah 2 → 8..293,
surah 3 → 294…, surah 114 aya 6 → 6236.

**Consequences — every query is cheap:**
- Surah text: `WHERE sura = ? [AND aya BETWEEN ? AND ?] ORDER BY aya` (uses the index).
- Single ayah: `WHERE sura = ? AND aya = ?`.
- Juz / page / ruku / hizb-quarter: `WHERE "index" BETWEEN ?start AND ?end` — a PK range
  scan, O(range size). Start/end globals are computed once at boot from the XML (see §4).

### 2.1 Metadata elements present in `quran-data.xml` (verified counts)

| Element | Count | Attributes | Used by |
|---|---|---|---|
| `<sura>` | 114 | `index ayas start name tname ename type order rukus` | surah meta |
| `<juz>` | 30 | `index sura aya` | `/juzs` |
| `<page>` | 604 | `index sura aya` | `/pages` (Hafs/Madani) |
| `<ruku>` | 556 | `index sura aya` | ruku markers |
| `<quarter>` | 240 | `index sura aya` | hizb quarters → 60 hizbs, 8 quarters/juz |
| `<manzil>` | 7 | `index sura aya` | manzil markers |
| `<sajda>` | 15 | `index sura aya type` | `type ∈ {recommended, obligatory}` |

Every navigational element except `<sura>` and `<sajda>` shares the identical
`(index, sura, aya)` start-marker shape, so **one generic parser + one `Range` builder
covers juz, page, ruku, quarter and manzil**. The previous revision of this doc modelled
only juz and page; rukus were stored as a per-surah count with no ranges, and sajdas were
parsed but never exposed. Both are corrected in §4/§5.

### 2.2 Script characteristics (verified — these drive §6, §9 and §11)

| Property | `quran-uthmani` | `quran-simple-clean` |
|---|---|---|
| NFC-normalized | **no** | yes |
| alef-wasla U+0671 | 4829 rows | 0 |
| superscript alef U+0670 | 4421 rows | 0 |
| tatweel U+0640 | 3698 rows | 0 |
| harakat | full | none |
| longest ayah | 1208 chars (2:282) | 679 chars |
| leading/trailing whitespace | none | none |

Surah 2 in Uthmani is 286 rows / ~107 KB of UTF-8 text — the largest single-surah payload.
Juz 30 is 564 ayahs, juz 1 is 148; this bounds the pagination defaults in §5.

**Bismillah characteristic (verified — and *not* uniform):** every surah except surah 9
carries the basmala at the start of ayah 1. For surah 1 the basmala *is* ayah 1. For the
other 112 it is a prefix on ayah-1 text — but in Uthmani that prefix is **not
byte-identical across all 112**:

- **110 surahs** use `بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ ` (39 chars + space), identical to 1:1.
- **Surahs 95 (At-Tin) and 97 (Al-Qadr)** use `بِّسْمِ …` — ب + U+0651 **shadda** + U+0650 kasra.
  Longest-common-prefix with 1:1 is **1 character**.
- `quran-simple-clean` *is* uniform: all 112 use `بسم الله الرحمن الرحيم `.

A naive exact-prefix strip therefore leaves a **doubled basmala on At-Tin and Al-Qadr**,
and a boot assertion phrased as "the prefix is present on exactly 112 surahs" would
**abort boot**. Handled correctly in §6.

Note also that the basmala appears **mid-ayah** at 27:30 (`إِنَّهُۥ مِن سُلَيْمَـٰنَ وَإِنَّهُۥ بِسْمِ ٱللَّهِ …`).
Any stripping must be anchored to the start of aya 1 — never a substring replace.

---

## 3. Data-layer decision: use the existing `.sqlite` directly

The two Arabic `.sqlite` files are **already the right shape** and are opened read-only as-is.
There is **no `quran.db`**, no consolidation build, no copy step. The only thing not already
in a database is metadata, and that is small enough to hold in memory:

- **Arabic** → `sqlx::SqlitePool` per script (Uthmani, simple-clean), opened
  `read_only(true)` + `query_only=1` (see §7 for the pragma set actually worth setting).
  Decoupled from the app's mutable `easyquran.db` (`sea_db`).
- **Metadata** → parsed from `quran-data.xml` **once at boot** into in-memory Rust structs.
  A `quick-xml` parse of a 77 KB file is milliseconds. No file, no build, no `Migrator`.
- **Translations (future)** → one `<id>.sqlite` pack per translation, served by an LRU of
  lazy-opened read-only pools. See §8.

---

## 4. Metadata model (in-memory, derived from XML at boot)

```rust
struct SuraMeta {
    index: u16,            // 1..114
    ayas: u16,
    start_global: u32,     // xml@start + 1
    end_global: u32,       // start_global + ayas - 1  (sura 114 -> 6236)
    revelation_order: u16,
    ruku_count: u16,       // xml@rukus — a COUNT, distinct from the ruku ranges below
    place: Place,          // Meccan | Medinan
    name_arabic: String,
    name_translit: String, // xml tname
    name_english: String,  // xml ename
    slug: String,          // see "Slugs" below
    bismillah: Bismillah,  // FirstAyah (sura 1) | None (sura 9) | Header (others)
}

/// One generic range type for every `(index, sura, aya)` start-marker element.
struct Range { index: u16, start_global: u32, end_global: u32, start_sura: u16, start_aya: u16 }

struct Sajda { index: u16, sura: u16, aya: u16, global_index: u32, kind: SajdaKind } // Recommended | Obligatory

struct QuranMeta {
    suras:    [SuraMeta; 114],
    juzs:     Vec<Range>,   // 30
    pages:    Vec<Range>,   // 604
    rukus:    Vec<Range>,   // 556
    quarters: Vec<Range>,   // 240 hizb quarters (4 per hizb, 8 per juz)
    manzils:  Vec<Range>,   // 7
    sajdas:   Vec<Sajda>,   // 15
}
```

**Range math (pinned to avoid off-by-one):** for a marker starting at `(start_sura,
start_aya)`, `start_global = suras[start_sura].start_global + (start_aya - 1)`;
`end_global = next.start_global - 1`, last → 6236. Assert at boot that **each** of the five
range families **tiles `[1, 6236]`** with no gaps/overlaps, and that the counts are exactly
30 / 604 / 556 / 240 / 7 / 15; fail boot otherwise. These assertions are also unit tests
(§13) so a bad data swap fails CI, not just production boot.

**Slugs.** The web already uses slugs like `al-fatihah` for its deep links (`/app/[surah]`).
The XML's `tname` (`Al-Faatiha`) transliterates differently, so slugs are **not** derived
blindly. Ship a committed `web/src/lib/data/surah-slugs.json` (114 entries) — sourced from
the existing 11 web slugs where present, stable kebab-case otherwise — that both the API
metadata load and the web catalog consume. Boot-assert 114 entries, all unique.

---

## 5. API surface — `modules/quran_v1/`

Public, read-only, no auth. Mirrors `category_v1`: `mod.rs` → `pub fn routes() ->
Router<AppState>`; nested at `/quran/v1` in `router.rs`. DTOs are `camelCase` to match
`web/src/lib/data/quran.ts`.

**Deliberate deviation from house convention:** every other list/query endpoint in this
codebase is `POST /<domain>/v1/list/query` (`category_v1`, `search_v1`). This module is
**all-GET** — the entire point is edge-cacheability (§10), which a POST cannot have.
Confirm at implementation time that `static_csrf` does not gate these GETs.

| Method + path | Returns |
|---|---|
| `GET /quran/v1/suras` | all 114 surah meta (small, fully cacheable) |
| `GET /quran/v1/suras/{num}` | one surah meta |
| `GET /quran/v1/suras/{num}/ayahs?script=&from=&to=&translations=` | surah text — Arabic (+ future translations) per ayah |
| `GET /quran/v1/juzs` · `/juzs/{num}` | 30 juz meta / a juz's ayahs (`?cursor=&limit=`) |
| `GET /quran/v1/pages` · `/pages/{num}` | 604 Madani pages / a page's ayahs |
| `GET /quran/v1/rukus` · `/hizbs` · `/manzils` | the remaining range families (meta only; ayah fetch via `/ayahs?from=&to=` global range) |
| `GET /quran/v1/sajdas` | 15 sajda markers with `kind` (recommended \| obligatory) |
| `GET /quran/v1/scripts` | `uthmani`, `simple-clean` + the **data version** (§10) |
| `GET /quran/v1/translations` | catalog (empty until packs ship; future) |
| `GET /quran/v1/ayah/{sura}/{aya}` · `/ayah/{key}` (`2:255`) | single ayah |
| `GET /quran/v1/random?seed=<iso-date>` | verse-of-the-day (deterministic → cacheable); `?seed=random` non-cacheable |
| `GET /quran/v1/search?q=&scope=&limit=` | Arabic FTS (LIKE fallback) |
| `POST /quran/v1/admin/reload` | rebuild in-memory registry / clear pools (admin-gated; future, for packs) |

**Relationship to the existing `search_v1` module.** `POST /search/v1/search` is the
blog/content search over `sea_db`. It is deliberately *not* extended: it queries a different
store, uses POST, and returns a different result shape. `/quran/v1/search` is a separate,
GET-cacheable, corpus-specific endpoint. Cross-linking the two in a unified result set is
explicitly out of scope.

**Query params.**
- `script=uthmani|simple-clean` (default `uthmani`).
- Surah endpoints use within-surah `from`/`to` (1-based).
- Juz/page/ruku ranges paginate on `cursor` (exclusive global index) + `limit` because they
  cross surah boundaries. **`limit` default 300, max 600** — juz 30 is 564 ayahs, so the
  previous default of 50 meant 12 round-trips for the most-read juz. A page (604 of them,
  ~10 ayahs each) always fits in one response.
- `translations=` (CSV) is parsed, **deduped + sorted**; if the client sent an
  unsorted/duplicated list the response is a **308** (not 301 — 301 is permanent, method-
  rewriting, and aggressively cached per-URL by browsers) to the canonical order so the edge
  shares one cache key. Cap 8 packs/request (future).
- `seed` on `/random` accepts an ISO date **bounded to `[today-370d, today+1d]`**; anything
  else → `InvalidValue`. Unbounded seeds would mint unbounded edge-cache entries.

**Response shapes.**

```rust
struct Ayah {
    key: String,                 // "2:255"
    sura: u16, aya: u16,
    global_index: u32,
    text: String,                // Arabic in requested script (bismillah handled per §6)
    sajda: Option<SajdaKind>,    // set on the 15 sajda ayahs
    translations: BTreeMap<String, String>, // empty until packs ship
}
struct SurahEnvelope { surah: SurahMeta, ayahs: Vec<Ayah>, page: PageMeta, dataVersion: String }
struct PageMeta { total: u32, limit: u32, cursor: Option<u32>, has_more: bool }
```

**Handler fan-out (future, with translations):** `futures::join!(arabic_fut,
join_all(translation_futs))`, seed a `BTreeMap<global_index, Ayah>` from Arabic, fold each
pack in → keeps mushaf order; latency ≈ slowest single query. MVP (Arabic only) is a single
query.

**Errors** reuse the existing wire-locked `ErrorCode` enum (verified: `RecordNotFound`,
`InvalidValue`, `InvalidInput` all exist; no changes needed): unknown sura/juz/page →
`RecordNotFound` (404); unknown script / out-of-range seed → `InvalidValue` (400); bad
params → `InvalidInput` (400).

**Rate limiting.** All deterministic reads unmetered (indexed seeks, like
category/tag/feed). Meter only `/search` and `/random?seed=random`, at
**`rate_limit_layer(&state, 30, 60)`** — matching the existing `search_v1` precedent rather
than inventing a second number. Per-IP resolution comes from `middlewares/client_ip`
(Cloudflare/Traefik-aware as of 3496bb6). `/random` default uses a **deterministic** seed
(`global_index = (days_since_epoch % 6236) + 1`), so it is a sub-ms PK seek and
edge-cacheable; never `ORDER BY RANDOM()` for the default path. That modulo weights
selection by ayah length (Al-Baqarah alone is 4.6% of all ayahs) and cycles every ~17
years — acceptable for a verse-of-the-day, noted here so it isn't mistaken for uniform.

**CORS.** `middlewares/cors.rs::origin_guard` rejects any request whose `Origin` is not in
the allowlist. A public, no-auth content API is unusable by third parties under that guard,
so `/quran/v1` **must explicitly opt out** — either mounted outside the guard or with a
permissive `Access-Control-Allow-Origin: *`. This is a decision, not an oversight: pick one
before Phase 1 ships. Whichever is chosen, it interacts with edge caching — see §10's `Vary`
requirement.

---

## 6. Bismillah handling

Tanzil embeds the basmala as a prefix on ayah-1 text for 112 surahs. The web currently
renders a separate basmala header and stores ayah-1 as bare text, so naively serving raw
Tanzil would **double the basmala** on surah pages and glue one mid-stream at each surah
start inside a juz/page.

**Decision:** strip the prefix **at the API response layer** (not in the source files, not
in the web).

Per §2.2 the prefix is **not** byte-identical in Uthmani — 95 and 97 carry a shadda on the
ba. So the match must be diacritic-tolerant, and the constant must not be hardcoded:

1. **Derive, don't hardcode.** At boot, read `sura=1 aya=1` from each script's pool. That is
   the canonical basmala for that script. A hardcoded Rust string literal is one editor save
   or `.gitattributes` filter away from silently ceasing to match — and Uthmani is **not
   NFC-normalized**, so the literal's exact bytes are not something the source file can be
   trusted to preserve.
2. **Match on a skeleton.** Compare `strip_marks(candidate_prefix) == strip_marks(basmala)`,
   where `strip_marks` drops Unicode combining marks (`Mn`). This matches all 112 including
   95/97, and is insensitive to the NFC question entirely.
3. **Strip by the matched length in the original string**, so the remaining text keeps its
   original bytes untouched.

Rule while building each `Ayah`: if `aya == 1 && sura != 1 && sura != 9`, apply the above.
Surah 1 ayah 1 stays the basmala (it *is* ayah 1); surah 9 has none. Anchor to the string
start only — 27:30 contains the basmala mid-ayah and must be left alone. The web's existing
`showsBismillah` / header logic is then unchanged. Expose `surah.bismillah` in the meta so
clients can render the header correctly.

**Boot assertion (corrected):** for each script, exactly **112** surahs (all but 1 and 9)
must skeleton-match the derived prefix, and surah 9's ayah 1 must **not**. Fail boot
otherwise. The previous phrasing — "the prefix is present on exactly those 112" under an
exact-byte match — would have aborted boot on the real data.

---

## 7. Runtime — `src/db/quran_store.rs`

```rust
pub struct QuranStore {
    arabic: EnumMap<Script, SqlitePool>,   // uthmani, simple-clean — always open
    meta: QuranMeta,                       // in-memory, per §4
    basmala: EnumMap<Script, String>,      // derived at boot per §6
    data_version: String,                  // per §10
    packs: RwLock<HashMap<String, PackMeta>>,        // future
    open_packs: Mutex<LruCache<String, SqlitePool>>, // future: 32-cap LRU
    settings: QuranSettings,
}
```

- Add `pub quran: Arc<QuranStore>` to `AppState` (`src/state.rs`); construct after `sea_db`
  in `main.rs`; **abort boot on failure** (missing/corrupt arabic sqlite, bad XML, failed
  §4/§6 assertions), matching the `sea_db`/settings fail-loud precedent.
- Pool lib: `sqlx`. sea-orm 1.1.2 already pulls it transitively; add it as a **direct dep
  pinned to the exact version in `rust/Cargo.lock`** so `Pool` types unify. If that pin ever
  drifts from sea-orm's, cargo links **two** SQLite stacks — precisely what this choice is
  meant to avoid — so add a CI check asserting a single `libsqlite3-sys` in the tree. No
  `rusqlite`. `lru` for the pack cache, `quick-xml` for metadata.
- **Pragmas that actually matter:** `read_only(true)` + `query_only=1`. Drop
  `journal_mode=OFF` (a no-op on a read-only handle) and size `mmap_size` to the file
  (~2 MB), not 256 MB — these files are 1.6 MB and 929 KB. `cache_size` is likewise
  irrelevant at this size; the whole corpus lives in page cache after the first query.
- **Do NOT use `immutable=true`.** It disables SQLite's change-counter check, so a bad swap
  or bit-rot would silently serve corrupted Arabic with no error — unacceptable for sacred
  text. `read_only(true)` gives the no-sidecar / read-only-volume properties without that
  cost.
- **Integrity is not optional here.** Compute a sha256 over each Arabic file at boot; it
  feeds `data_version` (§10) and gives a cheap tamper signal. This is the mechanism that
  makes "content can be corrected" (§10) observable to clients.
- **Cold-pack open (future):** reserve an LRU slot under the mutex, construct the pool +
  validate **outside** the lock, publish via `OnceCell` so a cold open never head-of-line-
  blocks other translation requests. Set an explicit `acquire_timeout(2-3s)` → 503-fast-fail.

**Config** (`src/config/settings.rs`): `QuranSettings { arabic_uthmani_path,
arabic_simple_path, metadata_xml_path, pack_dir (future), pack_lru_cap (default 32),
pack_pool_conns (default 16) }`. Bound `pack_lru_cap ≤ 115` with a hard ceiling; fail boot
if an env knob exceeds it.

---

## 8. Translations — future extension (no work for MVP)

Verified about the catalog: 115 entries, **all** with `ayaCount: 6236`, unique ids, 44
distinct language codes, and a nested `file.sha256` per dump.

When ready:

1. **Build packs** — extend `db/quran/tanzil/translations/scripts/` with a `build.ts` that
   streams each SQL dump (robust apostrophe/newline unescaping — the current `lib.ts`
   intentionally discards verse text and must not be reused for content) into
   `translations/packs/<id>.sqlite` with `verses(global_index PK, sura, aya, text)` + a
   single-row `pack_meta` (id, language, direction, names, `aya_count==6236` gate,
   `source_sha256`, license, source_url), plus a sidecar `packs/manifest.json`. Verification
   gates: `count==6236`, `sha256(source .sql) == index.json@file.sha256`, global-index
   contiguity.
2. **Serve packs** — the `open_packs` LRU opens `<id>.sqlite` read-only on demand. A request
   with `?translations=en.sahih,id.pickthall` fans out across the arabic pool + those pack
   pools concurrently and merges by global index.
3. **Add packs without redeploy** — registry rebuilt at boot and via `POST
   /quran/v1/admin/reload` (admin-gated). Swap protocol: write `<id>.sqlite.new` → fsync →
   `rename` → `admin/reload` (never `cp`-over in place). A pack swap bumps `data_version`.

> Search across translations needs build-time Arabic normalization before it is useful —
> see §9.

---

## 9. Search

`/search` defaults to **Arabic** (scope=`arabic`). Honest caveat: FTS5's
`unicode61 remove_diacritics 2` does **not** strip Arabic harakat on the real libsqlite3, and
Uthmani uses alef-wasla (`ٱ`, U+0671, in 4829 rows) which never matches regular alef (`ا`,
U+0627). Naively indexing Uthmani would make recall near-zero. So:

- Build the FTS index over **simple-clean**, which is already undiacriticated and uses
  regular alef throughout.
- **The normalization set is script-specific.** The previous revision prescribed
  "alef-maqsura/tatweel normalization" for simple-clean — but simple-clean contains **zero
  tatweel and zero alef-wasla** (verified). What it actually needs is orthographic folding:
  - hamza forms → bare alef (`أ إ آ` → `ا`)
  - alef-maqsura → ya (`ى` → `ي`)
  - ta-marbuta → ha (`ة` → `ه`)
  - strip any residual combining marks
  If Uthmani is ever indexed instead, *add* alef-wasla → alef, superscript-alef removal, and
  tatweel removal on top.
- Apply the **same** normalization to the query. Keep it in one function, used by both the
  indexer and the query path, with a property test asserting idempotence.
- Add a runtime probe at boot: index a known sample, confirm an undiacriticated query
  matches before advertising FTS; else fall back to `LIKE`.
- This remains **exact-token** matching, not root/stem (that needs the ICU tokenizer, which
  means flipping sqlx-sqlite to `bundled` + `-DSQLITE_ENABLE_FTS5` — a future hardening step).
- Results are returned as global indices; the text is then read from the **requested**
  script, so searching simple-clean still renders Uthmani.

Translation search reuses each pack's own FTS5 table (English tokenizer is fine for most).

---

## 10. Caching & data versioning

**`dataVersion`** — a short hash over the sha256s of the loaded Arabic files + the XML +
(future) the pack manifest, computed at boot (§7). It is returned on `/scripts` and in every
envelope, and is a component of every ETag. Without it, a text correction is invisible to
clients and the edge.

**ETag = hash of the response body** (or equivalently `dataVersion` + canonical query). The
previous revision hashed *only the canonical sorted query* — that is a cache **key**, not a
validator: a corrected ayah would ship under an unchanged ETag and clients would 304 onto
stale text indefinitely.

**Headers.** Deterministic GETs (`/suras`, `/juzs`, `/pages`, `/rukus`, `/hizbs`,
`/manzils`, `/sajdas`, `/scripts`, surah/ayah by key, `/random` default):
`Cache-Control: public, max-age=86400, s-maxage=3600` + `ETag`. **Avoid `immutable` and long
`s-maxage`** — content can be corrected, so let the edge revalidate within an hour.

**`Vary` is mandatory.** Whatever CORS decision §5 lands on, if the response carries
`Access-Control-Allow-Origin` reflecting the request `Origin`, the response **must** send
`Vary: Origin` or Cloudflare will serve one origin's CORS headers to another. Also
`Vary: Accept-Encoding` (tower-http gzip is on globally). If a wildcard ACAO is chosen
instead, `Vary: Origin` is unnecessary — another reason to settle §5's CORS question early.

Support conditional requests (`If-None-Match` → 304) and `HEAD` on all cacheable routes.

Not edge-cached (`private, no-store`): `/search?q=`, `/random?seed=random`.

Backend sits behind Cloudflare + Traefik, so cache hits are effectively free after the first.

---

## 11. Web integration (`web/`)

### 11.0 The script change is a visible regression — decide it first

The reader's current hardcoded verses are **simple-enhanced / imlaei** style:
`بِسْمِ اللَّهِ الرَّحْمَٰنِ الرَّحِيمِ` — plain alef U+0627, full harakat, no alef-wasla.
**Neither script on disk is that.** Uthmani adds alef-wasla, superscript alef and tatweel;
simple-clean removes all diacritics.

So swapping to `script=uthmani` preserves the TypeScript types but **changes the rendering of
every ayah** — glyph shaping, line breaking, font fallback and the reader's current
letter-spacing/line-height tuning all shift. Options, to be decided before Phase 2:

1. **Extract a third `quran-simple-enhanced.sqlite`** from Tanzil (same schema, same
   pipeline as the existing two) and default the reader to it — pixel-closest to today.
2. **Ship Uthmani and re-tune typography** as an explicit, budgeted Phase-2 task, with an
   Uthmani-capable font (KFGQPC / Amiri Quran) since alef-wasla and superscript alef render
   poorly in general-purpose Arabic faces.

Option 1 is the lower-risk default; option 2 is the better long-term reader. Either way this
is a deliberate product decision, not an implementation detail.

### 11.1 `Surah.verses` — the four call sites that break

The plan is `SURAHS = CATALOG.map(c => ({...c, verses: []}))`. Empty `verses` arrays silently
break four live consumers; an optional `ayahCount?` field fixes **none** of them. Each needs
an explicit edit:

| Site | Breakage | Fix |
|---|---|---|
| `_reader/Sidebar.svelte:141` | `{#each current.verses}` → empty verse jump-list | iterate `ayahCount` and render numbers; text lazily from the reader cache |
| `lib/data/quran.ts:268` `surahMeta` | renders "0 verses" | read `ayahCount`; drop the now-meaningless `partial` flag |
| `lib/data/quran.ts:251` `searchVerses` | pushes *inside* `s.verses.forEach`, so a name/number hit yields **zero results** — the "instant catalog-only matcher" stops working | restructure: emit one catalog hit per matching surah, independent of `verses` |
| `lib/stores/reader.svelte.ts:221,272` | verse-text lookups → empty bookmarks/copy/share | read from the new `#loaded` cache, with an async fetch-on-miss |

### 11.2 Prerender vs. live fetch — the doc's previous claim was wrong

`[surah]/+page.ts` currently sets `export const prerender = true`. With prerender on, `load`
runs **at build time only**; client-side navigation reads the baked `__data.json` and never
re-hits the API. The previous revision claimed load runs "in-browser on client nav" — it does
not. That also means prerendered surah content is **frozen until redeploy**, which
contradicts §10's "content can be corrected, revalidate within an hour": the HTML has no such
revalidation path.

Resolve explicitly. Recommended: **keep `prerender = true`** (real Arabic in the HTML is the
whole SEO argument) and accept build-frozen text, with a redeploy as part of the
data-correction runbook. Document that a content fix requires a web rebuild, not just a
backend swap. If freshness ever outranks SEO, flip to `ssr` + client fetch and drop the claim.

### 11.3 The rest

- **`web/src/lib/data/quran.ts`** stays the contract + pure helpers. Replace the 11 inline
  verse arrays with a **build-fetched 114-entry catalog** (`web/scripts/fetch-catalog.ts` →
  `GET /quran/v1/suras` → committed `quran-catalog.generated.ts`). Add `ayahCount: number`
  (required, not optional). Keep a tiny Al-Fatihah fallback so the build never hard-fails
  offline.
- **`web/src/lib/quran/client.ts`** — `quranClient.surah(num, {script})`, `.verse(num,aya)`,
  `.juz(n)`, `.verseOfTheDay(isoDate)` (`/random?seed=`). `apiBase()` resolves
  `PUBLIC_API_BASE_URL` (absolute for SSR/prerender, same-origin relative in-browser behind
  the CF/Traefik rewrite).
- **`[surah]/+page.ts`** — add `load` → `quranClient.surah(meta.num)`; unknown slug →
  SvelteKit 404; `entries()` grows from 11 to all 114 automatically once `SURAHS` does.
- **`[surah]/+page.svelte`** — consume `data.surah`; add a `browser`-guarded `$effect` that
  refetches when `reader.script` changes (skip the initial run → no flash). With §11.2's
  prerender decision this effect is also the only live-fetch path, so it must handle error
  and loading states, not just the happy path.
- **Reader store** — persist `script`; add an in-memory `#loaded` verse cache so
  bookmarks/copy/share/player keep returning real verse text (see §11.1).
- **Verse-of-the-day** — deterministic by UTC date, fetched client-side after mount,
  edge-cacheable.
- **Search** (`Results.svelte`) goes **async**: keep `searchVerses` as the instant
  catalog-only (name/number) matcher for zero-latency — after the §11.1 restructure — and add
  a debounced `/search` with loading/empty states.
- **Attribution** — render a visible tanzil.net backlink in the reader footer (license
  obligation once >3 translations ship; harmless to add now).

---

## 12. Phased plan

**Phase 0 — Backend store + metadata (Arabic).**
`Cargo.toml` (add `sqlx` pinned to `rust/Cargo.lock`, `lru`, `quick-xml`) + the single-
`libsqlite3-sys` CI check; `src/db/quran_store.rs`; generic `(index,sura,aya)` XML parser
covering all five range families + sajdas; derived basmala constants (§6); `data_version`
sha256 (§7); `src/config/settings.rs` (`QuranSettings`); `src/state.rs`; `main.rs`
(construct, fail-loud boot). *No routes yet.* Deliverable: backend boots, opens both arabic
`.sqlite` read-only, all §4/§6 assertions green.

**Phase 1 — `quran_v1` module + routes.**
`modules/quran_v1/{mod,controller,validator,dto}.rs`; nest in `router.rs`; **CORS opt-out
decision applied**; metered `/search` + `/random?seed=random` sub-nest at 30/60; bismillah
strip in the DTO builder; cache headers + body-hash ETag + `Vary`; conditional requests +
HEAD; OpenAPI annotations behind the existing `openapi` feature. Deliverable: all Arabic
endpoints live and documented.

**Phase 2 — Web swap.**
§11.0 script decision applied first; `web/scripts/fetch-catalog.ts` + generated catalog; the
four §11.1 refactors; `web/src/lib/quran/client.ts`; `[surah]/+page.ts` load; `+page.svelte`
script-switch `$effect` with error/loading states; reader-store `#loaded` cache + persisted
`script`; verse-of-the-day; `PUBLIC_API_BASE_URL`. Deliverable: reader renders all 114
surahs from the API with no visual regression against the §11.0 decision.

**Phase 3 — SEO + async search + attribution.**
Drop `noindex` on surah pages + real `<title>`/description + JSON-LD; sitemap entries for
`/app/<slug>`; async `Results.svelte`; tanzil backlink; consider `precompress:true`.

**Phase 4 (future) — Translations.**
Pack builder (`build.ts`), `open_packs` LRU, `?translations=` fan-out, `/translations`
catalog, `admin/reload`, FTS5 hardening (`bundled` + normalization/ICU).

---

## 13. Testing

The repo already has `tests/` and `src/test_utils/`; these belong there, not only as boot
assertions:

- **Data invariants** (§2, §4): index contiguity 1..6236; `sura.start + aya == index` for all
  6236; each of the five range families tiles `[1,6236]`; counts 114/30/604/556/240/7/15.
- **Bismillah** (§6): all 112 skeleton-match per script; 9:1 does not; 1:1 is left intact;
  27:30 is left intact; 95:1 and 97:1 strip correctly — these two are the regression test
  for the bug this revision fixes.
- **Slugs**: 114 entries, unique, and every one of the 11 pre-existing web slugs preserved.
- **Normalization** (§9): idempotence property test; a known undiacriticated query matches
  its Uthmani source ayah.
- **HTTP**: 404 on sura 0/115, juz 31, page 605; `InvalidValue` on unknown script and
  out-of-range seed; ETag changes when `data_version` changes; 304 on `If-None-Match`;
  `?translations=b,a` → 308 to `a,b`.

---

## 14. Verification commands

Re-run these whenever the source data changes; they are the basis for every **(verified)**
claim above.

```sh
cd db/quran/tanzil/arabic

# index contiguity + row count
sqlite3 quran-uthmani.sqlite \
  'select count(*), min("index"), max("index") from quran_text;'   # 6236|1|6236

# basmala prefix uniformity — prints 95 and 97 for uthmani, nothing for simple-clean
python3 - <<'EOF'
import sqlite3
for f in ("quran-uthmani.sqlite","quran-simple-clean.sqlite"):
    a1={s:t for s,a,t in sqlite3.connect(f).execute(
        'select sura,aya,text from quran_text where aya=1')}
    base=a1[1]
    print(f, [s for s in range(2,115) if s!=9 and not a1[s].startswith(base+" ")])
EOF

# script characteristics (§2.2)
python3 - <<'EOF'
import sqlite3, unicodedata
for f in ("quran-uthmani.sqlite","quran-simple-clean.sqlite"):
    rows=[t for (t,) in sqlite3.connect(f).execute('select text from quran_text')]
    print(f, "nfc", all(unicodedata.is_normalized("NFC",t) for t in rows),
          "wasla", sum('ٱ' in t for t in rows),
          "supalef", sum('ٰ' in t for t in rows),
          "tatweel", sum('ـ' in t for t in rows),
          "maxlen", max(map(len,rows)))
EOF

# xml element counts (§2.1)
cd .. && for e in sura juz page ruku quarter manzil sajda; do
  printf '%s\t' "$e"; grep -c "<$e " quran-data.xml; done
```

---

## 15. Open questions

1. **Script default** (§11.0) — extract `simple-enhanced` for visual parity, or ship Uthmani
   and re-tune typography + font? **Blocks Phase 2.**
2. **CORS** (§5) — mount `/quran/v1` outside `origin_guard` with wildcard ACAO (recommended
   for a public content API), or keep the allowlist and treat it as first-party-only?
   **Blocks Phase 1**, and determines whether §10 needs `Vary: Origin`.
3. **Mushaf edition** (§5) — this is a *schema* decision, not deferrable like the rest:
   introduce `?mushaf=hafs` now even though it is single-valued, because retrofitting it
   later changes `/pages/{num}` semantics for every cached client. **Recommended: add now.**
4. **Prerender vs. freshness** (§11.2) — keep build-frozen prerendered surah pages
   (recommended, SEO) and accept redeploy-to-correct, or go SSR + client fetch?
5. **Slugs** — ship a committed 114-entry slug table (recommended) vs. derive from `tname`.
6. **Search default scope** — Arabic-only default (recommended), or include a default
   translation once packs exist?
7. **Verse-of-the-day zone** — UTC (recommended; cacheable, globally consistent) vs.
   user-local.
8. **`admin/reload` auth** — reuse the existing admin `auth_guard`, or a dedicated operator
   token?
9. **FTS5 hardening** — defer with LIKE fallback (recommended for MVP), or commit now to
   `bundled` + `-DSQLITE_ENABLE_FTS5`?
