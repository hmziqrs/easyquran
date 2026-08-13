# Multiple stacked translations in the ayah-by-ayah reader (quran.com-style)

> Status: design plan (not yet implemented). Owner-approved approach.
> Scope: web reader only. Server / SSR / disk cache intentionally untouched.
> This revision incorporates a 7-lens adversarial code audit (see "Audit corrections applied" appendix).

## Context

Today the reader shows **one** translation at a time, encoded as URL path segments
`/t/[lang]/[translator]`, SSR'd + 7-day disk-cached per `sourceId`. `VerseRow.svelte` renders
exactly one text (Arabic XOR translation). This plan adds quran.com-style **multiple translations
stacked under each Arabic ayah**, selectable via a picker.

**Constraint (owner-decided):** multi-translation is **client-side only**. The server, SSR, and
7-day disk cache do not change — stacking N translations server-side is intractable for caching.
Extras hydrate purely in the browser, layered on top of the untouched primary SSR route.

Reference: `docs/quran-system.md` (Hard rules, Web delivery, Part 3 divergences) and
`docs/my-plan-raw.md` (single-translation original intent — multi-translation is net-new).

## Decisions (locked)

1. **Load strategy:** client-side only. Primary path route + disk cache untouched. Extras fetch
   in-browser via the existing OPFS web-worker (`quranWorker.readRange`) after hydration.
2. **Caps:** OPFS disk = effectively unlimited (raise retention caps; all 115 DBs total < 200 MB).
   In-memory resident LRU = **`TRANSLATION_DB_CAP = STACKED_MAX_EXTRAS + 2 = 7`** = 5 extras +
   current primary + 1 stale-primary buffer (during a primary route transition the old primary
   lingers in `translationDbs` because the worker has no release/evict message, so a single hop
   transiently holds old + new primary + 5 extras = 7; 6 would evict an extra and flash it).
3. **Picker:** a dedicated drawer triggered from the **Sidebar** (so it is available on *every*
   reader route — surah, juz, global-page — not just `SurahReader`), quran.com style: search,
   language-grouped checkboxes, selected-with-reorder. Built on the existing `$lib/components/ui/sheet`
   primitive (bits-ui `Dialog` — handles focus-trap, initial focus, Escape, restore-focus). The
   existing sidebar `TranslationPicker` (chooses the **primary/path** translation) stays unchanged.

## Core design — "extras are a client concern, mirrored like `mode`"

Extras are a persisted client selection mirrored to a `?more=<id>,<id>` query param via `replaceState`
— **exactly the `mode` precedent** (`mode-param.ts` + the backstop `$effect` in `+layout.svelte`).
Like `mode`, every **user-initiated** mutation (the picker) calls `replaceState` itself; the layout
`$effect` is an untracked **backstop** for navigation / deep-link / back-forward only. Consequences:

- `surah*For(ctx,…)` nav helpers **unchanged** (query is not a path segment) → nav-guard stays green.
- Disk-cache key built from the parsed route (`quran-disk-cache.ts` + `hooks.server.ts`
  `translationRouteCacheKey`) never sees the query string → **no new disk-cache entries**.
- The SW must normalize the **PAGES (navigational HTML) cache** key as well as the DATA cache, or
  `?more`/`?mode` combos fork PAGES into unbounded entries and break offline deep-link reload
  (see §4 — this also fixes the pre-existing `?mode` PAGES fork).
- On internal nav the new URL lacks `?more` (helpers don't carry it) → the backstop `$effect`
  re-adds it from the store.

---

## Implementation

### 1. Data model — `web/src/lib/data/quran-types.ts` (append near `Ayah`)

```ts
export interface StackedTranslation {
  readonly sourceId: string;          // "en.sahih"
  readonly translator: string;        // TranslationCatalogueEntry.translator
  readonly language: string;          // .language (display name, for the visible label)
  readonly languageCode: string;      // .languageCode (BCP-47, for the HTML lang="" attribute)
  readonly direction: "rtl" | "ltr";
  readonly text: string;
}
export type StackedTranslationsByVerse = ReadonlyMap<VerseKey, readonly StackedTranslation[]>;
export type StackedSourceState = "loading" | "ready" | "error";
export interface StackedTranslationsState {
  readonly byVerse: StackedTranslationsByVerse;
  readonly order: readonly string[];                       // effective extra sourceIds, display order
  readonly status: ReadonlyMap<string, StackedSourceState>;
}
```

Join key = `ayah.key` (`${surah}:${ayah}`). The orchestration builds `byVerse` by indexing each
extra source's `range.ayahs` on `ayah.key`. The map is **rebuilt on surah/route identity change and
merged (additively) across page-window expansion within a surah** — never cleared on a page load
(clearing flashes every stacked row to a skeleton). Only the *selection* (which sourceIds) is persisted.

### 2. New store — `web/src/lib/stores/stacked-translations.svelte.ts`

Clone `ReaderSourceStore` (`reader-settings.svelte.ts`) exactly:

- localStorage key `easyquran.reader.stacked`, `STACKED_SCHEMA_VERSION = 1`, shape `{ v: number; ids: string[] }`.
- Decode with `asArray(obj.ids, asString)` (from `$lib/storage/decoders`), `isFutureSchema` guard, fallback `[]`.
- `#ids = $state<string[]>([])`; cross-tab via `onStorageKey` (last-writer-wins, same as `readerSource`);
  `dispose()`.
- API: `get ids()`, `setIds(ids)` (dedupe via `es-toolkit` `uniq`, cap at `STACKED_MAX_EXTRAS = 5`,
  preserve order, write-through `writeJSON`), `toggle(id)`, `reorder(id, -1|1)`, `remove(id)`, `clear()`.
- `setIds` is a **replace**, not a merge — so callers that adopt a URL must guard (see §4 onMount).
- Singleton `export const stackedTranslations = new StackedTranslationsStore()`.
- **No coupling to `readerSource`.** Dedupe vs the route's path sourceId happens at consume time (§6).

### 3. OPFS / worker constants (decision #2)

- `web/src/lib/workers/quran.worker.ts` — `TRANSLATION_DB_CAP = 4` → **`7`** (= `STACKED_MAX_EXTRAS + 2`:
  5 extras + current primary + 1 stale-primary buffer during a route hop; the worker has no
  `releaseSource`/evict message so the old primary lingers until LRU pressure). LRU logic
  (`evictTranslationDbs`, delete-on-access in `translationRunner`) is already correct — no other
  worker change. Inline-comment the +2 rationale. (Optional hardening: add a `releaseSource(sourceId)`
  WorkerRequest fired from the reader when `primarySourceId` changes, so the stale primary is dropped
  explicitly instead of lingering — only worth it if 7 is deemed too high.)
- `web/src/lib/workers/opfs-retention.ts` — `CAP_COUNT = 12` → **`512`**;
  `CAP_BYTES = 128*1024*1024` → **`768*1024*1024`**; `TTL_MS` stays 30d. With these caps
  `computeEvictions` is effectively **TTL-only in production** (only drops DBs unused for a month;
  `stampLastUsed` re-stamps on access).
- **Lockstep test:** `web/src/lib/workers/__tests__/opfs-retention.test.ts` re-declares local
  `TTL_MS/CAP_COUNT/CAP_BYTES` mirrors — update all three. The count-cap (line ~38) and byte-cap
  (line ~81) assertions already exercise the eviction logic at the new thresholds; **also scale the
  byte-pressure fixtures at lines ~60 (70 MB candidates) and ~117 (200 MB giant) past 768 MB**, or
  those specific cases fail. The synthetic-100 test (~line 146) becomes trivial (cosmetic; the cap
  logic stays guarded by lines 38/81). Since production is TTL-only, add a TTL-eviction case
  (stale > 30d evicted, fresh kept) as the primary retention guard.

### 4. URL `?more=` roundtrip

- New `web/src/lib/reader/more-param.ts` — mirror `mode-param.ts`: `READER_MORE_PARAM = "more"`,
  `parseMoreParam(url) → string[]` (csv split, `uniq`, junk filtered), `withMoreParam(url, ids, base?) → URL`
  (`set` if non-empty else `delete`), `moreParamMatches(url, ids)` (order-insensitive csv equality).
- **Cold deep-link adoption in `onMount`** (mirrors `reader.hydrate(parseModeParam(...))` at
  `+layout.svelte:39-40`). Extend the existing `onMount`:
  ```ts
  const moreIds = parseMoreParam(page.url);
  if (moreIds.length) stackedTranslations.setIds(moreIds); // adopt cold deep-link ?more
  ```
  The `if (moreIds.length)` guard is **mandatory**: `setIds` is a replace, so a blind
  `setIds(parseMoreParam(url))` would wipe the persisted selection on a cold *bare* URL. (Mode avoids
  this because `reader.hydrate`'s override is merge-style.)
- **Backstop `$effect`** in `routes/(application)/app/+layout.svelte` next to the `mode` mirror.
  Keep `untrack` (the picker syncs the URL itself — §7), and make adopt + sync **mutually exclusive**
  (`else if`) so a URL→store adoption never feeds a stale local into the store→URL write on the same pass
  (the cold-deeplink thrash). Note: `replaceState` does **not** reactively reassign `page.url` (SvelteKit
  `client.js`), so convergence is measured across real-navigation re-runs, not within one tick:
  ```ts
  $effect(() => {
    const url = page.url;
    const ids = untrack(() => stackedTranslations.ids);
    const parsed = parseMoreParam(url);
    if (parsed.length && !moreParamMatches(url, ids)) {
      stackedTranslations.setIds(parsed);            // adopt explicit ?more (deep-link / back-forward)
    } else if (!moreParamMatches(url, ids)) {
      replaceState(withMoreParam(url, ids), page.state); // store → URL (e.g. after internal nav dropped ?more)
    }
  });
  ```
  Trace: cold `?more=en.sahih` + empty LS → onMount sets store; effect adopt skipped (matches), sync
  skipped → bar stays `?more=en.sahih`. Internal nav to bare URL → effect re-adds `?more` from store.
  Cap-drop `?more=a..f` → onMount caps to 5, effect sync normalizes the bar to the capped 5.
- **Disk cache: no change** (key excludes query).
- **SW — normalize BOTH caches.** Add `more` to the strip-set in `normalizeDataKey` (`service-worker.ts`
  + mirror `lib/offline/keys.ts`) so the `__data.json` DATA-cache is shared. **Additionally**,
  `handleNavigation` must store/look up the PAGES cache by the **normalized** key, else every
  `?more`/`?mode` combo becomes a distinct PAGES entry (unbounded; crowds `PAGES_MAX=300`) and
  offline reload of a deep-link misses → 404 shell. In `handleNavigation`, compute
  `const key = normalizeDataKey(req.url)` once and use `new Request(key)` for `pages.put`, `pages.match`,
  and `pages.delete` (the exact pattern `handleData` already uses); keep `fetch(req,…)` using the raw
  request. This also collapses the **pre-existing** `?mode` PAGES fork. Server HTML is byte-identical
  across all such combos, so one cached variant per logical page is correct.

### 5. `VerseRow.svelte` + the two callers

Add three props (pre-filtered per-`vKey` by the caller; VerseRow never reaches into maps):
```ts
stacked?: readonly StackedTranslation[];     // ready texts for THIS ayah, store-order, primary excluded
stackedPending?: readonly string[];          // extra sourceIds status==="loading" for THIS ayah (skeletons)
stackedErrored?: readonly string[];          // extra sourceIds status==="error" for THIS ayah (error rows)
```
Render under the existing primary `{#if translationActive}…{/if}` block. **No nested ternary**
(lint-enforced) — build a descriptor array, then `{#each}`:
```ts
type ExtraRow =
  | { kind: "skeleton"; sourceId: string }
  | { kind: "error"; sourceId: string }
  | { kind: "text"; t: StackedTranslation };
const extraRows = $derived<ExtraRow[]>([
  ...(stackedPending ?? []).map((sourceId): ExtraRow => ({ kind: "skeleton", sourceId })),
  ...(stackedErrored ?? []).map((sourceId): ExtraRow => ({ kind: "error", sourceId })),
  ...(stacked ?? []).map((t): ExtraRow => ({ kind: "text", t })),
]);
```
```svelte
{#each extraRows as row (row.kind === "text" ? row.t.sourceId : row.sourceId)}
  {#if row.kind === "skeleton"}
    <div class="verse-extra skeleton" aria-hidden="true"></div>
  {:else if row.kind === "error"}
    <span class="verse-extra verse-extra--error" role="status">{copy.stackedError}</span>
  {:else}
    <span class="verse-extra" dir={row.t.direction === "rtl" ? "rtl" : "auto"} lang={row.t.languageCode}
          style="font-size:var(--reader-translation-size, 1.0625rem)">
      <span class="verse-extra-label">{row.t.translator || row.t.language}</span>{row.t.text}
    </span>
  {/if}
{/each}
```
- `lang={row.t.languageCode}` (BCP-47), **not** the display `language` — otherwise you emit
  `<span lang="English">`, which is invalid and breaks screen-reader pronunciation.
- `.verse-extra { display:block; border-top:1px solid var(--line); margin-top:.5rem; padding-top:.5rem; text-align:start }`.
  Reuses the existing **stub** `--reader-translation-size` var (wire a size control later).
- Reading-mode inline-flow CSS is already scoped to `[data-source-kind="arabic"]` → extras stay block.
  Extras render only in verse mode (hide in reading mode like `VerseTools`).
- **Loading/error a11y:** keep the per-ayah skeleton `aria-hidden` (decorative). Add a **single
  visually-hidden polite live region at the reader root** (SurahReader/RangeReader, near the
  orchestration call — not inside the per-ayah `{#each}`) announcing loading then error, computed once
  from `state.status` (e.g. `[...status.values()].some(s => s === "loading")`). This wires
  `reader_stacked_loading` and `reader_stacked_error`; do not leave either id orphaned.
- **Callers** — pass the three props via lookups `loadingFor/erroredFor/stackedFor` (§6):
  - `SurahReader.svelte` (per-ayah in `{#each pageData.ayahs}`).
  - `RangeReader.svelte` (per-ayah in `{#each g.ayahs}`, grouped by surah).

### 6. Client fetch orchestration — `web/src/routes/(application)/app/_reader/stacked-translations.svelte.ts`

`createStackedTranslations({ from, to, validator, primarySourceId, catalogue }) → { state, dispose }`.
`validator: AyahCoordinateValidator` (typed; pass the **constructed function** `ayahIndexValidator(quranData)`,
exactly like `SurahReader.svelte:451` / `RangeReader.svelte:101` — **not** a factory; call as
`quranWorker.readRange(from, to, validator, id, onStatus)` with no parens on `validator`).

Internals (runes):
- `effectiveExtras = $derived.by(() => stackedTranslations.ids.filter((id) => id !== primarySourceId()))`
  — dedupe primary (`primarySourceId()` returns `null` on Arabic routes — see §7).
- **Clear keyed on ROUTE identity, not range extent.** The `byVerse` clear fires only when a stable
  route token changes (e.g. `${initial.normalization.sourceId}:${initial.surah.num}` — the
  `initial`-anchored subset of `routeKey` at `SurahReader.svelte:111`; do **not** key on `from()/to()`).
  The fetch `$effect` still depends on `[from(), to(), effectiveExtras]`, but within a route it only
  **adds** new ayah keys to `byVerse` (merge by `ayah.key`, idempotent) — never deletes existing entries
  and never flips an already-`ready` extra back to `loading` for keys it resolved. This kills the
  skeleton flash on every forward/backward page fill.
- For each extra: `status="loading"`, then `quranWorker.readRange(from, to, validator, id, onStatus)`.
  On success (gen match) resolve catalogue meta once per id (map cache), push
  `{sourceId, translator, language, languageCode, direction, text}` into `byVerse.get(a.key)`,
  `status="ready"`; on error `status="error"`.
- **Concurrency cap (cold fetches):** wrap the per-extra `readRange` in a semaphore limiting
  **concurrent COLD** translation fetches to ~2. Use `quranWorker.hasTranslation(id)` first; warm reads
  (OPFS pointer already present) bypass the cap. Rationale: each cold load runs a ~30-60 ms sync WASM
  validation on the single worker thread, and the primary's own `loadPage` `readRange`
  (`SurahReader.svelte:448`) competes for the same thread — 6 cold loads serializing can stall the
  primary's page-2 text. (Note: the worry about a "6236-row re-scan on every miss" is unfounded —
  `opfs-cache` already skips `runStagedValidator` when an OPFS pointer exists, running only a
  `verifyBytes` size check on warm reads. The semaphore is the proportionate mitigation.)
- Export `stackedFor(state, key)` (ready texts, store-order), `loadingFor(state, key)`
  (`status==="loading"` ids → skeleton), `erroredFor(state, key)` (`status==="error"` ids → error row).
  Do **not** fold error ids into the skeleton path. `pendingFor` from the earlier draft is split into
  `loadingFor`/`erroredFor` precisely so an errored source shows an error, not an endless skeleton.
- **`dispose()` contract (mandatory):** as its **first** action, increment the gen token and/or set a
  `#disposed` flag. Every async continuation (the `readRange` `.then`, the `onStatus` callback, the
  catalogue-meta resolution) re-checks `if (startGen !== this.#gen || this.#disposed) return;` before
  touching `byVerse`/`status`. `readRange` returns an uncancellable Promise (no AbortSignal), so this
  gen/disposed guard is the only thing preventing post-teardown writes into orphaned state. (Unlike
  `loadPage`'s `readRouteKey` pattern, the orchestration is a `.svelte.ts` module with no live signal
  to re-read post-dispose, so gen must be bumped explicitly at teardown.)
- **SurahReader:** wire `from`/`to` to min startGlobal / max endGlobal across loaded pages; paging
  expands the range → additive merge (no clear). **RangeReader:** `from = data.startGlobal`,
  `to = data.endGlobal`; construct `onMount`, `dispose` on destroy.

### 7. Picker panel — `web/src/routes/(application)/app/_reader/StackedTranslationsPicker.svelte`

- **Mount point = Sidebar** (`Sidebar.svelte`, next to the existing `TranslationPicker.svelte:311`),
  **not** `ReaderHeader`. `ReaderHeader` is imported only by `SurahReader` (`SurahReader.svelte:47`),
  so a button there would leave juz / global-page (RangeReader) routes with extras rendering but no
  way to manage them. Sidebar mounts in `ReaderShell` on **every** reader route → uniform coverage.
  The trigger opens a drawer via `$lib/components/ui/sheet` (bits-ui `Dialog`), which handles
  focus-trap, initial focus, Escape, and restore-focus automatically (no hand-rolled focus mgmt).
- **All picker-local state** (`searchQuery`, scroll offset, any virtualizer) **must live inside
  `<Sheet.Content>`** (not hoisted to the trigger/Sidebar) so bits-ui's close-time unmount resets it.
- **Every picker mutation** (`toggle`/`reorder`/`remove`/`clear`) calls
  `replaceState(withMoreParam(page.url, stackedTranslations.ids), page.state)` at the call site —
  exactly as `changeMode` pairs `setMode` + `replaceState` for `mode` (`SurahReader.svelte:288-289`).
  The layout `$effect` (§4) is an untracked backstop and does **not** re-run on store mutations, so
  the picker owns user-initiated store→URL sync. One `replaceState` line covers all four mutations
  (incl. `clear`, since `withMoreParam` deletes when empty).
- Search `<input type="search">` filtering `catalogueStore.translations` by `language`/`translator`/`id`;
  `catalogueStore.translations` always returns the baked fallback synchronously (`catalogue-store.svelte.ts:17-19`),
  so the list is never empty.
- **Selected section** renders header + ordered list (up/down `reorder` + remove ×) **only when
  `ids.length > 0`**; otherwise render `reader_stacked_none_selected` with `role="status"` *above* the
  language-grouped list (never as a ghost empty section). Mirror `TranslationPicker.svelte:195-199`.
- **Language-grouped list:** group by `language` (local `Map` reduce — no heavy helper);
  `<input type="checkbox">` per row, checked = `ids.includes(t.id)`.
- **Cap UX = hard-stop at `STACKED_MAX_EXTRAS` extras.** At `ids.length === STACKED_MAX_EXTRAS` disable
  unchecked boxes + show `reader_stacked_full`. **Parameterize the cap** (never bake `5` into localized
  text): `reader_stacked_count = "{n}/{max}"`, `reader_stacked_full` takes `{max}`; pass
  `STACKED_MAX_EXTRAS` at the call site in every locale. `reader_stacked_full` should hint the limit is
  concurrent-display, not storage (e.g. `"{max}/{max} stacked — remove one to add another. Offline cache holds many more."`).
- **Primary dedupe (guarded):** `const primaryId = page.params.lang ? translationIdFromSegments(page.params.lang, page.params.translator) : null;`
  — mirror the guard at `TranslationPicker.svelte:83-87`, else Arabic routes yield the junk id
  `"undefined.undefined"`. The `primaryId` row shows a "Primary translation" badge + is disabled (not
  stackable). When the user navigates to a different `/t/` primary, `page.params` reactively updates
  (`$app/state`) and the disabled row swaps correctly. Pass `primaryId` (or `null`) to
  `createStackedTranslations`'s `primarySourceId`.
- a11y (`pnpm check --fail-on-warnings`): associate `<label>`s with checkboxes/inputs, `aria-label`
  on the trigger icon + search. Note: svelte-check does **not** verify focus-trap — that is covered by
  using the `sheet` primitive (verified present). Skeletons `aria-hidden`.

### 8. Nav + guards (mostly unchanged)

- `*For(ctx,…)` helpers, `routeContextFromParams`, `routes.test.ts`, `nav-guard.test.ts` — **no change**
  (new param is query-only; picker uses `replaceState`, not `goto`, and adds no `/app/` literals).
  Re-run to confirm green.
- `docs-divergence-guard.test.ts` — audit `docs/quran-system.md` for any "exactly one translation per
  page" claim; if found, add a one-line client-extras note. Multi-translation is client-only → no SSG
  claim changes.

## i18n (paraglide) — JSON is the source, `m/*.ts` is codegen

Add all entries to **both** `web/messages/reader/en.json` AND `web/messages/reader/ar.json` (the
source-of-truth catalogs; see `project.inlang/settings.json` globs). Then import them via the
`reader-copy.ts` pattern: `import { reader_stacked_title, reader_stacked_count, … } from "$lib/i18n/m/reader"`
(alias is `$lib/…`, not `$$/…`).

Keys: `reader_stacked_title`, `reader_stacked_search_placeholder`, `reader_stacked_selected`,
`reader_stacked_count` (value `"{n}/{max}"` in **both** locales — param names must be identical),
`reader_stacked_full` (`"{max}/{max} — remove one to add another. Offline cache holds many more."`,
taking `{max}`), `reader_stacked_move_up`, `reader_stacked_move_down`, `reader_stacked_remove`,
`reader_stacked_primary_badge`, `reader_stacked_open`, `reader_stacked_loading`, `reader_stacked_error`,
`reader_stacked_none_selected`.

Three toolchain-enforced constraints (any violation fails the build on the next `pnpm check`/`lint`/`test`/`build`):
1. **Never hand-edit** `src/lib/i18n/m/reader.ts` or `src/lib/paraglide/**` — every `pre*` hook runs
   `pnpm i18n:check` (`scripts/check-i18n.mjs && i18n:compile && i18n:namespaces`), regenerating all of
   `m/*.ts` and `paraglide/messages/*.js` from `messages/**/*.json`. Hand-edits are wiped.
2. **Locale parity is machine-enforced** — `check-i18n.mjs` throws on any key present in only one
   locale (missing/unknown). Add `en` AND `ar` in the same commit.
3. **`{param}` signature must match across locales** — `check-i18n.mjs` extracts + sorts `{param}`
   names per message and fails on en/ar divergence. Use identical param names in both.

## Tests (Vitest via `vite-plus`, import from `vite-plus/test`)

1. `stores/__tests__/stacked-translations.test.ts` (NEW) — mirror `reader-persistence.test.ts`:
   `vi.hoisted` browser mock + `vi.useFakeTimers()` + real `localStorage`; default empty, `setIds`
   persists `{v:1,ids}`, `toggle`/`reorder`/`remove`/`clear`, cap-at-5 (6th ignored), dedupe, cross-tab
   reconcile via `onStorageKey`, future-schema `{v:99}`→`[]`, non-array→`[]`, `dispose`.
2. `reader/__tests__/more-param.test.ts` (NEW) — parse (split/dedupe/junk-filter/empty), with
   (set + delete-when-empty, round-trip), matches (order-insensitive). **Plus a cold-deeplink
   convergence case:** mount the §4 `$effect` with empty `localStorage` + `?more=en.sahih` and assert
   it converges in ≤2 runs with `store == parseMoreParam(bar)`, plus a cap-drop case (`?more=a..f`)
   asserting the bar normalizes to the capped 5.
3. `offline/__tests__/keys.test.ts` (NEW/UPDATE) — `normalizeDataKey` strips `mode` **and** `more`,
   keeps others; `?more=a,b`+`?mode=verse` collapse to the same key as the bare path. **Plus a PAGES
   assertion** that `handleNavigation` stores/looks up by the normalized key (so combos collapse to one
   PAGES entry and serve offline) — the current plan would not catch a keying regression.
4. `workers/__tests__/opfs-retention.test.ts` (UPDATE) — lockstep constant mirrors → new values;
   scale the byte-pressure fixtures (~lines 60, 117) past 768 MB; add a TTL-eviction case.
5. `_reader/__tests__/VerseRow.stacking.test.ts` (NEW) — `stacked`+`stackedPending`+`stackedErrored` →
   N rows in store-order, skeleton for loading, **error row for errored** (not a skeleton),
   translator label, `lang={languageCode}`; no nested-ternary regression.
6. `_reader/__tests__/stacked-translations.orchestration.test.ts` (NEW) — with mocked
   `quranWorker.readRange`: byVerse **accumulates** across `from/to` expansion within a route (no clear,
   no skeleton re-flash); clears only on route-identity change; `validator` passed as a function (no
   TypeError); `dispose()` invalidates in-flight callbacks (post-dispose writes no-op); error status
   routes to `erroredFor`, not `loadingFor`.
7. `_reader/__tests__/StackedTranslationsPicker.test.ts` (NEW) — cap disables unchecked, **primary-null
   on Arabic route** (no `lang` param → no junk id, no row disabled), search filters, reorder changes
   order, **each mutation writes `page.url.searchParams` immediately** (no navigation), none-selected
   copy renders with `role="status"`.
8. `(application)/app/__tests__/nav-guard.test.ts` + `routes.test.ts` — re-run, expect green.

## Hard-rule compliance

- **identity = id, never hash** — extras keyed by sourceId everywhere. ✓
- **nav via `*For(ctx,…)`** — zero new nav; picker mutates store then `replaceState(withMoreParam(…))`
  at the call site (like `changeMode` for `mode`); the layout `$effect` is an untracked backstop. ✓
- **no nested ternary** — descriptor array + `{#each}`/`{#if kind}`; each-key uses a flat ternary on a
  primitive (`row.kind === "text" ? … : …`), not a nested one. ✓
- **`pnpm check` warning-free** — full a11y on checkboxes/inputs/icon buttons; focus-trap via the
  `sheet` primitive (svelte-check does not verify focus-trap, so do not treat warning-free as the gate
  for that — the primitive is the gate).
- **es-toolkit** — `uniq` for dedupe (named import); keep `$lib/storage` `trailingDebounce`; local
  `Map` reduce for grouping.
- **Quran DBs immutable** — extras read via the same read-only worker path; no writes. ✓

## Risks + MVP build order

**Risks:** first-use download latency (per-source skeleton; concurrency semaphore bounds the
validation CPU pile-up so the primary's paging text doesn't stall); in-memory LRU thrash on rapid
primary rotation (cap = 7 absorbs one stale primary per hop); range rebuild on page expansion is
**additive** (no clear → no flash); one-time SW DATA/PAGES re-population after `more` joins the
strip-set (SWR + normalized keys handle it).

**Build order (each step compiles + tests green before next):**
1. Store (§2) + test.
2. `more-param.ts` (§4) + SW/keys strip + **PAGES normalization** + tests.
3. OPFS constants (§3) + lockstep test.
4. Orchestration module (§6) with mocked `quranWorker.readRange` (incl. accumulation + dispose-gen + validator-as-function tests).
5. `VerseRow` stacking (§5: skeleton + **error** + `languageCode`) wired into SurahReader (single page) + reader-root live region.
6. Picker (§7) + i18n (incl. **Sidebar mount**, **mutation→replaceState**, primary-null guard).
7. Wire RangeReader.
8. Full `pnpm check` + `pnpm test`.

## Verification

- `cd web && pnpm test` — all new + existing tests green.
- `cd web && pnpm check` — `svelte-check --fail-on-warnings`, zero warnings.
- `cd web && pnpm lint` — oxlint clean (incl. `no-nested-ternary`).
- Manual (`pnpm dev`):
  - `/app/al-baqarah`: open the Sidebar picker, check 2-3 translations → extras stack under each ayah
    in chosen order, skeletons during first download, **error row if one fails**; **scroll past a page
    boundary → stacked rows stay (no flash)**; `?more=` appears in the URL immediately after each toggle.
  - Persist across surah nav + reload; primary `/t/en/sahih` still SSRs + caches
    (`x-easyquran-quran-cache: hit` unchanged by `?more`).
  - Reading mode → extras hidden, Arabic flow intact.
  - Cold deep link `?more=ur.jalandhry` → adopts into store, bar stays `?more=ur.jalandhry` (no flicker).
  - `/app/juz/1` and `/app/page/1` → picker available from Sidebar, extras render.
  - Arabic route `/app/al-baqarah` (no `/t/`) → no row shows a "Primary translation" badge; all
    selected translations are extras.
  - Offline: reload a `?more=` deep-link with network down → still serves the cached page (PAGES key
    normalized).

---

## Appendix — Audit corrections applied

A 7-lens adversarial audit (reactivity, caching/SW/SSR, OPFS/worker, data-join, nav/guards,
a11y/i18n/lint, UX/state) verified each finding against real code; 21 confirmed, 3 refuted.

**Confirmed → incorporated above:**
1. SW PAGES cache forked per `?more`/`?mode` combo + offline deep-link miss → §4 PAGES normalization. *(caching, high)*
2. Picker mutations never reached `?more` (mirror `$effect` untracks store; picker had no `replaceState`) → §7 call-site `replaceState`. *(nav/reactivity, high)*
3. Cold deep-link `?more` adoption thrashed the URL (stale-local + missing onMount hydrate) → §4 guarded onMount + `else if`. *(nav/reactivity, med; severity corrected — one-shot desync, not infinite oscillation, since `replaceState` doesn't reactively update `page.url`)*
4. i18n is codegen from `messages/reader/{en,ar}.json`, not authored ids → §i18n rewrite. *(a11y/i18n, high)*
5. `lang={row.t.language}` bound the display name, not BCP-47 → §1 `languageCode`. *(a11y, high; verifier 429'd, manually confirmed)*
6. `reader_stacked_count`/`_full` hardcoded the `5` literal → §7 `{n}/{max}` parameterization. *(a11y/i18n, med)*
7. Skeleton `aria-hidden` + `reader_stacked_loading`/`_error` orphaned → §5 reader-root polite live region. *(a11y, low)*
8. `TRANSLATION_DB_CAP=6` undercounts during primary transitions (no worker release message) → §3 cap = 7. *(opfs, high; verifier 429'd, manually confirmed)*
9. Concurrent cold loads serialize WASM validation on the worker thread, stalling the primary → §6 cold-fetch semaphore (≤2). *(opfs, med; mechanism corrected — no re-scan on warm reads, `opfs-cache` already fast-paths)*
10. `byVerse` cleared on every page expansion → skeleton flash on every forward-fill → §6/§1 route-identity clear + additive merge. *(data-join/reactivity, high; two lenses, same root)*
11. Errored source rendered as a permanent skeleton (no error kind; `reader_stacked_error` unused) → §5 error `ExtraRow` + §6 `loadingFor`/`erroredFor` split. *(data-join, med)*
12. `validator()` called as a factory → `TypeError` for every extra → §6 pass the function, no parens. *(data-join, med)*
13. `dispose()` didn't bump gen → post-teardown writes into orphaned state → §6 explicit gen/disposed contract. *(reactivity, low)*
14. Picker only in `ReaderHeader` (SurahReader-only) → juz/page range routes had no picker → §7 Sidebar mount. *(ux, high)*
15. Arabic route produced junk `primarySourceId` `"undefined.undefined"` (no null guard) → §7 guard. *(ux, low)*
16. Empty/none-selected state underspecified → §7 explicit `{#if ids.length}` + `role="status"`. *(ux, low)*
17. Drawer-local state (search/scroll) teardown unspecified → §7 keep state inside `<Sheet.Content>`. *(ux, low)*
18. Cap copy didn't explain the dual limit → §7 `reader_stacked_full` hints concurrent-display vs storage. *(ux, low)*

**Refuted (dropped):**
- "No dialog/focus-trap primitive exists" — wrong: `$lib/components/ui/sheet` (bits-ui `Dialog`) exists and handles focus-trap/restore/Escape; `Nav.svelte` already shows the hand-rolled equivalent.
- "Bumping OPFS caps collapses the lockstep test coverage" — lines ~38 and ~81 still guard count/byte eviction at the new thresholds; the synthetic-100 test becoming trivial is cosmetic.
- "Unspecified `pendingFor` risks a hydration mismatch" — `pendingFor` is specified (`loading/error` ids), so both lookups return `[]` pre-`$effect` → SSR and first client render match; no mismatch.
