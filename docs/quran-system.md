# EasyQuran — Quran system

Single source of truth for Quran data, API, delivery, caching, and Arabic search normalization. **Condensed** — every claim is verified against code; implementation detail lives in code, not here. Supersedes the former `quran.md`, `caching-architecture.md`, and `quran-normalization-reasoning.md` (the full per-rule quran.com probe evidence and the dated caching snapshot remain in git history).

Part 1 is settled ground (contracts). Part 2 is status.

---

# Part 1 — settled

## Hard rules

- Quran databases (Arabic + every translation) are **immutable**. Sourced from Tanzil.net. No modifications, no versioning.
- **No content versioning.** No `contentVersion`/`searchVersion`, no version segment in R2 keys, no hash-keyed cache dirs. A database's identity is its **id** (`uthmani`, `simple-clean`, `en.sahih`, …).
- **Ayah text is verbatim.** The source `text` is never normalized, split, trimmed, or reordered — SQLite read → in-memory store → JSON → SSG HTML. `9:1` has no basmala, `1:1` *is* the basmala, `27:30` carries one mid-ayah, `95:1`/`97:1` carry a shadda spelling: all correct source state, none of it a cleanup target.
- **Integrity: SHA-256 is manual audit, never automated.** A DB's identity is its id; sha never names a cache key, profile, or catalogue row, and never digests **Quran data** at runtime. The one runtime sha over text is a digest of the normalized **search query** (user input), folded into `/search`'s ETag for variance — never over the corpus. The corpus + sqlite digests are checked by hand (`just quran-audit` [from `rust/backend/api/`], `pnpm audit:arabic`, `pnpm verify`) — not at boot, not on download (those size-check only, `verifyBytes`). Runtime protection is **content asserts** (6236 rows, tiling, ayah keys, packaging). Crypto sha (CSRF/HMAC/Argon2/PKCE) is unrelated and stays.
- **Boot is fail-fast.** Missing/corrupt source, XML failure, tiling failure, or wrong bismillah split count → exit non-zero (shape asserts, not sha).
- **Arabic renders SSG.** Translated pages render SSR + disk-TTL. Never ISR.

## Data

- `quran-uthmani.sqlite` = display **and search** corpus (quran.com parity — quran.com indexes the full Uthmani text; ornaments are searchable: ۞→199, ۩→15). `quran-simple-clean.sqlite` = a readable API/canonical-view script (still Rust-resident + audited; no longer the search corpus). Both: `quran_text("index" PK, sura, aya, text)` + `idx(sura,aya)`, 6236 rows, read **directly, read-only**. Not consolidated; no canonical db is built.
- `quran-data.xml` = metadata only (no verse text): 114 surah, 6236 ayah, 604 pages, 30 juz, 556 ruku, 240 hizb-quarter, 7 manzil, 15 sajda. Web consumes one compact `web/static/quran-meta/quran-data.json`; Rust parses the XML in memory at boot. That JSON's header row (`[digest, "1.0", "cc-by"]`) is **provenance only** — format-validated at load, never compared, never keyed on.
- **Global ayah index:** `quran_text."index"` is canonical — contiguous `1..6236`, unique, ordered by surah then ayah, `= sura.start + aya` (XML `start` zero-based, `aya` one-based). Asserted at boot; `/range` and its cap of 300 rest on it.
- **Marker families tile `[1,6236]`** with no gap or overlap (page/juz/ruku/hizb-quarter/manzil). `<quarter>` carries no hizb attribute — derive: `hizb = ((i-1)/4)+1`, `quarterInHizb = ((i-1)%4)+1`.
- **No FTS.** Search = normalize + substring-scan the 6236 **Uthmani** rows (ornaments retained → searchable, matching quran.com).
- Translations: 115 dumps → one `<id>.sqlite` each, same schema, across 44 languages. 186 MiB total: p50 1.25, p95 3.10, max 12.43 MiB. **Non-commercial license** — revisit if the project monetizes. The web's baked catalogue (`web/src/lib/data/translations.json`) is a flat positional array — `[id, language, languageCode, direction, name, translator, filePath, sizeBytes]`, decoded by `TranslationField` in `quran/catalogue.ts`; no sha256 (id-keyed).

## API — Rust `/quran`

Module `quran_v1`, nested at `/quran`. **No `/quran/v1`. No `/version`.**

One shape per navigation family — `/{family}` list, `/{family}/{n}` detail, `/{family}/{n}/ayahs` text — for `surahs`, `juzs`, `pages`, `rukus`, `hizb-quarters`, `manzils`, plus `sajdas` (list + detail). Also:

- `GET /quran/ayahs`, `/quran/ayahs/{surah}/{ayah}`, `/quran/ayahs/{verseKey}` (redirect).
- `GET /quran/sources/{id}/surah/{n}` → `Envelope<QuranSurahTextDto>`; `GET /quran/sources/{id}/range?from=&to=` → `Envelope<RangeText>` (cap 300). **This pair is also the translation read API** — it is already source-parameterized, so translations need no new routes, only a resolver.
- `GET /quran/search?q=…` — substring scan over normalized Uthmani (ornaments retained → searchable). A query must be 3..=64 Unicode scalars after normalization **or** contain a Quranic ornament mark (so a lone `۞`→199 / `۩`→15 works like quran.com, without relaxing the 3-char floor for plain text; helper `contains_searchable_ornament` matches `U+06D6–06DC | U+06DE–06ED`). `kind` is an **output** discriminator; only `ayah` is produced today (`opener` reserved, never emitted).
- `GET /quran/random` — deterministic ayah-of-the-day (date-seeded LCG, not RNG).
- `GET /quran/scripts` (R2 artifacts: id, sizeBytes, downloadUrl), `GET /quran/health/ready`, `/quran/openapi.json` under the `openapi` feature.
- **Rate limits, per IP:** 600/60s general, 30/60s on search — `/search` mounts on a separate router that carries the tighter 30/60s and remains under the merged 600/60s `quran-v1` ceiling too, so the CPU-heavy scan is **double-limited by design** (its own 30/60s trips first).
- `Envelope<T> = { data: T }`. `QuranApiError` is the closed error shape — nested `{ error: { code, message, detail } }` for 400 / 404 / 5xx — so a failed read never becomes an empty surah. The rate-limit layer's 429 uses the app `ErrorResponse` envelope (`{ type, status, retryAfter, … }`), the gate's store-unavailable 503 uses the gate's flat shape (`{ error, message }`) with **no** `Cache-Control` — neither is `QuranApiError`.

**Web routes are slug-based** (`al-fatihah`), the API numeric (`1..=114`). The web adapter resolves slug → number before calling Axum; the backend owns no slug table.

Auth lives outside `/quran`: web OAuth code flow (google/apple/facebook/github) + mobile `POST /auth/{google,apple,facebook}/v1/token` (client SDK token → cookie session, **not** JWT; GitHub is web-only). The three `/token` routes are CSRF-exempt.

## Normalization + canonical view — parity contract

One rule set, two implementations (Rust + web Worker) returning **identical ordered results**, validated against quran.com (the reference platform). quran.com indexes the full Uthmani text and **keeps** standalone Quranic ornaments searchable (`۞`→199, `۩`→15, small waw/yeh `ۥۦ` narrow, stop marks literal); its index **strips** intra-cluster combining marks (harakat, maddah `U+0653`, tatweel, superscript alef) so bare queries match. **We match that.**

- **Strip set (explicit, byte-identical Rust↔web):** `U+064B–U+0658` (harakat + maddah `U+0653`/hamza `U+0654`), `U+0640` (tatweel), `U+0670` (superscript alef), `U+06DD` (end-of-ayah, 0 occ). Stripping maddah `U+0653` is required for substring search — Tanzil Uthmani encodes alef-madda decomposed as `alef+0653` in ~3051 verses, so a bare query must not carry it; this matches quran.com's index behavior, no stemmer needed.
- **Keep (searchable tokens):** standalone ornaments `U+06D6–06DC`, `U+06DE`, `U+06DF–06ED` (incl. small waw/yeh `U+06E5`/`U+06E6`, sajda `U+06E9`).
- **Fold:** `آ أ إ ٱ → ا`, `ى → ي`, `ة → ه`.
- **offset map:** normalization emits a normalized-scalar → source-scalar map, so a hit found in normalized space highlights correctly in the rendered script (converted to UTF-16 for the web). The map bridges the harakat/tatweel/`U+0670` strip-gaps; kept ornaments ride through 1:1. The **display layer never normalizes** — every mark renders verbatim, matching quran.com.
- **canonical view** splits each surah into **body + opener** units (opener = rank 0, ayah = rank 1). **Opener classification is two orthogonal enums:** `OpenerKind` = `Verse | Header | None` × `OpenerPackaging` = `NumberedAyah | EmbeddedPrefix | ChapterFlag | SeparateRow | Absent`. Split counts (1 / 112 / 1) asserted at boot. Surahs 95 & 97 carry a shadda variant (`بِّسْمِ`) — match diacritic-insensitively, never exactly.
- A rule change ships new Rust + web together; it never mutates a sqlite. A shared neutral parity corpus (`web/src/lib/quran/__fixtures__/parity.json`, consumed by Rust `include_str!` + web import) enforces Rust/web identity.

### Normalize verdict vs quran.com (14 rules)

Reverse-engineered via live `api.quran.com/api/v4/search` A/B `total_results` probes (hit count at `response.search.total_results`). quran.com's normalize function is closed-source; verdicts need re-deriving if its backend changes.

| Rule | Codepoint(s) | Our behavior | quran.com search | Verdict |
|---|---|---|---|---|
| **F1** alef madda | `U+0622` آ | fold → ا | fold | **MATCH** |
| **F2** alef hamza above | `U+0623` أ | fold → ا | fold | **MATCH** |
| **F3** alef hamza below | `U+0625` إ | fold → ا | fold | **MATCH** |
| **F4** alef wasla | `U+0671` ٱ | fold → ا | **erratic** (folds some words, returns 0 for others — its inconsistency) | **DISPUTED** |
| **F5** alef maqsura | `U+0649` ى | fold → ي | fold | **MATCH** |
| **F6** ta marbuta | `U+0629` ة | fold → ه | fold | **MATCH** |
| **R1** harakat + maddah | `U+064B–U+0658` | strip | strip (index-side; keeps bare queries working) | **MATCH** |
| **R2** superscript alef | `U+0670` ٰ | strip | strip in chair role (dominant) | **MATCH†** |
| **R3** tatweel | `U+0640` ـ | strip | strip | **MATCH** |
| **R4** stop marks | `U+06D6–U+06DC` | keep | keep (literal, indexed) | **MATCH** |
| **R5** rub el hizb | `U+06DE` ۞ | keep | keep (199 results) | **MATCH** |
| **R6** small waw/yeh | `U+06E5`/`U+06E6` ۥۦ | keep | keep (search-narrowing) | **MATCH** |
| **R7** sajda | `U+06E9` ۩ | keep | keep (15 sajda verses) | **MATCH** |
| **R8** end-of-ayah + signs | `U+06DD`/`U+06EA–U+06ED` | keep `06EA–06ED`; strip `06DD` | keep `06EA–06ED`; `06DD` 0-occ | **MATCH** |

**Tally: 13 MATCH / 1 DISPUTED (F4).** All 14 match on **display** (both render Uthmani verbatim); every divergence is search-layer only. `R2` is MATCH†: matches in the dominant chair role; the only residual is the alef-maqsura (`ىٰ`) positional subset, where our uniform strip is the forgiving choice. **F4** (wasla) is DISPUTED because quran.com is itself internally inconsistent (folds wasla→alef on its index but not uniformly on the query side) — our uniform `ٱ→ا` fold is the correct, more-forgiving version; no rule can match an erratic reference.

R4–R8 were formerly DIVERGE (we stripped ornaments + searched simple-clean, where they don't occur). **Resolved** by switching the index to normalized Uthmani + keeping the ornaments — standalone marks are now searchable tokens on both platforms. Proven by `tests/quran_v1.rs::search_finds_ornament_bearing_query` and `search_allows_lone_ornament_query` (lone `۞`→199).

## Caching

Two domains, no shared cache; they meet only at the HTTP edge and the `/quran` JSON contract.

- **Rust, in memory:** both Arabic corpora load at boot and stay resident (never evicted); `Script` accepts `uthmani` | `simple-clean`. The `SearchIndex` is built once from normalized Uthmani. Translations are on-demand (moka pool, single-flight, byte+count bound — see constants).
- **Rust, responses:** no server-side response store. Reads come straight from the resident corpus; every response is edge-cacheable — weak `ETag` = `quran-corpus : canonical key` (a static id-based tag; the corpus is immutable so a constant ETag is correct — never a digest over Quran data). `/search` is the exception: its canonical key folds in a sha-256 digest of the normalized **query** (user input) for variance. `If-None-Match` → 304. `Cache-Control` per family (see constants). `QuranApiError`-originated 5xx → `no-store` (the rate-limit gate's 503 emits no `Cache-Control`).
- **Web, OPFS:** the `uthmani` Arabic corpus is eager on boot and pinned (never evicted); `simple-clean` is no longer worker-downloaded (search uses the Uthmani index). **Key = `spec.id`** (identity is the id; the prior `spec.sha256` key was a cache-dir violation). Downloaded bytes are size-checked only. Translations lazily download into OPFS and prune LRU + size + TTL.
- **Service worker:** four Cache-Storage buckets (app shell, pages LRU, `__data.json` SWR, offline pack) + IDB meta; atomic install/activate/handoff lifecycle; contract shared via `./lib/offline/messages` (only `META_DB`/`META_STORE` + `normalizeDataKey` genuinely duplicated).
- **SSR disk-TTL (translated pages only):** `adapter-node` SSR HTML disk cache for the four `/t` reader routes (Arabic routes stay SSG/prerendered). Canonical `(sourceId, kind, index[, localPage])` keys inside a SvelteKit build-id namespace (not Quran versioning). Atomic tmp+rename writes; `Server-Timing` + `X-EasyQuran-Quran-Cache: hit|miss`; `/health/quran` (web) reports it.
- **`/_quran` artifact gateway:** same-origin streaming proxy to R2 (allowlist of baked keys, `Range` forwarded, immutable caching, weak id-based ETag). No bytes rewritten or hashed.
- **R2 layout:** `tanzil/arabic/<file>.sqlite`, `tanzil/translations/sqlite/<id>.sqlite`, `tanzil/translations/index.min.json` (mutable, `max-age=300, must-revalidate`), `tanzil/quran-data.xml`. Everything except the catalogue is `immutable`.

### Key constants

| Value | Where |
|---|---|
| 6236 / 114 / 300 | `VERSE_COUNT` / `SURA_COUNT` / `RESPONSE_CAP` (`store.rs`) |
| `"quran-corpus"` / `"tanzil-{id}"` | ETag tags (`store.rs` / `controller.rs`) |
| Arabic: `max-age=300, s-maxage=86400, swr=604800, sie=604800` · search: `max-age=60, s-maxage=300` · `/random`: `immutable` · 5xx: `no-store` | `cache.rs` |
| 600 / 30 per 60s (search double-limited) | `main.rs` |
| 8 / 48 MiB / 1800s / `BUILD_CONCURRENCY=2` | translation pool bounds (`settings.rs` / `translation_pool.rs`) |
| 30d / 12 / 128 MiB / 4 | OPFS `TTL_MS` / `CAP_COUNT` / `CAP_BYTES` / `TRANSLATION_DB_CAP` (`opfs-retention.ts` / `quran.worker.ts`) |
| `PINNED_ARABIC=[uthmani]` | `quran.worker.ts` (simple-clean no longer worker-downloaded) |
| 7d / 256 MiB | SSR disk-TTL / budget (`quran-disk-cache.ts`) |
| 10s / 24h | rate-limit `FLUSH_INTERVAL_SECS` / `MAX_ATTEMPT_WINDOW` |

## Web delivery + pagination

- **Arabic = SSG.** Build-time `node:sqlite` reads uthmani and prerenders reader pages — real HTML for SEO/first paint; no WASM on the critical path.
- **Translated pages = SSR on Bun.** Production runs the adapter-node server on the **Bun** runtime (a deliberate choice — Bun holds translation-SSR throughput at roughly half Node's resident memory); Node is **build-only** (Arabic prerender reads sqlite via `node:sqlite`, which Bun lacks).
- **Page geometry is source-agnostic** — it describes Mushaf geometry, not text, so translations reuse it. Global page `1..604`; surah-local page (662 total); juz `1..30`. Routes: `/app/<surah>`, `/app/<surah>/page/<n>`, `/app/page/<globalPage>`, `/app/juz/<n>` — all prerendered.
- **Ayah → page is computed, never stored.** `surahLocalPageForAyah(surah, ayah)` → `{ localPage, globalPage }`. A deep link resolves to its containing local page and scrolls the ayah to a stable centered position (small-screen safe).
- **Reader loading:** each surah route reads **exactly one bounded local page** (cross-surah guarded). Continuous scroll pulls adjacent local pages on demand via Worker range reads; a ~5-page virtual window bounds the DOM.

---

# Part 2 — status

**Done (shipped):** artifact URL contract · no-sha identity · translation pool + metrics · `/sources` catalogue · `/t` translation reader routes · OPFS retention · production Bun SSR + disk-TTL · normalize-parity fixture · **quran.com search parity** (index → Uthmani, ornaments kept + searchable, ornament-mark eligibility exception) · loose ends.

**Remaining / deferred:**
- **Reader UI-locale prefix (planned in [`i18n.md`](i18n.md), not shipped by this document's current-route claims).** The internal SvelteKit reader grammar remains `/app/**`. The future public forms `/en/app/**` and `/ar/app/**` de-localize to it and select UI copy only; legacy `/app/**` temporarily redirects to the equivalent `/en/app/**` route without rendering or caching a second document. Arabic-source pages will gain one bounded static output per UI locale. The four translated-source route families remain SSR with their 7-day disk cache, never SSG; that HTML cache gains a bounded `UiLocale` dimension while the Quran identity and existing source-context key stay id-based. No UI locale reaches Rust, a Quran database, the translation catalogue, or source selection.
- **Canonical-view / opener / offset-map fixtures are still two private trees** (Rust `quran/testdata/view-*.json` vs web `view/__fixtures__/prefix-cuts.json`) with no cross-check. Only the normalize corpus (`parity.json`) is shared. Unifying them is a separate code task.
- **Word-level navigation** — reuses ayah page math with a finer in-ayah highlight; word-offset hash grammar undecided. No code yet.
- **Mobile client** — explicit non-goal in this repo. When scheduled, it must consume the shared normalization/parity contract rather than fork it.

---

# Part 3 — divergences from `my-plan-raw.md`

`docs/my-plan-raw.md` is owner-authored and read-only; it records intent, not the delivery architecture this repository ships. The seven boundaries below are load-bearing and must not be blurred by an implementation or a later doc edit. They are recorded here (not back in the raw plan) per the implementation plan's W9.

1. **Translated pages are SSR + 7-day disk cache, never SSG.** Prerendering the translated top-level surah route alone is `114 × 115 ≈ 13,110` pages, and the surah shape does not stop there: surah-local pages add `548 × 115 ≈ 63,020`, so **the surah shape alone is ~76,000 routes before juz and global pages** — the 13,110 number understates the cost by roughly six times, so quote the full figure, not just the top-level count. Translated prerender would also need a live API at build time (`quran-translation-page.ts` fetches ranges over HTTP), which Arabic prerender does not — a translated build artifact is not achievable from static input. SEO is served by hreflang alternates (`routes/sitemap.xml/+server.ts:31-42`), not by prerendered HTML. This deliberately diverges from `my-plan-raw.md:19,22`; no `/app/t/**`, `/app/[surah]/t/**`, or their future `/{ui}/app/...` public forms may be described as SSG or prerendered.
2. **Translated delivery spans four route families, not surah-only.** One source context flows through surah, surah-local-page, global-page, and juz routes — the same four shapes the sitemap groups with language alternates (`routes/sitemap.xml/+server.ts:62-82`). Reader navigation must preserve translation context via the `surah*For(ctx, …)` family and cannot fall back to Arabic (guarded by `nav-guard.test.ts`). This is the runtime boundary behind the shorter intent in `my-plan-raw.md:19`.
3. **"SSG-last" holds for Arabic only.** Arabic reader routes are prerendered, so their `__data.json` is a static build artifact served from the CDN / `eq-data-v1` — a real SSG-last tier while the static origin is up. Translated routes have no SSG artifact; their recovery order is local DB → API → matching server data, and is never called "SSG-last".
4. **The durable pool signal is API demand, not reader popularity.** The Rust translation pool only sees requests that reach the process; translated pages are SSR behind a 7-day disk cache and warm clients read their local worker DB, so pool hits measure **API demand across process restarts** — a more popular translation may produce *fewer* hits once caching works. Moka TinyLFU owns in-process popularity admission; the persisted signal is conceptually an API-demand score but is literally the `score` column (REAL, decayed at read via `score * 0.5^(elapsed / half-life)`) in the `translation_popularity` table (migration `m000003`) — not "reader popularity" (W1).
5. **Surah and range routes keep `+page.server.ts`.** SvelteKit resolves `__data.json` before new route props reach the component, so warm navigation still depends on the server load. Surah adjacent-page reads and the post-paint range swap (W6) upgrade content *after* paint; neither removes the server-load dependency, and neither does a universal `+page.ts`. Removing it is a route-architecture workstream, not a claim that hydration already replaced SSG/SSR.
6. **Authentication is client-hydrated everywhere.** Arabic reader and account shells are prerendered and auth-neutral; the four translated reader route families remain auth-neutral SSR. User state enters neither the build output, the translated HTML disk cache, nor Cache Storage (W8) — no per-user content in any shared cache. **W8e durable session binding:** the one-time web-auth enablement is a single clean re-authentication boundary — boot reconciliation (`services/auth.rs::reconcile_unbound_sessions`) not only revokes the `user_session` audit rows that pre-date the `auth_session_binding` table, it also enumerates the live tower-session store (`session_store.rs::delete_unbound_auth_sessions`) and forcibly removes the corresponding live AUTH sessions (matched by the `rux_auth` Record key, not a per-id binding lookup, since none exists for pre-binding sessions); anonymous sessions are preserved. This affects only that one cutover — after it, every session is bound and the sweep is a near-no-op on subsequent boots. **W8a origin sharing:** the single boot-built `AllowedOrigins` (`utils::cors::allowed_origins_from_env`, resolved once at `main.rs:147`) is stored as the `allowed_origins` field of `AppState` (cloned in once at construction — it is `Arc`-backed, so the clone is cheap and shares the parsed set) and read by `origin_guard` from the `AppState` extension (`req.extensions().get::<AppState>()` in `middlewares/cors.rs`), satisfying the W8a step-1 wording in `docs/implementation-plan.md` literally. The earlier standalone `axum::Extension(AllowedOrigins)` layering was removed when the field was added. The load-bearing contract is unchanged — one immutable value built at boot, shared by `CorsLayer` (which still takes the same `header_values()` list at boot) and the guard, with no per-request env read or parse; fail-closed behavior is preserved (a missing `AppState` extension rejects).
7. **Artifact safety is the W10 executable contract.** Production download specs are reconstructed from baked `{id, sizeBytes, r2Path, sameOriginDeliveryPath}` maps; remote catalogue metadata may describe availability but never selects Quran bytes, size, or a delivery origin, and `eq-data-v1` metadata is bounded the same way. Downloads stage into an id-scoped temp file, then validate exact baked size plus content invariants (6,236 contiguous rows, tiling, ayah keys, packaging — content asserts, never hashes) before an IDB `{sourceId, activeFile}` pointer switches atomically and the prior file is removed. A partially written DB can never become active; invalid legacy files stay inactive and trigger normal redownload (W10a/W10c).

---

## Appendix

Four genuinely-empty source verses — immutable source data, **not** bugs; do not "fix": `fa.safavi 80:39`, `ku.asan 108:3`, `sq.mehdiu 21:56`, `sq.mehdiu 77:14`. Detail in [`docs/research/translation-empty-verses.md`](./research/translation-empty-verses.md).
