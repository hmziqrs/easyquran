# Web code quality plan

Scope: `web/` — SvelteKit 2 + Svelte 5 (runes), `adapter-static`, prerendered SPA.
Verified against Svelte 5.56.8 as installed. Line references are accurate at time of writing; re-confirm before editing.

---

## 1. Critical bug: verse cache is not reactive

**File:** `web/src/lib/stores/reader.svelte.ts:111`

```ts
#versesBySurah = $state(new Map<number, string[]>());   // :111
seedSurah(num, verses) { this.#versesBySurah.set(num, verses); }  // :317
```

`$state` does not proxy `Map`/`Set`. From `node_modules/svelte/src/internal/client/proxy.js`:

```js
const prototype = get_prototype_of(value);
if (prototype !== object_prototype && prototype !== array_prototype) {
  return value;   // Map's prototype is neither → returned unproxied
}
```

`versesFor()` reads the `#versesBySurah` field (a reactive source), then calls `.get()` on the raw Map. A re-render therefore only occurs if the **field is reassigned** — which `seedSurah` never does.

**Symptom.** `[surah]/+page.svelte:38` calls `seedSurah` inside an `$effect`, and effects run after render. So on the first visit to any surah, the sidebar's ayah-browse list (`Sidebar.svelte:148`) renders empty and stays empty. Navigate away and back and it populates, because the Map is already filled at render time. The same root cause blanks the `text` field in `bookmarkList` (`reader.svelte.ts:270`).

**Fix.** Use `SvelteMap` from `svelte/reactivity`:

```ts
import { SvelteMap } from "svelte/reactivity";
#versesBySurah = new SvelteMap<number, string[]>();
```

**Verify.** Open a surah with the sidebar in ayah-browse mode on a cold load; the ayah list must be populated on first paint.

---

## 2. Structural refactors, ranked by payoff

### A. Module-singleton stores → `createContext`

Every store ends with a module-level instance: `export const reader = new ReaderStore()` (`reader.svelte.ts:386`), and the same in `prefs`, `consent`, `quran`, `notifications`.

SvelteKit documents this as a security issue, not just hygiene — shared server state leaks between users: *"if Alice submitted an embarrassing secret, and Bob visited the page after her, Bob would know Alice's secret."* Prerendering means it is not live today, but it becomes a cross-user leak the moment any route enables SSR. It is also why these components cannot currently be tested in isolation.

Migrate to `createContext` (type-safe; no key management):

```ts
// lib/stores/reader.svelte.ts
import { createContext } from "svelte";
export const [getReader, setReader] = createContext<ReaderStore>();
```

Set in the app layout, read in components. Highest-leverage change in the repo.

> **Version floor:** `createContext` requires **Svelte ≥ 5.40**. `web/package.json` declares `"svelte": "^5.56.1"` — do not widen that range below 5.40.

### B. Unify the four persistence layers

`reader`, `prefs`, `consent`, and `notifications` each hand-roll `STORAGE_KEY`, a `load()` with try/catch JSON parse + field validation, and a `persist()`. They have drifted: only `reader` registers cross-tab `storage` sync (`reader.svelte.ts:125`), so a theme or consent change in one tab never reaches another.

Build one generic helper:

```ts
// lib/stores/persisted.svelte.ts
createPersisted<T>(key: string, version: number, validate: (raw: unknown) => Partial<T>)
```

It owns: SSR guard, versioned schema gate, validated load, debounced write, and the cross-tab `storage` listener. Collapses ~120 duplicated lines and makes cross-tab behaviour uniform.

### C. Split `reader.svelte.ts` (386 lines)

It owns persistence, navigation, search query, browse mode, typography, bookmarks, notes, a verse cache, worker refresh, and clipboard/share. Split along the comment banners already in the file:

- `bookmarks.svelte.ts`
- `notes.svelte.ts`
- `verse-cache.svelte.ts` (worker refresh lives here)
- typography (`fontSize`, `arabicSizePx`, `bigger`, `smaller`) → merge into `prefs`
- `copyVerse` / `shareVerse` (`:349`, `:363`) → pure functions in `lib/quran/share.ts`; they take a key and touch no store state

### D. Collapse `VerseRow.svelte`'s four action blocks

**File:** `VerseRow.svelte:81–159`. Four near-identical `Tooltip` + `TooltipTrigger` + `TooltipContent` + `button` blocks, ~20 lines each, differing only in icon, label, handler, and active class.

Replace with one `{#snippet action({ icon, label, onclick, active })}` or a `VerseAction.svelte`. Takes ~80 lines to ~25. Highest-density duplication in the codebase.

Each action currently expands to roughly eight component instances (`Tooltip` → `TooltipPrimitive.Root` → `TooltipTrigger` → `TooltipPrimitive.Trigger` → `TooltipContent` → `TooltipPortal` → `TooltipPrimitive.Portal`/`Content`), instantiated per row. Al-Baqarah has 286 rows. Note that `content-visibility: auto` on `VerseRow.svelte:72` defers **paint**, not component instantiation, so it does not mitigate this. Profile before and after rather than assuming a figure.

### E. Break up the 100-line `onMount`

**File:** `+layout.svelte:30–127`. One hook doing store hydration, service-worker registration, offline-engine boot, Firebase analytics + performance init, the consent bridge, and global crash reporting — six unrelated concerns.

Extract to `lib/boot/{analytics,service-worker,crash-reporting}.ts`, each exporting a start function that returns its own cleanup. `onMount` becomes ~8 lines composing them.

### F. Split `SurahReader.svelte` (243 lines)

Extract:
- `ModeTabs.svelte` — the WAI-ARIA roving-tabindex keyboard handler at `:56–69`; reusable and unit-testable
- `FontSizeControl.svelte`
- `PrevNext.svelte` — duplicated in shape at `SurahReader.svelte:218–241` and `RangeReader.svelte:77–102`

### G. One schema for three hand-rolled wire validators

Three ad-hoc parsers cover two types:
- `search.ts:69–96` — API `SearchResponse`
- `worker-client.ts:129–154` — the *same* `SearchResponse`, from the worker
- `manifest.ts:33–41` — `normalizeScript`

Adopt **Valibot** (~1.37 kB gzipped; tree-shakes to a few hundred bytes for schemas this small). Define one schema per wire type in `lib/quran/schemas.ts`, share it across all three call sites, and infer the TS types from it instead of maintaining them separately.

This is the **only** third-party dependency worth adding — see §5.

---

## 3. Smaller fixes

| File | Issue | Fix |
|---|---|---|
| `reader.svelte.ts:260–263` | `Object.keys(bookmarks ? bookmarks : {})` is a dead ternary; the following `.filter()` re-checks what `toggleBookmark` already guarantees | `Object.keys(this.#s.bookmarks)`; convert the getter to `$derived.by` |
| `reader.svelte.ts:286` | `setNote` clones the whole notes object on every keystroke (Svelte-4 habit) | `$state` is deeply reactive — mutate directly |
| `reader.svelte.ts:200–211` | Six boolean getters for two enums; `Sidebar.svelte:183` re-derives `browseJuz ? "juz" : "page"` anyway | Compare the enum directly, or one `is(mode)` method |
| `RangeReader.svelte:39` | `openSurah` uses `goto()` on a `<button>` for what is semantically a link — loses preload, middle-click, open-in-new-tab | Use `<a href={surahPath(g.num)}>`, as `:82` in the same file already does |
| `RangeReader.svelte:44` | `data.kind === "juz" ? 30 : 604` hardcodes range bounds | Move to `quran-meta.ts` |
| `reader.svelte.ts:67, :224` | Font bounds `22`/`56`/step `3` duplicated between validation and mutators | Single `constants.ts`; also collect the 140 ms debounce, 1500 ms flash, and 300 ms tooltip delay (repeated in two files) |
| `manifest.ts:44–73` | `clearTimeout(timer)` is skipped when a fetch throws; the `signal` abort listener is never removed on success | Wrap in `try/finally` |
| `worker-client.ts:16–26` | Eight module-level mutable globals with manually managed `readyResolve[]`/`readyReject[]` arrays that only ever hold one entry | A class with one deferred |
| `worker-client.ts:48` | `request()` has no timeout — a dropped worker message hangs its promise forever; the `error`/`messageerror` handlers only cover load failure | Add a timeout that rejects and clears the pending entry |
| `quran.svelte.ts:24–30` | Public mutable fields that `offline.ts:15` writes directly (`quran.status = "resolving"`), bypassing the invariants `setWorkerStatus` maintains (clearing `download`/`error`) | Private fields + getters; expose intent methods only |

---

## 4. Tests

There are currently none, though the runner is already installed (`node_modules/.bin/vitest`; `vite-plus` ships `dist/test`).

Start with `lib/quran/search/__fixtures__/queries.json`, which is checked in and unused — `normalize.ts` documents itself as the cross-language parity spec the Rust backend must match byte-for-byte, so that fixture is a test suite waiting to be written.

Other high-value, zero-setup targets:
- the persistence validators (after §2B, one helper to test instead of four)
- `parseKey` / `verseKey`
- `nameNumberFallback` (`search.ts:24`)
- `RangeReader`'s surah-grouping logic (`:24–36`)
- `ModeTabs` keyboard handling (after §2F)

---

## 5. Dependency decisions

**Add:** Valibot, for §2G only.

**Do not add any TanStack library.** Each was evaluated against this codebase:

| Library | Finding |
|---|---|
| **Virtual** | `@tanstack/svelte-virtual@3.13.35` is still a Svelte 3/4 **store** adapter — its source imports `derived, writable` from `svelte/store` and returns a `Readable`. Its peer range `^3.48.0 \|\| ^4.0.0 \|\| ^5.0.0` reflects legacy store compat, not runes support. Upstream issue #866 is open, with users reporting empty renders under Svelte 5 and needing a manual `_willUpdate()` hack. Not viable. |
| **Query** | The v6 adapter is fully runes-based and well maintained (peers `svelte ^5.25`), but has almost no surface here: verses come from `page.data` at build time, search goes through a Web Worker, the manifest resolves once at boot. Cache keys, invalidation, refetch-on-focus and dedup have nothing to attach to. The only candidate is `quranSearch()` in `Results.svelte:20–38`; the existing ~19 lines of debounce-and-cancel are adequate. |
| **Pacer** | No Svelte adapter exists — TanStack's own site lists "Svelte Pacer needs a contributor." The core package is pre-1.0. Not worth pre-1.0 API churn to replace 15 lines of `setTimeout`. |
| **DB** | Built for sync engines with optimistic mutations against a server. Bookmarks and notes here are localStorage-only with no server writes, and the corpus sits behind a `postMessage` boundary in sqlite-wasm where live queries cannot reach. |
| **Store** | Exists to give non-reactive frameworks what `$state` classes provide natively. The current store classes are the correct Svelte 5 pattern. Adopting it would be a regression. |
| **Form** | One contact form. Revisit only if it grows real validation. |
| **Table / Router** | No tables; SvelteKit owns routing. |

**Virtualization**, if it ever becomes necessary: the desktop sidebar is `collapsible="offcanvas"` (`sidebar.svelte:64–91`), which keeps its content permanently in the DOM and moves it out of view with CSS — so 114 surah entries, or 604 in page-browse mode, are a fixed cost on every page load, not on open. 114 is modest; revisit only if profiling shows the 604-entry list actually hurts. If so, use `@humanspeak/svelte-virtual-list` (Svelte-5-native, peer `svelte: ^5.0.0`, zero runtime deps) — but note it is pre-1.0.

Never virtualize the verse list: the prerendered verse text is the SEO payload for `/app/[surah]`, and `?verse=N` deep links depend on `ayah-{n}` anchors existing in the DOM (`[surah]/+page.svelte:48`).

---

## 6. Execution order

1. **§1** — `SvelteMap` fix. One line, real bug.
2. **§2B** — extract `createPersisted`; unifies four stores and fixes cross-tab drift.
3. **§2C** — split `reader.svelte.ts`; move copy/share out as pure functions.
4. **§2D, §2F** — collapse `VerseRow`'s action blocks; split `SurahReader`.
5. **§4** — wire up Vitest. Steps 2–4 have produced testable units; start with the parity fixtures.
6. **§2A** — move stores to `createContext`.
7. **§2E, §3** — boot-module extraction and the small fixes.
8. **§2G** — Valibot schemas for the three parsers.

Steps 1–4 are independent of each other and safe to land separately. Step 6 touches every component that reads a store, so land it after the splits in step 3 have settled.
