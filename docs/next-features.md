# EasyQuran — Fine-Grained Feature Prompt (v2)

Grounded in current code. Implementation brief for the next feature batch: translation recents/labels,
bookmark collections with context, reading sessions, homepage widgets, personal analytics, azkar
snippets, author/source grouping, search recents/ranking, and locale suggest — plus a committed sync
design for bookmarks + reading sessions (§9) and a testable edge-case catalogue (§10).

**Authority**: this document is the latest direction. Where an older doc conflicts, this document
wins — see §14 (Superseded directions). Pending work absorbed from `ux-audit`,
`plan/09-locale-suggest.md`, `settings-system.md`, `search-system.md`, `copy-corrections.md`, the
security audits, and research docs is mapped in §13.

**Baseline**: "current state" claims and line numbers are measured at `5b0674b2` plus the
uncommitted R2-08 account-isolation work in the working tree (audited 2026-10-11). Where R2-08
already fixes a FIX scenario, the scenario says so and stays as a regression test.

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
  `expectedUserId`, owner/generation guards) is in flight in the working tree. §9.5 adopts it as the
  basis with one divergence: legacy unowned entries are **retained for review, never discarded**
  (EC-48). R2-08 as written calls `#discardUnownedQueue()` from `BookmarksStore.hydrate()`
  (`web/src/lib/bookmarks/store.svelte.ts`), which wipes those entries on the first boot of the new
  build — so **swap the discard for retain-and-review inside R2-08 before it lands**, otherwise the
  data EC-48 protects is gone before this batch starts. Everything else in R2-08 lands as is; this
  design rebases on it, never around it.

---

## Feature 1 — Recently used translations: recency ranking + proper labels

**Current state**

- Picker `web/src/routes/(application)/_reader/TranslationModal.svelte` groups by language rail, sorts
  by `Intl.Collator` name (`:160-179`), no recency.
- Only recency today: `recent[]` (cap 8, `READING_RECENT_MAX`) in
  `web/src/lib/stores/reading-text.svelte.ts:24,95-121`, surfaced as quick chips via
  `readingQuickPicks` (`web/src/routes/(application)/_reader/reading-flow.ts:22-32`). Stacked list
  (`stacked-translations.svelte.ts`) and search selection (`search-selection.svelte.ts`) are unordered.
- Provenance derivable only from id prefix: `translationSourceOf(id)` in
  `web/src/lib/quran/catalogue.ts:25-31` → `tanzil | quranenc | qul`. Tooltip already shows translator
  - provenance + language + size (`TranslationModal.svelte:666-713`).

**Build**

1. New persisted MRU store `web/src/lib/stores/translation-recents.svelte.ts`, key
   `easyquran.translations.recent` (v1 envelope like other stores:
   `{v, entries:[{id, lastUsedAt, useCount}]}`, cap ~20). Record on **every explicit selection**: modal
   pick, quick chip, search selection. Landing on a `/t/…` route (shared link, SSR entry, back
   button) is **not** a selection and records nothing — ranking changes only on in-app choices.
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
9. Tafsir is commentary, not translation (TRX-01, **P0**): the catalogue files `ar.jalalayn` and
   `ar.muyassar` (both `تفسير …`) under Arabic translations, and choosing one as the reading
   translation puts commentary in the verse position with verse markers. Derive a `tafsir` kind
   client-side from a frozen id allow-list (no catalogue change, same derive-only rule as
   Feature 8): own "Commentary (tafsir)" section in the picker, never offered as the reading
   translation; when stacked, render under the Arabic verse in a distinct block labelled
   "… (commentary)" in the UI font; every reader route with the `ar/muyassar` or `ar/jalalayn`
   translation segments (`/{surah}/t/ar/…`, `/t/ar/…/{page|juz|hizb}/{n}`) 308s to the Arabic
   reader with the tafsir stacked. EC-50.
10. Modal polish (TRX-07/08): at the 5-translation cap, disabled rows read as disabled (muted
    label, reduced opacity) and the footer ends at "Remove one to add another."; keep live apply,
    keep one close control ("Done" at the bottom on phones), confirm changes with an Undo toast.

**Acceptance**: recents survive reload; ranking only changes on explicit selection; no new catalogue
fields required (derivation only); modal labels show author + source for every row; picker search
finds "Saheeh" from "sahih"; empty verses render the placeholder + declared `dir`; chip strip never
overflows without a summary; no route shows tafsir in the verse position; gates green.

**Touch**: `TranslationModal.svelte`, new store, `reading-text.svelte.ts` (keep its 8-cap chips,
delegate ordering to MRU), `reading-flow.ts`, `VerseRow.svelte`/`ReadingAyah.svelte` (transliteration +
empty verse + tafsir block), tafsir route 308 (`web/src/lib/server/reader-route.ts` /
`hooks.server.ts`), copy strings.

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
   integer, LWW), optional plan metadata `{targetAyahsPerDay?}` for `kind:"daily"` (only if §11.7
   picks goal metadata). A folder `kind:"daily"` is a **set of verses** to study; it is unrelated to
   a reading session `kind:"daily"` (a position bucket, Feature 4) — copy must not conflate them.
2. Backend: the folder columns `kind`, `position`, `deleted_at` already land in phase 2's bookmarks
   migration (§9.1, `m000011`); this feature only wires them in `bookmark_folder/slice.rs` +
   validator (`bookmark_v1/validator.rs`, mutation cap 200 stays). `target_ayahs_per_day` gets its
   own migration (next free number) only if §11.7 adopts it. Folder delete is a tombstone (§9.3):
   the delete action must **still detach children explicitly** — the schema's
   `ON DELETE SET NULL` FK never fires on a tombstone `UPDATE` — and `resolve_folder_id` must treat
   tombstoned folders as missing (`deleted_at IS NULL`), so a stale offline move lands at root
   (EC-07).
3. UI: folder sections sorted by `position` then name; per-folder count (exists); bulk move/delete
   (P05); folder kind picker in `FoldersPanel.svelte`; for `kind:"daily"`, a progress bar = share
   of the folder's bookmarked verses at or below the **active session's** `furthestAyah` for their
   surah (Feature 4).
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
`BookmarkRow.svelte`, `bookmarks/+page.svelte`, `db/sea_models/bookmark_folder/*`, `bookmark_v1/*`,
toast/undo primitive; a Rust migration only for `target_ayahs_per_day` (§11.7).

---

## Feature 3 — Fine-grained bookmarks: translation + session context

**Current state**

- Toggle captures only `verseKey` (`VerseTools.svelte:80` → `bookmarks.toggleVerse`); `Bookmark` has no
  `sourceId`, note, page, or anchor. Bookmarks list opens **Arabic-only**
  (`ARABIC_CTX = surahRouteContext("uthmani")` at `bookmarks/+page.svelte:23`, `yours/+page.svelte:112`),
  even though the route context exists (`surahRouteContext(sourceId)`, `quran.ts:87-91`).
- Unique index `(user_id, surah, ayah)` = one row per verse, ever.

**Build**

1. Extend `Bookmark` with `sourceId?: string|null` (translation context), optional `page` +
   `anchorRatio` (P03 reading bookmark), `sessionId?: string` (Feature 4; `''` = global), nullable
   `endSurah`/`endAyah` on the wire (Feature 7 ranges; the columns land here, Feature 7 populates
   them). **No `note` field**: notes stay local (§11, decided); `BookmarkRow` looks up the local
   note by verse key at render time.
2. **Resolved uniqueness** (§9.1): one live row per
   `(user_id, session_id, surah, ayah, end_surah, end_ayah)`. Sentinel `session_id = ''` for
   global/legacy rows and `end = start` for single-ayah rows — SQLite treats NULLs as distinct in
   unique indexes, so never bare NULL in the key. The range end is part of the key so a range
   bookmark (2:285–286) and a single-ayah bookmark (2:285) coexist instead of verse-collapsing into
   each other (EC-51). `sourceId` is context metadata, not a key part (row-level LWW, §9.3).
3. Migration `m000011`: add the columns, backfill `end_surah = surah, end_ayah = ayah`, drop
   `idx_bookmarks_user_verse` and create the new partial unique index (§9.1). No dedupe pass —
   m000008 already guarantees one row per verse, and adding `session_id = ''` + `end = start`
   keeps that unique. Update wire slices, `bookmark_v1/validator.rs`, client `schema.ts` decoders
   (wire `null` end ⇄ stored `end = start`). Deletes become tombstones (§9.3). The client collapse
   in `applyServer` (today `newestPerVerse`) re-keys on the full unique key.
4. Capture at toggle time: read current `sourceId` from route context (`routeContextFromParams` /
   `SurahReader` initial). New bookmarks are **global by default** (`sessionId = ''`); a bookmark is
   session-scoped only when created with an explicit "this session only" choice — otherwise every
   bookmark would vanish from view whenever the reader switches sessions. Capture surface = verse
   mode (VerseTools); reading mode does not instantiate verse actions (accepted, EC-47).
5. List/resume: build hrefs with `surahAyahPathFor(surahRouteContext(bookmark.sourceId ?? "uthmani"), …)`
   so bookmarks reopen in their saved translation; show source badge on `BookmarkRow`.
6. Count + sync label (ACCT-03): home/bookmarks card shows the real count and "3 saved · synced" once
   sync is live.
7. Share seam: keep `reader-share.svelte.ts` untouched here; range/copy work is Feature 7.

**Acceptance**: bookmark created on a `/t/…` route reopens in that translation; lists show global
bookmarks plus the active session's scoped ones; deletions are tombstones so stale queued upserts
cannot resurrect (EC-02); range and single-ayah bookmarks on the same verse coexist (EC-51); anon
path unchanged; saved translation missing from catalogue falls back to Arabic (EC-38).

**Touch**: `schema.ts`, `store.svelte.ts`, `domain.ts`, `VerseTools.svelte`, `BookmarkRow.svelte`,
`bookmarks/+page.svelte`, `yours/+page.svelte`, Rust `migration/src/m000011_*.rs` + `bookmark_v1` +
`db/sea_models/bookmark/*` (`upsert_lww`/`delete_lww` rewritten for tombstones + the new key), tests
`rust/backend/api/tests/bookmarks_v1.rs`.

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
- Logout wipes position (`web/src/lib/auth/purge-hook.ts` `CLEAR_POSITION_KINDS`). No backend
  module/tables.
  P03/P04/P08 gaps.

**Build**

1. Model (local-first, then synced):
   `ReadingSession {id, name, kind: "daily"|"weekly"|"free"|"shared", translationId?: string|null, createdAt, archivedAt?: number|null, sharedWith?: string[]}`
   — `shared` is a **seam only**; no social feature, no invites, no server sharing.
2. Session storage gets **its own per-owner key**, not the reader blob:
   `easyquran.sessions.{owner}` v1 (owner = user id when signed in, `local` otherwise) =
   `{v, activeId, sessions:[…], state:{[sessionId]: {lastRead, lastReadAnchor, recents, progress}}}`.
   Why not a reader-blob v5 field: `writeBlob` writes an explicit field list
   (`reader-persistence.svelte.ts:176-210`), so a tab still running the previous build across a
   deploy rewrites the v4 shape and would wipe sessions embedded in the blob (EC-54); per-owner keys
   also keep one account's sessions out of another account's view (§9.5).
   - One-time migration: the v4 blob's `lastRead`/`lastReadAnchor`/`recents`/`progress` move into a
     default `free` session named "Reading" owned by `local`, with the fixed constant id
     `LOCAL_DEFAULT_SESSION_ID` (never uploaded). A marker makes it run once; the v4 fields become
     migration input only (new code stops writing them; an old tab still writing them is harmless).
   - The user id cannot seed this id: the reader hydrates before auth resolves, and anonymous users
     have none. On first login the merge review (§9.4) maps the local default session onto the
     **account default session** whose id is UUIDv5(fixed namespace, user id) — every device of the
     account derives the same id, so they converge instead of duplicating "Reading" (EC-22).
     Positions merge LWW, progress MAX (§9.3).
3. Reader blob v5 (bump `READER_SCHEMA_VERSION`, extend `decodeReader` + `writeBlob` +
   `applyPersisted`) absorbs the small pending items, **additive fields only**:
   - persist `translationFamily` (TRX-10; today session-only per `settings-system.md` §4), and add
     it to `SettingsReadingDoc` (`settings-document.ts`) so settings export/import carries it;
   - anonymous bookmark timestamps as a new `bookmarkSavedAt: Record<VerseKey, number>` beside the
     boolean map (BM-02 second pass) — an old tab dropping it only makes those entries sort as
     legacy (EC-46);
   - notes move to a per-owner key `easyquran.notes.{owner}` (privacy fix, EC-17). Legacy blob
     notes attach to the owner signed in at the first post-auth hydrate, else to `local`.
4. Writers take/derive `sessionId` (active session of the current owner): `openVerse`, `markRead`,
   `recordProgress`, `pushRecent`, `clearReadingPosition`; range readers start calling `markRead`
   with the range's first/last visible verse.
5. Surfaces: session switcher (in `/yours` header + settings), `/yours` continue card reads active
   session; palette `continueReading` (`app-actions.ts:39-67`), global hotkey,
   `resumeToVerse/resumeToLastRead` (`web/src/lib/reader/resume.ts`) become session-aware;
   `resumeCtxFor` call sites on index pages.
6. Position preservation on context switch (FLOW-07): switching translation/language from the reader
   keeps the current position (`translation-nav.ts` `hrefFor` path), never resets to surah top.
7. **Sync (committed, not optional)**: new `reading-session` domain → `reading_session_v1` module +
   tables (§9.1–9.2). Sessions, positions, and per-surah progress all sync; **recents stay
   device-local** (navigation convenience, no sync value). Client store modeled on `BookmarksStore`
   including the R2-08 owner/generation guards; per-owner outbox partition `sessions.user:{id}`;
   register via `registerDomain`. Logout/expiry follows the quarantine policy (§9.5).
   `ReadingSession.sharedWith` is a local-only seam — never a column, never sent.
8. Daily/weekly semantics: `kind:"daily"` = day buckets derived per **device-local day** (reuse
   `todayLocal()` from `engagement-state.ts:41-46` — module-private today, export it);
   `kind:"weekly"` = week bucket; buckets stay derived, never stored computed state (EC-36).
9. Per-script typography (TRX-12): per-script size ×1.25 and Nastaliq for `ur` — reader settings
   surface, rides the v5 bump.

**Acceptance**: switching sessions never clobbers another session's position/recents/progress;
page/juz/hizb reading updates the active session (EC-42); sync round-trips sessions/positions/progress;
progress never regresses across devices (EC-05); translation switch keeps position (FLOW-07); a tab
on the previous build never wipes sessions (EC-54); anon fully functional; gates green.

**Touch**: `reader-core.svelte.ts`, `reader-persistence.svelte.ts`, `reader-session.svelte.ts`,
`annotations.svelte.ts` (notes key), `settings-document.ts` (`translationFamily`),
`engagement-state.ts` (export `todayLocal`), `resume.ts`, `RangeReader.svelte`, `translation-nav.ts`,
`/yours`, palette sources, `purge-hook.ts`; new `web/src/lib/sessions/{schema,store,domain}.svelte.ts`

- Rust `reading_session_v1` module + migration `m000012`.

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
   `{v, widgets:[{id, visible, span, settings}]}`; default = the BM-01 hub order (item 7): Continue →
   Bookmarks → Notes → Recents visible, `most-read` + `daily-ayah` present but hidden. Continue stays
   first — this satisfies HOME-01 without touching the marketing hero. Unknown ids dropped on decode;
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
    only. `nav-guard.test.ts` only scans `(application)/**/*.svelte`, `#lib/components/**/*.svelte`
    and `#lib/search/**/*.ts` — extend its globs to `#lib/home/**` and `#lib/sessions/**` in the same
    commit, since widget and session code builds reader hrefs outside today's roots.
11. Sync input: `settings-system.md` §7 says prefs sync has "no storage, endpoint, or conflict
    policy" — that gap is the cost of the synced-widget option (open decision §11.3).

**Acceptance**: default layout = BM-01 hub order with Continue first; toggling/reordering persists;
widget decode is forward-compatible; no SSR/prerender regressions; marketing untouched; widgets obey
design-system §6/§13/§14/§34/§61; every widget has an empty state; nav-guard covers the new dirs.

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
   shape): `auth_guard::verified` as the module's own `route_layer` (as in `bookmark_v1/mod.rs`),
   nested in `router.rs` with a rate limit (~100/60 like bookmarks). Register in `modules/mod.rs`.
2. Migration (app SQLite only — Quran DBs untouched; next free number after `m000012`):
   `reading_events(id, user_id, surah, ayah, source_id NULL, kind, seconds NULL, local_day, created_at)`
   with index `(user_id, created_at)`, and/or rollup
   `reading_daily(user_id, local_day, surah, ayahs_read, seconds)`. `local_day` is the
   **client-supplied** device-local `YYYY-MM-DD` — the server never derives day boundaries (EC-36).
   `user_id` FK `ON DELETE CASCADE` so account deletion purges history. Most-read =
   `GROUP BY surah[,ayah] ORDER BY COUNT(*) DESC` (SQL pattern lives in
   `analytics_v1/controller.rs:582-628`; bucket helper in its validator).
3. Endpoints: `POST /user/analytics/v1/events` (batched, capped, RFC3339; verses validated against
   **per-surah ayah counts**, not the flat `1..286` the bookmark validator uses),
   `GET /user/analytics/v1/summary` (top surahs, top ayahs, totals, streak-adjacent day counts).
   Never expose other users' data. Retention: raw events kept a bounded window (initial: 90 days),
   then only the rollup; both included in account export/delete.
4. Web emit: from Feature 4 writers (`markRead`/`openVerse`/`recordProgress`), gated by an
   **explicit opt-in** that is separate from the GA `analytics` consent flag. A per-ayah reading
   history reveals religious practice — GDPR Art. 9 special-category data — so it is never on by
   default, regardless of how §11.4 settles the GA default. Transport: the sync engine outbox
   (domain `analytics`, per-owner partition) or `AuthClient` with CSRF (do not miss `csrf-token` —
   see the existing FCM registration bug for what happens when you do). If it rides the engine, its
   `sync` pull must return an empty state — the engine's periodic 30s round pulls every registered
   domain, so the summary is fetched separately via `GET`. Queued events follow §9.5 on logout;
   opt-out drops queued events. Update privacy copy (`web/messages/en.json`, settings privacy
   section) and resolve SCR-05 contradiction.
5. GA4 honesty (SCR-05 + SET-04): either scrub surah paths from `page_view` or make product analytics
   opt-in, so the landing copy ("nothing tracks your reading") is true. Consent default is an open
   decision (§11.4); current defaults are analytics ON (`consent.svelte.ts:16`).
6. Local fallback: the reader blob cannot answer "most read" (it holds only `furthestAyah` per surah
   and 8 recents, no counts). Anon users get a capped device-local rollup
   `easyquran.analytics.local` (per-surah/ayah counts by local day) written by the same emit points
   under the same opt-in; server summary takes over when signed in.

**Acceptance**: summary returns real most-read surah/ayah for a test user; opt-in off = no events
leave device and no local rollup is written; account deletion purges events + rollup; admin
analytics untouched; no sha/Quran DB writes; rate limits + per-surah validation tested; GA4 paths
scrubbed or consent default flipped.

**Touch**: Rust module + migration + sea-models + tests; web store/emit + consent + copy.

---

## Feature 7 — Sliced ayahs / azkar sets (Ayat al-Kursi, last two of Al-Baqarah)

**Current state**

- No azkar/zikr/dua content anywhere (D06 gap: "curated verse/range references rather than copied
  Quran text"). No range support in URLs (`?v=` read with the strict regex `/^(\d+):(\d+)$/` in
  `SurahPageRoute.svelte:77` + `Sidebar.svelte:109`, written at `SurahReader.svelte:574-577`),
  bookmarks (one ayah/row), or share (`reader-share.svelte.ts` single verse, no URL at all — FLOW-05).
  Route grammar is closed
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
    ranges; use a distinct param or extend deliberately at both read sites + the write site
    (`SurahReader.writeHistoryState`).
  - `#ayah-{s}-{a}` is read as exactly **one** ayah in four places (`SurahPageRoute.svelte:74`,
    `Sidebar.svelte:107`, `VerseRow.svelte:65`, `ReadingAyah.svelte:27`); range reveal needs new
    parsing.
  - Reserved-segment machinery must learn `azkar`: `RESERVED_SURAH_SEGMENTS`
    (`web/src/lib/i18n/reader.ts:24-54`) and `REJECT` (`web/src/params.ts:28-49`). **Not**
    `RANGE_SEGMENT_KINDS` / `RANGE_SEGMENT` — those are numeric range kinds (`/page/{n}`), and a
    snippet takes a slug. If translated snippets join the reader grammar (§11.6), they also need a
    slug branch in `parseReaderPath`, `READER_ROUTE_PATTERNS` (`web/src/hooks.ts:12-19`),
    `isReaderPathname` (`reader.ts:131-149`), and an azkar cache kind in the SSR disk-cache key
    (`hooks.server.ts:52` → `quran-disk-cache.ts` `diskCacheKey`).
  - `nav-guard.test.ts` `HAND_BUILT_RESERVED_SEGMENT` regex bans only `page|juz|hizb|rub|t` — extend
    it with the snippet segment so hand-built `/azkar/...` literals fail; keep new components inside
    the guarded glob roots (`(application)/**/*.svelte`, `#lib/components`, `#lib/search`).

**Build**

1. Curated snippet data `web/src/lib/data/snippets.ts` (or `azkar.json`):
   `{slug, title, kind:"essential"|"azkar"|"dua", refs:[{surah, ayahStart, ayahEnd}], description}`
   — references only, never copied Quran text (immutable-DB rule + D06 wording). Seed:
   `ayat-al-kursi` → 2:255; `baqarah-last-two` → 2:285-286; add a few essentials.
2. Routes: `/azkar` index (Arabic-only prerender allowed, static editorial) + `/azkar/[slug]` detail
   (Arabic; prerender allowed). The translated detail URL shape is an open decision (§11.6) —
   preferred `/t/{lang}/{translator}/azkar/{slug}`, mirroring range routes, which is SSR + 7-day disk
   cache like `/t/**` (never SSG) and needs the grammar/cache work listed above. Add
   `snippetPathFor(ctx, slug)` to the `quran.ts` helper family and extend `nav-guard.test.ts`
   accordingly; do not hand-build URLs.
3. Rendering: reuse the range reading path (`RangePageData`, `RangeReader` internals) with translation
   context; each snippet verse gets normal verse tools (bookmark/copy/share) with its saved/current
   `sourceId`.
4. Range bookmarks: populate the `endSurah`/`endAyah` columns that phase 2 already added (§9.1;
   wire `null` = single ayah, stored as `end = start`). Validator: `(endSurah, endAyah) ≥
(surah, ayah)` compared lexicographically, every endpoint checked against **per-surah ayah
   counts** (today's flat `ayah 1..286` lets `1:286` through), span cap (~50 like `?keys`, which the
   API caps at 50; EC-39). The range end is part of the unique key (§9.1), so a snippet bookmark
   never collapses an existing single-ayah bookmark (EC-51). `BookmarkRow` range label
   ("2:285–286"), and resume via `surahAyahPathFor` + range highlight.
5. Copy/share (FLOW-05 absorbed): every share — single verse or range — includes the absolute
   deep link (`surahAyahPathFor(…)` with the reader's translation context), plus the translation
   line when one is shown; a failed share copies the link and says "Couldn't share — link copied
   instead" (today the `"failed"` result is ignored). Range variant: loop `verseRef`, joined text +
   canonical URL. Permalink grammar (R11): either `?ayah=2:255-2:286` (extend the two `?v=` read
   sites + the write site) or keep fragment-based `#ayah-2-285`..`#ayah-2-286` + snippet route.
   Decide once (§11.5), then update the R11 note in the gap catalogue.
6. Search coordinate parsing (R12): teach `TRAILING_REF` (`web/src/lib/search/nav/parse.ts:27`) the
   range form `2:255-286` once the grammar is fixed; today `-` is a separator and `2-255` parses as a
   single ayah.
7. Optional: "save snippet to folder" = bookmark all refs with a shared folder (Feature 2 synergy).

**Acceptance**: Ayat al-Kursi and last-two-of-Baqarah pages render Arabic + any chosen translation;
bookmarking a snippet stores the range without touching single-ayah bookmarks; every share includes
a link; nav-guard green; reserved segments updated; caps enforced.

**Touch**: new snippets data + routes + i18n; `quran.ts` helpers + nav-guard + reserved-segment
machinery (+ `reader-route.ts`, `hooks.ts`, `hooks.server.ts`, `quran-disk-cache.ts` if translated
snippets join the grammar); `RangeReader`/`SurahReader`/`Sidebar` as needed;
`reader-share.svelte.ts`; `schema.ts`/`store.svelte.ts`/`domain.ts`; Rust `bookmark_v1`
validator/slice/tests (no migration — columns land in phase 2).

---

## Feature 8 — One author, multiple sources: grouped translations

**Current state**

- Catalogue = 378 baked rows in `web/src/lib/data/translations.json` (positional arrays,
  `TRANSLATION_FIELD_COUNT = 8`, decoder `web/src/lib/data/translations.ts`); provenance only via id
  prefix (`catalogue.ts:25-31`). No `authorId`/`publisher`/`source` field in web or Rust
  (`RawCatalogueEntry`/`CatalogueEntry` in `rust/backend/api/src/quran/loader.rs:176-194`,
  `store.rs:277-286`).
- Real duplicate-author data already present: 44 exact-string translator collisions across ≥2
  sources (47 under the normalized key of Build 1); exactly 7 exact `(language,name,translator)`
  groups: `en.sahih` ↔ `qul.r20.en-sahih-international`, `ro.grigore` ↔ `qul.r44.ro-grigore`,
  `quranenc.ja.sato` ↔ `qul.r218.ja-saeed`, `quranenc.pt.nasr` ↔ `qul.r103.pt-helmi`,
  `quranenc.sv.rwwad` ↔ `qul.r1627.sv-rwwad`, `quranenc.ta.omar_brief` ↔ `qul.r1633.ta-omar-brief`,
  `quranenc.uz.rwwad` ↔ `qul.r868.uz-rowwad`. Same translator but different `name` (so grouped,
  not exact): `en.pickthall` ↔ `qul.r19.quran-en-pickthall`, `fr.hamidullah` ↔
  `qul.r31.quran-fr-hamidullah`. Attribution mismatch: `quranenc.en.saheeh` (name "Saheeh
  International", translator "Noor International Center"). Modal does not dedupe
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
allow-list of the 7 exact-dup pairs listed above, keyed by the same normalization as Build 1.

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
   (when `?q` absent). Privacy-gated: local-only, no sync, explicit clear control plus a "Pause
   search history" toggle (new — the app has no private/no-persist mode today); when storage is
   unavailable, recents live in memory only and are never written.
2. Relevance ranking: add a scoring pass for translation hits (today mushaf order, §8) — term
   frequency + early-verse bias + exact-phrase boost; keep the existing Arabic normalization contract
   untouched.
3. Palette polish: cold-source search should not require a second keystroke (warm the resolved
   source); localized group labels. ⌘K "go to verse" (FLOW-06) looks shipped — the palette builds
   verse hrefs with `surahAyahPathFor` (`web/src/lib/search/palette/quran-nav.ts:45`) and
   `VerseRow`/`ReadingAyah` apply `revealed-ayah` from the hash; confirm in QA and tick it off in the
   ux-audit checklist.
4. One translation picker (FLOW-03): the search page's own `TranslationPicker`/`TranslationRow`
   (`web/src/routes/(application)/search/_components/`; flat, alphabetical, tick-then-Download
   two-step, indistinguishable duplicate rows) is replaced by the reader's picker grammar
   (language → translations, Feature 8 grouping with provenance badges); one "Add" action per row
   that downloads as needed with progress; empty result groups collapse to one line.
5. Reader drawer integration for the new search surface (D01 remaining) — lowest priority; ship after
   recents + ranking.
6. RTL-script translation search + Turkish `i`/`ı` folding; Arabic-script typo tolerance stays out
   unless bounded (document as known limit if not done).

**Acceptance**: recents persist + clear + pause; ranking demonstrably improves translation-hit
ordering vs mushaf order in a fixture test; search uses the reader's picker grammar; no regression
to the Arabic parity contract; `search-system.md` §5/§8 updated to reflect selection persistence +
shipped items.

**Dependencies**: none on Feature 6 — recents are local-only and ranking is client-side. Item 4 (one
picker) reuses Feature 8's grouping.

**Touch**: new store + palette/search page components (incl. `search/_components/TranslationPicker`),
`web/src/lib/quran/search/*`, `web/src/lib/search/**`, `web/messages/*`, `docs/search-system.md`.

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
5. Analytics: `locale_suggest_shown|accepted|dismissed` + `dismiss_reason` via the existing
   consent-gated GA4 `track()` (`web/src/lib/firebase/analytics.ts`, plan §5.5) — **not** Feature 6's
   first-party reading pipeline, so this feature has no dependency on Feature 6.
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

Migrations: `m000011` = bookmarks + bookmark_folders changes; `m000012` = the three reading-session
tables. Feature 6 and the optional `target_ayahs_per_day` column take the next free numbers when they
land.

**`bookmarks` (extend m000007, in `m000011`):**

- add `session_id TEXT NOT NULL DEFAULT ''`, `source_id TEXT NULL`,
  `end_surah INTEGER NOT NULL DEFAULT 0` + `end_ayah INTEGER NOT NULL DEFAULT 0` (backfilled to
  `surah`/`ayah`; single ayah = `end = start`), `deleted_at TEXT NULL`. **No `note` column** —
  notes stay local (§11).
- replace `idx_bookmarks_user_verse` with a **partial** unique index
  `(user_id, session_id, surah, ayah, end_surah, end_ayah) WHERE deleted_at IS NULL`. Sentinels
  (`''`, `end = start`) — never bare NULL (SQLite NULL-distinct in unique indexes). Partial so
  tombstones keep protecting their ids against stale replays while a newer live row takes the key.
- `list_for_user` filters `deleted_at IS NULL`.
- Delete = tombstone `UPDATE` (`deleted_at` = `updated_at` = the mutation's clamped `updatedAt`,
  §9.3), never row removal; refused when the stored row is strictly newer (LWW).
- `upsert_lww` changes: the newer-rival check (step 2) also counts **tombstoned** rivals on the key —
  a newer delete of that verse is the verse's latest state; the collapse step (3b) turns older live
  rivals into tombstones instead of `DELETE`, so their ids stay protected.

**`bookmark_folders` (in `m000011`):** add `kind TEXT NOT NULL DEFAULT 'generic'`,
`position INTEGER NULL`, `deleted_at TEXT NULL`; delete = tombstone that still detaches children
explicitly (the `ON DELETE SET NULL` FK never fires on an `UPDATE`); `resolve_folder_id` and
`list_for_user` filter `deleted_at IS NULL`.

**`reading_sessions`** (new, `m000012`): `id TEXT, user_id, name, kind TEXT,
translation_id TEXT NULL, created_at, updated_at, archived_at TEXT NULL, deleted_at TEXT NULL,
PRIMARY KEY(user_id, id)`.

**`reading_session_positions`** (new): `user_id, session_id TEXT, verse_key TEXT, surah INTEGER,
ayah INTEGER, local_page INTEGER NULL, ratio REAL NULL, source_id TEXT NULL, updated_at,
PRIMARY KEY(user_id, session_id)`.

**`reading_session_progress`** (new): `user_id, session_id, surah, furthest_ayah, updated_at,
PRIMARY KEY(user_id, session_id, surah)`.

- `user_id` leads every reading-session key: ids are client-minted, so a key without it lets one
  account write into another's row with a crafted id. Every `position.upsert`/`progress.upsert`
  first checks that a live (non-tombstoned) session with that id exists **for this user**; otherwise
  it is skipped and counted in `skipped` (EC-52).
- Session delete (tombstone): drop its position + progress rows in the same transaction; its
  session-scoped bookmarks re-home to global (`session_id = ''`), and one that would collide with a
  live global rival on the same key becomes a tombstone instead (EC-53).
- `user_id` FK `ON DELETE CASCADE` on every new table (account deletion purges them).

**Validation and caps** (all new string fields are bounded): session `name` 1..100 chars;
`kind` literal enum; `source_id`/`translation_id` ≤ 64 chars in the catalogue-id charset
`[a-z0-9._-]`; `ratio` 0..1; `local_page` ≥ 1; verses checked against per-surah ayah counts.
Per-user quotas (initial: 50 sessions, 10 000 live bookmarks); an over-quota upsert is skipped with a
surfaced error, never silently.

All ids are client-minted 36-char dashed-hex UUIDs (existing `newBookmarkEntityId` pattern and
`valid_client_uuid` validator; the UUIDv5 account default session id passes the same check). Every
query user-scoped.

### 9.2 Modules + client domains

- `POST /bookmark/v1/sync` — extended mutation fields; same envelope + verified guard + 100/60
  limiter; batch ≤ 200; same LWW helpers (`db/sea_models/bookmark/actions.rs` pattern). Adopt the
  in-flight R2-08 hardening: `expectedUserId` on the request, server rejects cookie-owner mismatch
  before reads or writes (`bookmark_v1/controller.rs` `sync`). It stays `Option` there only for
  tabs still running a pre-R2-08 build.
- New `reading_session_v1`: `POST /reading-session/v1/sync` — mutations `session.upsert`,
  `session.delete`, `position.upsert`, `progress.upsert`; response
  `{applied, skipped, sessions, positions, progress}` (full snapshot per user, same as bookmarks);
  `auth_guard::verified`; 100/60 limiter; batch ≤ 200; `expectedUserId` **required** (no legacy
  clients exist for this module).
- Client: `web/src/lib/sessions/{schema,store,domain}.svelte.ts` modeled on
  `web/src/lib/bookmarks/*`, per-owner outbox domain `sessions.user:{id}` (mirrors
  `bookmarksDomainFor`); register via `registerDomain`. `SyncPausedError` before any request when
  signed out or the owner changed (contract: `sync/types.ts:52-63`). An account-mismatch rejection
  pauses the domain; it never counts toward dead-lettering (§9.7).

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
  debounce 2s (`engine.svelte.ts:22-28`, `start()`).
- **Clock skew**: server validator normalizes incoming `updatedAt = min(updatedAt, server_now + 60s)`
  so a device with a fast clock cannot dominate LWW forever (EC-34). A **slow** clock is not
  corrected: its edits lose LWW against newer rows (e.g. a re-add older than a tombstone is skipped).
  Accepted LWW limit for v1; the skipped count surfaces in the sync result.
- **Ties**: snapshot queries gain secondary ordering `ORDER BY updated_at ASC, id ASC`; the client
  collapse uses the same deterministic rule (today tie order is undefined — FIX, EC-35).
- **Tombstones**: delete sets `deleted_at` (LWW compared on `updated_at`). A newer upsert clears the
  tombstone (intentional resurrection); an older upsert is skipped — this fixes the current
  stale-queued-upsert resurrection bug (no tombstones today; `delete_lww` hard-deletes). Tombstones
  older than 90 days are purged by a daily job on `services::scheduler` (`start_scheduler`,
  `main.rs`) or the hourly session-sweep task (`main.rs:765`) — **not** the rate-limit task's
  popularity flush, which exists only when demand collection is on (`main.rs:594`) and would leave
  the purge silently off. Accepted window: a device offline for more than 90 days can replay a stale
  upsert after its tombstone was purged and resurrect that row.

### 9.4 Merge on login — non-silent, reviewable

Replaces today's silent `#migrateLegacy` (`bookmarks/store.svelte.ts`, `:438` in the R2-08 tree).

- **Detection** after auth status → authenticated: mergeable local data exists if (a) legacy blob
  bookmarks non-empty AND marker absent, (b) the `local` owner's sessions (Feature 4.2) are
  non-empty, or (c) the legacy **unowned** outbox (pre-R2-08 `bookmarks` domain, EC-48) is
  non-empty. This account's own pending mutations are never a merge trigger — they auto-resume
  (§9.5).
- **UI**: `MergeReviewDialog.svelte` (bits-ui Dialog, `AuthModal.svelte` pattern — no AlertDialog
  primitive exists). Step 1 summary: "You have N bookmarks, M folders, K reading sessions on this
  device. Merge into your account?" Actions: **Merge** / **Review changes** / **Not now** /
  **Don't ask again**. Legacy unowned entries get their own line ("N changes from an earlier
  version of the app") with apply / discard — applying is the user's explicit choice of account.
- Step 2 review: tabs Bookmarks / Folders / Sessions; rows with include checkboxes; conflict labels
  ("Account has a newer version — account wins" / "Device version is newer — device wins"); counts of
  new vs conflicting vs duplicate.
- Merge executes: enqueue local items as upserts **with their existing timestamps** (LWW decides);
  flush; result toast "Merged X, skipped Y (newer account versions kept)" (ACCT-09 wording: promise
  "Keep your bookmarks and reading sessions safe and on all your devices" — notes stay local).
  - Legacy anonymous bookmarks have no timestamp (boolean map). Use `bookmarkSavedAt` when present
    (Feature 4.3), else the epoch `1970-01-01T00:00:00.000000Z`, so an account copy of the same
    verse always wins while new verses are still added (EC-10). Today's `#migrateLegacy` stamps
    `nowIso()`, so the device always wins — FIX.
  - The `local` default session (`LOCAL_DEFAULT_SESSION_ID`) is re-keyed to the account default
    session id UUIDv5(namespace, user id) before upload; positions LWW, progress MAX (EC-22).
    Other local sessions upload under their own UUIDs.
- **Not now**: nothing enqueued; marker `eq.merge.deferred.{userId}` in **sessionStorage**, so the
  prompt returns in the next browser session but not on every reload. **Don't ask again** sets
  `eq.merge.reviewed.{userId}` in localStorage. Markers are per-account.
- Failure handling: markers are written only after enqueues resolve (EC-11). The R2-08 working tree
  already does this for the legacy bookmark migration (`#migratedKeys.add` after a successful
  enqueue, marker after the loop); the merge dialog keeps that rule for every item kind.
- Legacy blob is never deleted (current behavior kept); the legacy boolean path migrates only through
  this review.

### 9.5 Logout, expiry, cache policy — **never destroy unsynced user data silently**

- **R2-08 basis**: adopt the in-flight account-isolation design — the durable outbox is partitioned by
  account (`bookmarks.user:{id}`, `web/src/lib/bookmarks/domain.ts:24`), transport/response/optimistic
  work is guarded by owner + generation, and the server rejects `expectedUserId` mismatches before
  reads or writes.
- **Quarantine = the account's own outbox partition**, left unregistered while that account is signed
  out. There is no separate quarantine key and no copying: R2-08's `#resetAccount` already stops
  clearing the queue on logout, so the partition simply waits in IndexedDB for its owner.
- **Legacy unowned entries** (pre-R2-08 `bookmarks` domain) — divergence, this doc wins: retained,
  **never discarded and never auto-attributed**. The session hint cannot name an owner (it stores
  `"1"`, `auth-state.svelte.ts:21,146`), and guessing would reopen exactly the R2-08 hole (A's edits
  uploaded with B's cookie). They surface once in the next login's merge review, where the user
  applies them to the signed-in account or discards them (EC-48).
- **Explicit logout**: account view cleared from memory (privacy — the next user on the browser sees
  nothing); pending unsynced mutations stay in the account's partition (never destroyed); `local`
  owner data (position, local sessions, anonymous notes) stays so offline reading continues;
  account sessions and notes are hidden until that account signs in again. If the partition has
  pending work, show a one-line notice on sign-out: "Your unsynced changes are kept on this device
  for your next sign-in" (ACCT-01 adapted — no copy-into-local prompt).
- **Session expiry / remote terminate / ban** (401/403 or probe → anonymous): same as logout for sync
  purposes + banner "Session expired — sign in to resume sync" (new copy). Wire the
  `current-session-terminated` transition — it is a dead kind today (no producer in `web/src`; the
  `purge-hook.ts` branch for it is unreachable). Detect via auth-client on 401/403
  (`AUTH_004`/`AUTH_009`) and probe; also clear the stale in-memory CSRF token and retry once (FIX:
  stale-CSRF wedge, EC-15).
  Note R2-04: sessions now have a 14-day **absolute** age on top of inactivity expiry — expiry is
  bounded and reachable; the banner must be ready for it.
- **Re-login same user**: quarantine auto-resumes (visible via `SyncIndicator` + toast). No merge
  dialog for your own pending mutations.
- **Re-login different user**: quarantine untouched and hidden; resumes when its owner logs in
  (EC-12/EC-17).
- **On-disk account data is partitioned by owner, never rendered for another owner.** Cached server
  snapshots (bookmarks, folders) stay memory-only. What does persist per account — outbox partitions,
  `easyquran.sessions.{userId}`, `easyquran.notes.{userId}` — is keyed by owner and only read when
  that owner is the signed-in account. The `local` owner holds signed-out data.
- **Outbox schema upgrades preserve rows.** `idbQueueStorage`'s `onupgradeneeded` deletes the store on
  any `SYNC_DB_VERSION` bump (`web/src/lib/sync/outbox.ts:176-179`); any bump this batch needs (dead
  letters, partitions) must copy rows forward instead (FIX, EC-55).
- **Reading position on logout**: change the current purge behavior (`clearReadingPosition` today wipes
  position/recents/progress on explicit logout but NOT on expiry — inconsistent). New policy: keep
  device-local reader data on both; only the account view is hidden (EC-14/EC-16).
- **Notes**: local-only notes currently survive logout and leak to the next account (privacy bug).
  Notes move to per-owner keys (Feature 4.3) so the next account never sees them (FIX, EC-17).

### 9.6 Multi-tab

- **Auth cross-tab**: add a `storage` listener on `eq.auth.session-hint` → other tabs re-probe and
  fire transitions (none today — FIX, EC-31). The hint is `"1"` for every account today, so an
  account switch rewrites `"1"` → `"1"` and fires no `storage` event; write a fresh per-login nonce
  instead (not the user id) so every sign-in and sign-out changes the value.
- **Overlay retire cross-tab**: BroadcastChannel `easyquran-sync`, messages
  `{domain, drainedIds, userId}` → other tabs retire optimistic overlays (the cross-tab gap the old
  DEFERRED note in `store.svelte.ts` described; R2-08 dropped the note, not the gap). Payloads are
  ids only, never user content.
- **Shared outbox**: two tabs may take the same batch and push concurrently → server LWW is idempotent
  (safe, EC-32); cross-tab retire removes the double-overlay flicker.
- Engines keep independent retry state per tab.

### 9.7 Audit fixes this design must land (each has an EC scenario)

**Already fixed by R2-08 in the working tree** — land as regression tests, no new code:

- Per-user gating: `applyServer`/enqueue callbacks now gate on owner + generation (`#isCurrent`), not
  a boolean `authed` (EC-17, sync half; the notes half is still open below).
- Outbox no longer destroyed on logout: per-owner partitions, `#resetAccount` does not clear the
  queue (EC-14). EC-15 still needs the expiry banner + CSRF retry.
- Legacy-migration marker written only after enqueues resolve (EC-11).
- Coalesced flush with a different domain filter now widens to all domains (`engine.flush`) (EC-44).
- Engine with no registered domains makes no network calls (`unregister` → no targets); its
  window listeners and intervals still run — tearing those down is the remaining EC-30 work.

**Still open — this batch fixes:**

- Legacy unowned outbox entries: R2-08 discards them on hydrate; swap for retain-and-review before
  R2-08 lands (EC-48, §0).
- No tombstones (`delete_lww` hard-deletes) (FIX, EC-02); tombstone model above.
- Pending-delete matching is id-scoped, not key-scoped — a same-key rival from a stale snapshot
  re-marks the verse (FIX, EC-40).
- Enqueue rejection is silent (overlay retired, nothing applied, no error;
  `BookmarksStore.#enqueue` rejection handler) (FIX, EC-27).
- Pull-only round failure with an empty queue schedules no backoff retry (`#scheduleRetry` returns
  when nothing is pending). The 30s periodic round still re-pulls, so this is latency, not loss
  (FIX, EC-28).
- Corrupt outbox rows are immortal (skipped by decode, never erased) (FIX, EC-26).
- No IDB→memory fallback when `indexedDB.open` fails at runtime (FIX, EC-25).
- Outbox store deleted on `SYNC_DB_VERSION` bump (FIX, EC-55).
- Poison mutation can head-of-line-block a domain forever (no failure cap) → dead-letter after N
  failures with a surfaced error (FIX, EC-29). Count **per-mutation server rejections only**
  (validation 4xx on that batch) — never the engine's per-domain `#failures`, which also grows on
  every offline round, and never `SyncPausedError` or an account-mismatch rejection.
- `SyncPausedError` can clear a real `lastError`; paused + no failures → phase "idle" (FIX, EC-43).
- Snapshot tie order undefined (`ORDER BY updated_at` only) (FIX, EC-35).
- Legacy anonymous bookmarks migrate with `nowIso()` and always beat the account (FIX, EC-10).

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
fixes. FIXED BY R2-08 = already fixed in the working tree; the scenario still lands as a regression
test. Test targets: Rust integration (`rust/backend/api/tests/*.rs`), web unit
(`web/src/lib/**/__tests__/*.test.ts`), manual QA.

### A. Multi-device conflicts

**EC-01 · Same bookmark edited on two devices**
Given devices A+B edit bookmark X (folder/source) offline. When both sync. Then newer `updatedAt`
wins the whole row; older push is skipped; both devices converge on the next pull.
Test: extend `bookmarks_v1.rs` + `store.test.ts`.

**EC-02 · Delete vs stale offline upsert (FIX: tombstone)**
Given A deletes bookmark X (tombstone), B offline later replays a stale upsert for X. When B syncs.
Then upsert's `updatedAt` < `deleted_at` → skipped; X stays deleted; no resurrection. A stale upsert
of a _different_ id Y on the same key is skipped too (the newer tombstone is the key's latest
state). Accepted window: after the 90-day purge a replay can resurrect (§9.3).
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
deleted; the tombstone detached F's existing children explicitly (no FK `SET NULL` on an `UPDATE`);
X's folder upsert resolves to root because `resolve_folder_id` treats tombstoned folders as missing —
no orphan, no FK trip. Test: `bookmarks_v1.rs` (extends existing same-batch case).

### B. Merge on login

**EC-08 · Offline → login shows merge review**
Given anonymous local bookmarks/folders/sessions exist. When the user logs in. Then
`MergeReviewDialog` appears with accurate counts; nothing is enqueued before an explicit choice;
Merge → items sync; result toast shows applied/skipped. Test: `store.test.ts` + manual QA.

**EC-09 · Not now / Don't ask again**
Given the dialog is open. When "Not now" → nothing enqueued, marker `eq.merge.deferred.{userId}`, no
re-prompt until next app start. When "Don't ask again" → `eq.merge.reviewed.{userId}`, never
re-prompted. Local data stays intact either way. Test: `store.test.ts`.

**EC-10 · Legacy blob + account rows (FIX: `nowIso()` stamp)**
Given legacy blob bookmarks exist (no timestamp) and the account already has the same verse. When
merge review runs. Then the legacy item carries `bookmarkSavedAt` or the epoch timestamp, the review
labels the conflict ("account wins"), merge skips it (LWW), new verses are still added, and the
legacy blob remains untouched. Today `#migrateLegacy` stamps `nowIso()`, so the device always wins.
Test: `store.test.ts`.

**EC-11 · Merge network failure (FIXED BY R2-08 for the legacy migration)**
Given merge enqueues fail midway (network). When the user retries later. Then no marker was written,
no item was silently dropped (mark only after enqueues resolve), retry presents the same items. The
working tree already does this in `#migrateLegacy`; the merge dialog must keep it for every kind.
Test: `store.test.ts`.

**EC-12 · Login as a different account with another user's quarantine**
Given user A logged out with pending mutations in partition `bookmarks.user:{A}`; user B logs in.
When sync runs. Then A's partition is not registered, not flushed, not shown, not merged into B; it
resumes when A logs back in. Test: `store.test.ts`.

**EC-13 · Review shows both conflict directions**
Given device item newer than account item (and vice versa). When the review step renders. Then labels
show "device wins" / "account wins" per row, matching server LWW outcome after merge. Test:
component test / manual QA.

### C. Logout, expiry, cache

**EC-14 · Explicit logout keeps unsynced data (FIXED BY R2-08: engine.clear loss)**
Given pending unsynced mutations. When the user logs out. Then account view is cleared, mutations
stay in the account's outbox partition, device-local reader data remains, re-login resumes the
queue, and the sign-out notice is shown (notice still to build). Nothing is destroyed. Test:
`store.test.ts` + `engine.test.ts`.

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
Then B sees none of A's synced bookmarks/folders/sessions/notes. Owner + generation gating (FIXED BY
R2-08) prevents A's in-flight rounds landing in B's view; still open: notes and sessions live under
per-owner keys and are only read for their owner. Test: `store.test.ts` + `sessions/store.test.ts`.

**EC-18 · Re-login same user auto-resumes quarantine**
Given quarantined mutations for user A. When A logs back in. Then the queue flushes automatically with
SyncIndicator + toast; no merge dialog for own pending mutations. Test: `store.test.ts`.

**EC-19 · Banned / revoked account mid-session**
Given a signed-in user who gets banned (`403` AUTH_006 AccountLocked) or whose session is revoked.
When the client probes or calls a guarded route. Then signed-out state + the expiry/locked banner;
the account's outbox partition is kept. Out of scope here: the **login form** maps AUTH_006 to the
generic credential message on purpose (`auth-copy.test.ts` "no separate locked kind" — avoids
revealing that an account exists); changing that is §11.11, not a bug fix.
Test: `auth-state.test.ts` + rust ban tests.

### D. Identity

**EC-20 · Two devices create separate sessions**
Given A creates session "Daily" offline, B creates "Weekly" offline. When both sync. Then two distinct
UUIDs → two sessions, no collision, no merge dialog. Test: `reading_session_v1` tests.

**EC-21 · Same session id on two devices**
Given both devices hold session S (synced earlier) and both edit it offline. When both sync. Then
per-row LWW converges; no duplication, no crash; snapshot shows one S. Test: `reading_session_v1`
tests + `sessions/store.test.ts`.

**EC-22 · Deterministic default session id**
Given legacy reader blobs migrated (signed out or before auth resolved) on two devices, each into the
`local` default session `LOCAL_DEFAULT_SESSION_ID`. When the same account signs in on both. Then
merge re-keys each local default session to UUIDv5(fixed namespace, user id) → one account default
session, not two; positions LWW, progress MAX; the constant local id is never uploaded. Test:
`sessions/schema.test.ts` + `sessions/store.test.ts`.

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
Given queue empty, pull returns 500. When the round fails. Then a backoff retry is scheduled (today
`#scheduleRetry` returns when no registered domain has pending work, so only the 30s periodic round
re-pulls — latency, not loss). Test: `engine.test.ts`.

**EC-29 · Poison mutation (FIX: dead-letter)**
Given a mutation the server always rejects (malformed beyond validation — e.g. foreign id handling
regression). When N attempts fail. Then it is moved to a dead-letter store with a surfaced error;
the rest of the queue keeps draining (no head-of-line block). Offline rounds, `SyncPausedError` and
account-mismatch rejections never count toward N — a queue that was offline for a week is not
dead-lettered. Test: `engine.test.ts`.

**EC-30 · Engine teardown on logout (partly FIXED BY R2-08)**
Given logout. When timers keep firing. Then no network calls while signed out (already true:
`unregister` leaves no targets) and `phase` stays idle; remaining fix: tear down the window listeners
and intervals while no domain is registered, restart them on the next `register`. Test:
`engine.test.ts`.

### F. Cross-tab

**EC-31 · Logout propagates across tabs (FIX: no auth cross-tab)**
Given two tabs open, A logs out (or switches account). When B receives the storage event on
`eq.auth.session-hint`. Then B transitions anonymous (or re-probes the new account), clears the
account view, leaves A's pending work in A's partition. An account switch must change the hint value
(per-login nonce) — a `"1"` → `"1"` rewrite fires no event. Test: `auth-state.test.ts` + manual QA.

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
Given a bookmark with session, source, and range. When it round-trips
client→server→snapshot→decode. Then all fields survive (wire `null` end ⇄ stored `end = start`);
malformed rows are dropped individually (existing decoder behavior); no note ever leaves the device.
Test: `schema.test.ts` + `bookmarks_v1.rs`.

**EC-38 · Saved translation no longer in catalogue**
Given `bookmark.sourceId` references an id absent from `TRANSLATIONS`. When the bookmark is opened.
Then the href falls back to the Arabic context (`surahRouteContext("uthmani")`), no crash, no broken
link. Test: `bookmarks/+page` component test / manual QA.

**EC-39 · Range span cap**
Given a range bookmark with span > cap, an end before its start, or an endpoint past its surah's ayah
count (e.g. `1:8`). When validated. Then the round is rejected with a clear error (server-side,
all-mutations-validated-first behavior preserved). Test: `bookmarks_v1.rs`.

**EC-40 · Pending delete vs stale same-key rival (FIX: id-scoped delete)**
Given an in-flight delete for verse V (session S) and a stale snapshot containing a rival row for V.
When the snapshot applies. Then the pending delete is matched by the full key
`(session, surah, ayah, endSurah, endAyah)`, not id alone; the verse does not re-mark. Test:
`store.test.ts`.

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

**EC-44 · Coalesced flush with domain filter (FIXED BY R2-08)**
Given `flush("a")` in flight and `flush("b")` called. When the drain loops. Then both domains drain —
the working tree widens a mismatched coalesced filter to all domains. Keep the regression test
before the sessions/analytics domains land. Test: `engine.test.ts`.

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
Given outbox entries in the pre-partition `bookmarks` domain (no owner). When the new build boots and
the next login runs. Then the entries are still there (no discard on hydrate), nothing is flushed or
auto-attributed, and the merge review lists them once; the user applies them to the signed-in account
or discards them. R2-08 discards on hydrate; this doc wins, and the swap lands inside R2-08.
Test: `store.test.ts` + `outbox.test.ts`.

**EC-49 · Locale suggest dismissal (Feature 10)**
Given the locale-suggest dialog shown for locale `ur`. When the user dismisses. Then
`easyquran.localeSuggest.resolved.ur = "dismissed"` persists across reload and tabs; forced-close
writes nothing; no redirect ever fires; re-trigger only for a different, still-unsuggested locale.
Test: `locale-suggest.test.ts` + manual QA.

### J. Added by the 2026-10-11 audit

**EC-50 · Tafsir never in the verse position (TRX-01)**
Given `ar.muyassar` or `ar.jalalayn`. When the user opens the picker, lands on a reader route with
those translation segments (e.g. `/al-baqarah/t/ar/muyassar`), or stacks it. Then it is offered only under "Commentary (tafsir)", the reader route 308s to the Arabic
reader with the tafsir stacked, and every tafsir block is labelled commentary in the UI font.
Test: `TranslationModal`/`VerseRow` component tests + `reader-route.test.ts`.

**EC-51 · Range and single-ayah bookmarks on the same verse**
Given a single-ayah bookmark on 2:285 in session S. When the user bookmarks the snippet 2:285–286 in
S. Then both rows exist (the range end is part of the unique key); neither verse-collapses the other.
Test: `bookmarks_v1.rs` + `store.test.ts`.

**EC-52 · Crafted session id from another account**
Given user B knows user A's session id S. When B pushes `position.upsert`/`progress.upsert` for S or
`session.upsert` with id S. Then nothing of A's changes: positions/progress are skipped (no live
session S for B), and B's `session.upsert` creates B's own `(B, S)` row. Test:
`reading_session_v1` tests.

**EC-53 · Session delete re-homes scoped bookmarks**
Given session S with scoped bookmarks, one of which shares its key with a live global bookmark. When S
is deleted. Then S's positions/progress are dropped, its scoped bookmarks become global, and the
colliding one becomes a tombstone (the global row stays). Test: `reading_session_v1` +
`bookmarks_v1.rs`.

**EC-54 · Previous-build tab across a deploy**
Given one tab on the new build (sessions under `easyquran.sessions.{owner}`) and one still on the
v4 build. When the old tab writes the reader blob. Then no session data is lost (it never lived in
the blob); at most `bookmarkSavedAt` entries the old tab drops sort as legacy. Test:
`reader-persistence.test.ts` + `sessions/store.test.ts`.

**EC-55 · Outbox schema upgrade preserves queued mutations (FIX)**
Given queued mutations under `SYNC_DB_VERSION` n. When the build bumps to n+1. Then every row survives
the upgrade (copied forward, not `deleteObjectStore`). Test: `outbox.test.ts` (needs an IndexedDB
test double — the suite has none today).

---

## 11. Open decisions (ask the user before building these)

1. ~~Bookmark uniqueness~~ **Resolved (§9.1)**: partial unique
   `(user_id, session_id, surah, ayah, end_surah, end_ayah) WHERE deleted_at IS NULL`, sentinels
   `''` (global) and `end = start` (single ayah).
2. ~~Reading sessions sync~~ **Resolved (§9.2)**: sessions + positions + progress sync; recents stay
   device-local.
3. Widget layout: local-only (`easyquran.home`) or synced (wire `settings-document.ts` + backend)?
   Cost input: `settings-system.md` §7 — prefs sync currently has no storage, endpoint, or conflict
   policy.
4. Analytics: GA consent default (GA currently defaults analytics ON) and granularity sent (surah
   only vs surah+ayah)? Includes SCR-05/SET-04: scrub GA paths vs opt-in. Feature 6's first-party
   reading history is **opt-in regardless** (Art. 9 data) — not part of this decision.
5. Range permalink grammar + span cap (Feature 7.5 / EC-39).
6. Azkar route name/scope: `/azkar` vs `/duas`; editorial governance for the curated list; translated
   detail URL shape (`/t/{lang}/{translator}/azkar/{slug}` joining the reader grammar + disk cache,
   vs Arabic-only snippet pages that link out to translated range readers).
7. Folder `kind:"daily"` semantics: name-only label vs real goal metadata (`targetAyahsPerDay`) +
   progress bar.
8. Accepted losses to confirm: session row-level LWW (EC-06) and verse-collapse context loss (EC-04) —
   field-level merge is out of scope; confirm acceptable.
9. ~~Locale-suggest Phase 2 catalogue flag~~ **Resolved (§14)**: superseded — derive-only, no bake
   pipeline change.
10. ~~Search recents/ranking~~ **Resolved**: bundled as Feature 9.
11. Login copy for locked accounts: keep AUTH_006 → generic credential message (current, tested,
    hides whether an account exists) or show specific "account locked" copy? Default: keep.
12. Per-user quotas (§9.1): initial 50 sessions / 10 000 live bookmarks — confirm or adjust.
13. ~~Notes sync~~ **Decided 2026-10-11 (default; revisit if wanted)**: notes stay local — no
    `bookmarks.note` column, notes keyed per owner on device. Matches the ACCT-09/C01 copy promise
    ("bookmarks + sessions sync; notes local") and keeps private reflections off the server.
14. ~~R2-08 legacy discard~~ **Decided 2026-10-11 (default; revisit if wanted)**: swap the discard
    for retain-and-review inside R2-08 before it lands (§0, §9.5, EC-48).

## 12. Suggested sequencing

0. **R2-08 lands** with the discard → retain-and-review swap (§0, EC-48) and the regression tests
   for what it already fixes (EC-11, 14, 17 sync half, 44, 30 network half).
1. **Feature 1 + 8** — translation UX, no schema risk (absorbed TRX items incl. TRX-01 + EC-45,
   EC-50).
2. **Feature 3 + 4 + §9.1–9.3** — `m000011` (bookmark context, range key, tombstones, folder columns)
   - `m000012` (reading-session tables), `reading_session_v1` backend, local sessions under
     `easyquran.sessions.{owner}`, reader blob v5 (translationFamily, `bookmarkSavedAt`), notes per
     owner, range readers call `markRead` (EC-42). The client sessions domain registers in step 3 —
     uploading local sessions needs the merge review. (Keep the existing bookmarks migration until
     step 3 lands.)
3. **§9.4–9.8 + §10 FIX items** — merge review (non-silent), sessions domain registration, cross-tab
   auth + overlay retire, copy pass (C01/C02/C06/C07/C08 + merge/session copy), and every open FIX
   scenario (EC-02, 10, 15, 17 notes half, 25–29, 30 listener half, 35, 40, 43, 55).
4. **Feature 2** — folders enrich on top (kind/position/bulk + absorbed UI items; no migration
   unless §11.7 adds `target_ayahs_per_day`).
5. **Feature 7** — snippets; populates the range columns from step 2 (reserved-segment machinery
   first; FLOW-05 share links).
6. **Feature 5** — widgets; consumes sessions (design-system constraints; HOME-01/02/04; BM-01/03).
7. **Feature 6** — analytics (next free migration number); consumes session emit points
   (SCR-05/SET-04 included).

**Independent** — no dependency on the steps above, slot anywhere: **Feature 9** (search recents +
ranking, local-only; its one-picker item 9.4 reuses Feature 8 grouping, so that item follows step 1)
and **Feature 10** (locale suggest Phase 1; events via the existing GA4 `track()`).

Each phase ends with all three gates green and its own commit(s) on the current branch. Every phase
lands its EC scenarios as tests (Rust integration + web unit) in the same commit.

---

## 13. Absorbed work map

| Source                                                                                             | Items                                                                                                          | Absorbed into                                                                                      |
| -------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `research/ux-audit`                                                                                | BM-01, BM-02, BM-03, BM-04, BM-05                                                                              | Features 2, 3, 5, §9.8                                                                             |
|                                                                                                    | ACCT-01, ACCT-03, ACCT-04, ACCT-05, ACCT-09                                                                    | §9.5, §9.8, Features 2, 3                                                                          |
|                                                                                                    | HOME-01, HOME-02, HOME-04                                                                                      | Feature 5                                                                                          |
|                                                                                                    | TRX-01, TRX-02, TRX-04, TRX-05, TRX-06, TRX-07, TRX-08, TRX-10, TRX-11, TRX-12, TRX-13, TRX-14, TRX-15, TRX-16 | Features 1, 4, 8, EC-45, EC-50                                                                     |
|                                                                                                    | TRX-03, TRX-09                                                                                                 | Superseded (§14)                                                                                   |
|                                                                                                    | FLOW-03, FLOW-05, FLOW-07, FLOW-08                                                                             | Features 9, 7, 4, 2                                                                                |
|                                                                                                    | FLOW-06                                                                                                        | Looks shipped — verify (Feature 9.3)                                                               |
|                                                                                                    | SCR-05, SET-04                                                                                                 | Feature 6                                                                                          |
|                                                                                                    | Guardrails: catalogue duplicate `(language, translator)` test; round-trip persistence test                     | Feature 8 acceptance, §10                                                                          |
|                                                                                                    | ACCT-07, ACCT-08, HOME-03, FLOW-01, FLOW-02, FLOW-04                                                           | **Not absorbed** — stay in the ux-audit checklist (EC-19 touches only mid-session ban copy)        |
| `plan/09-locale-suggest.md`                                                                        | Phase 1 (all)                                                                                                  | Feature 10                                                                                         |
|                                                                                                    | Phase 2 + open q2 (catalogue flag)                                                                             | Superseded (§14)                                                                                   |
| `settings-system.md`                                                                               | §4 translationFamily persistence; §7 prefs-sync gap                                                            | Feature 4.3, §11.3                                                                                 |
| `search-system.md`                                                                                 | recent queries, relevance ranking, drawer integration, §8 known limits; selection-persistence doc-sync gap     | Feature 9                                                                                          |
| `remaining/copy-corrections.md`                                                                    | C01, C02, C06, C07, C08                                                                                        | §9.8 copy pass                                                                                     |
| `security-audit-2026-10-11.md`                                                                     | deploy validation, recent-auth, Argon2 semaphore, CI gates                                                     | **Not product features** — tracked in the audit; only auth-lifecycle items referenced (§9.5, §9.7) |
| `security-audit-rounds-2026-10-11.md`                                                              | R2-08 account isolation (adopted + retain-not-discard divergence), R2-04 absolute age                          | §0, §9.2, §9.5, §9.7, EC-48                                                                        |
| `research/translation-empty-verses.md`                                                             | 4 blank verses + display decision                                                                              | Feature 1.8, EC-45                                                                                 |
| `research/navigation-audit.md` + `url-scheme-a.md`                                                 | path-only parser, `?v=` scope, fragment single-ayah, reserved segments, nav-guard regex                        | Feature 7 constraints                                                                              |
| `design-system.md`                                                                                 | §6/§13/§14/§34/§61 constraints; Card/MetricCard unused + hue cap                                               | Feature 5                                                                                          |
| `reader-virtualization.md`                                                                         | reading mode has no verse actions                                                                              | EC-47                                                                                              |
| `plan/README.md`                                                                                   | 10 owner items (Section.svelte delete, CI workflow, dead tokens, accents)                                      | **Not absorbed** — separate small backlog                                                          |
| `plans/remove-primary-translation.md`                                                              | done (`502effc2`)                                                                                              | Archive; "never primary wording" noted in Feature 1.6                                              |
| `research/mushaf-rendering-research.md`, `indopak-*`, `qurancom-mirror.md`, `design-enhancements/` | orthogonal projects/proposals                                                                                  | **Not absorbed** — separate tracks                                                                 |

## 14. Superseded directions (this doc wins)

1. **ux-audit TRX-03** (build-time dedupe by `(language, translator)` + alias ids) → superseded by
   Feature 8 runtime grouping. Alias-id machinery is not needed; no catalogue rewrite.
2. **ux-audit TRX-09** (readable QUL slugs `/t/en/haleem`) → superseded by Feature 8.4; URL grammar
   unchanged.
3. **ux-audit ACCT-01** (copy bookmarks into the local store on sign-out) → superseded by §9.5
   quarantine + sign-out notice.
4. **security rounds R2-08 "discard legacy unowned entries"** → superseded by §9.5/EC-48: retain and
   surface for user review, never discard and never auto-attribute; the swap lands inside R2-08
   before it ships. The rest of R2-08 (partition, `expectedUserId`, owner/generation guards) is
   adopted.
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
