# Build nsh-starter

## Goal

Turn this empty repository into `nsh-starter`, the Native Shell counterpart of `gpui-starter`: a production-grade application boilerplate someone can clone, rename and ship on macOS, iOS, Android and the web, that also works as the living reference for everything NSH can do. It uses `nsh-query` and `nsh-form` for all async state and all forms, and it runs for real on each target.

This repository holds only directories, `.agents/skills`, `.zcode` and this prompt. You build everything else. `nsh-query` and `nsh-form` are finished and are not your job. Firebase, sign-in and the HTTP library are being built in separate projects, so do not build them here. Leave clean seams instead, as described below.

## What the reference looks like

Read `gpui-starter` before designing anything, at `~/ps/torii/gpui-starter`. Its `README.md`, `docs/architecture.md`, `docs/desktop-boilerplate-features-plan.md`, `src/` layout and `AGENTS.md` are the bar for structure and depth: `app`, `shell`, `features`, `ui`, `services`, `state`, `platform`, `foundation`, and `tests`. Reproduce the ideas, translated to NSH, and never copy GPUI code. Its feature list includes a lifecycle state machine, routed pages with deep links, command palette, undo and redo, an event bus, typed config with migrations, persistence, a background task manager, a status bar, a central error surface, a diagnostics view, first-run setup, file logging, telemetry with pluggable sinks, i18n in two locales, accessibility, themes, and a capability registry. Each of those gets an NSH version unless NSH makes it meaningless, in which case `docs/gpui-starter-parity.md` says why.

## Architecture

- A Rust app (`nsh init . --template rust`, `"rust"` and `"root"` in `nsh.json`) is the shell and most features, since the official guidance prefers Rust for an app shipped in its binary.
- `packages/remote-screen` is a TSX feature using the TypeScript `nsh-query` and `nsh-form`. It proves the mixed path and is published as a signed package and loaded on demand, as the `nsh-signed-delivery` skill describes. Throwaway keys only.
- Dependencies: `nsh-query = { path = "../nsh-query/crates/nsh-query" }`, `nsh-form = { path = "../nsh-form/crates/nsh-form" }`.
- The look: read `nsh-design-guides` and `nsh-component-library` first. Write a theme file with semantic tokens, including `destructive-foreground`, which the default lacks. Write `src/ui/` with one wrapper per `nsh_base` component in use, covering every part in that skill's parts map. Screens import from `src/ui` only. No raw colors outside the theme. The target is a credible shadcn-level look, in light and dark, adapted per platform for density, navigation and overlays.
- Seams for the other projects: `src/services/http.rs` over NSH's `fetch` with one small trait, `src/services/auth.rs` with a mock provider, `src/services/analytics.rs` and `src/services/monitoring.rs` as sinks. Each is a trait with a local implementation, and swapping in the real library is one file. Do not fake those features' depth here.
- Notifications: 0.4.0 has no `Notifications` module; `~/fo/.refs/nsh-main` does, unreleased. Build the in-app inbox and toast, and put the system notification behind a trait with a stub that says so.

## Every feature, as a page

Add as much as possible. The app must demonstrate every element and base component NSH 0.4.0 ships, in real screens rather than a bare list. Check `https://nsh.dev/llms.txt`, `~/fo/.refs/nsh-v0.4.0/examples/gallery-rs` and `examples/gallery`, and the exports in `crates/nsh-base`, then keep `docs/coverage.md` as a table of every export with where it is used. Anything unused needs a stated reason.

Areas, with disjoint files:

1. **shell**: lifecycle, root, `nav_stack` with a typed route enum, deep-link parsing, sidebar on desktop and tabs on mobile, title bar and application menus on desktop, status bar, command palette (`Command`), keyboard shortcuts, undo and redo, the desktop and web `header` slot on every page, Android back order.
2. **foundation and state**: errors, ids, time, validation, typed config with schema migrations in `Storage`, event bus, background task manager, capability registry, first-run flow, file logging, diagnostics page, central error surface.
3. **ui library**: theme, tokens, the wrappers, states, focus rings, press feedback, screenshots of each control in both appearances.
4. **forms**: with `nsh-form`, a full sign-up (email, password and confirm, async username check with debounce, phone, numeric text, date picker, time field, select, combobox, radio group, checkbox, switch, slider, color picker, OTP input, number input, rich editor, server errors, draft autosave, dirty guard using `pop_on_back`, keyboard flow, autofill), plus a settings form that loads with `form.load`.
5. **data**: with `nsh-query`, a feed on `virtual_list` with `use_infinite_query`, pull to refresh, optimistic like with rollback, invalidation after a mutation, stale-while-revalidate, retry with an error state and a Retry button, offline banner, skeletons, a detail page with a tracked key, and a query devtools page listing cache entries. A local fake API in Rust, with latency and failure switches, backs it, so nothing needs a server.
6. **lists and data views**: `Table`, `DataTable`, `Tree`, `Calendar`, sticky headers, grids, chat with `inverted` list and `on_load_more_at_start`, viewability, carousel, pagination.
7. **canvas and graphics**: `canvas` drawing app with responder input and undo, animated canvas, `Plot` charts, `shader` with WGSL, SVG, images with placeholders and blur, video, `web_view` with messaging.
8. **motion and gestures**: every tool from `nsh-motion-gestures`: motions, transitions, layout animations, springs, swipe rows, drag and drop, pinch and rotate, scroll-linked header with `diff_clamp`, `pan_activation`, reduced-motion paths.
9. **overlays and desktop surfaces**: `Dialog`, `AlertDialog`, `Sheet`, `Popover`, `Menu`, `ContextMenu`, `HoverCard`, `Tooltip`, toasts, `DockArea`, resizable panels, `Editor`, `DiffView`, `TextView`.
10. **platform and system**: safe areas, keyboard, device and environment page, clipboard, picker, speech, file and folder dialogs, `nsh/mobile` `TabBar`, `nsh/desktop` window and menus, i18n in `en` and `zh-CN` with the `rust-i18n` skill plus RTL checks, accessibility per `https://nsh.dev/docs/accessibility.md`, capabilities declared tightly in `nsh.json`, icons and app metadata, `nsh package` for each target.
11. **remote screen**: the signed TSX package, with throwaway keys generated at run time and never committed, a real `nsh publish`, `nsh verify`, loading in the app or a documented reason it needs an embedding host, and a revocation path shown.
12. **project**: `README.md`, `CONTRIBUTING.md`, `AGENTS.md`, `CHANGELOG.md`, `justfile` with `check`, `test`, `run-ios` and friends, `scripts/rename.sh` that renames the app id and name everywhere, `docs/architecture.md`, `docs/coverage.md`, `docs/gpui-starter-parity.md`, no CI workflows.

Sandbox: no account and no server anywhere. The fake API is the backend, and external URLs are stubbed in tests.

## Satisfaction criteria

All of these hold, each with evidence in `docs/STATUS.md`:

1. `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and `nsh test` all exit 0. In `packages/remote-screen`: `nsh check`, `nsh build` with no warnings, `nsh test`.
2. `docs/coverage.md` accounts for every `nsh_base` and `nsh_core` element export on 0.4.0, and every page is reachable from the shell.
3. `nsh test --target macos`, `--target ios` and `--target android` pass a smoke suite that opens every page and takes a screenshot. `nsh test --target web` passes where the page's features exist on the web, with the rest listed.
4. `nsh run --target ios` and `--target android` show the app, driven by simulator or computer-use tools: navigate five pages, fill and submit the sign-up form with the keyboard up, like an item, go back. Screenshots saved and looked at.
5. A vision pass over every page in light and dark at phone and desktop width finds no clipped text, no invisible control, no raw default look, and no contrast problem. Fix what it finds.
6. `nsh package --target macos`, `ios-simulator`, `android` and `web` all produce artifacts, and the macOS app and web site run.
7. `--trace` shows that a tap on the feed's like button costs a handful of mount items, and scrolling keeps `nodes` flat. Record the numbers in `docs/STATUS.md`.
8. Screens use `nsh-form` for all forms and `nsh-query` for all async data. A grep for raw hex or `rgb(` outside the theme finds nothing unexplained.
9. `scripts/rename.sh` renames a copy of the repo to another app, and that copy passes criterion 1.
10. A person can follow `README.md` from a clean clone to a running app.

## Environment, as measured on this Mac

- `nsh` 0.5.1 (224d9a7), installed by the release's `install.sh` into `~/.nsh`, linked at `~/.local/bin/nsh`, which comes before Homebrew on this Mac's `PATH`. Homebrew's `nsh` 0.4.0 (e626a8d) stays installed at `/opt/homebrew/bin/nsh`; do not uninstall or upgrade it. Platform runtimes are components (`nsh component list`); `android`, `ios` and `web` are installed. The release archives no longer bundle the embedding SDKs (`nsh-embed-ios.zip`, `nsh-embed-android.zip` are separate release assets); the 0.4.0 frameworks under `$(brew --prefix nsh)/libexec/embed/` are 0.4.0's.
- Rust stable with `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android` and `wasm32-unknown-unknown` installed.
- Xcode with an iOS 27 simulator runtime. Android SDK at `$ANDROID_HOME`, NDK 29, one AVD named `indopak-audit`.
- `/usr/bin/java` is a stub with no runtime behind it. Use Android Studio's JDK: `export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"`.
- `bun` and `node` 24 exist. Installing dev tools with them inside this repository is fine.
- Reference source, read-only: `~/fo/.refs/nsh-v0.5.0` (a shallow clone of the v0.5.0 tag, f4d68f7), including its `examples/`. The installed 0.5.1 is a bug-fix release of it with no API changes (release notes: `docs/evidence/project/nsh-0.5.1/release-notes-v0.5.1.md`). `~/fo/.refs/nsh-v0.4.0` is the previous release. `~/fo/.refs/nsh-main` dates from 2026-11-07 and is not a release. `~/fo/.refs/react-native-firebase` and `~/fo/.refs/flutterfire` are the Firebase references.
- Sibling libraries, already built and tested by someone else: `~/fo/nsh-query` and `~/fo/nsh-form`. Each has a Rust crate and a TypeScript package with the same behavior. Read their READMEs. Do not edit them. If one has a bug, write it up in `docs/upstream-issues.md`, and work around it.
- Installed for this repository: the `z-nsh-engineer` agent and the skills under `.agents/skills`, by `z-project-init`. Start with `nsh`, `nsh-testing` and `nsh-design-guides`.

## Tools you may use to look at the real thing

You have full access to this machine. Use it. A headless pass alone never finishes visible or native work.

- `nsh test` headless, and `nsh test --target macos|ios|android|web`, which use real input, and `app.screenshot(path)`.
- `nsh run --target ios|android|macos|web`, `nsh package --target ...`, `xcodebuild`, `xcrun simctl`, Gradle, `adb`, and the Android emulator.
- The computer-use, ios-simulator and android-emulator plugins, the `mobile-mcp` server, `mobilecli`, the `mobile-automation` and `open-computer-use` skills for live capture, and `agent-browser` for the web.
- Save evidence as PNGs and logs under `docs/evidence/<area>/`, and look at every PNG you save. A screenshot nobody looked at proves nothing.

## This machine is shared right now

Other agents are building `nsh-starter`, `nsh-firebase`, `nsh-http` and `nsh-auth` on this Mac at the same time, one per repository. Do not interfere with them.

- Stay inside this repository. Reading `~/fo/.refs`, `~/fo/nsh-query` and `~/fo/nsh-form` is fine. Never write to another repository.
- Use your own devices, named after this repository: `xcrun simctl create <repo> "<a current iPhone>"` and refer to it by UDID, and `avdmanager create avd -n <repo> ...` for your own AVD, with your own `-port`, and `adb -s <serial>` on every call. Never run `xcrun simctl shutdown all`, `xcrun simctl erase all`, `adb kill-server`, `killall`, or anything that stops a process you did not start.
- Pick your own ports for dev servers and local servers with `--port 0` or a free port. Never kill whatever holds a port.
- Prefer capture that does not take over the screen: `xcrun simctl io <udid> screenshot`, `adb -s <serial> exec-out screencap -p`, `nsh test --target ... ` with `app.screenshot(path)`. Use whole-screen computer-use only when nothing else can do the step, and keep it short.
- If a build is slow because the machine is busy, wait. Do not lower a check because of it.

## Facts that bite

Each of these was hit by a real run. Trust them over the docs. They were first measured on 0.4.0; the 0.5.0 migration (2026-11-11) re-checked them, and each entry says how far: "re-checked on 0.5.0" means a run or the 0.5.0 source confirmed it, "carried over" means it was not re-run and is unverified on 0.5.0.

- Check `.nsh/types` for TSX and the crate source in `~/.cargo/registry/src/*/nsh-core-0.5.0` (or `~/fo/.refs/nsh-v0.5.0`) for Rust. On 0.4.0 the upstream skill was ahead of the release; 0.5.0 now ships `nsh update-manifest`, the updater, the `Notifications` module and the `notifications` capability (re-checked on 0.5.0: `nsh --help`, `nsh-core/src/element/`).
- Capability keys in 0.5.0, from `nsh-core/src/access.rs`: `windows`, `network`, `webview`, `statusBar`, `clipboard`, `picker`, `speech`, `notifications`, `audio`, `openUrl`, `process`, `fs`, `host`. A typo no longer fails `nsh build`: it prints `warn nsh.json: capabilities.<key>, not granted` and exits 0, so read the build output (re-checked on 0.5.0: `docs/evidence/project/nsh-0.5.0/phase7-fact-capability-typo-0.5.0.log`).
- `nsh_core::prelude::*` exports a `t!` of its own since 0.5.0, which collides with rust-i18n's `#[macro_use]` `t!`: a module that globs the prelude and calls `t!` needs `use rust_i18n::t;` (re-checked on 0.5.0: the migration's compile errors).
- A headless `press` lands where the view is drawn since 0.5.0: a press on a view covered by another (a scrim's center under its dialog, page buttons under an open sheet) fails with "is covered". Use `press_at` for an uncovered point, or do what a user could do. A step can also move the manual clock by a display frame while native animations finish, so measure debounces from the event itself, not from the step (re-checked on 0.5.0: `tests/overlays.rs`, nsh-form `tests/rows2.test.tsx`).
- `If` and `Each` are globals in TSX. Importing them from `nsh` is a type error (carried over).
- `nsh check` needs an `App.tsx` and a `tsconfig.json` at the package root, or an entry passed to it. Generated files in `.nsh/` are git-ignored, and `nsh` writes them (re-checked on 0.5.0 for the `tsconfig.json` half).
- The docs' `nsh-engine = "0.1"` samples are stale. Dev-dependencies must be `0.5.0`, as all NSH crates must come from one place. The generated `Cargo.toml` has no `log` crate (carried over).
- `Headless::start` can return before the first frame, so `wait_for` something first or a step sees `on screen: []` (carried over).
- `Headless::shows` takes plain text. `wait_for`, `press` and TypeScript's `app.shows` take selectors. `wait_for_gone` also takes a timeout. `screen().count` returns a `Result` (carried over).
- `Signal::update` needs `T: Clone`. For state machines holding user types, pair `stored(..)` with a version `signal`. See `~/fo/nsh-query/crates/nsh-query/src/tracked.rs` (carried over).
- A component's signals are already disposed when its `cleanup` runs, so reading one there panics. Capture what cleanup needs in an `Rc<Cell<..>>`. `create_root` gives an owner that outlives the component, which suits caches and in-flight writes (carried over).
- `open`'s host handlers need annotated parameters: `async ({ id }: { id: number }) => ...`. Selector regexes use Rust's `regex` syntax, so no look-ahead. On a manual clock, `open(entry, { clock: {} })`, a test that took 13.7 s took 18 ms (carried over).
- A TSX app consumes a sibling package through `node_modules`, with `mkdir -p node_modules && ln -s <path> node_modules/<name>`. The bundler and `nsh check` both read it, and `tsconfig.json` needs no change. Overriding `compilerOptions.paths` there breaks the `nsh` types. The sibling packages need their own `bun install` first, or `@tanstack/*` does not resolve (re-checked on 0.5.0).
- On 0.4.0, `nsh package` had no setting for extra native SDKs, URL schemes or entitlements, so anything needing those went through an embedding host (`NshAppView`/`app.handle(..)` on iOS, `NshView`/`nsh.handle(..)` on Android, as `examples/embed-ios` and `examples/embed-android` do; the app talks to it with `host.call`, `host.post` and `host.on`, listing the names in `capabilities.host`). 0.5.0's release notes add Info.plist keys, entitlements and URL schemes to packaging; this repo has not adopted them, and the embedding path is unchanged (carried over).
- Failed host calls reject with `ERR_ACCESS_DENIED` when undeclared, `ERR_NSH_HOST_ERROR` when unhandled or thrown, and `ERR_FEATURE_UNAVAILABLE_ON_PLATFORM` with no host (carried over).
- Bytecode loads only from the app bundle, the APK or a verified package (carried over).
- `nsh package --target web` and `nsh test --target web` build C code (zstd) for wasm with `AR_wasm32_unknown_unknown=llvm-ar`, and `llvm-ar` is not on this Mac's `PATH`: pass `AR_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/llvm-ar CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/clang` (re-checked on 0.5.0: `docs/evidence/project/nsh-0.5.0/phase6-package-web-with-llvm.log`).
- On this Mac, NDK 29's (and 27's) `llvm-ar` is a shell script that execs itself and never returns, so a C build for Android (the `ring` crate) hangs. `AR_aarch64_linux_android` pointed at NDK 28's real `llvm-ar` works (measured 2026-11-11; machine state, not NSH).

## How to run this

Use the `z-workflow` skill. All subagents run on GLM-5.3-Flash with high thinking. Split the work into the areas named below, with disjoint files, each advancing alone through its own chain. Name these phases for every area: implement, audit, unit tests. Add native device testing to any area touching iOS, Android or desktop, and a vision phase to any area with a screen. End with a whole-tree smoke gate. Run audit and smoke until zero issues, capped at three rounds, and report honestly if a cap is reached rather than weakening a check.

Set this session's goal to the goal above, and keep working until the satisfaction criteria hold or you are truly blocked. When you are blocked on one area, finish every other area first. Do not stop to ask whether to continue.

## Hard rules

- No real accounts, no real credentials, no billing, and no signing in as anyone. Never type a password or create an account on any provider's site. Everything here must work in a sandbox, as described below.
- Never commit a secret, a real `google-services.json` or `GoogleService-Info.plist`, or a real client secret. Fixtures with obviously fake values go under `fixtures/`.
- Do not publish to crates.io or npm, do not change repository visibility, and do not push. Commit on `main` as each milestone passes, using Conventional Commits with the reason in the body.
- Comments are 1-2 lines and state a non-obvious constraint. Never narrate the next line. Dead code is deleted. No speculative abstractions.
- Docs read like a competent human wrote them: direct, concrete, no marketing filler. The `antislop-copy` and `code-comments` skills apply.
- Never invent an API. Look the name up, in the source for 0.5.0.
- A Rust gate is `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`. A TSX gate is `nsh check`, `nsh build` with every warning fixed, and `nsh test`. No check is weakened, skipped or deleted to get a pass.
- Tests go through the UI on a manual clock, with no real network and no real time, except the labelled sandbox runs below. Every new test is seen to fail once, by breaking the code on purpose. Use `test-quality-guardrails`.
- `docs/STATUS.md` stays current with what is done, what was verified how, and what was not verified. A claim with no evidence is reported as unverified.
