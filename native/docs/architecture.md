# Architecture

nsh-starter is a [Native Shell](https://nsh.dev) (NSH) 0.5.1 application boilerplate. One Rust crate at the repo root is the UI and most features; `packages/remote-screen` is a TypeScript (TSX) feature delivered as a signed package. All async state runs on `nsh-query`, all forms on `nsh-form`, both as path dependencies from `../nsh-query` and `../nsh-form`. There is no server: `src/services/fake_api.rs` is the backend, with latency and failure switches, so the data screens run offline.

## Mode and entry points

`nsh.json` says `"rust": "."` and `"root": "App"`: the app is a Rust app whose root component is `App`, exported to the TSX side through `#[native_module("nsh-starter/native")]` in `src/lib.rs`. What runs where:

- **macOS/Linux/Windows** — `src/main.rs` calls the platform crate's `run_app` with `Root::of::<App>()`. Running the binary directly on a non-desktop target is a compile error; mobile and web go through the dev host.
- **iOS, Android, web** — `nsh run --target ios|android|web` boots the development host and mounts the same `App` root. The host scaffolding a Rust app needs per target is generated into `.nsh/` (`nsh` writes it; `.nsh/` is git-ignored) — the repository keeps no hand-written TSX except tests and the remote-screen package.
- **Packaged** — `nsh package --target macos|ios-simulator|android|web` builds an installable app from the prebuilt runtime; no Xcode project and no Gradle are involved. Anything needing entitlements, URL schemes or extra native SDKs goes through an embedding host (`NshAppView` + `app.handle(..)` on iOS, `NshView` + `nsh.handle(..)` on Android), with the called names declared under `capabilities.host`.

## Module map

Everything lives under `src/`. Arrows name the modules a module builds on.

| Module | Contents | Depends on |
|---|---|---|
| `main` | Desktop entry: `env_logger`, `run_app`, window title and app id. | `shell` (via the root) |
| `lib` | Crate root: `i18n!` block, module tree, the native module registry. | everything |
| `app` | Composition owned by the shell: providers, appearance, window assembly. | `shell`, `state`, `ui` |
| `shell` | Lifecycle, the typed route enum, `nav_stack`, deep-link parsing, sidebar/tabs, title bar and menus, status bar, command palette, shortcuts, undo/redo. | `state`, `ui`, `features` |
| `features/forms` | Sign-up and settings screens on `nsh-form`. | `ui`, `services` |
| `features/data` | Feed, detail, query devtools on `nsh-query`. | `ui`, `services::fake_api`, `state` |
| `features/lists` | Tables, tree, calendar, grid, chat, carousel, pagination. | `ui` |
| `features/canvas` | Drawing app, animated canvas, plots, shader, SVG, media, `web_view`. | `ui` |
| `features/motion` | Transitions, springs, gestures, scroll-linked effects. | `ui` |
| `features/overlays` | Dialogs, sheets, popovers, menus, toasts, dock, editor, diff. | `ui` |
| `features/remote_screen` | The page that loads and controls the signed TSX package. | `ui` |
| `ui` | One wrapper per `nsh_base` component in use; screens import from here only. | `themes/theme.json` tokens |
| `services` | `http`, `auth`, `analytics`, `monitoring` seams (foundation) and `fake_api` (data). | `foundation` |
| `state` | Stores, appearance and locale signals, undo/redo history types. | `foundation` |
| `foundation` | Errors, ids, time, validation, config + migrations, event bus, task manager, capability registry, first run, file logging, diagnostics page, error surface. | — |
| `platform` | Safe areas, keyboard, clipboard, picker, speech, file dialogs, `nsh/mobile`, `nsh/desktop`, i18n, accessibility, capabilities in `nsh.json`. | `foundation`, `ui` |

The layout gives each area one integration test file under `tests/` (`shell.rs`, `foundation.rs`, `ui.rs`, `forms.rs`, `data.rs`, `lists.rs`, `canvas.rs`, `motion.rs`, `overlays.rs`, `platform.rs`, `remote_screen.rs`) — planned until each area lands its file; `docs/STATUS.md` accounts for what exists. The data area also records its `--trace` runs under `trace/` (same: planned until it lands).

## Shell routing

The route enum in `src/shell` is the single source of navigation. Every page is a variant; deep links parse into the same enum, so an external open and a sidebar click take one path. Pages mount through `nav_stack` (native stack on iOS and Android: swipe back, system header, predictive back; a stack with a toolbar on desktop). `pop_on_back` decides whether the back gesture pops the route; Android back runs through the nav stack's back order, with the dirty-form guard ahead of it.

`NavPage`'s `header` slot is the app's desktop/web header on every page (those platforms have no system header); `headerLeft`, `headerRight` and `headerTitle` feed the system header on iOS and Android. The desktop shell adds a title bar, application menus, the command palette (built on `nsh_base`'s `Command`), keyboard shortcuts and an undo/redo stack whose history types live in `src/state`. The status bar shows route, running background tasks, connectivity and the last error.

## State and queries

Two layers, both signals all the way down.

**App state** (`src/state`, `src/foundation`): plain `signal`/`computed` values plus the event bus, the background task manager, the capability registry and typed config persisted through NSH's `Storage` with schema migrations. One NSH constraint shapes this layer: `Signal::update` needs `T: Clone`, so state machines holding user types pair `stored(..)` with a version signal rather than mutating in place. A component's signals are already disposed when its `cleanup` runs, so code that must outlive a component (caches, in-flight writes) captures what it needs up front or uses `create_root`.

**Async state** (`nsh-query`): every server-shaped read is a `use_query*` with a typed key, a `CachePolicy` (TTL, stale-while-revalidate, none) and a `RetryPolicy`; writes are `use_mutation*` that invalidate keys on success. The feed is `use_infinite_query` over `virtual_list`; the optimistic like writes `query.set_data` and rolls back on failure. The query client is shared; the devtools page lists its cache entries. The fetcher calls `src/services/fake_api.rs`, never a real network.

**Forms** (`nsh-form`): `use_form` owns one value type per form; `field!` binds fields with `rules` (required, email, length, async checks with debounce); `form.submit` maps server failures to field errors with `SubmitError`; dirty state drives the back guard (`pop_on_back`) and draft autosave; the settings form hydrates with `form.load` instead of initializing constants.

## UI layer policy

`themes/theme.json` defines the semantic tokens, including `destructive-foreground`, which NSH's default theme lacks. `src/ui` holds one wrapper per `nsh_base` component in use, styled from those tokens; screens import from `src/ui` only — never from `nsh_base` directly, and no raw hex or `rgb(` outside the theme file (data colors excepted, and each one explained). The wrappers carry the states (hover, focus ring, press feedback, disabled), and the gallery page renders every control in light and dark at both widths. Density, navigation and overlays adapt per platform; the semantics stay shared.

## Service seams

Each seam is a trait with a local implementation, so the real library from the parallel projects swaps in by changing one file:

| Seam | Local implementation | Swapped by |
|---|---|---|
| `services/http.rs` — one small trait over NSH `fetch` | local requester with injected latency/failures | the nsh-http project |
| `services/auth.rs` — session state machine | mock provider, no real account | the nsh-auth / nsh-firebase projects |
| `services/analytics.rs` — event sink | recording sink the diagnostics page can show | analytics backend |
| `services/monitoring.rs` — error/trace sink | in-memory sink | monitoring backend |

System notifications sit behind a trait whose stub reports them unavailable, while the inbox and toast surfaces are real. The stub dates from 0.4.0, which had no `Notifications` module; 0.5.0 ships one, and wiring it in behind the trait is a follow-up. The capability registry records every seam's status, and the diagnostics page renders it — nothing degrades silently.

## i18n

`i18n!` in `src/lib.rs` loads `i18n/en.yml` and `i18n/zh-CN.yml` at compile time; `t!` reads them with `en` as fallback. Locale is a state signal the settings screen switches; RTL checks run against the layouts. New user-facing strings go through `t!` in both locales.

## The signed package path

`packages/remote-screen` proves the delivery path a real app uses for code that changes independently of the binary:

1. **Develop** — TSX importing `nsh`, `nsh-query`, `nsh-form`. The TypeScript libraries reach the bundler through `node_modules` symlinks to the sibling checkouts (`mkdir -p node_modules && ln -s <path> node_modules/<name>`); `tsconfig.json` needs no change, and overriding `compilerOptions.paths` there breaks the `nsh` types.
2. **Gate** — `nsh check`, `nsh build` with no warnings, `nsh test` (the `test-tsx` recipe). `If` and `Each` are globals in TSX: importing them from `nsh` is a type error. `nsh check` needs `App.tsx` at the package root, or an entry passed to it.
3. **Sign** — `nsh keygen` makes a throwaway key pair at run time (never committed), `nsh certify` certifies it for this app id and a time window, `nsh publish` builds and signs the `.nshpkg`.
4. **Deliver** — the app verifies with `nsh verify` semantics against its trusted root and loads the package on demand. Bytecode loads only from the app bundle, the APK or a verified package. `nsh revoke` signs a revocation list; the app shows the revocation path.
5. **Fail closed** — host calls reject with `ERR_ACCESS_DENIED` when the capability is undeclared, `ERR_NSH_HOST_ERROR` when unhandled or thrown, and `ERR_FEATURE_UNAVAILABLE_ON_PLATFORM` with no host.

Capability keys declared in `nsh.json` come from the real 0.5.0 set: `windows`, `network`, `webview`, `statusBar`, `clipboard`, `picker`, `speech`, `notifications`, `audio`, `openUrl`, `process`, `fs`, `host` — kept tight. Since 0.5.0 a typo is only a warning from `nsh build` (the key is not granted), so read the build output.

## Testing

Four layers, all through the UI:

- **cargo** (`tests/<area>.rs`): headless engine tests with `nsh-testing`, manual clock, no real time. `Headless::start` can return before the first frame, so tests `wait_for` something first; `Headless::shows` takes plain text while `wait_for`/`press` take selectors.
- **TSX headless** (`*.test.tsx`): `nsh test` runs each file's tests with an engine per test after `cargo test`. `App.test.tsx` is the root smoke.
- **Per-target smoke**: `nsh test --target macos|ios|android` opens every page and screenshots it; web runs what the web supports and lists the rest.
- **Budgets**: `nsh test --trace` shows mount batches; a tap on the feed's like button costs a handful of mount items and scrolling keeps `nodes` flat.

## Adding a page

1. Add the route variant in `src/shell` (enum, deep-link arm, palette entry).
2. Build the screen in your area under `src/features/<area>`, importing from `src/ui` only.
3. Cover it in your area's `tests/<area>.rs` and the smoke list; add strings to both locales.
