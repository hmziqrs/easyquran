# gpui-starter parity

This repo mirrors `gpui-starter` (a desktop GPUI boilerplate) feature for feature where NSH gives the idea a place to live. This page says, for each feature of that boilerplate, what the NSH version here is — or why NSH makes it meaningless. The source list is gpui-starter's README feature list and its `docs/desktop-boilerplate-features-plan.md`.

Legend: **ported** — a real implementation exists in this app; **adapted** — the same job, done the NSH way; **out** — NSH's model removes the problem.

## Shell and lifecycle

| gpui-starter | Here |
|---|---|
| Lifecycle state machine (startup, shutdown, crash handling, first-run detection) | **ported** in `src/shell` + `src/foundation`: lifecycle stage tracking, panic capture into file logs, first-run flow persisted in typed config. |
| Routed pages with deep links (`gpui-starter://settings`) | **ported** in `src/shell`: a typed route enum; deep links parse into the same enum through `nav_stack`, which gives them the native back stack on iOS and Android for free. |
| Command palette (Cmd+K, fuzzy search) | **ported**: `nsh_base`'s `Command` component driving the same registry the menus and shortcuts use. |
| Undo/redo stack (Cmd+Z / Cmd+Y) | **ported** in `src/state` + `src/shell`: history types next to the stores, palette and menu entries wired to them. |
| Event bus (typed `AppEventQueue`, emit/drain) | **ported** in `src/foundation`: signals and an app event bus; cross-module state never goes through one-off globals. |
| Custom title bar with drag regions | **adapted**: the desktop shell builds its title bar area and every page's `header` slot; mobile gets the system header through `NavPage` instead of a hand-rolled bar. |
| Status bar (route, tasks, degraded state, last error) | **ported** in `src/shell`. |
| Application menus, keyboard shortcuts | **ported**: `nsh/desktop` `Menu` plus the shared command registry; shortcuts through the keymap. |
| Single-instance guard + IPC deep-link forwarding | **out**: a desktop-process concern. NSH apps ship as packaged apps on four targets; the boilerplate's shell covers deep links at the app layer, and the packaging model had no second-instance IPC surface on 0.4.0. 0.5.0 adds single-instance mode (`next_second_instance`); adopting it is a follow-up. |
| Android back order | **adapted** (gpui-starter has no mobile story): back order through the nav stack with `pop_on_back` and the dirty-form guard. |

## State, persistence, data

| gpui-starter | Here |
|---|---|
| JSON config with schema migrations, atomic writes, quarantine | **ported** in `src/foundation`: typed config in NSH `Storage` with schema migrations and recovery; the diagnostics page shows the migration result. |
| SQLite persistence (`rusqlite`) | **adapted**: NSH apps get `Storage` from the runtime, so the boilerplate demonstrates typed config + query cache rather than embedding a second database; a document saying when to reach for one lives in the architecture page if a real app needs it. |
| gpui-query async state (TanStack-style) | **ported** as `nsh-query`: caching, retries, invalidation, optimistic updates, infinite lists — with the feed, detail and devtools pages as the demo. |
| Window bounds persistence | **out**: window frames are the host's business on NSH; `nsh/desktop` `Window` manages them, and there is nothing app-side to persist. |
| Session/account placeholder | **adapted**: `src/services/auth.rs` holds the session state machine with a mock provider; the real provider swaps in from the nsh-auth project by replacing one file. |
| Secure storage boundary (`keyring`) | **adapted**: secrets are never persisted in this sandbox (no accounts at all); the auth seam documents where the real provider would store tokens. |

## Observability and support surface

| gpui-starter | Here |
|---|---|
| File logging (`tracing-appender`) | **ported** in `src/foundation`: file logging with the path surfaced on the diagnostics page. |
| Telemetry with off/local/remote sinks + consent gate | **adapted**: `src/services/analytics.rs` and `src/services/monitoring.rs` are sink traits with local implementations; remote sinks arrive with the analytics/monitoring projects and keep the same consent gate. |
| Diagnostics page (subsystem status, log path, actions) | **ported** in `src/foundation`. |
| Central error surface (structured errors, retry/dismiss) | **ported**: structured errors surfaced through toasts (`Toast`/`ToastStack`) and the error surface, never panics. |
| Background task manager with drain-on-shutdown | **ported** in `src/foundation`; the status bar shows running tasks. |
| Capability registry (supported/enabled/degraded/reason) | **ported** in `src/foundation`, backed by `nsh.json` capabilities — NSH already gates native surfaces (network, webview, clipboard, picker, speech, host) at the manifest level, so the registry tracks the app's own seams on top. |
| Frame-time debugger | **adapted**: `nsh run --fps` / `nsh test --fps` and `--trace` give mount budgets and frame costs; `trace/` holds the recorded runs (data area, planned until it lands). |
| Crash report generation | **adapted**: panics land in the file log with lifecycle context; there is no separate crash-store surface on 0.4.0 or 0.5.0. |
| Error boundary | **ported**: the root wraps pages in an `ErrorBoundary` (TSX) / `catch_error` (Rust) so a broken page degrades to the error surface. |

## Desktop platform surfaces

| gpui-starter | Here |
|---|---|
| macOS tray icon | **out**: 0.4.0 exposed no tray surface. 0.5.0 adds trays (`Tray`, `set_tray`); building one is a follow-up, not part of the 0.5.0 port. |
| Global hotkey (Alt+Space) | **out**: no global-hotkey API on 0.4.0; in-app shortcuts cover the same registry. 0.5.0 adds `register_global_shortcut`; using it is a follow-up. |
| Native local notifications + inbox + toast fallback | **adapted**: the in-app inbox and toasts are real and the system notification sits behind a trait whose stub says notifications are unavailable. That reason is stale: 0.4.0 had no `Notifications` module, 0.5.0 ships one (`show_notification`, `set_badge`). Swapping it in behind the trait is a follow-up. |
| Native file/folder pickers, clipboard, opener | **ported** on `src/platform` over NSH's `picker`, `Clipboard`, `openUrl`/`open_path`, with capability keys declared tightly in `nsh.json`. |
| Connectivity probe (reqwest) + interface diagnostics | **adapted**: connectivity is a state signal fed by the http seam's probe against the fake API; the offline banner on the feed is the demo. |
| Web contents (`web_view`) | **ported** on the canvas page with messaging, behind the `webview` capability. |

## Content and delivery

| gpui-starter | Here |
|---|---|
| i18n in en + zh-CN | **ported**: `rust-i18n` with `i18n/{en,zh-CN}.yml`, locale switching, RTL checks. |
| Accessibility (AccessKit bridge, live regions) | **adapted**: NSH's accessibility surface (`announce`, roles, `setAccessibilityFocus`, `screenReader`) per https://nsh.dev/docs/accessibility.md; the platform page demonstrates labels and live announcements. |
| 24 hot-reloadable themes | **adapted**: one semantic token theme (`themes/theme.json`) with light and dark, including `destructive-foreground`; NSH's theme pipeline takes the JSON, and the boilerplate's point is the token discipline, not theme count. |
| Marketing site (`web/`) | **out**: not part of a boilerplate's job here; the `web` target is the app itself as a static site (`nsh package --target web`). |
| Auto-updater (signed manifests) | **adapted**: NSH's delivery path is signed packages (`nsh publish`/`verify`/`revoke`) — proven end to end by `packages/remote-screen`; update machinery is the runtime's, not the boilerplate's. |
| Wasm build | **adapted**: the `web` target is a supported NSH destination (`nsh run --target web`, `nsh package --target web`) rather than a hand-maintained wasm toolchain. |
| Demo app features (tray quick menu, hotkey launcher) | **out**: replaced by the four-target page map in the README. |

## What the GPUI list does not cover (and this repo adds)

- The mixed Rust + TSX delivery: a signed TSX package (`packages/remote-screen`) loaded on demand with verification and revocation.
- Native mobile as a first-class target: simulator/device smoke suites, `nsh/mobile` tab bar, safe areas, keyboard and back order.
- `nsh-query` and `nsh-form` as the only paths for async state and forms, with a fake API and latency/failure switches instead of a server.
- Per-area integration tests under `tests/` (planned until each area lands its file) plus headless TSX tests, all through the UI on a manual clock.
