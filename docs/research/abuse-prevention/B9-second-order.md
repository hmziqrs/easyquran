# B9 — Second-order adversarial audit of DESIGN-v1

Auditor: B9 (second-order pass). Date: 2026-10-12.
Target: `DESIGN-v1.md` (converged after round-3 A1..A10).
Read in full: DESIGN-v0/v1, A1, A2, A3, A9, A10. Read for scope: A5, A8. Repo checked at HEAD
(`/Users/hmziq/os/easyquran`).
Method: assume every v1 fix is implemented **exactly as written**, then ask (a) what the fix broke,
(b) what it left undefined, (c) whether the resulting system does anything at all. A finding is
reported only if it is absent from the round-3 reports and from the excluded list (cohort key,
`BanUnit::Device`, client-authored coherence booleans, per-request DB writes, WASM timing,
fresh-auth-as-step-up, localStorage token, `COOKIE_KEY`-derived pepper, shared-HMAC edge,
fail-closed edge, L4 no timer, evidence in `device_records`, unenforceable retention / missing
`expires_at`, minors, India DPDP, Art 22 automation, DSAR path, appeal channel absent,
entity-graph no-go, per-credential limits, `device` name collision, `audit_logs`, non-transactional
migrations, lossy downgrade, `DELETE ... LIMIT`, metrics cardinality).

---

## 0. Bottom line

v1 is a careful document, and the fixes are individually correct. But the fixes were assembled
without re-asking the first question, and the result is a system whose **enforcement output is
empty**, whose **queue is the attacker's cheapest lever**, whose **new tables have no lifecycle**,
and whose **compliance programme is aimed at the decorative layer while the shipped legal debt
stays open**. Ten of the fourteen findings below are about v1's _own new mechanisms_ — the boot
map, the case/evidence pair, the appeal channel, the panel, the randomization, the worker, the
retention ladder vs the ban's 12-month purpose. The other four are about what the fixes left
unresolved (payload optionality, edge with no consumer, solo-operator reality, compliance scope).

---

## 1. Answers to the ten questions

### Q1 — "Device = evidence axis only": what is the device actually FOR?

Trace every claimed device-enabled action in v1:

| v1 claim                          | Actual mechanism after v1's fixes                                                                                                                                   | Effective?                                                                                                                             |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| B1 "device signals may challenge" | L2 challenge fires only for a request that carries a payload, matches a bad `trust_state`/`device_bans` row, and keeps its token                                    | Evaded by clearing storage or by never sending the payload (B2: missing = unknown, never new)                                          |
| B3 `device_bans`                  | Table + boot map + admin API specified; **no scope→action mapping, no code path from a matching row to any user-visible effect, no interaction with L0–L7 defined** | Either dead code or an unspecified enforcement path                                                                                    |
| B5 L4–L6                          | Account-scoped by construction ("never the sole cause of a ban"); device evidence is one case-file input                                                            | Device-derived edges are inadmissible as corroboration (A2 inheritance lemma: cohort key → strangers; token → shared-browser co-users) |
| B6 panel                          | 2-hop traversal                                                                                                                                                     | A graph query (see Q9 / finding 11)                                                                                                    |
| B7 guard                          | Purpose limitation                                                                                                                                                  | No CI to fail (see finding 11)                                                                                                         |

So the device layer's residual products are: (1) friction for clients that volunteer a payload and
do not clear storage; (2) a `device_bans` table with no defined effect; (3) a suggestive but
inadmissible linkage hint for a human reviewer. Against that: four tables, a new secret, a
Cloudflare Worker, a sweeper, an admin API + TUI, an appeals channel, a DPIA/LIA/ROPA/retention
regime, i18n, a machine guard, and a false-positive burden concentrated on shared-phone users
(A2's base rates: India 30% of adults do not own a phone; 56% of non-owners borrow a family
phone).

**Verdict: the device layer is decorative as enforcement.** Its only honest remaining purpose is
"evidence for human review", and v1 provides no review workflow that consumes it (no
prioritisation rule, no case-scoring rule, no SOP saying how device evidence is weighed) — while
the one analysis that did look at that use (A2) rules it inadmissible. The design's own Q2 ("is
fingerprinting the right tool at all?") is still open, yet B2–B7 are already normative against it.

What would make it load-bearing: (i) a **session-binding / ATO** control (non-exportable key,
DBSC/WebAuthn-style) where the device actually gates something (stolen cookie alone insufficient)
— a different feature with a real effect; (ii) a **device-token-keyed limiter for authenticated
writes** with per-account mint caps (bounded cardinality, real cost increase for IP-rotating
abuse); or (iii) **server-side signals only** (no collector, no terminal access, much cheaper
legally). Pick one or cut the layer.

### Q2 — The rewritten ladder with a solo operator

v1 B5: L4 queue-only, L5 human-approved or auto-reversing ≤72h, L6 human before effect,
**two-person for permanent**. v1 B6: roles Moderator/Admin/SuperAdmin, JSON API + TUI, no web UI.

- **Two-person + solo operator = permanent bans are structurally impossible.** The single operator
  cannot satisfy "two-person". The same applies to appeal review, which by the round-3/A5 shape
  requires a reviewer different from the first. Creating a second admin account to satisfy the
  rule is one human rubber-stamping twice, and it _looks_ like independence — worse than an
  honestly documented single review.
- **Throughput, with numbers.** L4 triage (read evidence, decide keep/close) 5–10 min. L5/L6
  (evidence read, corroboration check, rationale, notice, second-look fields) 20–45 min. Appeal
  intake + review 20–40 min. A solo operator with ~1 h/day for moderation: **~4 L4 triage/day or
  ~1.5 L5/L6 actions/day (~20–25 enforcement actions/month)**.
- **Backlog.** Any case rate above ~4/day grows the queue. L4 is queue-only with a hard expiry, so
  unreviewed cases silently expire = **no effect**; the ladder above L3 therefore degrades to
  "nothing is ever enforced" for every day the operator is busy. There is no aging job, no alert,
  no bulk-close, no per-key/IP/account proposal cap in v1 (only a "queue aging" metric).
- **The queue is the DoS.** An abuse spike, or an attacker who deliberately emits flagged signals
  from cheap identities, produces 100–1000 proposals/day; at 5–45 min each that is 25–250 h/day of
  review. The attacker's cost is near zero; the defender's cost is bounded by one human. The
  attacker's best strategy is volume, not stealth.
- **Steady state:** temporary bans that expire, recidivists returning, no permanent removal. The
  ladder's top is dead code and the enforcement system is a treadmill.

### Q3 — `device_evidence` + `enforcement_cases`, append-only, no FKs

- **Append-only vs the ladder is a contradiction.** L4 expiry, L5 approve/auto-reverse, L6
  propose→finalize, two-person finalize and the appeal link are _state transitions_. v1 specifies
  no `state`, no `proposed_at`/`decided_at`/`effective_at`/`expires_at`, and no event-fold. Either
  the table is mutable (the append-only claim is false) or the TUI must fold an event log with
  nothing indexed to fold on.
- **Retention gap.** B3's retention names only `device_bans`/`device_records` (30d/12mo). The two
  new append-only tables have **no retention class at all** — the exact defect v1 fixed one table
  over, reintroduced on the tables that grow fastest.
- **No FK ⇒ orphans with no detector.** Evidence can reference a case that never existed (bug) or
  was removed; a case can exist with no ban and no notice. Self-contained duplication
  (device_key/user_id copied into evidence) means the copies can drift silently and nothing
  reconciles them.
- **Operator cannot find a case later.** The only stated index is `(device_key, created_at)`, and
  `device_key` is the **non-unique cohort digest** (v1 B3). So "all cases for this device" is
  "all cases for this hardware class"; nothing indexes the stable `device_records.id` (ULID) or
  `user_id`, so one actor's history cannot be reconstructed reliably. After user deletion the
  denormalised ids dangle with no resolver.
- **Finalize is a multi-table write with no stated transaction.** Case + `user_bans` (v1 B1) must
  move together; a crash yields an unappealable ban (no case → no notice/appeal) or a silent case
  (no effect). The repo has the `conn.transaction` precedent; v1 does not use it.

### Q4 — The appeal channel

`appeals` + public `POST /appeal/v1/submit` + `/appeal/<token>` outside the `ready` gate + one mail
template + `NotificationKind::Enforcement`.

- **Enumeration / oracle.** v1 sets no uniform-response rule. If submit distinguishes "no such
  case" from "submitted", the endpoint is a banned-account oracle. If it accepts an email/username,
  it is an account-existence oracle. Neither is addressed.
- **Spam / DoS.** Public unauthenticated POST with no stated rate limit, cap, or challenge, and
  each submission sends mail — the operator's mailbox is the target. The CSRF token from
  `/csrf/v1/generate` stops cross-site posts, not bots.
- **Token guessing / leakage.** v1 never says how the token is generated, what it resolves to, or
  whether it expires or can be revoked. A URL bearer token leaks via Referer (Turnstile on the page
  would leak it to `challenges.cloudflare.com` unless `no-referrer`), browser history, shared
  devices, and screenshots. The page must be `X-Robots-Tag: noindex` + `Referrer-Policy:
no-referrer` and must load **no** third-party scripts — analytics on an enforcement page exports
  "this person was banned" to Google. None of this is in v1.
- **Laundering.** v1 does not say that an appeal is inert until a decision, nor that a reversal
  needs new evidence or a recorded rationale. (`restore <= 40` hysteresis is unchanged, so the
  round-3 A1 F9 concern stands; not re-reported here.)
- **Notice durability — a concrete repo failure.** `services/mail/router.rs:241-255` drops any
  send to a suppressed recipient and returns `Ok` by design ("Anti-enumeration… suppressed
  transactional send dropped"); `is_transactional` (`services/mail/provider.rs`) covers only
  verification + password_reset, so a new enforcement template is non-transactional unless added.
  `NotificationKind::Enforcement` does not exist (`ruxlog-types/src/enums/notification.rs`:
  NewComment, NewFollower, PaymentReceived, PostPublished, Mention, System), and the in-app channel
  is unreachable for a banned session. **The only durable notice channel can silently drop the
  notice while the ban stands, with no delivery proof recorded.** No email (OAuth/passkey-only
  accounts, device-only bans) = no notice at all.
- **Self-containment.** The notice must survive suspension (v1 got this right: outside `ready`),
  but also user deletion and the retention sweeper. The token must resolve to the _case_, not to
  the user row; v1 does not say so. A late appeal after the case's unstated retention 404s.

### Q5 — Ed25519 edge attestation with the key in a Worker

- **No consumer.** A9 §4.2 established that the origin already knows the request transited
  Cloudflare (mandatory `CF-Connecting-IP`, ingress locked to CF ranges) and that the signature's
  only new value is attesting `request.cf` fields the origin cannot verify: ASN, bot score, JA4.
  On the target plan (non-Enterprise) JA4/botScore are absent; v1's ladder does not use them; so
  v1 signs a header set (method/host/path/query/body-digest/cf-ray/CF-Connecting-IP) that is
  already present and already trusted by network position. The attested claim set is empty or
  ASN-only. A9 recommended **deferring** edge attestation; v1 promoted it to normative B4 while
  there is still no Worker, no wrangler config, no tests, no repo code.
- **New trust root and custody.** The private key lives in a Worker secret — readable by anyone
  with Cloudflare account or `wrangler` access. The origin therefore trusts the CF _account_, not a
  hardware boundary; there is no HSM, no ceremony, no revocation, no absence-rate alarm specified.
  Worker compromise = attestation forgery for any request, including spoofed `cf` fields.
- **Rotation across deploys / kid staleness.** Rotation needs a coordinated origin public-key list
  update. Worker-first rotation drops all attestations (observe mode: silent; required mode: 503
  for the named endpoints). Origin-first is safe but v1 does not order it. The `kid` list at the
  origin is boot config with no stated refresh.
- **"Only Cloudflare sees the eyeball connection"** means the attestation is Cloudflare attesting
  Cloudflare's own view. That is not useless, but it delivers nothing the design needs today.

### Q6 — The typed payload: what an attacker can forge or withhold

- **Withhold:** the payload is optional by construction. B2: missing = "unknown device, never new
  device"; the collector never gates `ready`, never runs on reader routes. Not sending it is free.
- **Forge:** the fix (server recomputes every predicate from a closed typed payload) validates
  **internal consistency, not authenticity**. The payload is 100% client-authored and its schema is
  public (open source), so a synthetic payload can be made fully coherent (~30 lines) and passes
  every server predicate. There is no independent ground truth for any field.
- **Therefore the mechanism is dominated.** For any informed adversary: `{send nothing}` ≤
  `{send synthetic}` < `{send truth}` in risk, and all three are available. The device axis can
  only ever observe cooperative, non-evading clients.
- **"Missing = unknown" is correct privacy policy and fatal detection policy.** It is exactly the
  unpunishable evasion path, and nothing client-side closes it. The only closures are (a) mandatory
  server-side signals (TLS/headers/IP) as the axis, (b) treating absence as _lower privilege_ on
  authenticated writes — rejected by A2's posture and hostile to privacy users, or (c) accept it
  and cut the axis.

### Q7 — `device_bans` as a separate table with a boot-hydrated in-process map

- **The correctness bug.** Per-request enforcement reads the map; `expires_at` is enforced by a
  batched sweeper (B3); L5 is "auto-reversing ≤72h" and `restore <= 40` can auto-restore (B5).
  **Nothing connects the sweeper or the reversal path to the map.** If the map check uses
  presence/scope (not `expires_at` per read), an expired or auto-reversed ban keeps enforcing until
  restart — i.e. the design's own compliance guarantee (auto-reversal) is broken by its own
  hot-path optimisation. If the map stores `expires_at`, the sweeper is redundant but the
  _reversal_ still bypasses the map. B3's "write on login/link; read from session payload with its
  own TTL; 60s ticker" adds a **third** device-state cache with no stated precedence.
- **Restart:** rehydration is fine (bans created while down are picked up).
- **Another process:** v1's single-container assumption is true today but unstated; any maintenance
  script, migration CLI, or future second instance diverges silently. The map never shrinks
  (12-month ban rows stay in memory).
- **Ban created while running:** only via the admin API in-process, so OK — provided the admin path
  is the only writer. State it.

### Q8 — Two thresholds + "small randomized enforcement delay"

- **It does not achieve the stated goal.** A delay shifts _when_ an effect lands, not _what_ crosses
  the threshold. The crossing remains observable (status codes, latency, feature availability), so
  a handful of samples still measures the boundary. If the randomization is on _whether_ rather
  than _when_, the same score sometimes bans and sometimes does not — arbitrary, indefensible, and
  worse.
- **It destroys reproducibility.** Two subjects with the same score at the same time get different
  treatment; the operator's two-person review cannot reproduce the decision; B6's precision /
  overturn / TTD metrics are contaminated by the random delay; a DSAR answer and an Art 22(3)
  contestation ("why me, why now?") cannot be reconstructed.
- **What must be recorded instead:** `rule_version`, `score`, `threshold_ban`, `threshold_restore`,
  `evaluated_at`, `effective_at`, `reason_codes`, `decision_source` (auto/human), `reviewer_id`,
  `rationale` — plus a fixed, documented grace period instead of a random one. If boundary probing
  is the threat, address it by not exposing ladder transitions (uniform responses/latency), not by
  randomizing decisions.

### Q9 — "No entity graph" vs the 2-hop panel

- **Contradiction.** B6 says "No entity-graph clustering in v1" and ships, in the same bullet, an
  on-demand 2-hop expansion capped at 20 accounts. That is a graph query with a _display_ cap; the
  underlying traversal can scan a cohort's whole edge set. The claim should read "no precomputed
  clustering".
- **Untyped edges = cohort collateral in the operator's head.** The panel does not say it
  distinguishes same-token edges (strong-ish) from `device_key`/cohort-digest co-occurrence
  (strangers on identical hardware — the inadmissible class) from credential reuse from IP. An
  operator who reads a cohort's case history as "linked accounts" will propagate enforcement across
  strangers — the exact failure the design removed from the machine, reintroduced through the
  human.
- **Access is unaudited deanonymization.** Moderator is a read role (B6) with no case scoping; an
  operator can enter any account and see its devices and co-accounts. For a Quran app that is a
  device/association map of a sensitive population. `audit_logs` is named as an evidence-read log,
  but v1 sets no read policy (case-scoped only), no bulk-read alarm, no periodic review.
- **The guard must whitelist the very join it claims to forbid.** B7's guard "fails the build on
  any device × reader-data join"; the panel is a device × account join, so the guard needs a
  blessed-join list — and the review process for that list is the new control point, unstated. The
  guard also has no automatic enforcement path today: `.github/workflows/` contains only
  `images.yml`, and the only hook is `pre-commit` → `vp staged` (staged files). "Fails the build"
  depends on someone running `bun run test` by hand.

### Q10 — Compliance scope vs engineering

The artefact list in B7 exists almost entirely because of the device layer. Cut the layer (Q1/Q2)
and most of it collapses: DPIA sections, per-MS ePrivacy analysis, device retention rows, device
Art 17 template, device appeal SOP, minors device mode, device i18n paragraph, `legal_holds` table,
device machine guard.

Meanwhile the **shipped** system has live legal debt v1 does not touch:

- `user_sessions.device` stores a raw User-Agent and `ip_address` stores the client IP, with no
  retention rule and no policy mention (A8 F6).
- The privacy policy is a placeholder explicitly marked "not legal advice".
- `user_bans` is unwired, so account-level enforcement does not exist yet (IP-only at the edge).
- `post_comment_v1` already restricts user content → DSA Art 17 statements of reasons may already
  be owed today.
- The FCM `devices` table has no expiry.

**Irreducible minimum for legality of what exists:** (1) a real privacy policy; (2) a documented
basis + LIA for account-level abuse prevention; (3) a notice + appeal route for account/content
restrictions (Art 17 / Art 22(3) shape); (4) retention rules for already-stored identifiers
(`user_sessions` UA/IP, FCM `devices`); (5) a DSAR/erasure procedure. Everything else is
feature-contingent and should be gated on a go/no-go for the device layer.

---

## 2. Findings (14, all NEW)

### 1. The device layer has no enforcement consumer; `device_bans` is undefined or a no-op

**Severity: High. NEW.**
Why v1 still fails: B1 forbids device from ever being the sole cause of a ban; B3 specifies the
`device_bans` table, boot map and admin API but **no scope→action mapping, no code path from a
matching row to a user-visible effect, and no interaction with L0–L7**. The remaining device
products are (a) an L2 challenge for clients that volunteer a payload and keep their token, (b) a
ban table with no defined effect, (c) a linkage hint that A2's inheritance lemma rules inadmissible.
Cost: four tables, a secret, a Worker, a sweeper, an admin API/TUI, a full compliance regime, and
FP burden on the core shared-phone audience. v1's own Q2 ("is fingerprinting the right tool at
all?") is still open while B2–B7 are normative.
Fix: pick the one load-bearing use or cut the layer. Candidates: (i) session binding/ATO with a
non-exportable key (DBSC/WebAuthn-style) — a different feature with a real effect; (ii)
device-token-keyed limiter for authenticated writes with per-account mint caps (bounded
cardinality); (iii) server-side signals only, no collector. Do not ship `device_bans` without a
defined effect.

### 2. The payload is optional and synthetic-coherent: the collector is a dominated strategy, so no rational adversary ever reveals

**Severity: High. NEW.**
Why v1 still fails: the F1 fix ("server recomputes every predicate from a closed typed payload")
validates internal consistency, not authenticity. The payload is still 100% client-authored, the
schema is public, and a synthetic coherent payload passes every predicate. B2 makes the payload
optional ("missing = unknown device, never new device"; never gates `ready`; never on reader
routes), so `{send nothing}` is free and `{send synthetic}` dominates `{send truth}` in risk. The
device axis can only observe cooperative clients; "missing = unknown" is correct privacy policy and
fatal detection policy.
Fix: either (a) move the axis to mandatory server-side signals (TLS/headers/IP), (b) treat absence
as lower privilege on authenticated writes (explicitly rejected by A2's posture — do not), or (c)
accept and cut the axis. If the payload stays, drop every claim that it detects evasion and label it
"cooperative-client telemetry".

### 3. Boot-hydrated `device_bans` map vs auto-reversal/expiry sweeper: expired bans keep enforcing

**Severity: High. NEW.**
Why v1 still fails: per-request enforcement reads the in-process map (B3); `expires_at` is enforced
by the batched sweeper (B3); L5 auto-reverses ≤72h and `restore <= 40` auto-restores (B5). Nothing
connects the sweeper or the reversal path to the map. If the map check is presence/scope-based, an
expired or auto-reversed ban enforces until restart — the compliance guarantee broken by the
hot-path optimisation. B3's session-payload TTL + 60s ticker is a third device-state cache with no
stated precedence. Single-process is assumed, not stated.
Fix: one writer for device-ban state; every mutation (create/revoke/auto-reverse/expiry) writes DB
then map, or make the map expiry-aware and re-read at read time; reconcile the map from the DB on
the existing 60s scheduler tick (one indexed SELECT/min); define the session field as a cache of
the map; add a boot + post-mutation invariant test (map == active rows).

### 4. Solo-operator collapse: two-person rule and independent appeal review unreachable; queue throughput ~1–4 cases/day

**Severity: High. NEW.**
Why v1 still fails: L6 permanent requires two people; appeal review requires a reviewer different
from the first. With one operator, permanent bans are impossible and appeals are self-reviewed.
Numbers: L4 triage 5–10 min; L5/L6 20–45 min; appeal 20–40 min; ~1 h/day → **~4 L4 triage/day or
~1.5 actions/day (~20–25/month)**. Any rate above ~4 cases/day grows the backlog; L4's hard expiry
silently closes unreviewed cases = no effect. A spam wave or a deliberate flood (100–1000 cheap
proposals/day) is 25–250 h/day of review; v1 has no proposal cap, no duplicate/cohort collapse, no
aging job or alert (only a metric).
Fix: state the solo reality (permanent bans unavailable → auto-expiring bans with renewal; self-review
appeals recorded as such) or fund a second reviewer; add per-key/IP/account proposal caps,
duplicate/cohort auto-collapse, tiered triage, and an aging alert. Treat the queue as a defended
surface, not a dashboard.

### 5. `enforcement_cases` "append-only" cannot express the ladder; no retention class; case lookup keyed by the cohort digest

**Severity: High. NEW.**
Why v1 still fails: L4 expiry, L5 approve/auto-reverse, L6 propose→finalize, two-person finalize and
the appeal link are state transitions; v1 specifies no state/timestamps and no fold — either the
table is mutable (the append-only claim is false) or the TUI must event-fold with nothing to fold
on. B3's retention names only `device_bans`/`device_records`; `device_evidence` and
`enforcement_cases` have **no retention class** → unbounded append-only growth. No FK: evidence can
reference a non-existent case, a removed case orphans its evidence, duplicated fields drift
silently. The only stated index is `(device_key, created_at)` and `device_key` is the non-unique
cohort digest, so "all cases for this device" is "all cases for this hardware class"; nothing
indexes the stable record id or user id. Finalize writes case + `user_bans` with no stated
transaction → an unappealable ban (no case) or a silent case (no ban).
Fix: define a case state machine (state, proposed_at, decided_at, effective_at, expires_at,
reviewer_id, second_reviewer_id, rationale, ban_ref) with an append-only `case_events` child; give
both tables retention classes and sweeper entries; index by case id (PK), user id and stable device
record id, never by the cohort digest; wrap finalize in one transaction (repo `conn.transaction`
precedent).

### 6. Legal holds and the retention sweeper are unconnected; hold subjects are unresolvable

**Severity: High. NEW.**
Why v1 still fails: B7 adds a `legal_holds` table; B3 adds a batched sweeper enforcing `expires_at`.
No predicate makes the sweeper consult holds (A10's SQL predates holds). A hold placed before the
sweep is silently ignored → spoliation; a hold placed after cannot resurrect deleted rows. And what
does a hold name? The subject-identifying tables are exactly the ones the sweeper deletes, and they
have no FK: a hold by user id dangles after `admin_delete`; a hold by `device_key` is a cohort
digest. So the hold cannot be reliably resolved at sweep time.
Fix: make the sweeper predicate hold-aware (`NOT EXISTS` on held subjects); key holds by the stable
identifiers copied at hold time (case id + device record id + user id) plus authority/owner/expiry;
apply holds in a transaction with the retention boundary; add a boot invariant that no held subject
is older than the hold.

### 7. Appeal token/notice lifecycle unspecified: bearer URL with no expiry, binding, self-containment or page hygiene

**Severity: High. NEW.**
Why v1 still fails: `/appeal/<token>` renders unauthenticated outside `ready` (correct for
suspension), but v1 never says how the token is generated, what it resolves to, whether it expires
or can be revoked, or whether it survives user deletion and the retention sweeper. All required
properties are absent: ≥128-bit server-minted entropy; binding to the _case_ (self-contained, so
Art 17 deletion or a key sweep cannot 404 the notice); expiry ≥ the appeal window with rotation on
re-decision; no session powers; no leakage via Referer, browser history, shared devices. The page
must be `X-Robots-Tag: noindex`, `Referrer-Policy: no-referrer`, and load no third-party scripts
(Turnstile on the page would leak the token in the Referer; analytics would export "this person was
banned").
Fix: specify token generation (128-bit, stored hashed, single-purpose, expiring, revocable), case
binding, page headers (noindex/no-referrer/no analytics/no challenge), and a retention rule that
keeps the case (not the user) for the appeal window.

### 8. The appeal endpoint has no abuse controls, and the durable notice can be silently dropped

**Severity: High. NEW.**
Why v1 still fails: public unauthenticated POST + mail = spam/mail-bomb; v1 states no rate limit,
no per-token/per-IP cap, no challenge, and no uniform-response rule → the endpoint becomes a
"is this account banned?" oracle. Repo evidence: `services/mail/router.rs:241-255` drops suppressed
recipients and returns `Ok` by design ("Anti-enumeration… suppressed transactional send dropped");
`is_transactional` (`services/mail/provider.rs`) covers only verification + password_reset, so a
new enforcement template is non-transactional unless added; `NotificationKind::Enforcement` does not
exist (`ruxlog-types/src/enums/notification.rs`), and the in-app channel is unreachable for a
banned session. The only durable notice channel can silently drop the notice while the ban stands,
with no delivery proof. No email (OAuth/passkey-only, device-only bans) = no notice at all.
Fix: rate-limit and cap the endpoint; uniform responses (never reveal case existence/status without
the token); add the template to the transactional set and either bypass suppression for enforcement
notices or treat a suppressed address as "no contact details known" (DSA Art 17 conditional) and
record it; add `notice_channel`, `notice_sent_at`, `notice_delivery_state`, `notice_proof` to the
case; add the missing enum variant + template + tests.

### 9. Ed25519 edge attestation attests nothing the origin lacks, promotes a deferred item, and adds a new trust root

**Severity: Medium. NEW.**
Why v1 still fails: A9 §4.2 showed the origin already knows the request transited Cloudflare
(mandatory `CF-Connecting-IP`, CF-only ingress) and that the only new value is `request.cf` fields
the origin cannot verify (ASN, bot score, JA4). On the target plan JA4/botScore are absent, and
v1's ladder does not use them, so v1 signs an already-present, already-trusted header set with an
empty claim set. A9 recommended deferring edge attestation; v1 made it normative with no Worker,
no wrangler config, no tests. New trust root: the private key is a Worker secret readable by anyone
with CF account/`wrangler` access — the origin trusts the CF _account_, not a hardware boundary.
Rotation needs a coordinated origin public-key list update; Worker-first rotation drops all
attestations (silent in observe mode, 503 in required mode); no revocation, ceremony, or
absence-rate alarm specified.
Fix: cut edge attestation from v1 (or mark defer-until-consumer). If kept, name the one decision it
gates, state the plan requirement per claimed field, define key ceremony + rotation order
(origin first), add an absence-rate alarm, and accept that CF account compromise = attestation
forgery.

### 10. Randomized enforcement delay: no obfuscation benefit, real audit/DSAR/Art 22(3) cost

**Severity: Medium. NEW.**
Why v1 still fails: a delay shifts _when_ an effect lands, not _what_ crosses the threshold; the
crossing stays observable, so the boundary is still measurable — the stated goal is not achieved.
If the randomization is on _whether_, the same score sometimes bans and sometimes does not:
arbitrary and indefensible. It also destroys reproducibility (same score, different treatment),
contaminates B6's precision/overturn/TTD metrics, and makes DSAR answers and Art 22(3)
contestations ("why me, why now?") unreconstructible.
Fix: deterministic evaluation with a fixed documented grace period; record `rule_version`, `score`,
`threshold_ban`, `threshold_restore`, `evaluated_at`, `effective_at`, `reason_codes`,
`decision_source`, `reviewer_id`, `rationale`. If boundary probing is the threat, stop exposing
ladder transitions (uniform responses/latency) instead of randomizing decisions.

### 11. "No entity graph" contradicts the 2-hop panel; untyped edges and unaudited read access

**Severity: High. NEW.**
Why v1 still fails: B6 ships an on-demand 2-hop expansion capped at 20 accounts in the same bullet
that claims no entity-graph clustering — a graph query with a display cap. Edges are untyped: same
token (strong-ish), `device_key`/cohort co-occurrence (strangers on identical hardware — A2's
inadmissible class), credential reuse, IP. An operator who reads a cohort's case history as
"linked accounts" propagates enforcement across strangers. Access: Moderator is a read role with no
case scoping, so any operator can enter any account and see its devices and co-accounts — for a
Quran app, a device/association map of a sensitive population; `audit_logs` is named but no
case-scoping, bulk-read alarm, or periodic review is defined. The purpose-limitation guard must
whitelist the panel's device × account join, making the whitelist the new control point (unstated);
and the guard has no automatic path today (`.github/workflows/` = `images.yml` only; the sole hook
is `pre-commit` → `vp staged`).
Fix: rename the claim (no precomputed clustering; case-scoped expansion only); type and weight
edges, display them separately, never merge into "linked accounts", and require a second
independent edge before enforcement use; gate the panel on an open case id + logged reason, cap and
alarm bulk reads, review the read log; define the guard-whitelist review; add CI or a pre-push gate
for the guard.

### 12. Compliance scope misallocated: device artefacts are contingent; the shipped legal debt is untouched

**Severity: High. NEW.**
Why v1 still fails: B7's DPIA, per-MS ePrivacy analysis, retention rows, Art 17 template, appeal
SOP, minors device mode, device i18n, `legal_holds` table and machine guard exist almost entirely
because of the device layer — cut the layer and most collapse. Meanwhile the shipped system has
live debt v1 does not touch: `user_sessions.device` raw UA + `ip_address` client IP with no
retention rule or policy mention; the privacy policy is a placeholder marked "not legal advice";
`user_bans` is unwired so account-level enforcement does not exist; `post_comment_v1` already
restricts user content (Art 17 may already be owed); the FCM `devices` table has no expiry.
Fix: sequence compliance to the shipped system first — real policy; documented basis + LIA for
account-level abuse prevention; notice + appeal for account/content restrictions; retention rules
for already-stored identifiers (`user_sessions` UA/IP, FCM `devices`); a DSAR/erasure procedure.
Make device-specific artefacts conditional on a go/no-go for the device layer. Irreducible minimum
for legality of what exists is those five items.

### 13. The "dedicated lazy Worker" collector cannot read several declared signals; B2's invariants contradict each other

**Severity: Medium. NEW.**
Why v1 still fails: B2 says the adapter is "the only file that touches browser APIs" and that
collection "runs in a dedicated lazy Worker… budget <=30ms total, <=5ms chunks". Worker scope lacks
`screen`, viewport/`visualViewport`, `matchMedia`, `display-mode`/standalone context and document
state — the declared "coarse screen bucket", "context (browser/webview/standalone)" and the
screen-vs-inner coherence predicate cannot be computed there; `navigator.platform`/UA-CH
availability is engine-dependent in workers; and `<=5ms chunks` implies cooperative main-thread
scheduling, which a worker does not need. Either the main thread also touches APIs (contradicting
"only file"), or the payload loses signals and the server predicates for them are unimplementable.
The 30ms/5ms budget is asserted, not measured on low-end devices.
Fix: split the seam explicitly — main-thread adapter for screen/display-mode/viewport/context,
worker for compute-heavy signals — or drop the worker and run a chunked main-thread collector;
state which signals come from which thread; replace the asserted budget with a measured budget and
a low-end profile test.

### 14. 30-day key retention and the panel's investigation value are mutually exclusive

**Severity: Medium. NEW.**
Why v1 still fails: B3 deletes keyed records at 30d; the panel and case work need device→account
edges (`device_sessions`, retention unstated, presumably child rows die with the record). The ban
row lives 12mo, but the linkages it was meant to help investigate die at 30d — "who else used this
device" is unanswerable 31 days after the last sighting, which is exactly when a 12-month ban is
investigated. Keeping the key/edges longer breaks the retention ladder and the "no profile past key
expiry" rule (A5 §2.3). v1 does not resolve the tension.
Fix: choose one and write it: (a) accept a 30d investigation window and say so in the SOP; (b) keep
a ban-scoped minimal linkage record (case id + stable record id + keyed edge digest, no profile
fields, no raw signals) for the ban's lifetime under Art 17(3)(e) with an explicit expiry; (c) drop
the panel. Either way the retention schedule and the panel must reference the same rule.

---

## 3. What to cut

If the round-4 answers to Q1/Q2 keep the device layer: the boot-map enforcement path, the panel, the
randomized delay, the Worker-only collector claim, and the edge attestation should be removed or
re-scoped before any code is written. If Q1/Q2 resolve to "not worth it": cut the collector, the
tables, the Worker, the panel and the device-specific legal artefacts; keep the account-scoped ban
work (B1) and the five-item legal minimum from finding 12. In both cases, fix the three things v1
did not touch: the queue (caps, aging, tiering), the case lifecycle (state machine + retention +
transaction), and the notice channel (delivery proof + suppression bypass).
