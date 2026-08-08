# EasyQuran — Quran plan

Single source of truth for Quran data, API, delivery, caching. **This is the plan**, not an index.

Part 1 is settled ground: rules and contracts that constrain everything else, verified against code. Part 2 is the work — ordered by dependency, each item carrying its decision, approach, and done-when. Implemented detail lives in code; this doc does not restate it.

---

# Part 1 — settled

## Hard rules

- Quran databases (Arabic + every translation) are **immutable**. Sourced from Tanzil.net. No modifications, no versioning.
- **No content versioning.** No `contentVersion`/`searchVersion`, no version segment in R2 keys, no hash-keyed cache dirs. A database's identity is its **id** (`uthmani`, `simple-clean`, `en.sahih`, …).
- **Ayah text is verbatim.** The source `text` is never normalized, split, trimmed, or reordered — SQLite read → in-memory store → JSON → SSG HTML. `9:1` has no basmala, `1:1` *is* the basmala, `27:30` carries one mid-ayah, `95:1`/`97:1` carry a shadda spelling: all correct source state, none of it a cleanup target.
- **Integrity: SHA-256 is manual audit, never automated.** A DB's identity is its id; sha never names a cache key, profile, or catalogue row, and never digests **Quran data** at runtime. The one runtime sha over text is a digest of the normalized **search query** (user input), folded into `/search`'s ETag for variance — never over the corpus. The corpus + sqlite digests are checked by hand (`just quran-audit`, `pnpm audit:arabic`, `pnpm verify`) — not at boot, not on download (those size-check only, `verifyBytes`). Runtime protection is **content asserts** (6236 rows, tiling, ayah keys, packaging). The golden corpus digest is a literal, never self-derived — a normalizing loader corrupts both sides of a self-derived check identically; 5,782 Uthmani rows are non-NFC, so a driver/encoder upgrade can rewrite thousands of ayahs while pages still look right, which is why the audit exists. Crypto sha (CSRF/HMAC/Argon2/PKCE) is unrelated and stays.
- **Boot is fail-fast.** Missing/corrupt source, XML failure, tiling failure, or wrong bismillah split count → exit non-zero. (Digest audit is not boot — it's manual `just quran-audit`; boot fail-fasts on shape, not sha.)
- **Arabic renders SSG.** Translated pages render SSR + disk-TTL (Part 2 §7). Never ISR.

## Data

- `quran-uthmani.sqlite` = display **and search** corpus (quran.com parity — quran.com indexes the full Uthmani text; ornaments are searchable: ۞→199, ۩→15). `quran-simple-clean.sqlite` = a readable API/canonical-view script (still Rust-resident + audited; no longer the search corpus). Both: `quran_text("index" PK, sura, aya, text)` + `idx(sura,aya)`, 6236 rows, read **directly, read-only**. Not consolidated; no canonical db is built.
- `quran-data.xml` = metadata only (no verse text): 114 surah, 6236 ayah, 604 pages, 30 juz, 556 ruku, 240 hizb-quarter, 7 manzil, 15 sajda. Web consumes one compact `web/static/quran-meta/quran-data.json`; Rust parses the XML in memory at boot. That JSON's header row (`[digest, "1.0", "cc-by"]`) is **provenance only** — format-validated at load, never compared, never keyed on, not a content version.
- **Global ayah index:** `quran_text."index"` is canonical — contiguous `1..6236`, unique, ordered by surah then ayah, `= sura.start + aya` (XML `start` zero-based, `aya` one-based). Asserted at boot; `/range` and its cap of 300 rest on it.
- **Marker families tile `[1,6236]`** with no gap or overlap (page/juz/ruku/hizb-quarter/manzil), marker `index` order asserted to match global order. `<quarter>` carries no hizb attribute — derive: `hizb = ((i-1)/4)+1`, `quarterInHizb = ((i-1)%4)+1`.
- **No FTS.** Search = normalize + substring-scan the 6236 **Uthmani** rows (ornaments retained → searchable, matching quran.com).
- Translations: 115 dumps → one `<id>.sqlite` each, same schema, across 44 languages. 186 MiB total: p50 1.25, p95 3.10, max 12.43 MiB. **Non-commercial license** — revisit if the project monetizes. The web's baked catalogue (`web/src/lib/data/translations.json`) is a flat positional array mirroring `quran-data.json`'s surah rows — `[id, language, languageCode, direction, name, translator, filePath, sizeBytes]`, decoded by `TranslationField` in `quran/catalogue.ts`; no sha256 (id-keyed). `db/.../index.min.json` is the object-form twin Rust loads at boot; both are sha-free.

## API — Rust `/quran`

Module `quran_v1`, nested at `/quran`. **No `/quran/v1`. No `/version`.**

One shape per navigation family — `/{family}` list, `/{family}/{n}` detail, `/{family}/{n}/ayahs` text — for `surahs`, `juzs`, `pages`, `rukus`, `hizb-quarters`, `manzils`, plus `sajdas` (list + detail). Also:

- `GET /quran/ayahs`, `/quran/ayahs/{surah}/{ayah}`, `/quran/ayahs/{verseKey}` (redirect).
- `GET /quran/sources/{id}/surah/{n}` → `Envelope<QuranSurahTextDto>`; `GET /quran/sources/{id}/range?from=&to=` → `Envelope<RangeText>` (cap 300). **This pair is also the translation read API** — it is already source-parameterized, so translations need no new routes, only a resolver (§3).
- `GET /quran/search?q=…` — substring scan over normalized Uthmani (ornaments retained → searchable, e.g. a query carrying ۞/۩/ۥ finds the matching verses, matching quran.com). A query must be 3..=64 Unicode scalars after normalization **or** contain a Quranic ornament mark (so a lone `۞`→199 / `۩`→15 works like quran.com, without relaxing the 3-char floor for plain text). `kind` is an **output** discriminator; only `ayah` is produced today (`opener` reserved, never emitted).
- `GET /quran/random` — deterministic ayah-of-the-day (date-seeded LCG, not RNG).
- `GET /quran/scripts` (R2 artifacts: id, sizeBytes, downloadUrl), `GET /quran/health/ready`, `/quran/openapi.json` under the `openapi` feature.
- **Rate limits, per IP:** 600/60s general, 30/60s on search — `/search` mounts on a separate router that carries the tighter 30/60s, and it remains under the merged 600/60s `quran-v1` ceiling too, so the CPU-heavy scan is **double-limited by design** (its own 30/60s bound trips first).
- `Envelope<T> = { data: T }`. `QuranApiError` is the closed error shape — nested `{ error: { code, message, detail } }` for 400 / 404 / 5xx — so a failed read never becomes an empty surah. The rate-limit layer's 429 uses the app `ErrorResponse` envelope (`{ type, status, retryAfter, … }`, camelCase; no `error` key, `message` is debug-only), while the gate's store-unavailable 503 uses the gate's flat shape (`{ error, message }`) — neither is `QuranApiError`.

**Web routes are slug-based** (`al-fatihah`), the API numeric (`1..=114`). The web adapter resolves slug → number before calling Axum; the backend owns no slug table, so a URL-spelling change never touches API content.

Auth lives outside `/quran`: web OAuth code flow (google/apple/facebook/github) + mobile `POST /auth/{google,apple,facebook}/v1/token` (client SDK token → cookie session, **not** JWT; GitHub is web-only). The three `/token` routes are CSRF-exempt.

## Normalization + canonical view — parity contract

One rule set, two implementations (Rust + web Worker) returning identical ordered results, validated against quran.com (the reference platform) in [`docs/quran-normalization-reasoning.md`](./quran-normalization-reasoning.md). quran.com indexes the full Uthmani text and **keeps** standalone Quranic ornaments searchable (`۞`→199, `۩`→15, small waw/yeh `ۥۦ` narrow, stop marks literal); its index **strips** intra-cluster combining marks (harakat, maddah `U+0653`, tatweel, superscript alef) so bare queries match. We match that: search scans normalized **Uthmani** and the strip set is the explicit, byte-identical `U+064B–U+0658` (harakat + maddah), `U+0640` (tatweel), `U+0670` (superscript alef), `U+06DD` (end-of-ayah, 0 occ) — **standalone ornaments `U+06D6–U+06DC`, `U+06DE`, `U+06DF–U+06ED` are KEPT** (searchable tokens). Fold `آ أ إ ٱ → ا`, `ى → ي`, `ة → ه`. (Stripping maddah `U+0653` is required for substring search — Tanzil Uthmani encodes alef-madda decomposed as `alef+0653` in ~3051 verses, so a bare query must not carry it; this matches quran.com's index behavior, no stemmer needed.) A shared neutral parity corpus enforces the Rust/web contract — see §8.

- **normalize** (`quran/normalize.rs` ↔ web): drop combining marks `U+064B–U+0658` (harakat + maddah `U+0653`/hamza `U+0654`), `U+0670` (superscript alef), `U+0640` (tatweel), `U+06DD` (end-of-ayah, 0 occ). **Keep** Quranic ornaments `U+06D6–U+06DC, U+06DE, U+06DF–U+06ED` (incl. small waw/yeh `U+06E5`/`U+06E6`, sajda `U+06E9`) — they are searchable tokens. Fold `آ أ إ ٱ → ا`, `ى → ي`, `ة → ه`.
  This is **search/normalize-layer only**; the **display layer never normalizes**, so every mark renders verbatim in the reader — matching quran.com. The offset map still bridges the harakat/tatweel/`U+0670` strip-gaps; kept ornaments ride through 1:1.
- **offset map:** normalization emits a normalized-scalar → source-scalar map, so a hit found in normalized space highlights correctly in the rendered script (converted to UTF-16 for the web). Offsets crossing the boundary are **Unicode scalar** (Rust `char` = web `Array.from`).
- **canonical view** splits each surah into **body + opener** units (opener = rank 0, ayah = rank 1).
- **Opener classification is two orthogonal enums**, not one: `OpenerKind` = `Verse | Header | None` (what the opener *is*) × `OpenerPackaging` = `NumberedAyah | EmbeddedPrefix | ChapterFlag | SeparateRow | Absent` (how the source stores it). Split counts (1 / 112 / 1) asserted at boot. Surahs 95 & 97 carry a shadda variant (`بِّسْمِ`) — match the prefix diacritic-insensitively, never exactly.
- A rule change ships new Rust + web together; it never mutates a sqlite. Shared neutral fixtures enforce Rust/web parity — see §8.

## Caching — as built

Databases are immutable and read-only: no write handling anywhere.

- **Rust, in memory:** both Arabic corpora load at boot and stay resident. `Script` accepts `uthmani` | `simple-clean` only.
- **Rust, responses:** no server-side store. Reads come straight from the resident corpus, but every response is edge-cacheable — weak `ETag` = `quran-corpus : canonical key` (a static id-based tag; the corpus is immutable so a constant ETag is correct — never a digest over Quran data). `/search` is the exception: its canonical key folds in a sha-256 digest of the normalized **query string** (user input, not Quran data) so same-normalizing queries share an ETag while distinct queries vary. `If-None-Match` → 304, `Cache-Control` per family: Arabic `max-age=300, s-maxage=86400, swr=604800, sie=604800` (also the fully-verified `/scripts` artifacts); search `max-age=60, s-maxage=300`; `/random?date=` `immutable`; `QuranApiError`-originated 5xx `no-store`, so a CDN cannot pin a transient read failure (the rate-limit gate's store-unavailable 503 emits **no** `Cache-Control` header).
- **Web, OPFS:** Arabic eager on boot. **Key = `spec.id`** (a db's identity is its id; the prior `spec.sha256` key was a cache-dir violation). Downloaded bytes (Arabic + translation) are size-checked only on read/download — no digest ships in any catalogue, spec, or ETag; sha verification is the manual `pnpm audit:arabic`. Older sha256-keyed caches orphan on upgrade (one-time re-download). Retention/eviction: §6.
- **Web artifact delivery:** browser downloads use the same-origin streaming `/_quran/<R2 key>` gateway. It accepts only baked Arabic/translation keys and forwards range requests to R2 with immutable caching. This keeps OPFS independent of bucket/custom-domain CORS while preserving the publisher's exact `tanzil/…` layout; no bytes are rewritten or hashed.
- **Offline route pack:** its immutable filename uses SvelteKit's build-version id, not a content digest. Download validation is byte-count + closed-schema validation; the build and browser never hash serialized Quran routes.
- **R2 layout** (publisher `translations/scripts/upload-sqlite.ts`, bucket prefix `tanzil/`): `tanzil/arabic/<file>.sqlite`, `tanzil/translations/sqlite/<id>.sqlite`, `tanzil/translations/index.min.json` (mutable, `max-age=300, must-revalidate`), `tanzil/quran-data.xml`. Everything except the catalogue is `immutable`. Raw `sql/` dumps are never published.

## Web delivery + pagination — as built

- **Arabic = SSG.** Build-time `node:sqlite` reads uthmani and prerenders reader pages — real HTML for SEO and first paint. Hydration adopts the prerendered page; no WASM on the critical path.
- **Service worker constraint:** SvelteKit forbids `$lib`/relative/npm imports in the SW, so the SW↔client contract is **duplicated** (SW-inline + `web/src/lib/offline/*`). Audit both sides on any contract change. Updates are an atomic SW-lifecycle cache swap — install verifies the complete new cache before activate; one accepted update reloads all open tabs.
- **Page geometry is source-agnostic** — it describes Mushaf geometry, not text, so translations reuse it unchanged. Global Mushaf page `1..604`; surah-local page = a global page clipped to one surah, renumbered from 1 within it (662 total); juz `1..30`. Routes: `/app/<surah>` (local page 1), `/app/<surah>/page/<n>`, `/app/page/<globalPage>`, `/app/juz/<n>` — all prerendered.
- **Ayah → page is computed, never stored.** `surahLocalPageForAyah(surah, ayah)` → `{ localPage, globalPage }`. A deep link `/app/<surah>#ayah-<surah>-<ayah>` resolves to its containing local page; if that is not the prerendered one the client redirects (`replaceState`, no scroll jump) to `/app/<surah>/page/<n>#ayah-…` and scrolls the ayah to a stable centered position (small-screen safe).
- **Word → page (planned, no code yet)** will reuse ayah navigation — a word lives inside an ayah, so the page math is unchanged — with a finer in-ayah highlight. The word-offset hash grammar is undecided.
- **Reader loading:** each surah route reads **exactly one bounded local page** (cross-surah guarded), never a whole surah. Continuous scroll pulls adjacent local pages on demand via Worker range reads; a ~5-page virtual window bounds the DOM.

---

# Part 2 — status

**Done (shipped):** §1 artifact URL · §2 no-sha identity · §3 translation pool + metrics · §4 `/sources` · §5 `/t` reader routes · §6 OPFS retention · §7 production Bun SSR + disk TTL · §8 normalize-parity fixture (view/opener/offset fixtures still private — see §8) · §9 loose ends.
**Remaining:** the canonical-view / opener / offset-map fixtures are still two private trees (Rust `quran/testdata/view-*.json` vs web `view/__fixtures__/prefix-cuts.json`) with no cross-check — deferred from §8. Word-level navigation and a future mobile client remain separate, unscheduled product work; neither has a data model or client in this repository today.

Original dependency order + decisions preserved below.

## 1. Fix the artifact URL contract — *done*

`/scripts` formerly built `{public_url}/quran/arabic/{id}/{filename}` (the now-fixed `resolve_scripts` lives at `controller.rs:798`; it emits `{public_url}/tanzil/arabic/{filename}` at `controller.rs:810`), but the publisher writes `tanzil/arabic/<file>.sqlite` and the web bakes the same `r2Path`. With the wrong URL the HEAD verify always failed: `/scripts` never returned 2 artifacts, the response was stamped `no-store` and never cached, and `resolveManifest` silently fell back to the baked manifest forever — the endpoint was effectively dead.

**Decision:** the publisher's layout wins. It is what is deployed, what the web has baked, and what immutable keys forbid re-shuffling.

**Approach:** change `resolve_scripts` to `{public_url}/tanzil/arabic/{filename}`. Keep the HEAD size check and the partial-response `no-store` behaviour — both are correct, they were just guarding a wrong URL.

**Done when:** `/scripts` returns 2 verified artifacts with `ARABIC_CACHE`; the web's `ResolvedManifest.source` flips from `baked` to `api`; a test asserts the emitted URL against the publisher's key constant so the two cannot drift again.

## 2. No SHA-256 as a Quran identity key — *done*

A Quran DB's identity is its **id** (`uthmani`, `en.sahih`, …); DBs are immutable, so id is complete and a sha key only renames it. SHA-256 was removed from every place it named or keyed Quran data — the translation catalogue, the OPFS/IDB cache (now keyed by `spec.id`), the worker spec, Rust `CatalogueEntry` + `/sources` `SourceDto` + `/scripts` `Artifact` + `/health` (fields gone, no `Option`), the Arabic boot golden assert, `resolveSourceProfile`'s compare, the SSG boot validate, client download sha-verify, and the Arabic ETag (now a static `quran-corpus` tag). The catalogue is a flat positional array (`TranslationField` in `quran/catalogue.ts`), like `quran-data.json`'s surah rows.

**What stays sha — manual audit only** (a human runs these; never automated): `just quran-audit` + `pnpm audit:arabic` (Arabic corpus + sqlite digests), `pnpm verify` in `db/quran/tanzil/translations` (translation build digests vs repo-only `index.json` + `manifest.json`). Plus crypto primitives (CSRF/HMAC/Argon2/PKCE) — unrelated to Quran data.

Guards: `tests/quran_v1.rs` (`sources_rows_never_carry_sha256`, `scripts_endpoint_carries_no_sha256`) + `web catalogue-sha-guard.test.ts`. Runtime protection is content asserts, not sha.

## 3. Translation sources in Rust — *done*

`Script::parse` accepts Arabic only, so `/quran/sources/{id}/…` 400s for every translation. No pool exists.

**Decision:** introduce `SourceId = Arabic(Script) | Translation(TranslationId)` and resolve it in `parse_source`. **No new routes** — `sources/{id}/surah/{n}` and `sources/{id}/range` already carry the right shape, and this is the cold-read path that serves a client before its sqlite is in OPFS.

**Approach:** a translation loads into the same `Corpus` arena representation as Arabic (`Corpus::from_texts`), built on demand from `translations/sqlite/<id>.sqlite`, then cached. Pool bounds, as starting values in settings, not constants:

| Bound | Value | Why |
|---|---|---|
| max resident translations | 8 | p95 artifact is 3.10 MiB; 8 covers realistic concurrent distinct picks |
| max resident bytes | 48 MiB | binds first when large ones stack — max single artifact is 12.43 MiB |
| idle TTL | 30 min | a translation unused for half an hour is not hot |
| eviction | LRU, whichever bound trips first | |

Single-flight is mandatory: two concurrent cold requests for the same id must build once — `moka::future::Cache::try_get_with` or equivalent. **No `std::sync` guard may be held across an `.await`** (`MutexGuard` is `!Send`, axum handler futures must be `Send`).

The golden-digest rule does **not** extend to translations: there is no per-translation literal to assert, and none is wanted — a translation's identity is its id. Integrity is fixed at build (repo-only `index.json` + `manifest.json` digests, checked by `pnpm verify`); on download the client size-checks bytes only — no sha-verify for any source, Arabic included. The digest audit is manual (`just quran-audit` / `pnpm audit:arabic`). See §2.

**Done when:** `/quran/sources/en.sahih/surah/2` returns text; a cold-start concurrency test proves one build for N simultaneous requests; the pool exports resident-count, resident-bytes, hit-rate, and evictions/min so the bounds above are tuned on evidence rather than reset by guess.

## 4. `/quran/sources` catalogue endpoint — *done*

The reader's translation picker needs the list; `/scripts` is the SSG bootstrap for the two Arabic artifacts and should stay that narrow.

**Decision:** add `GET /quran/sources` returning every readable source — Arabic and translations — as `{ id, kind, language, languageCode, direction, name, translator, sizeBytes, downloadUrl }` (no sha256 — a source's identity is its id; Arabic integrity ships via `/scripts`). `/scripts` is untouched.

**Approach:** back it with the catalogue from §2 (read at boot, same fail-fast policy). Reuse the `/scripts` HEAD-verify + partial-response discipline: an unverified entry is omitted and the response is `no-store`, never a half-list pinned by a CDN.

**Done when:** the endpoint lists 117 sources with working download URLs, and a partial upstream produces `no-store` rather than a cached truncation.

## 5. Translation reader routes — *done*

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

**Approach:** parameterize by source id — path helpers in `lib/data/quran.ts`, the `readSurahRouteData` loader, `QuranSourceId` + `QuranSourcePlan` (the web worker plan is frozen to the single `uthmani` Arabic id for reader + search.match + search.display; Rust still loads + serves both Arabic scripts), and the Worker protocol (`init` takes a manifest; `readSurah`/`readRange` need the source). Page geometry, accessors, and the virtual window stay untouched.

**Done when:** a translated deep link past local page 1 has a real URL; `/app/<surah>/page/<n>` and `/app/<surah>/t/<lang>/<translator>` cannot shadow each other (route test); the picker switches source without losing reading position.

## 6. OPFS retention — *done*

No TTL, no pruner, no size cap. Fine at 1–2 databases; the failure case is a tester pulling ~100 (186 MiB) and hitting the origin quota.

**Decision:** adaptive TTL — use renews, disuse expires. **Arabic is pinned and never evicted**; only translations are subject to it.

**Approach:** per-id `lastUsed` alongside the cached bytes. Prune on worker boot and after each successful download, evicting LRU until within: ≤ 12 cached translations, ≤ 128 MiB total, and nothing untouched for 30 days. A normal user (1–2 translations) never trips any bound, so the common path does no work.

**Done when:** a synthetic 100-database run stays inside the caps, an evicted database re-downloads transparently, and the pinned `uthmani` Arabic artifact survives every prune (simple-clean is no longer worker-downloaded or pinned — search uses the Uthmani index).

## 7. `adapter-node` + SSR disk-TTL for translated pages — *done*

Arabic stays prerendered; translated pages cannot be — 115 sources × 662 local pages is not a build.

**Decision:** SSR on demand, cached to disk with TTL. Not ISR: no build-time revalidation contract, no version handshake.

**Approach:** `adapter-node` keeps Arabic routes prerendered and renders translations at runtime. Production runs the adapter-node server on the **Bun** runtime (a deliberate choice — owner benchmarks showed Bun holding translation-SSR throughput where Node folds, at roughly half the resident memory); Node is **build-only**, because Arabic prerender reads the sqlite corpus through `node:sqlite`, which Bun does not implement. It enables the Axum API and persists a disposable `web_quran_cache` volume. The HTML cache uses canonical `(sourceId, kind, index[, localPage])` keys inside a SvelteKit build-id namespace — the namespace invalidates derived markup when immutable JS/CSS filenames change and is not Quran data versioning. TTL is 7 days with a 256 MiB LRU budget. It never intercepts SvelteKit `__data.json`. API JSON gets no server-side disk store; it stays edge-cacheable exactly as today (§Caching) — "uncached" here means no disk copy, not no caching. `Server-Timing` plus `X-EasyQuran-Quran-Cache: hit|miss` make cold/warm delivery directly verifiable; `/health/quran` exposes disk entries, bytes, hits, misses, writes, evictions, and errors for runtime monitoring.

**Done when:** a cold translated page renders and lands on disk (pending/degraded translation responses — those carrying the `x-eq-translation-pending` header — are excluded from the disk cache), a warm one is served from disk, Arabic routes remain build-time prerendered, and TTL/LRU/budget behavior is covered by tests. Production Docker packages the immutable Quran sources read-only for Axum, uses the private container API for SSR, and keeps the public API base for browsers.

## 8. Shared parity fixtures — *partly done*

Rust fixtures (`quran/view.rs`) and web fixtures (`web/src/lib/quran/view/__fixtures__/`) used to live in separate trees under different names with no cross-check. They agreed because they were written to agree; a one-sided rule change was not caught.

**Decision:** one fixture corpus, read by both suites.

**Approach:** a neutral JSON corpus (normalize inputs/outputs, canonical-view units, opener classification, offset maps). Rust reads it via `include_str!`; the web imports it. Neither may hold a private copy.

**Status — half met.** The **normalize parity** corpus IS shared: `web/src/lib/quran/__fixtures__/parity.json`, consumed by Rust (`include_str!` in `normalize.rs`) and imported by the web suite. Deleting a fold rule from `normalize.rs` alone now fails the Rust suite, and deleting it from the web alone fails the web suite. The **canonical-view / opener / offset-map** fixtures, however, remain **two private trees** with zero cross-refs — Rust `quran/testdata/view-uthmani.json` + `view-simple-clean.json` vs web `view/__fixtures__/prefix-cuts.json`. Unifying them is deferred (a separate code task); until then a one-sided view/opener rule change is still not caught.

## 9. Loose ends — *done*

- **U+06DE (`۞`, rub-el-hizb / ruku marker) — resolved (quran.com parity).** 199 ayahs of Uthmani. The **display layer renders it verbatim** (the reader never normalizes → matches quran.com). The **search layer now KEEPS** it (and `۩`, small waw/yeh `ۥۦ`, stop marks `06D6–06DC`) as a searchable token — the search index scans normalized **Uthmani**, so `۞`-bearing queries match the 199 hizb verses just like quran.com (previously these ornaments were stripped and the search corpus was simple-clean, which has none). The offset map bridges only harakat/tatweel/`U+0670` strip-gaps; ornaments ride through 1:1. U+06DD (end-of-ayah) is stripped (0 occurrences); see `docs/quran-normalization-reasoning.md` for the full per-rule validation.
- **Deep-link highlight — resolved.** `VerseRow.svelte` reacts to the canonical ayah hash and applies the reduced-motion-safe `revealed-ayah` marker.
- **Legacy link generation — resolved.** `surahPath` emits route-only links; all ayah links use `surahAyahPath`, which computes the containing local page and canonical `#ayah-S-A` target. The reader still accepts old inbound `?verse=N` URLs as a compatibility redirect, but no generator emits them.
- **Surah-local page tiling — resolved at its owner.** The web metadata owner asserts all 662 clipped pages. Axum serves global navigation families and source text ranges, not local web pages, so duplicating web-local tiling at server boot would create a second owner.
- **Mobile parity — explicit non-goal.** No Flutter/mobile client exists in this repository. When one is scheduled, it must consume the shared normalization/parity contract rather than fork it.

---

## Appendix

Four genuinely-empty source verses — immutable source data, **not** bugs; do not "fix": `fa.safavi 80:39`, `ku.asan 108:3`, `sq.mehdiu 21:56`, `sq.mehdiu 77:14`. Detail in [`docs/research/translation-empty-verses.md`](./research/translation-empty-verses.md).
