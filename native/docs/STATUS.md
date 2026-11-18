# Status

What is done, what was verified how (command + evidence path under
`docs/evidence/<area>/`), and what is NOT verified. A claim with no evidence
is unverified. Every area updates its own section as it lands work.

## Baseline (bootstrap, pre-workflow)

- Scaffolded the Rust app at the repo root with `nsh init --template rust`
  shape: `nsh.json` (`"rust"`, `"root"`, theme at `themes/theme.json`),
  `Cargo.toml` with `nsh-query`/`nsh-form` path deps, `rust-i18n`, and
  `nsh-engine`/`nsh-testing` 0.4.0 dev-deps; module tree in `src/` with
  placeholder pages for every feature area; `App.test.tsx` + `tsconfig.json`.
- Verified on the baseline only: `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test`, `nsh check`,
  `nsh build` (no warnings), `nsh test` (1 passing test). No feature work
  exists yet; every satisfaction criterion is unverified.

## Project docs and tooling

- Done: The project area's full file set — `justfile` (12 recipes: check,
  test, test-tsx, smoke, run-macos/ios/android/web, package, rename, status),
  `scripts/rename.sh` (in-place rename of every name/id token form plus a
  `--roundtrip-test` mode that copies to a sibling, renames, runs the
  criterion-1 gate there, and always cleans the copy up), and the docs:
  README (clean clone to running app, page map aligned with the shell's
  `Route` enum), CONTRIBUTING (gates, manual-clock and seen-to-fail test
  rules, doc standards), AGENTS.md (PROMPT pointer kept, plus gates,
  ownership map, evidence conventions), CHANGELOG 0.1.0,
  `docs/architecture.md`, `docs/coverage.md` (every nsh_core, nsh_base,
  plot, nsh/mobile and nsh/desktop export as a row, named from
  `.nsh/types/nsh/`), `docs/gpui-starter-parity.md`, and
  `docs/upstream-issues.md`.
- Verified: scoped gate `bash -n scripts/rename.sh` pass and
  `test -x scripts/rename.sh` pass (re-run after every fix round);
  in-place rename functional test in a throwaway /tmp copy — all three
  token forms (`nsh-starter`, `nsh_starter`, `com.example.nshstarter`)
  renamed across 9 files, Cargo.lock correctly left to cargo, zero
  leftovers outside it, original untouched; roundtrip trap-fix repro in
  /tmp with the gate stubbed — pass path exits 0 with the copy deleted,
  FAIL path exits 1 with the copy deleted; 3 audit rounds (round 1: 5
  findings incl. 1 high EXIT-trap unbound-variable bug, round 2: 4/5
  confirmed fixed + 2 residual, round 3 fixes self-verified by checking
  every coverage row name against `.nsh/types` and confirmed by the
  orchestrator's spot-check); `just -n` parses all 12 recipes; every
  documented nsh flag checked against the installed nsh 0.4.0 help
  (`nsh --help`, `nsh test/run/package/keygen/publish/verify/revoke
  --help`); coverage rows and cross-references verified against
  `.nsh/types/nsh/*.d.ts`, the nsh-base/nsh-core 0.4.0 crate sources, and
  the `~/fo/nsh-query` / `~/fo/nsh-form` sources (`form.load` at
  nsh-form `form.rs:157`, nsh-query exports, `tracked.rs`).
- Not verified: the `--roundtrip-test` full criterion-1 gate inside a
  renamed copy (fmt + clippy + cargo test + nsh test compile the whole
  graph; it runs at the whole-tree smoke gate); docs/architecture.md and
  README factual claims about code other areas are still building (shell
  header/menus, overlays pages, platform surfaces, `smoke/`,
  `tests/<area>.rs`, `trace/`, `scripts/delivery.sh` in the remote-screen
  package — re-verify against the tree at the join);
  `docs/coverage.md` unused-export accounting (the *planned* rows need a
  final grep pass at the join, and anything still unused needs its
  stated reason); the `brew install nsh` formula name was inferred from
  the brief's `$(brew --prefix nsh)` and never run.

## Lists area audit notes (round 2)

- Wrapper gap kept local: `src/features/lists/calendar_view.rs` does NOT
  use `ui::pickers::calendar_of`, because that wrapper hardcodes its own
  `render_item` (it labels weekdays and months with bare numbers, so the
  month header would read "11" instead of "November 2026"), takes no
  `disabled_matcher` (the demo's weekends-off behavior is asserted by
  `tests/lists.rs`), and returns `impl IntoElement`, so neither
  `render_item` nor `disabled_matcher` can be chained by the caller. To
  adopt it, `ui::pickers::calendar_of` needs `disabled_matcher` and
  `render_item` passthrough parameters (or a `_with` variant).
- Retracted an earlier engine-quirk claim: "a row prepended by
  `on_load_more_at_start` never mounts, even after `scroll_to_index(0)`"
  was an artifact of the demo's old loader bookkeeping (a broken window
  fed duplicate ids into the list's keys). With the fixed window, the
  start-loader test asserts the arrival mounts after the Latest jump and
  passes; no upstream issue.
- Verified engine fact worth keeping: on an inverted list whose array is
  newest-first, a loader that prepends (array head) is a layout-tail
  append, so dropping `.keys(…)` produces no visible difference in these
  flows; keys stay in place per the coding guides, and the chat's loader
  test breaks on loader bookkeeping instead.

## Lists and data views

- Done: All nine deliverables in `src/features/lists/` — `Table` semantics
  parts over an order book plus a sectioned list with sticky headers
  (orders.rs), `DataTable` over 3,000 deterministic quotes with app-side
  sorting, a fixed symbol column and stripes on `ui::table::data_table_of`
  (quotes.rs), a controlled `Tree` with depth-indented, expansion-aware
  rows (tree.rs), a controlled `Calendar` with a weekends-off matcher and
  named month/weekday items (calendar_view.rs), a 2,000-card grid with
  viewability logging, jump controls and data-keyed stars (grid.rs), an
  inverted chat opened mid-history whose loaders produce gapless pages at
  both ends, with Receive/Latest controls and a `ui::field::text_field`
  draft (chat.rs), a `ui::carousel_of` cover carousel (carousel_view.rs),
  and `ui::pagination_of` over 500 generated entries (pager.rs); the
  audit round moved text, table, pagination and carousel surfaces onto the
  ui wrappers and landed the chat loader redesign (window 240..=280 of
  room 1..=300, page math in pure `older_page`/`newer_page` helpers).
- Verified: `cargo test --test lists` — 7/7 pass (contiguity unit test,
  sort orders, grid scroll keeps stars, chat start loader + no-jump, chat
  older loader + mounted-run span check, calendar selection + refused
  weekend, pager rows) on the manual clock, final run after the audit
  fixes; scoped `cargo clippy --all-targets` — no findings in
  `src/features/lists/**` or `tests/lists.rs`; scoped `cargo fmt --check`
  clean for the area's files; `tsc -p tsconfig.json` exit 0 covering
  `vision/lists.test.tsx`; two audit rounds — r1 six findings (the
  chat-loader HIGH window bug, missing loader tests, calendar/input
  wrapper swaps, in-repo break records, the Latest-mount re-check), r2
  clean with two comment nits fixed (the older page is 225..=239; the
  pager break fails at the Archive-21 wait, not the caption); seen-to-fail
  re-verified per test with the records in the `tests/lists.rs` header;
  the calendar wrapper-gap record and the retracted Latest-mount claim
  are in this file's "Lists area audit notes (round 2)" section.
- Not verified: pixels and real-platform runs — the vision phase has not
  run yet; `vision/lists.test.tsx` (8 views × light/dark × phone/desktop)
  is written and type-checks but has never executed, and it presses the
  shell's "Lists" nav entry, so it needs the shell and a real target;
  whole-tree gates (`cargo clippy/test --all-targets` over all areas,
  `nsh build`, the smoke suite) are left to the join/smoke phases —
  my scoped runs were blocked at times by other areas' in-flight files
  and re-run until green; two suspected engine quirks remain
  observations only, repro notes for `docs/upstream-issues.md`: (a) on a
  manual clock, an animated `scroll_to_index` can leave cell rebinds
  lagging the geometry (viewability reported rows 0–17 while the cells at
  those rows still showed their pre-jump texts, repro: grid demo, jump to
  card 1500 and back, read `screen().texts()`), and (b) on an inverted
  list, a row prepended by `on_load_more_at_start` while the viewport was
  anchored away from the start did not appear in the mounted pool after a
  later jump to the start in early probes — not reproducible after the
  loader fix (the Latest-mount assertion passes), so it is recorded as a
  first-half observation, not a claim.

## UI pass on macOS, iOS and Android (2026-11-08)

- Done: every page captured in light and dark on all three targets, reviewed, fixed, then captured again. Fixes:
  - Diagnostics rendered blank inside the shell: its `nav_page` sat inside the shell's `nav_page`. The shell now mounts `foundation::diagnostics_content`.
  - On macOS dark, pages switched between the window gray (#1e1e1e) and the theme background. `shell/page.rs` now paints the theme background on every page and caps content at 1040 pt.
  - The title bar reserved space on the right, but macOS puts the window controls on the left. It now pads by `title_bar().left` and `.right`.
  - Home duplicated the sidebar. It is now a grid of page cards, each with a one-line summary (`Route::summary`).
  - Phones: dropped the status strip above the tab bar. Added tab icons (SF Symbols on iOS, system drawables on Android). Tab roots no longer show a back button. Forms shows one header instead of two.
  - Android: the system header follows the app's appearance (`ui::nav::themed_header`), and the Forms stack paints the theme background.
  - Controls: Lists chips and Platform buttons had labels stuck to the top-left; they are now centered. Off-state switch tracks are visible in light and dark. Badges and pills hug their text. Primary buttons no longer stretch full width on desktop. The gallery's popup triggers sit in one row at one size. The motion drawer hides when shut. Overlays flows sit in titled cards. Diagnostics shows undeclared capabilities in a neutral color instead of red.
- Verified: `nsh test --target macos|ios|android .audit/capture2.test.tsx` (a throwaway capture script kept out of git) passed on each target. Its 52 screenshots per target are in `docs/evidence/shell/ui-pass/{macos,ios,android}/`, and the key ones were looked at after the fixes. `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test` (all suites), `nsh check`, `nsh test App.test.tsx` and `nsh test smoke/pages.test.tsx` all pass. Test updates: `tests/shell.rs` (the lifecycle test now runs at desktop width, where the status bar lives; the back-order test uses the "Sign up" marker) and `App.test.tsx` (the root check uses Home's own text). Each failed before its update.
- Not verified: the web target; widths between phone and desktop (tablet); `vision/*.test.tsx` was not re-run, and its saved screenshots predate this pass.

## 0.5.1 update (2026-11-17)

NSH 0.5.1 (224d9a7, released 2026-11-14) is bug-fix only, with no API changes. Evidence: `docs/evidence/project/nsh-0.5.1/` (below, `E/`); `E/gate-summary.txt` lists every gate's exit code, baseline and after.

- Changed: `Cargo.toml` pins 0.5.0 → 0.5.1; `cargo update -p` for each NSH crate only (`E/nsh-starter/cargo-update.log`; the lock diff is the 14 NSH crates' versions and checksums, nothing else); CI `NSH_VERSION` v0.5.1; README install line, `CONTRIBUTING.md`, `docs/PROMPT.md` environment, `docs/architecture.md` and `docs/coverage.md` say 0.5.1. No source or test change. `cargo tree` shows every NSH crate at 0.5.1, one version each (`E/nsh-starter/final-0.5.1/cargo-tree-nsh.log`). The `pub` lists in `nsh-core` and `nsh-base` `src/lib.rs` are the same in 0.5.0 and 0.5.1.
- Baseline on 0.5.0 (`E/nsh-starter/baseline-0.5.0/`), run with the 0.5.1 CLI: fmt 0, clippy 0, `cargo test --no-fail-fast` 125 passed / 2 failed (the known `data::retry_recovers_a_failed_feed` and `remote_screen::the_signed_package_verifies_runs_and_then_refuses_after_revocation`), `nsh check` 1 (23 errors, all in `packages/remote-screen`), `nsh build` 0 with no warning, `nsh test App.test.tsx` 2/2, `nsh test smoke/pages.test.tsx` 1/1, `packages/remote-screen` check/build/test 1.
- On 0.5.1 (`E/nsh-starter/final-0.5.1/`): the same exit codes, the same 125/2 split with the same two failures, and the same 23 type errors, line for line. Nothing regressed, so nothing was fixed.
- Trace, `nsh test trace/feed.test.tsx --trace`: mount items per step identical on 0.5.0 and 0.5.1 (`open()` 527, like tap 1, scroll 42 per step while the pool fills), `nodes` flat at 479 on both.
- Siblings, same method (`E/<repo>/`): `nsh-query` and `nsh-form` gates match their baselines (TSX 74/74 and 100/100). The `feat/tanstack` worktrees moved 0.4.0 → 0.5.1; `nsh-form`'s r3/r5 failed on 0.5.1 exactly as on master at 0.5.0, and took master's 0.5.0 change (the debounce is measured from the blur); seen to fail with the debounces broken (`E/nsh-form-tanstack/final-0.5.1/rows2-seen-to-fail.log`).
- Not run: device and packaging runs, the vision pass, cross-target `cargo check`. 0.5.1 changed lists, focus and overlays on devices; none of that was looked at on a screen.
- Fixed after the update (`E/fixes/`, `E/fixes/summary.txt`): both old failures came from the siblings' master moving to TanStack (`nsh-query` f6c0db8/e17723f, `nsh-form` 67fcf0c), not from NSH. `packages/remote-screen` is ported to master's TypeScript API (`QueryClientProvider`, `createQuery(() => ({ queryKey, retry: false, ... }))`, `createMutation` with `invalidateQueries` in `onSuccess`, `createForm(() => ({ defaultValues, validators: { onSubmitAsync } }))` for the server errors, `form.Field` with an `onBlur` rule). The Rust crate now keeps infinite lists in the client cache, so the data test's "Remount feed" got the cached pages and never failed; it now calls `query_client().remove(feed_key())` before remounting, as the crate's README says a first load needs. The app itself is unchanged: a remounted feed shows its cached pages, and the error state shows on a load with no pages. Gates: fmt 0, clippy 0, `cargo test` 127 passed / 0 failed, root `nsh check` 0, `nsh build` 0 with no warning, `nsh test App.test.tsx` 0, `nsh test smoke/pages.test.tsx` 0, remote-screen check/build/test 0 (9/9, build with no warning). Seen to fail: the retry test with Retry wired to nothing (`E/fixes/data-retry-seen-to-fail.log`), the signed-package test with a section title changed, and four package tests with invalidation, the server-error mapping, Retry and the name rule each broken (`E/fixes/remote-screen-*`). `scripts/delivery.sh` ran again with fresh throwaway keys in the git-ignored `.nsh/keys` (`docs/evidence/remote-screen/delivery.log`), so the Remote screen page has a delivery record again.

## 0.5.0 migration (2026-11-11)

Plan: `docs/nsh-0.5.0-migration.md`. Evidence: `docs/evidence/project/nsh-0.5.0/` (below, `E/`). Load on this Mac reached 200+ during the run (other agents' builds); nothing was lowered for it.

### What changed

- CLI: nsh 0.5.0 (f4d68f7) installed with the release's `install.sh` into `~/.nsh`, linked at `~/.local/bin/nsh`, which comes first on `PATH`, so no `justfile` override was needed (`E/phase1-which-a-nsh.log`). Homebrew's 0.4.0 is untouched. Components `android`, `ios`, `web` added (`E/phase1-component-add.log`). Help of every subcommand saved for both versions (`E/help/`); no flag the repo uses was removed or renamed (`E/phase1-help-diff-0.4.0-vs-0.5.0.txt`).
- Sibling libraries, uncommitted: `~/fo/nsh-query` (Cargo pins, `Cargo.lock`, CI `NSH_VERSION`, README "Requires NSH 0.5.0") and `~/fo/nsh-form` (the same, plus `packages/nsh-form/App.tsx` and `tests/rows2.test.tsx`: r3/r5 now measure the form-level async debounce from the blur, because a 0.5.0 test step can move the manual clock by a display frame; both were seen to fail with the debounce broken, `E/phase2-0.5.0-nsh-form-rows2-seen-to-fail.log`). Their `node_modules` were missing, so `bun install --frozen-lockfile` ran in both packages first.
- Starter: `Cargo.toml` pins 0.5.0 and `cargo update` (the old lock is kept at `E/Cargo.lock.before-0.5.0`); every NSH crate resolves to 0.5.0 (`E/phase3-nsh-versions-in-lock.txt`). Code: `LayoutCurve::FittedSpring { damping_ratio: 0.7 }` (reorder), `frame_rate: None` on the `Spring` const, `ColumnFixed` from `bool` (quotes), the window calls ported to `main_window()` / `WindowAttributes` / `set_mode` / `set_bounds` / `close()` (`src/ui/desktop.rs`, `src/shell/chrome.rs`, `src/platform/desktop_card.rs`), `use rust_i18n::t;` in ten modules (0.5.0's prelude has its own `t!`), `Idiom::Terminal` on the device page (+ `en`/`zh-CN` strings), and the capability registry lists 0.5.0's thirteen keys. Two overlay tests changed to press what a user can reach (see below).

### Verified, and how

- Baseline on 0.4.0 before any edit (`E/baseline-0.4.0-*.log`): fmt 0, clippy 0, `cargo test --no-fail-fast` 125 passed / 2 failed (`data::retry_recovers_a_failed_feed`, `remote_screen::the_signed_package_verifies_runs_and_then_refuses_after_revocation`), `nsh test App.test.tsx` 2/2, `nsh test smoke/pages.test.tsx` 1/1, `just test-tsx` failing on remote-screen type errors.
- On 0.5.0 (`E/final-0.5.0-*.log`): fmt 0, clippy 0, `cargo test --no-fail-fast` 125 passed / the same 2 failed, `nsh test App.test.tsx` 2/2, `nsh test smoke/pages.test.tsx` 1/1. `nsh check` at the root fails only on `packages/remote-screen` files (23 errors), the same errors the package shows on 0.4.0 (`E/baseline-0.4.0-withdeps-remote-screen-tsx-check.log` vs `E/phase5-0.5.0-remote-screen-tsx-check.log`); `vision/`, `trace/` and `smoke/` type-check on 0.5.0.
- New 0.5.0 failures fixed, not reported: `overlays::the_dialog_closes_on_a_scrim_press` and `the_sheet_opens_as_a_bottom_sheet_on_phone_width`. 0.5.0's headless `press` lands where a view is drawn; the scrim's center is under the dialog, and the page's detent buttons are under the open sheet. The tests now press the scrim at an uncovered corner (`press_at`) and pick each detent while the sheet is closed. Both seen to fail with the documented breaks (`E/phase6-overlays-seen-to-fail.log`).
- Cross-target `cargo check --lib`: `aarch64-apple-ios-sim` 0, `wasm32-unknown-unknown` 0, `aarch64-linux-android` 0 (`E/phase3-check-*.log`; Android needed `CC`/`AR` for `ring`, and NDK 28's `llvm-ar` because NDK 29's is a self-exec loop on this Mac).
- Siblings on 0.5.0: nsh-query fmt/clippy (default and `--all-features`)/test (both) all 0, TSX check/build/test 0 with 74/74 (`E/phase2-0.5.0-nsh-query-*`); nsh-form the same, TSX 100/100 (`E/phase2-0.5.0-nsh-form-*`). Their 0.4.0 TSX baselines with deps installed also pass (`E/baseline-0.4.0-withdeps-*`).
- Signed delivery flags on 0.5.0: keygen → certify → publish → verify → revoke → verify refuses, with the flags `tests/remote_screen.rs` and `scripts/delivery.sh` use, on a throwaway app in the scratchpad (`E/phase5-signed-delivery-probe-0.5.0.log`). The remote-screen package itself does not build on either version (below).
- Trace (criterion 7), `nsh test trace/feed.test.tsx --trace` on 0.5.0 (`E/final-0.5.0-trace.log`): like tap 1 mount item (was 1); scroll: 42 items per step while the pool fills, then 1–27, `nodes` flat at 479 after warm-up (was flat at 398); `open()` 527 items (was 363). The old numbers (`docs/evidence/data/trace-feed.txt`) predate the WIP shell redesign, so the node and open counts are not a clean version comparison.

### Behavior changes (Phase 4)

- Media grants (#1417): every image, video and SVG source in `src/` is `asset!` (an `asset:` URI) or inline SVG markup; `nsh-core-0.5.0/src/access.rs` always allows `asset:` and `data:`, so no `fs.read` grant was added. The picker shows a file name only. `packages/remote-screen` has no media. Inferred from source; no device run looked at the canvas page.
- `focus-visible:` keyboard-only (#1519): the app's wrappers use `focus:`, so they are unaffected; `nsh_base` components' own rings now show for keyboard focus only. Unverified on screen.
- Lists (#1132, #1057): `tests/lists.rs` (chat start loader, no-jump) and `tests/data.rs` pass as before. Not looked at on a device.
- `scrollbar(false)` (#937): the gallery rail now also hides its horizontal indicator on desktop. Unverified on screen.
- Back in an unselected tab (#996): `tests/shell.rs` back-order tests pass. Unverified on Android.
- Popover Tab order (#1616): `tests/overlays.rs` keyboard paths pass headless.
- macOS file drops (#1092): no drop handler exists in `src/`; nothing to change.
- macOS test hosts in the background (#907): see "Not verified".
- `nsh::perf` → `nsh::fps` (#1322): no use in `src/`, `trace/` or docs.
- Reorder spring: same damping ratio and duration; the feel was not looked at.

### Not verified, and why

- Packaging and device runs, partial (`E/phase6-package-*.log`): `nsh package --target macos` exit 0, and the packaged app started and stayed up 15 s (`E/phase6-macos-app-launch.log`; a window capture was refused by macOS, so no screenshot). `nsh package --target ios-simulator` exit 0. `nsh package --target web` and `nsh test --target web` fail because `nsh` sets `AR_wasm32_unknown_unknown=llvm-ar` and no `llvm-ar` is on this Mac's `PATH` (the same code is in 0.4.0's CLI); with Homebrew's `llvm-ar`/`clang` passed in, the web runtime built (`E/phase6-package-web-with-llvm.log`), then `cargo metadata` waited an hour on the shared cargo package lock held by another agent's processes and I stopped it, so the web site was neither packaged nor launched. `nsh test --target macos smoke/pages.test.tsx` reached 12 of 15 pages and then hit the test's 30 s limit at a load average near 200 (`E/phase6-package-test-macos.log`); the 10 screenshots it took are in `E/macos-smoke/`, all looked at, and show no breakage (the Data shots catch the loading state, the Forms shot catches the push mid-transition, and `page-ui.png` shows Home because the marker "UI gallery" already matches the sidebar; that last one is a test issue that predates 0.5.0). Not run: `nsh test --target ios|android`, `nsh package --target android`, the vision pass, and `scripts/rename.sh --roundtrip-test` (its gate cannot pass while the two old `cargo test` failures remain). The Android toolchain on this Mac also has the NDK `llvm-ar` loop described above.
- `packages/remote-screen` does not type-check or build on 0.4.0 or 0.5.0: it calls `nsh-query`/`nsh-form` APIs (`noRetries`, `invalidates`, `initial`, `form.field`, ...) that the siblings' current TanStack-based packages do not have. Pre-existing, not caused by 0.5.0; this also fails `just test-tsx` and the signed-package test. Fixing it is a port of the package, left to the owner.
- `packages/remote-screen/.nsh/` was deleted to regenerate types, as the plan says; it also held the git-ignored delivery record (`.nsh/delivery/state.json`) and throwaway keys. `scripts/delivery.sh` cannot rebuild them until the package builds, so the Remote screen page shows its "no delivery yet" state.
- 0.5.0's `nsh test` logs `warn the status bar is not granted` for apps that declare `capabilities` without `statusBar` (84 times in nsh-query's run); the tests pass. Not investigated beyond the source line that prints it.

### Follow-ups (out of scope)

Real `Notifications` behind the existing trait; tray and global shortcuts (and moving `src/shell/command_bar.rs` onto `Window`/`Tray`); updater/OTA; `EmojiPicker`; `Terminal`; multi-window beyond the main window; haptics; URL schemes via `nsh.json`. See `docs/coverage.md` "New in 0.5.0, not yet used".
