# D2 — Adversarial audit of the RECOMMENDED v1 scope (C5 §8.1, WS0–WS9)

Auditor: D2, adversarial pass. Date: 2026-10-12. Repo: `/Users/hmziq/os/easyquran` @ HEAD `53d3d421`.
Target: **the plan**, not the fingerprint design — C5 §8.1 ("smallest defensible v1", WS0–WS9) + §8.2/§8.3,
as scoped by C1 §8 (PR order, Gate 0/1, dev-days) and C4 (shipped compliance debt).
Method: read C5, C1, C3, C4, B2, B7, B3, B9; re-verify every new load-bearing repo claim at HEAD
(`file:line` below); attack each workstream as if implemented **as written**, then ask what it breaks,
what it leaves open, and what the operator does at 3am.

**Verdict up front.** The _direction_ (account-centric, token deferred, reader path as cost) survives.
The _plan_ does not, in five places:

1. **WS1 is not the compliance minimum.** C4's own blocking register is ten items (B1–B10); WS1 covers
   about four, and it publishes a policy text that is false in **12 specific sentences** the moment it
   ships. C4's one sequencing rule ("retention job before policy") is violated by the ordering.
2. **WS2/WS3 are sized for a system that has cases.** They build a case state machine, queue caps/aging,
   and an appeal portal for a system with **zero** cases, while the five items that _are_ the ban's
   legal contract (index, session kill, banned UX, audit row, notice row) are not listed as a bundle.
3. **WS4 ranks the wrong things first and guards one door.** The limiter fix is the only weapon-removal
   item; the registration wall hits CGNAT/schools; the Gmail dot-alias hole voids the cost model; and
   **OAuth account creation bypasses every admission control** the workstream adds.
4. **WS8 is inside the "smallest defensible v1" without the Gate 0 that C1/C3 require**, at a measured
   base rate of zero, as a 12-month bearer identifier whose limiter value is defeated by one line of
   storage clearing.
5. **Six blocking compliance items are in no workstream at all**, plus the live DSA Art 17 surface
   (comment hide/delete), the incident-response runbook, and the two measurement one-liners every gate
   depends on.

Corrected ordering in §12; bottom-up estimate **≈28–42 dev-days** for the true smallest v1 (not 21–31);
the item most likely to blow up is **WS6 retention execution**; the gate in §10 replaces Gate 0/1.

---

## 1. WS1 — compliance minimum: wrongly ordered first, and not the minimum

### 1.1 The ordering rule C5 breaks

C4 §7, "One sequencing rule", verbatim: _"B3's retention job must land before B2's policy is published,
or the policy will state periods the system does not honour — the exact failure mode this audit exists
to stop."_ C5 orders **WS1 (policy) at 2** and **WS6 (retention execution) at 7**. The full policy text
(C4 §2) therefore ships before the mechanism that makes any retention sentence true.

What breaks when a policy precedes the mechanism:

- Art 13/14 accuracy + Art 5(1)(a) fairness. A published retention period with no job behind it is not
  an oversight, it is a **documented false statement**, quotable in a complaint. C4's own rule exists
  because that is worse than no policy.
- The DSAR/erasure procedure (WS1) promises a one-month response; the erasure it describes cannot be
  executed truthfully while `user_sessions` UA/IP, FCM tokens and `email_suppression` are "forever"
  (C4 §3).
- Once published, every later fix has to _amend a published claim_, which is more expensive than
  writing the true claim once.

**Correct shape:** split WS1 into **WS1a — truthful interim policy** (only claims the code can honour
today; D2/D5-reword/D10/D11/D13/D18, ~2–3 d) and **WS1b — full policy text** (C4 §2, after WS6 is green
and its boot invariants pass, ~0.5–1 d). The interim policy is _required now_ (processing is live); the
full text is required only once the mechanisms exist.

### 1.2 Every claim in C4's §2 policy text that is false at the moment WS1 ships

Checked against C4 §0/§1 and C3. `[WS6]` = made true only by the retention job; `[missing]` = no
workstream in C5 creates it.

| #   | Sentence (C4 §2)                                                                                                                 | Why false at ship time                                                                                                                                                                                                |
| --- | -------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | "Reading needs no account **and no analytics**"                                                                                  | Analytics initialises on every boot **before** any consent check (`boot/analytics.ts:20-31`); `analytics: true` default (`consent.svelte.ts:16`); logged-out readers are tracked. **C4 B1 — blocking, and in no WS.** |
| 2   | "We store your IP address and browser string … **Kept for 90 days, then deleted**"                                               | `[WS6]` No sweeper exists; `user_sessions` is forever (C4 D3).                                                                                                                                                        |
| 3   | "Push token … **90 days after your device last registered**"                                                                     | `[WS6]` No sweeper (C4 D4).                                                                                                                                                                                           |
| 4   | "Account details … **deleted within 30 days of account deletion**"                                                               | True only if WS1's self-service delete ships; it is admin-only today (C4 D5).                                                                                                                                         |
| 5   | "Usage analytics … **it is off unless you turn it on**"                                                                          | False today (see #1).                                                                                                                                                                                                 |
| 6   | "Crash reports … **Only if analytics is ON**"                                                                                    | Crash reporting rides the same default-on pipeline (`boot/crash-reporting.ts:2-6`).                                                                                                                                   |
| 7   | "Comments … **up to 12 months after we hide it**"                                                                                | `[WS6]` No sweeper; hidden comments are forever.                                                                                                                                                                      |
| 8   | "Newsletter … **While subscribed + 12 months after you unsubscribe**"                                                            | `[WS6]` No sweeper; and the table does not even exist (C3 F8).                                                                                                                                                        |
| 9   | "**Every restriction can be appealed, and the appeal is decided by a person**"                                                   | No appeal path exists; WS3 is later than WS1 in C5's order.                                                                                                                                                           |
| 10  | "If we hide or remove something you posted … **we will tell you what was decided, why, and how to challenge it**"                | No notice channel; comment hide/delete is silent **today** (C4 D7 — live Art 17 exposure, and §11.1 below).                                                                                                           |
| 11  | "the ban notice contains **a link that works without an account**"                                                               | No ban notice, no page.                                                                                                                                                                                               |
| 12  | "We use a small number of providers, **each under a data processing agreement** … **with a transfer impact assessment on file**" | `[missing]` No DPA/TIA/ROPA exist (C4 D16 — "nice-to-have before growth", but the policy asserts them as present). Also "We take backups" (item 4's honest-limit sentence) — **no backup exists at all** (C4 D15).    |

Also in the same text and in no workstream: minors ("we do not knowingly create accounts for children
under 13 (or under 18 in India)", "no analytics or push for minors", "minors cannot post") — C4 D12/B8,
India DPDP s.9(3) is a hard prohibition; and the children's section is a promise the schema cannot keep
(`user` has no DOB/age field).

**Net:** the policy text is not a copy task. It is a claim ledger, and 12 of its sentences are blocked
on code that C5 schedules _after_ it or does not schedule at all.

---

## 2. WS2 — `user_bans` wiring + audit + case machine + queue caps: what is the actual minimum

### 2.1 What is over-engineered

C5 WS2 = (a) `user_bans` admin API, (b) `audit_logs` writers, (c) **enforcement-case + `case_events`
state machine (D26)**, (d) **queue caps/aging/tiering + solo-operator policy (D12/O6)**.

D26's state machine (`state`, `proposed_at`, `decided_at`, `effective_at`, `expires_at`, `reviewer_id`,
`second_reviewer_id`, `rationale`, `ban_ref` + append-only child) exists in the ledger to carry
**device evidence** and the L0–L7 ladder. The device layer is deferred (D1), and the ladder is not
implemented in v1 (D14: the only enabled rule that can touch a user is R01/R04a/R04b/R06, all
non-ladder). So in v1 the state machine has:

- no device evidence to link,
- no automated rule to ladder,
- **zero cases** (C3 §3: `user_bans = 0`, no automated ban by configuration, no admin route),
- no second reviewer to populate `second_reviewer_id` (B9 F4: one operator).

Queue caps/aging/tiering with a 1–4 cases/day throughput ceiling and zero cases is a control for a
queue that does not exist. It is **not** in DSA Art 17 or GDPR Art 22.

C4 §4.4 already specifies the right minimum and it is duplicative with D26:
`enforcement_notices` — `subject_type/subject_id/decision/scope/starts_at/ends_at/reason_code/
reason_text/facts/automated_means/contract_ground/decided_by/decided_at/notice_channel/notice_sent_at/
notice_delivery_state/appeal_token_hash/appeal_window_ends_at/appeal_outcome/retention_class` —
wired into **both** `post_comment_v1` admin hide/delete and the new `user_bans` admin path.
That single table _is_ the case record for a one-operator, human-decided system. The state machine is a
second, incompatible representation of the same facts.

### 2.2 What breaks if `user_bans` wiring ships WITHOUT the case machine

Nothing functional. The case machine is a reporting artefact. The failures are elsewhere:

- **Without session revocation:** the ban bites in ≤5 min on `not_banned()` routes and never on routes
  that lack the requirement (C1 F9). A ban that leaves the session alive is not an enforcement primitive.
- **Without the banned login UX:** the client shows "Email or password is incorrect" because AUTH_006 is
  in `CREDENTIAL_TYPES`; the limiter runs _before_ the ban check, so the user burns the login budget and
  eventually sees "Too many attempts" instead of the ban (C1 F10).
- **Without notice:** silent permanent lockout; the in-app channel is unreachable for a 403'd session.
- **Without appeal:** no redress route; DSA Art 17(3) redress info cannot be given.
- **Without audit rows:** no record of who banned whom and why — C3 F7, and the C1 instrument dies.
- **Without the index:** `get_active_ban` is a full scan on a pool-of-1 connection, executed per login
  **and per authenticated user every 5 minutes** (C1 §1; `m000001_init.rs:58-73` — no `CREATE INDEX`).
- **Without idempotent create:** `get_active_ban` returns one row (newest active); stacked bans make
  revoke ambiguous (C1 §5.1.4).

### 2.3 Minimum viable version (WS2a) — the five-item ban contract

1. `modules/admin_user_bans_v1` — `POST/GET/DELETE /admin/users/bans` (list/create/revoke; reason,
   `expires_at`, `banned_by`), ROLE_ADMIN + CSRF, idempotent create (refuse or revoke-then-create),
   bearer export token **never** on mutation routes.
2. Migration: `CREATE INDEX IF NOT EXISTS idx_user_bans_user_active ON user_bans (user_id, revoked_at,
expires_at)` + `enforcement_notices` table (C4 §4.4) with `expires_at INTEGER`/`retention_class`.
3. `session_auth_secret` bump in the same transaction as the ban row (C1 F9) + `#[serde(default)]`
   compat test for `AuthSessionState` (constraint 20).
4. Client split: remove AUTH_006 from `CREDENTIAL_TYPES`, add a `banned` error kind with copy + appeal
   link, stop the retry loop, move the ban check before the limiter.
5. `services/audit.rs` writer + rows for ban create/revoke + one `enforcement_notices` row per decision.

**Cut from WS2:** `enforcement_cases` + `case_events` (defer with the device tranche; fold `state` into
`enforcement_notices` if a state is ever needed), queue caps/aging/tiering (re-add when cases/day > 0),
the L0–L7 ladder, and the two-person substitution policy text (keep one sentence: "permanent bans are
issued as auto-expiring bans, renewed on review; `single_operator_control` is recorded").

**Ships with WS2a, in the same release:** WS3a (notice + record) and the WS9a policy invariants (§7.2).

---

## 3. WS3 — appeals + notice + delivery proof at zero cases

### 3.1 Is it dead code?

Half of it is. Split by what is actually owed:

| Duty                                       | Attaches today?                                                                                                                                            | Minimum                                                                                                                                           |
| ------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| **DSA Art 17** statement of reasons        | **Yes** — hosting service, comments live, `admin_hide`/`admin_delete` already restrict content (C4 §4.1). Art 17 is Section 2, **not** exempted by Art 19. | Notice template + a durable `enforcement_notices` row + a working redress route.                                                                  |
| **DSA Art 20** internal complaint-handling | **Exempt while micro/small** (Art 19 excludes Section 3). Document the exemption + a review trigger (C4 §4.5).                                             | A written micro/small assessment (0.5 d).                                                                                                         |
| **GDPR Art 22**                            | **No** — nothing automated exists, and C5's own D12 requires human-before-effect. "The duty is _not to build it automated_" (C4 §4.1).                     | **Do not build automation.** If a scored rule ever proposes and a human rubber-stamps, Art 22(3) attaches — that is the trap, not the ban button. |
| **LGPD Art 20**                            | If Brazilian users are restricted — broader than Art 22.                                                                                                   | Same notice + a human decision (already required).                                                                                                |

So the **legally load-bearing** minimum is: a notice email (transactional, with reason, human/automated
flag, how to challenge, and a "what we looked at" line), a durable record row, and **a redress route
that works without an account**.

### 3.2 The cheapest sufficient redress route (what can be deferred)

A tokenized page is _not_ required. The notice can say: reply to this email, or write to
`appeals@easyquran.fyi`. That is a real, human, documented redress route, and it is better than a page
for a zero-case system because it needs no web work, no i18n (the web i18n gate requires **all locales**
for new user-facing strings — constraint 23; the mail templates are not under that gate), no token
lifecycle, and no public endpoint that must be built to resist enumeration.

**Ship in WS3a (~1.5–2.5 d):**

- `enforcement_notice.html` template + `NotificationKind::Enforcement` + `enforcement_notice` in
  `is_transactional` (`provider.rs:17-22`).
- `enforcement_notices` row written **before** the send (poor-man's outbox — the row is the queue), then
  `notice_sent_at` / `delivery_state` updated. Without this ordering, an inline `tokio::spawn` crash
  leaves "sent" recorded for a mail that never left.
- Delivery-state honesty: adding the template to the transactional set does not by itself make it
  deliverable — transactional + suppressed is dropped with `Ok`
  (`mail/router.rs:241-258`). The correct fix: on a **Complaint**-reason suppression, send anyway
  (CAN-SPAM exempts transactional messages; a marketing complaint must not block a legal notice); on a
  **Bounce**-reason suppression, do not send and record `delivery_state='suppressed'` (H3 #22).
  That distinction is the whole point of the field.
- Appeal intake = monitored mailbox (`appeals@easyquran.fyi` — must actually exist; see D11) + one SOP
  paragraph: who reads it, response window, self-review recorded honestly, reversal → revoke ban.

**Defer safely:** the public `POST /appeal/v1/submit` endpoint, the tokenized page, appeal rate limits,
uniform-response design, `X-Robots-Tag`/`Referrer-Policy` page hygiene, the SLA dashboard, the
out-of-court-settlement line. Build the page **at the first real ban** (~1 d, from the template), when
there is a case to bind a token to. Building it now is ~2–3 d of code that cannot be tested against a
single real case and will be rewritten once the first appeal reveals what it must do.

**Cannot be deferred:** the notice itself (Art 17 is owed for comment hides **today**), the record row,
and the redress statement. Also cannot be deferred: wiring the writer into `post_comment_v1`
admin hide/delete — see §11.1.

---

## 4. WS4 — limiter fix + registration + Turnstile + sessions + comment hardening

### 4.1 Ranking by real effect at a measured base rate of zero

| Rank | Item                                                                                                                                                                                                | Effect                                                                                                                                          | Cost      | Cut/keep                             |
| ---- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | --------- | ------------------------------------ |
| 1    | **Limiter correctness** (failures only, `abuse_peek`/`abuse_record`, blocked path must not append, success clears the credential key only, never a 429 on the credential, ban check before limiter) | Removes the free 1 h/24 h account-lockout weapon; makes every future auth metric unconfounded                                                   | 2–3 d     | **Keep — safety-critical**           |
| 2    | **Fresh-account privilege limits + comment hardening** (<24 h: 1 comment, no links/media; confirmed-violating duplicate detection scoped per D15)                                                   | The harm reduction actually lands here (C1 §3.4 item 5), and it is the answer to incognito-per-signup farming (C1 F12 #2) without a device axis | 1.5–2.5 d | **Keep**                             |
| 3    | **Gmail dot-alias canonicalization** (`verification_identity_hash`, dot-strip for dot-insensitive providers, cap accounts/identity)                                                                 | 60 LOC; kills the "one inbox = ∞ verified accounts" model that voids per-domain/disposable/inbox controls (C1 F3)                               | 0.5–1 d   | **Keep**                             |
| 4    | **Registration soft caps + Turnstile prod deploy check** (3 free → 5 challenge → 20 block/24 h; CIDR allowlist; per-ASN challenge **only if** the ASN dimension exists)                             | Removes the naive 144 k/day/IP case; raises the floor to "cheap"                                                                                | 1–1.5 d   | **Keep, re-scoped**                  |
| 5    | **Turnstile on login/reset**                                                                                                                                                                        | Real only against naive credential stuffing, which the per-credential + per-IP counters already cover                                           | 1–2 d     | **Defer behind the fallback ladder** |
| 6    | **Session list** (terminate-all, coarse label, new-session email)                                                                                                                                   | Security/ATO alarm, **not abuse reduction** (B2 F10 class)                                                                                      | 2–3 d     | **Defer**                            |
| 7    | Per-domain/day cap, hard per-IP block, invite mode, per-ASN block                                                                                                                                   | Zero evasion value / high FP / emergency switch only                                                                                            | —         | **Cut**                              |

### 4.2 What the limiter fix breaks

It is not a controller edit. C1 F1: `abuse_check` must split into `abuse_peek` (read-only) and
`abuse_record` (append+evaluate) on the `RateLimitStore` trait + the in-memory impl + the
`PersistentStore` wrapper, with **five clone call sites** (`auth_v1/controller.rs:299,411,582,813,918`;
`email_verification_v1:49`; `forgot_password_v1:337,503`) and existing non-auth callers (comments,
resends) where **counting throughput is intended**. Failure modes if done carelessly:

- Keeping `abuse_check` = `record` for the old callers preserves their semantics but leaves the
  blocked-path append in place for them — acceptable only if the credential keys are switched to
  `peek`/`record` explicitly. A blanket "no append on block" change silently weakens the comment limiter.
- Clearing on success must be scoped to `login_fail:<email>` only. Clearing the IP key lets an attacker
  log into their own account to reset the counter (C1 §2.3 rule 3).
- Deleting the long tier for credential keys but not for the IP key leaves a 1 h CGNAT-wide 429 weapon
  (bounded, accepted residual — but state it).
- The 24 h lockout is **not** fixed by "failures only": it is the blocked-path append + the long tier
  (C1 F2). A patch that only counts failures keeps the weapon.

### 4.3 What registration control breaks for legitimate users

- Hard per-IP/day cap: CGNAT mobile (the core audience), madrasa/school labs, cyber-cafés, family
  sharing one connection. A block at 3–5/day is the school-block failure A2 warned about. Soft cap +
  challenge + CIDR allowlist only.
- Per-domain cap: schools/companies on one Workspace domain; useless against gmail.com. Only a
  disposable-domain _list_ is defensible.
- Per-ASN: mobile carrier = one ASN = a country; and it needs the ASN dimension, which needs the Worker
  or a baked table — neither is in this plan. Skip if not shipped.
- Invite-only: growth cost; emergency flag only, off.

### 4.4 Turnstile on login: availability cost, and the deploy trap

`turnstile.rs:17-19,42-47`: **in production, unconfigured → 503; configured → token mandatory**;
`:91-94` fail-closed. And the web only renders the widget when `PUBLIC_TURNSTILE_SITE_KEY` is set
(`web/src/lib/auth/turnstile.ts:4-7`, `RegisterForm.svelte:51-55`). `docker-compose.yml:73` passes
`PUBLIC_TURNSTILE_SITE_KEY` through with an empty default while the API secret lives in `.env` — so the
default deploy state is **either registration-gated or registration-dead**, and nothing tests which.
Putting the same fail-closed primitive on login means: one missing env var, one blocked
`challenges.cloudflare.com`, or one Cloudflare challenge outage locks every user out of their account.
Correct form: challenge only after N failures, with a fallback ladder (Turnstile → email OTP →
progressive delay), and never on reading, recovery, or appeals.

### 4.5 The door WS4 does not guard: OAuth

Verified at HEAD: `google_auth_v1`, `github_auth_v1`, `discord_auth_v1`, `facebook_auth_v1`,
`apple_auth_v1` each create/attach accounts. They check provider email verification carefully
(`google_auth_v1/controller.rs:545` rejects `email_verified != Some(true)`;
`github_auth_v1/controller.rs:540-582,644-656` prefers the verified `/user/emails` address and treats
the public email as unverified) — that part is sound. But **no OAuth module calls Turnstile** (grep:
zero `turnstile` hits in the provider modules) and none is covered by any per-IP/day or per-identity
cap; the only limit is the shared 60/min nest bucket (`router.rs`), i.e. **86,400 accounts/day/IP per
provider**. A registration admission workstream that guards only the password door is defeated by
pressing "Continue with Google". Fix: extend the admission rule to the OAuth callback (per-IP/day +
verification-identity cap; no Turnstile — it would break the redirect flow), or state the residual.

---

## 5. WS6 — retention: is it really required before WS2/WS3?

**No.** The dependency runs the other way.

- The legal exposure of shipping bans before a retention job exists is **negligible at zero case
  volume**: a ban row is a `reason` string + an admin id; a notice row is an email + a reason. At
  ~0–5 cases/year that is nothing next to the live exposure it does not change: **raw UA + IP per login,
  forever, for real traffic** (C4 D3), FCM tokens forever (D4), `email_suppression` forever.
- The real constraint is C4's sequencing rule: **the full policy text must not ship before the sweeper**
  (§1.1). That is a constraint on WS1b, not on WS2/WS3.
- One sharp addition: the notice/appeal records are _new_ PII, and appeal free-text can contain
  special-category material (a user explaining their religious context). New tables must therefore carry
  `expires_at INTEGER` + `retention_class` **at creation**, and be registered in the sweeper config in
  the same PR — otherwise WS2/WS3 create the next "forever" class, and the full policy's 12-month claim
  is false on the day it is published.

**Correct order with reasons:** WS2a/WS3a (enforcement + notice, retention columns from day one) →
WS6 sweeper (covers the live tables first: `user_sessions`, `devices`; then the new classes) →
WS1b full policy. If forced to choose a single first item between "sweeper" and "ban button": sweeper,
because it acts on real volume; but the gap between them is days, not weeks, and neither is a
prerequisite of the other.

---

## 6. WS7 — reader-path cost work: reliability, and it belongs first

### 6.1 Abuse or reliability?

Mostly **reliability with a cost tail**, and C5 files it as abuse work at position 8. B3's findings are
not adversarial-only:

- **F2 (the biggest):** the disk cache and the edge cache are disabled by _any_ cookie: GA4's
  default-on `_ga` on every returning reader, plus `ruxlog.sid` once a visitor's first unsafe `/api` call
  mints an anonymous session (E6; `router.rs:57-60`, `main.rs:781-788`; predicate
  `hooks.server.ts:345-351`, `:130-137`). The **default** state of a returning visitor is "cookie present" → `private, no-store` → full SSR on every reader page. That is a site-wide
  performance bug for every user, attacker or not. The fix is a path-scoped session cookie plus a
  name-specific cache predicate keyed on a login-only marker (E6 P0.1), plus a test.
- **F1:** every translated render funnels through **one global 600/min internal bucket**
  (`rate_limit.rs:68-79`, `PathKey::Fixed("quran-v1")`). ~600 requests/min from one IP 429s _every_
  translated page and `.md` on the site. No credential, no cost. The secondary hazard is worse for
  detection: with `INTERNAL_QURAN_API_BASE` unset, all SSR collapses into the web container's single
  egress peer and is indistinguishable from abuse (B3 F1).
- **F3:** `/offline/pack` is 14.1 MB unauthenticated from the un-limited web origin; a client omitting
  `Accept-Encoding` gets 14.1 MB per request; one IP at 600/min = up to 8.4 GB/min of origin egress;
  `.json` is not edge-cached by default (B3 F3).
- **F6:** with a partially published R2 bucket, the all-or-nothing memo turns **one** request into up to
  380 outbound HEADs in prod (382 non-prod), serialized by the memo mutex, through the shared `http_client` that also serves mail, billing and FCM (B3 F6).
- **F11:** one IP cycling 378 translation ids thrashes the 8-entry / 48 MB corpus pool (B3 F11).

Honest caveat: this is demonstrated **by construction, not by an observed outage** — production has no
per-route 429 visibility at all (§11.5). But it needs no adversary: a crawler, an LLM harvester, or a
misconfigured client triggers it, and F2 makes it worse for everyone.

### 6.2 Does it belong, and first?

Yes, and mostly first, because:

- it adds **no identity surface, no FP class, no legal artefact** — the cheapest kind of change;
- its biggest item (cache restoration) is a **product-wide perf fix**, not abuse work;
- it removes the _only_ documented availability risk in the plan (B3 F1/F3), while the abuse
  workstreams at zero base rate address risks with no evidence (C3 §3).

**Move into phase 0 (WS7a, ~3–4 d):** cache restoration (O8 decision + test), split the internal bucket
by route class, `INTERNAL_QURAN_API_BASE` missing = production boot error, per-IP limiter + byte budget
in the web fetch pipeline, `/offline/*` byte budget + committed CF Cache Rule, `/_quran/*` per-IP limit,
robots fix, `docs/bulk-data.md` + `/.well-known/quran-bulk.json` + `/llms.txt` section.
**Cut/defer:** ASN cost tier (needs Worker or baked table — not in this plan), harvest-shape score
(needs the shadow/rules machinery — device tranche), in-flight cap (fold into the limiter).

---

## 7. WS8/WS9 — device token and shared-device protection

### 7.1 Is WS8 justified now? No — and C5 contradicts its own gate

C1 §8.2: _"**Do not run PR5 before Gate 0**"_, and Gate 0 requires ≥5 ban-worthy events/quarter and ≥10
confirmed evasion cases. C3 F5: the C1/C2 denominators are **zero**. Yet C5 §8.1 lists **WS8 at order 9
inside the "smallest defensible v1"**, with no gate, and C5 §8.4 only gates WS10+. So the plan schedules
the thing its own evidence says not to schedule.

At a measured base rate of zero, WS8 ships:

- a **second 12-month bearer identifier** (`HttpOnly` prevents reads, not use; C1 F6) whose theft
  consequence is attribution poisoning — the same CSP trusts `gstatic`/`googletagmanager`/
  `challenges.cloudflare.com` with live Firebase Analytics (B10 §1.10);
- a **limiter whose value is defeated by one line of storage clearing** (C1 F8; 32% of browser
  instances clear cookies in 8 months, Li & Cao) — the only genuine delta is the low-collision linkage
  edge, which needs **cases** to be actionable;
- a new retention class, a privacy paragraph, and a `device_account_links` table that will sit empty.

**Correct trigger (see §10):** the token starts only when the operator can point at a real case where a
durable identifier was the missing corroborator. Until then, record the _policy invariants_ (§7.2) and
nothing else.

### 7.2 Is WS9 buildable before WS8? No — but most of WS9 is not code

C5 §8.1: WS9 "Depends on WS8", and it references the token tables (`self-claim`, kiosk grants bind to
the device record). As written it cannot precede WS8. But B5 P1–P6 are **rules**:

- device is never a ban unit; unknown device = zero friction;
- a shared-device self-claim **never clears an account ban** and never changes a quota;
- device friction never gates login, reading, recovery, or appeals;
- linkage is evidence only, never auto-action; a second independent corroborator is required.

Those four lines cost ~0.5 d and belong in **WS2a/WS3a** as written operator rules + the notice copy —
because the first ban is exactly when they matter. The code half (self-claim, kiosk code, `kiosk_review`,
`non_actionable`, context join rule) is WS9b, after WS8, ~2–3 d.

---

## 8. WS0 — gates restore: legitimate, but under-scoped and misread

### 8.1 Is restoring them an overreach?

No — but the justification in C5 ("protects §5 constraints") is too broad. The user removed the gates
deliberately in `53d3d421`, and B7 §8 records that they were **removed before ever being observed
green in CI**: provisioning failed on a fresh runner (`bed80b13` → fixed in `37d90c41` → removed in
`53d3d421`). So "restore the gates" is not a revert; it is a **re-validation project** with an unknown
number of provisioning/serialization passes. The legitimate scope is the minimum that guards _this_
work, not the whole pre-`53d3d421` pipeline.

### 8.2 The minimum that protects this work

1. **Web gate block** (`check`, `lint`, `test`, `bun audit`) as a separate required job in
   `publish.needs` — sub-minute compute (B7 §8: 9.5 s + 3.9 s + 11.2 s + 0.33 s), and it brings back
   every web machine guard (nav, catalogue-sha, i18n, route isolation, design).
2. **Rust gate block for the crates this work touches** (`cargo fmt --check`, `cargo clippy
--workspace --all-targets -D warnings`, `cargo test --test-threads=1`) — the expensive one; expect a
   provisioning pass.
3. **New guard tests in the same PRs that create the invariants** — otherwise WS0 buys nothing for this
   plan: `ban_create_bumps_session_auth_secret`, `idx_user_bans_user_active` via `EXPLAIN QUERY PLAN`,
   `reader_cache_predicate_ignores_anonymous_csrf_cookie`, `no_device_cookie_on_reader_routes`,
   `retention_boot_invariant`. B7 §4 ranks these as the invariants most likely to break silently.
4. **Doc truth** (`documentation/history.md:36`, `DEPLOYMENT.md:69,74`, `rust/backend/api/AGENTS.md` sentence,
   stale `images.yml` comments).
5. **Do not restore** `cargo-audit` inline (install + network on cold runners) unless cached; keep it
   as a scheduled job or drop it. Keep `deny.toml` policy as is (B7 §2.1: not relaxed).

### 8.3 Is the commit-hook fix separate?

**Yes.** B7 F2 proves empirically that `vp check` reads the workspace-root `lint`/`fmt` blocks only, so
`no-nested-ternary`, `no-shadow`, `no-floating-promises`, the anti-slop rules, `svelte-check` and every
test are invisible at commit time — and root `vp check` type-resolves web files unreliably in both
directions (B7 §5). Fix = root `lint.overrides`/`fmt.overrides` (documented Vite+ monorepo pattern) or a
`web/**` staged command (`cd web && vp check --fix`), ~0.5 d. It is a different change from CI restore
and it should not be bundled into the same PR as the workflow edit.

---

## 9. Attack the dev-day estimates

### 9.1 C1's numbers, item by item

| C1 item                           | C1        | D2                               | Why the delta                                                                                                                         |
| --------------------------------- | --------- | -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| A1 limiter correctness            | 1.5–2.5   | **2–3**                          | Trait split + `PersistentStore` wrapper + 5 clone sites + non-auth caller semantics preserved + 6 tests.                              |
| A2 client banned/credential split | 0.5–1     | **0.5–1.25**                     | New user-facing strings hit the i18n gate (all locales) — C1 does not price it.                                                       |
| A3 ban wiring                     | 2.5–3.5   | **2.5–4**                        | Index + idempotency + session kill + serde compat test + banned UX are in _other_ items; the module + route + tests alone is 2.5–3.5. |
| A4 notice + appeal                | 3–4       | **2.5–3.5 (email-only minimum)** | Portal deferred; template + transactional semantics + delivery state + record + SOP. Portal adds 1–1.5 later.                         |
| A5 audit writers                  | 1.5–2.5   | **1–2.5**                        | One service + ~5 call sites; if it doubles as the C1 instrument + block ledger, 1.5–2.5.                                              |
| A6 registration admission         | 2.5–3.5   | **2.5–4**                        | + OAuth door + Gmail canonicalization + Turnstile deploy smoke test.                                                                  |
| A7 fresh-account/comment          | 1.5–2     | **1.5–2.5**                      |                                                                                                                                       |
| A8 session notifications          | 2–3       | **defer**                        | Not abuse reduction.                                                                                                                  |
| A9 CSAM/image moderation          | 3–5       | **3–5.5**                        | Provider seam + hash interface + SOP + retention class + `legal_holds`.                                                               |
| B (token)                         | 9–14      | **7–11**                         | Split: WS8 token 5–8, WS9 guards 2–3 (only after Gate B).                                                                             |
| **A core + B**                    | **21–31** | **21–33**                        | Directionally agrees; C1 is right that B2 was low.                                                                                    |

### 9.2 Bottom-up estimate for C5's actual "smallest v1" (WS0–WS7 + minimums)

| Workstream (as corrected)                                             | Days       | Notes                                                                                |
| --------------------------------------------------------------------- | ---------- | ------------------------------------------------------------------------------------ |
| WS0 gates restore + hook + docs                                       | 1.5–2.5    | incl. one provisioning pass (B7 §8)                                                  |
| WS1a truthful interim policy (D2, D5-reword, D10, D11, D13, D18)      | 2–3        | engineering only; C4 text exists                                                     |
| WS7a reader-path cost (non-identity subset)                           | 3–4        | cache restore, bucket split, /offline budget, CF rules, bulk doc, robots             |
| WS2a ban contract (5 items)                                           | 4–6        | admin module + index + session kill + UX + audit + notices row                       |
| WS3a notice + record + redress (email-first)                          | 1.5–2.5    | + micro/small assessment 0.5                                                         |
| WS5 CSAM/image moderation + legal_holds                               | 3.5–5.5    | uploads are live (`media_v1 /create` behind verified ROLE_AUTHOR)                    |
| Measurement (429 log fix + block ledger + `just abuse-report`)        | 0.5–1      | §11.5                                                                                |
| WS4a limiter + fresh-account + dot-alias + reg soft caps + OAuth door | 6–8.5      | the limiter is the risk item                                                         |
| WS6 retention sweeper + receipts + backups/runbook                    | 4–6        | the schedule risk item                                                               |
| WS1b full policy text + ar translation                                | 0.5–1      | after WS6 green                                                                      |
| **Total (true smallest v1)**                                          | **≈27–42** | + 1–2 policy days; **not** C1's 21–31, because C5's scope adds WS5, WS6, WS7 and WS0 |
| WS8 + WS9b (only after Gate B)                                        | +7–11      |                                                                                      |
| Fingerprint tranche (WS10–WS14, C-only)                               | +38–61     | B2's number; unchanged                                                               |

So C5's "smallest defensible v1" is **6–8.5 solo weeks**, and it is mislabelled: it is smaller than the
full design but it is not small. C1's 12–17/9–14 is a fair estimate _of C1's scope_; C5 silently added
four workstreams without re-costing.

### 9.3 The item most likely to blow up

**WS6 — retention execution.** It is the keystone (C4: "every retention sentence is unenforceable until
a job exists"), and its mechanics are treacherous in exactly the ways that fail _silently_:

- timestamps are mixed TEXT formats (`user_bans.expires_at` RFC3339 vs `audit_logs.created_at` SQLite
  `CURRENT_TIMESTAMP`); a TEXT-vs-TEXT comparison across formats deletes **nothing** and logs no error
  (C4 §3 note 1; C3 F1);
- `DELETE … LIMIT` is unavailable on this build (A10 F15);
- pool of 1: the sweeper must not ride the 10 s flush task (A10 F15/B8 F13);
- no backups exist, so the honest claim is "removed from the live database; backups within 30 days" —
  which requires a backup job that does not exist (C4 D15);
- deletion is not physical erasure without `PRAGMA secure_delete` + checkpoint (C4 §3 note 3);
- the boot invariant is the only thing that turns "we have a job" into "we have a control", and it is
  easy to skip.

Second place, largest **blast radius**: the WS4 limiter store-API change (auth for everyone; a wrong
split either re-opens the lockout or weakens the comment limiter).

---

## 10. Gate 0 / Gate 1: measurable? (No — and C3's own instrumentation is circular)

### 10.1 Attack on C1's Gate 0

| Gate                                                                                          | Verdict                                                                                                                                                                                                                                                          |
| --------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| G0.1 ≥5 ban-worthy events/quarter                                                             | **Measurable** once the audit writer + flags + Cloudflare/GA4 exist. Sound.                                                                                                                                                                                      |
| G0.2 ≥10 confirmed ban-evasion cases                                                          | **Measurable only after bans exist** (circular in time but not in logic: the gate runs after PR1–PR4, and evasion requires a prior ban). Acceptable, but 10 cases at 1–4 cases/day throughput and zero evidence of abuse is a very high bar — that is the point. |
| G0.3 per-IP linkage produced ≥2 unrelated-account collisions **or** failed to link the evader | **Broken.** Under CGNAT — the app's core audience — per-IP collisions are the _normal_ state. This condition fires on legitimate traffic, so the gate **always says "IP is insufficient"** → B always justified.                                                 |
| G0.4 <20% of linked-account investigations resolve as shared device                           | **Circular.** "Linked-account investigations" require the linkage that B creates. Unmeasurable before B.                                                                                                                                                         |

### 10.2 Attack on C3's instrumentation

- The `audit_logs` writer **works** and needs no migration (C3 §4, recommendation) — good.
- The `token_state` field at the decisive event **requires a token to exist**, so C1's numerator can
  only be populated _after_ WS8 ships — i.e. you must build B to measure whether B is justified. C3
  does not flag this circularity.
- **A recapture query must select the banned user explicitly and compare epoch to epoch** (C3 §4's C2
  query does both). Two easy mistakes bias it toward ending the project:

1.  picking the linked user with `(SELECT user_id … LIMIT 1)` selects an **arbitrary** linked user,
    not the banned one;
2.  comparing an INTEGER-epoch `first_seen` (B8 F7) with the TEXT `user_bans.created_at`
    (`CURRENT_TIMESTAMP`) is **always false** in SQLite (INTEGER sorts before TEXT) → `recaptured = 0` →
    C2 reads **0% recapture** → "the token already catches everything" → never build C.

### 10.3 The gate I would use

**Gate B — "does the device token exist as a product?"** Code-enforced by _not scheduling the work_:
ship the audit writer + `just abuse-report` + the 429 log fix (phase 1), then measure over ≥90 days
**or** ≥1,000 real signups, whichever is later:

```sql
-- B1: was anyone actually banned?
SELECT COUNT(*) FROM audit_logs
WHERE action IN ('ban.create','enforcement.decision');
-- B2: did any case lack a durable corroborator? (operator-recorded, one row per case)
SELECT COUNT(*) FROM audit_logs
WHERE action = 'evasion.suspected'
  AND json_extract(metadata,'$.corroborators') NOT LIKE '%credential_reuse%'
  AND json_extract(metadata,'$.corroborators') NOT LIKE '%content_match%';
```

Build WS8 iff **B1 ≥ 1 AND B2 ≥ 1 AND** the operator can name the case in writing. Otherwise record
"not measurable / not justified" and re-run the recipe quarterly. `corroborators` is a closed enum
recorded by the operator (`credential_reuse | content_match | ip | none`) — **not** `token_state`, which
does not exist before the token does.

If built: evidence-only; the token never gates a privilege; WS9b's guards in the same release.

**Gate C (fingerprint):** keep B2's C1/C2/C3, but mark C1/C2 **"not computable until the token
exists"** (C3 F5) rather than "pending", and add C4 (B's own linkage FP budget). Replace G0.3/G0.4 with the B2-style
operator-recorded case count above. **A condition that cannot be measured cannot gate a build** —
including this one, until the ledger exists.

---

## 11. What is missing from the scope entirely

### 11.1 The live DSA Art 17 surface (comment hide/delete) is in no workstream

C4 D7: `admin_hide`/`admin_unhide`/`admin_delete` (`post_comment_v1/controller.rs:233-313`) restrict
user content with no reason, no notice, no appeal — **already happening**, Art 17 owed today. C4 §4.4
says wire the notice writer into those routes. C5 WS2/WS3 are account-ban-centric (`user_bans`, appeals
token bound to the _case_). Net: the plan builds notice machinery and leaves the only live Art 17
surface unwired. Fix: WS3a must include `post_comment_v1` hide/delete → `enforcement_notices` row →
notice email to the comment author. **NEW.**

### 11.2 Six blocking compliance items in no workstream

C4 §7's BLOCKING list vs C5 WS0–WS9:

| C4 blocking item                                                                        | In WS0–WS9?                                             |
| --------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| B1 D2 analytics consent off + gated + dead toggle + consent import + single surface     | **No**                                                  |
| B2 D1 real policy                                                                       | WS1 (partially — see §1)                                |
| B3 D14+D3+D4 retention job + UA/IP + FCM                                                | WS6 (but ordered after policy)                          |
| B4 D5 deletion claim                                                                    | WS1 (self-service)                                      |
| B5 D11 undeliverable contact address (`easyquran.app` has **no MX**; `dig MX` verified) | **No**                                                  |
| B6 D7+D9 statement of reasons + notice                                                  | WS3 (account-only — §11.1)                              |
| B7 D6 DSAR intake                                                                       | WS1 (partially)                                         |
| B8 D12 minors posture (India DPDP s.9(3) — hard prohibition)                            | **No**                                                  |
| B9 D10 newsletter unsubscribe integrity + `List-Unsubscribe`                            | **No**                                                  |
| B10 D13 remaining consent integrity                                                     | **No**                                                  |
| (D15 backups, D16 ROPA/LIA/DPIA/DPAs/TIA, D18 Terms moderation clause)                  | **No** (D16 is asserted as existing in the policy text) |

### 11.3 Incident response does not exist — the 3am question

Uploads are live (`media_v1 /create` behind `verified_with_role::<ROLE_AUTHOR>`; verified email does not
stop CSAM), comments are live, and there is: no image moderation at all (`IMAGE_MODERATION_ENABLED=false` →
`image_moderator = None`; `NoOpModerator` is dead code), no hash list, no NCMEC path, no `legal_holds`, no LE contact, no preservation SOP, no breach-notification
runbook (Art 33/34), no on-call. WS5 exists but is marked "(a) **if** uploads accept untrusted content"
and depends on WS1/WS2. The honest reading: **WS5 is mandatory now**, and it needs a 0.5–1 d runbook
that is not in any workstream: _hide → preserve (hold row) → report (NCMEC if a US nexus; local LE
otherwise) → ban → notice_. C3 §6 point 4 already calls the ban button an incident-response gap, not an
abuse-reduction gap; the runbook is the other half.

### 11.4 Newsletter: a mounted public write endpoint whose table does not exist

C3 F8: `newsletter_subscribers` has no migration and no runtime DDL; `POST /api/newsletter/v1/subscribe`
is mounted (405 on GET) and its `INSERT` fails → unhandled 500; the two abuse limiters guard a write
that cannot succeed. Not in any workstream. Fix: add the migration (columns are fixed) or unmount.
**NEW.**

### 11.5 The measurement one-liners every gate depends on

- **`layer.rs:323` is `debug!`** → at `RUST_LOG=info` (compose default) the 600/min ceiling and **every
  per-route 429 are invisible**. C3 F4 calls this the only abuse work justified at zero base rate:
  one word. Not in any workstream.
- **`rate_limit_state` is a snapshot, not a ledger** (C3 F1: the 10 s flush deletes expired rows; no
  `blocked_at`, no history). "How many blocks in 90 days" is unanswerable, and every threshold in the
  design is calibrated against it. C5 WS2 says "wire `audit_logs` writers" but does not include the
  block ledger. Fix: one `INSERT` per block state transition inside the existing flush transaction, or
  `audit_logs` rows with `action='ratelimit.block'`.
- **`just abuse-report`** (C3 §5's five queries) as the scheduled recipe.

### 11.6 Other unassigned surfaces (checked)

- **Billing card testing:** `/billing/v1/checkout` is behind `auth_guard::authenticated` (not verified)
  at 60/min/IP (`router.rs`), with 11 env-gated provider registrations (the module is mounted but
  eight of its nine tables are unmigrated — E3). A fresh account can create checkout sessions
  → card testing/chargebacks → processor risk. The webhook receiver is public but verifies signatures
  and dedups by body hash (comments at `controller.rs:34,76`), so the webhook side looks sound. The
  plan has zero billing items. Add a per-account checkout cap + require verified email. Medium.
- **Public search:** `POST /search/v1/search` is public at 30/min/IP over posts (FTS cost). Not in
  scope. Low–medium.
- **Seed system:** the prod image builds `-p ruxlog` without `seed-system` (CI `images.yml:191`), so the
  seed binaries are not in production — low risk. Still add the `seed_runs` migration **before** any
  seed run against production (C3 F6) or provenance becomes permanently unanswerable.
- **TUI:** not in the prod image; local admin tool only. No new risk. The plan's D30 bun TUI/CLI is
  fine, but it inherits the audit gap — wire `audit_logs` for TUI mutations too.
- **FCM fan-out:** no cross-user fan-out found; the FCM `devices` table's ownership-transfer semantics
  are a device-link edge already covered by WS8/WS9. Low.
- **R2 artifact hotlinking:** `QURAN_R2_UPSTREAM_BASE` is a public domain and `/_quran/[...artifact]` is
  an allowlisted same-origin gateway with no limiter (B3 A5). R2 egress is free, so the cost is ops +
  licensing/attribution, not dollars. Fold the `/_quran` limiter into WS7a; do not sign URLs (breaks the
  bulk path). Low.

---

## 12. Corrected workstream ordering (with days)

Phase 0 — truth and availability (no new identity, no FP, unblocks everything):

1. **WS0a** gates restore (web block + rust block for touched crates) + hook fix + doc truth — **1.5–2.5 d**
2. **WS1a** truthful interim policy: D2 analytics consent, D5 reword, D10 newsletter, D11 contact,
   D13 consent integrity, D18 Terms clause — **2–3 d**
3. **WS7a** reader-path cost/reliability: cache restore (O8), internal bucket split,
   `INTERNAL_QURAN_API_BASE` boot error, web-origin limiter + byte budget, `/offline/*` budget + CF
   rule, `/_quran/*` limit, robots/llms/bulk doc — **3–4 d**
4. **Measure** 429 log fix + block ledger + `just abuse-report` — **0.5–1 d**

Phase 1 — enforcement + incident response (ships as one bundle):

5. **WS2a** ban contract: admin API + index + idempotent create + session kill + banned UX + audit
   writer + `enforcement_notices` — **4–6 d**
6. **WS3a** notice + record + email redress + micro/small assessment + **comment-moderation notice
   wiring (§11.1)** — **2.5–3.5 d**
7. **WS5** image moderation + hash interface + `legal_holds` + IR/NCMEC/breach runbook — **3.5–5.5 d**
8. **WS9a** shared-device precedence rules as written operator policy (B5 P1–P6) — **0.5 d**

Phase 2 — abuse reduction where harm lands:

9. **WS4a** limiter correctness + fresh-account privilege limits + comment hardening + Gmail dot-alias +
   registration soft caps + Turnstile deploy check + OAuth door — **6–8.5 d**
10. **WS4b** Turnstile on login/reset **with** the fallback ladder — defer until the ladder exists

Phase 3 — retention, then the full policy:

11. **WS6** sweeper + erasure receipts + backups/runbook + boot invariants — **4–6 d**
12. **WS1b** full policy text (C4 §2) + ar translation — **0.5–1 d**

Phase 4 — device, gated:

13. **Gate B** (§10.3) → **WS8** token (evidence-only) + **WS9b** guards, same release — **7–11 d**
14. **Gate C** (§10.3) → fingerprint tranche WS10–WS14 — **38–61 d**

**True smallest v1 = ≈28–42 dev-days + 1–2 policy days.** The device tranche is not in v1.

---

## 13. Findings (16)

**F1 — WS1 is not the compliance minimum, and the ordering violates C4's own sequencing rule. NEW. Critical.**
Evidence: C5 §8.1 WS1 (policy + LIA + retention rules + DSAR) vs C4 §7 B1–B10 (ten blocking items;
analytics consent, retention job, deletion claim, contact address, content notice, DSAR, minors,
newsletter, consent integrity). C4 §7: "B3's retention job must land before B2's policy is published."
Fix: WS1a truthful interim policy now (2–3 d); WS1b full text after WS6.

**F2 — Twelve sentences in C4's §2 policy are false on the day WS1 ships. NEW. Critical.**
Evidence: §1.2 table — analytics "off unless you turn it on" (default `true`, `consent.svelte.ts:16`),
all four retention cells (`[WS6]`), appeal/notice claims (WS3 later), DPA/TIA "on file" and "we take backups" (neither exists), minors/India.
Fix: publish only the claims the code honours; move the rest behind WS6/WS3/D12/D16.

**F3 — WS2's case state machine + queue caps/aging are over-engineered for a system with zero cases and duplicate C4's `enforcement_notices`. NEW. High.**
Evidence: C3 §3 (zero cases, no automated ban by configuration); D26 (enforcement_cases + case_events) carries
device evidence and the L0–L7 ladder, both deferred; B9 F4 (one operator). Fix: minimum = `user_bans`
API + index + session kill + banned UX + audit row + `enforcement_notices` row; defer the state machine
and queue machinery with the device tranche.

**F4 — What breaks without the case machine: nothing. What breaks without notice/appeal/audit: the silent permanent lockout. NEW. High.**
Evidence: C1 F9/F10/F11; `get_active_ban` has no index (`m000001_init.rs:58-73`) and runs per login and
per 5-min guard refresh; AUTH_006 is in `CREDENTIAL_TYPES`; the limiter runs before the ban check. Fix:
bundle the five-item contract in one release (WS2a + WS3a).

**F5 — WS3's appeal portal is over-scoped: DSA Art 20 is exempt while micro/small and GDPR Art 22 does not attach to human-decided bans. NEW. High.**
Evidence: C4 §4.1 ("Art 20 exempt while micro/small"; "Art 22 — No — nothing automated exists"; "the
duty is not to build it automated"). Fix: notice email + record row + monitored-mailbox redress now
(1.5–2.5 d); build the tokenized page at the first real ban; keep the email-only route as the primary.

**F6 — Adding the enforcement template to the transactional set does not make it deliverable: transactional + suppressed is dropped with `Ok`, and Complaint-vs-Bounce suppression must be distinguished. NEW. High.**
Evidence: C4 §0 line 34 + `mail/router.rs:241-258` (transactional suppressed → `Ok`). Fix: record `delivery_state`; send on Complaint-reason
suppression (CAN-SPAM exempts transactional), skip on Bounce and record `suppressed` (H3 #22); write the
notice row before the send.

**F7 — WS4 ranks the wrong things and guards one door: OAuth account creation bypasses every admission control. NEW. High.**
Evidence: `google_auth_v1/controller.rs:545` (email_verified enforced — good); grep: zero `turnstile`
in the five OAuth modules; only the shared 60/min nest bucket → 86,400 accounts/day/IP/provider. Fix:
rank limiter fix > fresh-account limits > dot-alias > soft caps + deploy check; extend admission to the
OAuth callback; cut hard per-IP/per-domain blocks, ASN tier, invite mode.

**F8 — Turnstile on login is an availability regression: prod fail-closed (503), censored networks, and the API-secret/web-site-key coupling. NEW. Medium-High.**
Evidence: `turnstile.rs:17-19,42-47,91-94`; `web/src/lib/auth/turnstile.ts:4-7`;
`docker-compose.yml:73` (empty default site key, secret in `.env`). Fix: challenge only after N
failures with the fallback ladder (Turnstile → email OTP → delay); deploy smoke test; never on
reading/recovery/appeals.

**F9 — WS6 is ordered after WS2/WS3, but the real constraint is retention-before-policy, not retention-before-bans. NEW. High.**
Evidence: C4 §7 sequencing rule; ban/notice PII at zero cases is negligible vs live per-login UA/IP
volume (C4 D3). Fix: order = WS2a/WS3a (new tables carry `expires_at INTEGER` + `retention_class` at
creation) → WS6 sweeper → WS1b policy; if forced, sweeper first.

**F10 — WS7 is reliability work filed as abuse work at position 8; it is the only workstream addressing a demonstrated availability risk and belongs first. NEW. High.**
Evidence: B3 F2 (any cookie kills both caches; GA4's `_ga` is the default state of every returning reader, E6), F1 (one
global 600/min internal bucket → one IP 429s every translated page site-wide), F3 (14.1 MB offline pack
unlimited = 8.4 GB/min egress), F6 (up to 380 HEADs/request in prod through the shared mail/billing/FCM http client),
F11. Fix: move the non-identity subset to phase 0 (3–4 d); cut ASN tier and harvest-shape score.

**F11 — WS8 is inside the "smallest defensible v1" without the Gate 0 that C1/C3 require; at zero base rate it is a 12-month bearer identifier with no product. NEW. High.**
Evidence: C5 §8.1 order 9 vs C1 §8.2 "Do not run PR5 before Gate 0"; C3 F5 (denominators zero); C1
F6/F8. Fix: remove WS8 from v1; trigger = Gate B (§10.3); keep only the WS9a policy invariants.

**F12 — WS9 cannot precede WS8 as code, but most of WS9 is written policy that must ship with the first ban. NEW. Medium.**
Evidence: C5 §8.1 "Depends on WS8" + token-table references; B5 P1–P6 are rules. Fix: WS9a (0.5 d) in
WS2a/WS3a; WS9b (2–3 d) after WS8.

**F13 — WS0 is legitimate but under-scoped, and the commit-hook fix is a separate change. NEW. Medium.**
Evidence: B7 §8 (gates removed before ever observed green; expect a provisioning pass), B7 F2 (`vp
check` reads root config only), B7 §4 (the new invariants need new guards). Fix: web block + rust block
for touched crates as separate `needs` jobs, hook fix via root `lint.overrides` or a `web/**` staged
command, and write the four new guard tests inside the PRs that create the invariants; do not restore
cargo-audit inline.

**F14 — Missing: the live DSA Art 17 surface (comment hide/delete) is in no workstream, and six C4 blocking items are unassigned. NEW. Critical.**
Evidence: C4 D7 ("already happening") + §4.4; C4 §7 B1 (D2 analytics), B5 (D11 no-MX address), B8
(D12 minors/India s.9(3)), B9 (D10 newsletter), B10 (D13), D15/D16 (backups, DPAs/TIA asserted in the
policy text). Fix: WS3a wires `post_comment_v1`; fold D2/D10/D11/D13/D18 into WS1a; add D12 (1–2 d) and
D16/D15 as named artefacts.

**F15 — Missing: incident response. Uploads and comments are live; there is no 3am runbook, no preservation, no NCMEC/LE path, no breach runbook. NEW. High.**
Evidence: `media_v1 /create` behind verified ROLE_AUTHOR (uploads live); no image moderation
(`IMAGE_MODERATION_ENABLED=false` → `image_moderator = None`); no `legal_holds`; WS5 is conditional and depends on WS1/WS2; C3 §6.4 calls the ban button
an incident-response gap. Fix: make WS5 mandatory, add the 0.5–1 d runbook (hide → preserve → report →
ban → notice) + breach SOP.

**F16 — Missing: the measurement one-liners every gate depends on, plus the newsletter 500 and billing card-testing. NEW. High/Medium.**
Evidence: `layer.rs:323` `debug!` at `RUST_LOG=info` → per-route 429s invisible (C3 F4);
`rate_limit_state` deletes expired rows with no history (C3 F1); `newsletter_subscribers` has no
migration while `POST /newsletter/v1/subscribe` is mounted (C3 F8); `/billing/v1/checkout` is
authenticated-only at 60/min/IP (billing mounted, non-functional — E3). Fix: promote the 429 log to `warn!`; add the
block ledger inside the existing flush transaction; `just abuse-report`; newsletter migration or unmount;
per-account checkout cap + verified-email requirement.

_End of D2. Reports read: C5, C1, C3, C4, B2, B3, B7, B9 (partial: B9 findings index + F12 context).
Repo re-verified at HEAD this session: `m000001_init.rs:58-73` (user_bans columns incl. `created_at`,
no index), `router.rs` nest limits (media/search 30, newsletter 100, billing 60),
`billing_v1/mod.rs` (checkout = authenticated; webhook public + body cap),
`media_v1/mod.rs` (create behind verified ROLE_AUTHOR), `google_auth_v1/controller.rs:545`,
`github_auth_v1/controller.rs:540-582,644-656`, `docker-compose.yml:73,129`, mail
`provider.rs:17-22` / `router.rs:241-258`, `.github/workflows/images.yml:191` (no seed-system feature)._
