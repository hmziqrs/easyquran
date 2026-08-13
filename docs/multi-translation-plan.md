# Multiple stacked translations in the ayah-by-ayah reader (quran.com-style)

> Status: design plan (not yet implemented). Owner-approved approach.
> Scope: web reader only. Server SSR + 7-day disk cache intentionally untouched.
> This revision merges two independent adversarial code audits (see appendix).

## Context

Today the reader shows **one** translation at a time, encoded as URL path segments
`/t/[lang]/[translator]`, SSR'd + 7-day disk-cached per `sourceId`. `VerseRow.svelte` renders
exactly one text — **Arabic XOR translation** (`{#if translationActive}`, `VerseRow.svelte:70-88`):
on `/t/**` routes the page carries **no Arabic** (`quran-translation-page.ts` has zero Arabic refs),
so the row is a translation; on Arabic routes (`/app/[surah]`) it is Arabic. This plan adds
quran.com-style **multiple translations stacked under each ayah's primary line**, selectable via a
picker. Concretely:
- **Arabic route** (`/app/[surah]`): Arabic + N extras (extras literally "under the Arabic").
- **Translation route** (`/t/**`): primary translation + N extras (no Arabic anywhere; extras under the primary translation).

**Constraint (owner-decided):** multi-translation is **client-side only**. The SSR + disk cache do
not change — stacking N translations server-side is intractable for caching. Extras hydrate purely in
the browser, layered on top of the untouched primary SSR route. Note: on first use each extra's range
is served over the **HTTP Quran API** immediately (`withSourceFallback`, `worker-client.ts:194-217`);
the OPFS download is fire-and-forget background. So "server SSR untouched" still holds — extras hit
the read API (client→API), not the SSR server.

Reference: `docs/quran-system.md` (Hard rules, Web delivery, Part 3 divergences) and
`docs/my-plan-raw.md` (single-translation original intent — multi-translation is net-new).

## Decisions (locked)

1. **Load strategy:** client-side only. Primary path route + disk cache untouched. Extras fetch
   in-browser via `quranWorker.readRange` (HTTP API on first use; OPFS once warm).
2. **Caps:** OPFS disk retention raised to hold the whole corpus with margin — **`CAP_COUNT = 128`,
   `CAP_BYTES = 256 MiB`** (corpus = 115 DBs / **185.9 MiB** total, largest 12.4 MiB, per
   `translations.json`; 256 MiB covers it ~1.4× without inviting browser-side OPFS quota eviction,
   which the app cannot control). Selected extras are **pinned** (§3), so retention caps only govern
   *unselected* DBs. In-memory resident LRU = **`TRANSLATION_DB_CAP = STACKED_MAX_EXTRAS + 2 = 7`**
   (5 extras + current primary + 1 stale-primary buffer; see §3 for the `sources`/`translationDbs`
   split that makes this route-dependent).
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
  `translationRouteCacheKey`) never sees the query string → **no new disk-cache entries**. Query
  params also never reach `<link rel="canonical">` (`Seo.svelte:56` builds `SITE.url + path` from a
  path-helper result) → `?more` is SEO-neutral.
- The SW must normalize the **PAGES (navigational HTML) cache** key as well as the DATA cache, or
  `?more`/`?mode` combos fork PAGES into unbounded entries and break offline deep-link reload (§4).
- On internal nav the new URL lacks `?more` (helpers don't carry it) → the backstop `$effect`
  re-adds it from the store.

---

## Implementation

### 1. Data model — `web/src/lib/data/quran-types.ts` (append near `Ayah`)

```ts
export interface StackedTranslation {
  readonly sourceId: string;            // "en.sahih"
  readonly translator: string | null;   // TranslationCatalogueEntry.translator (nullable; quran-types.ts:157)
  readonly language: string;            // .language (display name, for the visible label)
  readonly languageCode: string;        // .languageCode (BCP-47, for the HTML lang="" attribute)
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

`translator` is `string | null` to match the catalogue type (strict `tsconfig` would fail the
assignment otherwise). The visible label falls back to `name`/`language` when null (0 of 115 baked
rows are null today, so this is type-correctness, not a runtime-fallback need). Join key =
`ayah.key` (`quran-types.ts:179`). The orchestration builds `byVerse` by indexing each extra source's
`range.ayahs` on `ayah.key`. The map is **rebuilt on surah/route identity change and merged
(additively) across page-window expansion within a surah** — never cleared on a page load (clearing
flashes every stacked row to a skeleton). Only the *selection* (which sourceIds) is persisted.

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

### 3. OPFS / worker constants + pinning (decision #2)

**In-memory LRU cap** — `web/src/lib/workers/quran.worker.ts` — `TRANSLATION_DB_CAP = 4` → **`7`**
(= `STACKED_MAX_EXTRAS + 2`). Route-dependent nuance: Arabic sources live in a **separate `sources`
map** (`quran.worker.ts:67`); only translation routes put the primary into `translationDbs`. So on a
**translation route**: 5 extras + primary + 1 stale-primary buffer = 7 (the buffer absorbs one linger
during a primary hop, since the worker has no `releaseSource`/evict message — `evictTranslationDbs`
runs `while (size >= CAP)` *before* insert, so resident max == cap exactly). On an **Arabic route**:
primary is Arabic (in `sources`), so `translationDbs` holds only the 5 extras (+2 spare). LRU logic
(`evictTranslationDbs`, delete-on-access in `translationRunner`) is already correct — no other worker
change. Inline-comment the +2 rationale. (Optional hardening: a `releaseSource(sourceId)` WorkerRequest
fired when `primarySourceId` changes, dropping the stale primary explicitly — only if 7 is deemed high.)

**OPFS retention** — `web/src/lib/workers/opfs-retention.ts` — `CAP_COUNT = 12` → **`128`**;
`CAP_BYTES = 128*1024*1024` → **`256*1024*1024`**; `TTL_MS` stays 30d. Corpus is 115 DBs / 185.9 MiB,
so 128/256 MiB holds it all with margin while keeping a real backstop (avoids browser OPFS quota
eviction the app can't control). With these caps + pinning (below), `computeEvictions` is effectively
**TTL-only for unselected DBs** in production.

**Pin selected extras (mandatory).** `pruneTranslations` runs after every fresh download
(`quran.worker.ts:328`) **and on every `init`/boot** (`:487`), and both pass only
`pinnedArabicIds: PINNED_ARABIC` — `computeEvictions` pins nothing else. A persisted-but-cold extra is
an ordinary artifact and can be TTL/cap-evicted at boot, then silently re-downloaded. Fix: add a
`pinnedTranslationIds` (or generic `pinnedIds`) field to `PruneOptions` (`opfs-retention.ts:63-66`),
populated from `stackedTranslations.ids` **plus the current primary translation id**, and include it
in both prune call sites. The client must post the stacked ids to the worker (extend the `init` message
or add a `setPinnedTranslations` message; refresh on every selection change) so the worker can include
them in prune calls. Without this, OPFS thrashes re-downloading the user's active selection.

**Lockstep test:** `web/src/lib/workers/__tests__/opfs-retention.test.ts` re-declares local
`TTL_MS/CAP_COUNT/CAP_BYTES` mirrors (`:6-8`) — update all three. The count-cap (`:38`) and byte-cap
(`:81`) assertions already exercise eviction at the new thresholds. **Rewrite the 100-candidate
property test (`:150-160`) to generate `CAP_COUNT + N` candidates** — at any cap ≥ 100 the current
hard-coded 100 evicts nothing, so `expect(evicted).not.toContain(...)` passes vacuously and proves
nothing. Scale the byte-pressure fixtures (`:60`, `:117`) past 256 MiB. Add a TTL-eviction case
(stale > 30d evicted, fresh kept) and a **pin-protection case** asserting `computeEvictions` never
evicts a currently-selected extra id.

### 4. URL `?more=` roundtrip

- New `web/src/lib/reader/more-param.ts` — mirror `mode-param.ts`: `READER_MORE_PARAM = "more"`,
  `parseMoreParam(url) → string[]` (csv split, `uniq`, junk filtered), `withMoreParam(url, ids, base?) → URL`
  (`set` if non-empty else `delete`), `moreParamMatches(url, ids)` (order-insensitive csv equality).
- **Cold deep-link adoption in `onMount`** (mirrors `reader.hydrate(parseModeParam(...))` at
  `+layout.svelte:39-40`; `onMount` is queued before the mirror `$effect` at `:53`). Extend `onMount`:
  ```ts
  const moreIds = parseMoreParam(page.url);
  if (moreIds.length) stackedTranslations.setIds(moreIds); // adopt cold deep-link ?more
  ```
  The `if (moreIds.length)` guard is **mandatory**: `setIds` is a replace, so a blind
  `setIds(parseMoreParam(url))` would wipe the persisted selection on a cold *bare* URL.
- **Backstop `$effect`** in `routes/(application)/app/+layout.svelte` next to the `mode` mirror.
  Keep `untrack` (the picker syncs the URL itself — §7), and make adopt + sync **mutually exclusive**
  (`else if`) so a URL→store adoption never feeds a stale local into the store→URL write on the same
  pass. (A bare `return`-after-adopt is **not** sufficient — it would skip URL normalization in the
  cap-drop case `?more=a..f`, leaving an invalid 6-item `?more` in the bar.) Note: `replaceState` does
  **not** reactively reassign `page.url` (SvelteKit `client.js`), so convergence is across real-nav
  re-runs, not one tick:
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
- **Disk cache: no change** (key excludes query). **SW — normalize BOTH caches.** Add `more` to the
  strip-set in `normalizeDataKey` (`service-worker.ts:45` + mirror `lib/offline/keys.ts`) so the
  `__data.json` DATA-cache is shared. **Additionally** `handleNavigation` must store/look up the PAGES
  cache by the **normalized** key, else every `?more`/`?mode` combo becomes a distinct PAGES entry
  (unbounded; crowds `PAGES_MAX=300`) and offline reload of a deep-link misses → 404 shell. In
  `handleNavigation`, compute `const key = normalizeDataKey(req.url)` once and use `new Request(key)`
  for `pages.put`, `pages.match`, `pages.delete` (the pattern `handleData` already uses); keep
  `fetch(req,…)` using the raw request. This also collapses the **pre-existing** `?mode` PAGES fork.
  Server HTML is byte-identical across combos, so one cached variant per logical page is correct.

### 5. `VerseRow.svelte` + the two callers

Add three props (pre-filtered per-`vKey` by the caller; VerseRow never reaches into maps):
```ts
stacked?: readonly StackedTranslation[];     // ready texts for THIS ayah, store-order, primary excluded
stackedPending?: readonly string[];          // extra sourceIds status==="loading" for THIS ayah (skeletons)
stackedErrored?: readonly string[];          // extra sourceIds status==="error" for THIS ayah (error rows)
```
Render under the existing primary `{#if translationActive}…{/if}` block, **gated to verse mode**
(`reader` is already imported in `VerseRow`). Reading mode inlines `.verse-row`/`.verse-text`
(`VerseRow.svelte:111-119`, scoped to `[data-reader-mode="reading"] [data-source-kind="arabic"]`); a
`display:block` `.verse-extra` inside an inlined `<li>` would break that flow, so extras must be
absent, not just CSS-hidden:
```svelte
{#if reader.isVerseMode}
  {#each extraRows as row (row.kind === "text" ? row.t.sourceId : row.sourceId)}
    {#if row.kind === "skeleton"}
      <div class="verse-extra skeleton" aria-hidden="true"></div>
    {:else if row.kind === "error"}
      <span class="verse-extra verse-extra--error" role="status">{copy.stackedError}</span>
    {:else}
      <span class="verse-extra" dir={row.t.direction === "rtl" ? "rtl" : "auto"} lang={row.t.languageCode}
            style="font-size:var(--reader-translation-size, 1.0625rem)">
        <span class="verse-extra-label">{row.t.translator ?? row.t.language}</span>{row.t.text}
      </span>
    {/if}
  {/each}
{/if}
```
**No nested ternary** (lint-enforced) — build a descriptor array, then `{#each}`/`{#if kind}`:
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
- `lang={row.t.languageCode}` (BCP-47), **not** the display `language` — otherwise you emit
  `<span lang="English">` (invalid; breaks screen-reader pronunciation + hyphenation).
- `.verse-extra { display:block; border-top:1px solid var(--line); margin-top:.5rem; padding-top:.5rem; text-align:start }`.
  Reuses the existing **stub** `--reader-translation-size` var (wire a size control later).
- **Loading/error a11y:** keep the per-ayah skeleton `aria-hidden` (decorative). Add a **single
  visually-hidden polite live region at the reader root** (SurahReader/RangeReader, near the
  orchestration call — not inside the per-ayah `{#each}`) announcing loading then error, computed once
  from `state.status`. This wires `reader_stacked_loading` and `reader_stacked_error`; do not leave
  either id orphaned.
- **Callers** — pass the three props via lookups `loadingFor/erroredFor/stackedFor` (§6):
  - `SurahReader.svelte` (per-ayah in `{#each pageData.ayahs}`).
  - `RangeReader.svelte` (per-ayah in `{#each g.ayahs}`, grouped by surah).

### 6. Client fetch orchestration — `web/src/routes/(application)/app/_reader/stacked-translations.svelte.ts`

`createStackedTranslations({ from, to, validator, primarySourceId, catalogue }) → { state, dispose }`.
`validator: AyahCoordinateValidator` (typed; pass the **constructed function** `ayahIndexValidator(quranData)`,
exactly like `SurahReader.svelte:451` / `RangeReader.svelte:101` — **not** a factory; call as
`quranWorker.readRange(from, to, validator, id, onStatus)` with no parens on `validator`).

**Read path is API-served on first use, not OPFS-blocked.** `readRange` routes through
`withSourceFallback` (`worker-client.ts:194-217`): `tryLocal()`; on miss, `onMiss` fires
`ensureTranslation` **fire-and-forget** (background OPFS download, `:368-370`), then if `QURAN.apiBase`
(set here via `.env:68`), `await apiFetch()` serves the range over HTTP **now**. So:
- First use of an extra = an **HTTP API range call** (client→read API), returned immediately. Skeletons
  are brief/rare — the OPFS download continues in the background and warms future reads. Use
  `onStatus.servedBy` (`"api"` vs `"local"`, `:190,196,205`) to optionally surface a network tier.
- Background `ensureTranslation` runs the staged WASM validation on the worker thread; it does **not**
  block the read (which already returned via API). The earlier "validation stalls the primary's read"
  worry is therefore moot for cold reads — **no concurrency semaphore is required for correctness.**
  (If warm-read latency from background validation pile-up is ever observed, a cold-fetch cap can be
  added then; it is not needed for the MVP.)
- **Offline + cold extra** (no `apiBase` reachable + OPFS miss) → `fail()` throws `ReadChainError`
  (`:188-192`). The orchestration must catch → `status="error"` → error row (§5), not crash. (Extras
  work offline only once their DB is in OPFS.)

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
  `status="ready"`; on error/throw → `status="error"`.
- Export `stackedFor(state, key)` (ready texts, store-order), `loadingFor(state, key)`
  (`status==="loading"` ids → skeleton), `erroredFor(state, key)` (`status==="error"` ids → error row).
  Do **not** fold error ids into the skeleton path — an errored source shows an error, not an endless
  skeleton.
- **`dispose()` contract (mandatory):** as its **first** action, increment the gen token and/or set a
  `#disposed` flag. Every async continuation (the `readRange` `.then`, the `onStatus` callback, the
  catalogue-meta resolution) re-checks `if (startGen !== this.#gen || this.#disposed) return;` before
  touching `byVerse`/`status`. `readRange` returns an uncancellable Promise (no AbortSignal), so this
  gen/disposed guard is the only thing preventing post-teardown writes into orphaned state.
- **SurahReader:** wire `from`/`to` to min startGlobal / max endGlobal across loaded pages; paging
  expands the range → additive merge (no clear). **RangeReader:** `from = data.startGlobal`,
  `to = data.endGlobal`; construct `onMount`, `dispose` on destroy.

### 7. Picker panel — `web/src/routes/(application)/app/_reader/StackedTranslationsPicker.svelte`

- **Mount point = Sidebar** (`Sidebar.svelte`, next to the existing `TranslationPicker.svelte:311`),
  **not** `ReaderHeader` (`ReaderHeader` is imported only by `SurahReader`, `:47`, so a button there
  would leave juz / global-page routes with extras rendering but no way to manage them). Sidebar mounts
  in `ReaderShell` on **every** reader route → uniform coverage. The trigger opens a drawer via
  `$lib/components/ui/sheet` (bits-ui `Dialog`), which handles focus-trap, initial focus, Escape, and
  restore-focus automatically (verified present; `Nav.svelte:106-148` shows the hand-rolled equivalent).
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
  `"undefined.undefined"` (`translationIdFromSegments` takes non-optional strings, `quran.ts:68`). On
  Arabic routes `primaryId` is `null` → no row badged/disabled → every selected translation is an extra.
  The `primaryId` row (when non-null) shows a "Primary translation" badge + is disabled. When the user
  navigates to a different `/t/` primary, `page.params` reactively updates (`$app/state`) and the
  disabled row swaps correctly. Pass `primaryId` (or `null`) to `createStackedTranslations`'s `primarySourceId`.
- a11y (`pnpm check --fail-on-warnings`): associate `<label>`s with checkboxes/inputs, `aria-label`
  on the trigger icon + search. svelte-check does **not** verify focus-trap — that is covered by the
  `sheet` primitive (the gate), not by warning-free check. Skeletons `aria-hidden`.

### 8. Nav + guards (mostly unchanged)

- `*For(ctx,…)` helpers, `routeContextFromParams`, `routes.test.ts`, `nav-guard.test.ts` — **no change**
  (new param is query-only; picker uses `replaceState`, not `goto`, and adds no `/app/` literals).
  Re-run to confirm green.
- `docs-divergence-guard.test.ts` — audit `docs/quran-system.md` for any "exactly one translation per
  page" claim; if found, add a one-line client-extras note. Multi-translation is client-only → no SSG
  claim changes.

## i18n (paraglide) — JSON is the source, `m/*.ts` is codegen

Add all entries to **both** `web/messages/reader/en.json` AND `web/messages/reader/ar.json` (the
source-of-truth catalogs; locales are `en`+`ar` only, `i18n/locales.ts:1`). Then import them via the
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
   it converges in ≤2 runs with `store == parseMoreParam(bar)` (the adopt path must not blank `?more`
   before re-writing it), plus a cap-drop case (`?more=a..f`) asserting the bar normalizes to the capped 5.
3. `offline/__tests__/keys.test.ts` (NEW/UPDATE) — `normalizeDataKey` strips `mode` **and** `more`,
   keeps others; `?more=a,b`+`?mode=verse` collapse to the same key as the bare path. **Plus a PAGES
   assertion** that `handleNavigation` stores/looks up by the normalized key (combos → one PAGES entry,
   serves offline).
4. `workers/__tests__/opfs-retention.test.ts` (UPDATE) — lockstep mirrors → 128/256 MiB; **rewrite the
   100-candidate test to `CAP_COUNT + N`**; scale byte fixtures past 256 MiB; add a TTL-eviction case
   and a **pin-protection case** (a selected extra id is never evicted).
5. `_reader/__tests__/VerseRow.stacking.test.ts` (NEW) — `stacked`+`stackedPending`+`stackedErrored` →
   N rows in store-order, skeleton for loading, **error row for errored** (not a skeleton),
   translator label (`??` null fallback), `lang={languageCode}`, **extras absent in reading mode**;
   no nested-ternary regression.
6. `_reader/__tests__/stacked-translations.orchestration.test.ts` (NEW) — with mocked
   `quranWorker.readRange`: byVerse **accumulates** across `from/to` expansion within a route (no clear,
   no skeleton re-flash); clears only on route-identity change; `validator` passed as a function (no
   TypeError); `dispose()` invalidates in-flight callbacks (post-dispose writes no-op); error/throw →
   `erroredFor` (incl. an offline-cold-extra throw case), not `loadingFor`.
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
  `sheet` primitive (svelte-check does not verify focus-trap, so the primitive is the gate).
- **es-toolkit** — `uniq` for dedupe (named import); keep `$lib/storage` `trailingDebounce`; local
  `Map` reduce for grouping.
- **Quran DBs immutable** — extras read via the same read-only worker/API path; no writes. ✓

## Out of scope (state explicitly)

- **Copy/share excludes extras.** `VerseTools` copy + share use only the `text` prop
  (`VerseTools.svelte:17,32,40`), so stacked translations are not included in copied or shared verse
  text. (A future "copy all" can compose the extras; not in this MVP.)
- **Offline extras require OPFS.** An extra works offline only once its DB is in OPFS; offline + a
  cold extra throws and surfaces as an error row (§6), not a silent degradation.

## Risks + MVP build order

**Risks:** first-use is an API call per extra per range (cheap; not an OPFS-download wait);
in-memory LRU thrash on rapid primary rotation (cap = 7 absorbs one stale primary per hop); range
rebuild on page expansion is **additive** (no clear → no flash); unselected DBs subject to TTL/cap
eviction (selected ones pinned); one-time SW DATA/PAGES re-population after `more` joins the strip-set
(SWR + normalized keys handle it).

**Build order (each step compiles + tests green before next):**
1. Store (§2) + test.
2. `more-param.ts` (§4) + SW/keys strip + **PAGES normalization** + tests.
3. OPFS constants + **pinning plumbing** (§3) + lockstep test.
4. Orchestration module (§6) with mocked `quranWorker.readRange` (incl. accumulation + dispose-gen + validator-as-function + offline-throw→error tests).
5. `VerseRow` stacking (§5: skeleton + **error** + `languageCode` + **verse-mode gate**) wired into SurahReader (single page) + reader-root live region.
6. Picker (§7) + i18n (incl. **Sidebar mount**, **mutation→replaceState**, primary-null guard).
7. Wire RangeReader.
8. Full `pnpm check` + `pnpm test`.

## Verification

- `cd web && pnpm test` — all new + existing tests green.
- `cd web && pnpm check` — `svelte-check --fail-on-warnings`, zero warnings.
- `cd web && pnpm lint` — oxlint clean (incl. `no-nested-ternary`).
- Manual (`pnpm dev`):
  - `/app/al-baqarah`: open the Sidebar picker, check 2-3 translations → extras stack under the
    **Arabic** in chosen order (brief skeleton, then text via API; OPFS warms in background); **error
    row if one fails**; **scroll past a page boundary → stacked rows stay (no flash)**; `?more=` appears
    in the URL immediately after each toggle.
  - `/app/al-baqarah/t/en/sahih`: extras stack under the **primary translation** (no Arabic on the page).
  - Persist across surah nav + reload; primary `/t/en.sahih` still SSRs + caches
    (`x-easyquran-quran-cache: hit` unchanged by `?more`); canonical URL has no `?more`.
  - Reading mode → extras hidden, Arabic flow intact.
  - Cold deep link `?more=ur.jalandhry` → adopts into store, bar stays `?more=ur.jalandhry` (no flicker).
  - `/app/juz/1` and `/app/page/1` → picker available from Sidebar, extras render.
  - Arabic route (no `/t/`) → no "Primary translation" badge; all selected translations are extras.
  - Offline: reload a warm `?more=` deep-link with network down → still serves the cached page (PAGES
    key normalized); a **cold** extra offline → error row, not a crash.

---

## Appendix — Audit corrections applied

Two independent adversarial code audits were merged: a 7-lens workflow audit (reactivity, caching/SW/SSR,
OPFS/worker, data-join, nav/guards, a11y/i18n/lint, UX/state) and a second source-verified audit
(commit `a16c74c`). Every item below was re-proved against code (`file:line` in each section).

**Confirmed → incorporated above:**
1. SW PAGES cache forked per `?more`/`?mode` combo + offline deep-link miss → §4 PAGES normalization. *(caching, high)*
2. Picker mutations never reached `?more` (mirror `$effect` untracks store; picker had no `replaceState`) → §7 call-site `replaceState`. *(reactivity, high)*
3. Cold deep-link `?more` adoption thrashed the URL (stale-local + missing onMount hydrate) → §4 guarded onMount + `else if` (supersedes the audit's "return after adopt", which would skip cap-drop normalization). *(reactivity, med)*
4. i18n is codegen from `messages/reader/{en,ar}.json`, not authored ids → §i18n rewrite. *(a11y/i18n, high)*
5. `lang={row.t.language}` bound the display name, not BCP-47 → §1 `languageCode`. *(a11y, high)*
6. `reader_stacked_count`/`_full` hardcoded the `5` literal → §7 `{n}/{max}` parameterization. *(a11y/i18n, med)*
7. Skeleton `aria-hidden` + `reader_stacked_loading`/`_error` orphaned → §5 reader-root polite live region. *(a11y, low)*
8. `TRANSLATION_DB_CAP=6` undercounts during primary transitions → §3 cap = 7 (+ separate `sources`/`translationDbs` split: Arabic primary is not in `translationDbs`). *(opfs, high)*
9. `byVerse` cleared on every page expansion → skeleton flash on every forward-fill → §6/§1 route-identity clear + additive merge. *(data-join/reactivity, high)*
10. Errored source rendered as a permanent skeleton (no error kind; `reader_stacked_error` unused) → §5 error `ExtraRow` + §6 `loadingFor`/`erroredFor` split. *(data-join, med)*
11. `validator()` called as a factory → `TypeError` for every extra → §6 pass the function, no parens. *(data-join, med)*
12. `dispose()` didn't bump gen → post-teardown writes into orphaned state → §6 explicit gen/disposed contract. *(reactivity, low)*
13. Picker only in `ReaderHeader` (SurahReader-only) → juz/page range routes had no picker → §7 Sidebar mount. *(ux, high)*
14. Arabic route produced junk `primarySourceId` `"undefined.undefined"` (no null guard) → §7 guard. *(ux, low)*
15. Empty/none-selected state underspecified → §7 explicit `{#if ids.length}` + `role="status"`. *(ux, low)*
16. Drawer-local state (search/scroll) teardown unspecified → §7 keep state inside `<Sheet.Content>`. *(ux, low)*
17. Cap copy didn't explain the dual limit → §7 `reader_stacked_full` hints concurrent-display vs storage. *(ux, low)*
18. `translator` typed `string` but catalogue is `string | null` (strict `tsconfig`) → §1 `translator: string | null`, label `??` fallback. *(types, med)*
19. Reader is Arabic XOR translation — extras can't sit "under Arabic" on `/t/**` → Context + §5 clarify the per-route stack shape. *(semantics, med)*
20. Extras never gated to verse mode (snippet had no gate; `display:block` would break reading-mode inline flow) → §5 explicit `{#if reader.isVerseMode}`. *(rendering, med)*
21. **First-use is API-served, not an OPFS-download wait** (`withSourceFallback`, `worker-client.ts:194-217`; `ensureTranslation` is fire-and-forget) → §6 corrected; offline+cold throws → error row. The earlier "validation stalls the read / semaphore" worry is retracted (validation is background, off the read path). *(mechanism, high)*
22. Selected extras not pin-protected; pruning runs at boot + after each download pinning only Arabic (`quran.worker.ts:328,487`) → §3 `pinnedTranslationIds` plumbing. *(opfs, high)*
23. Retention caps 512/768 MiB overshoot the 185.9 MiB corpus ~4× and invite browser OPFS quota eviction → §3 128/256 MiB. *(sizing, med)*
24. The 100-candidate property test passes vacuously at any cap ≥ 100 → §3 rewrite to `CAP_COUNT + N`. *(tests, low)*
25. `?more` does not reach `<link rel="canonical">` (`Seo.svelte:56`) → Core design notes it is SEO-neutral. *(seo, info)*

**Refuted (dropped):**
- "No dialog/focus-trap primitive exists" — wrong: `$lib/components/ui/sheet` (bits-ui `Dialog`) exists; `Nav.svelte:106-148` shows the equivalent.
- "Bumping OPFS caps collapses lockstep coverage" — lines ~38 and ~81 still guard count/byte eviction; the synthetic test is rewritten per item 24.
- "Unspecified `pendingFor` risks a hydration mismatch" — `pendingFor` is specified (`loading`/`error` ids), so both lookups return `[]` pre-`$effect` → SSR and first client render match.

**Re-verified as correct in the plan (no action):** `readRange(from,to,validator,source,onStatus)` arg order (`worker-client.ts:348-353`); both `mode` strip sites (`service-worker.ts:45`, `lib/offline/keys.ts`); `Ayah.key` join key (`quran-types.ts:179`); disk-cache key excludes query; `surah*For(ctx,…)` unchanged; nav-guard unaffected; `docs-divergence-guard.test.ts` exists; locales `en`+`ar` only.
