# G4 — Pre-mortem: the plan, twelve months out

Auditor: pre-mortem subagent. Date: 2026-10-12. Repo read-only.
Inputs: `C5-decision-ledger.md` (WS0–WS9 + do-not-build), `D2-v1-attack.md` (corrected ordering + Gate B),
`C3-base-rate.md` (no measured abuse base rate), `B10-falsification.md` (the steelman), `E5-fresh-eyes.md`
(the independent first change), `F1-ingress-trust.md` (the ingress finding), `F6-remaining-gaps.md`
(the exhaustion verdict). **All findings in this report are NEW.**

Working assumption for the fiction: the plan is approved 2026-10-20 and starts 2026-11-01, so
"six months in" = April 2027 and "twelve months out" = October 2027. Nothing below assumes the plan is
bad; it assumes it is executed by one person, as written, with the evidence the audits already produced.

---

## 0. Verdict up front

The direction survives a pre-mortem. The shape does not. The five most probable futures are:

1. **It is never started** — its first item (restore CI gates) is a re-validation project on a repo
   where the owner deliberately removed those gates, it has no timebox and no fallback, and it gates
   everything else.
2. **It is finished and moves no number** — the plan's instrument (blocks, bans, flags, suppressions)
   reads zero because the app's abuse is not account abuse; meanwhile the abuse it does have arrives
   through the one door the plan does not guard (the OAuth callback).
3. **Its first ban wave produces a public false-positive story** — the ban contract ships without the
   shared-device rules (they sit in the gated Phase 4), without notice-delivery honesty, and with one
   operator.
4. **Its first erasure request becomes a DPA complaint** — the retention job ships silently broken, the
   policy makes claims the code cannot honour, and no one owns the LIA/ROPA.
5. **Its own new reader ceilings break the reader in the app's peak week** — set from numbers nobody
   measured, shipped into Ramadan, with no shared-IP allowance.

Two structural defects run through all five:

- **The plan omits the ingress fix (F1) and ships a new IP-keyed reader limiter before it.** On a
  forgeable `CF-Connecting-IP` that converts a weakness into a weapon (F1 §10: "shipping them on a
  forgeable identity converts weaknesses into weapons"). The web limiter must share the marker gate
  (F1 §8) and the marker gate must land first.
- **The plan is large where the evidence is absent and small where the app is actually exposed.** The
  attack surface (ingress, auth-work saturation, mail, storage growth, media takedown, the OAuth door)
  is not in any workstream; the identity/enforcement machinery is 28–42 days of it.

---

## 1. Five failure stories

### 1.1 (a) The plan was never implemented — "the plan died of first-item friction"

**Six months in (2027-04-12), the plan was never implemented because its first item was the
highest-friction, lowest-value, deliberately-removed thing in the repo, and it gated everything
behind itself.**

The dated version:

- **2026-11-02.** First PR = WS0 (restore the web + Rust gates in `images.yml`). The plan's own evidence
  says this is not a revert but a re-validation project: the gates were removed in `53d3d421` **before
  ever being observed green in CI** (`bed80b13` → fixed `37d90c41` → removed `53d3d421`; B7 §8), so
  "restore" means a fresh-runner provisioning pass with unknown length. The plan budgets 1.5–2.5 days
  and gives it no timebox and no fallback.
- **2026-11-04 → 2026-11-09.** Two provisioning failures on a fresh runner, same class as the one that
  produced the removal. The attempt is reverted 2026-11-10 in a `fix(ci)` commit and never retried.
  The owner had removed the gates _deliberately_; the plan silently overrode that decision without the
  owner signing up for the pipeline work.
- **Because WS2/WS3 are ordered after WS0**, the ban contract — the item that makes the app able to
  respond at all — never starts. Because WS7a (reader cost work) sits later in "Phase 0", the disk
  cache stays off for every cookie-holder, and the translated reader stays the slowest page in the app.
- **The C3 §5 measurement is never run.** It needed host access (a prod DB copy) and 2–3 hours the
  first time; the plan schedules neither. Six months on, the base rate is still unmeasured, so every later decision
  is still calibrated blind — the exact condition C3 was written to end.
- **2027-04-12.** The plan's ledger: two landed commits (the 429 `debug!`→`warn!` word, the newsletter
  unmount/`documentation/history.md` correction) out of 28–42 dev-days. The app still cannot ban anyone; the
  login limiter still locks out any account with three requests; the reader still bypasses the disk
  cache for every cookie-holder; the newsletter table still does not exist.

**Early warning signal.** By day 14: the open-question register still contains ≥4 implementation-blocking
items (O8 cache mechanism, O13 artefact owner, O1 India, O17 erasure), and the first PR is WS0. By day
30: `git log --since=30.days -- .github/workflows/images.yml` is empty, and the C3 §5 measurement has
not been run. Both are visible without reading a single line of new code.

**Plan change.** (1) The first ten days become the **truth bundle**, which needs no CI, no legal
capacity, and no new control: 429→`warn!`, the newsletter migration or unmount, the four doc-truth
edits, and the C3 §5 measurement run with its result written into the plan as a dated sentence.
(2) **WS0 is timeboxed to two days** with a declared fallback (web-only gate block first; the Rust
block for touched crates only; `cargo-audit` stays out of the inline path), and **WS0 does not gate
WS1a/WS7a/WS2a**. (3) O8/O13/O1/O17 are decided in the plan text before it is written. (4) The plan
lists its first five PRs by name.

### 1.2 (b) It was implemented and abuse did not decrease — "the password door was guarded; the wave came through the OAuth door"

**Six months in (2027-03-14 → 2027-04-12), abuse did not decrease because the plan's registration
admission guarded one door (password + Turnstile), the abuse used another (Google OAuth, no cap at
all), and the plan's own instrument could not see the abuse the app actually had.**

The dated version:

- **2026-12 → 2027-02.** Phase 0 (truth bundle + reader cost work), Phase 1 (ban contract, notice,
  IR runbook), Phase 2 (limiter redesign, registration soft caps, fresh-account limits, dot-alias,
  comment hardening) all ship. Gates restored after a 5-day provisioning fight. The app can now ban,
  notice, and appeal.
- **2027-03-14, 18:40 UTC.** First account-abuse wave: **380 accounts in 90 minutes**, all created via
  the Google OAuth callback, all posting the same link. D2 F7, verified: **zero `turnstile` hits in the
  five OAuth modules**; the only limit is the shared 60/min nest bucket — 86,400 accounts/day/IP/provider.
  The plan's registration admission controls cover the password door; they do not exist on the callback.
- **The fresh-account limits work as designed** (1 comment/24h, no links, <24h) — the wave produces ~190
  comments instead of 3,800, and the operator bans all 380 accounts in two evenings. But the next wave
  comes **2027-03-21**, and again **2027-04-02**. Registration is still free; the accounts are the
  product.
- **The report recipe reads zeros** in every abuse column (bans were made but the wave's accounts were
  banned _after_ the fact, not prevented). The only non-zero is 429 warn lines: 1.1M/month on
  `quran-v1`, from 41 IPs. Cloudflare's per-IP/ASN distribution puts the same 41 IPs on top; March 2027 egress is
  2.9 TB against a 0.4 TB baseline. That is bulk reading of a public text — measured, visible, and
  deliberately not defended: the harvest-shape score was cut, the ASN cost tier was cut, and the reader
  path has no identity by design.
- **2027-04-05.** The operator adds the OAuth cap — three days of work the plan did not include.

**Early warning signal.** At plan review, `rg -l 'turnstile|admission' rust/backend/api/src/modules/*_auth_v1/`
returns nothing, and the `just abuse-report` recipe has no reader-path / per-IP / ASN column.
The first wave's provider is the one whose module has no cap.

**Plan change.** (1) Extend the admission rule to all five OAuth callbacks (per-IP/day + a
per-verification-identity cap; no Turnstile — it breaks the redirect flow) **or** state the residual in
the plan as a named risk. (2) Add the reader-path columns (429 counts by route and identity class + the Cloudflare per-IP/ASN
distribution, D3 §2) to the report recipe so "abuse did not decrease" is a visible
number, not an opinion. (3) Name the app's real threat list (bulk reading, takedown, review-bombing;
R2-evasion §8.1–8.2) and mark which workstream addresses which — the plan currently defends only the
account axis.

### 1.3 (c) A false-positive incident harmed real users — "the first ban wave banned a family"

**Six months in (2027-03-20), the ban contract produced its first false-positive incident because it
shipped without the shared-device/shared-IP rules (they are in the gated Phase 4), without
notice-delivery honesty, and with one operator and no second corroborator.**

The dated version:

- **2027-03-19, 21:10 UTC.** Eleven accounts post the same link in 40 minutes.
- **2027-03-20.** The operator bans all eleven via the new `/admin/users/bans` API. Two are not
  spammers: a father and son on one shared phone and one household IP; the father had posted one
  unrelated comment in the same thread. D2 F12 is the cause: most of WS9 is **written policy that must
  ship with the first ban** (0.5 d), but the plan put shared-device guards in Phase 4, gated behind
  Gate B — so at the moment of the first ban, the SOP has no shared-device line and the ban form has no
  corroborator field.
- **Sessions die correctly** (per-user `session_auth_secret` rotation; that part of the contract is
  right). The banned UX shows the appeal link. **The notice emails never arrive**: both addresses are on
  `email_suppression` with reason `Complaint` (a 2025 newsletter), and transactional + suppressed
  returns `Ok` (`mail/router.rs:241-258`) — so the `enforcement_notices` rows record `sent` for mail
  that never left. The users see the ban only when they next try to log in; the in-app channel is
  unreachable for a killed session.
- **2027-03-31.** The father appeals after 11 days (the mailbox is read on weekends). The appeal is
  upheld; the ban is revoked. The other user never appeals and posts publicly that the app "bans
  families".
- **FP rate on this wave: 2/11 = 18%.** The plan's stated target is <0.1% (A8 — a sourced _target_, not
  an empirical rate); at N=11 it is unmeasurable and was never going to be met.

**Early warning signal.** At review: the ban rationale field has no required corroborator, the ban SOP
has no shared-device/shared-IP line, the notice path has no Complaint/Bounce distinction
(`mail/router.rs` unchanged), and the appeal mailbox has no owner and no response clock. All four are
visible before the first ban exists.

**Plan change.** (1) Bundle WS9a's four rules into the ban release: device is never a ban unit; a
shared-device self-claim never clears an account ban; device friction never gates login/reading/
recovery/appeals; a second independent corroborator is required before enforcement. (2) Require a
`corroborator ∈ {credential_reuse, content_match, ip, none}` field on every ban. (3) Fix notice delivery in the same PR: write the notice row **before** the send; send despite a
Complaint suppression for the enforcement template only (E4 verdict item 3); on Bounce record
`delivery_state='suppressed'` (H3 #22); add `TEMPLATE_ENFORCEMENT_NOTICE` to `is_transactional`. (4) Give the appeal a response clock (7 days) and a queue-aging alert.

### 1.4 (d) A privacy/legal incident — "the erasure that could not be honoured"

**Six months in (2027-04-08), a privacy incident became a DPA complaint because the retention job
shipped silently broken, the policy published claims the code could not honour, and the LIA/ROPA had
no owner.**

The dated version:

- **2027-02-06.** A German user (banned correctly in January) asks by email for erasure of his data.
- **2027-03-02.** The operator runs the self-service delete + admin erase and writes the erasure
  receipt.
- **2027-04-08.** The user asks for confirmation. The operator finds:
  - `user_sessions` (raw UA + IP) and `email_suppression` rows are still present. The Phase 3
    sweeper's first run deleted **zero** rows: its `expires_at` comparison was written against mixed
    TEXT/INTEGER columns, and a TEXT-vs-TEXT or INTEGER-vs-TEXT mismatch deletes nothing and logs
    nothing (B8 F7; D2 §9.3's "fails silently" list). The boot invariant that would have caught it was
    written as a _test_ and skipped, because tests are optional and ship gates are not.
  - Deletion is not physical erasure: no `PRAGMA secure_delete`, no checkpoint, so the row bytes remain
    in the SQLite file and its WAL (B8 F2). O17 (physical-erasure strategy) was never decided.
  - The published policy says "deleted within 30 days" — one of the 12 sentences D2 §1.2 predicted
    would be false on the day the policy shipped.
  - The Phase 1 `appeals` free text (a user explaining his religious context — special-category
    material) has no `expires_at`/`retention_class`, because the plan specified retention columns for
    Phase 3 tables only. D2 §5: the new tables must carry retention at creation, "otherwise WS2/WS3
    create the next 'forever' class".
- **2027-04-20.** The user complains to the Berlin DPA. The DPA asks for the LIA and the ROPA. Neither
  exists (B10 §4.5: no legal capacity; O13 was never assigned an owner).

**Early warning signal.** At review: the `enforcement_notices`/`appeals` DDL has no `expires_at` or
`retention_class`; the sweeper's dry run deletes 0 rows while rows are past expiry and no one compares
the count to the expected; the erasure sentence ships before the backup job exists and before O17 is
decided. Every one of these is visible in the plan text and the migration diff.

**Plan change.** (1) Every new table carries `expires_at INTEGER NOT NULL` + `CHECK` +
`retention_class` at creation and is registered in the sweeper config **in the same PR**. (2) The
sweeper ships with a boot invariant as a **ship gate**: a canary row past its expiry must be gone after
boot; a run that deletes 0 rows while rows are past expiry alerts; the dry-run count is checked before
deletes are enabled. (3) The full policy text ships **only after the sweeper's boot invariant is
green** (C4's one sequencing rule). (4) The erasure claim wording is "removed from the live database;
backups within 30 days" only after a backup job exists; otherwise the honest claim is used. (5) O17 is
decided. (6) Either an owner is named for the LIA/ROPA or the policy stops claiming them.

### 1.5 (e) The app's availability/reader experience broke — "Ramadan 2027"

**Six months in (2027-03-05), the reader broke in the app's peak week because the new reader-path
ceilings were calibrated blind, the change shipped into the pre-Ramadan window, and the limiter had no
shared-IP allowance or logged-in exemption.**

The dated version:

- **2026-12.** Phase 0 reader work ships: web-origin per-IP limiter (240/min reader pages, 600/min
  static/md), internal bucket split by route class, disk-cache restoration, offline-pack byte budget.
  The ceilings are derived from the plan's own numbers. The repo has no traffic figures (C5 §7: A7's
  25k MAU is a **stated assumption**; every threshold in the plan is a "v1 default to be recalibrated");
  no baseline is taken before the ceilings are committed, and no load test is run (there is no staging
  environment — constraint 35).
- **2027-02-18 → 2027-03-19.** Ramadan, the app's annual peak.
- **2027-03-05, 22:00 UTC (Friday night).** The translated-reader class saturates. Every translated
  page degrades to an empty shell for **41 minutes** (E5 A2's mechanism:
  `reader-route-guards.ts:39-45` marks it pending/no-store). Arabic (prerendered) pages stay up, which
  makes the failure look like a content bug, not a limiter. The operator learns from a Telegram message
  three hours later.
- **Same week, 09:00 local.** A madrasa lab in Kerala on one NAT'd connection (~35 students) hits the
  240/min reader-page ceiling; students get 429s on translated pages. There is no shared-IP step-down
  and no logged-in exemption: the G01–G07 population guards are Phase 4 device-tranche work.
- Neither failure needs an attacker. Both are the plan's own new controls, set blind, on the
  highest-traffic week of the year.

**Early warning signal.** The first ceiling is committed with no baseline traffic number in the plan;
the limiter has no shared-IP allowance and no logged-in exemption at review; the change window falls
inside the six weeks before Ramadan; there is no synthetic check that a translated page rendered.

**Plan change.** (1) Run the C3 §5 measurement **before any ceiling is set**, and set ceilings at ≥4×
the measured peak. (2) Exempt logged-in readers; add a shared-IP step-down. (3) The reader limiter must
share the F1 ingress marker gate, and F1 must land first (F1 §8/§10). (4) Load-test the reader path at
3× the measured peak before the split ships. (5) Ship reader-path changes outside the six weeks before
Ramadan. (6) Add a synthetic translated-page check with an alert on the empty-shell path.

---

## 2. Early warning signals and plan changes (compact)

| Story                       | Early warning signal (observable before the failure)                                                                                                                                  | Plan change that prevents it                                                                                                                                                                                            |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| (a) Never implemented       | Day 14: ≥4 blocking open questions (O8/O13/O1/O17) unresolved and PR#1 = WS0. Day 30: no commit to `images.yml`; the C3 §5 measurement not run.                                       | First 10 days = truth bundle + measurement (no CI, no legal capacity needed); WS0 timeboxed 2 d with a web-only fallback and no gating of WS1a/WS7a/WS2a; O8/O13/O1/O17 decided in the plan text; first five PRs named. |
| (b) Abuse did not decrease  | At review: zero admission checks in the five OAuth modules; the report recipe has no reader/per-IP/ASN column; the plan names no scraping/takedown/review-bombing item.               | Extend admission to all OAuth callbacks or state the residual; add reader-path + per-IP/ASN columns to the recipe; name the real threat list and map workstreams to it.                                                 |
| (c) False-positive ban wave | At review: no shared-device line in the ban SOP; no required corroborator field; no Complaint/Bounce distinction in the notice path; no appeal clock/owner.                           | Bundle WS9a's four rules + corroborator field into the ban release; fix notice delivery (row before send; Complaint sends for the enforcement template only; Bounce → `suppressed`); appeal clock 7 d + aging alert.    |
| (d) Privacy/legal incident  | New tables lack `expires_at`/`retention_class`; sweeper dry-run deletes 0 rows past expiry with no count check; erasure claim ships before the backup job and O17; no artefact owner. | Retention columns at creation + sweeper config in the same PR; boot invariant as a ship gate; policy only after sweeper green; O17 decided; LIA/ROPA owner named or the claim removed.                                  |
| (e) Reader breaks at peak   | No baseline number when the first ceiling is committed; no shared-IP step-down or logged-in exemption; change window before Ramadan; no synthetic reader check.                       | Measure first; ceilings ≥4× measured peak; logged-in exemption + shared-IP step-down; F1 marker gate first; 3× load test; ship outside the pre-Ramadan window; synthetic check + alert.                                 |

---

## 3. The "plan is too big" failure mode — the 1/10th case

**The strongest case.** The plan is 28–42 dev-days plus 1–2 policy days (D2 §9.2) on a repo where:
the measured abuse base rate is zero, not low — zero active bans, zero escalation history, zero comment
flags, zero mail suppressions, zero audit rows, and three of six abuse classes are **structurally
invisible** (C3 §3); the owner has deliberately removed the CI gates; there is one operator, no staging
environment, and no legal capacity; and the deployment is days old, so the observation window that
produced "zero" is itself short (C3 §3 caveat 2). The plan's own audits rank its items and the ranking
does not match the plan's size: per-credential rate limits are the highest impact-per-effort
countermeasure and need no fingerprint (A8 §9, R2-evasion §10, B10 §7); the only abuse work justified
at zero base rate is closing the three blind spots plus the ban button, because "without them the next
measurement is equally blind" (C3 §6 point 3–4); the reader-path work is mostly **reliability**, not
abuse (D2 §6.1), and its biggest item is a product-wide perf fix. Everything else in the plan is
contingent on things that do not exist: cases (WS2's case machine, queue caps, ladder), volume (WS6
acts on a live table that is currently 42 rows in dev), legal capacity (WS1b), or a measured target
(WS4's registration numbers are proposals — O18). The plan spends its budget on machinery for a
population that has not been observed, while the app cannot yet do the two things that are justified
today: **respond** (no account can be banned) and **see** (429s are `debug!`; `audit_logs` has zero
writers).

**The 1/10th version.** The single workstream that matters most is the **enforcement-and-notice
contract**: the five-item ban contract (`/admin/users/bans` list/create/revoke with reason + expiry +
`banned_by`; the partial index; idempotent create; per-user `session_auth_secret` session kill; the banned UX with an explicit `banned` classifier branch for `AUTH_006`/`AUTH_BANNED` ahead of the
credential-status fallback (E4 verdict item 2) and the ban check before the limiter) **plus**
the audit writer **plus** the enforcement notice + email redress (`enforcement_notices` row before the
send, Complaint/Bounce delivery honesty, `NotificationKind::Enforcement`) **plus** the comment-hide/
delete notice wiring. ≈5–6 dev-days, ~1/5–1/6 of 28–42; the honest floor is not 1/10th because the
legal minimum cannot be cut without losing the point (drop the appeal page and the notice and you have
no redress, which is the Art 17/Art 22(3) core). Add the two one-day blind-spot fixes (429→`warn!`,
`just abuse-report`) and the C3 §5 measurement run as passengers. **Defer everything else behind the
first 90 days of measured data.** The strongest alternative — the login-limiter redesign (2–3 d), because
the current throttle is itself the most effective abuse tool in the app (E5 #4) — is the one item that
removes an active weapon; if only one item can ship, the ban contract wins only because the app has a
**live** Art 17 surface (`post_comment_v1` hide/delete is silent today) and the limiter is a bounded
residual once the plan states it. If the 1/10th must be literal (~3 days), it is: 429→`warn!` +
`just abuse-report` + the C3 §5 measurement + the limiter redesign — the blindness and the weapon — and
nothing else until the number exists.

---

## 4. The "plan is too small" failure mode — what it under-protects

**The strongest case.** The plan is large in new identity/enforcement machinery and small in the app's
**existing attack surface**. Six surfaces where a single unauthenticated or freshly-registered caller
can cause an outage or a legal exposure are in no workstream:

1. **Ingress forgery (F1, Critical).** Every per-IP rate limit, every IP ban, the escalation engine, the
   Turnstile `remoteip`, and the future device token's IP evidence are keyed on `CF-Connecting-IP`,
   believed from any peer inside `TRUSTED_PROXY_CIDRS` — whose default is every private range, i.e.
   every container on the shared Dokploy network (F1 F1-1). Nothing sets or strips the header in any
   layer (F1 F2). An attacker who knows the origin IP can forge identity, pre-fill a victim's bucket
   (targeted 429s), and — once escalation or IP bans are live — direct a durable ban at a third party
   (F1 §2). The plan ships a **new** IP-keyed web limiter (Phase 0) and an IP ban path (Phase 1) on top
   of this, and F1 §10 says the fix must be first and the web limiter must share the marker gate.
2. **Auth-work saturation (E5 #7, A1).** `PASSWORD_WORK` is a 2-slot global fail-fast semaphore; any
   unauthenticated caller can saturate it and make **every** login, registration, reauth and backup-code
   check return 429. Complete auth DoS with no account required.
3. **Mail bombing (E5 #6/A5, B3).** Registration sends a verification mail to an attacker-chosen address
   at up to 100/min/IP (Turnstile-gated in prod) with a per-recipient bucket (≤29/h) but no global budget —
   the provider-wide bucket is skipped for transactional mail (E2 §2); the newsletter path shows the
   intended pattern and this path lacks it. Burning the provider's reputation turns into a silent auth outage
   (`deploy/README.md:463-506` already warns this).
4. **Account-existence oracle + missing nest limiters (E5 #5/B2).** `forgot_password/v1/request` returns
   `RecordNotFound` for unknown emails by design, and the `forgot_password`/`email_verification` nests
   have no `rate_limit_layer`. It is the input for the lockout weapon and a privacy leak.
5. **Anonymous session inflation (E5 A3).** `POST /csrf/v1/generate` inserts a `sessions` row for every
   cookie-less call; unbounded growth on the single SQLite volume → disk exhaustion → API crashloop.
6. **Bookmark sync growth (F6 §2.2).** No per-user folder cap and an unbounded snapshot: 200 folders ×
   100 req/min = 1.2M folders/hour from one verified account, then ~100 MB+ JSON responses built in
   memory on a pool-of-1 connection. A self-inflicted OOM/availability kill.

Plus three that are in-scope-but-incomplete:

7. **Media variants survive deletion (F6 §2.1).** `media_v1::delete` removes the original object only;
   `@480w/@768w/@1024w/@1600w/@2048w/@lqip` variants stay public. The plan's incident-response
   capability promises takedown; for images, the takedown is incomplete by construction.
8. **The OAuth door (D2 F7).** Registration admission that guards only the password door is defeated by
   "Continue with Google" (86,400 accounts/day/IP/provider).
9. **Minors and the newsletter (C4 D12/B9, C3 F8).** No minors posture (India DPDP s.9(3) is a hard
   prohibition), and a mounted public newsletter endpoint whose table does not exist (a 500 surface and
   a false sense of coverage).

**What to add.** The ingress fix as Phase 0 item 0 (Tunnel or per-zone AOP + the marker boot gate +
runtime header overwrite + `tests/ingress_trust.rs` + the non-CF canary), then the six one-day items as
a named "existing attack surface" bundle in Phase 0/2, then the OAuth cap, the media-variant deletion,
the minors posture, and the newsletter migration/unmount. Nothing here is identity machinery; all of it
is the app defending what it already ships.

---

## 5. The governance failure — what rots first, and the minimum that survives

**What rots first: the quarterly re-measurement.** It is the only recurring item whose omission produces
no symptom — no one is harmed by an unrun report, and the plan's other recurring items (the IR drill,
the legal artefacts) at least fail loudly when an incident arrives. With one person, "quarterly" becomes
"when there is time", and there is never time. Second: **the IR drill** — a drill needs a second human
or a committed artefact, and with one person the first skipped drill is invisible until the first real
incident. Third: **the LIA/ROPA** — if Phase 4 never fires (the likely case), they are never needed; the
failure is _claiming_ them in a published policy, which is a lie with a date on it. Fourth: **the rule
maintenance** — it never gets a chance to rot because Gate B likely never fires; if it does fire, the
thresholds go stale fastest, and C3's `ABUSE_RULES_REVIEWED_AT` alarm is the only thing that notices.

**The minimum governance that survives one person.** Convert every recurring intention into a committed
artefact plus a machine alarm, and let the alarm be the governance:

1. `app_constants` rows — `ABUSE_REPORT_RUN_AT`, `IR_DRILL_AT`, `LEGAL_DEBT_REVIEW_AT`,
   `RULES_REVIEWED_AT` — with a boot-time `warn!` when stale and one line on
   `/dashboard/summary` (the existing surface; zero new tables, C3 §4 C3).
2. `just abuse-report` output **committed** to `docs/abuse/YYYY-QN.md` within 10 days of quarter end.
   "Did we measure" becomes a git question, not a memory question.
3. `docs/legal-debt.md` — a committed register where each item carries its trigger condition ("LIA for
   account-level abuse prevention: active now"; "DPIA/ROPA: activates only if the device tranche
   ships"), reviewed at the report cadence. If a claim cannot be backed, the policy must not make it.
4. `docs/ir-drill.md` — the runbook as a checklist with dates; one walkthrough per year, solo if no
   second human exists, with the response SLA written as best-effort and the mailbox owner named.
5. One custody document (O15): who holds the pepper, who answers DSARs, who owns the ban ledger, and
   the recovery steps — so the app is answerable if the operator is unavailable.
6. The Phase 4 lock: no device rules without the C3 C3 commitment (≥1 engineer-day/week) recorded in
   `app_constants` with its staleness alarm. If the commitment cannot be made, the rules do not ship.

---

## 6. The falsification test

**WRONG (within 90 days):** _Within 90 days of the ban contract shipping, the operator confirms two or
more ban-evasion cases whose returning accounts share no credential, no verification identity
(including dot-alias), no content signature, and no actionable IP/ASN with the banned account — the
plan's own Gate B condition met repeatedly inside the window the deferral assumed would produce zero,
which means the missing device identifier was the bottleneck and the account/credential-first bet was
wrong._

**RIGHT (within 90 days):** _Within 90 days, the operator records zero cases that needed a device
corroborator — either no evasion occurred, or every confirmed evasion was linked to the banned account
by credential reuse, verification-identity reuse, or an identical violating content signature, and the
per-credential limits did not block a single legitimate user._

---

## 7. Verdict — concrete edits before the plan is written

1. **Add Phase 0 item 0: the ingress trust fix (F1 §6).** Tunnel or per-zone AOP; `INGRESS_TRUST_MODE`,
   `INGRESS_EDGE_MARKER`, `INGRESS_ALLOWED_PEER_CIDRS` boot gate; marker check + header overwrite in
   `resolve_client_ip`; `tests/ingress_trust.rs`; the non-CF canary. No new IP-keyed control ships
   before it, and the web limiter shares the marker gate.
2. **Replace the phase order with D2 §12 + F1 §10.** Phase 0: truth bundle (429→`warn!`, newsletter
   migration/unmount, four doc-truth edits, the C3 §5 measurement + written base-rate sentence) → WS7a
   reader cost work (ceilings set from the measurement) → WS0a gates (timeboxed 2 d, web block first,
   web-only fallback). Phase 1: WS2a + WS3a + WS9a + WS5 + the IR runbook as **one release**. Phase 2:
   WS4a + the existing-attack-surface bundle (§4). Phase 3: WS6 → WS1b. Then Gate B → Phase 4.
3. **Split WS1**: WS1a interim truth policy that makes only claims the code honours, carrying D2 §1.2's
   claim ledger with the code item that makes each sentence true; WS1b full text only after the
   sweeper's boot invariant is green.
4. **Cut from WS2**: `enforcement_cases`/`case_events`, queue caps/aging/tiering, the L0–L7 ladder. Use
   `enforcement_notices` as the case record. Keep the five contract items, the admin API, the partial
   index, idempotent create, and the per-user `session_auth_secret` kill (stated explicitly, not
   implied).
5. **Bundle WS9a (0.5 d) into the ban release**: the four rules, a shared-device/shared-IP line in the
   ban SOP, and a required `corroborator` field on every ban.
6. **Fix notice delivery in the same PR**: notice row before send; send despite a Complaint suppression
   for the enforcement template only (E4); Bounce → `delivery_state='suppressed'` (H3 #22);
   `TEMPLATE_ENFORCEMENT_NOTICE` added to `is_transactional`; appeal response clock (7 d) + queue-aging alert.
7. **Extend registration admission to all five OAuth callbacks** (per-IP/day + per-verification-identity
   cap; no Turnstile) or state the residual as a named risk in the plan.
8. **Every new table carries `expires_at INTEGER NOT NULL` + CHECK + `retention_class` at creation** and
   is registered in the sweeper config in the same PR.
9. **The sweeper ships with a boot invariant as a ship gate**: a canary row past its expiry is gone
   after boot; a 0-row run while rows are past expiry alerts; the dry-run count is checked before
   deletes are enabled; the sweeper runs off the flush task.
10. **Reader-path ceilings**: run the C3 §5 measurement first; set ceilings at ≥4× measured peak; exempt
    logged-in readers; add a shared-IP step-down; load-test 3× peak; ship outside the six weeks before
    Ramadan; add a synthetic translated-page check with an alert.
11. **Add the existing-attack-surface bundle to Phase 0/2**: password-work queueing + reserved capacity;
    `forgot_password` uniform response + nest limiters; mail per-recipient/per-IP/global budget +
    auth-mail priority; CSRF anonymous-session cap; bookmark folder cap + snapshot pagination; media
    variant deletion on takedown; minors posture (D12); newsletter.
12. **Write the base-rate sentence (C3 §6) into the plan verbatim, dated, with the reopen condition**,
    and add the two falsification sentences (§6) as the plan's kill/keep criterion.
13. **Resolve in the plan text**: O1 (India), O8 (cache mechanism), O13 (artefact owner), O17 (erasure).
    Move the device-tranche O-items (O2/O4/O5/O7/O9/O10/O12) into the Phase 4 annex, out of the v1
    section.
14. **Add the governance minimum (§5)**: `app_constants` staleness rows + boot warn + dashboard line;
    `just abuse-report` output committed to `docs/abuse/`; `docs/legal-debt.md` with trigger
    conditions; `docs/ir-drill.md`; the custody document (O15).
15. **Add the KPI section (§8) with the three thresholds and a monthly read**, and restate the
    do-not-build list so Phase 4 cannot creep.

---

## 8. Judging criteria in six months (three, with numeric thresholds)

1. **It can respond, end to end.** By 2027-04-12: ≥1 real enforcement case has gone ban → session kill →
   `enforcement_notices` row with a truthful `delivery_state` → appeal → decision, with 100% of the
   required rows present (ban, audit, notice, decision); and 100% of comment hide/delete actions since
   the wiring shipped have a notice row. **Threshold: 1 case, 100% of rows, 0 unwired hide/delete
   actions.**
2. **The base rate is measured, not assumed.** **Threshold: 2/2 quarterly `abuse-report` runs committed
   to `docs/abuse/` (Q1 + Q2 2027), each within 10 days of quarter end, each containing the C3 §5 five
   counters + the Cloudflare per-IP/ASN distribution; the 429 `warn!` line verified live; the plan's
   base-rate sentence updated with the dated numbers.**
3. **The controls did not harm the audience or the reader.** **Threshold: 0 confirmed false-positive
   enforcement actions (0 overturned appeals where the ban's recorded corroborator was `none`); 0
   reader-path 429s on logged-in users; 0 translated-page degradation-to-empty-shell incidents; 0 merges
   to master without the required CI gates in the last 30 days.**

---

## 9. Honesty notes

- The failure stories assume the plan is executed **as summarised**: no ingress fix, no OAuth door, no
  attack-surface bundle, WS9a in Phase 4, notice delivery unfixed, ceilings set from assumptions. If
  §7's edits land, the stories are the residual risks, not the expected futures.
- The dates are illustrative; the mechanisms are the deliverable. Every mechanism cited is a `file:line`
  or a finding in the inputs listed at the top.
- This pre-mortem does not re-open the direction (account/credential-first, fingerprint deferred). It
  attacks the plan's **shape**: the first item, the ordering, the unguarded doors, the unmeasured
  ceilings, and the ownership of the recurring work.
- At zero measured base rate, the correct posture is not "build less" or "build more" — it is **measure
  first, then let the number choose**. The plan currently chooses, then measures.

_End of G4._
