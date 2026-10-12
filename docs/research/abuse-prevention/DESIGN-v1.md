# Design sketch v1 — converged after round-3 audits

Changes from v0 are driven by 10 adversarial audits (A1..A10, all in this dir). Where an audit found a blocker, the blocker's fix is folded in. Open questions are listed at the end and are the round-4 targets.

## A. What changed and why (v0 -> v1)

| v0 element                                         | Verdict                                                                                    | v1                                                                                                                    |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------- |
| `device_key = HMAC(components)` as the ban key     | cohort key, not a device key; 18.5% mobile uniqueness; cohort bans                         | **Dropped as an enforcement key.** Device is an _evidence_ axis only                                                  |
| `BanUnit::Device(device_key)`                      | breaks canonical-unit invariant, export contract, admin filter, escalation gating          | **Dropped.** Separate `device_bans` table + own admin API. Export stays IP-only                                       |
| `UNIQUE(device_key)`                               | cohort key + client-chosen input = attacker-chosen ban unit                                | dropped; `device_key` non-unique, indexed for cohort lookups only                                                     |
| Client-authored coherence + tamper booleans scored | attacker's own `{webdriver:false}` = one-line bypass                                       | server recomputes every predicate from a closed typed payload                                                         |
| Per-request device evaluation                      | violates the repo's "no DB write on the hot path" rule; pool = 1; ~950 req/s ceiling total | **0 DB ops on the request path.** Write on login/link; read from session payload with its own TTL; 60s ticker         |
| WASM/JS timing signature                           | measured 3.78ms compile, 50-400ms low-end; 2-4 bits; unstable -> mints false "new device"  | **Dropped**                                                                                                           |
| "step-up = existing fresh-auth grants"             | those grants cannot challenge; process-global cap 10k = DoS; no challenge at login         | real session-bound challenge (WebAuthn UV / TOTP), per-user limits, prompt cap 1/5min                                 |
| Device token in localStorage                       | XSS-readable (CSP allows gstatic/gtm/turnstile scripts)                                    | **HttpOnly cookie, Path=/api** (OPEN Q1)                                                                              |
| Pepper derived from COOKIE_KEY                     | one leak = session forgery + device forgery                                                | separate `DEVICE_KEY_PEPPER`, HKDF subkeys per purpose, `Zeroizing`                                                   |
| Edge headers signed with shared HMAC               | origin can forge edge claims; canonicalization unspecified                                 | **Ed25519**, private key only in Worker, `kid` list at origin                                                         |
| Edge enforcement default                           | fail-closed on absence = site down                                                         | observe-only default, `EDGE_ATTEST_REQUIRED` opt-in per endpoint                                                      |
| L4 hold <=72h, L6 ban + second look                | no timer exists; "second look" = theatre                                                   | L4 queue-only with hard expiry; L5 human-approved or auto-reversing; L6 human before effect, two-person for permanent |
| Evidence in `device_records.evidence`              | dies at 30d with the key; not case-scoped                                                  | append-only `device_evidence` + `enforcement_cases`, self-contained, no FKs                                           |
| `audit_logs` reuse for evidence                    | mutable, no case id, no index, `user_id` SET NULL                                          | dedicated tables; `audit_logs` = operator-action audit + evidence-read log                                            |
| Retention ladder asserted                          | `device_records` had no `expires_at` -> unenforceable                                      | `expires_at NOT NULL` + CHECK + batched sweeper (no `DELETE ... LIMIT` on this build)                                 |

## B. v1 architecture (normative)

**B1. Primary defense = account + credential, not the device.**

- Per-credential and per-API-key rate limits (A8 ranked this #1 impact/effort and it is missing today).
- Account-scoped bans live in the existing `user_bans` table, which already has create/revoke/`check_ban` enforcement and zero callers. Wire it. Add `/admin/users/bans`.
- Device signals may challenge; they may never be the sole cause of a ban.

**B2. Client collector** (`web/src/lib/device/`)

- Injected seam: `DeviceSignals` interface + `DeviceSignalsProvider`. The browser adapter is the only file that touches browser APIs. Emits a closed typed payload (enums/small ints, no free strings, no raw UA/screen/plugins).
- Signals kept: hardware class (WebGL/WebGPU family + extension set + shader precision), platform class, JS feature matrix, UA-CH high-entropy values when present, language quintuple + Intl resolver, coarse screen bucket, context (browser/webview/standalone), and tamper flags **as inputs the server re-derives**.
- Runs in a dedicated lazy Worker after first paint; budget <=30ms total, <=5ms chunks. Never gates `ready`/first paint. Missing payload = "unknown device", never "new device".
- Never on reader routes: no payload on `/t/**`, `/{surah}/t/**`, quran content API, or internal SSR. Machine-guarded.

**B3. Server**

- `device_records` (id ULID, token_hash UNIQUE, device_key non-unique + kid, user_id NULL SET NULL, first_seen, last_seen, seen_count, trust_state TEXT enum, `expires_at NOT NULL`, tri-state coherence columns as nullable INTEGER, evidence JSON tail).
- `device_sessions` (no FKs to users/user_sessions; denormalized user_id) so account deletion cannot destroy ban-evasion linkage.
- `device_evidence` + `enforcement_cases` (append-only, case-scoped, self-contained, indexed `(device_key, created_at)`).
- `device_bans` (own table; boot-hydrated in-process map; own admin API; never in the IP ban namespace or the export feed).
- `device_bans` / `device_records` retention: keyed record 30d, ban row 12mo, aggregates forever, `expires_at` enforced by a batched sweeper registered in `scheduler.rs`.

**B4. Crypto**

- `DEVICE_KEY_PEPPER` (32B, prod boot panic + placeholder blocklist) -> HKDF subkeys `ruxlog-device-token-v1` / `ruxlog-device-fp-v1`, `Zeroizing` `OnceLock`s.
- Token: random-only, server-minted, >=128-bit, stored HMAC'd, answers "which record" only, never an auth factor. `revoked_at` + revoke UI + per-user cap.
- Edge: Ed25519 over a fixed closed ordered header list, REQUIRED headers, `get_all` exactly-one, binds method/scheme/host/path/query/body-digest/cf-ray/cf-connecting-ip, +/-120s + `cf-ray` single-use dedupe, origin strips client `X-Eq-Edge-*` outermost.
- No weekly epoch rotation + mapping table (adds cost, no unlinkability gain). Stable record id + versioned dual-key-rotatable `fp_digest`.

**B5. Enforcement ladder (rewritten)**

- L0 observe -> L1 throttle -> L2 challenge -> L3 cooldown -> L4 queue-only hold with hard expiry (no restriction) -> L5 interim restriction, human-approved or auto-reversing <=72h -> L6 ban, human before effect, two-person for permanent -> L7 legal referral.
- Deterministic rules only where non-forgeable (CSAM hash, prior ban + credential reuse, court order). Everything else scored, versioned, shadow-mode first.
- Two thresholds with hysteresis (ban >=80, restore <=40), never exposed to the client, with a small randomized enforcement delay.

**B6. Ops**

- No web admin UI. JSON admin API + bun TUI/CLI (matches the existing API-only admin convention). Roles: read/propose = Moderator (currently dead role), suspend/lift/finalize = Admin, policy = SuperAdmin.
- Appeals: `appeals` table + public `POST /appeal/v1/submit` (CSRF via `/csrf/v1/generate`) + `/appeal/<token>` page rendering unauthenticated outside the `ready` gate + one transactional mail template + `NotificationKind::Enforcement`.
- Metrics: precision, appeal overturn, queue aging, time-to-detect. Never "bans issued".
- No entity-graph clustering in v1 (pool = 1, shared phones are the core audience). On-demand 2-hop linked-accounts panel capped at 20 accounts.

**B7. Compliance**

- Purpose limitation enforced by schema boundary + a machine guard that fails the build on any device x reader-data join.
- `expires_at` on every device row; field-level expiry; evidence = decision metadata only.
- Minors: self-declared DOB -> minor flag -> collector off + records deleted; no fingerprint for logged-out readers.
- Artifacts to produce: DPIA, LIA, ROPA entry, retention schedule, privacy-policy rewrite + device paragraph, Art 17 statement-of-reasons template, appeal SOP, LE preservation SOP + `legal_holds` table.
- DSA: service already hosts posts/comments -> Art 17 applies now (Section 2, not exempt).

## C. OPEN QUESTIONS (round-4 targets)

- Q1: token in HttpOnly `Path=/api` cookie vs localStorage+IDB. Cache-safety vs XSS-safety. Which wins, and what exactly must the guard test assert?
- Q2: Is fingerprinting the right tool at all? Compare (a) credential/account-centric only, (b) light device (token only), (c) full v1. Cost/benefit, and what each actually stops.
- Q3: Anonymous/scraping abuse: readers never send a payload. What defends the reader path without fingerprinting readers?
- Q4: Concrete v1 rule set with thresholds + precision estimates + how each rule is evaded.
- Q5: Shared-device positive design: family, madrasa, kiosk/org registration, shared-device claim.
- Q6: Rollout sequencing, feature flags, shadow mode, rollback, migration safety on a live DB.
- Q7: What else in the repo has silently rotted (CI gates were deliberately removed in HEAD — 53d3d421)? Which invariants are now unguarded?
- Q8: Retention/deletion execution: can we actually delete, propagate to backups, delete the pepper, and issue a receipt?
