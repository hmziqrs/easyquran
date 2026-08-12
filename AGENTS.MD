# Hard rules:
- Quran databases are all immutable. No modifications and never versioning.
- No SHA-256 over Quran data in any automated path (build, boot, runtime, ETag, cache, catalogue). A DB's identity is its **id**, never a hash. SHA-256 is manual audit only — three commands a human runs: `just quran-audit`, `pnpm audit:arabic`, `pnpm verify` (in `db/quran/tanzil/translations`). Crypto sha (CSRF, HMAC, Argon2, PKCE) is unrelated and stays. Don't re-add Quran sha — guards in `tests/quran_v1.rs` + `web catalogue-sha-guard.test.ts` fail the build (see docs/quran-system.md — Hard rules).
- Always load and follow the `caveman` skill at the beginning of every session. Keep Caveman full mode active for every response. This rule is also applied to subagents.
- Reader navigation hrefs must preserve translation context: use the `surah*For(ctx,...)` family in `web/src/lib/data/quran.ts` (`surahPathFor`/`surahLocalPagePathFor`/`surahAyahPathFor`/`globalPagePathFor`/`juzPathFor`) with `ctx` from `surahRouteContext(sourceId)` or `routeContextFromParams(page.params)`. Never call the Arabic-only `surahPath`/`surahLocalPagePath`/`surahAyahPath` from a component, and never hand-build `/app/` URL strings. Machine-guarded by `web/src/routes/(application)/app/__tests__/nav-guard.test.ts` (`pnpm test`).
- Delivery divergences from `docs/my-plan-raw.md`: translated reader routes (`/app/t/**` and `/app/[surah]/t/**`) are SSR + 7-day disk cache — never SSG; prerender stays Arabic-only, and production Quran bytes/size/path come from baked id maps, never remote metadata. Full record: `docs/quran-system.md` Part 3 (Divergences from `my-plan-raw.md`).
- All web keyboard shortcuts (cmdk, reader nav, any key-based action) use **TanStack Hotkeys** (`@tanstack/hotkeys`, pinned `0.8.0` — alpha) via the shared wrapper `web/src/lib/hotkeys.svelte.ts` (`registerHotkey()` over the `HotkeyManager` singleton). Never hand-roll `keydown`/`addEventListener`/`metaKey`/`isPaletteChord`-style handlers for app shortcuts. The library MUST be dynamically imported (`await import("$lib/hotkeys.svelte")`) from its consumer — never statically imported into an always-mounted component — so it stays out of the initial bundle. Gotcha: TanStack does not guard IME composition; any shortcut clashing with an IME session keeps its own `event.isComposing` early-return. Register from a client-only Svelte `$effect`, `unregister()` on cleanup.

# Dynamic context loading
Load relevant context ONLY when working on that area:

- **Rust backend** (`rust/backend/api/`): load `rust/backend/api/AGENTS.md` + `rust/backend/api/README.md` if exists.
- **Web** (`web/`): load `web/README.md` + Svelte skills (`svelte-core-bestpractices`, `svelte-code-writer`).
- **DB/migrations**: load `rust/backend/api/migration/README.md`.
- **Deploy**: load `deploy/README.md`.

Do NOT preload all context every session. Load on-demand based on files touched.


