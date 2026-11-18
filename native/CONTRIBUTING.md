# Contributing to nsh-starter

## Setup

Follow [README.md](README.md) — nsh 0.5.1, the Rust targets, Xcode and the Android SDK, and `../nsh-query` and `../nsh-form` checked out beside this repo.

Work happens in disjoint file areas so parallel changes cannot collide; the map of who owns what is in [AGENTS.md](AGENTS.md). Do not edit files outside your area, including other areas' tests.

## The gates

No check is weakened, skipped or deleted to get a pass. Everything green before you call the work done:

```sh
# Rust gate
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test

# TSX gate (repo root: the TSX entry's tests)
nsh check
nsh build          # fix every warning it prints
nsh test

# TSX gate (packages/remote-screen), same three commands in that directory
```

`just check`, `just test` and `just test-tsx` run these. `just smoke` is the whole-tree headless gate. Real-input smoke runs go per target: `nsh test --target macos|ios|android` (web passes only what exists on the web; list the rest in `docs/STATUS.md`).

`nsh build` and `nsh check` are not interchangeable: a typo in a `nsh.json` capability key fails `nsh build` and passes `nsh check`. Run both when you touch capabilities.

## Test rules

- Tests go through the UI: open the app or page, press, type, assert on screen. No reaching into internals that a real user could not touch.
- **Manual clock.** No real time and no real network in tests. Anything async runs against a manual/fake clock — a test that waits on wall time took 13.7 s and 18 ms on a manual clock here; take the 18 ms.
- **Seen to fail.** Every new test is seen to fail once, by breaking the code on purpose, before it counts as coverage. A test that never failed proves nothing about what it detects.
- No real accounts, credentials or billing, anywhere. Fixtures with obviously fake values live under `fixtures/`.
- Screenshots and logs you save as evidence go under `docs/evidence/<area>/`, and you look at every PNG you save — a screenshot nobody looked at proves nothing.
- Keep tests fast enough to run on every change; the headless runner gives each test its own engine.

## Doc standards

- Prose reads like a competent human wrote it: direct, concrete, no marketing filler. No "seamless", no "leverage", no rule-of-three padding.
- Code comments are 1-2 lines and state a non-obvious constraint — why the code is shaped this way, a workaround, an invariant. Never narrate the next line. Never explain a diff in code; that belongs in the commit message.
- Dead code is deleted, not commented out. No speculative abstractions; write the second implementation when the second use case exists.
- Every API name in docs is verified against nsh 0.5.1 (`nsh <cmd> --help`, the `.nsh/types` declarations, or the crate source). Never invent an API.
- `docs/STATUS.md` stays current: what is done, what was verified how (command + evidence path), what is unverified. A claim with no evidence is unverified.

## Commits

Conventional Commits (`feat:`, `fix:`, `docs:`, ...), subject under 72 characters, body explaining why. The orchestrator commits for parallel builds; humans commit their own work.

## When a library misbehaves

If `nsh-query` or `nsh-form` has a bug, do not fork or patch them: write it up in [docs/upstream-issues.md](docs/upstream-issues.md) with a reproduction and a workaround, and work around it in your area.
