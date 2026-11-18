# native/AGENTS.md

EasyQuran native app on Native Shell (NSH 0.6.0), Rust mode. One crate graph for macOS, iOS, Android (Linux/Windows from the same code). Counterpart of `web/` at 1:1 feature parity; the checklist is [PARITY.md](PARITY.md). The monorepo's root `AGENTS.MD` hard rules bind here too.

## Before any work

Load skills `caveman`, `nsh`, `nsh-design-guides`. Read the nsh skill's `references/rust.md`, the "Rules for coding agents" section of `references/coding-guides.md`, and the design guide sections named by the design skill. Never invent an NSH API: look it up in the resolved crate source (`cargo metadata --format-version 1` gives the paths) before writing it.

## Hard rules

- Never edit `web/`, `documentation/`, `db/`, `rust/` or the repo-root files. Read them freely. A needed backend change is reported as a proposal, not made.
- Quran databases are immutable: opened read-only, identified by **id**, never hashed (no SHA-256 over Quran data anywhere), never modified, never versioned. Text goes verbatim from database to screen: no trimming, no normalising.
- Do not invent features `web/` lacks; do not drop ones it has. Mirror the web behaviour named in PARITY.md.
- No nested ternaries / no nested `if … else` expressions used as a ternary chain: use a named function with early returns or a `match`.
- No raw colours (`rgb(`, hex) outside `themes/` and `src/ui/tokens.rs`. Screens import controls from `src/ui` only, never `nsh_base` directly.
- User-facing strings come from the web catalogues through `eq_core::i18n` (`tr!`). No hard-coded English in screens.
- Session value (`x-eq-session`), CSRF token and device key never appear in logs, traces, analytics or error reports.
- Subagents do not commit, branch, stash or reset. The lead commits.
- Other agents build on this Mac. Never `killall`, never kill a process you did not start, never erase simulators.

## Layout

```
native/
  Cargo.toml            workspace: app crate (root) + crates/eq-core
  nsh.json              app manifest, capabilities
  themes/theme.json     semantic tokens (palettes, light/dark)
  assets/               fonts, icons
  crates/eq-core/       pure Rust domain logic. No nsh-* dependency. cargo-testable without an engine
    src/i18n/           catalogue baked at build time from web/messages/**
    src/quran/          catalogue (baked from web data), read-only SQLite, divisions, names, view rules, words, search
    src/routes.rs       reader URL grammar (deep links, share links)
    src/net.rs          Transport trait, request/response types (lead-owned)
    src/kv.rs           KeyValue + SecretStore traits, MemoryKv (lead-owned)
    src/clock.rs        Clock trait, SystemClock, ManualClock (lead-owned)
    src/api/            session transport (x-eq-session), CSRF, endpoints, wire types
    src/device/         install id, P-256 device key, register
    src/sync/           ids (UUID v7), clock, registers on eq-sync-core, outbox, engine
    src/store/          local app database (rusqlite): rows per register, kv
    src/audio/ src/search/ src/reading/ src/home/ src/settings/   feature logic with no UI
  src/                  the NSH app crate
    lib.rs main.rs      root, native module (lead-owned)
    app/                providers: stores made once, context
    shell/              routes, nav stack, sidebar/tabs, palette host, shortcuts
    ui/                 app-styled wrappers over nsh_base, tokens.rs token accessors, Arabic text
    services/           NSH-backed impls of eq-core traits: fetch transport, storage, files, audio, notifications; http/auth/analytics/monitoring seams
    stores/             Copy stores of signals over eq-core state
    features/<area>.rs  screens, one module per area (a big area becomes a directory); page fn signatures are the shell's mounting contract
    foundation/ state/ platform/   starter foundation: errors, config, logging, capabilities, event bus, tasks, diagnostics, locale
  tests/                headless UI tests (nsh-testing)
```

Domain logic goes in `crates/eq-core` (plain types, injected transport and storage, `cargo test -p eq-core`). The app crate holds signals, screens and NSH service impls only.

## Gates

`cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p eq-core`, `cargo test` for headless UI tests touched. On battery (`pmset -g batt`): fmt + debug check only. Tests follow the root policy: write them where a bug is costly (sync merge, session transport, device keys, Quran text guards, route grammar), not for copy or layout.

## Facts that bite

- A component body runs once; pass closures/signals to setters for anything that changes.
- `Signal::update` needs `T: Clone` and always notifies.
- `blocking`/`background` closures cannot capture signals: return a value, write it in `spawn`.
- SQLite and file I/O are blocking: run through `blocking(..)`, never on the engine thread.
- Device registration: the backend accepts `client` in `web | react_native | gpui` only. Until the owner approves a backend change, this app registers as `react_native` on iOS/Android and `gpui` on desktop (see PARITY.md "Proposed backend changes").
