# Primary translation removal

Product decision (final): remove the ENTIRE "primary translation" concept — nothing primary-related survives in behavior, code, copy, tests, or persisted state. Motivating bug: the primary-translation mechanism could REPLACE THE ARABIC TEXT in the ayah-by-ayah (verse) render mode.

All line refs verified against the committed tree at `98445ccb`/`e8a5be87` (working tree clean at planning time; only untracked `.claude/launch.json`, `.v2c/`, `.video_agent/`). Planning was read-only: file reads, greps, `git show` only — no gates were run (see Verification).

Out of scope (never touch): theme tokens (`bg-primary`, `text-primary`, `border-primary`, `primary-foreground`, `--primary`, Button variant "primary"), `nav_primary_label` / `reader_primary_nav` (primary _navigation_), `web/src/routes/design/_variants/mix/TranslationsDialog.svelte` (prototype; "primary" hits there are theme tokens only), the SurahReader lead/arabicCompanion architecture (it IS the bug fix), reader-core/persistence/session `sourceId` fields (resume/recents context), `readingText`/`ReadPick`/`readingFlowId`, `translationIdFromSegments`, `Sidebar.svelte:38` "soft primary fill" comment (theme prose).

## Bug root cause

Mechanism: the modal's per-row switch affordance was a trailing icon-only link `<a data-switch href={publicHref(readerHrefFor(copy.locale, href))} onclick={() => onPrimary(t)}>` — `web/src/routes/(application)/_reader/TranslationModal.svelte:680-695`; `href` from `rowHref` (`TranslationModal.svelte:312-314`) → `hrefFor` (`web/src/routes/(application)/_reader/translation-nav.ts:131-139`), building the `/t/{lang}/{translator}` route at the same reader position. `onPrimary` (`TranslationModal.svelte:316-320`) persisted the pick via `readerSource.setSourceId` + `noteTranslationChosen` and closed the modal, while the browser followed the `href`.

Pre-fix render (verified via `git show 6d8c3cc1^:...app/_reader/SurahReader.svelte`, line 1004): `VerseRow` was called with `text={bodyText(ayah.text...)}` and NO `isTranslation` prop, and VerseRow's fallback `translationActive = isTranslation ?? ("lang" in page.params && "translator" in page.params)` (still at `web/src/routes/(application)/_reader/VerseRow.svelte:66-68`) flipped true on `/t/**` — the route translation became the row's MAIN text: **Arabic replaced in ayah-by-ayah**.

Fix already shipped for surah routes in `6d8c3cc1`: `createArabicCompanion` (`SurahReader.svelte:403-410`) + `rowView` branch 1 (`:430-449`) keeps Arabic as row text and demotes the route translation to the lead lane, prepended by `lanesFor` (`:473-476`).

RESIDUAL LIVE PATH (not fixed, out of scope for this removal): `RangeReader` (`web/src/routes/(application)/_reader/RangeReader.svelte:282-292`) still renders `VerseRow text={bodyText(a.text...)}` with no `isTranslation` and no Arabic companion (`createArabicCompanion` imported only by `SurahReader.svelte:59,:403`), so `/t/lang/translator/{page,juz,hizb,rub}/N` verse rows are translation-only today.

The persisted `readerSource` store never feeds render (grep: sole read `StorageSection.svelte:53`, sole write `TranslationModal.svelte:317`) — it was the concept's persisted shadow; the replacement came from the switch-link navigation landing verse mode on `/t/**` routes whose row text was the route translation.

## Decisions

1. **Modal switch links** — keep the trailing-control anatomy (commit `98445ccb`): every non-picking row keeps its trailing icon-only `<a data-switch>` with `href=rowHref` (position-preserving `/t/` route via the `surah*For(ctx,...)` family; nav-guard safe) + `data-sveltekit-preload-data`. `onclick` becomes a plain `open = false` close (no preventDefault, no store write, no prefetch call). Row names stay inside the toggle label (name tap = check). The close itself is not a new fix — `onPrimary` already closed the modal — but removing the `{#if href && !isPrimary}` gate newly gives the CURRENT route's own row a link, so the plain close (plus a regression-pin test) covers the same-URL case this change itself makes reachable.
2. **Accessible name for the icon-only link** — RENAME key `reader_translations_switch` → `reader_translations_go_to` (en "Go to this translation"; ar value "الانتقال إلى هذه الترجمة" unchanged — already navigation copy) and the reader-copy field `switchTo` → `goTo`. Deleting the name outright was rejected: the link's only child is an Icon → unnamed control (a11y regression, `--fail-on-warnings` risk) and every aria-label-based test finder would break. Tests keep finding links via aria-label containing the translator name — zero selector rewrites.
3. **noteTranslationChosen** — DELETE the export + its "explicit translation choice" tests. Sole production caller was `onPrimary` (grep-verified); the `/t/` route load already fetches the DB (worker `readSurah` onMiss → `ensureTranslation`). Its `docs/plan/09-locale-suggest.md:326` mention is rewritten in step 5.
4. **TranslationButton count badge** — KEEP the exclusion under the renamed prop `routeTranslationId` (route-derived, `ReaderShell.svelte:31-35`). Badge counts stacked texts not already on screen. Rationale: `?more=` hydration does not exclude the route id from the store and the stacked controller dedupes that id from render, so counting it would over-report by one. TranslationModal no longer receives the prop.
5. **readerSource store** — DELETE entirely (`reader-settings.svelte.ts:116-163`: class, export, `SOURCE_STORAGE_KEY`, `decodeSource`, `PersistedSource`) including the now-unused `browser` import at `:1`. No stub: after steps 2–4 nothing imports it; render never read it. `createReaderSettings` (`:27-114`, fonts/mode) stays.
6. **Stale localStorage key `easyquran.reader.source`** — one-shot `removeJSON("easyquran.reader.source")` in `web/src/routes/(application)/+layout.svelte` `onMount` (helper exported at `web/src/lib/storage/safe-storage.ts:23-28` via `index.ts:4`). The single intentional surviving occurrence of the literal — accounted for in residue check 3. Chosen over the engagement-seed site because the `legacySeeded` guard would skip already-seeded users; layout mount runs for every user every boot and no-ops once clean.
7. **StorageSection pinned set** — `inUseIds = new Set(stackedTranslations.ids)`. Honest consequence (code-verified): the layout pin effect (`+layout.svelte:150-157`) lacks the `isNonReaderAppRoute` guard its sibling effects have (`:105,:123`) and re-runs on route change, so on `/settings` it unpins the route translation (params gone) — today `readerSource.sourceId` was the only thing keeping that DB in the in-use set there. Post-removal, the current `/t/**` route's DB is deletable from the settings storage page ("Remove all translations" and per-row Remove) and re-downloads on return (worker onMiss → `ensureTranslation`). Accepted; no pin-effect guard added (separate behavior change).
8. **Rail auto-select anchor** — `anchorId` falls back from `readPick?.current` to `null` with rail fallback `languages[0]` — Arabic (rail rank 0, `TranslationModal.svelte:188-194`). Tests pin Arabic-first; every English-pane test gains an explicit rail click.
9. **Rows and chips uniform** — every non-picking row is a checkbox toggle row disabled only at the cap (`isFull && !checked`) and gets the trailing go-to link; chips render in stacked-store order with reorder arrows + remove on every chip; the isPrimary lock, spacer, static cover, badge chip (`{@const isPrimary}`, the `pe-3`/`pe-0.5` class ternary, the Tooltip), badge pill, and `primaryTip` tooltip all die.
10. **createStackedTranslations option** — renamed `routeSourceId` everywhere (including the orchestration-test fixture field), dedupe behavior KEPT — removing the filter would double-fetch/double-render the route translation as a stacked extra next to its lead lane on `/t/**` verse mode.
11. **Engagement seeding** — delete the `SOURCE_KEY` seed block; legacy users lose one seed signal (chosen-but-never-routed translation no longer counts one `sourceViews` entry toward the prefetch gate). `isEngaged` keeps the bookmarks/notes + `distinctDays`/`totalViews` paths.
12. **Copy** — `reader_stacked_primary_badge` and `reader_translations_primary_tip` deleted from both locales; `reader_translations_switch` renamed to `reader_translations_go_to` with navigation wording; `cap_note` reworded keeping `{max}` (check-i18n.mjs signature-compares placeholders per key). Paraglide + namespace barrels regenerated via `pnpm -C web i18n:check`.
13. **RangeReader residual** — OUT OF SCOPE: kept as-is apart from the `routeSourceId` rename. Porting `createArabicCompanion` to range routes is a separate render-mode feature decision (see Bug root cause).

## Ordered steps

Each boundary leaves the tree compiling (and tests green where noted).

### Step 1 — rename the route-dedup option (self-contained)

Files: `web/src/routes/(application)/_reader/stacked-translations.svelte.ts`, `SurahReader.svelte`, `RangeReader.svelte`, `__tests__/stacked-translations.orchestration.test.ts`

- `CreateStackedTranslationsOptions.primarySourceId` → `routeSourceId` (`stacked-translations.svelte.ts:32`); `sync()`'s local `primary` → `routeSourceId` (`:217-218`, filter stays).
- Call sites: `SurahReader.svelte:158` and `:393`, `RangeReader.svelte:67-71`.
- Test: fixture field `Inputs.primary` → `routeSourceId` (`:67`; same name as the option, consistent with the `routeKey` field beside it) and all `primarySourceId` refs (`:88,:182,:330` + 16 `primary: null` literals at `:115,:134,:156,:226,:242,:259,:283,:304,:351,:372,:394,:410,:425,:439,:455,:472`); reword `:199` title to "dedupes the route translation so it is never fetched as an extra".

### Step 2 — engagement decouple

Files: `web/src/lib/quran/engagement-state.ts`, `web/src/lib/quran/__tests__/engagement.test.ts`

- Delete `SOURCE_KEY` (`:21`) and the seed block in `seedFromReaderEvidence` (`:127-134`). Bookmarks/notes seed (`:113-126`) and route-driven `bumpReaderView` stay.
- Tests: drop `SOURCE_KEY` const (`:41`), `seedReaderState`'s `sourceId` param + write (`:122-143`); rewrite `:354-393` to bookmarks/notes-only qualification (drop `sourceViews[TRANSLATION]==1` asserts at `:364,:390` and the TanzilUthmani-seed case `:367-377`; keep `qualified==true` + fr.hamidullah asserts).

### Step 3 — StorageSection in-use set

Files: `web/src/routes/(application)/settings/_components/StorageSection.svelte`, `settings/__tests__/route-isolation.test.ts`

- `inUseIds = new Set(stackedTranslations.ids)` (`:52-54`); drop the `readerSource` import (`:16`, sole use was `:53` — noUnusedLocals forces removal).
- Test `:167-176`: rename case to "the in-use guard pins stacked translation ids", drop the `toContain("readerSource.sourceId")` assert (`:170`), keep `stackedTranslations.ids` / `inUse={inUseIds.has(artifact.id)}` / `disabled={inUse}` asserts.

### Step 4 — modal/button/shell + tests + noteTranslationChosen deletion

Files: `TranslationModal.svelte`, `TranslationButton.svelte`, `ReaderShell.svelte`, `__tests__/TranslationModal.test.ts`, `__tests__/TranslationModalHost.svelte`, `__tests__/surah-reader.test.ts`, `web/src/lib/quran/engagement.ts`, `web/src/lib/quran/__tests__/engagement.test.ts`, `translation-nav.ts`

`TranslationModal.svelte`:

- Delete `primaryId` prop (`:55,:59`), `readerSource` import (`:20`), `noteTranslationChosen` import (`:24`), `onPrimary` (`:316-320`).
- Rail anchor effect drops the `primaryId` term → `anchorId = readPick?.current ?? null` with rail fallback `languages[0]` (`:102-107`); reword comments `:100-101` and `:216-218`.
- `selectedRows` becomes store-order only (`:247-258`, drop primaryEntry push + skip); reword comment `:245-246`.
- `rowCoverClass` drops the `"static"` kind (`:306-311`); reword comment `:298-303`.
- Chip block: delete `{@const isPrimary = t.id === primaryId}` (`:449`); replace the class-array `isPrimary ? "pe-3" : "pe-0.5"` ternary (`:451-456`) with the flat `pe-0.5` string; delete the badge Tooltip (`:461-485`); make BOTH `{#if !isPrimary}` gates unconditional — reorder (`:489-519`) and remove (`:520-529`).
- Row snippet: delete `{@const isPrimary}` (`:564`) and set `disabled = !picking && (isFull && !checked)` (`:565`); delete spacer branch (`:612-614`), row badge pill (`:649-655`), static-cover branch (`:667-670`).
- Trailing link gate `:680` becomes `{#if href}`; `onclick` `:688` becomes `() => (open = false)`; KEEP `aria-label` (`:689`, still `` `${copy.translations.switchTo}: ${rowLabel(t)}` `` for now — renamed in step 6) and `title` (`:690`); reword comments `:575-579` and `:680-683`.

`TranslationButton.svelte`: rename prop `primaryId` → `routeTranslationId` (`:7`); `hiddenCount` keeps excluding it (`:12-14`); stop forwarding to the modal (`:92` → `<TranslationModal bind:open />`).

`ReaderShell.svelte`: rename derived `primaryId` → `routeTranslationId` (`:31-35`), pass `{routeTranslationId}` (`:56`).

`engagement.ts`: delete `noteTranslationChosen` (`:92-116`). `engagement.test.ts`: delete the "explicit translation choice" describe (`:542-557`). `translation-nav.ts`: reword comment `:95` ("position-preserving primary switches" → "position-preserving translation navigation").

### Step 5 — delete the store + key cleanup + comment/doc rewording

Files: `web/src/lib/stores/reader-settings.svelte.ts`, `web/src/routes/(application)/+layout.svelte`, `web/src/lib/stores/reading-text.svelte.ts`, `docs/plan/09-locale-suggest.md`

- `reader-settings.svelte.ts`: delete `:116-163` and prune ALL now-unused imports — `asObject/asString/isFutureSchema/onStorageKey/readJSON/writeJSON` AND `browser` from `$app/env` (`:1`; used only at `:139,:141-144,:159` inside the deleted block; `createReaderSettings` reads none of them).
- `+layout.svelte`: rename the pinned-set local `primary` → `routeTranslationId` (`:154-155`); add the one-shot cleanup inside `onMount` (`onMount` imported `:2`, instance at `:87`): `removeJSON("easyquran.reader.source")` from `#lib/storage/index.js`, with a comment marking it as removal of the retired primary-translation key. This file is the ONE intentional surviving occurrence of the literal — residue check 3 accounts for it.
- `reading-text.svelte.ts`: reword doc comment `:13-18` to drop the phrase "primary translation" (e.g. "...switching it does not navigate (quran.com's model); Reading flows exactly one text at a time").
- `docs/plan/09-locale-suggest.md` — BOTH residue sites (grep found exactly two): `:191` swaps the `easyquran.reader.source` key-naming example for a live key (`easyquran.reader.stacked`); `:326` ("Selection persists via `readerSource.setSourceId(id)` + `noteTranslationChosen(id)`") rewritten to the post-removal truth (selection persists via the stacked-translations store + the `?more=` URL param; the row link is pure navigation).

### Step 6 — copy, i18n keys, regeneration

See "Copy and messages" below.

### Step 7 — gates + residue sweep

Run the gates (see Verification), then every residue check below.

## Copy and messages

`web/messages/reader/en.json` / `web/messages/reader/ar.json` (both locales change together — check-i18n.mjs parity + per-key placeholder signature gates; `precheck`/`prelint`/`pretest` chain `i18n:check` per `web/package.json`):

- DELETE `reader_stacked_primary_badge` (`en.json:176` / `ar.json:176`).
- DELETE `reader_translations_primary_tip` (`:187` / `:187`).
- RENAME `reader_translations_switch` → `reader_translations_go_to` (`:191` / `:191`): en value "Go to this translation"; ar value unchanged "الانتقال إلى هذه الترجمة".
- REWORD `reader_translations_cap_note` (`:188` / `:188`) keeping `{max}`: en "Up to {max} translations."; ar "حتى {max} ترجمات."

`web/src/lib/i18n/reader-copy.ts`: drop imports `:193` (`reader_stacked_primary_badge`) and `:219` (`reader_translations_primary_tip`); rename import `:222` → `reader_translations_go_to`; interface `:416` (`stacked.primaryBadge`) and `:432` (`translations.primaryTip`) deleted, `:429` `switchTo` → `goTo`; wiring `:707` and `:726` deleted, `:720` → `goTo: noArgs(reader_translations_go_to)`.

`TranslationModal.svelte:689-690` rebind to `copy.translations.goTo` (icon-only link keeps its accessible name + title; aria-label keeps the `: rowLabel(t)` suffix so test finders still work).

Regenerate (never hand-edit generated files):

```
pnpm -C web i18n:check
```

(check-i18n.mjs && i18n:compile && i18n:namespaces — rewrites untracked `web/src/lib/paraglide/**` and the tracked barrel `web/src/lib/i18n/m/reader.ts`.)

Do NOT touch `nav_primary_label` / `reader_primary_nav` (primary navigation, excluded).

## Tests

`web/src/routes/(application)/_reader/__tests__/TranslationModal.test.ts`:

- Delete `readerSource` + `noteTranslationChosen` mocks (`:44,:56-61`) and `mockClear` (`:137`).
- `open()` drops `primaryId` plumbing (`:208-211`); all `open({primaryId})` call sites become `open()` — `:285,:294,:309,:329,:345,:364,:455,:470,:487,:527,:536,:577,:603,:642,:671,:702,:721,:742,:771`; Host mount `:547-555` drops the `primaryId` prop (`:550`); picker mount `:786-793` drops `primaryId:null` (`:790`).
- Rail now defaults to Arabic → every test querying English-pane content must first `railOption('English')?.click()` + `await settle()`: `:308-318`, `:344-361`, `:453-467`, `:469-483`, `:641-667`, `:720-728`, and the live-position tests `:521-539`/`:541-571` (Pickthall link lives in the English pane).
- `:284-291` → "defaults the rail to Arabic on open".
- `:293-306` → cover open-auto-select scroll via a picker mount with `readPick` current `qul.ur.bayan` (Urdu still anchors via readPick; the Arabic default is already visible so "nearest" would not scroll); keep the keyboard-move half.
- `:363-370` Primary-badge test DELETED → replaced by "every non-picking row is a toggleable checkbox, no Primary text anywhere" (with English click).
- `:453-467` → drop the `:456-459` "primary has no switch of its own" absence assert (every row now gets the trailing link); keep the Pickthall href assert and the modal-closed assert (`:466`); drop the `setSourceId` assert (`:463`); EXTEND: clicking the current-route row's own link (href == current URL) also closes the modal — regression pin for the newly-reachable same-URL case.
- `:469-483` → drop the `setSourceId`-not-called assert (`:481`); keep the rest.
- `:500-511` cap-note wording assert updated in step 6 to the new copy.
- `:575-599` → chips equal `['ur.jalandhry','ms.basmeih']` in store order; delete badge asserts `:584-589`; arrows+remove asserted on both chips.
- `:641-667` → keep label/checkbox/link-beside-label anatomy (`:645-658`); invert `:663-666` (en.sahih row now HAS `label[data-row-target]` + checkbox, NO `div[data-row-cover]`).
- `:700-718` → `:709` `toHaveLength(3)` → `2`.
- Badge describe at `:832-838` → reword title (`:833`, "shows the stacked count excluding the route translation") and rename the prop in ALL THREE TranslationButton mounts (`:835,:844,:850`) to `routeTranslationId`; assert "2" at `:837` unchanged (3 ids minus route en.sahih).

`TranslationModalHost.svelte`: drop the `primaryId` prop (`:9-14,:24`).

`surah-reader.test.ts`: delete `setSourceIdSpy` (`:33,:80,:257`), the `readerSource` module mock (`:119-121`), and the engagement mock (`:122-124`).

`engagement.test.ts`: delete the "explicit translation choice" describe (`:542-557`) + the step-2 seeding fixtures.

`stacked-translations.orchestration.test.ts`: the step-1 `routeSourceId` rename (`:67,:88,:182,:330` + 16 literals) and reworded `:199` title.

`route-isolation.test.ts` (settings): the step-3 rewrite (`:167-176`).

## Residue checks

All run from the repo root after implementation. Zero-hit checks must return 0.

1. `grep -rn 'readerSource\|ReaderSourceStore\|SOURCE_STORAGE_KEY\|decodeSource\|PersistedSource' web/src docs --include='*.ts' --include='*.svelte' --include='*.md'` → 0 (docs/plan/09-locale-suggest.md:326 rewritten in step 5).
2. `grep -rn 'noteTranslationChosen' web/src docs` → 0.
3. `grep -rn 'easyquran.reader.source' web/src web/messages docs | grep -v '(application)/+layout.svelte'` → 0, PLUS `grep -c 'easyquran.reader.source' 'web/src/routes/(application)/+layout.svelte'` → exactly 1 (the intentional one-shot `removeJSON` cleanup).
4. `grep -rn 'primaryId' web/src --include='*.ts' --include='*.svelte'` → 0.
5. `grep -rn 'primarySourceId' web/src --include='*.ts' --include='*.svelte'` → 0.
6. `grep -rn 'onPrimary' web/src --include='*.ts' --include='*.svelte'` → 0.
7. `grep -rn 'reader_stacked_primary_badge\|reader_translations_primary_tip\|reader_translations_switch' web/src web/messages` → 0 (`src/lib/paraglide/**` and `src/lib/i18n/m/**` regenerate away).
8. `grep -rn 'primaryBadge\|primaryTip\|switchTo' web/src --include='*.ts' --include='*.svelte' | grep -v 'src/lib/paraglide/' | grep -v 'src/lib/i18n/m/'` → 0.
9. Scoped semantic sweep (reader-copy.ts keeps `reader_primary_nav`/`primaryLabel` by decision and `Sidebar.svelte:38` keeps theme prose, both excluded; their dying surfaces are covered by checks 7–8): `grep -rniE 'primary'` over `TranslationModal.svelte`, `TranslationButton.svelte`, `ReaderShell.svelte`, `RangeReader.svelte`, `SurahReader.svelte`, `VerseRow.svelte`, `translation-nav.ts`, `stacked-translations.svelte.ts`, `reader-settings.svelte.ts`, `reading-text.svelte.ts` (all under `web/src/routes/(application)/_reader/` and `web/src/lib/stores/`), piped through `grep -vE 'bg-primary|text-primary|border-primary|primary-foreground|primary-hover|ring-primary|accent-primary|var\(--primary|--primary'` → 0 (requires every TranslationModal comment naming primary — `:100-101,:216-218,:245-246,:298-303,:575-579,:680-683` — reworded per step 4).
10. `grep -rn 'alongside the primary' web/messages`; `grep -rn 'إلى جانب الترجمة الأساسية' web/messages`; `grep -rn '"الأساسية"' web/messages/reader` → 0 (cap_note reworded, badge key deleted).
11. Keep-assertions (must stay NON-zero): `grep -rn 'nav_primary_label' web/src/lib/i18n web/messages | wc -l > 0` and `grep -rn 'reader_primary_nav' web/src/lib/i18n/reader-copy.ts web/messages/reader | wc -l > 0` — primary-navigation copy and theme tokens are the design system, not this concept.

## Verification

Run from `web/` after all steps land (implementer's post-edit gates — NOT run during planning; planning was read-only and running them on the unmodified tree would not exercise the plan):

```
pnpm check   # svelte-check --fail-on-warnings + worker tsc; precheck runs i18n:check
pnpm lint    # vp lint --deny-warnings
pnpm test    # vp test run
```

Then the residue checks above. Also verify the keep-set intact: `design/_variants/mix/TranslationsDialog.svelte` unchanged (verified at planning: zero primaryId/readerSource/switchTo hits; its "primary" matches are theme tokens), `nav_primary_label`/`reader_primary_nav` still present, SurahReader lead/arabicCompanion architecture unchanged (it IS the bug fix), reader-core/persistence/session `sourceId` fields unchanged, `Sidebar.svelte:38` comment kept.

Known accepted consequences to state in the change description: (a) previously-primary translation DBs become deletable from settings, including the active `/t/**` route's DB while on `/settings` (pin effect unguarded, see Decision 7) — re-downloads on return; (b) legacy users lose one engagement seed signal; (c) `/t/` range-route verse rows remain translation-only (pre-existing, flagged out of scope).
