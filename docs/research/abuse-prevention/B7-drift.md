# B7 — Repo drift audit: what else rotted, which invariants are unguarded

Repo: `/Users/hmziq/os/easyquran` @ HEAD `53d3d421` (master). The docs restructure that was uncommitted at audit
time has since landed; doc references below are to the current tree.

Scope: Q7 of DESIGN-v1. Established fact (not re-verified): `53d3d421 fix(ci): remove security
publication gates` deleted the web/Rust test, lint, fmt, clippy and dependency-audit gates from
`.github/workflows/images.yml` and touched `rust/deny.toml`, `deploy/README.md`, `web/README.md`.

---

## 1. What the pipeline verifies now, and what it does not

### 1.1 The pipeline, exactly

`.github/workflows/images.yml` @ HEAD has four jobs:

| Job       | Lines   | Steps that verify anything                                                                                                                                              |
| --------- | ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `web`     | 62–146  | `bun install --frozen-lockfile` (102); `bun run quran:check` (104–105); `bun run --filter web build` (110)                                                              |
| `api`     | 148–233 | `cargo build --release --locked -p ruxlog --features vendored-openssl --target aarch64-unknown-linux-gnu` (187–192); runtime-linker check on the loaded image (220–226) |
| `publish` | 239–266 | pushes the tars; only job with `packages: write`; never runs on PR                                                                                                      |
| `deploy`  | 269–292 | Dokploy redeploy on master                                                                                                                                              |

`bun run quran:check` = `quran:typecheck` (tsc on `scripts/quran`) + `quran:test` — exactly three
tsx test files: `scripts/quran/{upload,qurancom-mirror,qurancom-parity}.test.ts`.

### 1.2 Removed by `53d3d421` (diff `37d90c41..HEAD`)

```
- Web security gates:        bun run check; bun run lint; bun run test        (web/)
- JavaScript dependency audit: bun audit
- Rust security gates:       cargo fmt --all --check
                             cargo clippy --workspace --all-targets --locked -- -D warnings
                             cargo test --workspace --locked -- --test-threads=1
                             cargo check --locked -p ruxlog --features seed-system \
                               --bin ruxlog_tui --bin seed_now
- Install dependency auditor: cargo install cargo-audit --version 0.22.2 --locked
- Rust dependency audit:     python3 -m unittest discover -s scripts/security -p 'test_*.py'
                             python3 scripts/security/cargo-audit-gate.py
```

### 1.3 What can now reach production unverified

A master push (or `v*` tag) → `publish` (needs `[meta, web, api]`) → `deploy`. `publish` has no
gate dependency beyond the two build jobs. Everything below ships if the build compiles:

- **svelte-check** (`--fail-on-warnings`): every type error and every a11y/Svelte compiler warning.
  Vite build strips types without checking and does not fail on Svelte warnings.
- **oxlint** (`--deny-warnings`): `no-nested-ternary`, `no-shadow`, `no-floating-promises`,
  `no-misused-promises`, `unicorn/no-array-callback-reference`, `unicorn/prefer-number-properties`,
  `prefer-promise-reject-errors`, and all 16 `anti-slop/*` rules (`web/vite.config.ts:26–64`).
- **2,399 web tests / 190 files** — every web machine guard (nav-guard, catalogue-sha-guard,
  docs-divergence-guard, message-barrel-guard, route-isolation ×2, logical-properties, design
  guards, account-isolation).
- **cargo clippy `-D warnings`** and **cargo fmt**.
- **1,398 Rust tests** (serial): `quran_v1.rs` sha/CORS/route guards, `security_tests.rs` PII
  source scans, `quran_prod_gate.rs`, `admin_bans_export.rs`, `rux-request-gate` lockstep,
  `ruxlog-types` error-code snapshot, migration tests.
- **`cargo check --features seed-system`** for `ruxlog_tui` / `seed_now` (the seed binaries can
  silently stop compiling).
- **`bun audit`** (currently clean: 464 packages, no advisories, 0.3 s).
- **`cargo-audit-gate.py` + its 6 unit tests** (see §2).
- **i18n is still checked** (`prebuild` runs `i18n:check` inside `bun run --filter web build`), and
  the Quran publisher tests still run. Those two are the only survivor gates.

---

## 2. `rust/deny.toml` and the audit gate

### 2.1 deny.toml diff

`git diff 37d90c41 HEAD -- rust/deny.toml` = **comment wording only**:

```diff
-# Publication uses scripts/security/cargo-audit-gate.py in images.yml.
+# Local dependency graph audit: scripts/security/cargo-audit-gate.py.
-  ... CI cargo-audit-gate.py rejects it if enabled. ...
+  ... The local cargo-audit-gate.py check rejects it if enabled. ...
```

Policy is NOT relaxed in the file: `yanked = "deny"`, `unmaintained = "all"`, `unsound = "all"`,
`[sources] unknown-registry/git = "deny"`, the license allowlist, and the
`RUSTSEC-2023-0071` (rsa) ignore entry with its reason are all unchanged. What was relaxed is
**execution**: nothing runs deny/audit in CI anymore.

### 2.2 The gate script still works

`scripts/security/cargo-audit-gate.py` still fails on any finding except the one narrow exception
(`UNUSED_ADVISORIES = {RUSTSEC-2023-0071: rsa}`, lines 9–11) when that exact package/version is
absent from the aarch64 graph (`assess`, lines 30–48). Operational/parser errors exit 1 (lines
69–73, 86–90).

Ran locally:

```
$ python3 scripts/security/cargo-audit-gate.py
UNUSED LOCKFILE PATH (verified absent from aarch64-unknown-linux-gnu graph): RUSTSEC-2023-0071: rsa 0.9.10
Dependency gate passed; unused lockfile findings above remain visible.
(4.7 s warm)

$ python3 -m unittest discover -s scripts/security -p 'test_*.py'
Ran 6 tests in 0.000s — OK
```

So: **the gate still fails on new advisories when run, but no automated path runs it**, and the
RSA/RUSTSEC exception is no longer displayed anywhere in CI (the script's whole point — keep the
accepted finding visible — is lost until someone runs it by hand).

---

## 3. Docs that are now false

`deploy/README.md` and `web/README.md` were **already corrected by `53d3d421`** — they no longer
claim publication gates (`deploy/README.md:18–35`; `web/README.md` "Check/lint/test and dependency
audits remain local checks"). The coordinator's premise is outdated for those two. What is still
false:

| Doc                                              | Claim                                                                                                                     | Status                                                                                                                                                      |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `documentation/history.md:36`                    | "Resolved since: CI now runs the web/Rust suites (`.github/workflows/images.yml`)"                                        | **FALSE.** CI runs neither suite.                                                                                                                           |
| `DEPLOYMENT.md:69`                               | "Translations are **not** fetched in CI — the web build never reads them, and the api gets them from the host bind mount" | **FALSE since `37d90c41`**: api job runs `deploy/fetch-quran-db.sh all` (images.yml:180–181), which fetches every translation sqlite.                       |
| `DEPLOYMENT.md:74–76`                            | "The three gates (`pnpm check`, `pnpm lint`, `pnpm test`) and `cargo clippy` / `cargo test` are not in it"                | Statement true at HEAD, but **pnpm-era names** (repo is bun since `92450204`/`64b3385f`) and the advice ("add as a separate job") is exactly the open work. |
| `rust/backend/api/AGENTS.md` hard-rule paragraph | "`tests/quran_v1.rs` guards fail the build"                                                                               | **FALSE as machine claim**: the build no longer runs them.                                                                                                  |

Not false but now unbacked (requirement lists, previously read as enforced by the removed gates):
`AGENTS.MD` ("All three gates are green and must stay green"), `docs/next-features.md:24–25`,
`documentation/search-system.md:212`, `documentation/settings-system.md:272`, `documentation/design-system.md:91,680`.

---

## 4. Invariants asserted only by tests that no longer run

All of these execute only via `bun run test` (web vitest) or `cargo test` (Rust). Neither runs in
CI; neither runs in the commit hook.

**Web machine guards** (`web/src/**/__tests__/`): `nav-guard` (reader href grammar + translation
context — AGENTS.MD machine-guard claim), `catalogue-sha-guard` (no SHA-256 in published
catalogues), `docs-divergence-guard` (documentation/quran-system.md divergences vs code), `message-barrel-guard`
(i18n barrel/tree-shaking), `route-isolation` (settings + search), `logical-properties` (RTL
physical-property ban), `marketing-surface-guard`, `landing-guard`, `usage-bar-guard`,
`account-isolation`, plus design guards (token-contrast, fonts, geometry).

**Rust guards**:

- `rust/backend/api/tests/quran_v1.rs`: `cors_no_cookies_on_every_public_route:843` (ACAO `*`, no
  credentials, **no `Set-Cookie`** on all 31 public routes; route-count coverage at 879–886),
  `modules_quran_v1_has_no_sqlite_refs:926`, `w8a_anonymous_quran_headers_remain_public_with_csrf_cookie:1094`,
  `scripts_endpoint_carries_no_sha256:1237`, `api_response_text_equals_store_source:1301`,
  `sources_rows_never_carry_sha256:1665`.
- `rust/backend/api/tests/security_tests.rs`: `auth_path_logs_contain_no_sensitive_values:343`,
  PII tracing captures at 603/730/777/853 (source scans via `include_str!`).
- `rust/backend/api/tests/quran_prod_gate.rs`: production refuses IndoPak/Tajweed scripts.
- `rust/backend/api/tests/admin_bans_export.rs`: export-token isolation (227/268/294/317).
- `rust/backend/api/crates/rux-request-gate/src/layer.rs:569` `canonical_unit_key_mirrors_ban_unit`
  (IP ban namespace lockstep).
- `rust/crates/ruxlog-types/src/error.rs:436` `status_code_map_is_total_and_stable` (error-code
  snapshot).

### Ranking by blast radius (for this feature)

1. **SHA-free Quran-data guards** (web `catalogue-sha-guard` + Rust `scripts_endpoint_carries_no_sha256`
   / `sources_rows_never_carry_sha256`). Hardest rule in the repo; a device digest placed in the
   Quran tree breaks data identity silently.
2. **Public-route header/cookie/CORS guards** (`cors_no_cookies_on_every_public_route`,
   `w8a_anonymous_quran_headers_remain_public_with_csrf_cookie`, `api_response_text_equals_store_source`).
   A device cookie/header on reader routes flips `requestHasCookie` in `web/src/hooks.server.ts` →
   `cacheable` false → the 7-day SSR disk cache silently stops caching. No error; slower pages.
3. **`security_tests.rs` PII source scans** — a device key in a log line is a privacy leak the
   scans would catch.
4. **`admin_bans_export.rs` + `canonical_unit_key_mirrors_ban_unit`** — a device ban entering the
   IP ban namespace/export feed.
5. **`nav-guard` + `docs-divergence-guard`** — reader URL grammar and the SSR/SSG divergence
   contract.
6. **message-barrel / route-isolation / logical-properties / design guards** — unrelated to device
   work, but part of the now-unguarded surface.

---

## 5. The pre-commit hook (verified empirically)

Chain: `git config core.hooksPath` = `.vite-hooks/_` → `.vite-hooks/_/pre-commit` wrapper →
`.vite-hooks/pre-commit` = `vp staged` → root `vite.config.ts:4–6` `staged: { "*": "vp check --fix" }`.

`.vite-hooks/_/pre-push` exists as a wrapper, but `.vite-hooks/pre-push` does not → no-op. No Rust
coverage of any kind at commit time (`vp check` does not touch `.rs`).

**`vp check` uses the workspace-root `lint` and `fmt` blocks only.** Vite+ docs, verbatim:
`node_modules/vite-plus/docs/config/lint.md`: "`vp check` uses the workspace-root `lint` block when
it exists, including from a package directory. Package configs do not replace these lint settings
in `vp check`." Same statement in `docs/config/fmt.md`; `docs/guide/troubleshooting.md:29`:
"`vp check` selects the workspace-root `lint` and `fmt` blocks when they exist."

Root `vite.config.ts:13–17` lint block has exactly one rule: `vite-plus/prefer-vite-plus-imports`.
Root `fmt` block (`:7–12`) has only the qurancom ignore. The web rules live in
`web/vite.config.ts:26–64` and are invisible to `vp check`.

**Empirical proof** (probe files, since removed):

```
$ cd web && bun x vp lint src/lib/__drift_probe__.ts
src/lib/__drift_probe__.ts:2:17: error eslint(no-nested-ternary): Do not nest ternary expressions.

$ bun x vp check web/src/lib/__drift_probe__.ts        # from repo root
pass: Found no warnings, lint errors, or type errors in 1 file
```

```
$ cd web && bun x vp fmt --check src/lib/__drift_probe2__.ts   # unsorted imports
Format issues found in above 1 files.

$ bun x vp check web/src/lib/__drift_probe2__.ts        # from repo root
pass: All 1 file are correctly formatted
```

Bonus: root `vp check` type-resolves web files from the root, so a real web dependency reported
`x typescript(TS2307): Cannot find module 'zod'` — its type checking of web files is unreliable in
both directions.

**Conclusion**: at commit time the hook enforces fmt via the root block, one root lint rule, and a
mis-resolved typecheck. It does NOT enforce `no-nested-ternary`, `no-shadow`, `no-floating-promises`,
any `anti-slop/*` rule, web import/Tailwind sorting, `svelte-check`, or any test. Every
"machine-guarded by `bun run lint`" claim in `AGENTS.MD` is now a human promise on both ends
(hook and CI).

---

## 6. Other rot

1. **Stale CI comments describing the removed gates** — `.github/workflows/images.yml:12–15`
   ("the Rust gates need `all` — cargo test opens the corpus"), `:94–96` ("the generated runtime
   catalogue the web sha-guard test reads"), `:170–173` ("cargo test opens the corpus … before the
   gates"). The api job still provisions every translation (`:180–181`) that only `cargo test`
   needed — pure waste now, and misleading to the next maintainer.
2. **No TODO/FIXME/HACK in code** (only doc prose `CATEGORY_XXX` in
   `rust/backend/api/docs/ERROR_HANDLING.md`). Clean.
3. **Generated artifacts fresh** — `bun run i18n:namespaces:check` → "16 namespaces up to date
   (699 messages)"; running the writers produced no tracked-file diff.
4. **Docs restructure** (uncommitted at audit time, since landed): the false CI claim survives at
   `documentation/history.md:36`.
5. **Only one workflow** (`.github/workflows/images.yml`); no dependabot/secret-scanning config in
   the repo, so removing the audits leaves zero automated dependency signal.
6. `deploy/fetch-quran-db.sh` exists and works (CI still calls it); `deploy/provision-quran.sh`,
   `deploy/dokploy-auto-update.sh`, `justfile` recipes (`web-check`, `api-test`, `api-lint`) all
   intact. Manual audit commands (`just quran-audit`, `web/ bun run audit:arabic`) intact.

---

## 7. Which unguarded invariant would a device-fingerprinting change most likely break silently?

Two, in order:

1. **Device token/header/cookie reaching reader routes.** DESIGN-v1 says "Never on reader routes"
   and "HttpOnly cookie, Path=/api". If the token rides a cookie (or the header reaches SSR), the
   `cacheable` predicate in `web/src/hooks.server.ts` flips on `requestHasCookie` and the 7-day
   SSR disk cache silently stops caching — no error, only slower pages and a privacy-shaped leak.
   The Rust guards that would catch it (`cors_no_cookies_on_every_public_route`,
   `w8a_anonymous_quran_headers_remain_public_with_csrf_cookie`) do not run; A6's proposed G2/G5
   guards do not exist yet.
2. **A device digest (HMAC/SHA) landing in a sha-free Quran-adjacent file** (`web/src/lib/quran/wire.ts`,
   `web/src/lib/data/quran-types.ts`, or a `quran_v1` payload). The repo's loudest hard rule is
   "no SHA-256 over Quran data"; `catalogue-sha-guard` + `scripts_endpoint_carries_no_sha256` +
   `sources_rows_never_carry_sha256` exist precisely to fail the build — and do not run. A6 §6
   already flags that device code must stay out of the sha-guard file list; nothing enforces it.

Both failures are invisible at runtime: no exception, no 5xx — only a test that no pipeline runs.

---

## 8. Minimum restoration, order, and measured cost

Measured on this machine (warm, M-series; not CI-cold):

| Gate                                                  | Measured       | Notes                                                              |
| ----------------------------------------------------- | -------------- | ------------------------------------------------------------------ |
| `web: bun run check`                                  | **9.5 s**      | svelte-check 0 errors/warnings + tsc worker; includes i18n compile |
| `web: bun run lint`                                   | **3.9 s**      | `vp lint --deny-warnings`; includes anti-slop sync + i18n          |
| `web: bun run test`                                   | **11.2 s**     | 190 files / 2,399 tests passed; vitest 8.3 s                       |
| `bun audit`                                           | **0.33 s**     | clean, 464 packages                                                |
| `rust: cargo fmt --all --check`                       | **0.5 s**      |                                                                    |
| `scripts/security` unit tests                         | **<0.1 s**     | 6 tests                                                            |
| `cargo-audit-gate.py`                                 | **4.7 s** warm | CI adds `cargo install cargo-audit 0.22.2` ≈ 1–2 min unless cached |
| `cargo clippy --workspace --all-targets -D warnings`  | not measured   | the expensive one on a cold runner; `rust-cache` mitigates         |
| `cargo test --workspace --locked -- --test-threads=1` | not measured   | 1,398 tests, serialized by design (password admission semaphore)   |

Order (cheapest, highest-value first):

1. **Restore the web gate block** (`check`, `lint`, `test`, `bun audit`) — sub-minute compute;
   brings all web machine guards back.
2. **Restore the Rust gate block** (`fmt`, `clippy -D warnings`, `test --test-threads=1`,
   seed-system `cargo check`, then the python audit tests + `cargo-audit-gate.py`) — the costly
   one; provisioning (`deploy/fetch-quran-db.sh all`) is already wired.
3. **Fix the commit hook** so local enforcement matches CI: move the web rules to root
   `lint.overrides` / `fmt.overrides` (the documented Vite+ monorepo pattern) or add a
   `web/**` staged command (`cd web && vp check --fix`). Without this, step 1 only catches drift at
   push time.
4. **Doc cleanup**: `documentation/history.md:36`, `DEPLOYMENT.md:69,74`, `rust/backend/api/AGENTS.md`
   hard-rule sentence, images.yml comments.

Restore as **separate required jobs**, not inline in the build jobs:

- `gates` (web) and `rust-gates`, `permissions: contents: read`, no secrets, no registry token,
  run on PR + push. Add both to `publish.needs` (`images.yml:240`) so publication is gated by
  branch-protection-visible checks rather than by steps inside the jobs that build the artifacts.
- Rationale: publish keeps the only `packages: write` token; gates can be required and re-run
  without rebuilding images; a PR runs its own workflow copy, so the gate must be an explicit
  `needs`, not a step.
- Known risk from the one day the gates existed: they were added in `bed80b13`, failed on a fresh
  runner for missing corpus, fixed in `37d90c41` (provisioning changes remain), and were removed in
  `53d3d421` **before ever being observed green in CI**.
  Expect one provisioning/serialization pass before trusting them.

---

## 9. Findings (condensed)

1. CI verifies builds only — all test/lint/type/fmt/clippy/audit gates removed (CRITICAL).
2. Commit hook cannot enforce web lint/fmt rules — `vp check` reads root config only (HIGH, NEW empirical).
3. `cargo-audit-gate.py` still works and still fails on new advisories; nothing runs it; RSA exception no longer displayed (HIGH).
4. `documentation/history.md:36` + `DEPLOYMENT.md:69` are false; `deploy/README.md`/`web/README.md` already fixed (MEDIUM).
5. Stale CI comments + dead `all` provisioning in the api job (LOW).
6. All machine guards (web + Rust, ranked) unenforced (HIGH).
7. Device cookie/header on reader routes → silent SSR-cache loss; device digest in sha-free file → hard-rule break; both unguarded (HIGH).
8. Hook chain / no pre-push / no Rust coverage (MEDIUM, NEW detail).
9. No TODO rot, generated artifacts fresh, one workflow, no dependabot (INFO).
10. Restoration costs measured; restore as separate `gates` + `rust-gates` jobs in `publish.needs` (RECOMMENDATION).
