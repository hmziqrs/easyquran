# EasyQuran — Fine-Grained Feature Prompt (v2)

Grounded in current code. Implementation brief for the next feature batch: translation recents/labels,
bookmark collections with context, reading sessions, homepage widgets, personal analytics, azkar
snippets, and author/source grouping.

---

## 0. Context the implementing agent must load first

- Read `AGENTS.MD` (root), `web/README.md`, `rust/backend/api/AGENTS.md`. Load
  `svelte-core-bestpractices` + `svelte-code-writer` before touching `web/`. Caveman mode active
  (also for subagents).
- Gates must stay green: `pnpm check` (warning-free — `--fail-on-warnings`), `pnpm lint`
  (`--deny-warnings`), `pnpm test`. Lint rules live in `web/vite.config.ts` → `lint.rules`; never add
  `.oxlintrc.json`.
- Hard rules: Quran DBs immutable; `db/` gitignored; DB identity = **id**, never sha (guards:
  `rust/backend/api/tests/quran_v1.rs`,
  `web/src/lib/quran/__tests__/catalogue-sha-guard.test.ts`); translated reader routes are
  **SSR + 7-day disk cache, never SSG** (prerender stays Arabic-only); all reader hrefs use the
  `surah*For(ctx,…)` family in `web/src/lib/data/quran.ts` (guarded by
  `web/src/routes/(application)/__tests__/nav-guard.test.ts`); no nested ternaries; es-toolkit named
  imports; TanStack Hotkeys wrapper (`web/src/lib/hotkeys.svelte.ts`, dynamic import only).
- Sync template: client engine (`web/src/lib/sync/engine.svelte.ts`, `outbox.ts` IDB,
  `registry.svelte.ts`, `SyncDomain` in `types.ts`) + domain module
  (`web/src/lib/bookmarks/domain.ts`) + backend LWW module
  (`rust/backend/api/src/modules/bookmark_v1/`). Auth = backend session cookie + CSRF
  (`web/src/lib/auth/auth-client.ts`). Firebase = GA4 + FCM only — no Firestore, no Firebase Auth.
- Reader state today: one device-local blob `easyquran.reader` (schema v4) —
  `web/src/lib/stores/reader-core.svelte.ts:37-52`, decode `reader-persistence.svelte.ts:41-133`,
  explicit-field write `:181-210`. Only bookmarks+folders sync. Migrations end at `m000010`; next is
  `m000011`.
- Gap IDs to align with: `docs/remaining/feature-gap-catalogue.md` — P01/P02/P03/P04/P05/P08, R04,
  R10, R11, D06, D08.
- **Never repurpose `translation_popularity`**
  (`rust/backend/api/src/services/translation_popularity_store.rs`) as reader popularity — docs
  explicitly say it measures API demand (`docs/quran-system.md:330,346`).

---

## Feature 1 — Recently used translations: recency ranking + proper labels

**Current state**

- Picker `web/src/routes/(application)/_reader/TranslationModal.svelte` groups by language rail, sorts
  by `Intl.Collator` name (`:160-179`), no recency.
- Only recency today: `recent[]` (cap 8, `READING_RECENT_MAX`) in
  `web/src/lib/stores/reading-text.svelte.ts:24,95-121`, surfaced as quick chips via
  `readingQuickPicks` (`web/src/lib/reader/reading-flow.ts:22-32`). Stacked list
  (`stacked-translations.svelte.ts`) and search selection (`search-selection.svelte.ts`) are unordered.
- Provenance derivable only from id prefix: `translationSourceOf(id)` in
  `web/src/lib/quran/catalogue.ts:25-31` → `tanzil | quranenc | qul`. Tooltip already shows translator
  - provenance + language + size (`TranslationModal.svelte:666-713`).

**Build**

1. New persisted MRU store `web/src/lib/stores/translation-recents.svelte.ts`, key
   `easyquran.translations.recent` (v1 envelope like other stores:
   `{v, entries:[{id, lastUsedAt, useCount}]}`, cap ~20). Record on **every explicit selection**: modal
   pick, quick chip, search selection, and landing on a `/t/…` route (`SurahReader.svelte`
   `initial.normalization.sourceId`).
2. Modal: add a pinned "Recent" group (global, above rails) + recency boost inside the active rail.
   Untouched users see today's behavior exactly (no reorder until real usage exists).
3. Labels: visible secondary line = `translator — name · Provenance`; show provenance badge
   (Tanzil / QuranEnc / QUL). Handle the attribution-mismatch case (`quranenc.en.saheeh`: name
   "Saheeh International", translator "Noor International Center") without merging them.
4. Persist per-author last-used variant (pairs with Feature 8): `Map<normalizedTranslator, id>`.

**Acceptance**: recents survive reload; ranking only changes on explicit selection; no new catalogue
fields required (derivation only); modal labels show author + source for every row.

**Touch**: `TranslationModal.svelte`, new store, `reading-text.svelte.ts` (keep its 8-cap chips,
delegate ordering to MRU), `reading-flow.ts`.

---

## Feature 2 — Bookmark folders → named study sets (daily reading / surah-ayah learning)

**Current state**

- Flat single-level folders, **authed only**: `BookmarkFolder {id, name, createdAt, updatedAt}`,
  `Bookmark {id, folderId|null, surah, ayah, createdAt, updatedAt}`
  (`web/src/lib/bookmarks/schema.ts:16-48`); backend `bookmark_folders` + `bookmarks`
  (`migration/src/m000007_bookmarks.rs`), unique `(user_id, surah, ayah)` (`m000008`). No tags, order,
  bulk, nesting (P05 remaining).
- Anon users only get a local boolean map (`reader-core.svelte.ts:46`) — no folders at all.

**Build**

1. Folder metadata: `kind: "generic" | "daily" | "learning"`, optional `position` (manual order,
   integer, LWW), optional plan metadata `{targetAyahsPerDay?}` for `kind:"daily"`.
2. Backend: extend `bookmark_folders` (m000011) + wire fields in `bookmark_folder/slice.rs` + validator
   (`bookmark_v1/validator.rs`, mutation cap 200 stays).
3. UI: folder sections sorted by `position` then name; per-folder count (exists); bulk move/delete
   (P05); folder kind picker in `FoldersPanel.svelte`; per-folder progress bar derived from session
   progress (Feature 4) when `kind:"daily"`.
4. Anon: decide either "folders require sign-in" (keep) or a local-only folder mode; don't silently
   degrade.

**Acceptance**: manual order survives sync; `kind` round-trips LWW; no change to per-verse uniqueness
in this feature (that's Feature 3).

**Touch**: `web/src/lib/bookmarks/schema.ts`, `store.svelte.ts`, `domain.ts`, `FoldersPanel.svelte`,
`BookmarkRow.svelte`, `bookmarks/+page.svelte`, Rust `migration/src/m000011_*.rs` (first new
migration), `db/sea_models/bookmark_folder/*`, `bookmark_v1/*`.

---

## Feature 3 — Fine-grained bookmarks: translation + session context

**Current state**

- Toggle captures only `verseKey` (`VerseTools.svelte:80` → `bookmarks.toggleVerse`); `Bookmark` has no
  `sourceId`, note, page, or anchor. Bookmarks list opens **Arabic-only**
  (`ARABIC_CTX = surahRouteContext("uthmani")` at `bookmarks/+page.svelte:23`, `yours/+page.svelte:112`),
  even though the route context exists (`surahRouteContext(sourceId)`, `quran.ts:87-91`).
- Unique index `(user_id, surah, ayah)` = one row per verse, ever.

**Build**

1. Extend `Bookmark` with `sourceId?: string|null` (translation context), `note?: string` (link to
   local notes later), optional `page` + `anchorRatio` (P03 reading bookmark), and
   `sessionId?: string|null` (Feature 4).
2. Decide uniqueness (open decision, see §9): keep `(user_id, surah, ayah)` and treat `sourceId` as
   "last context", or relax to `(user_id, surah, ayah, sessionId)`/`+sourceId`. SQLite NULL-distinct
   caveat: use `''` sentinel or partial indexes, not bare NULLs.
3. Migration m000011/m000012: normalize + dedupe + recreate index (pattern:
   `m000008_bookmark_verse_unique.rs`). Update wire slices, `bookmark_v1/validator.rs` ranges, client
   `schema.ts` decoders.
4. Capture at toggle time: read current `sourceId` from route context (`routeContextFromParams` /
   `SurahReader` initial) and active session id; store both.
5. List/resume: build hrefs with `surahAyahPathFor(surahRouteContext(bookmark.sourceId ?? "uthmani"), …)`
   so bookmarks reopen in their saved translation; show source badge on `BookmarkRow`.
6. Share seam: keep `reader-share.svelte.ts` untouched here; range/copy work is Feature 7.

**Acceptance**: bookmark created on a `/t/…` route reopens in that translation; session-scoped
bookmarks filter by active session; anon path unchanged; sync LWW + stale-queued-upsert resurrection
caveat documented (no tombstones today).

**Touch**: `schema.ts`, `store.svelte.ts`, `domain.ts`, `VerseTools.svelte`, `BookmarkRow.svelte`,
`bookmarks/+page.svelte`, `yours/+page.svelte`, Rust migration + `bookmark_v1` +
`db/sea_models/bookmark/*`, tests `rust/backend/api/tests/bookmarks_v1.rs`.

---

## Feature 4 — Multiple reading sessions (daily / weekly / shared seam)

**Current state**

- One global slot: `lastRead {num,n,sourceId?}` + `lastReadAnchor {verseKey,localPage,ratio}`
  (`reader-core.svelte.ts:21-31`); `recents` cap 8 keyed `${sourceId ?? "ar"}:${num}`
  (`reader-session.svelte.ts:11-17`); `progress: Record<surah, {furthestAyah, ts}>` — surah-only, no
  source/session dimension (`:19-25`).
- Writers: `openVerse`/`markRead` (`reader-session.svelte.ts:88-108`) unconditionally overwrite. Range
  readers (`/page`, `/juz`, `/hizb` → `RangeReader.svelte:105,113`) call only `setPosition` —
  **never `markRead`** → reading by page/juz never updates continue-reading (bug to fix in this
  feature).
- Logout wipes position (`web/src/lib/auth/purge-hook.ts:5-14`). No backend module/tables.
  P03/P04/P08 gaps.

**Build**

1. Model (local-first):
   `ReadingSession {id, name, kind: "daily"|"weekly"|"free"|"shared", translationId?: string|null, createdAt, archivedAt?: number|null, sharedWith?: string[]}`
   — `shared` is a **seam only**; no social feature, no invites, no server sharing.
2. Schema v5: bump `READER_SCHEMA_VERSION`, extend `decodeReader` + `writeBlob` + `applyPersisted`;
   shape: active session id + per-session `{lastRead, lastReadAnchor, recents, progress}`. Old blobs
   migrate into a default `free` session named e.g. "Reading".
3. Writers take/derive `sessionId`: `openVerse`, `markRead`, `recordProgress`, `pushRecent`,
   `clearReadingPosition`; range readers start calling `markRead` with the range's first/last visible
   verse.
4. Surfaces: session switcher (in `/yours` header + settings), `/yours` continue card reads active
   session; palette `continueReading` (`app-actions.ts:39-67`), global hotkey,
   `resumeToVerse/resumeToLastRead` (`web/src/lib/reader/resume.ts`) become session-aware;
   `resumeCtxFor` call sites on index pages.
5. Optional phase 2 (only after local model lands): sync domain `reading_session_v1` — tables
   `reading_sessions` + `reading_session_progress` (LWW `updated_at`, user-scoped, unique
   `(user_id, session_id, surah)`), registered via `registerDomain`; client store modeled on
   `BookmarksStore`. Logout purge must clear/keep per policy — decide explicitly.
6. Daily/weekly semantics: `kind:"daily"` = resets/snapshot per local day (reuse `todayLocal()` from
   `engagement-state.ts:41-46`); `kind:"weekly"` = week bucket; keep buckets derived, don't store
   computed state.

**Acceptance**: switching sessions never clobbers another session's position/recents/progress;
page/juz/hizb reading updates the active session; anon fully functional; gates green.

**Touch**: `reader-core.svelte.ts`, `reader-persistence.svelte.ts`, `reader-session.svelte.ts`,
`annotations.svelte.ts`, `resume.ts`, `RangeReader.svelte`, `/yours`, palette sources,
`purge-hook.ts`; phase 2: `web/src/lib/sync/*`, new Rust module + migrations.

---

## Feature 5 — Customizable homepage widgets (bookmark, continue reading)

**Current state**

- App home = `/yours` (`web/src/routes/(application)/yours/+page.svelte`), fixed sequence: Continue
  card (`:152-192`) → Recents (`:206-236`) → Bookmarks preview (`:238-271`). Data aggregation for
  authed+anon already there (`:51-98`).
- No widget/bento/dashboard system anywhere; no DnD dependency. Reusable primitives: `Card.svelte`,
  `MetricCard.svelte` (currently unused, 4-hue strip = closest "widget" grammar), `Panel.svelte`,
  `Band.svelte`. Empty states partial (Recents/Bookmarks vanish entirely when empty).
- Prefs: `easyquran.prefs` (`prefs.svelte.ts:70-86`); frozen settings doc with `reserved`
  forward-compat slot (`settings-document.ts:29-44`) — no production consumer.
- Marketing `/` stays prerendered static — widgets are an **app-home** feature; don't touch marketing
  (D02 note).

**Build**

1. Widget registry: `web/src/lib/home/widgets.ts` —
   `{id, title, component, defaultSpan, settingsSchema}`. V1 widgets: `continue-reading`, `bookmarks`,
   `recents`, `most-read` (consumes Feature 6 when ready), `daily-ayah` (backend `/quran/random`
   already deterministic date-seeded — D07).
2. Layout model persisted under new key `easyquran.home` (v1):
   `{v, widgets:[{id, visible, span, settings}]}`; default = exactly today's `/yours` order. Unknown
   ids dropped on decode; missing ids appended.
3. Edit mode: show/hide toggles + move up/down (buttons first; drag optional, no new dep without
   approval). Per-widget settings (e.g. bookmarks widget: folder filter; recents: count).
4. Every widget needs explicit empty state (fix the vanishing-sections issue).
5. Keep `/yours` `prerender = false`, client hydration via `onMount` as today; hrefs via `*For(ctx,…)`
   only.

**Acceptance**: default layout = today's `/yours` pixel-order; toggling/reordering persists; widget
decode is forward-compatible; no SSR/prerender regressions; marketing untouched.

**Touch**: `yours/+page.svelte`, new `web/src/lib/home/*`, `prefs.svelte.ts` or new store, i18n
messages (`web/messages/reader/*.json`), possibly `settings-document.ts` if synced later (decide).

---

## Feature 6 — Personal analytics API (most-read surah/ayah)

**Current state**

- Zero per-user analytics. `analytics_v1` is ADMIN-only, post-centric
  (`rust/backend/api/src/modules/analytics_v1/`). No reading-time measurement anywhere.
  `translation_popularity` = API demand only (do not touch). GA4 gets reader paths (audit SCR-05) — no
  consent-gated product analytics.

**Build**

1. New module `rust/backend/api/src/modules/user_analytics_v1/` (copy `device_v1`/`bookmark_v1`
   shape), nested in `router.rs` with `auth_guard::verified` + rate limit (~100/60 like bookmarks).
   Register in `modules/mod.rs`.
2. Migration m000011+ (app SQLite only — Quran DBs untouched):
   `reading_events(id, user_id, surah, ayah, source_id NULL, kind, seconds NULL, created_at)` with
   index `(user_id, created_at)`, and/or rollup
   `reading_daily(user_id, day, surah, ayahs_read, seconds)`. Most-read =
   `GROUP BY surah[,ayah] ORDER BY COUNT(*) DESC` (SQL pattern lives in
   `analytics_v1/controller.rs:582-628`; bucket helper in its validator).
3. Endpoints: `POST /user/analytics/v1/events` (batched, capped, validated verse ranges 1..114 /
   1..286, RFC3339), `GET /user/analytics/v1/summary` (top surahs, top ayahs, totals,
   streak-adjacent day counts). Never expose other users' data.
4. Web emit: from Feature 4 writers (`markRead`/`openVerse`/`recordProgress`), **consent-gated**
   (`web/src/lib/stores/consent.svelte.ts`); offline-friendly via sync engine outbox domain or
   `AuthClient` with CSRF (do not miss `csrf-token` — see the existing FCM registration bug for what
   happens when you do). Update privacy copy (`web/messages/en.json`, settings privacy section) and
   resolve SCR-05 contradiction.
5. Local fallback: compute the same summary client-side from the reader blob for anon users; server
   summary takes over when signed in.

**Acceptance**: summary returns real most-read surah/ayah for a test user; consent off = no events
leave device; admin analytics untouched; no sha/Quran DB writes; rate limits + validation tested.

**Touch**: Rust module + migration + sea-models + tests; web store/emit + consent + copy.

---

## Feature 7 — Sliced ayahs / azkar sets (Ayat al-Kursi, last two of Al-Baqarah)

**Current state**

- No azkar/zikr/dua content anywhere (D06 gap: "curated verse/range references rather than copied
  Quran text"). No range support in URLs (`?v=` regex strictly `/^(\d+):(\d+)$/` in
  `SurahPageRoute.svelte:77`, `Sidebar.svelte:109`, `SurahReader.svelte:574-577`), bookmarks (one
  ayah/row), or share (`reader-share.svelte.ts` single verse, no permalink). Route grammar is closed
  (`web/src/lib/server/reader-route.ts:131-156` `parseReaderPath`).
- API substrate already exists: `GET /quran/sources/{id}/range?from=&to=`
  (`quran_v1/controller.rs:1258-1329`) and `GET /quran/ayahs?keys=2:285,2:286` or
  `fromGlobal/toGlobal` (`:466-546`); web stitches spans via `range-fetch.ts` (300-ayah chunks, cap
  2).

**Build**

1. Curated snippet data `web/src/lib/data/snippets.ts` (or `azkar.json`):
   `{slug, title, kind:"essential"|"azkar"|"dua", refs:[{surah, ayahStart, ayahEnd}], description}`
   — references only, never copied Quran text (immutable-DB rule + D06 wording). Seed:
   `ayat-al-kursi` → 2:255; `baqarah-last-two` → 2:285-286; add a few essentials.
2. Routes: `/azkar` index (Arabic-only prerender allowed, static editorial) + `/azkar/[slug]` detail
   (SSR + disk cache for translated context — same divergence rule as `/t/**`). Add
   `snippetPathFor(ctx, slug)` to the `quran.ts` helper family and extend `nav-guard.test.ts`
   accordingly; do not hand-build URLs.
3. Rendering: reuse the range reading path (`RangePageData`, `RangeReader` internals) with translation
   context; each snippet verse gets normal verse tools (bookmark/copy/share) with its saved/current
   `sourceId`.
4. Range bookmarks: extend `Bookmark` with nullable `endSurah`/`endAyah` (single ayah = nulls). Update
   validator (end ≥ start, span cap — pick ~50 like `?keys` or the 300 fetch cap), unique-index
   handling (see Feature 3 NULL caveat), decoders, `BookmarkRow` range label ("2:285–286"), and
   resume via `surahAyahPathFor` + range highlight.
5. Copy/share ranges: extend `reader-share.svelte.ts` with a range variant (loop `verseRef`, joined
   text + canonical URL). Permalink grammar (R11 open decision): either `?ayah=2:255-2:286` (extend
   the three `?v=` regex sites) or keep fragment-based `#ayah-2-285`..`#ayah-2-286` + snippet route.
   Decide once, then update R11 note.
6. Optional: "save snippet to folder" = bookmark all refs with a shared folder (Feature 2 synergy).

**Acceptance**: Ayat al-Kursi and last-two-of-Baqarah pages render Arabic + any chosen translation;
bookmarking a snippet stores the range; share includes permalink; nav-guard green; caps enforced.

**Touch**: new snippets data + routes + i18n; `quran.ts` helpers + nav-guard; `RangeReader`/
`SurahReader` as needed; `reader-share.svelte.ts`; `schema.ts`/`store.svelte.ts`/`domain.ts`; Rust
migration + `bookmark_v1` validator/slice/tests.

---

## Feature 8 — One author, multiple sources: grouped translations

**Current state**

- Catalogue = 378 baked rows in `web/src/lib/data/translations.json` (positional arrays,
  `TRANSLATION_FIELD_COUNT = 8`, decoder `web/src/lib/data/translations.ts`); provenance only via id
  prefix (`catalogue.ts:25-31`). No `authorId`/`publisher`/`source` field in web or Rust
  (`RawCatalogueEntry`/`CatalogueEntry` in `rust/backend/api/src/quran/loader.rs:176-194`,
  `store.rs:277-286`).
- Real duplicate-author data already present: 44 translator-name collisions across ≥2 sources; 7
  exact `(language,name,translator)` groups — e.g. `en.sahih` ↔ `qul.r20.en-sahih-international`,
  `en.pickthall` ↔ `qul.r19.…`, `fr.hamidullah` ↔ `qul.r31.…`, `quranenc.ja.sato` ↔
  `qul.r218.ja-saeed`, `quranenc.sv.rwwad` ↔ `qul.r1627.sv-rwwad`, `quranenc.en.saheeh` (name
  "Saheeh International", translator "Noor International Center"). Modal does not dedupe
  (`TranslationModal.svelte:160-179`).

**Build**

1. Preferred: **derive** grouping client-side — normalized translator key
   (trim/casefold/whitespace-collapse, `"Unknown"` bucket) + `translationSourceOf(id)`. No catalogue
   schema change, no Rust change, no upload/deploy ripple.
2. Modal: one row per author (within language rail) → expandable source variants with provenance
   badges + size; "show all sources" toggle for exact-dup works; selection resolves to a concrete id
   and records MRU per author (Feature 1.4).
3. Only if the user approves a catalogue change (authorId/publisher fields), the full ripple must be
   done together: `db/quran/translations/scripts/lib.ts` (`writeCatalog` :331-344),
   `db/quran/qul/scripts/catalog.ts` (:91-112), `web/src/lib/data/translations.ts`
   (+`TRANSLATION_FIELD_COUNT`), `scripts/quran/upload.ts`, `deploy/fetch-quran-db.sh`, Rust
   `loader.rs`/`store.rs`, and sha-guard tests must stay green. Default recommendation: **don't** —
   derive.
4. URL grammar stays `/[surah]/t/[lang]/[translator]` (first-dot split, `quran.ts:60-67`); grouping
   is presentation-only.

**Acceptance**: picker shows grouped authors with source variants; selecting a variant opens the
right id; recents rank the author not the variant; no catalogue break; gates green.

---

## 9. Open decisions (ask the user before building these)

1. Bookmark uniqueness: one row per verse (source = last context) vs per `(verse, session)` vs per
   `(verse, source)`. Affects Feature 3 + 7 migrations.
2. Reading sessions: local-first only, or sync phase 2 in the same release?
3. Widget layout: local-only (`easyquran.home`) or synced (wire `settings-document.ts` + backend)?
4. Analytics: consent default (GA currently defaults analytics ON) and granularity sent (surah only vs
   surah+ayah)?
5. Range permalink grammar + span cap (Feature 7.5).
6. Azkar route name/scope: `/azkar` vs `/duas`; editorial governance for the curated list.
7. Folder `kind:"daily"` semantics: name-only label vs real goal metadata (`targetAyahsPerDay`) +
   progress bar.

## 10. Suggested sequencing

1. **Feature 1 + 8** (translation UX, no schema risk) → 2. **Feature 3 + 4** (bookmark context +
   sessions; core data model, both touch the same blob/migrations) → 3. **Feature 2** (folders enrich
   on top) → 4. **Feature 7** (snippets; extends bookmark model with ranges) → 5. **Feature 5**
   (widgets; consumes sessions) → 6. **Feature 6** (analytics; consumes session emit points).

Each phase ends with all three gates green and its own commit(s) on the current branch.
