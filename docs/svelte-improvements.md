
web/src/lib/stores/reader.svelte.ts:111 — the synchronous verse cache is not reactive:

#versesBySurah = $state(new Map<number, string[]>());   // :111
seedSurah(num, verses) { this.#versesBySurah.set(num, verses); }  // :317

$state deep-proxies plain objects and arrays — not Map/Set. That's exactly why svelte/reactivity ships SvelteMap (confirmed in the v5 docs: "A reactive version of the built-in Map… reading contents… will cause it to be re-evaluated as necessary when the map is updated"). So .set() here mutates silently and nothing re-renders.

The visible consequence: Sidebar.svelte:148 iterates reader.versesFor(current.num) in ayah-browse mode. When refreshFromWorker (reader.svelte.ts:326) seeds a surah, that list won't update. bookmarkList (:270) reads through the same cache and has the same problem. Fix is one line: new SvelteMap() from svelte/reactivity.

2. Structural improvements, ranked by payoff

A. Module-singleton stores → context. Every store ends in export const reader = new ReaderStore() (reader.svelte.ts:386, and the same in prefs, consent, quran, notifications). The Svelte best-practices skill is explicit here: prefer createContext over shared-module state, because it scopes state and "eliminate[s] the possibility of it leaking between users when server-side rendering." You prerender today so it isn't live, but it becomes a cross-user data leak the moment any route turns on SSR — and it's what currently makes these components impossible to test in isolation. This is the highest-leverage change in the repo.

B. Four hand-rolled copies of the same persistence layer. reader, prefs, consent, notifications each define STORAGE_KEY, a load() with try/catch JSON parse + field validation, and a persist(). They've drifted: only reader registers cross-tab storage sync (:125), so a theme or consent change in one tab never reaches another. One generic PersistedState<T>(key, version, validate) collapses ~120 duplicated lines and makes the cross-tab behavior uniform.

C. reader.svelte.ts is a god object (386 lines). It owns persistence, navigation, search query, browse mode, typography, bookmarks, notes, a verse cache, worker refresh, and clipboard/share. Split along the seams that already exist as comment banners: bookmarks.svelte.ts, notes.svelte.ts, verse-cache.svelte.ts, typography into prefs, and copyVerse/shareVerse (:349, :363) as pure functions in lib/quran/share.ts — they take a key and touch no store state.

D. VerseRow.svelte:81–159 — four near-identical Tooltip+button blocks. ~20 lines each, differing only in icon, label, handler, and active class. One {#snippet action(...)} or a VerseAction.svelte takes 80 lines to ~25. Highest-density duplication in the codebase.

E. +layout.svelte:30–127 — a ~100-line onMount doing store hydration, service-worker registration, offline-engine boot, Firebase analytics + performance init, the consent bridge, and global crash reporting. Six unrelated concerns. Extract to lib/boot/{analytics,service-worker,crash-reporting}.ts, each returning its own cleanup; onMount becomes ~8 lines.

F. SurahReader.svelte (243 lines) should shed ModeTabs.svelte (the whole WAI-ARIA roving-tabindex handler at :56–69 is reusable and unit-testable), FontSizeControl.svelte, and PrevNext.svelte — that last one is duplicated in shape at SurahReader.svelte:218–241 and RangeReader.svelte:77–102.

G. Three hand-rolled wire-format validators for two types. search.ts:69–96 (API response), worker-client.ts:129–154 (the same SearchResponse from the worker), manifest.ts:33–41 (normalizeScript). This is the one place a tiny schema library genuinely pays — Valibot (~1.5 kB tree-shaken) gives you one schema per wire type, shared by all three call sites, with the TS type inferred from it instead of maintained separately.

H. Smaller, concrete items:
- reader.svelte.ts:260–263 — Object.keys(this.#s.bookmarks ? this.#s.bookmarks : {}) is a dead ternary (bookmarks is always an object), and the .filter() after it re-checks what toggleBookmark already guarantees. Also should be $derived, not an O(n) getter.
- reader.svelte.ts:286 — setNote clones the entire notes object on every keystroke. $state is deeply reactive; mutation works. Svelte-4 habit.
- reader.svelte.ts:200–211 — six boolean getters (browseSurah/Ayah/Juz/Page, isVerseMode/isReadingMode) for two enums; Sidebar.svelte:183 then re-derives browseJuz ? "juz" : "page" anyway.
- RangeReader.svelte:39 — openSurah uses goto() on a <button> for what is semantically a link, losing preload, middle-click and open-in-new-tab. The same file uses real <a href> correctly at :82.
- RangeReader.svelte:44 — data.kind === "juz" ? 30 : 604 hardcodes constants that belong in quran-meta.ts. Same story for font bounds 22/56/3 (duplicated between reader.svelte.ts:67 and :224), the 140 ms debounce, 1500 ms flash, and 300 ms tooltip delay (repeated in two files).
- manifest.ts:44–73 — clearTimeout(timer) is skipped when the fetches throw, and the signal abort listener is never removed on success. Wrap in try/finally.
- worker-client.ts:16–26 — eight module-level mutable globals with manually-managed readyResolve[]/readyReject[] arrays that only ever hold one entry. A class with one deferred. Separately, request() (:48) has no timeout: a dropped worker message hangs its promise forever — the error/messageerror handlers only cover load failure.
- quran.svelte.ts:24–30 — public mutable fields that offline.ts:15 writes directly (quran.status = "resolving"), bypassing the invariants setWorkerStatus maintains (clearing download/error). Two mutation paths, one unsafe.

I. Zero tests, despite vite-plus (Vitest) being in the toolchain and search/__fixtures__/queries.json sitting there unused — with normalize.ts documented as the cross-language parity spec the Rust backend must match byte-for-byte. That fixture file is a test suite waiting to be written. Other free wins: the persistence validators, parseKey/verseKey, nameNumberFallback, RangeReader's grouping logic.

3. TanStack: verified against npm, not from memory

┌────────────────┬───────────────────────────────────────────────────────┬───────────────────────────────────┐
│    Library     │                    Svelte adapter                     │              Verdict              │
├────────────────┼───────────────────────────────────────────────────────┼───────────────────────────────────┤
│ Virtual        │ @tanstack/svelte-virtual 3.13.35                      │ Adopt — narrowly                  │
├────────────────┼───────────────────────────────────────────────────────┼───────────────────────────────────┤
│ Query          │ @tanstack/svelte-query 6.1.38 (peers svelte ^5.25)    │ Skip                              │
├────────────────┼───────────────────────────────────────────────────────┼───────────────────────────────────┤
│ Pacer          │ none published (core @tanstack/pacer 0.21.1, pre-1.0) │ Skip                              │
├────────────────┼───────────────────────────────────────────────────────┼───────────────────────────────────┤
│ DB             │ @tanstack/svelte-db 0.1.94                            │ Skip                              │
├────────────────┼───────────────────────────────────────────────────────┼───────────────────────────────────┤
│ Store          │ @tanstack/svelte-store 0.12.0                         │ No — strict downgrade             │
├────────────────┼───────────────────────────────────────────────────────┼───────────────────────────────────┤
│ Form           │ @tanstack/svelte-form 1.33.2                          │ Skip                              │
├────────────────┼───────────────────────────────────────────────────────┼───────────────────────────────────┤
│ Table / Router │ —                                                     │ No tables; SvelteKit owns routing │
└────────────────┴───────────────────────────────────────────────────────┴───────────────────────────────────┘

Virtual is the one real win, and only for the sidebar. Sidebar.svelte renders all 114 surahs unconditionally (:114), and in page-browse mode all 604 range entries (:181) — each wrapped in SidebarMenuItem + SidebarMenuButton + a child snippet. That's real component-instantiation cost on every sidebar open. Virtualize those two lists.

Do not virtualize the verse list. Al-Baqarah's 286 rows look like the obvious target, but the prerendered verse text is the SEO payload for /app/[surah], and ?verse=N deep-links depend on ayah-{n} anchors existing in the DOM ([surah]/+page.svelte:48). Virtualizing breaks both. The existing content-visibility: auto on VerseRow.svelte:72 is the right tool there — though note it defers paint, not component instantiation, so the four-tooltips-per-row problem (item D above) is ~1,144 bits-ui component instances on that page. Collapsing D is the bigger perf win than any virtualization would be.

Query is a poor fit and I'd skip it. This is an adapter-static prerendered SPA. Verses come from page.data at build time, search goes through a Web Worker, the manifest resolves once at boot. Query's entire value proposition — cache keys, invalidation, refetch-on-focus, request dedup — has almost no surface to attach to. The single candidate is quranSearch() in Results.svelte:20–38, where you'd get dedup of repeated queries plus placeholderData to kill the "Searching…" flicker. That's real but small, and it doesn't justify the dependency or the QueryClientProvider wrapper. Your ~18 lines of debounce-and-cancel there are honestly fine.

Pacer has no Svelte adapter — you'd be importing pre-1.0 core (0.21.1) to replace 15 lines of setTimeout + a cancelled flag. Not worth the API-churn risk.

Store would be a downgrade. It exists to give non-reactive frameworks what $state classes already give you natively, with better ergonomics and no dependency. Your store classes are the correct Svelte 5 pattern; don't replace them.

DB is built for sync engines (Electric et al.) with optimistic mutations against a server. Bookmarks and notes here are localStorage-only with no server writes, and the corpus lives behind a postMessage boundary in sqlite-wasm where live queries can't reach. Overkill.

Form: one contact form. TanStack Form is a heavy abstraction for that; revisit only if it grows real validation.

The honest headline: no third-party library fixes this codebase's actual quality gaps. The problems are the god object, the four duplicated persistence layers, the module singletons, the four-way copy-paste in VerseRow, and the missing test suite. TanStack Virtual helps one list; Valibot helps the three parsers. Everything else on your list is either a no-op or a regression here.
