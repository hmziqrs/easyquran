# C6 — Plan-shape review: the final abuse-prevention plan document

**NEW.** Reviewer pass over the _shape_ of the plan that will be written to `docs/` — not the design.
Inputs read: `AGENTS.MD`; `docs/quran-system.md`; `docs/security.md` (working tree; replaces the
staged-deleted `docs/security-hardening-2026-10-11.md`, read from HEAD via `git show`); `docs/next-features.md`;
`docs/locale-suggest.md`; `docs/README.md`; `docs/history.md`; `docs/remaining/feature-gap-catalogue.md`
(P09/P10); `docs/remaining/copy-corrections.md` (C02); `web/README.md`; `deploy/README.md`;
`DESIGN-v1.md`; `A3`, `A5`, `A6`, `B6`, `B7`, `B8`, `B10`; `web/src/routes/(application)/__tests__/docs-divergence-guard.test.ts`;
`web/src/lib/i18n/privacy-copy.ts`; `rust/backend/api/src/services/scheduler.rs`; `main.rs`; migration `lib.rs`.

**Missing input (NEW).** `C5-decision-ledger.md` does not exist in this directory. The plan must build
its own decision ledger from `DESIGN-v1.md` + the A/B/R2 reports. Do not cite a C5 file.

**Working-tree fact (NEW).** An uncommitted docs restructure is in the tree (106 entries): 76 staged
deletions (incl. all three `security-*-2026-10-11.md` long-form reports), untracked new
`docs/README.md`, `docs/history.md`, `docs/indopak.md`, `docs/locale-suggest.md`, `docs/security.md`.
The plan must be authored against the **post-restructure tree**: index it in `docs/README.md`, treat
`docs/security.md` as the live security record, and never resurrect deleted long-form docs
(replaced text lives in git history; overviews name the commit to `git show`).

---

## 1. Target path, filename, one doc vs plan + companion

**Target (primary):** `/Users/hmziq/os/easyquran/docs/abuse-prevention.md` — one file, kebab-case,
top-level under `docs/` (the old `docs/plan/` folder is gone from the working tree; long-form plans
compacted to top level, e.g. `locale-suggest.md` came from `docs/plan/09-locale-suggest.md`).

**Alternative if the owner keeps the device framing:** `docs/device-identity.md`. Do not use both.
Do not date the filename — dates belong to records (`security-hardening-2026-10-11.md` @ HEAD), not
to live plans (`next-features.md`, `locale-suggest.md`).

**One doc. No companion verification doc (NEW).** Precedent, three ways:

| Precedent                                      | What it proves                                                                                                                                                                            |
| ---------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `docs/locale-suggest.md`                       | A draft spec is a single self-contained doc: Status, guarantee/non-negotiables, spec sections, Docs to amend, Tests, manual matrix, Open questions.                                       |
| `docs/next-features.md` (~1,350 lines)         | A large batch stays one doc: §0 context, per-feature Build/Acceptance/Touch, §9 normative design, §10 EC catalogue, §11 open questions, §12 sequencing, §13 absorbed map, §14 superseded. |
| `docs/security-hardening-2026-10-11.md` (HEAD) | The security-record template: Implemented protections / Evidence and limits / Rollout / Final checks — one doc, with rollout + verification inside it.                                    |

The plan's rollout/verification sections are the "companion" — inline, not a second file. A separate
verification doc would duplicate the manual matrix + abort criteria and rot independently.

**Post-landing compaction:** when it lands, the record compacts into `docs/security.md` (or
`history.md`), matching how the three 2026-10-11 reports were folded. The plan should say so.

---

## 2. Heading skeleton (exact), with what each section must contain for THIS feature

```
# Abuse prevention and device identity            ← H1; topic title, not a status word

Status: **draft, not implemented.** Baseline: current-state claims measured <date> at <commit>.
Relations: next-features.md wins on conflict for sync/bookmark/reading-session work; this doc wins
for abuse-prevention, device identity and legal artifacts; my-plan-raw.md is read-only.
Evidence: round audits (A1–A10, B*, R2-*) summarised in §15; raw reports are working material.

## 0. Context the implementing agent must load first
## 1. Guarantee and non-negotiables
## 2. Current state (measured, file:line)
## 3. Scope decisions — adopted, dropped, deferred
## 4. Workstreams (each: Build / Acceptance / Touch)
## 5. Data model and retention
## 6. Enforcement rules, shadow mode, promotion gates
## 7. Rollout and migration safety
## 8. Tests
## 9. Manual verification matrix
## 10. Docs to amend in the same change
## 11. Open questions for the owner (with defaults)
## 12. Non-goals
## 13. Sequencing and commit plan
## 14. Gate commands and their enforcement status
## 15. Evidence appendix — decision ledger
```

Per-section content notes:

- **Status block.** `locale-suggest.md:3` shape: `Status: **draft, not implemented.**` + which doc
  wins on conflict (next-features §Authority precedent) + baseline commit/date (`next-features.md:13`
  precedent: "measured 2026-10-11 at `5b0674b2`"). Must state CI gates are not currently enforced
  (see §14).
- **§0 Context.** What to load (`AGENTS.MD`, `web/README.md`, `rust/backend/api/AGENTS.md`,
  `rust/backend/api/migration/README.md`, `deploy/README.md`, caveman incl. subagents). The repo
  facts that shape every step: pool = 1 connection, no staging environment, every master push
  auto-deploys, `user_bans` exists with zero callers, `BanUnit` is IP-only, device_v1 = FCM.
  **Hard-rule compliance statement goes here:** Quran DBs immutable; `db/` gitignored; no SHA-256
  over Quran data in any automated path (device HMAC is unrelated crypto and must stay out of the
  sha-guarded files — `web/src/lib/quran/wire.ts`, `web/src/lib/data/quran-types.ts`, the
  `catalogue-sha-guard` SHA_FREE list); translated reader routes stay SSR + 7-day disk cache, never
  SSG; no nested ternaries; es-toolkit named imports; no standalone `.oxlintrc.json`.
- **§1 Guarantee and non-negotiables.** One-sentence guarantee (locale-suggest §1 shape), then the
  numbered non-negotiables: no device signal on reader routes (`/t/**`, `/{surah}/t/**`, quran
  content API, internal SSR); no device × reader-data join; no automated L5/L6; no reuse for
  analytics/personalisation; retention enforced by `expires_at` + sweeper, not prose; account and
  credential first, device is evidence only; minors and India handled.
- **§2 Current state.** Table with `file:line` for: `user_bans` model + `check_ban` zero callers;
  `BanUnit` IP-only (`dto.rs:16-43`); fixed limits 600/30/600/120; escalation default-off;
  `device_v1` FCM + `MAX_DEVICES_PER_USER = 20`; `AuthSessionState` no serde attrs (`state.rs:6-26`);
  `sessions.device` overloaded (UA / NULL / WebAuthn type); `audit_logs` zero writers; no appeals
  anything; privacy page placeholder ("not legal advice"); CI gates deleted in `53d3d421`; commit
  hook blind to `web/vite.config.ts` rules. Baseline commit pinned.
- **§3 Scope decisions.** Fold `DESIGN-v1.md` §A (v0→v1 verdict table) and the dropped controls with
  reasons: device key as ban key, `UNIQUE(device_key)`, WASM/JS timing, client-authored coherence,
  per-request device eval, step-up via fresh-auth grants, `BanUnit::Device`, `audit_logs` reuse,
  weekly epoch rotation. Precision lines that must survive: "not biometric" is correct; "not
  profiling" is false — trust scoring is Art 4(4) profiling.
- **§4 Workstreams.** `next-features` shape (numbered Build steps → **Acceptance** → **Touch**).
  W1 wire `user_bans` (create/revoke/`check_ban` + `/admin/users/bans`); W2 per-credential and
  per-API-key rate limits; W3 client collector (injected `DeviceSignals` seam, closed payload, lazy
  worker, browser adapter the only file touching browser APIs); W4 server data model + retention;
  W5 enforcement ladder + shadow + pause endpoint; W6 appeals/notices/operator surfaces; W7 legal
  artifacts (DPIA, LIA, per-MS ePrivacy, ROPA, retention schedule, policy rewrite + device addendum,
  Art 17 statement-of-reasons template, appeal SOP, DSAR SOP, legal-hold SOP + `legal_holds`, breach
  runbook, processor DPAs, children's memo — each with owner, trigger, done-when, and which are
  launch blockers); W8 edge attestation observe-only; W9 CI gate restoration (prerequisite, not the
  feature); W10 doc-truth cleanup.
- **§5 Data model and retention.** Exact tables (`device_records`, `device_sessions`,
  `device_evidence`, `device_bans`, `appeals`, `legal_holds`, shadow tables), columns, indexes
  (partial UNIQUE `device_bans WHERE revoked_at IS NULL`), FK actions (no FK from `device_sessions`
  to users/user_sessions), `expires_at NOT NULL` + CHECK on every device table, the per-table ladder
  (keyed record 30d, ban 12mo, evidence/aggregates per schedule), the field-level strip on expiry,
  the retention job's module + registration point (own spawn after `start_scheduler`,
  `main.rs:761`; 6h interval + jitter; ≤500-row batches; **no `DELETE … LIMIT`** on this SQLite
  build; ban guard on both record and session deletes), evidence = decision metadata only.
- **§6 Enforcement.** Rules as data with `rule_version`; thresholds 80/40 + hysteresis + randomized
  enforcement delay, never exposed to clients; shadow mode mechanism (AtomicU64 counters + bounded
  ring, off-path 60s flush, separate from the 10s durability flush); promotion gates G1–G7 with
  numbers; abort criteria; runtime pause/resume; L4 = queue-only with no user-visible restriction;
  L5/L6 human-approved or auto-reversing; single-operator compensating control named honestly.
- **§7 Rollout.** Flag table (name, default, prod-gate behavior, boot position, flippable-without-
  redeploy?); `DeviceConfig` own struct — never new fields on `HttpSettings`/`RateLimitSettings`
  (eight exhaustive struct literals); WAL-safe backup procedure + restore rehearsal; migration
  idempotency (SQLite migrations are **not** transactional in sea-orm 1.1.20; version row written
  after `up()`); deploy steps + verification commands; per-step abort criteria; per-step rollback;
  irreversible steps (pepper, evidence export, `down` after writers); one deploy trigger; staging =
  production shadow window.
- **§8 Tests.** Real file names, co-located (see §3 checklist below). Include existing tests that
  must change and the machine guards the feature adds.
- **§9 Manual verification matrix.** Table: scenario | setup | expected | evidence kept. Fresh
  profile per row where relevant.
- **§10 Docs to amend.** See §4 of this review for the list; each entry gets its exact reason.
- **§11 Open questions with defaults.** `next-features` §11 shape — numbered, each with a stated
  default so implementation is never blocked.
- **§12 Non-goals.** See §5 of this review.
- **§13 Sequencing.** PR-0 (gates) → schema → session compat → collector/writer → shadow → 14/30-day
  gate → challenge → bans + appeals. Each phase = own commit(s) on the current branch, gates green
  (`next-features` §12 precedent).
- **§14 Gate commands.** Exact commands + enforcement status (see §3 below).
- **§15 Evidence appendix.** Compact decision ledger (see §8 below).

---

## 3. Convention checklist (every item must be satisfied)

1. **Status line format** — `Status: **draft, not implemented.**` + relations/authority line + pinned
   baseline commit/date (locale-suggest:3; next-features:13).
2. **Current state citations** — every "current state" claim carries `file:line`, measured at a named
   commit/date. No unsourced behaviour claims.
3. **Per-workstream Acceptance** — testable sentences, not "works well" (next-features precedent).
4. **Per-workstream Touch** — concrete file/dir list; new files named.
5. **Tests co-located with real names.** Web: `web/src/lib/device/__tests__/collector.test.ts`,
   `token.test.ts`; `web/src/lib/auth/__tests__/auth-client-device.test.ts`; route-level guard
   `web/src/routes/(application)/__tests__/device-guard.test.ts` (G1/G2/G5/G6 style source scans,
   matching `catalogue-sha-guard`/`nav-guard` house style). Rust: inline tests in the migration file;
   `rust/backend/api/tests/device_identity_regressions.rs`; `device_http_regressions.rs`;
   `edge_headers.rs` (only if the verifier lands); `rux-request-gate` layer/ip tests;
   `ruxlog-types/src/error.rs` snapshot update. Existing tests that must change: `auth-client.test.ts`
   call counts, `server-headers.test.ts` public-var list, `ruxlog-types` count 55 → 56.
6. **Manual verification matrix** — a real table (locale-suggest:151 precedent), not a sentence.
7. **Open questions for the owner with defaults** — every question states its default.
8. **Non-goals** — explicit; must include what `DESIGN-v1` itself deferred (clustering, web admin UI)
   and what B10 says is unimplementable here.
9. **Docs-to-amend list** — per file, with reason (locale-suggest §7 precedent).
10. **Gate commands + enforcement truth.** Web: `bun run check`, `bun run lint`, `bun run test`
    (from `web/`). Rust: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked
-- -D warnings`; `cargo test --workspace --locked -- --test-threads=1`; seed-system
    `cargo check`. Audits: `bun audit`; `python3 -m unittest discover -s scripts/security -p
'test_*.py'`; `python3 scripts/security/cargo-audit-gate.py`. Quran: `bun run quran:check`.
    **These are NOT enforced today** — cite B7: `53d3d421` deleted the web/Rust/audit gates from
    `.github/workflows/images.yml`; the pre-commit hook (`vp check`) reads only root config, so
    `no-nested-ternary`, `no-shadow`, `no-floating-promises`, `anti-slop/*`, `svelte-check` and all
    tests are unenforced (B7 §5). Every "machine-guarded" claim in `AGENTS.MD` is currently a human
    promise. The plan's W9 restores gates as separate `gates` + `rust-gates` jobs added to
    `publish.needs` (B7 §8).
11. **Hard-rule compliance statement** — Quran immutability, no SHA-256 over Quran data (with the
    sha-guard file-list carve-out), `db/` gitignored, translated routes SSR + 7-day cache never SSG,
    no nested ternaries, es-toolkit named imports, no standalone `.oxlintrc.json`.
12. **Migration numbering** — say "next free migration number" (or explicitly reconcile with
    `next-features.md`: `m000012` = bookmarks, `m000013` = reading sessions). **Never hard-code
    `m000012` for device work** — the reports all do and it collides (see §7, flag 1).
13. **Commit convention** — "Always commit to the current branch"; each phase its own commit(s).
14. **No broken links.** Report IDs cited as plain text (A1 §x, B6 §y), not as relative links —
    the raw reports live outside the repo. Any in-repo links must resolve (docs-divergence-guard
    checks `docs/quran-system.md` + `AGENTS.MD` links; README/history convention expects all).

---

## 4. Docs that must be AMENDED in the same change (exact reason)

| Doc                                       | Why                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `docs/quran-system.md`                    | **Part 4 (Rust ingress, rate limiting, bans, prewarm)** is the ingress/ban contract; device bans, per-credential limits and edge attestation change it. **Part 1 (Caching / Web delivery)** only if reader-path behaviour changes — it must not; state that and keep the disk-cache invariant. **Part 6 (Known gaps)** for the `IP_SOURCE` doc mismatch if touched. Machine-guarded by `docs-divergence-guard` — edits must not introduce banned SSG/artifact-safety claims and links must resolve. |
| `docs/security.md`                        | The live security record. Add the device/abuse work (or a pointer) under a new section; move items out of "Remaining" as they land. Its CI-gate sentence is accurate; keep it accurate. Do not cite the staged-deleted `security-hardening-2026-10-11.md` as current truth (its publication-gate claims are false at HEAD — B7 §3).                                                                                                                                                                 |
| `docs/history.md`                         | Line 38 claims "Resolved since: CI now runs the web/Rust suites" — **false** (B7 §3). Fix in the same change as the W9 gate restore, or the plan must not repeat the claim.                                                                                                                                                                                                                                                                                                                         |
| `docs/README.md`                          | Index the plan under "Plans and backlog" (post-restructure table).                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `docs/next-features.md`                   | Boundary, not edit-by-default: EC-19 (mid-session ban copy), §9.5 (logout/expiry/quarantine), R2-08 (adopted, do not re-implement), P10 (account/data deletion — device DSAR plugs into it), P09 (reminders — unrelated). If the plan supersedes anything, add a §14-style superseded entry.                                                                                                                                                                                                        |
| `docs/remaining/copy-corrections.md`      | C02 ("Privacy sync and deletion placeholders") is completed/superseded by the policy rewrite + P10 flow; update the ledger entry.                                                                                                                                                                                                                                                                                                                                                                   |
| `docs/remaining/feature-gap-catalogue.md` | P10 evidence list gains the device-data deletion path; add the shipped abuse-prevention facts to the parity register if relevant.                                                                                                                                                                                                                                                                                                                                                                   |
| Privacy pages (in-repo)                   | `web/src/routes/(marketing)/privacy/+page.svelte` → `web/src/lib/i18n/privacy-copy.ts` → `web/src/lib/i18n/m/privacy.ts` → `web/messages/en.json`, `web/messages/ar.json` (remove the "placeholder text, not legal advice" marker in `legal_placeholder_note`/`legal_updated_date` when the real policy lands). Terms too: `terms-copy.ts` + terms messages — DSA Art 14 needs abuse bans named as a contractual ground. Arabic wording needs fluent review.                                        |
| `web/README.md`                           | New public env var (`PUBLIC_DEVICE_COLLECTION`) must be declared in `web/src/env.ts` **and** `web/server.ts` `DYNAMIC_PUBLIC_ENV_VARS` (pinned by `server-headers.test.ts`); update the check/lint/test statements if W9 changes enforcement.                                                                                                                                                                                                                                                       |
| `deploy/README.md`                        | Owns: origin-ingress restriction to Cloudflare ranges, the `IP_SOURCE` doc, the `/admin/bans/export` feed contract (device bans must NOT enter it), the new Dokploy env vars, the backup/restore runbook (none exists today), and the one-deploy-trigger decision (`dokploy-auto-update.sh` vs `images.yml`).                                                                                                                                                                                       |
| `AGENTS.MD`                               | Only if new hard rules are added — e.g. "device signals never on reader routes; fingerprinting APIs only in the device adapter; no device × reader-data join". The hard rules list is link-guarded (docs-divergence-guard).                                                                                                                                                                                                                                                                         |
| `rust/backend/api/AGENTS.md`              | Its "guards fail the build" sentence is false as a machine claim (B7 §3); fix with W9.                                                                                                                                                                                                                                                                                                                                                                                                              |
| `rust/backend/api/migration/README.md`    | Load-on-touch doc; add the device migration's idempotency/`IF NOT EXISTS`/up-twice rules and the numbering reconciliation.                                                                                                                                                                                                                                                                                                                                                                          |
| `.github/workflows/images.yml`            | Not prose but part of the change: restore gate blocks, fix stale comments, add gates to `publish.needs` (B7 §6.1, §8).                                                                                                                                                                                                                                                                                                                                                                              |
| `DEPLOYMENT.md`                           | Lines 69/74–76 are false/pnpm-era (B7 §3); W9 corrects them.                                                                                                                                                                                                                                                                                                                                                                                                                                        |

---

## 5. What the plan must NOT do

1. **Re-propose landed controls.** `docs/security.md` records what landed: recent-auth grants for
   trust changes, Argon2 password-admission semaphore, Turnstile fail-closed registration, passkey
   fixes, push token ownership (`m000011`), R2-08 account isolation, 14-day absolute session age,
   R2-07 redirect validation, config fail-closed. The plan builds on them. Specifically:
   **"step-up = existing fresh-auth grants" is dead** (grants cannot challenge); do not resurrect it.
2. **Duplicate `next-features.md`.** EC-19 (banned/revoked account mid-session — copy/UX), §9.5
   (logout/expiry/quarantine policy), R2-08 (committed `fbf5edcd`/`d1c47299`; adopt as regression
   tests, never re-design), P10 (self-service account/data deletion — device DSAR extends it),
   P09 (goal reminders — unrelated). Reference, do not re-specify.
3. **Edit `docs/my-plan-raw.md`.** Read-only owner intent (`docs/README.md:36-37`).
4. **Touch the Quran DBs.** Immutable; `db/` gitignored; no schema/versioning; no hashing; device
   crypto stays out of sha-guarded files.
5. **Hard-code `m000012` for device work** (collides with next-features' reservation).
6. **Claim CI enforcement that does not exist** (B7). State "not enforced today" until W9 lands.
7. **Promise what the stack cannot do:** server-side TLS/JA4 collection (Traefik has no TLS hook; CF
   JA4 is Enterprise+BM; no Worker in repo), KMS-backed field encryption (no KMS; only env-held
   `FIELD_ENC_KEY`), two-person permanent-ban review (one operator — use the single-operator
   compensating control), `VACUUM INTO` file swap on a live single-container volume, precision/overturn
   KPIs at N≈0, appeals SLA numbers nobody is scheduled to keep.
8. **Store raw signals** ("evidence JSON" backdoor) or reuse `audit_logs` as the evidence store.
9. **Put a device cookie/header on reader routes** — `requestHasCookie` flips `cacheable` false and
   the 7-day SSR disk cache silently stops caching.
10. **Call it "biometric"** (wrong) or imply "not profiling" (trust scoring is profiling).
11. **Add fields to `HttpSettings`/`RateLimitSettings`** (breaks eight exhaustive struct literals).
12. **Use `DELETE … LIMIT`** (syntax error on this SQLite build) or ride the 10s durability flush.

---

## 6. Reviewer's checklist (answerable from the doc alone)

1. Does the status line say draft/not implemented, name the baseline commit + date, and state which
   doc wins on conflict?
2. Does it name every table (device records/sessions/evidence/bans, appeals, legal holds, shadow
   tables) and say exactly which table stores the ban, with `expires_at NOT NULL`?
3. Does it name the retention job (module + spawn point after `start_scheduler`), its interval,
   jitter, batch size, ban guard, and the no-`DELETE…LIMIT` rule?
4. Does it say where a device ban is enforced (own namespace / boot-hydrated in-process map, **not**
   `BanUnit`, **not** the export feed) and where the un-ban path lives?
5. Does it say "next free migration number" (or explicitly reconcile with next-features'
   m000012/m000013) and require idempotent DDL + an up-twice test?
6. Does it state the session-payload compat rule (`#[serde(default)]`, append-only, legacy decode
   test) and order compat before the writer?
7. Does it say exactly where the device payload attaches (`AuthClient.#request`), what is exempt,
   and that nothing reaches `/t/**`, reader routes, or internal SSR — with the guard test named?
8. Does it state the reader-cache invariant (cookie → `requestHasCookie` → `cacheable` false →
   silent disk-cache loss) and the test that pins it?
9. Does it give the retention ladder as numbers per table, plus the field-level strip on expiry?
10. Does it say evidence = decision metadata only (rule version, score, reason codes, reviewer,
    rationale) and forbid raw signals, UA/IP, and reader joins?
11. Does it name the shadow mechanism (atomics + bounded ring, off-path 60s flush, not the 10s
    flush), the shadow tables, and the status endpoint?
12. Does it give the promotion gate numbers (14/30 days, ≥50k evals, ≥200 accounts, ≤5%/≤1% FP,
    κ≥0.6, ≤15% unknown, ≤+1 ms p99, zero request-path writes) and the abort criteria (>20% on one
    cohort; two consecutive G3/G5 failures)?
13. Does it state L5/L6 human approval (or auto-reversal ≤72h), the pause/resume endpoint, and the
    single-operator compensating control instead of an unstaffable two-person promise?
14. Does it name the appeal flow end-to-end (table, public POST with CSRF, unauthenticated page
    outside the `ready` gate, mail template, `NotificationKind::Enforcement`) and the DSA Art 17
    content list?
15. Does it list legal artifacts with owners and done-when, marking launch blockers vs follow-ups?
16. Does it handle India (no processing or consent-gated with a Rule 3 notice) and minors
    (self-declared DOB → collector off + records deleted) explicitly?
17. Does it resolve the ePrivacy decision for the client collector (drop high-entropy probes vs
    just-in-time consent with equal-weight decline) and name the per-MS analysis artifact?
18. Does it give the flag table (name, default, prod-gate behaviour, boot position,
    flippable-without-redeploy?) and state config is its own struct?
19. Does it give rollout with WAL-safe backup, deploy verification commands, per-step abort criteria,
    per-step rollback, and the irreversible steps (pepper, evidence, `down` after writers)?
20. Does it name the test files (real paths) and the existing tests that must change (ruxlog-types
    count, auth-client call counts, env-var test) plus the guards it adds?
21. Does it include a manual matrix with concrete rows (real signal capture, Turnstile, forged edge
    header, ban→appeal→unban, 31-day retention, proxy header limits, ITP eviction)?
22. Does it list docs to amend with per-file reasons (quran-system Part 4, security.md, history.md:38,
    README index, privacy/terms copy + messages, web/README, deploy/README, AGENTS.MD, migration
    README)?
23. Does it state the gate commands and that they are **not currently enforced** (B7), with W9
    restoring them as separate jobs in `publish.needs`?
24. Does it state what is deliberately not built and why (non-goals), including deferred device
    enforcement, edge enforcement, and clustering?
25. Does it include a falsification/kill criterion (measured abuse base rate; stop rule) and the
    honest cost/benefit (B10 §7 steelman), so the feature can be cancelled on evidence?

---

## 7. Tempted claims the plan cannot support (B10/B7)

**Unsourced numbers — cite or drop:**

- "~950 req/s ceiling" (derived from an unmeasured 0.5 ms fsync estimate).
- "18.5 % mobile uniqueness" (2018 French-site study, transferred to this audience).
- "45.52 % churn ⇒ our key churns" (coarse key ≠ the study's attribute set).
- "measured 3.78 ms / 50–400 ms" presented as one measured pair (only the first is measured).
- Cloudflare Workers pricing (no URL in the report).

**Absolute claims that are false as written:**

- "0 DB ops on the request path" — login writes; a refresh may read.
- "no fingerprint for logged-out readers" — the CSRF bootstrap is an anonymous `AuthClient.#request`;
  needs a path allowlist or the claim dies.
- "raw signals never stored" — contradicted by `evidence JSON` and `audit_logs` reuse unless the plan
  makes evidence decision-metadata-only.
- "purpose limitation enforced by schema boundary" — a source-scan guard cannot stop a runtime ORM
  join; say what the guard actually enforces.
- "origin strips client-supplied `X-Eq-Edge-*`" — nothing strips today; that is build work (W8).
- "machine-guarded by `bun run lint`/tests" — unenforced until W9 (B7 §5).
- "two-person for permanent bans" — one operator; use the compensating control.

**Unimplementable recommendations — do not promise:** server-side JA4/TLS collection; KMS-backed
field encryption with role-scoped decryption; `VACUUM INTO` + live file swap; precision/overturn
KPIs as designed; the 17-artefact compliance set as "doable" without an owner per artifact.

**Scope overclaims:** device bans do not cover anonymous scraping (readers never send a payload);
the device key is a cohort key, not a device key; trust scoring is profiling; self-declared DOB is
not age verification.

---

## 8. Length and structure trade-off — recommendation

**One plan doc, self-contained, with a compact decision ledger as §15; do NOT ship the 30+ raw
reports as repo docs (NEW recommendation).**

- The repo is actively compacting docs (working-tree restructure: 76 deletions; long-form in git
  history). Adding 30 report files fights that direction and re-creates rot surface.
- The plan must stand alone when executed months later: every load-bearing decision needs its
  outcome + reason in the plan, not in a report the implementer may never read.
- §15 ledger shape: one row per decision — `decision | source (report ID, §) | status
(adopted/dropped/superseded) | plan section`. Plain-text report IDs, no relative links (files are
  outside the repo; broken links violate repo link conventions).
- If the owner wants the evidence in-repo: one digest only, `docs/research/abuse-prevention/README.md`
  (precedent: `docs/research/ux-audit/README.md`), not 30 files.
- Size budget: ~1,000–1,500 lines is normal here (`next-features.md` ≈ 1,350). Prefer one long doc
  over a doc tree. Legal artifacts (DPIA, LIA, SOPs) are deliverables with owners, not plan sections —
  list them; don't write them inside the plan.

**Flags for the coordinator (NEW):**

1. **Migration-number collision:** every device report says `m000012`; `next-features.md:42,649-655`
   reserves `m000012` (bookmarks) and `m000013` (reading sessions). The plan must use "next free
   number" or reconcile explicitly. This is the single most likely concrete implementation collision.
2. **Docs restructure in flight:** author against the post-restructure tree; do not resurrect deleted
   docs; `docs/history.md:38` is false until W9.
3. **CI truth:** the plan's own gates are not machine-enforced; W9 (gate restore) is a prerequisite
   (B6 §10), not optional, and belongs in the same change as the doc-truth fixes.
