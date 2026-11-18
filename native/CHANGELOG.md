# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Changed: NSH 0.5.1

- All NSH crates move from 0.5.0 to 0.5.1, a bug-fix release with no API changes; the sibling `nsh-query` and `nsh-form` crates move with them. Only `Cargo.toml` pins, `Cargo.lock` (NSH crates only), CI's `NSH_VERSION` and the install line in the README changed; no app code changed. Install the 0.5.1 CLI with the release's `install.sh` (README).

### Changed: NSH 0.5.0

- All NSH crates move from 0.4.0 to 0.5.0 (`nsh-core`, `nsh-base`, `nsh-platform-{ios,linux,macos,windows}`, `nsh-engine`, `nsh-testing`); the sibling `nsh-query` and `nsh-form` crates move with them, so one NSH version resolves. The CLI must be 0.5.0 too: install it with the release's `install.sh` (README); Homebrew still ships 0.4.0.
- Desktop windows use 0.5.0's `WindowRef` model: `main_window().set(..)` for the title bar and title, `set_mode` for maximize, `set_bounds` for the frame, `close()` to close (`src/ui/desktop.rs`, `src/shell/chrome.rs`, `src/platform/desktop_card.rs`). Behavior is meant to be unchanged.
- The Reorder demo's layout spring is `LayoutCurve::FittedSpring { damping_ratio: 0.7 }`, the same 0.7 damping ratio over 420 ms as before; 0.5.0's layout springs are physics-timed, so the motion may feel slightly different.
- The Quotes table's symbol column is fixed with `ColumnFixed::Left` (was `fixed: true`).
- Modules that call `t!` import rust-i18n's explicitly: 0.5.0's prelude brings its own `t!`.
- The device page names the new `Terminal` idiom.
- The diagnostics page's capability registry lists 0.5.0's thirteen keys (`windows`, `notifications` and `audio` are new; none is declared).
- The "not exposed" notes on the device and speech cards now say 0.5.0 (the device model and reading aloud are still not exposed).
- Two overlay tests press what a user can reach: the dialog's scrim at an uncovered corner, and the sheet's detents while the sheet is closed. 0.5.0's headless `press` lands where a view is drawn and fails on a covered view.

### Behavior changes from NSH 0.5.0 you may notice

- `focus-visible:` styles in `nsh_base` components show for keyboard focus only, no longer after a click or tap. This app's own wrappers use `focus:`.
- `scrollbar(false)` on the gallery's horizontal rail now also hides the horizontal indicator on desktop.
- An inverted list at its start shows rows inserted above it instead of keeping the first row in place (the chat page).
- Back no longer pops a stack in an unselected tab.
- An unknown or misspelled capability in `nsh.json` is a build warning, not an error.

## [0.1.0] - 2026-11-07

### Added

- Rust app scaffolded with the `nsh init` shape: `nsh.json` (`"rust"`, `"root"`),
  `Cargo.toml` with `nsh-query` and `nsh-form` path dependencies, theme at
  `themes/theme.json`, and the module tree (`app`, `shell`, `foundation`,
  `state`, `services`, `ui`, `platform`, `features/{forms,data,lists,canvas,
  motion,overlays,remote_screen}`) with placeholder pages per feature area.
- `App.test.tsx` headless smoke test; `nsh check`, `nsh build` and `nsh test`
  pass on the scaffold.
- `justfile`: `check`, `test`, `test-tsx`, `smoke`, `run-macos`, `run-ios`,
  `run-android`, `run-web`, `package`, `rename`, `status`.
- `scripts/rename.sh`: renames the app's name and id everywhere they live,
  with a `--roundtrip-test` mode that proves the renamed copy passes the full
  gate (fmt, clippy, `cargo test`, `nsh test`) and always cleans the copy up.
- Project docs: this changelog, `CONTRIBUTING.md` (gates, test rules, doc
  standards), `AGENTS.md` (ownership map, evidence conventions), `README.md`
  (clean clone to running app, page map), `docs/architecture.md`,
  `docs/coverage.md`, `docs/gpui-starter-parity.md`, `docs/upstream-issues.md`.
- i18n scaffold with `en` and `zh-CN` locales.
- Shell: typed route enum with deep-link parsing, a nav stack every entry
  point drives, the lifecycle state machine, undo snapshots on navigation and
  the back guard (`src/shell/`).
- Foundation primitives: ids, time, validation, typed config, and the
  capability registry (`src/foundation/`); state modules for the event bus,
  background tasks and undo history (`src/state/`).
- Data feature: the infinite feed with optimistic likes and retry, the post
  detail, and the query devtools, backed by the local fake API with latency,
  failure and offline switches (`src/features/data/`,
  `src/services/fake_api.rs`).
- Lists feature: virtualized orders, the quotes `DataTable`, and the tree
  demo (`src/features/lists/`).
- Remote-screen package: the TSX feature over the TypeScript `nsh-query` and
  `nsh-form`, with its manifest, theme and `node_modules` symlinks to the
  sibling checkouts (`packages/remote-screen/`).
