# EasyQuran — Fine-Grained Feature Prompt (v2)

Grounded in current code. Implementation brief for the next feature batch: translation recents/labels,
bookmark collections with context, reading sessions, homepage widgets, personal analytics, azkar
snippets, and author/source grouping — plus a committed sync design for bookmarks + reading sessions
(§9) and a testable edge-case catalogue (§10).

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
- Sync design is normative in **§9**; every behavior in §9 has a scenario in **§10** (EC-xx).
  FIX-marked scenarios are current-behavior bugs found in the audit and must be fixed as part of this
  batch.

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
   (`bookmark_v1/validator.rs`, mutation cap 200 stays). Folder delete becomes a tombstone
   (`deleted_at`, §9.3) so a stale offline upsert can't resurrect a deleted folder.
3. UI: folder sections sorted by `position` then name; per-folder count (exists); bulk move/delete
   (P05); folder kind picker in `FoldersPanel.svelte`; per-folder progress bar derived from session
   progress (Feature 4) when `kind:"daily"`.
4. Anon: decide either "folders require sign-in" (keep) or a local-only folder mode; don't silently
   degrade.

**Acceptance**: manual order survives sync; `kind` round-trips LWW; tombstone deletion (EC-07); no
change to per-verse uniqueness in this feature (that's Feature 3).

**Touch**: `web/src/lib/bookmarks/schema.ts`, `store.svelte.ts`, `domain.ts`, `FoldersPanel.svelte`,
`BookmarkRow.svelte`, `bookmarks/+page.svelte`, Rust `migration/src/m000011_*.rs`,
`db/sea_models/bookmark_folder/*`, `bookmark_v1/*`.

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
   local notes later), optional `page` + `anchorRatio` (P03 reading bookmark), `sessionId?: string`
   (Feature 4; `''` = global), nullable `endSurah`/`endAyah` (Feature 7 ranges).
2. **Resolved uniqueness** (§9.1): one row per `(user_id, session_id, surah, ayah)`, sentinel
   `session_id = ''` for global/legacy rows — SQLite treats NULLs as distinct in unique indexes, so
   never bare NULL in the key. `sourceId` and range columns are context metadata, not key parts
   (row-level LWW, §9.3).
3. Migration m000011: normalize legacy rows to `session_id = ''` + dedupe + recreate index (pattern:
   `m000008_bookmark_verse_unique.rs`). Update wire slices, `bookmark_v1/validator.rs` ranges, client
   `schema.ts` decoders. Deletes become tombstones (§9.3).
4. Capture at toggle time: read current `sourceId` from route context (`routeContextFromParams` /
   `SurahReader` initial) and active session id; store both.
5. List/resume: build hrefs with `surahAyahPathFor(surahRouteContext(bookmark.sourceId ?? "uthmani"), …)`
   so bookmarks reopen in their saved translation; show source badge on `BookmarkRow`.
6. Share seam: keep `reader-share.svelte.ts` untouched here; range/copy work is Feature 7.

**Acceptance**: bookmark created on a `/t/…` route reopens in that translation; session-scoped
bookmarks filter by active session; deletions are tombstones so stale queued upserts cannot resurrect
(EC-02); anon path unchanged.

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
  **never `markRead`** → reading by page/juz never updates continue-reading (FIX, EC-42).
- Logout wipes position (`web/src/lib/auth/purge-hook.ts:5-14`). No backend module/tables.
  P03/P04/P08 gaps.

**Build**

1. Model (local-first, then synced):
   `ReadingSession {id, name, kind: "daily"|"weekly"|"free"|"shared", translationId?: string|null, createdAt, archivedAt?: number|null, sharedWith?: string[]}`
   — `shared` is a **seam only**; no social feature, no invites, no server sharing.
2. Schema v5: bump `READER_SCHEMA_VERSION`, extend `decodeReader` + `writeBlob` + `applyPersisted`;
   shape: active session id + per-session `{lastRead, lastReadAnchor, recents, progress}`. Old blobs
   migrate into a default `free` session named "Reading" with a **deterministic id** derived from the
   user id (UUIDv5-style, fixed namespace; EC-22) so two devices of the same account migrate to the
   same session instead of duplicating it.
3. Writers take/derive `sessionId`: `openVerse`, `markRead`, `recordProgress`, `pushRecent`,
   `clearReadingPosition`; range readers start calling `markRead` with the range's first/last visible
   verse.
4. Surfaces: session switcher (in `/yours` header + settings), `/yours` continue card reads active
   session; palette `continueReading` (`app-actions.ts:39-67`), global hotkey,
   `resumeToVerse/resumeToLastRead` (`web/src/lib/reader/resume.ts`) become session-aware;
   `resumeCtxFor` call sites on index pages.
5. **Sync (committed, not optional)**: new `reading-session` domain → `reading_session_v1` module +
   tables (§9.1–9.2). Sessions, positions, and per-surah progress all sync; **recents stay
   device-local** (navigation convenience, no sync value). Client store modeled on `BookmarksStore`;
   register via `registerDomain`. Logout/expiry follows the quarantine policy (§9.5).
6. Daily/weekly semantics: `kind:"daily"` = day buckets derived per **device-local day** (reuse
   `todayLocal()` from `engagement-state.ts:41-46`); `kind:"weekly"` = week bucket; buckets stay
   derived, never stored computed state (EC-36).

**Acceptance**: switching sessions never clobbers another session's position/recents/progress;
page/juz/hizb reading updates the active session (EC-42); sync round-trips sessions/positions/progress;
progress never regresses across devices (EC-05); anon fully functional; gates green.

**Touch**: `reader-core.svelte.ts`, `reader-persistence.svelte.ts`, `reader-session.svelte.ts`,
`annotations.svelte.ts`, `resume.ts`, `RangeReader.svelte`, `/yours`, palette sources,
`purge-hook.ts`; new `web/src/lib/sessions/{schema,store,domain}.svelte.ts` + Rust
`reading_session_v1` module + migrations.

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
2. Migration (app SQLite only — Quran DBs untouched):
   `reading_events(id, user_id, surah, ayah, source_id NULL, kind, seconds NULL, created_at)` with
   index `(user_id, created_at)`, and/or rollup
   `reading_daily(user_id, day, surah, ayahs_read, seconds)`. Most-read =
   `GROUP BY surah[,ayah] ORDER BY COUNT(*) DESC` (SQL pattern lives in
   `analytics_v1/controller.rs:582-628`; bucket helper in its validator).
3. Endpoints: `POST /user/analytics/v1/events` (batched, capped, validated verse ranges 1..114 /
   1..286, RFC3339), `GET /user/analytics/v1/summary` (top surahs, top ayahs, totals,
   streak-adjacent day counts). Never expose other users' data.
4. Web emit: from Feature 4 writers (`markRead`/`openVerse`/`recordProgress`), **consent-gated**
   (`web/src/lib/stores/consent.svelte.ts`); offline-friendly via the sync engine outbox (domain
   `analytics`) or `AuthClient` with CSRF (do not miss `csrf-token` — see the existing FCM
   registration bug for what happens when you do). Queued events follow §9.5 quarantine on logout;
   consent revoke drops queued events. Update privacy copy (`web/messages/en.json`, settings privacy
   section) and resolve SCR-05 contradiction.
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
4. Range bookmarks: extend `Bookmark` with nullable `endSurah`/`endAyah` (single ayah = nulls).
   Update validator (end ≥ start, span cap — pick ~50 like `?keys` or the 300 fetch cap; EC-39),
   uniqueness per §9.1 (range columns not in the key), decoders, `BookmarkRow` range label
   ("2:285–286"), and resume via `surahAyahPathFor` + range highlight.
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

## 9. Sync architecture — bookmarks + reading sessions

**Decision**: both sync. Bookmarks (extended: session/translation/range context, tombstones) and
reading sessions (sessions, positions, per-surah progress). The existing engine/outbox/registry are
reused as-is; one backend module per domain; migrations `m000011`+.

### 9.1 Data model (backend, app SQLite only — Quran DBs untouched)

**`bookmarks` (extend m000007):**

- add `session_id TEXT NOT NULL DEFAULT ''`, `source_id TEXT NULL`, `end_surah INTEGER NULL`,
  `end_ayah INTEGER NULL`, `note TEXT NULL`, `deleted_at TEXT NULL`.
- replace unique index: `(user_id, session_id, surah, ayah)`. Sentinel `''` for global/legacy rows —
  never bare NULL (SQLite NULL-distinct in unique indexes).
- `list_for_user` filters `deleted_at IS NULL`.
- Delete = tombstone `UPDATE` (`deleted_at` + `updated_at` = now), never row removal.

**`bookmark_folders`:** add `kind TEXT NOT NULL DEFAULT 'generic'`, `position INTEGER NULL`,
`deleted_at TEXT NULL`; delete = tombstone; `list_for_user` filters.

**`reading_sessions`** (new): `id TEXT PK, user_id, name, kind TEXT, translation_id TEXT NULL,
created_at, updated_at, deleted_at NULL`.

**`reading_session_positions`** (new): `session_id TEXT PK, user_id, verse_key TEXT, surah INTEGER,
ayah INTEGER, local_page INTEGER NULL, ratio REAL NULL, source_id TEXT NULL, updated_at`.

**`reading_session_progress`** (new): `user_id, session_id, surah, furthest_ayah, updated_at,
PRIMARY KEY(session_id, surah)`.

All ids are client-minted UUIDs (existing `newBookmarkEntityId` pattern; validator: 36-char dashed
hex). Every query user-scoped.

### 9.2 Modules + client domains

- `POST /bookmark/v1/sync` — extended mutation fields; same envelope + verified guard + 100/60
  limiter; batch ≤ 200; same LWW helpers (`db/sea_models/bookmark/actions.rs` pattern).
- New `reading_session_v1`: `POST /reading-session/v1/sync` — mutations `session.upsert`,
  `session.delete`, `position.upsert`, `progress.upsert`; response
  `{applied, skipped, sessions, positions, progress}` (full snapshot per user, same as bookmarks);
  `auth_guard::verified`; 100/60 limiter; batch ≤ 200.
- Client: `web/src/lib/sessions/{schema,store,domain}.svelte.ts` modeled on
  `web/src/lib/bookmarks/*`; register via `registerDomain`. `SyncPausedError` before any request when
  signed out (contract: `sync/types.ts:58-63`).

### 9.3 Conflict policy (multi-device) — **no realtime handling**

- Per-entity **LWW on `updatedAt`** (server `julianday()` compare, existing pattern). Same id = same
  entity (ids are identity; a row may move verses/sessions, existing behavior).
- Special merges (NOT plain LWW):
  - `progress.furthestAyah = MAX(stored, incoming)` — progress never regresses; `updated_at` LWW.
  - `position` — pure LWW (latest position wins; no max on ratio/page).
  - Session rename vs archive race: row-level LWW means the newer row wins entirely and the loser's
    other field change is lost. **Accepted loss for v1** — no field-level merge (EC-06).
- Session identity: UUID minted once client-side. Syncing the same session across devices is the
  intended model (same id = same logical session). Two devices editing the same session → per-row LWW,
  converge on the next sync round. Two devices creating separate sessions → different UUIDs, no
  conflict (EC-20).
- **Explicit non-goals**: no websockets/presence, no CRDT/OT, no live collaboration. Convergence
  happens on the next flush trigger: online poll 5s, periodic 30s, visibilitychange, online event,
  debounce 2s (`engine.svelte.ts:22-28,173-220`).
- **Clock skew**: server validator normalizes incoming `updatedAt = min(updatedAt, server_now + 60s)`
  so a device with a fast clock cannot dominate LWW forever (EC-33).
- **Ties**: snapshot queries gain secondary ordering `ORDER BY updated_at ASC, id ASC`; the client
  collapse uses the same deterministic rule (today tie order is undefined — FIX, EC-35).
- **Tombstones**: delete sets `deleted_at` (LWW compared on `updated_at`). A newer upsert clears the
  tombstone (intentional resurrection); an older upsert is skipped — this fixes the current
  stale-queued-upsert resurrection bug (no tombstones today). Tombstones older than 90 days are purged
  by the existing maintenance task (rides the rate-limit task like `translation_popularity` flush,
  `main.rs:586-652`).

### 9.4 Merge on login — non-silent, reviewable

Replaces today's silent `#migrateLegacy` (`bookmarks/store.svelte.ts:447-478`).

- **Detection** after auth status → authenticated: mergeable local data exists if (a) legacy blob
  bookmarks non-empty AND marker absent, (b) anon/local session or bookmark store non-empty, or
  (c) quarantined pending mutations exist for this user.
- **UI**: `MergeReviewDialog.svelte` (bits-ui Dialog, `AuthModal.svelte` pattern — no AlertDialog
  primitive exists). Step 1 summary: "You have N bookmarks, M folders, K reading sessions on this
  device. Merge into your account?" Actions: **Merge** / **Review changes** / **Not now**.
- Step 2 review: tabs Bookmarks / Folders / Sessions; rows with include checkboxes; conflict labels
  ("Account has a newer version — account wins" / "Device version is newer — device wins"); counts of
  new vs conflicting vs duplicate.
- Merge executes: enqueue local items as upserts **with their existing timestamps** (LWW decides);
  flush; result toast "Merged X, skipped Y (newer account versions kept)".
- **Not now**: nothing enqueued; marker `eq.merge.deferred.{userId}`; re-prompt on next app start (not
  every reload). **Don't ask again** sets `eq.merge.reviewed.{userId}`. Markers are per-account.
- Failure handling: markers are written only after enqueues resolve — fixes the current bug where
  `#migratedKeys` marks keys before the enqueue and a partial failure permanently loses those verses
  (EC-11).
- Legacy blob is never deleted (current behavior kept); the legacy boolean path migrates only through
  this review.

### 9.5 Logout, expiry, cache policy — **never destroy unsynced user data silently**

- **Explicit logout**: account view cleared from memory (privacy — the next user on the browser sees
  nothing); pending unsynced mutations move to a per-user quarantine key
  `eq.sync.quarantine.{userId}` (never destroyed — fixes today's `engine.clear("bookmarks")` data
  loss at `store.svelte.ts:415`); device-local reader data (position, "on this device" session, notes)
  kept so offline reading continues; synced sessions hidden until re-login.
- **Session expiry / remote terminate / ban** (401/403 or probe → anonymous): same as logout for sync
  purposes + banner "Session expired — sign in to resume sync" (new copy). Wire the
  `current-session-terminated` transition — it is a dead kind today (no producer in `web/src`;
  `purge-hook.ts:7` branch unreachable). Detect via auth-client on 401/403 (`AUTH_004`/`AUTH_009`)
  and probe; also clear the stale in-memory CSRF token and retry once (FIX: stale-CSRF wedge, EC-15).
- **Re-login same user**: quarantine auto-resumes (visible via `SyncIndicator` + toast). No merge
  dialog for your own pending mutations.
- **Re-login different user**: quarantine untouched and hidden; resumes when its owner logs in
  (EC-12/EC-17).
- **Cached server snapshots** are memory-only — nothing account-scoped persists on disk, so logout
  needs no disk purge for them.
- **Reading position on logout**: change the current purge behavior (`clearReadingPosition` today wipes
  position/recents/progress on explicit logout but NOT on expiry — inconsistent). New policy: keep
  device-local reader data on both; only the account view is hidden (EC-14/EC-16).
- **Notes**: local-only notes currently survive logout and leak to the next account (privacy bug).
  Scope notes per account or clear them from the view on logout (FIX, EC-17).

### 9.6 Multi-tab

- **Auth cross-tab**: add a `storage` listener on `eq.auth.session-hint` → other tabs re-probe and
  fire transitions (none today — FIX, EC-30).
- **Overlay retire cross-tab**: BroadcastChannel `easyquran-sync`, messages
  `{domain, drainedIds, userId}` → other tabs retire optimistic overlays (fixes the DEFERRED note at
  `store.svelte.ts:346-347`). Payloads are ids only, never user content.
- **Shared outbox**: two tabs may take the same batch and push concurrently → server LWW is idempotent
  (safe, EC-32); cross-tab retire removes the double-overlay flicker.
- Engines keep independent retry state per tab.

### 9.7 Audit fixes this design must land (each has an EC scenario)

- Per-user gating: `applyServer` and enqueue callbacks gate on a boolean `authed`, not user id — a
  round from user A can land in user B's view on fast account switches (FIX, EC-17).
- Outbox destroyed on logout/expiry (FIX, EC-14/EC-15); quarantine replaces it.
- No tombstones (FIX, EC-02); tombstone model above.
- Pending-delete matching is id-scoped, not verse-scoped — a same-verse rival from a stale snapshot
  re-marks the verse (FIX, EC-40).
- Enqueue rejection is silent (overlay retired, nothing applied, no error) (FIX, EC-27).
- Pull-only round failure with an empty queue gets no retry (FIX, EC-28).
- Corrupt outbox rows are immortal (skipped by decode, never erased) (FIX, EC-26).
- No IDB→memory fallback when `indexedDB.open` fails at runtime (FIX, EC-25).
- Engine listeners never torn down after logout; timers keep running (FIX, EC-30).
- Poison mutation can head-of-line-block a domain forever (no failure cap) → dead-letter after N
  failures with a surfaced error (FIX, EC-29).
- `SyncPausedError` can clear a real `lastError`; paused + no failures → phase "idle" (FIX, EC-43).
- Coalesced flush with a different domain filter drops the second filter (latent today; keep the
  guard when a second domain lands) (FIX, EC-44).

---

## 10. Edge-case catalogue — testable scenarios

Format: **EC-nn · title** — Given / When / Then + resolution. FIX = current-behavior bug this batch
fixes. Test targets: Rust integration (`rust/backend/api/tests/*.rs`), web unit (`web/src/lib/**/__tests__/*.test.ts`), manual QA.

### A. Multi-device conflicts

**EC-01 · Same bookmark edited on two devices**
Given devices A+B edit bookmark X (folder/source) offline. When both sync. Then newer `updatedAt`
wins the whole row; older push is skipped; both devices converge on the next pull.
Test: extend `bookmarks_v1.rs` + `store.test.ts`.

**EC-02 · Delete vs stale offline upsert (FIX: tombstone)**
Given A deletes bookmark X (tombstone), B offline later replays a stale upsert for X. When B syncs.
Then upsert's `updatedAt` < `deleted_at` → skipped; X stays deleted; no resurrection.
Test: `bookmarks_v1.rs`.

**EC-03 · Same verse in two sessions on two devices**
Given A bookmarks 2:255 in session "Daily", B bookmarks 2:255 in session "Study". When both sync.
Then two rows (uniqueness is per session) — no conflict, both visible per session.
Test: `bookmarks_v1.rs`.

**EC-04 · Same verse, same session, different folders**
Given A files 2:255 in folder F1, B files it in F2 (same session), both offline. When both sync. Then
verse-collapse keeps the newer row; loser's folder context is dropped (accepted lossy collapse, no
field-level merge). Test: `bookmarks_v1.rs`.

**EC-05 · Progress from two devices**
Given A read to 2:100, B (offline, stale) reports 2:50 for the same session/surah. When both sync.
Then `furthestAyah = MAX(100, 50) = 100` — never regresses; position row follows LWW.
Test: `reading_session_v1` tests + `sessions/store.test.ts`.

**EC-06 · Rename vs archive race (accepted loss)**
Given A renames session S (newer ts), B archives S (older ts). When both sync. Then newer row wins;
the older field change is lost. Documented accepted loss — no field-level merge in v1.

**EC-07 · Folder delete vs child move**
Given A deletes folder F (tombstone), B offline moves bookmark X into F. When both sync. Then F stays
deleted; X's folder upsert resolves to root (`resolve_folder_id` pattern) — no orphan, no FK trip.
Test: `bookmarks_v1.rs` (extends existing same-batch case).

### B. Merge on login

**EC-08 · Offline → login shows merge review**
Given anonymous local bookmarks/folders/sessions exist. When the user logs in. Then
`MergeReviewDialog` appears with accurate counts; nothing is enqueued before an explicit choice;
Merge → items sync; result toast shows applied/skipped. Test: `store.test.ts` + manual QA.

**EC-09 · Not now / Don't ask again**
Given the dialog is open. When "Not now" → nothing enqueued, marker `eq.merge.deferred.{userId}`, no
re-prompt until next app start. When "Don't ask again" → `eq.merge.reviewed.{userId}`, never
re-prompted. Local data stays intact either way. Test: `store.test.ts`.

**EC-10 · Legacy blob + newer account rows**
Given legacy blob bookmarks exist and the account already has a newer version of the same verse. When
merge review runs. Then the review labels the conflict ("account wins"); merge skips the older local
item (LWW); legacy blob remains untouched. Test: `store.test.ts`.

**EC-11 · Merge network failure (FIX: partial marker)**
Given merge enqueues fail midway (network). When the user retries later. Then no marker was written,
no item was silently dropped (fix: mark only after enqueues resolve), retry presents the same items.
Test: `store.test.ts`.

**EC-12 · Login as a different account with another user's quarantine**
Given user A logged out with quarantined mutations; user B logs in. When sync runs. Then A's
quarantine is not flushed, not shown, not merged into B; it resumes when A logs back in.
Test: `store.test.ts`.

**EC-13 · Review shows both conflict directions**
Given device item newer than account item (and vice versa). When the review step renders. Then labels
show "device wins" / "account wins" per row, matching server LWW outcome after merge. Test:
component test / manual QA.

### C. Logout, expiry, cache

**EC-14 · Explicit logout keeps unsynced data (FIX: engine.clear loss)**
Given pending unsynced mutations. When the user logs out. Then account view is cleared, mutations are
quarantined under `eq.sync.quarantine.{userId}`, device-local reader data remains, re-login resumes
the queue. Nothing is destroyed. Test: `store.test.ts` + `engine.test.ts`.

**EC-15 · Session expiry mid-use (FIX: dead event + stale CSRF)**
Given an expired session (401 `AUTH_009` / 403 `AUTH_004`) while mutations are queued. When the next
request fails. Then the tab transitions to signed-out with a "Session expired — sign in to resume
sync" banner, CSRF token is cleared + one retry, quarantine keeps the queue, no data loss. Test:
`auth-state.test.ts` + `store.test.ts`.

**EC-16 · Remote session terminate on device B**
Given device A terminates device B's session from `/account`. When B next probes or calls any guarded
route. Then B detects signed-out, shows the expiry banner, quarantines pending work.
Test: `auth-state.test.ts` + rust terminate tests (existing) + manual QA.

**EC-17 · Privacy: next user sees nothing of the previous account (FIX: cross-account leaks)**
Given A logs out (or expires) and B uses the same browser. When B opens reader/bookmarks/sessions.
Then B sees none of A's synced bookmarks/folders/sessions/notes; per-user gating by user id (not
boolean `authed`) prevents A's in-flight rounds landing in B's view; notes are account-scoped.
Test: `store.test.ts`.

**EC-18 · Re-login same user auto-resumes quarantine**
Given quarantined mutations for user A. When A logs back in. Then the queue flushes automatically with
SyncIndicator + toast; no merge dialog for own pending mutations. Test: `store.test.ts`.

**EC-19 · Banned / revoked account**
Given a banned user (`403` AccountLocked) or a revoked session. When the client probes or calls.
Then signed-out state + specific copy (not the current "Email or password is incorrect." mapping bug
for login 403 AUTH_006); quarantine kept. Test: `auth-state.test.ts` + rust ban tests.

### D. Identity

**EC-20 · Two devices create separate sessions**
Given A creates session "Daily" offline, B creates "Weekly" offline. When both sync. Then two distinct
UUIDs → two sessions, no collision, no merge dialog. Test: `reading_session_v1` tests.

**EC-21 · Same session id on two devices**
Given both devices hold session S (synced earlier) and both edit it offline. When both sync. Then
per-row LWW converges; no duplication, no crash; snapshot shows one S. Test: `reading_session_v1`
tests + `sessions/store.test.ts`.

**EC-22 · Deterministic legacy session id**
Given the same legacy reader blob migrated on two devices of one account. When both migrate. Then both
derive the same UUID (UUIDv5-style, fixed namespace + user id) → one session, not two. Test:
`sessions/schema.test.ts`.

### E. Outbox / queue robustness

**EC-23 · Rapid toggle bookmark on/off/on**
Given three toggles within the debounce window. When the queue drains. Then FIFO preserves final state
"on"; server verse-collapse leaves exactly one row; no duplicates. Test: `store.test.ts`.

**EC-24 · 250 queued mutations**
Given 250 mutations queued. When flush runs. Then batches of ≤100 drain in order; request payloads
stay ≤200; everything lands. Test: `engine.test.ts`.

**EC-25 · IDB unavailable at runtime (FIX)**
Given `indexedDB.open` rejects (private mode / blocked storage). When a mutation is enqueued. Then
memory fallback engages + a visible "changes may not survive reload" warning (today: enqueue rejects
permanently, no fallback). Test: `outbox.test.ts`.

**EC-26 · Corrupt outbox row (FIX)**
Given a row that fails `decodeMutation`. When the queue is read. Then the row is skipped AND purged
(today: immortal). Test: `outbox.test.ts`.

**EC-27 · Enqueue rejection (FIX: silent failure)**
Given the outbox write rejects. When the user toggles a bookmark. Then the failure is surfaced
(toast/indicator), the optimistic overlay is retired, and no data is silently lost. Test:
`store.test.ts`.

**EC-28 · Pull-only failure with empty queue (FIX: no retry)**
Given queue empty, pull returns 500. When the round fails. Then a retry is scheduled (today: no retry
when `#pending <= 0`). Test: `engine.test.ts`.

**EC-29 · Poison mutation (FIX: dead-letter)**
Given a mutation the server always rejects (malformed beyond validation — e.g. foreign id handling
regression). When N attempts fail. Then it is moved to a dead-letter quarantine with a surfaced
error; the rest of the queue keeps draining (no head-of-line block). Test: `engine.test.ts`.

**EC-30 · Engine teardown on logout (FIX)**
Given logout. When timers keep firing. Then listeners/timers are torn down or the engine is parked so
`phase` stops cycling; no network calls while signed out. Test: `engine.test.ts`.

### F. Cross-tab

**EC-31 · Logout propagates across tabs (FIX: no auth cross-tab)**
Given two tabs open, A logs out. When B receives the storage event on `eq.auth.session-hint`. Then B
transitions anonymous, clears the account view, quarantines its pending work. Test:
`auth-state.test.ts` + manual QA.

**EC-32 · Simultaneous flush, same batch**
Given two tabs flush the same batch concurrently. When the server receives both. Then LWW is
idempotent — second round applies/skips without duplicates; cross-tab overlay retire (BroadcastChannel)
removes stale overlays. Test: `bookmarks_v1.rs` replay test + manual QA.

**EC-33 · Cross-tab overlay retire**
Given tab A drains a mutation while tab B holds the optimistic overlay. When the retire message
arrives. Then B removes its overlay and renders the snapshot row without flicker. Test:
`store.test.ts` + manual QA.

### G. Clocks, ties, time zones

**EC-34 · Device clock ahead (FIX: clamp)**
Given device A's clock is +1 day. When it syncs an edit. Then the server clamps `updatedAt` to
`now + 60s` max; A cannot dominate LWW forever. Test: `bookmarks_v1.rs` validator test.

**EC-35 · Tie timestamps (FIX: undefined order)**
Given two rows with equal `updated_at`. When the snapshot is built. Then deterministic order
`(updated_at, id)`; client collapse uses the same rule; repeated syncs converge identically. Test:
`bookmarks_v1.rs` + `store.test.ts`.

**EC-36 · Daily session across midnight / timezone change**
Given a `kind:"daily"` session and a device timezone change across midnight. When the day bucket is
derived. Then buckets follow device-local day (documented); server never computes day boundaries; no
data is attributed to the wrong bucket retroactively. Test: `sessions/store.test.ts`.

### H. Ranges, decode, snapshots

**EC-37 · Bookmark round-trip with all new fields**
Given a bookmark with session, source, range, note. When it round-trips client→server→snapshot→decode.
Then all fields survive; malformed rows are dropped individually (existing decoder behavior). Test:
`schema.test.ts` + `bookmarks_v1.rs`.

**EC-38 · Saved translation no longer in catalogue**
Given `bookmark.sourceId` references an id absent from `TRANSLATIONS`. When the bookmark is opened.
Then the href falls back to the Arabic context (`surahRouteContext("uthmani")`), no crash, no broken
link. Test: `bookmarks/+page` component test / manual QA.

**EC-39 · Range span cap**
Given a range bookmark with span > cap. When validated. Then the round is rejected with a clear
error (server-side, all-mutations-validated-first behavior preserved). Test:
`bookmarks_v1.rs`.

**EC-40 · Pending delete vs stale same-verse rival (FIX: id-scoped delete)**
Given an in-flight delete for verse V (session S) and a stale snapshot containing a rival row for V.
When the snapshot applies. Then the pending delete is matched by `(session, verse)`, not id alone;
the verse does not re-mark. Test: `store.test.ts`.

**EC-41 · Unbounded snapshot growth**
Given a user with thousands of bookmarks. When every sync returns the full snapshot. Then behavior is
documented as accepted for now (pagination seam = the two `list_for_user` reads, `controller.rs:103-108`);
no UI jank at expected scale; a pagination plan is noted for later. Test: manual QA.

**EC-42 · Range readers update the active session (FIX)**
Given reading on `/juz/1` or `/page/5`. When the user scrolls. Then `markRead` fires for the
session with the visible verse (today: only `setPosition`, continue-reading never updates). Test:
`reader.test.ts` + manual QA.

**EC-43 · Paused round must not clear lastError (FIX)**
Given a previous real sync error. When the next round pauses (signed out). Then `lastError` is
preserved and the UI keeps showing the actionable error. Test: `engine.test.ts`.

**EC-44 · Coalesced flush with domain filter (FIX)**
Given `flush("a")` in flight and `flush("b")` called. When the drain loops. Then both domains drain
(today: b is never drained by that call). Latent until a second domain exists — fix before adding the
sessions/analytics domains. Test: `engine.test.ts`.

---

## 11. Open decisions (ask the user before building these)

1. ~~Bookmark uniqueness~~ **Resolved (§9.1)**: unique `(user_id, session_id, surah, ayah)`, sentinel
   `''` for global rows.
2. ~~Reading sessions sync~~ **Resolved (§9.2)**: sessions + positions + progress sync; recents stay
   device-local.
3. Widget layout: local-only (`easyquran.home`) or synced (wire `settings-document.ts` + backend)?
4. Analytics: consent default (GA currently defaults analytics ON) and granularity sent (surah only vs
   surah+ayah)?
5. Range permalink grammar + span cap (Feature 7.5 / EC-39).
6. Azkar route name/scope: `/azkar` vs `/duas`; editorial governance for the curated list.
7. Folder `kind:"daily"` semantics: name-only label vs real goal metadata (`targetAyahsPerDay`) +
   progress bar.
8. Accepted losses to confirm: session row-level LWW (EC-06) and verse-collapse context loss (EC-04) —
   field-level merge is out of scope; confirm acceptable.

## 12. Suggested sequencing

1. **Feature 1 + 8** — translation UX, no schema risk.
2. **Feature 3 + 4 + §9.1–9.3** — bookmark context + sessions: migrations (`m000011`+), tombstones,
   new `reading_session_v1` module, client domains, LWW/merge rules. (Keep the existing silent
   bookmarks migration until step 3 lands.)
3. **§9.4–9.7 + §10 FIX items** — merge review (non-silent), quarantine, cross-tab auth + overlay
   retire, and every FIX scenario (EC-02, 11, 14–17, 25–30, 40, 42–44).
4. **Feature 2** — folders enrich on top (kind/position/bulk).
5. **Feature 7** — snippets; extends the bookmark model with ranges.
6. **Feature 5** — widgets; consumes sessions.
7. **Feature 6** — analytics; consumes session emit points.

Each phase ends with all three gates green and its own commit(s) on the current branch. Every phase
lands its EC scenarios as tests (Rust integration + web unit) in the same commit.
