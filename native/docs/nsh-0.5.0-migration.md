# NSH 0.4.0 → 0.5.0 migration plan

Source of truth: [release notes](evidence/project/nsh-0.5.0/release-notes-v0.5.0.md) (v0.5.0, published 2026-11-11). All eight NSH crates this repo uses are on crates.io at 0.5.0. Evidence for every step goes under `docs/evidence/project/nsh-0.5.0/`.

## Decisions already made by the owner

- **Sibling libraries are bumped in place.** `~/fo/nsh-query` and `~/fo/nsh-form` pin `nsh-core`/`nsh-base`/`nsh-engine`/`nsh-testing` at `"0.4.0"`; edit them there, run their own gates, leave the changes uncommitted. This is a one-off exception to the "never write to another repository" rule, for these two repos and this migration only.
- **CLI 0.5.0 is installed globally** with the official `install.sh` into `~/.nsh` (links `~/.local/bin/nsh`). Homebrew's tap still ships 0.4.0; do not `brew uninstall` or `brew upgrade` it. This supersedes "Do not upgrade it" in `docs/PROMPT.md`.

## Constraints

- The tree has uncommitted WIP (shell redesign, forms, `src/ui/native_*`, `src/shell/command_bar.rs`). Do not revert, stash, reset or commit anything. Migration edits go on top.
- No git machinery, no pushing, no killing processes you did not start, own simulator/AVD names (see `AGENTS.md`).
- No check is weakened, skipped or deleted to get a pass. Never invent an API: look names up in the 0.5.0 source.
- If something cannot be done, say so in `docs/STATUS.md` as unverified. Do not paper over it.

## Phase 0: baseline on 0.4.0 (before any edit)

1. Run and save output of: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `just smoke` → `baseline-0.4.0-*.log`.
2. Record which tests already fail because of the WIP. Those are pre-existing, not migration regressions; list them, do not fix them unless 0.5.0 is the cause.
3. Save `nsh --version` and `cargo tree -i nsh-core` output.

## Phase 1: toolchain and reference source

1. Install the CLI: download `install.sh` from the v0.5.0 release with `gh release download v0.5.0 -R longbridge/nsh -p install.sh`, read it, run with `NSH_VERSION=v0.5.0`. Confirm `~/.nsh/bin/nsh --version` prints 0.5.0.
2. `which -a nsh`: if `/opt/homebrew/bin/nsh` (0.4.0) wins on PATH, make this repo's commands use 0.5.0 explicitly: a `nsh := env("NSH", ...)`-style variable in the `justfile` resolving to `~/.nsh/bin/nsh`, and say so in `README.md`. Do not edit the user's shell profile.
3. Platform runtimes are now components (#1302). Run `nsh component --help`, then add what the gates need (`android`, `web`, `ios` as named by the help) before any `--target` run.
4. Shallow-clone the tag for reference: `git clone --depth 1 --branch v0.5.0 https://github.com/longbridge/nsh ~/fo/.refs/nsh-v0.5.0` (read-only afterwards). `~/fo/.refs/nsh-main` dates from Nov 7 and is not the release.
5. Save `nsh --help` and each subcommand's `--help` for 0.5.0; diff against 0.4.0 help to find renamed flags. Every flag used in `justfile`, `README.md`, `CONTRIBUTING.md`, `scripts/`, `tools/ci` is checked against it.

## Phase 2: sibling libraries (`~/fo/nsh-query`, `~/fo/nsh-form`)

For each repo, Rust crate and TypeScript package:

1. Bump the four pins in `crates/*/Cargo.toml` to `0.5.0`; `cargo update`.
2. Fix compile errors against 0.5.0 (see the API table below).
3. Run that repo's own gates as its `AGENTS.md`/`README.md` define them (fmt, clippy `-D warnings`, `cargo test`, and for `packages/*`: `nsh check`, `nsh build`, `nsh test`). Also check `examples/`.
4. Leave changes uncommitted. Record a per-repo summary of files touched in `docs/STATUS.md`.

## Phase 3: starter Rust crates

1. `Cargo.toml`: all `nsh-*` versions `0.4.0` → `0.5.0` (core, base, platform-ios/linux/macos/windows, engine, testing). `cargo update`; confirm `cargo tree -d | grep nsh` shows a single version of every NSH crate.
2. Known break sites (found by grep; the compiler will find the rest):

| 0.5.0 change | Site | Action |
| --- | --- | --- |
| `LayoutCurve::Spring { damping }` → `FittedSpring { damping_ratio }` / `Spring(Spring)` (#1305) | `src/features/motion/reorder.rs:23` | `FittedSpring { damping_ratio: 0.7 }` keeps the old meaning; re-check the reorder feel |
| `Spring` gained fields (#998) | `src/features/motion/springs.rs:18`, `src/features/motion/swipe.rs:57` | add the new fields or `..Default::default()`; `springs.rs` uses a `const fn`, so check whether `Default` is usable there |
| `DataTableColumn::fixed: bool` → `ColumnFixed` (#1395) | `src/features/lists/quotes.rs:135` | `Left` for the symbol column; verify the "not fixed" form (`Option`?) in source; consider demoing `Right` |
| `nsh_platform_macos::AppOptions` gained fields (#1390) | `src/main.rs:31` | add fields or `..Default::default()`; check the iOS/Linux/Windows `AppOptions` too |
| `Theme::from_json_lenient` → `from_json_with_diagnostics` (#1571) | no hit in `src`; check tests and `tools/` | rename if found |
| `SurfaceOptions`, `ContextMenuEvent` non-exhaustive; `Command::Haptic` | iOS-only code in `src/platform`, `src/main.rs` | build for `aarch64-apple-ios-sim` to surface these |
| New fields on `Accessibility`, `AccessibilityState`, `ViewProps`, `ResolvedTextStyle`, `LinkProps`, `BundleOptions`, `PerfSnapshot`, `Driver`/`KeyframeAnimation`/`Timing` | none found by grep | fix what the compiler reports |
| Window API replaced by `WindowRef` / `Window` kinds (#1137, #1219) | `src/ui/desktop.rs`, `src/platform/desktop_card.rs`, `src/shell/chrome.rs` | port to the new model from `examples/gallery-rs` at v0.5.0 |

3. Compile every target, not just the host: `cargo check` for host, `--target aarch64-apple-ios-sim`, `aarch64-linux-android`, `wasm32-unknown-unknown`. cfg-gated code only breaks there.

## Phase 4: behavior changes to test, not just compile

Each one gets a look in the running app or a test; note the result in `docs/STATUS.md`.

- **Media capability grants (#1417).** `nsh.json` declares `capabilities`, so local-file images/video/SVG now need `fs.read` grants, relative media paths load nothing natively, and `${pluginDir}` is needed for `import.meta.url` assets. Audit every `image`/`video`/`svg`/Markdown-image source in `src/features/canvas`, `src/ui`, `src/platform/picker_card.rs` and `packages/remote-screen`. Add the tightest grant that works.
- **`focus-visible:` is keyboard-only (#1519).** No literal hit in `src`, but check `themes/theme.json`, `nsh_base` class overrides and the focus-ring screenshots in `docs/evidence/ui`. Rings after a click are gone by design; rings on Tab must still show.
- **Lists.** A list at its start now shows rows inserted above (#1132): check the chat page (`inverted`, `on_load_more_at_start`) and the feed's pull-to-refresh. `keyExtractor` now drives reconciliation (#1057): check keys are stable and unique.
- **`scrollbar(false)` hides horizontal indicators too (#937):** `src/ui/gallery.rs:1001`.
- **Back no longer pops a stack in an unselected tab (#996):** `src/shell/back.rs`, `tests/shell.rs` Android back order.
- **Popover Tab order (#1616):** overlays page and `tests/overlays.rs` keyboard paths.
- **macOS file drops arrive as `file:` URLs (#1092):** any drop handler in `src/platform/files.rs`.
- **macOS test hosts start in the background (#907):** `vision/` and smoke screenshot runs may need the window brought forward explicitly.
- **Perf log target `nsh::perf` → `nsh::fps` (#1322):** `trace/`, `src/foundation/diagnostics.rs`, docs.
- **Trace numbers.** Re-measure criterion 7 (like-tap mount items, flat `nodes` on scroll); 0.5.0 changed list virtualization, so record new numbers beside the old.

## Phase 5: TSX side

1. `packages/remote-screen`: delete `.nsh/` generated types and let 0.5.0 regenerate; `nsh check`, `nsh build` (zero warnings; unreadable `nsh.json` settings are now warnings, so read the output), `nsh test`.
2. Root: `nsh test App.test.tsx`, `nsh test smoke/pages.test.tsx`, `trace/`, `vision/` files type-check against 0.5.0 `nsh/test`.
3. Signed delivery: re-run keygen → publish → verify → revoke with throwaway keys; 0.5.0 added OTA packages and store publishing to `nsh publish`, so confirm the flags the docs and `tests/remote_screen.rs` use still exist.

## Phase 6: gates

All on 0.5.0, output saved:

1. `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
2. `just test-tsx`, `just smoke`.
3. `nsh test --target macos` smoke suite; `--target ios` and `--target android` on this repo's own simulator/AVD; `--target web`. Look at every screenshot.
4. `nsh package --target macos`, `ios-simulator`, `android`, `web`; launch the macOS app and the web site.
5. `scripts/rename.sh --roundtrip-test`.
6. Compare with the Phase 0 baseline: any test that passed on 0.4.0 and fails now is a regression to fix, not to report.

## Phase 7: docs

- Every `0.4.0` mention outside `docs/evidence` and `.agents/skills`: `README.md`, `CONTRIBUTING.md`, `AGENTS.md`, `docs/architecture.md`, `docs/coverage.md`, `docs/gpui-starter-parity.md`, `docs/upstream-issues.md`, `docs/STATUS.md`, and source comments (`src/app/mod.rs`, `src/shell/{lifecycle,deep_link,command_bar}.rs`, `src/platform/{environment,speech_card}.rs`, `src/ui/native.rs`, `src/foundation/capabilities.rs`, `src/features/{remote_screen/mod,canvas/world}.rs`, `tests/remote_screen.rs`, `smoke/pages.test.tsx`, `vision/platform.test.tsx`). Update the ones that state a version; delete or rewrite comments describing a 0.4.0 limitation that 0.5.0 removed, after verifying it is gone.
- `docs/PROMPT.md`: update the environment line and re-test each "Facts that bite on 0.4.0" entry; keep what still bites, drop what does not, retitle the section.
- `docs/upstream-issues.md`: re-test each issue on 0.5.0 and mark fixed ones.
- `docs/coverage.md`: diff the `nsh_base`/`nsh_core` element exports between 0.4.0 and 0.5.0 and add a "new in 0.5.0, not yet used" section listing them (`EmojiPicker`, `Terminal`, `Window`, `Sound`/`AudioPlayer`, tray, notifications, haptics, …).
- `CHANGELOG.md`: one entry for the upgrade, listing the code changes and behavior changes users of the starter will notice.
- `.agents/skills` are upstream copies: leave them.

## Out of scope (list as follow-ups, do not build)

Adopting 0.5.0 features: real `Notifications` behind the existing trait (the stub's reason is now stale; say so in `docs/gpui-starter-parity.md`), tray and global shortcuts, updater/OTA, `EmojiPicker`, `Terminal`, multi-window demo beyond what the port requires, haptics, deep-link URL schemes via `nsh.json`.

## Done means

Phase 6 gates pass on 0.5.0 with saved output, no NSH crate resolves to 0.4.0 anywhere, the docs state 0.5.0, and `docs/STATUS.md` has a "0.5.0 migration" section listing: what changed, what was verified and how, what was not verified and why, and the follow-ups above.
