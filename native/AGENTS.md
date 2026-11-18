# AGENTS.md

The brief for this repository, with its goal, sandbox, rules and satisfaction criteria, is [docs/PROMPT.md](docs/PROMPT.md). Read it first. Its hard rules bind every agent and subagent here. Track progress in `docs/STATUS.md`.

Working rules on top of that brief:

## Gates

Per area, before calling work done: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`; for TSX, `nsh check`, `nsh build` (every warning fixed), `nsh test`. Tests go through the UI on a manual clock with no real network; every new test is seen to fail once by breaking the code on purpose. `just` wraps these (`just check`, `just test`, `just test-tsx`, `just smoke`).

## Ownership

File areas are disjoint. Before touching anything, find its area and stay in it; the docs describing an area's code live with the area that owns the code, except the files below.

- **shell**: `src/lib.rs`, `src/main.rs`, `src/app`, `src/shell`, `App.test.tsx`, `smoke/`, `tests/shell.rs`
- **foundation + state**: `src/foundation`, `src/state`, `src/services/{mod,http,auth,analytics,monitoring}.rs`, `tests/foundation.rs`
- **ui**: `src/ui`, `themes/theme.json`, `tests/ui.rs`
- **forms**: `src/features/forms`, `tests/forms.rs`
- **data**: `src/features/data`, `src/services/fake_api.rs`, `tests/data.rs`, `trace/`
- **lists**: `src/features/lists`, `tests/lists.rs`
- **canvas**: `src/features/canvas`, `tests/canvas.rs`
- **motion**: `src/features/motion`, `tests/motion.rs`
- **overlays**: `src/features/overlays`, `tests/overlays.rs`
- **platform**: `src/platform`, `i18n/`, icon assets, `nsh.json`, `tests/platform.rs`
- **remote-screen**: `packages/remote-screen`, `src/features/remote_screen`, `tests/remote_screen.rs`
- **project**: `README.md`, `CONTRIBUTING.md`, `AGENTS.md`, `CHANGELOG.md`, `justfile`, `scripts/rename.sh`, `docs/architecture.md`, `docs/coverage.md`, `docs/gpui-starter-parity.md`, `docs/upstream-issues.md`

`docs/PROMPT.md` and `docs/STATUS.md` belong to the orchestrator. If your docs depend on another area's facts (page names, file paths), draft from the plan, then re-verify against the tree in your audit round and fix.

## Evidence

- Every satisfaction-criteria claim needs evidence: the command you ran and its output path under `docs/evidence/<area>/`.
- Look at every PNG you save. A screenshot nobody looked at proves nothing.
- Record numbers (trace mount counts, timings) in `docs/STATUS.md`.
- A claim with no evidence is reported as unverified, in your section and in the docs you write.

## Shared machine

Other agents build other repos on this Mac right now. Stay inside this checkout; reading `~/fo/.refs`, `~/fo/nsh-query` and `~/fo/nsh-form` is fine, writing to any other repository never is. Never run `shutdown-all`, `erase-all`, `kill-server` or `killall`; never kill a process or port holder you did not start. Make your own simulator/AVD names after this repo.

## No git machinery

Subagents do not commit, branch, stash or reset; the orchestrator commits. Edit the current checkout only.

## Facts that bite

`docs/PROMPT.md` section "Facts that bite" overrides any docs, including upstream skills, when they disagree. Verify every nsh subcommand and flag against `nsh --help` and the 0.5.0 source (`~/fo/.refs/nsh-v0.5.0`) before documenting it.
