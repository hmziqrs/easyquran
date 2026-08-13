# Multiple stacked translations in the ayah-by-ayah reader (quran.com-style)

> Status: design plan (not yet implemented). Owner-approved approach.
> Scope: web reader only. Server / SSR / disk cache intentionally untouched.

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
   In-memory resident LRU = **5 extras + 1 primary = `TRANSLATION_DB_CAP 6`** (the primary
   translation is read through the same in-memory `translationDbs` map, so 5 would evict it).
3. **Picker:** new dedicated drawer/panel from a `ReaderHeader` button (quran.com style: search,
   language-grouped checkboxes, selected-with-reorder). The existing sidebar `TranslationPicker`
   (chooses the **primary/path** translation) stays unchanged — clean split.

## Core design — "extras are a client concern, mirrored like `mode`"

Extras are a persisted client selection mirrored to a `?more=<id>,<id>` query param via a `$effect`
+ `replaceState` — **exactly the `mode` precedent** (`mode-param.ts` + the mirror `$effect` in
`+layout.svelte`). Consequences:

- `surah*For(ctx,…)` nav helpers **unchanged** (query is not a path segment) → nav-guard stays green.
- Disk-cache key built from the parsed route (`quran-disk-cache.ts` + `hooks.server.ts`
  `translationRouteCacheKey`) never sees the query string → **no new disk-cache entries, no
  combinatorial blowup**.
- Only the SW `normalizeDataKey` must strip `more` (else the `__data.json` DATA-cache forks per combo).
- On internal nav the new URL lacks `?more` (helpers don't carry it) → store is source of truth,
  extras re-fetch on the new page, the `$effect` re-syncs `?more`.

---

## Implementation

### 1. Data model — `web/src/lib/data/quran-types.ts` (append near `Ayah`)

```ts
export interface StackedTranslation {
  readonly sourceId: string;        // "en.sahih"
  readonly translator: string;      // TranslationCatalogueEntry.translator
  readonly language: string;        // .language (display)
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
extra source's `range.ayahs` on `ayah.key`. The map is range-scoped (rebuilt on page/route change),
**not** a global store; only the *selection* is persisted.

### 2. New store — `web/src/lib/stores/stacked-translations.svelte.ts`

Clone `ReaderSourceStore` (`reader-settings.svelte.ts`) exactly:

- localStorage key `easyquran.reader.stacked`, `STACKED_SCHEMA_VERSION = 1`, shape `{ v: number; ids: string[] }`.
- Decode with `asArray(obj.ids, asString)` (from `$lib/storage/decoders`), `isFutureSchema` guard, fallback `[]`.
- `#ids = $state<string[]>([])`; cross-tab via `onStorageKey`; `dispose()`.
- API: `get ids()`, `setIds(ids)` (dedupe via `es-toolkit` `uniq`, cap at `STACKED_MAX_EXTRAS = 5`,
  preserve order, write-through `writeJSON`), `toggle(id)`, `reorder(id, -1|1)`, `remove(id)`, `clear()`.
- Singleton `export const stackedTranslations = new StackedTranslationsStore()`.
- **No coupling to `readerSource`.** Dedupe vs the route's path sourceId happens at consume time (§6),
  so the store doesn't churn on nav.

### 3. OPFS / worker constants (decision #2)

- `web/src/lib/workers/quran.worker.ts` — `TRANSLATION_DB_CAP = 4` → **`6`** (5 extras + 1 primary;
  the primary translation is read through the same `translationDbs` LRU map). LRU logic
  (`evictTranslationDbs`, delete-on-access in `translationRunner`) is already correct — no other
  worker change. Add an inline comment explaining the +1.
- `web/src/lib/workers/opfs-retention.ts` — `CAP_COUNT = 12` → **`512`**;
  `CAP_BYTES = 128*1024*1024` → **`768*1024*1024`**; `TTL_MS` stays 30d (only drops DBs unused for a
  month; `stampLastUsed` re-stamps on access). With these caps `computeEvictions` effectively only
  TTL-evicts.
- **Lockstep test:** `web/src/lib/workers/__tests__/opfs-retention.test.ts` re-declares local
  `TTL_MS/CAP_COUNT/CAP_BYTES` mirrors — **update all three** + any `<= CAP_COUNT` assertion, or every
  eviction test breaks.

### 4. URL `?more=` roundtrip

- New `web/src/lib/reader/more-param.ts` — mirror `mode-param.ts`: `READER_MORE_PARAM = "more"`,
  `parseMoreParam(url) → string[]` (csv split, `uniq`, junk filtered), `withMoreParam(url, ids, base?) → URL`
  (`set` if non-empty else `delete`), `moreParamMatches(url, ids)` (order-insensitive csv equality).
- **Mirror `$effect`** in `routes/(application)/app/+layout.svelte`, next to the `mode` mirror. Store is
  source of truth; URL→store adoption **only when the URL explicitly carries `?more`** (deep-link /
  back-forward), **never clear on empty** (avoids wiping the store on internal nav, since `*For` hrefs
  don't carry the query):

  ```ts
  $effect(() => {
    const url = page.url;
    const ids = untrack(() => stackedTranslations.ids);
    const parsed = parseMoreParam(url);
    if (parsed.length && !moreParamMatches(url, ids)) stackedTranslations.setIds(parsed); // adopt explicit ?more
    if (!moreParamMatches(url, ids)) replaceState(withMoreParam(url, ids), page.state);   // store → URL
  });
  ```

  Store self-hydrates from localStorage in its constructor (browser-only) — no explicit hydrate call.
- **Disk cache: no change** (key excludes query). **SW strip-set:** add `if (key === "more") continue;`
  in both `web/src/service-worker.ts` (`normalizeDataKey`) and `web/src/lib/offline/keys.ts`.

### 5. `VerseRow.svelte` + the two callers

Add two props (pre-filtered per-`vKey` by the caller; VerseRow never reaches into maps):

```ts
stacked?: readonly StackedTranslation[];        // ready texts for THIS ayah, store-order, primary excluded
stackedPending?: readonly string[];             // extra sourceIds still loading for THIS ayah (skeletons)
```

Render under the existing primary `{#if translationActive}…{/if}` block. **No nested ternary**
(lint-enforced) — build a descriptor array, then `{#each}`:

```ts
type ExtraRow = { kind: "skeleton"; sourceId: string } | { kind: "text"; t: StackedTranslation };
const extraRows = $derived<ExtraRow[]>([
  ...(stackedPending ?? []).map((sourceId): ExtraRow => ({ kind: "skeleton", sourceId })),
  ...(stacked ?? []).map((t): ExtraRow => ({ kind: "text", t })),
]);
```

```svelte
{#each extraRows as row (row.kind === "text" ? row.t.sourceId : row.sourceId)}
  {#if row.kind === "skeleton"}
    <div class="verse-extra skeleton" aria-hidden="true"></div>
  {:else}
    <span class="verse-extra" dir={row.t.direction === "rtl" ? "rtl" : "auto"} lang={row.t.language}
          style="font-size:var(--reader-translation-size, 1.0625rem)">
      <span class="verse-extra-label">{row.t.translator || row.t.language}</span>{row.t.text}
    </span>
  {/if}
{/each}
```

- `.verse-extra { display:block; border-top:1px solid var(--line); margin-top:.5rem; padding-top:.5rem; text-align:start }`.
  Reuses the existing **stub** `--reader-translation-size` var (wire a size control later).
- Reading-mode inline-flow CSS is already scoped to `[data-source-kind="arabic"]` → extras stay block.
  Extras render only in verse mode (hide in reading mode like `VerseTools`).
- **Callers** — pass the two props via tiny lookups (`stackedFor(state, key)` / `pendingFor(state, key)`, §6):
  - `SurahReader.svelte` (per-ayah in `{#each pageData.ayahs}`).
  - `RangeReader.svelte` (per-ayah in `{#each g.ayahs}`, grouped by surah).

### 6. Client fetch orchestration — `web/src/routes/(application)/app/_reader/stacked-translations.svelte.ts`

`createStackedTranslations({ from, to, validator, primarySourceId, catalogue }) → { state, dispose }`.
Internals use runes (`$state` maps, `$derived`, `$effect`):

- `effectiveExtras = $derived.by(() => stackedTranslations.ids.filter((id) => id !== primarySourceId()))`
  — **dedupe primary here**.
- `$effect` on `[from(), to(), effectiveExtras]`: bump a `gen` token (ignore stale, like `loadPage`'s
  `readRouteKey`); clear `byVerse` when the range identity changes; for each extra set `status="loading"`,
  then `quranWorker.readRange(from, to, validator(), id, onStatus)` (unchanged signature); on success
  (gen match) resolve catalogue meta once per id (map cache), push
  `{sourceId, translator, language, direction, text}` into `byVerse.get(a.key)`, set `status="ready"`;
  on error `status="error"`.
- Assemble each ayah's array in `effectiveExtras` index order (settle out-of-order via `Promise.all`,
  sort on consume).
- Export `stackedFor(state, key)` (ready texts, store-order) + `pendingFor(state, key)` (loading/error ids).
- **SurahReader:** wire `from`/`to` to min startGlobal / max endGlobal across loaded pages; paging
  expands the range → re-fetch (cheap; resident DBs = in-memory query). **RangeReader:**
  `from = data.startGlobal`, `to = data.endGlobal`; construct `onMount`, `dispose` on destroy.
- First use of a translation DB = full OPFS download → per-source skeleton via `status`/`onStatus`.

### 7. Picker panel — `web/src/routes/(application)/app/_reader/StackedTranslationsPicker.svelte`

- Opened from a new icon button in `ReaderHeader.svelte` (toolbar cluster, render only when `clientMounted`).
- Search `<input type="search">` filtering `catalogueStore.translations` by `language`/`translator`/`id`;
  `catalogueStore.ensure()` on mount (same as existing `TranslationPicker.svelte`).
- **Selected section:** ordered list, up/down (`stackedTranslations.reorder`) + remove (×); shows
  `translator · language`.
- **Language-grouped list:** group by `language` (local `Map` reduce — no heavy helper);
  `<input type="checkbox">` per row, checked = `ids.includes(t.id)`.
- **Cap UX = hard-stop at 5 extras** (beyond 5 renders nothing since in-memory cap = 5). At
  `ids.length === 5` disable unchecked boxes + copy "5/5 — remove one to add another."
- **Primary dedupe:** the path sourceId (`translationIdFromSegments(page.params.lang, page.params.translator)`)
  row shown disabled with a "Primary translation" badge — not stackable.
- a11y (`pnpm check --fail-on-warnings`): associate `<label>`s with checkboxes/inputs, `role="dialog"`
  + `aria-label` + focus-trap on the drawer (reuse `$lib/components/ui` dialog primitive if present),
  `aria-label` on icon/search, `aria-hidden` skeletons.

### 8. Nav + guards (mostly unchanged)

- `*For(ctx,…)` helpers, `routeContextFromParams`, `routes.test.ts`, `nav-guard.test.ts` — **no change**
  (new param is query-only; new component has no `/app/` literals). Re-run to confirm green.
- `docs-divergence-guard.test.ts` — audit `docs/quran-system.md` for any "exactly one translation per
  page" claim; if found, add a one-line client-extras note. Multi-translation is client-only → no SSG
  claim changes.

## i18n (paraglide)

Add message ids (per-locale, follow the `reader-copy.ts` / lazy `reader-settings-copy.ts` pattern):
`reader_stacked_title`, `reader_stacked_search_placeholder`, `reader_stacked_selected`,
`reader_stacked_count` (`{n}/5`), `reader_stacked_full`, `reader_stacked_move_up`,
`reader_stacked_move_down`, `reader_stacked_remove`, `reader_stacked_primary_badge`,
`reader_stacked_open`, `reader_stacked_loading`, `reader_stacked_error`, `reader_stacked_none_selected`.

## Tests (Vitest via `vite-plus`, import from `vite-plus/test`)

1. `stores/__tests__/stacked-translations.test.ts` (NEW) — mirror `reader-persistence.test.ts`:
   `vi.hoisted` browser mock + `vi.useFakeTimers()` + real `localStorage`; default empty, `setIds`
   persists `{v:1,ids}`, `toggle`/`reorder`/`remove`/`clear`, cap-at-5 (6th ignored), dedupe, cross-tab
   reconcile via `onStorageKey`, future-schema `{v:99}`→`[]`, non-array→`[]`, `dispose`.
2. `reader/__tests__/more-param.test.ts` (NEW) — parse (split/dedupe/junk-filter/empty), with
   (set + delete-when-empty, round-trip), matches (order-insensitive).
3. `offline/__tests__/keys.test.ts` (NEW/UPDATE) — `normalizeDataKey` strips `mode` **and** `more`,
   keeps others; `?more=a,b`+`?mode=verse` collapse to same key as bare path.
4. `workers/__tests__/opfs-retention.test.ts` (UPDATE) — lockstep constant mirrors → new values;
   `<= CAP_COUNT` assertion → `512`.
5. `_reader/__tests__/VerseRow.stacking.test.ts` (NEW) — `stacked`+`stackedPending` → N rows in
   store-order, skeleton for pending, translator label, `dir`; no nested-ternary regression.
6. `_reader/__tests__/StackedTranslationsPicker.test.ts` (NEW) — cap disables unchecked, primary
   disabled, search filters, reorder changes order.
7. `(application)/app/__tests__/nav-guard.test.ts` + `routes.test.ts` — re-run, expect green.

## Hard-rule compliance

- **identity = id, never hash** — extras keyed by sourceId everywhere. ✓
- **nav via `*For(ctx,…)`** — zero new nav; picker uses store only; `?more` via `replaceState`. ✓
- **no nested ternary** — descriptor array + `{#each}`/`{if kind}`. ✓
- **`pnpm check` warning-free** — full a11y on checkboxes/dialog/inputs/icon buttons.
- **es-toolkit** — `uniq` for dedupe; keep `$lib/storage` `trailingDebounce`; local `Map` reduce for grouping.
- **Quran DBs immutable** — extras read via the same read-only worker path; no writes. ✓

## Risks + MVP build order

**Risks:** first-use download latency (per-source skeleton + optional `ensureTranslation` pre-fetch on
select — MVP+1); in-memory LRU thrash if user rotates primary among many with 5 extras (cap = 6
mitigates); range rebuild on page expansion (cheap while resident); one-time SW DATA-cache
re-population after `more` joins the strip-set (SWR handles it).

**Build order (each step compiles + tests green before next):**

1. Store (§2) + test.
2. `more-param.ts` (§4) + SW/keys strip + tests.
3. OPFS constants (§3) + lockstep test.
4. Orchestration module (§6) with mocked `quranWorker.readRange`.
5. `VerseRow` stacking (§5) wired into SurahReader (single page).
6. Picker (§7) + i18n.
7. Wire RangeReader.
8. Full `pnpm check` + `pnpm test`.

## Verification

- `cd web && pnpm test` — all new + existing tests green.
- `cd web && pnpm check` — `svelte-check --fail-on-warnings`, zero warnings (a11y on new UI).
- `cd web && pnpm lint` — oxlint clean (incl. `no-nested-ternary`).
- Manual (`pnpm dev`): open `/app/al-baqarah`; pick 2-3 translations in the new header drawer → extras
  stack under each ayah in chosen order, skeletons during first download, persist across surah nav +
  reload, `?more=` in URL. Confirm primary `/t/en/sahih` route still SSRs + caches (Network tab:
  `x-easyquran-quran-cache: hit` unchanged by `?more`). Toggle reading mode → extras hidden, Arabic
  flow intact. Open a deep link `?more=ur.jalandhry` cold → adopts into store.
