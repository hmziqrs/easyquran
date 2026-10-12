# EasyQuran — Quran system

Canonical contract for shipped Quran data, API, delivery, caching, UI locale handling, web auth
isolation, and Rust ingress. Code owns implementation detail; this document records boundaries
that must survive refactors. Parts 1–5 are settled contracts; Part 6 lists known gaps.

Related: [`search-system.md`](./search-system.md) (client search and ranking),
[`settings-system.md`](./settings-system.md) (settings, storage administration, reader
presentation), [`my-plan-raw.md`](./my-plan-raw.md) (owner intent, not delivered architecture).

---

# Part 1 — Hard rules and Quran delivery

## Hard rules

- Quran databases are immutable source material. Never modify or version them.
- `db/` is gitignored. Databases live in R2 and are provisioned onto disk; nothing required after
  a fresh clone may live under `db/`.
- Database identity is its id (`uthmani`, `simple-clean`, `en.sahih`, …), never a digest.
- SHA-256 over Quran data is manual audit only: `just quran-audit`, `bun run audit:arabic` (in
  `web/`), and `bun run verify` inside `db/quran/translations`. Build, boot, runtime, ETag, cache,
  and catalogue paths never hash Quran data. Cryptographic use for CSRF, HMAC, Argon2, PKCE, and
  normalized search-query variance is unrelated.
- Ayah text stays verbatim from SQLite through API, SSG, and display. Never normalize, trim,
  split, reorder, or "repair" source text.
- Boot fails on missing/corrupt sources, invalid XML, broken tiling, invalid coordinates, or
  opener-packaging failure. Runtime integrity uses content and shape assertions, never hashes.
- Arabic reader routes use SSG. Translated reader routes use SSR plus a seven-day disk cache;
  they are never prerendered and never use ISR.
- UI locale and Quran translation-content locale are independent. UI locale never selects a
  source id, API source, DB, migration, request header, or query parameter.
- Reader navigation preserves translation context: build hrefs with `surah*For(ctx, …)`,
  `globalPagePathFor(ctx, …)` and `juzPathFor(ctx, …)`, then `readerHrefFor(ui, …)`. Components
  never use Arabic-only helpers or hand-build reader URL strings.
- No machine translation ships. Quran text is not UI copy.

## Sources and metadata

| File                             | Role                                                                                  |
| -------------------------------- | ------------------------------------------------------------------------------------- |
| `quran-uthmani.sqlite`           | Arabic-search corpus; selectable display script                                       |
| `quran-simple-clean.sqlite`      | Readable API/canonical-view script                                                    |
| `quran-uthmani-annotated.sqlite` | Reader default (`uthmani-annotated`, quran.com word lineage, 2026-10)                 |
| `quran-indopak.sqlite`           | IndoPak mushaf variant (Naveed Ahmad text, 2026-09); see [`indopak.md`](./indopak.md) |
| `quran-tajweed.sqlite`           | Tajweed mushaf variant (Dar Al-Islam colored tajweed via alquran.cloud, 2026-09)      |

- All five expose `quran_text("index", sura, aya, text)` with 6,236 contiguous rows, read
  directly and read-only; no consolidated canonical DB is built. `"index"` is canonical global
  ayah order `1..6236`.
- Tanzil Uthmani and simple-clean embed the bismillah in first ayahs; the annotated, IndoPak and
  Tajweed sources do not — their opener is the source's own 1:1 on a separate row (packaging
  measured 1/0/0/112/1 across numbered/embedded/chapter-flag/separate/absent).
- The annotated edition differs from Tanzil by small-meem pausal density (U+06E2 510→2445,
  U+06ED 99→4807) and three precomposed-maddah variants; sajdah and inline waqf marks are
  identical. Built by `scripts/quran/build-uthmani-annotated.ts`. Arabic search stays on plain
  Uthmani.
- Tajweed `text` carries inline rule markup verbatim (`[h:1468[ٱ]`). Rendering parses it in
  `web/src/lib/quran/view/tajweed.ts`; plain-text views (copy, share, previews, titles) strip it
  there. Stored, wire and non-tajweed text are never touched.
- `quran-data.xml` supplies metadata only: 114 surahs, 6,236 ayahs, 604 pages, 30 juz, 556 ruku,
  240 hizb quarters, 7 manzil, 15 sajda. Page, juz, ruku, hizb-quarter and manzil ranges tile the
  corpus without gaps or overlap. Web consumes generated JSON; Rust parses the XML at boot.
- Diacritized surah names come from Al Quran Cloud, mirrored under gitignored
  `db/quran/surah-names/` and baked into `web/src/lib/data/surah-names.json` by
  `bun run quran:names`. Tanzil's plain names stay the search form (`CatalogEntry.arabicPlain`);
  reader and index render `CatalogEntry.arabic`. The generator refuses to write on any
  base-letter mismatch.
- **Translations**: 378 immutable SQLite dumps across 105 languages — 115 Tanzil, all 75 QuranEnc
  (`quranenc.<iso>.<slug>`; Russian and Bengali only from Tanzil), all 188 QUL (`qul.r<N>.<slug>`;
  91 languages, 16 RTL). Web decodes baked
  `[id, language, languageCode, direction, name, translator, filePath, sizeBytes]` records;
  production artifact selection uses baked id maps only. Four Tanzil verses are blank upstream and
  stay blank (immutable data; display handled in `next-features.md` Feature 1.8): `fa.safavi 80:39`,
  `ku.asan 108:3`, `sq.mehdiu 21:56`, `sq.mehdiu 77:14` — found by scanning the 115 Tanzil packs
  for `length(trim(text)) = 0`; QuranEnc and QUL packs were not scanned.
- **Licensing**: Tanzil is non-commercial; QuranEnc requires attribution, no alteration and
  update-to-latest; QUL publishes no per-resource license (0 of 195 mirrored resources state
  terms) — resolve rights before monetization. Surah names: non-commercial with
  acknowledgement (about page).

**Provisioning.** `deploy/fetch-quran-db.sh` / `just quran-fetch` fills dev and CI disk from baked
R2 paths over credential-free HTTPS; Compose's one-shot `quran-init` fills deployed `quran_data`
(`QURAN_DB_DIR` may bind-mount a host path). Both are idempotent per file, validate expected size
and SQLite shape without hashing, and generate the runtime translation catalogue from the tracked
compact map, so a stale or partial remote catalogue never controls production paths, sizes, or
availability.

**Publishing.** One entry point, `scripts/quran/upload.ts` (`just upload-sqlite`), uploads Arabic,
Tanzil, QuranEnc, QUL, metadata and merged-catalogue objects in one validated plan. Paths and sizes
come from baked web maps; immutable PUTs are create-only and the mutable catalogue publishes last,
only after the immutable set succeeds.

## Script availability

- Production serves the Uthmani family only: `uthmani-annotated` (reader default), `uthmani`
  (search corpus), `simple-clean`. IndoPak and Tajweed are production-disabled on web
  (`web/src/lib/quran/script-availability.ts` — `PRODUCTION_DISABLED_SOURCES`, `isScriptEnabled`,
  `enabledScriptIds`) and on the API (`Script::available_in`, which also filters `/quran/scripts`
  and `/quran/sources`).
- The gate is an environment check only — no middleware, route removal or build exclusion.
  Disabled scripts stay visible in settings as disabled pills ("IndoPak and Tajweed are disabled
  in production."). Hydration, the settings setter, worker reads/staging and settings-document
  apply all refuse a disabled script in production, so a stale preference cannot activate one.
- The quran.com mirror under committed `web/static/qurancom/` is an audit lane, not an app
  dependency (`bun run quran:mirror`, `bun run quran:audit`); QCF page fonts and page-glyph
  scripts are excluded. Licence register: [`qurancom-mirror.md`](./qurancom-mirror.md).

## Rust API

Quran routes live under `/quran`; there is no `/quran/v1` or content-version route. Web routes use
surah slugs; API routes use numeric `1..=114`.

- List/detail/ayah routes for surah, juz, page, ruku, hizb-quarter, manzil and sajda.
- `GET /quran/sources/{id}/surah/{n}` and `/quran/sources/{id}/range?from=&to=` (capped at 300
  ayahs) serve Arabic and translation sources.
- `GET /quran/search?q=…` is Arabic-only exact substring search over normalized Uthmani; client
  fuzzy-name and translation search never cross this boundary.
- `GET /quran/random` is a deterministic ayah of the day.
- `GET /quran/scripts` and `/quran/sources` serve API consumers (web boot uses baked metadata);
  production filters both to the Uthmani family.
- `/quran/health/ready` exposes readiness and bounded public counts. OpenAPI is feature-gated.
- Success payloads are `{ data: T }`; `QuranApiError` owns Quran 400/404/5xx; rate-limit and gate
  middleware keep their app-level error shapes.

## Normalization and canonical view

Rust and web implement one parity-tested Arabic normalization contract
(`web/src/lib/quran/__fixtures__/parity.json`, shared by both test suites; a rule change ships
both implementations together):

- Fold `آ أ إ ٱ → ا`, `ى → ي`, `ة → ه`.
- Strip `U+064B–U+0658`, tatweel `U+0640`, superscript alef `U+0670`, end-of-ayah `U+06DD`.
- Keep standalone Quranic ornaments `U+06D6–U+06DC`, `U+06DE`, `U+06DF–U+06ED` searchable.
- Emit normalized-to-source scalar offsets so highlights map back to verbatim text (web converts
  to UTF-16).

Search transliteration and translation-text normalization are presentation transforms outside
this contract. The canonical view represents body plus opener units; `OpenerKind` and
`OpenerPackaging` stay orthogonal and are asserted at boot. SQLite remains unchanged.

## Caching and fallback

- Rust pins both Arabic corpora in memory; translation DBs use an on-demand Moka pool with
  single-flight construction and count/byte bounds.
- Quran responses are edge-cacheable with weak, id-based ETags. Search variance may digest the
  normalized user query, never corpus data.
- Web pins Uthmani in OPFS. Translation DBs download lazily behind id-keyed active-file pointers
  and prune by LRU, TTL, count and bytes. Downloads stage to an id-scoped temp file, verify baked
  size and Quran structure, then switch `{sourceId, activeFile}` atomically — a partially written
  DB can never become active.
- `/_quran` is the allowlisted same-origin R2 gateway. Runtime remote metadata never selects
  Quran bytes, size, path, or delivery origin.
- Service-worker buckets cover app shell, bounded pages, bounded `__data.json` and the offline
  pack; `/api/` always bypasses Cache Storage.
- Translated HTML uses a server disk cache (`web/src/lib/server/quran-disk-cache.ts`, adapter-bun)
  keyed by build id, source id, route shape, index and bounded UI locale. Cookie-bearing or
  session-setting responses bypass it.
- Browser source reads use one ladder: local → API → local re-check → typed failure; cold
  translations may download through the worker on the last local attempt. Browser and SSR share
  a range fetcher that chunks to 300 ayahs and rejects partial or non-adjacent results.
- API outage memory is passive: transport failures and repeated 5xx open a circuit; no probes.

**Reader corpus.** Arabic primary plus up to five client-only stacked translations. The mushaf
script is a device preference (reader schema v4): Arabic routes prerender and server-render on the
annotated default; other scripts arrive post-paint through the worker/API ladder, staged into OPFS
like translations. Worker boot is fail-soft — if the annotated artifact cannot download it warns
and serves plain Uthmani, and search/translation boot never depend on the reader corpus. Extras
never alter route identity, canonical URL, server cache key or primary delivery; `?more=` is
client-mirrored and stripped from cache keys; selected extras and the primary are pinned against
OPFS eviction.

**Rollout limits (2026-10).** The annotated DB is not yet in R2 — `just upload-sqlite` publishes
it; until then `deploy/fetch-quran-db.sh` skips that key (an on-disk copy is still asserted to
1,605,632 bytes) and the worker serves the plain-Uthmani fallback. Rust `Script::parse` does not
know `uthmani-annotated`, so the degraded worker's API fallback for it 404s.

Engagement-gated translation prefetch uses durable local reading activity; explicit selection
bypasses the gate; Arabic-only reading never counts toward translation downloads.

## Web delivery

- Arabic-source pages build from local SQLite without WASM on first paint. Translated-source pages
  render on Bun; the SvelteKit build runs under Bun too (adapter-bun, `node:sqlite` for the Arabic
  prerender). Node remains for ancillary tooling only.
- URL scheme A: reader paths are prefix-less for the default UI (`/al-baqarah`, `/page/13`,
  `/juz/1`) and `/ar`-prefixed for Arabic UI; there is no `/app` marker. Route families: surah,
  global page, juz and hizb, each Arabic and translated (`/{surah}/t/{lang}/{translator}`,
  `/t/{lang}/{translator}/{page|juz|hizb}/{n}`). One URL per surah — the surah-local page family is
  removed.
- Legacy spellings (`/en/**`, `/app/**`, `/ar/app/**`, `/{slug}/page/N`, `/rub/N`,
  `/t/…/rub/N`) get a single-hop 308 with a bounded public TTL to their live canonical (the old
  `/app` hub → `/surah`; removed page and rubʿ shapes → the surah root at the first ayah). Legacy
  shapes never render reader HTML or enter the disk cache. Numeric aliases (`/2`, `/ar/2`) 308 to
  the slug. Top-level segments are never numeric; `/en/**` and `/ar/**` stay reserved.
- Page geometry is source-independent; ayah-to-page mapping comes from metadata.

**Reader rendering.** Both modes virtualize with TanStack Svelte Virtual over window scroll
(viewport plus 80% of its height each side, bounded 500–1200 px, no item cap). Verse mode renders
measured ayahs as `li` rows in one `ol`, each carrying its mushaf page as metadata for the header,
URL, history and worker reads. Reading mode renders measured page sections of inline text spans
and never instantiates verse actions, translation lanes or note editors. A focused row may stay
mounted outside the range. Height changes above the viewport are scroll-compensated; font, mode
and note changes keep the ayah text anchor; deep links load and mount their target first, and a
jump selects the contiguous loaded page span around it. Primary text loads by page with 900 px
lookahead; the Arabic companion and stacked translations follow mounted page coverage; reading
mode fetches only its selected translation.

---

# Part 2 — UI i18n

## Locale and routing

- Paraglide v2 owns compiled UI copy. Catalogs live under `web/messages/`; generated
  `web/src/lib/paraglide/` is ignored build output.
- `UI_LOCALES`, not the translation catalogue, owns locale routing, direction, switchers, sitemap
  fan-out and prerender discovery.
- Marketing publishes English at `/` and selected Arabic pages under `/ar/`; the publication
  matrix (`MARKETING_PUBLICATIONS`) decides which exist; unsupported pairs are 404.
- Canonical reader paths are the unprefixed `en` forms; Arabic UI adds `/ar`. UI locale changes
  shell copy and direction only; translation segments stay unchanged.
- Arabic-source output is prerendered for both UI locales. Translated-source paths are never
  prerendered; UI locale creates bounded SSR cache variants keyed `__ui-{en|ar}`.
- Reader canonicals and sitemap `<loc>`s use the unprefixed form with an `en`/`ar`/`x-default`
  hreflang triple. Quran-content hreflang describes source content, never shell locale.
- Locale switches are full-reload links that preserve query/fragment; there is no locale cookie,
  storage preference, or Accept-Language override.

Quran passages always declare their own language and direction; UI shell direction never changes
scripture or translation-content semantics.

## Copy ownership

- Structural config stores ids, URLs and membership, not rendered copy.
- Message functions run at render time or inside request-scoped functions, never in module-level
  server-global initialization.
- Shared Nav, Footer, Tweaks, Brand and SEO components receive resolved copy via props.
- Catalog keys are meaning-named and structurally identical across locales; catalogs contain no
  HTML, classes, URLs or icon ids.

## Message chunking

`web/scripts/gen-message-namespaces.ts` maps every message key to exactly one namespace via
`web/i18n-namespaces.json` and emits committed typed barrels under `web/src/lib/i18n/m/`;
unclaimed or multiply claimed keys fail generation.

- Global `#lib/paraglide/messages.js` imports are banned.
- Page copy stays in page namespaces; shared chrome resolves once in the layout; appearance and
  settings panel copy loads lazily.
- `bun run i18n:check` runs before check, lint, test and build.
- Paraglide embeds all locales in each imported message module; revisit per-locale builds at four
  or five locales.

---

# Part 3 — Web auth and cache isolation

- Auth is client-hydrated. Arabic reader/account shells and translated SSR output stay
  auth-neutral. User state never enters build output, disk-cached reader HTML, or shared Cache
  Storage.
- `auth-client.ts` wraps the API. `auth-state.svelte.ts` owns the auth status
  (`unknown | anonymous | authenticated`) and single-flight session bootstrap. The CSRF token
  stays in memory; unsafe requests await bootstrap; session rotation refreshes CSRF state.
- Password, TOTP, verification, recovery, OAuth, passkey, profile and session flows share one
  private/no-store boundary. Rust private routes always return
  `Cache-Control: private, no-store`; public Quran routes keep their immutable/public policy.
- Login, logout and account switch await service-worker acknowledgement while deleting page and
  data caches; OPFS Quran DBs and the offline pack remain. Offline sync queues are partitioned per
  account (see [`next-features.md`](./next-features.md) §9.5).
- Session audit rows bind to opaque tower session ids; rotation updates the binding before
  success, and binding failure destroys the rotated session. Logout revokes audit and live state.
- One immutable `AllowedOrigins` value, parsed at boot, is shared by CORS and the origin guard.
- Production auth/provider/mail/WebAuthn configuration fails closed. Logs and readiness never
  expose credentials, tokens, email addresses or session ids.

---

# Part 4 — Rust ingress, rate limiting, bans, and prewarm

- Request identity resolves before rate limiting; internal SSR traffic authenticates separately
  from external traffic.
- Production derives client identity from the edge only behind restricted origin ingress;
  `deploy/README.md` owns the procedure.
- Rate limiting and ban escalation run in Rust ahead of handlers. Their values and criteria live
  in code, not in documentation. Ban state is bounded and persisted; operator endpoints are
  admin-only and `no-store`.
- The translation-pool durable score measures API demand across restarts, not reader popularity.
  Decay happens in Rust; prewarm is bounded, background-only and never blocks boot. Public
  readiness excludes pool budgets, hit rates and demand ranking.

---

# Part 5 — Divergences from `my-plan-raw.md`

[`my-plan-raw.md`](./my-plan-raw.md) is read-only owner intent. Shipped architecture wins:

1. Translated pages use SSR plus a seven-day disk cache and are never SSG (surah shapes alone
   would be ~76,000 routes).
2. Translation context spans surah, global page, juz and hizb routes; navigation never falls back
   to Arabic context. Rubʿ URLs are retired (308 to the quarter's first ayah).
3. "SSG-last" applies only to Arabic. Translation recovery is local DB → API → matching server
   data.
4. The durable translation-pool score measures API demand, not reader popularity (Part 4).
5. Surah and range routes keep `+page.server.ts`; post-paint worker upgrades do not remove the
   SvelteKit server-load dependency.
6. Authentication stays client-hydrated and absent from every shared output and cache (Part 3).
7. Production artifact specs come from baked id maps; staged validation and atomic pointer
   switching mean an incomplete download never becomes active (Part 1, Caching).
8. Mushaf script is a device-side reader preference, not a route dimension. Arabic SSG renders
   the annotated default; other scripts never prerender and arrive post-paint; production gates
   IndoPak and Tajweed (Part 1, Script availability).
9. One URL per surah: the surah-local page scheme is removed and page addressing is global
   `/page/N` only; scroll position rides in `?v={surah}:{ayah}` and `history.state`, never the
   path.
10. URL scheme A: `en` is unprefixed, `ar` lives under `/ar/`, the `/{ui}/app` double prefix and
    the `/app` hub are gone, and one first-match 308 table maps every legacy spelling with
    `cache-control: public, max-age=86400`. `/en/**` is permanently reserved. `/search`,
    `/settings`, `/bookmarks` and `/yours` have `/ar` twins via the reroute table. Background:
    [`history.md`](./history.md) (Navigation audit and URL scheme A).

---

# Part 6 — Known gaps and product decisions

- Arabic marketing copy beyond home awaits fluent review; unpublished pairs remain 404.
- Landing/app-home English literals and several physical RTL utilities/arrows remain.
- `IP_SOURCE` documentation has one deploy README location mismatch.
- Word-level navigation and shared canonical-view fixtures remain deferred.
- Future UI locales require SSG and SSR-cache capacity review.
- Translation contribution/review ownership remains undefined.
- Mobile is outside this repo; any future client must share normalization fixtures and
  immutable-source contracts.
