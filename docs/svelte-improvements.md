# Web code quality plan

Scope: `web/` — SvelteKit 2 + Svelte 5 in runes mode, using `adapter-static` and prerendered routes.

Last verified: **2026-07-31**, against the installed Svelte 5.56.8, SvelteKit 2.70.1, and Bits UI 2.18.1. Reconfirm line references and dependency status before implementing because the Svelte and TanStack packages are moving quickly.

The goal is maintainability, correctness, and testability. A dependency should be added only when it removes meaningful application complexity or provides behavior that would be risky to maintain locally. No performance change should be described as an improvement until it is measured in this application.

---

## Progress (updated 2026-08-01)

Steps 1–3 of the execution order (§8) are **complete and verified**: `svelte-check` 0/0, `vp lint --deny-warnings` 0 warnings, `vp fmt --check` clean, 22 Vitest tests pass, `vp build` succeeds. Steps 4–11 remain.

| # | Step | Status |
|---|---|---|
| 1 | SvelteMap fix + regression test | ✅ |
| 2 | Quality gates | ✅ (local scripts — **no CI**) |
| 3 | Harden async boundaries | ✅ |
| 4 | Persistence foundation + debounce notes | ☐ next |
| 5 | Split reader, remove dead state | ☐ |
| 6 | shadcn-svelte Tabs | ☐ |
| 7 | VerseRow actions | ☐ |
| 8 | Factories / contexts | ☐ |
| 9 | Boot services | ☐ |
| 10 | Centralize wire schemas | ☐ |
| 11 | Profile sidebar / virtualization | ☐ |

**No CI.** This project does **not** use GitHub Actions or any CI. The quality floor is enforced by running the local `web/package.json` scripts (`format:check`, `lint`, `check`, `test`, `build`) before commit — not by a workflow. The "add CI / make CI run" guidance elsewhere in this document is superseded by this decision.

Corrections found while implementing (re-audit):
- **§A:** confirmed empirically — `$state(new Map())` does **not** make a `Map` reactive; `SvelteMap` is required. (A prior attempt with `$state(new Map())` shipped a no-op fix; the regression test catches this.)
- **§B:** `worker-client.ts` already had a `failAll` path for worker load/eval failure; the real gaps were the missing per-request timeout, no disposal, and the post-init `fatal` branch not clearing `pending`. All fixed. The "eight module-level mutable variables" was actually 7 `let` + 3 const-mutable collections; the dead resolver arrays were removed.
- **§E / §5:** range-path literals `/app/${kind}/${index}` appear in **7 files**, not just `Sidebar.svelte` — step 5 needs a real `rangePath()` helper. The project uses **shadcn-svelte** (which wraps Bits UI), so step 6 should add the shadcn `tabs` component rather than importing `bits-ui` directly.
- **§1:** all `localStorage.setItem` calls are already guarded; the only unvalidated `JSON.parse` left is the anti-FOUC inline script in `app.html`.

---

## 1. Current quality baseline

The project compiles cleanly, and its automated quality floor is now in place (raised by step 2):

- `pnpm check`: **0 Svelte errors and 0 warnings**
- `vp lint` (`--deny-warnings`): **0 warnings**
- `vp fmt --check`: **clean**
- web tests: **22 passing** (Vitest; config in `web/vitest.config.ts`)
- CI: **none, by decision** — the floor runs as local `pnpm` scripts, not a workflow

The commands are reproducible in `web/package.json` (`check`, `lint`, `format`, `format:check`, `test`, `test:watch`, `build`). The first objective is not an arbitrary coverage percentage; it is regression protection for the state and persistence code that will be changed below.

---

## 2. Immediate correctness fixes

### A. Replace the non-reactive verse `Map` ✅

**File:** `web/src/lib/stores/reader.svelte.ts:111`

```ts
#versesBySurah = $state(new Map<number, string[]>());
```

`$state` deeply proxies arrays and simple objects, but it does not make a native `Map` reactive. `seedSurah()` mutates the existing map with `.set()`, so consumers of `versesFor()` are not notified.

Use Svelte's reactive collection implementation:

```ts
import { SvelteMap } from "svelte/reactivity";

#versesBySurah = new SvelteMap<number, string[]>();
```

The visible regression is the Sidebar ayah list remaining empty after a cold surah load. `bookmarkList.text` has the same latent problem, although `bookmarkList` is not currently rendered anywhere.

**Verification:** cold-load a surah, switch the Sidebar to Ayah mode, and confirm that the list populates without another navigation or unrelated state update. Add a regression test around cache seeding and reactive consumption.

### B. Harden asynchronous boundaries ✅

These are correctness improvements, not cosmetic refactors:

- `worker-client.ts`: add a timeout per request, clear the pending entry on timeout, and reject all pending requests on worker failure or disposal.
- `manifest.ts`: always clear the timeout and detach the caller abort listener in `finally`, or use composed timeout/abort signals where supported by the browser target.
- `search.ts`: do not call `String()` on an unknown `key`; validate that it is a string. The current coercion can turn an object into `"[object Object]"`.
- `static/sw.js` and `vite-plugin-quran.ts`: resolve the current floating-promise warnings with explicit error handling or an intentional `void`.

Prefer a small `QuranWorkerClient` class or factory with an explicit lifecycle over eight module-level mutable variables. A single deferred is sufficient for readiness; the current resolver arrays only ever represent one start operation.

---

## 3. Add tests before structural refactors

Vitest is already available through the workspace tooling and is the recommended unit/component runner for a Vite-based Svelte project.

Start with pure, high-value tests that require no browser environment:

- `lib/quran/search/__fixtures__/queries.json` against `normalize.ts`
- `parseKey` / `verseKey`
- `nameNumberFallback`
- range grouping extracted from `RangeReader.svelte`
- persistence decoders and schema-version behavior
- manifest response decoding
- Worker request settlement, timeout, and failure behavior with a fake Worker boundary

Then add browser/component coverage for:

- the cold-load `SvelteMap` regression
- reading-mode keyboard and ARIA behavior
- note autosave and persistence timing
- Sidebar mode switching and links

Scripts for `test`, `test:watch`, `lint`, `format`, and `format:check` are in `web/package.json`. ✅ Done. Run them locally before commit — there is **no CI** by project decision. Vitest is configured in `web/vitest.config.ts` (happy-dom + `resolve.conditions: ["browser"]` so `SvelteMap` and effects behave as in the browser).

To run the floor locally:

1. `pnpm format:check`
2. `pnpm lint` (fails on any warning)
3. `pnpm check`
4. `pnpm test`
5. `pnpm build`

Tests should land immediately after the `SvelteMap` fix and before persistence or ReaderStore is split.

---

## 4. Structural refactors, ranked by payoff

### A. Extract a policy-aware persistence foundation

`reader`, `prefs`, `consent`, and `notifications` repeat JSON parsing, validation, localStorage error handling, and hydration. Extract reusable storage mechanics, but do not force every domain through identical scheduling or side effects.

A useful boundary owns:

- SSR/browser guards
- safe JSON reading and writing
- schema-version checking
- decoding `unknown` into validated domain data
- optional subscription to the cross-tab `storage` event
- explicit teardown of listeners

The domain store still decides when and why to persist:

- **notes:** debounce writes because `localStorage` is synchronous and notes change on every keystroke
- **preferences:** update the DOM immediately; persistence can be immediate
- **consent:** persist immediately, then apply the Firebase consent bridge
- **notifications:** persistence is coupled to token registration and revocation; an external-tab update may require more than assigning fields

Cross-tab behavior should therefore be supported by the foundation but opted into and handled per store. Avoid a configurable “god helper” whose options become harder to understand than the duplicated code it replaces.

### B. Split `reader.svelte.ts` by cohesive responsibility

The file is 386 lines and currently combines transient UI state, durable settings, annotations, verse caching, navigation bookkeeping, persistence, and browser sharing.

A practical split is:

- `reader-session.svelte.ts` — query, browse mode, open note, and navigation token
- `reader-settings.svelte.ts` — font size and reading mode
- `annotations.svelte.ts` — bookmarks, notes, last-read data, and their persistence
- `verse-cache.svelte.ts` — `SvelteMap` plus Worker refresh coordination
- `quran/share-text.ts` — pure verse-share text formatting
- `quran/web-share.ts` — clipboard and Web Share API side effects

Do not automatically merge Arabic font size into the global appearance store. It is a reader-specific setting and can remain independently hydratable.

Remove unused state and public API before moving it. At the time of this audit, persisted `current`, the public `current`/`surah` getters, `surahCount`, `fontSize`, `bookmarkList`, and `bookmarkCount` have no consumers. If an item is retained for an imminent feature, document that consumer explicitly.

Directly mutate deeply reactive note and bookmark records when it improves clarity; cloning is not required for Svelte 5 reactivity. The larger note-performance win is debounced persistence, not the removal of one object spread.

### C. Introduce factories first, contexts selectively

The module singletons are **not a current cross-user security vulnerability**. The application is statically prerendered, and its user-specific mutations occur in browser lifecycle code. A shared module becomes dangerous if request-specific data is written to it during SSR; merely enabling an SSR route does not automatically leak the current browser-only state.

Still, store factories improve unit testing and allow isolated app instances. Introduce `createReaderState()`, `createPreferences()`, and similar factories while splitting the domains. Use Svelte's type-safe `createContext` for state that:

- may become request-scoped under SSR
- needs replacement in component tests
- should have multiple provider instances
- belongs to a specific subtree rather than the whole browser session

Keep genuinely process-wide browser services, such as a single Worker or FCM lifecycle manager, as explicit services unless contextual scoping provides a concrete benefit. Do not migrate every singleton in one high-churn change.

`createContext` requires Svelte 5.40 or newer; the current dependency range satisfies that requirement.

### D. Break up root boot orchestration

**File:** `web/src/routes/+layout.svelte`

The root `onMount` hydrates stores, registers the service worker, downloads the offline Quran corpus, initializes analytics/performance, bridges consent, and installs crash reporting.

Extract lifecycle-owned services such as:

- `lib/boot/service-worker.ts`
- `lib/boot/analytics.ts`
- `lib/boot/crash-reporting.ts`
- `lib/boot/offline-engine.ts`

Each `start` function should return its cleanup so asynchronous listeners, including the consent bridge, can be removed reliably.

Do not automatically download the approximately 2.5 MB Quran corpus on every marketing-page visit. Start the offline engine when the user enters `/app`, or deliberately schedule a preload after a clear product signal. This reduces background bandwidth, battery use, and competition with resources the marketing page actually needs while preserving prerendered first paint in the reader.

### E. Use existing accessible primitives in `SurahReader`

Before extracting the custom WAI-ARIA tab code into `ModeTabs.svelte`, replace it with the already-installed Bits UI Tabs primitive. Bits UI handles controlled values, orientation-aware keyboard navigation, automatic activation, and panel labelling.

This also fixes the current panel always using `aria-labelledby="mode-verse"`, including while Reading mode is selected.

Other focused extractions remain reasonable:

- `FontSizeControl.svelte`
- `PrevNext.svelte`, shared with `RangeReader.svelte`

Do not create components solely to reduce line count; extract when the result has a clear API, independent behavior, or meaningful test surface.

### F. Deduplicate `VerseRow` actions for readability

The four Tooltip action blocks in `VerseRow.svelte` repeat nearly identical markup. Replace them with a local snippet or a small `VerseAction.svelte` API.

This is a readability and consistency improvement. It does **not** reduce the number of Tooltip roots rendered per verse, and a child component adds another component boundary. Treat any runtime-performance claim as unproven until it is profiled.

The existing `content-visibility: auto` reduces offscreen layout and painting while leaving content in the DOM. Retain it and benchmark before introducing more complex rendering behavior.

### G. Centralize wire schemas

`search.ts`, `worker-client.ts`, and `manifest.ts` maintain overlapping manual decoders. Define each wire shape once and infer its TypeScript output from the runtime schema.

**Valibot is a reasonable, low-risk candidate** because it is stable, dependency-free, modular, and tree-shakable. Its actual contribution must be measured in this production bundle; a generic package-page gzip figure is not an application measurement.

This dependency is justified for correctness and developer experience, not as a direct runtime-performance optimization. A centralized handwritten decoder is also acceptable if the team prefers zero dependencies, provided it is shared and thoroughly tested.

---

## 5. Smaller fixes

| File | Finding | Recommendation |
|---|---|---|
| `reader.svelte.ts` | Dead bookmark ternary and unused bookmark list/count API | Remove unused API, or simplify and memoize only when a real consumer exists |
| `reader.svelte.ts` | Notes and bookmarks clone reactive records | Mutate directly where clearer; debounce note persistence |
| `reader.svelte.ts` | Multiple boolean getters mirror two enums | Compare the enum directly or expose one `is(mode)` helper |
| `RangeReader.svelte` | A button plus `goto()` performs link navigation | Use a real `<a href={surahPath(...)}>` for preload and browser link affordances |
| `RangeReader.svelte` | Juz/page maxima are hardcoded | Move the bounds to Quran metadata/constants |
| `reader.svelte.ts` | Font bounds and timing values are duplicated | Define named domain constants near their owning feature |
| `manifest.ts` | Timeout and abort listener cleanup is incomplete | Use `finally` or composed abort signals |
| `worker-client.ts` | Requests can remain pending forever | Add timeout, disposal, and deterministic rejection |
| `quran.svelte.ts` | Public fields allow invariant-bypassing writes | Use private fields, getters, and intent methods |
| `Sidebar.svelte` | Range links build literal `/app/...` URLs | Keep route construction centralized and compatible with the project's base-path policy |
| `+layout.svelte` | Async consent listener is not included in cleanup | Return and compose cleanup functions from the extracted service |

---

## 6. TanStack and third-party dependency decisions

The decision is **not** “TanStack is bad for Svelte.” Several Svelte adapters are current and capable. The decision is that this application does not yet have the problems most of those libraries solve.

| Library | Current decision | Evidence and future trigger |
|---|---|---|
| **Query** | Do not add now | The v6 Svelte adapter is runes-native and actively maintained. Current verses are prerendered, search is Worker-first, and there is no remote bookmark/note state to invalidate or reconcile. Revisit when authenticated server reads and mutations exist. |
| **DB** | Do not add now; reevaluate for local-first sync | TanStack DB now supports local-only and localStorage collections, schema validation, optimistic updates, and automatic cross-tab synchronization. The earlier claim that it only fits sync engines was incorrect. It remains a 0.x/beta collections/live-query/transaction system and is excessive for a few small preference and annotation records. Revisit when bookmarks and notes become normalized, synchronized, offline-first records. |
| **Store** | Do not add | The current Svelte adapter is rune-friendly and provides selectors; it is not merely a workaround for non-reactive frameworks. This app has no cross-framework portability requirement, and Svelte's fine-grained rune classes already provide the needed reactivity with less indirection. |
| **Pacer** | Do not add | It is beta and has no Svelte adapter. One search debounce and one note autosave do not justify adopting the vanilla scheduling model. Revisit if the app develops several queues, rate limits, batches, or cancellable async scheduling flows. |
| **Form** | Do not add | A Svelte adapter exists, but the current contact page intentionally has no form. Revisit for a genuinely complex form with nested values, async validation, or reusable field composition. |
| **Virtual** | Do not add without a measured problem and a spike | `@tanstack/svelte-virtual@3.13.35` still imports `svelte/store` and returns a `Readable`. Its current reference documentation says it returns a direct Virtualizer while its source and examples use a store, creating an API-documentation mismatch. The latest source already calls `_willUpdate()`, so the historical manual-workaround report should not be presented as definitely required by the latest version. |
| **Table** | Do not add | The application has no data-table surface. |
| **Router** | Do not add | SvelteKit owns routing, loading, prerendering, and navigation. |

### Virtualization policy

The default Sidebar mounts 114 simple surah entries. The 604-entry page list is mounted only when Page browse mode is selected; it is not the default cost on every page load. Profile DOM size, scripting, rendering, INP, and memory on representative mobile hardware before adding virtualization.

If measurement proves virtualization necessary, run a small comparison spike covering:

- the current implementation
- `virtua/svelte`
- `@humanspeak/svelte-virtual-list`
- the latest `@tanstack/svelte-virtual`

Test dynamic heights, keyboard access, scroll restoration, route changes, mobile Safari, and bundle output. Both Svelte-focused alternatives are still pre-1.0, so a framework-native API alone is not enough to select them.

Do not virtualize the verse body at present. The largest surah is modest, `content-visibility` already reduces offscreen rendering cost, and virtualization would complicate SEO/prerendered text, find-in-page, selection, accessibility, and `ayah-{n}` deep-link anchors. This is a current product decision rather than a claim that verse virtualization can never be implemented safely.

---

## 7. Dependency decision

**Potentially add:** Valibot, narrowly for shared persistence and wire schemas, after confirming the generated bundle and agreeing that a maintained schema is preferable to shared handwritten decoders.

**Do not add now:** TanStack Query, DB, Store, Pacer, Form, Virtual, Table, or Router.

This decision should be revisited when product requirements change:

- remote authenticated reads/mutations → evaluate Query
- normalized local-first synchronized annotations → evaluate DB
- several complex forms → evaluate Form
- measured long-list rendering bottleneck → evaluate Virtual and Svelte-native alternatives
- multiple scheduling/queueing workflows → evaluate Pacer core

---

## 8. Recommended execution order

1. **Fix the `SvelteMap` bug** and add its regression test. ✅
2. **Establish quality gates**: formatting, lint with no warnings, `svelte-check`, Vitest, build (local scripts — no CI). ✅
3. **Harden boundaries**: API decoding, manifest cleanup, Worker timeout/failure/disposal. ✅
4. **Extract persistence mechanics** with domain-specific scheduling; debounce note writes.
5. **Split Reader state** and remove unused persisted/public state before introducing new abstractions.
6. **Replace custom mode tabs with Bits UI Tabs**, then extract focused controls where useful.
7. **Refactor `VerseRow` actions** for readability, with no assumed performance claim.
8. **Introduce store factories and selective contexts** after domain boundaries have settled.
9. **Extract boot services** and start the offline corpus download from the application experience rather than every marketing route.
10. **Centralize wire schemas**, adding Valibot only after measuring the actual bundle result.
11. **Profile the Sidebar**; adopt virtualization only if the measurements justify its UX and maintenance costs.

Each step should be independently reviewable. Avoid mixing the state split, context migration, UI extraction, and a new third-party state library into one change.

---

## 9. Primary references

- [Svelte `$state` and built-in classes](https://svelte.dev/docs/svelte/%24state)
- [Svelte reactive collections (`SvelteMap`)](https://svelte.dev/docs/svelte/svelte-reactivity)
- [Svelte context and replacing global state](https://svelte.dev/docs/svelte/context)
- [SvelteKit state management and SSR guidance](https://svelte.dev/docs/kit/state-management)
- [Svelte testing guidance](https://svelte.dev/docs/svelte/testing)
- [Bits UI Tabs](https://bits-ui.com/docs/components/tabs)
- [Web Storage synchronization and performance characteristics](https://developer.mozilla.org/en-US/docs/Web/API/Web_Storage_API)
- [TanStack Query Svelte v6 migration](https://tanstack.com/query/latest/docs/framework/svelte/migrate-from-v5-to-v6)
- [TanStack DB localStorage collections](https://tanstack.com/db/latest/docs/collections/local-storage-collection)
- [TanStack Store Svelte quick start](https://tanstack.com/store/latest/docs/framework/svelte/quick-start)
- [TanStack Pacer supported frameworks](https://tanstack.com/pacer/latest/docs/framework)
- [TanStack Form supported frameworks](https://tanstack.com/form/latest/docs/framework)
- [TanStack Svelte Virtual source](https://github.com/TanStack/virtual/blob/main/packages/svelte-virtual/src/index.ts)
- [Valibot v1 bundle design](https://valibot.dev/blog/valibot-v1-the-1-kb-schema-library/)
- [`content-visibility` behavior](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/content-visibility)
