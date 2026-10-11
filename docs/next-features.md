# EasyQuran — Fine-Grained Feature Prompt (v2)

Grounded in current code. Implementation brief for the next feature batch: translation recents/labels,
bookmark collections with context, reading sessions, homepage widgets, personal analytics, azkar
snippets, author/source grouping, search recents/ranking, and locale suggest — plus a committed sync
design for bookmarks + reading sessions (§9) and a testable edge-case catalogue (§10).

**Authority**: this document is the latest direction. Where an older doc conflicts, this document
wins — see §14 (Superseded directions). Pending work absorbed from `ux-audit`,
`plan/09-locale-suggest.md`, `settings-system.md`, `search-system.md`, `copy-corrections.md`, the
security audits, and research docs is mapped in §13.

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
  R10, R11, D01, D06, D08.
- **Never repurpose `translation_popularity`**
  (`rust/backend/api/src/services/translation_popularity_store.rs`) as reader popularity — docs
  explicitly say it measures API demand (`docs/quran-system.md:330,346`).
- Sync design is normative in **§9**; every behavior in §9 has a scenario in **§10** (EC-xx).
  FIX-marked scenarios are current-behavior bugs found in the audit and must be fixed as part of this
  batch.
- **Security coordination**: the R2-08 account-isolation work (outbox partitioned by account,
  `expectedUserId`, generation guards) is in flight in the working tree. §9.5 adopts it as the basis
  with one divergence: legacy unowned entries are **quarantined, never discarded** (EC-48). Land R2-08
  first; this design rebases on it, never around it.

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
3. Labels (TRX-13/15/16): visible secondary line = `translator — name · Provenance`; provenance badge
   (Tanzil / QuranEnc / QUL). **Display name always from the catalogue, never reconstructed from URL
   segments** (TRX-15). Credit on its own line with `<bdi lang="en">` (TRX-13). Short language tags
   per line (EN · UR · FR · ID · BN), full credits once per page (TRX-14). Hover card = full name +
   translator + one-line description ("Modern, easy English") + "Downloaded" / "Needs internet"
   instead of byte size (TRX-16). Handle the attribution-mismatch case (`quranenc.en.saheeh`: name
   "Saheeh International", translator "Noor International Center") without merging them.
4. Persist per-author last-used variant (pairs with Feature 8): `Map<normalizedTranslator, id>`.
5. Picker search (TRX-04): accent/case-insensitive matching + aliases (Sahih/Saheeh,
   Pickthall/Pickthal) + bounded edit distance ≤ 1.
6. Chip strip (TRX-05/06): wrap chips or collapse to "Main: Saheeh International · +5 more ▾"; the
   reading chip has no ×; wording uses "reading translation"/"main", **never "primary"** (the primary
   concept was removed end to end in `502effc2`).
7. Transliteration rendering (TRX-02): view-layer parse `<u>`/`<b>`; legend "underlined = stretch the
   vowel"; strip markup in copy.
8. Empty translation verses (TRX-11 + `docs/research/translation-empty-verses.md`): show Arabic plus
   "No translation for this verse in {translator}."; set `dir` from the declared direction, not
   `auto`. Four known upstream blanks: `fa.safavi` 80:39, `ku.asan` 108:3, `sq.mehdiu` 21:56 +
   77:14. DBs stay immutable — never "fix" the rows. EC-45.

**Acceptance**: recents survive reload; ranking only changes on explicit selection; no new catalogue
fields required (derivation only); modal labels show author + source for every row; picker search
finds "Saheeh" from "sahih"; empty verses render the placeholder + declared `dir`; chip strip never
overflows without a summary; gates green.

**Touch**: `TranslationModal.svelte`, new store, `reading-text.svelte.ts` (keep its 8-cap chips,
delegate ordering to MRU), `reading-flow.ts`, `VerseRow.svelte`/`ReadingAyah.svelte` (transliteration +
empty verse), copy strings.

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
4. Folder UI polish (ACCT-04): full-width "New folder" field + pill **Add** button; per-row selects
   replaced with a "Move to…" menu (folder icon + chevron); one row component shared between
   signed-in and anonymous bookmarks.
5. Bookmark rows (BM-01/BM-02): single row component on Yours / Bookmarks / Home; one verse format
   everywhere — **"Al-Baqarah · verse 1"**; row shows the first ~80 characters of the verse (Arabic,
   plus translation if the reader uses one), the saved date ("Saved 3 days ago"), and a note icon when
   a note exists.
6. Remove flow (BM-04): optimistic remove + toast "Bookmark removed · Undo" for 5 seconds; applies to
   signed-in and offline removals alike.
7. List controls (FLOW-08): sort by date / folder / surah; filter by folder; group headers.
8. Sync status line (ACCT-05): `text-caption` (13.5 px) instead of 12 px grey; offline banner above
   the list.
9. Anon: decide either "folders require sign-in" (keep) or a local-only folder mode; don't silently
   degrade.

**Acceptance**: manual order survives sync; `kind` round-trips LWW; tombstone deletion (EC-07); one
row component reused across surfaces; undo restores the row; anon sort-by-date works once Feature 4's
v5 gives legacy anon bookmarks timestamps (EC-46).

**Touch**: `web/src/lib/bookmarks/schema.ts`, `store.svelte.ts`, `domain.ts`, `FoldersPanel.svelte`,
`BookmarkRow.svelte`, `bookmarks/+page.svelte`, Rust `migration/src/m000011_*.rs`,
`db/sea_models/bookmark_folder/*`, `bookmark_v1/*`, toast/undo primitive.

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
   `SurahReader` initial) and active session id; store both. Capture surface = verse mode (VerseTools);
   reading mode does not instantiate verse actions (accepted, EC-47).
5. List/resume: build hrefs with `surahAyahPathFor(surahRouteContext(bookmark.sourceId ?? "uthmani"), …)`
   so bookmarks reopen in their saved translation; show source badge on `BookmarkRow`.
6. Count + sync label (ACCT-03): home/bookmarks card shows the real count and "3 saved · synced" once
   sync is live.
7. Share seam: keep `reader-share.svelte.ts` untouched here; range/copy work is Feature 7.

**Acceptance**: bookmark created on a `/t/…` route reopens in that translation; session-scoped
bookmarks filter by active session; deletions are tombstones so stale queued upserts cannot resurrect
(EC-02); anon path unchanged; saved translation missing from catalogue falls back to Arabic (EC-38).

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
3. Schema v5 also absorbs three pending reader-state items:
   - persist `translationFamily` (TRX-10; today session-only per `settings-system.md` §4);
   - give anonymous bookmarks a `savedAt` timestamp so sort-by-date works (BM-02 second pass);
   - scope notes per account (privacy fix, EC-17).
4. Writers take/derive `sessionId`: `openVerse`, `markRead`, `recordProgress`, `pushRecent`,
   `clearReadingPosition`; range readers start calling `markRead` with the range's first/last visible
   verse.
5. Surfaces: session switcher (in `/yours` header + settings), `/yours` continue card reads active
   session; palette `continueReading` (`app-actions.ts:39-67`), global hotkey,
   `resumeToVerse/resumeToLastRead` (`web/src/lib/reader/resume.ts`) become session-aware;
   `resumeCtxFor` call sites on index pages.
6. Position preservation on context switch (FLOW-07): switching translation/language from the reader
   keeps the current position (`translation-nav.ts` `hrefFor` path), never resets to surah top.
7. **Sync (committed, not optional)**: new `reading-session` domain → `reading_session_v1` module +
   tables (§9.1–9.2). Sessions, positions, and per-surah progress all sync; **recents stay
   device-local** (navigation convenience, no sync value). Client store modeled on `BookmarksStore`;
   register via `registerDomain`. Logout/expiry follows the quarantine policy (§9.5).
8. Daily/weekly semantics: `kind:"daily"` = day buckets derived per **device-local day** (reuse
   `todayLocal()` from `engagement-state.ts:41-46`); `kind:"weekly"` = week bucket; buckets stay
   derived, never stored computed state (EC-36).
9. Per-script typography (TRX-12): per-script size ×1.25 and Nastaliq for `ur` — reader settings
   surface, rides the v5 bump.

**Acceptance**: switching sessions never clobbers another session's position/recents/progress;
page/juz/hizb reading updates the active session (EC-42); sync round-trips sessions/positions/progress;
progress never regresses across devices (EC-05); translation switch keeps position (FLOW-07); anon
fully functional; gates green.

**Touch**: `reader-core.svelte.ts`, `reader-persistence.svelte.ts`, `reader-session.svelte.ts`,
`annotations.svelte.ts`, `resume.ts`, `RangeReader.svelte`, `translation-nav.ts`, `/yours`, palette
sources, `purge-hook.ts`; new `web/src/lib/sessions/{schema,store,domain}.svelte.ts` + Rust
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
- Design-system constraints (`docs/design-system.md` is normative): §6 hue slots
  (`var(--hue-N)`/`-soft`/`-legible`/`--on-hue-N`, never a colour prop), §13 radii (blocks 10 px,
  large panels 14 px, pills 999, `rounded-[Npx]` banned in `ui/`), §14 no elevation/shadows, §34
  `MetricCard` is non-interactive by design, §61 semantic tokens only. `Card` and `MetricCard` have
  **zero live consumers** and `MetricCard` is hue-capped `1|2|3|4` — widget primitives must extend
  the card grammar within these rules, not invent a parallel one.

**Build**

1. Widget registry: `web/src/lib/home/widgets.ts` —
   `{id, title, component, defaultSpan, settingsSchema}`. V1 widgets: `continue-reading`, `bookmarks`,
   `notes` (BM-03), `recents`, `most-read` (consumes Feature 6 when ready), `daily-ayah` (backend
   `/quran/random` already deterministic date-seeded — D07).
2. Layout model persisted under new key `easyquran.home` (v1):
   `{v, widgets:[{id, visible, span, settings}]}`; default = exactly today's `/yours` order (Continue
   first — this satisfies HOME-01 without touching the marketing hero). Unknown ids dropped on decode;
   missing ids appended.
3. Edit mode: show/hide toggles + move up/down (buttons first; drag optional, no new dep without
   approval). Per-widget settings (e.g. bookmarks widget: folder filter; recents: count).
4. Every widget needs explicit empty state (fix the vanishing-sections issue).
5. First-run card (HOME-04 adapted to `/yours`): one-line choice "Read in: Arabic only · Arabic +
   English · Arabic + اردو …" (default to browser language, remember the choice) and show the chosen
   translation name on the Continue card.
6. Home polish (HOME-02): centre column, stretch cards, real bookmark **count** in the numeral slot,
   icon fixes, 2560 px width no longer clamps at 1024 px.
7. Yours as the single hub (BM-01): Continue → Bookmarks → Notes → Recent, one verse format
   everywhere ("Al-Baqarah · verse 1"), same row component as `/bookmarks` (Feature 2.5).
8. Notes surfacing (BM-03): Notes widget on Yours (verse + first line of note + date); small note icon
   on verses in the reader; the verse panel opens straight to the note.
9. Wire the dormant `prefs.instantResume` (exists in `prefs.svelte.ts:85,203-205` but no UI consumer):
   "Resume instantly" becomes a Settings → Reading toggle (HOME-01 second pass).
10. Keep `/yours` `prerender = false`, client hydration via `onMount` as today; hrefs via `*For(ctx,…)`
    only.
11. Sync input: `settings-system.md` §7 says prefs sync has "no storage, endpoint, or conflict
    policy" — that gap is the cost of the synced-widget option (open decision §11.3).

**Acceptance**: default layout = today's `/yours` pixel-order; toggling/reordering persists; widget
decode is forward-compatible; no SSR/prerender regressions; marketing untouched; widgets obey
design-system §6/§13/§14/§34/§61; every widget has an empty state.

**Touch**: `yours/+page.svelte`, new `web/src/lib/home/*`, `prefs.svelte.ts` or new store, i18n
messages (`web/messages/reader/*.json`), `settings-document.ts` only if synced later (decide), card
primitives under `web/src/lib/components/card/*`.

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
5. GA4 honesty (SCR-05 + SET-04): either scrub surah paths from `page_view` or make product analytics
   opt-in, so the landing copy ("nothing tracks your reading") is true. Consent default is an open
   decision (§11.4); current defaults are analytics ON (`consent.svelte.ts:16`).
6. Local fallback: compute the same summary client-side from the reader blob for anon users; server
   summary takes over when signed in.

**Acceptance**: summary returns real most-read surah/ayah for a test user; consent off = no events
leave device; admin analytics untouched; no sha/Quran DB writes; rate limits + validation tested; GA4
paths scrubbed or consent default flipped.

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
- Route-grammar constraints (from `navigation-audit.md` + `url-scheme-a.md`, both implemented; stale
  `rub` rows there are superseded, §14):
  - `parseReaderPath` is **path-only** — no query or fragment enters the parser. Range permalinks must
    be additive query/fragment, never a new path shape.
  - `?v={surah}:{ayah}` is the scroll-position mirror and stays `{s}:{a}` — do not overload it for
    ranges; use a distinct param or extend deliberately at all three read sites + the write site
    (`SurahReader.writeHistoryState`).
  - `#ayah-{s}-{a}` is read as exactly **one** ayah in three places; range reveal needs new parsing.
  - Reserved-segment machinery must learn `azkar`: `RESERVED_SURAH_SEGMENTS` +
    `RANGE_SEGMENT_KINDS` (`web/src/lib/i18n/reader.ts:19-54`), `REJECT` (`web/src/params.ts:28-49`),
    `RANGE_SEGMENT`/`READER_ROUTE_PATTERNS` (`web/src/hooks.ts:12-19`), `isReaderPathname` segment
    cases (`reader.ts:131-149`).
  - `nav-guard.test.ts` `HAND_BUILT_RESERVED_SEGMENT` regex bans only `page|juz|hizb|rub|t` — extend
    it with the snippet segment so hand-built `/azkar/...` literals fail; keep new components inside
    the guarded glob roots (`(application)/**/*.svelte`, `#lib/components`, `#lib/search`).

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
   text + canonical URL). Permalink grammar (R11): either `?ayah=2:255-2:286` (extend the three `?v=`
   regex sites) or keep fragment-based `#ayah-2-285`..`#ayah-2-286` + snippet route. Decide once
   (§11.5), then update the R11 note in the gap catalogue.
6. Search coordinate parsing (R12): teach `TRAILING_REF` (`web/src/lib/search/nav/parse.ts:27`) the
   range form `2:255-286` once the grammar is fixed; today `-` is a separator and `2-255` parses as a
   single ayah.
7. Optional: "save snippet to folder" = bookmark all refs with a shared folder (Feature 2 synergy).

**Acceptance**: Ayat al-Kursi and last-two-of-Baqarah pages render Arabic + any chosen translation;
bookmarking a snippet stores the range; share includes permalink; nav-guard green; reserved segments
updated; caps enforced.

**Touch**: new snippets data + routes + i18n; `quran.ts` helpers + nav-guard + reserved-segment
machinery; `RangeReader`/`SurahReader`/`Sidebar` as needed; `reader-share.svelte.ts`;
`schema.ts`/`store.svelte.ts`/`domain.ts`; Rust migration + `bookmark_v1` validator/slice/tests.

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

1. **Derive grouping client-side** — normalized translator key
   (trim/casefold/whitespace-collapse, `"Unknown"` bucket) + `translationSourceOf(id)`. No catalogue
   schema change, no Rust change, no upload/deploy ripple. This supersedes the build-time dedupe +
   alias-id direction from ux-audit TRX-03 (§14).
2. Modal: one row per author (within language rail) → expandable source variants with provenance
   badges + size; "show all sources" toggle for exact-dup works; selection resolves to a concrete id
   and records MRU per author (Feature 1.4). Picker search uses the alias/fuzzy rules from Feature 1.5
   (TRX-04).
3. Only if the user approves a catalogue change (authorId/publisher fields), the full ripple must be
   done together: `db/quran/translations/scripts/lib.ts` (`writeCatalog` :331-344),
   `db/quran/qul/scripts/catalog.ts` (:91-112), `web/src/lib/data/translations.ts`
   (+`TRANSLATION_FIELD_COUNT`), `scripts/quran/upload.ts`, `deploy/fetch-quran-db.sh`, Rust
   `loader.rs`/`store.rs`, and sha-guard tests must stay green. Default recommendation: **don't** —
   derive.
4. URL grammar stays `/[surah]/t/[lang]/[translator]` (first-dot split, `quran.ts:60-67`); grouping
   is presentation-only. Readable slugs (`/t/en/haleem`, TRX-09) are superseded — not adopted (§14).
5. Labels come from the catalogue (`translator`, `name`, provenance), never from URL segments
   (TRX-15).

**Acceptance**: picker shows grouped authors with source variants; selecting a variant opens the
right id; recents rank the author not the variant; no catalogue break; gates green; catalogue guard
test extended for duplicate `(language, translator)` groups (ux-audit guardrail) with a frozen
allow-list for the 7 known exact-dup works.

---

## Feature 9 — Search: recent queries + relevance ranking (absorbed)

**Current state** (`docs/search-system.md`)

- Search shipped Aug 2026: palette discovery, Arabic worker→Rust fallback, translation full-text.
  Selection persistence already shipped (`web/src/lib/stores/search-selection.svelte.ts`,
  `easyquran.search.selection`, cross-tab) — the doc's §5 does not list it (doc-sync gap, fix in the
  same commit as this feature).
- Pending per search-system §5 + §8: reader-drawer integration, richer relevance ranking, recent-query
  history, Arabic-script typo tolerance, RTL-script translation search, Turkish dotted/dotless-i
  rules, palette cold-source second keystroke, localized group labels.

**Build**

1. Recent queries: new persisted store `easyquran.search.recent` v1
   (`{v, entries:[{q, at}]}`, cap ~10, dedupe consecutive). Display on empty palette + `/search`
   (when `?q` absent). Privacy-gated: local-only, no sync, explicit clear control; never record
   queries when the user is in a private/no-persist mode.
2. Relevance ranking: add a scoring pass for translation hits (today mushaf order, §8) — term
   frequency + early-verse bias + exact-phrase boost; keep the existing Arabic normalization contract
   untouched.
3. Palette polish: cold-source search should not require a second keystroke (warm the resolved
   source); localized group labels.
4. Reader drawer integration for the new search surface (D01 remaining) — lowest priority; ship after
   recents + ranking.
5. RTL-script translation search + Turkish `i`/`ı` folding; Arabic-script typo tolerance stays out
   unless bounded (document as known limit if not done).

**Acceptance**: recents persist + clear; ranking demonstrably improves translation-hit ordering vs
mushaf order in a fixture test; no regression to the Arabic parity contract; `search-system.md` §5/§8
updated to reflect selection persistence + shipped items.

**Touch**: new store + palette/search page components, `web/src/lib/quran/search/*`,
`web/src/lib/search/**`, `web/messages/*`, `docs/search-system.md`.

---

## Feature 10 — Locale suggest (Phase 1, absorbed)

**Source**: `docs/plan/09-locale-suggest.md` (draft, not implemented) — adopted as-is for Phase 1,
with the Phase 2 conflict resolved below.

**Build**

1. Detection: `suggestedUiLocale()` from first-match `navigator.languages`; `currentUiLocale(pathname)`;
   target URL via `localizeHref(deLocalizeHref(...))` + `/ar` normalization + same-path guard. Never
   redirect, never set cookies, never use geo/server signals.
2. Trigger matrix + dismissal contract from the plan §3: one-time popup, 6 trigger conditions,
   dismissal permanent per locale, forced-close writes nothing, re-check at fire, cross-tab
   `onStorageKey`.
3. Storage `easyquran.localeSuggest` v1 `{v, resolved:{locale:"accepted"|"dismissed"}}` (literal-union
   decode, in-memory write-through).
4. UI: `LocaleSuggest.svelte` (bits-ui Dialog copied from AuthModal, `z-[92]`, SSR-safe `{#if open}`),
   current-locale `lang`/`dir` + endonym, 5 message ids with Unicode isolation characters, 1500 ms
   double-rAF + `afterNavigate` re-eval + shown-latch + retry-once.
5. Analytics: `locale_suggest_shown|accepted|dismissed` + `dismiss_reason` — consent-gated, same
   pipeline as Feature 6.
6. Phase 2 (translation suggest): **superseded** — no `recommended` flag in the bake pipeline
   (§14). If revisited, derive recommendations client-side from browser languages + the existing
   catalogue (Feature 8 grouping gives the data).
7. Docs to amend in the same commit: `docs/quran-system.md` Part 2 (if it lists locale behavior) and
   gap catalogue L01 shipped-evidence.

**Acceptance**: first-run popup fires only under the trigger matrix; "dismiss" is permanent per
locale and cross-tab; no redirect ever; gates green; analytics events respect consent.

**Touch**: `web/src/lib/components/i18n/LocaleSuggest.svelte`, `web/src/lib/i18n/locale-suggest.svelte.ts`,
root layout mount, messages, tests per plan §8.

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
  limiter; batch ≤ 200; same LWW helpers (`db/sea_models/bookmark/actions.rs` pattern). Adopt the
  in-flight R2-08 hardening: `expectedUserId` on the request, server rejects cookie-owner mismatch
  before reads or writes.
- New `reading_session_v1`: `POST /reading-session/v1/sync` — mutations `session.upsert`,
  `session.delete`, `position.upsert`, `progress.upsert`; response
  `{applied, skipped, sessions, positions, progress}` (full snapshot per user, same as bookmarks);
  `auth_guard::verified`; 100/60 limiter; batch ≤ 200; same `expectedUserId` guard.
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
  so a device with a fast clock cannot dominate LWW forever (EC-34).
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
  flush; result toast "Merged X, skipped Y (newer account versions kept)" (ACCT-09 wording: promise
  "Keep your bookmarks and reading sessions safe and on all your devices" — notes stay local).
- **Not now**: nothing enqueued; marker `eq.merge.deferred.{userId}`; re-prompt on next app start (not
  every reload). **Don't ask again** sets `eq.merge.reviewed.{userId}`. Markers are per-account.
- Failure handling: markers are written only after enqueues resolve — fixes the current bug where
  `#migratedKeys` marks keys before the enqueue and a partial failure permanently loses those verses
  (EC-11).
- Legacy blob is never deleted (current behavior kept); the legacy boolean path migrates only through
  this review.

### 9.5 Logout, expiry, cache policy — **never destroy unsynced user data silently**

- **R2-08 basis**: adopt the in-flight account-isolation design — the durable outbox is partitioned by
  account, transport/response/optimistic work is guarded by generation/owner, and the server rejects
  `expectedUserId` mismatches before reads or writes. Divergence (this doc wins): legacy unowned
  entries are **quarantined, not discarded** — attribute them to the last known account when
  determinable (`eq.auth.session-hint`), else park them in `eq.sync.quarantine.legacy` and surface a
  one-time review on the next login (EC-48).
- **Explicit logout**: account view cleared from memory (privacy — the next user on the browser sees
  nothing); pending unsynced mutations move to a per-user quarantine key
  `eq.sync.quarantine.{userId}` (never destroyed — fixes today's `engine.clear("bookmarks")` data
  loss at `store.svelte.ts:415`); device-local reader data (position, "on this device" session, notes)
  kept so offline reading continues; synced sessions hidden until re-login. If pending/quarantined
  data exists, show a one-line notice on sign-out: "Your unsynced changes are kept on this device for
  your next sign-in" (ACCT-01 adapted — no copy-into-local prompt).
- **Session expiry / remote terminate / ban** (401/403 or probe → anonymous): same as logout for sync
  purposes + banner "Session expired — sign in to resume sync" (new copy). Wire the
  `current-session-terminated` transition — it is a dead kind today (no producer in `web/src`;
  `purge-hook.ts:7` branch unreachable). Detect via auth-client on 401/403 (`AUTH_004`/`AUTH_009`)
  and probe; also clear the stale in-memory CSRF token and retry once (FIX: stale-CSRF wedge, EC-15).
  Note R2-04: sessions now have a 14-day **absolute** age on top of inactivity expiry — expiry is
  bounded and reachable; the banner must be ready for it.
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
  fire transitions (none today — FIX, EC-31).
- **Overlay retire cross-tab**: BroadcastChannel `easyquran-sync`, messages
  `{domain, drainedIds, userId}` → other tabs retire optimistic overlays (fixes the DEFERRED note at
  `store.svelte.ts:346-347`). Payloads are ids only, never user content.
- **Shared outbox**: two tabs may take the same batch and push concurrently → server LWW is idempotent
  (safe, EC-32); cross-tab retire removes the double-overlay flicker.
- Engines keep independent retry state per tab.

### 9.7 Audit fixes this design must land (each has an EC scenario)

- Per-user gating: `applyServer` and enqueue callbacks gate on a boolean `authed`, not user id — a
  round from user A can land in user B's view on fast account switches (FIX, EC-17). R2-08's
  generation/owner guards are the adopted mechanism.
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
- Legacy unowned outbox entries: R2-08 discards; this doc quarantines (FIX, EC-48).

### 9.8 Sync UI + copy (absorbed)

- `SyncIndicator` typography: 12 px grey → `text-caption` (13.5 px); offline banner above the list
  (ACCT-05).
- Sign-out notice copy when quarantined data exists (§9.5, ACCT-01 adapted).
- Merge toast copy (ACCT-09): "2 bookmarks from this device were added to your account" style —
  honest promise wording that matches final scope (bookmarks + sessions sync; notes local).
- Bookmark local-only copy (BM-05): "Your bookmarks are saved on this device. Sign in (free) to keep
  them safe and see them on your other devices."
- Copy ledger pass (same phase): C01 (auth copy promises notes/progress sync — rewrite to match final
  scope), C02 (privacy/delete-account claims), C06/C07 (stale "on the way" FAQ copy — bookmark sync
  and 378 sources/105 languages are shipped), C08 (feature-truth review). Update
  `docs/remaining/copy-corrections.md` as entries resolve.

---

## 10. Edge-case catalogue — testable scenarios

Format: **EC-nn · title** — Given / When / Then + resolution. FIX = current-behavior bug this batch
fixes. Test targets: Rust integration (`rust/backend/api/tests/*.rs`), web unit
(`web/src/lib/**/__tests__/*.test.ts`), manual QA.

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
the queue, and the sign-out notice is shown. Nothing is destroyed. Test: `store.test.ts` +
`engine.test.ts`.

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

### I. Absorbed edge cases (from ux-audit / research)

**EC-45 · Empty translation verse (TRX-11)**
Given a verse blank in the selected translation (`fa.safavi` 80:39, `ku.asan` 108:3, `sq.mehdiu`
21:56 + 77:14). When it renders. Then Arabic shows with "No translation for this verse in
{translator}."; the translation span uses the declared direction, not `dir="auto"`; copy/share
strips the placeholder. DB rows stay untouched (immutable). Test: `ReadingAyah`/`VerseRow`
component test + copy test.

**EC-46 · Anonymous bookmark sort-by-date after v5 (BM-02)**
Given legacy anonymous bookmarks with no timestamps. When schema v5 hydrates them with `savedAt`.
Then they sort deterministically (legacy entries without a known time sort last, stable); newly
saved anon bookmarks carry real timestamps. Test: `reader-persistence.test.ts` + `store.test.ts`.

**EC-47 · Reading-mode bookmark capture (accepted)**
Given the user reads in reading mode (no verse actions instantiated by design,
`reader-virtualization.md`). When they want to bookmark. Then capture stays verse-mode-only —
documented accepted behavior, no invisible dead control. Test: manual QA.

**EC-48 · Legacy unowned outbox entries (FIX: R2-08 divergence)**
Given outbox entries with no account owner (pre-partition legacy). When the next login runs. Then
entries are attributed to the last known account when determinable, else parked in
`eq.sync.quarantine.legacy` and surfaced once for review — never silently discarded (R2-08 discards;
this doc wins). Test: `store.test.ts` + `outbox.test.ts`.

**EC-49 · Locale suggest dismissal (Feature 10)**
Given the locale-suggest dialog shown for locale `ur`. When the user dismisses. Then
`easyquran.localeSuggest.resolved.ur = "dismissed"` persists across reload and tabs; forced-close
writes nothing; no redirect ever fires; re-trigger only for a different, still-unsuggested locale.
Test: `locale-suggest.test.ts` + manual QA.

---

## 11. Open decisions (ask the user before building these)

1. ~~Bookmark uniqueness~~ **Resolved (§9.1)**: unique `(user_id, session_id, surah, ayah)`, sentinel
   `''` for global rows.
2. ~~Reading sessions sync~~ **Resolved (§9.2)**: sessions + positions + progress sync; recents stay
   device-local.
3. Widget layout: local-only (`easyquran.home`) or synced (wire `settings-document.ts` + backend)?
   Cost input: `settings-system.md` §7 — prefs sync currently has no storage, endpoint, or conflict
   policy.
4. Analytics: consent default (GA currently defaults analytics ON) and granularity sent (surah only vs
   surah+ayah)? Includes SCR-05/SET-04: scrub GA paths vs opt-in.
5. Range permalink grammar + span cap (Feature 7.5 / EC-39).
6. Azkar route name/scope: `/azkar` vs `/duas`; editorial governance for the curated list.
7. Folder `kind:"daily"` semantics: name-only label vs real goal metadata (`targetAyahsPerDay`) +
   progress bar.
8. Accepted losses to confirm: session row-level LWW (EC-06) and verse-collapse context loss (EC-04) —
   field-level merge is out of scope; confirm acceptable.
9. ~~Locale-suggest Phase 2 catalogue flag~~ **Resolved (§14)**: superseded — derive-only, no bake
   pipeline change.
10. ~~Search recents/ranking~~ **Resolved**: bundled as Feature 9.

## 12. Suggested sequencing

1. **Feature 1 + 8** — translation UX, no schema risk (absorbed TRX items + EC-45).
2. **Feature 3 + 4 + §9.1–9.3** — bookmark context + sessions: migrations (`m000011`+), tombstones,
   new `reading_session_v1` module, client domains, LWW/merge rules; schema v5 (translationFamily,
   anon timestamps, notes scoping). (Keep the existing silent bookmarks migration until step 3 lands.)
3. **§9.4–9.8 + §10 FIX items** — merge review (non-silent), quarantine, cross-tab auth + overlay
   retire, R2-08 rebase (EC-48), copy pass (C01/C02/C06/C07/C08 + merge/session copy), and every FIX
   scenario (EC-02, 11, 14–17, 25–30, 40, 42–44).
4. **Feature 2** — folders enrich on top (kind/position/bulk + absorbed UI items).
5. **Feature 7** — snippets; extends the bookmark model with ranges (reserved-segment machinery first).
6. **Feature 5** — widgets; consumes sessions (design-system constraints; HOME-01/02/04; BM-01/03).
7. **Feature 6** — analytics; consumes session emit points (SCR-05/SET-04 included).
8. **Feature 9** — search recents + ranking (after F6 consent pipeline exists).
9. **Feature 10** — locale suggest Phase 1 (independent; can slot anywhere after F6 for its events).

Each phase ends with all three gates green and its own commit(s) on the current branch. Every phase
lands its EC scenarios as tests (Rust integration + web unit) in the same commit.

---

## 13. Absorbed work map

| Source                                                                                             | Items                                                                                                      | Absorbed into                                                                                      |
| -------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `research/ux-audit`                                                                                | BM-01, BM-02, BM-03, BM-04, BM-05                                                                          | Features 2, 3, 5, §9.8                                                                             |
|                                                                                                    | ACCT-01, ACCT-03, ACCT-04, ACCT-05, ACCT-07, ACCT-08, ACCT-09                                              | §9.5, §9.8, Features 2, 3                                                                          |
|                                                                                                    | HOME-01, HOME-02, HOME-04                                                                                  | Feature 5                                                                                          |
|                                                                                                    | TRX-02, TRX-04, TRX-05, TRX-06, TRX-10, TRX-11, TRX-12, TRX-13, TRX-14, TRX-15, TRX-16                     | Features 1, 4, 8, EC-45                                                                            |
|                                                                                                    | TRX-03, TRX-09                                                                                             | Superseded (§14)                                                                                   |
|                                                                                                    | FLOW-07, FLOW-08                                                                                           | Features 4, 2                                                                                      |
|                                                                                                    | SCR-05, SET-04                                                                                             | Feature 6                                                                                          |
|                                                                                                    | Guardrails: catalogue duplicate `(language, translator)` test; round-trip persistence test                 | Feature 8 acceptance, §10                                                                          |
| `plan/09-locale-suggest.md`                                                                        | Phase 1 (all)                                                                                              | Feature 10                                                                                         |
|                                                                                                    | Phase 2 + open q2 (catalogue flag)                                                                         | Superseded (§14)                                                                                   |
| `settings-system.md`                                                                               | §4 translationFamily persistence; §7 prefs-sync gap                                                        | Feature 4.3, §11.3                                                                                 |
| `search-system.md`                                                                                 | recent queries, relevance ranking, drawer integration, §8 known limits; selection-persistence doc-sync gap | Feature 9                                                                                          |
| `remaining/copy-corrections.md`                                                                    | C01, C02, C06, C07, C08                                                                                    | §9.8 copy pass                                                                                     |
| `security-audit-2026-10-11.md`                                                                     | deploy validation, recent-auth, Argon2 semaphore, CI gates                                                 | **Not product features** — tracked in the audit; only auth-lifecycle items referenced (§9.5, §9.7) |
| `security-audit-rounds-2026-10-11.md`                                                              | R2-08 account isolation (adopted + divergence), R2-04 absolute age                                         | §9.2, §9.5, §9.7, EC-48                                                                            |
| `research/translation-empty-verses.md`                                                             | 4 blank verses + display decision                                                                          | Feature 1.8, EC-45                                                                                 |
| `research/navigation-audit.md` + `url-scheme-a.md`                                                 | path-only parser, `?v=` scope, fragment single-ayah, reserved segments, nav-guard regex                    | Feature 7 constraints                                                                              |
| `design-system.md`                                                                                 | §6/§13/§14/§34/§61 constraints; Card/MetricCard unused + hue cap                                           | Feature 5                                                                                          |
| `reader-virtualization.md`                                                                         | reading mode has no verse actions                                                                          | EC-47                                                                                              |
| `plan/README.md`                                                                                   | 10 owner items (Section.svelte delete, CI workflow, dead tokens, accents)                                  | **Not absorbed** — separate small backlog                                                          |
| `plans/remove-primary-translation.md`                                                              | done (`502effc2`)                                                                                          | Archive; "never primary wording" noted in Feature 1.6                                              |
| `research/mushaf-rendering-research.md`, `indopak-*`, `qurancom-mirror.md`, `design-enhancements/` | orthogonal projects/proposals                                                                              | **Not absorbed** — separate tracks                                                                 |

## 14. Superseded directions (this doc wins)

1. **ux-audit TRX-03** (build-time dedupe by `(language, translator)` + alias ids) → superseded by
   Feature 8 runtime grouping. Alias-id machinery is not needed; no catalogue rewrite.
2. **ux-audit TRX-09** (readable QUL slugs `/t/en/haleem`) → superseded by Feature 8.4; URL grammar
   unchanged.
3. **ux-audit ACCT-01** (copy bookmarks into the local store on sign-out) → superseded by §9.5
   quarantine + sign-out notice.
4. **security rounds R2-08 "discard legacy unowned entries"** → superseded by §9.5/EC-48: quarantine
   and attribute, never discard silently. The rest of R2-08 (partition, `expectedUserId`, generation
   guards) is adopted.
5. **ux-audit HOME-01** (continue card above the marketing hero) → superseded by Feature 5: widgets on
   `/yours`, marketing stays static; HOME-04's first-run choice lands on `/yours`.
6. **`plan/09-locale-suggest.md` Phase 2 + open q2** (catalogue `recommended` flag) → superseded by
   Feature 8 derive-only. Phase 1 adopted as-is.
7. **`copy-corrections.md` C01 prescribed fix** ("scope the promise to bookmarks") → superseded by the
   new scope: bookmarks + sessions sync; notes stay local. Copy pass in §9.8.
8. **`design-enhancements/`** (card/outline studies) → not adopted; its specifics (font weights >600,
   radii 12–20 px) violate current design-system guards.
9. **`navigation-audit.md` D6 / `url-scheme-a.md` §1–2 rub rows** (`/rub/N` live) → stale; rub routes
   were retired (`5d54ca6b`), 308 to the quarter's first ayah. Code wins.

Doc status after absorption: `plans/remove-primary-translation.md` archive (done); `plan/README.md`
keep (separate backlog); `plan/09-locale-suggest.md` keep (Feature 10 is its implementation contract);
`feature-gap-catalogue.md` keep (parity register; R11 note to update when §11.5 is decided);
`ux-audit` keep (checklist; absorbed items are cross-referenced here).
