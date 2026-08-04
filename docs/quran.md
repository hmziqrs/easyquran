# EasyQuran — Quran plan

Single source of truth for Quran data, API, delivery, caching. **This is the plan**, not an index.

Part 1 is settled ground: rules and contracts that constrain everything else, verified against code. Part 2 is the work — ordered by dependency, each item carrying its decision, approach, and done-when. Implemented detail lives in code; this doc does not restate it.

---

# Part 1 — settled

## Hard rules

- Quran databases (Arabic + every translation) are **immutable**. Sourced from Tanzil.net. No modifications, no versioning.
- **No content versioning.** No `contentVersion`/`searchVersion`, no version segment in R2 keys, no hash-keyed cache dirs. A database's identity is its **id** (`uthmani`, `simple-clean`, `en.sahih`, …).
- **Ayah text is verbatim.** The source `text` is never normalized, split, trimmed, or reordered — SQLite read → in-memory store → JSON → SSG HTML. `9:1` has no basmala, `1:1` *is* the basmala, `27:30` carries one mid-ayah, `95:1`/`97:1` carry a shadda spelling: all correct source state, none of it a cleanup target.
- **Integrity: two layers, both deliberate.**
  - *Corpus text* — golden sha256 over the joined corpus, asserted at boot as a **literal**, never self-derived (`quran/loader.rs`): a normalizing loader corrupts both sides of a self-derived check identically and slips past. 5,782 of 6,236 Uthmani rows are non-NFC — NFC composes ALEF + MADDAH into U+0622 (2,945 of each, across 2,044 rows; zero pre-existing U+0622), so a driver or encoder upgrade rewrites thousands of ayahs while every page still looks correct. Simple-clean is NFC/NFKC-stable.
  - *Downloaded db bytes* — the client re-hashes cached OPFS bytes against `spec.sha256` on read (`workers/opfs-cache.ts`); the SSG build verifies the digest it reads.
  - What is banned is **runtime identity by hash**: no sha256/blake3 in an R2 key, cache dir, or version handshake. Hash verifies bytes; it never names them.
- **Boot is fail-fast.** Missing/corrupt source, XML failure, tiling failure, digest mismatch, or a wrong bismillah split count → log the specific invariant, exit non-zero. No "Arabic not ready" served state.
- **Arabic renders SSG.** Translated pages render SSR + disk-TTL (Part 2 §7). Never ISR.

## Data

- `quran-uthmani.sqlite` = display text. `quran-simple-clean.sqlite` = search corpus (undiacriticated, regular alef). Both: `quran_text("index" PK, sura, aya, text)` + `idx(sura,aya)`, 6236 rows, read **directly, read-only**. Not consolidated; no canonical db is built.
- `quran-data.xml` = metadata only (no verse text): 114 surah, 6236 ayah, 604 pages, 30 juz, 556 ruku, 240 hizb-quarter, 7 manzil, 15 sajda. Web consumes one compact `web/static/quran-meta/quran-data.json`; Rust parses the XML in memory at boot. That JSON's header row (`[digest, "1.0", "cc-by"]`) is **provenance only** — format-validated at load, never compared, never keyed on, not a content version.
- **Global ayah index:** `quran_text."index"` is canonical — contiguous `1..6236`, unique, ordered by surah then ayah, `= sura.start + aya` (XML `start` zero-based, `aya` one-based). Asserted at boot; `/range` and its cap of 300 rest on it.
- **Marker families tile `[1,6236]`** with no gap or overlap (page/juz/ruku/hizb-quarter/manzil), marker `index` order asserted to match global order. `<quarter>` carries no hizb attribute — derive: `hizb = ((i-1)/4)+1`, `quarterInHizb = ((i-1)%4)+1`.
- **No FTS.** Search = normalize + substring-scan the 6236 simple-clean rows.
- Translations: 115 dumps → one `<id>.sqlite` each, same schema, across 44 languages. 186 MB total: p50 1.19 MB, p95 3.07 MB, max 12.43 MB. **Non-commercial license** — revisit if the project monetizes.

## API — Rust `/quran`

Module `quran_v1`, nested at `/quran`. **No `/quran/v1`. No `/version`.**

One shape per navigation family — `/{family}` list, `/{family}/{n}` detail, `/{family}/{n}/ayahs` text — for `surahs`, `juzs`, `pages`, `rukus`, `hizb-quarters`, `manzils`, plus `sajdas` (list + detail). Also:

- `GET /quran/ayahs`, `/quran/ayahs/{surah}/{ayah}`, `/quran/ayahs/{verseKey}` (redirect).
- `GET /quran/sources/{id}/surah/{n}` → `Envelope<QuranSurahTextDto>`; `GET /quran/sources/{id}/range?from=&to=` → `Envelope<RangeText>` (cap 300). **This pair is also the translation read API** — it is already source-parameterized, so translations need no new routes, only a resolver (§3).
- `GET /quran/search?q=…` — substring scan over normalized simple-clean. `kind` is an **output** discriminator; only `ayah` is produced today (`opener` reserved, never emitted).
- `GET /quran/random` — deterministic ayah-of-the-day (date-seeded LCG, not RNG).
- `GET /quran/scripts` (R2 artifacts: id, sizeBytes, sha256, downloadUrl), `GET /quran/health/ready`, `/quran/openapi.json` under the `openapi` feature.
- **Rate limits, per IP:** 600/60s general, 30/60s on search — `/search` mounts on a separate router so the CPU-heavy scan does not inherit the coarse limit.
- `Envelope<T> = { data: T }`. Closed error shape (400 / 404 / 429 / 5xx) — a failed read never becomes an empty surah.

**Web routes are slug-based** (`al-fatihah`), the API numeric (`1..=114`). The web adapter resolves slug → number before calling Axum; the backend owns no slug table, so a URL-spelling change never touches API content.

Auth lives outside `/quran`: web OAuth code flow (google/apple/facebook/github) + mobile `POST /auth/{google,apple,facebook}/v1/token` (client SDK token → cookie session, **not** JWT; GitHub is web-only). The three `/token` routes are CSRF-exempt.

## Normalization + canonical view — parity contract

One rule set, two implementations (Rust + web Worker) returning identical ordered results:

- **normalize** (`quran/normalize.rs` ↔ web): drop combining marks U+064B–U+0658, U+0670 (superscript alef — dropped, not folded, to match the web's `/\p{Mn}/u`), U+0640 (tatweel), Quranic signs U+06D6–U+06ED (incl. U+06E9 place-of-sajda). Fold `آ أ إ ٱ → ا`, `ى → ي`, `ة → ه`.
- **offset map:** normalization emits a normalized-scalar → source-scalar map, so a hit found in normalized space highlights correctly in the rendered script (converted to UTF-16 for the web). Offsets crossing the boundary are **Unicode scalar** (Rust `char` = web `Array.from`).
- **canonical view** splits each surah into **body + opener** units (opener = rank 0, ayah = rank 1).
- **Opener classification is two orthogonal enums**, not one: `OpenerKind` = `Verse | Header | None` (what the opener *is*) × `OpenerPackaging` = `NumberedAyah | EmbeddedPrefix | ChapterFlag | SeparateRow | Absent` (how the source stores it). Split counts (1 / 112 / 1) asserted at boot. Surahs 95 & 97 carry a shadda variant (`بِّسْمِ`) — match the prefix diacritic-insensitively, never exactly.
- A rule change ships new Rust + web together; it never mutates a sqlite. Parity is currently **by construction, not enforced** — see §8.

## Caching — as built

Databases are immutable and read-only: no write handling anywhere.

- **Rust, in memory:** both Arabic corpora load at boot and stay resident. `Script` accepts `uthmani` | `simple-clean` only.
- **Rust, responses:** no server-side store. Reads come straight from the resident corpus, but every response is edge-cacheable — weak `ETag` = `store tag : canonical key`, `If-None-Match` → 304, `Cache-Control` per family: Arabic `max-age=300, s-maxage=86400, swr=604800, sie=604800`; search `max-age=60, s-maxage=300`; artifacts `immutable`; 5xx `no-store`, so a CDN cannot pin a transient failure.
- **Web, OPFS:** Arabic eager on boot. **Key = `spec.sha256`**; the id is only a filename, and bytes are re-verified against that hash on every read. No eviction exists yet (§6). The only LRU today is the service worker's route cache, cap 300.
- **R2 layout** (publisher `translations/scripts/upload-sqlite.ts`, bucket prefix `tanzil/`): `tanzil/arabic/<file>.sqlite`, `tanzil/translations/sqlite/<id>.sqlite`, `tanzil/translations/index.min.json` (mutable, `max-age=300, must-revalidate`), `tanzil/quran-data.xml`. Everything except the catalogue is `immutable`. Raw `sql/` dumps are never published.

## Web delivery + pagination — as built

- **Arabic = SSG.** Build-time `node:sqlite` reads uthmani and prerenders reader pages — real HTML for SEO and first paint. Hydration adopts the prerendered page; no WASM on the critical path.
- **Service worker constraint:** SvelteKit forbids `$lib`/relative/npm imports in the SW, so the SW↔client contract is **duplicated** (SW-inline + `web/src/lib/offline/*`). Audit both sides on any contract change. Updates are an atomic SW-lifecycle cache swap — install verifies the complete new cache before activate; one accepted update reloads all open tabs.
- **Page geometry is source-agnostic** — it describes Mushaf geometry, not text, so translations reuse it unchanged. Global Mushaf page `1..604`; surah-local page = a global page clipped to one surah, renumbered from 1 within it (662 total); juz `1..30`. Routes: `/app/<surah>` (local page 1), `/app/<surah>/page/<n>`, `/app/page/<globalPage>`, `/app/juz/<n>` — all prerendered.
- **Ayah → page is computed, never stored.** `surahLocalPageForAyah(surah, ayah)` → `{ localPage, globalPage }`. A deep link `/app/<surah>#ayah-<surah>-<ayah>` resolves to its containing local page; if that is not the prerendered one the client redirects (`replaceState`, no scroll jump) to `/app/<surah>/page/<n>#ayah-…` and scrolls the ayah to a stable centered position (small-screen safe).
- **Word → page** reuses ayah navigation (same page) with a finer in-ayah highlight. A word-offset hash grammar is future work.
- **Reader loading:** each surah route reads **exactly one bounded local page** (cross-surah guarded), never a whole surah. Continuous scroll pulls adjacent local pages on demand via Worker range reads; a ~5-page virtual window bounds the DOM.

---

# Part 2 — the plan

Ordered by dependency. §1–§2 unblock §3–§4, which unblock §5, which unblocks §6–§7. §8–§9 are independent.

## 1. Fix the artifact URL contract — *defect, blocks everything downstream*

`/scripts` builds `{public_url}/quran/arabic/{id}/{filename}` (`quran_v1/controller.rs:649`), but the publisher writes `tanzil/arabic/<file>.sqlite` and the web bakes the same `r2Path`. The HEAD verify therefore always fails, `/scripts` never returns 2 artifacts, the response is stamped `no-store` and never cached, and `resolveManifest` silently falls back to the baked manifest forever. The endpoint is effectively dead.

**Decision:** the publisher's layout wins. It is what is deployed, what the web has baked, and what immutable keys forbid re-shuffling.

**Approach:** change `resolve_scripts` to `{public_url}/tanzil/arabic/{filename}`. Keep the HEAD size check and the partial-response `no-store` behaviour — both are correct, they were just guarding a wrong URL.

**Done when:** `/scripts` returns 2 verified artifacts with `ARABIC_CACHE`; the web's `ResolvedManifest.source` flips from `baked` to `api`; a test asserts the emitted URL against the publisher's key constant so the two cannot drift again.

## 2. Publish sqlite digests in the catalogue

OPFS keys on `spec.sha256` of the **downloaded sqlite**. `index.json` only carries the sha256 and size of the **SQL dump** — a different file. Nothing today can name a translation artifact by its real digest, and `web/src/lib/data/translations.json` (which the README says `catalog` writes) does not exist.

**Decision:** the catalogue is the digest authority for translations. Static digest pinning stays **Arabic-only** — `resolveSourceProfile` pins two digests because they register a canonical-view profile; 115 translations cannot and should not be pinned in source.

**Approach:** `build:sqlite` emits `sqlite/manifest.json` (`id`, `sizeBytes`, `sha256`) per artifact; `catalog` folds it into `index.min.json` under a `sqlite` field and copies the result to `web/src/lib/data/translations.json`; `verify` fails if any translation lacks a sqlite entry or its digest disagrees with the file on disk.

**Done when:** every catalogue entry carries a sqlite digest + size, `pnpm verify` fails on a missing or stale one, and the web can construct an `ArtifactSpec` for any translation without hardcoding.

## 3. Translation sources in Rust

`Script::parse` accepts Arabic only, so `/quran/sources/{id}/…` 400s for every translation. No pool exists.

**Decision:** introduce `SourceId = Arabic(Script) | Translation(TranslationId)` and resolve it in `parse_source`. **No new routes** — `sources/{id}/surah/{n}` and `sources/{id}/range` already carry the right shape, and this is the cold-read path that serves a client before its sqlite is in OPFS.

**Approach:** a translation loads into the same `Corpus` arena representation as Arabic (`Corpus::from_texts`), built on demand from `translations/sqlite/<id>.sqlite`, then cached. Pool bounds, as starting values in settings, not constants:

| Bound | Value | Why |
|---|---|---|
| max resident translations | 8 | p95 artifact is 3.07 MB; 8 covers realistic concurrent distinct picks |
| max resident bytes | 48 MB | binds first when large ones stack — max single artifact is 12.43 MB |
| idle TTL | 30 min | a translation unused for half an hour is not hot |
| eviction | LRU, whichever bound trips first | |

Single-flight is mandatory: two concurrent cold requests for the same id must build once — `moka::future::Cache::try_get_with` or equivalent. **No `std::sync` guard may be held across an `.await`** (`MutexGuard` is `!Send`, axum handler futures must be `Send`).

The golden-digest rule does **not** extend to translations: there is no per-translation literal to assert. Their integrity is fixed at build (§2) and verified on download.

**Done when:** `/quran/sources/en.sahih/surah/2` returns text; a cold-start concurrency test proves one build for N simultaneous requests; the pool exports resident-count, resident-bytes, hit-rate, and evictions/min so the bounds above are tuned on evidence rather than reset by guess.

## 4. `/quran/sources` catalogue endpoint

The reader's translation picker needs the list; `/scripts` is the SSG bootstrap for the two Arabic artifacts and should stay that narrow.

**Decision:** add `GET /quran/sources` returning every readable source — Arabic and translations — as `{ id, kind, language, languageCode, direction, name, translator, sizeBytes, sha256, downloadUrl }`. `/scripts` is untouched.

**Approach:** back it with the catalogue from §2 (read at boot, same fail-fast policy). Reuse the `/scripts` HEAD-verify + partial-response discipline: an unverified entry is omitted and the response is `no-store`, never a half-list pinned by a CDN.

**Done when:** the endpoint lists 117 sources with working download URLs, and a partial upstream produces `no-store` rather than a cached truncation.

## 5. Translation reader routes

No translation route exists — not even a page-1 form. Geometry and accessors are already source-agnostic, so this is wiring, not new page math.

**Decision:** a dedicated `t` segment. `/t/<lang>/<translator>` sits **directly after the surah slug**, or **directly after `/app`** when the route has no surah. No reserved-word list, no future collision with `page` or `juz`.

```
/app/al-baqarah                       Arabic, local page 1
/app/al-baqarah/page/3                Arabic, local page 3
/app/al-baqarah/t/en/sahih            translation, local page 1
/app/al-baqarah/t/en/sahih/page/3     translation, local page 3
/app/t/en/sahih/page/42               translation, global page 42
/app/t/en/sahih/juz/30                translation, juz 30
```

**Approach:** parameterize by source id — path helpers in `lib/data/quran.ts`, the `readSurahRouteData` loader, `QuranSourceId` + `QuranSourcePlan` (today frozen to two Arabic ids), and the Worker protocol (`init` takes a manifest; `readSurah`/`readRange` need the source). Page geometry, accessors, and the virtual window stay untouched.

**Done when:** a translated deep link past local page 1 has a real URL; `/app/<surah>/page/<n>` and `/app/<surah>/t/<lang>/<translator>` cannot shadow each other (route test); the picker switches source without losing reading position.

## 6. OPFS retention

No TTL, no pruner, no size cap. Fine at 1–2 databases; the failure case is a tester pulling ~100 (186 MB) and hitting the origin quota.

**Decision:** adaptive TTL — use renews, disuse expires. **Arabic is pinned and never evicted**; only translations are subject to it.

**Approach:** per-id `lastUsed` alongside the cached bytes. Prune on worker boot and after each successful download, evicting LRU until within: ≤ 12 cached translations, ≤ 128 MB total, and nothing untouched for 30 days. A normal user (1–2 translations) never trips any bound, so the common path does no work.

**Done when:** a synthetic 100-database run stays inside the caps, an evicted database re-downloads transparently, and the two Arabic artifacts survive every prune.

## 7. `adapter-node` + SSR disk-TTL for translated pages

Arabic stays prerendered; translated pages cannot be — 115 sources × 662 local pages is not a build.

**Decision:** SSR on demand, cached to disk with TTL. Not ISR: no build-time revalidation contract, no version handshake.

**Approach:** migrate `adapter-static → adapter-node` (Arabic routes stay prerendered under it), then a disk cache keyed `(sourceId, surah, localPage)` — TTL 7 days, LRU inside a disk budget. Cache HTML only; JSON stays uncached (§Caching).

**Done when:** a cold translated page renders and lands on disk, a warm one is served from disk, Arabic route output is byte-identical to the current static build, and the disk budget is enforced rather than assumed.

## 8. Shared parity fixtures

Rust fixtures (`quran/view.rs`) and web fixtures (`web/src/lib/quran/view/__fixtures__/`) live in separate trees under different names with no cross-check. They agree because they were written to agree; a one-sided rule change is not caught.

**Decision:** one fixture corpus, read by both suites.

**Approach:** a neutral JSON corpus (normalize inputs/outputs, canonical-view units, opener classification, offset maps). Rust reads it via `include_str!`; the web imports it. Neither may hold a private copy.

**Done when:** deleting a fold rule from `normalize.rs` alone fails the Rust suite, and deleting it from the web alone fails the web suite.

## 9. Loose ends

- Deep-link highlight: the target ayah scrolls into view but is not visually marked — add a `revealed-ayah` / `:target` marker.
- Link generators still emit legacy `?verse=N` (`quran.ts:46`) instead of the page-aware path — costs a redirect hop.
- Surah-local page tiling (662) is asserted in `quran.test.ts` only. Server-side tiling is asserted at boot for the global families; add the local-page assert **only if** the server starts serving local pages.
- Mobile (Flutter) parity — not scheduled.

---

## Appendix

Four genuinely-empty source verses — immutable source data, **not** bugs; do not "fix": `fa.safavi 80:39`, `ku.asan 108:3`, `sq.mehdiu 21:56`, `sq.mehdiu 77:14`. Detail in [`docs/research/translation-empty-verses.md`](./research/translation-empty-verses.md).
