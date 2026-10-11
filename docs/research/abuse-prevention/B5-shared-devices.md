# B5 — Shared-device positive design (family phones, madrasas, schools, cafés, libraries, kiosks)

Round 4, Q5 of DESIGN-v1. This document is the _solution_ to the risk that `A2-false-positives.md`
established (findings 3, 4, 11), constrained by `A8-ops-realism.md` (findings 1, 13) and the
operating doctrine in `R2-ban-operations.md` (multi-account legitimate uses, the Stack Exchange
functional test, Wikipedia IP block exemption). It is written to be implemented on top of
DESIGN-v1's primitives, not to replace them.

**Hard constraints carried in from the repo (non-negotiable):**

- Quran DBs are immutable; no SHA-256 over Quran data in any automated path. Everything here
  concerns abuse/device evidence only and must never enter the catalogue, ETag, or `/health` path.
- `pool = 1` SQLite connection. **Zero DB operations on the request path** (DESIGN-v1 B3). Any
  shared-device policy read must come from an in-process, boot-hydrated map with a refresh ticker
  (the `route_blocker_service.rs` precedent: snapshot + TTL + eager refresh on mutation).
- `auth_guard::authenticated()` / `verified()` already include `not_banned()`; `check_ban` is the
  only account-ban enforcement point (`services/auth.rs:530`, `auth_guard.rs:31/66/84`,
  `rux-auth/src/middleware/guard.rs:46`). Nothing in this design may add a second ban path.
- No nested ternaries; `web/` gates (`check`, `lint`, `test`) stay green.
- The device axis may **challenge**; it may never be the sole cause of a ban (A2 F1/F2/F4,
  DESIGN-v1 B1). This document does not weaken that; it makes it operational for shared hardware.

**One-line thesis.** A shared device is not a suspect; it is a _place_. The account is the only
enforcement subject, the device is only evidence, and the two shared-device mechanisms below
(self-claim, org kiosk) change **which friction the device causes** — never **which ban an account
carries**.

---

## 1. Account-level trust model and enforcement precedence

### 1.1 What is trusted, and at which layer

| Layer                                           | Trust                                                     | Scope           | May cause enforcement?                                              | Storage / lifetime                                                                                                          |
| ----------------------------------------------- | --------------------------------------------------------- | --------------- | ------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Password / passkey / OAuth subject              | **Trusted** (credential)                                  | account         | Yes — authentication                                                | `users.password` (Argon2id PHC), `passkey_credentials`, `users.google_id`/`oauth_provider`; indefinite                      |
| Account ban                                     | **Trusted** (operator or gated rule)                      | account         | **Yes — the only ban subject**                                      | `user_bans` (`m000001:58`), `expires_at`/`revoked_at`; enforced by `check_ban`                                              |
| Fresh-auth grant                                | Trusted (session-bound, 5 min)                            | session         | Yes — gates credential writes                                       | `services/fresh_auth.rs` (in-memory, `MAX_GRANTS=10_000`)                                                                   |
| Session                                         | Trusted                                                   | account+session | Yes (authn)                                                         | tower session + `user_sessions` audit row; 14 d                                                                             |
| Device token (server-minted, client-held)       | **Semi-trusted** — it answers "which record", never "who" | device          | **Challenge only**                                                  | `device_records.token_hash`; keyed record 30 d                                                                              |
| Device signals / components (client-supplied)   | **Untrusted**                                             | device          | No — evidence only                                                  | `device_records` derived columns + `evidence` JSON; 30 d                                                                    |
| Shared-device grant (org kiosk, admin-approved) | **Trusted for suppression only**                          | device          | No — can only _remove_ device friction, never add an account action | `shared_device_grants`; 12 mo                                                                                               |
| Shared-device claim (self-serve)                | Untrusted but harmless                                    | device          | No — clears device reputation, suppresses prompts                   | `shared_device_grants` (`kind='self_claim'`); 30 d                                                                          |
| Network / IP                                    | Untrusted                                                 | request         | Evidence only (existing IP limiter/export unchanged)                | `rate_limit_state`, `user_sessions.ip_address`; per A8 finding 6                                                            |
| Behaviour / reading trail                       | **Not collected punitively** (A2 F9)                      | —               | No                                                                  | —                                                                                                                           |
| `device_v1` push token (`devices` table)        | Trusted for delivery only                                 | device          | No                                                                  | `devices` (`m000011`), `MAX_DEVICES_PER_USER=20`, ownership transfer = shared devices are already explicitly non-suspicious |

The row that matters: **only the first two layers can produce a lasting adverse effect.** Every
device-layer row can produce at most one session's challenge, and every one of them is
self-clearable.

### 1.2 Precedence rules (normative)

```
effect(user, device) =
    account_banned(user)                  -> BLOCK (existing check_ban; unchanged)
  : shared_grant_active(device)           -> NOTHING (device friction suppressed)
  : device_challenge_earned(device)       -> CHALLENGE (L2 max, self-clearable, session-scoped)
  : otherwise                             -> NOTHING
```

- **P1 — Account ban wins over everything.** A device grant, a self-claim, a kiosk enrolment, or a
  cleared device reputation must never change the result of `check_ban`. The grant tables are not
  consulted by `check_ban`, by `auth_guard::not_banned()`, or by any `user_bans` query. A test
  asserts no call path from `shared_device_grants`/`device_records` into the ban decision.
- **P2 — Device state never bans.** No rule may open an L5+ case whose only input is a device
  match, a device fan-out, a context change, or a component-key collision. Device evidence may
  raise review priority and may _suppress_ friction; it may not create exclusion.
- **P3 — Device friction is self-clearable, always.** Every challenge the device axis can produce
  has a one-click, in-language clear path reachable from the prompt itself (§3).
- **P4 — A ban on one account does not propagate to a device.** `user_bans.user_id` is the only
  key. A registered shared/kiosk device is **non-actionable**: it can never receive a
  `device_bans` row, and any operator attempt to create one is rejected by the handler
  (machine-guarded, §2.5).
- **P5 — Suppression never changes quotas.** Only an admin-approved org kiosk grant may raise a
  rate-limit budget (§2.4). A self-claim may not. This keeps A8's ranked #1 defense (per-account
  and per-credential limits) intact.
- **P6 — Unknown device is zero friction.** Absent token = unknown = baseline (A2 §5.2). Unknown
  never contributes to a ban score and never triggers a step-up on its own.

### 1.3 Data model additions on `users`

**None required.** Explicitly rejected candidates and where they live instead:

| Candidate                         | Verdict                                                                                                      | Where it lives instead                               |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------- |
| `users.is_org_account` / org role | Reject. A new role value touches `UserRole`, `auth_guard` consts, and every `role_min` check for no benefit. | `shared_device_grants.org_label` + `grantee_user_id` |
| `users.is_shared_device_user`     | Reject — that is a device property, not a person property.                                                   | `device_records.trust_state` / `shared_until`        |
| `users.minor`                     | Already DESIGN-v1 B7; not this subsystem's column.                                                           | DESIGN-v1 B7 (`users` DOB/flag), collector-off rule  |
| `users.shared_claim_at`           | Reject — claims need history + rate-limit counting.                                                          | `shared_device_grants` (one row per claim)           |

The only account-side state this design reads is what already exists: `user_bans`,
`users.role`, `users.is_verified`, `users.two_fa_enabled`, and the session. That is deliberate:
it means the shared-device subsystem can be shipped, tested, and rolled back without touching the
`users` table at all.

---

## 2. Kiosk / shared-device registration

Two mechanisms with deliberately different strength. Confusing them is the main way this design
could be abused, so the split is normative.

|               | Self-claim (§3)                                         | Org kiosk (this section)                                                         |
| ------------- | ------------------------------------------------------- | -------------------------------------------------------------------------------- |
| Who           | any authenticated account                               | admin, after a light out-of-band check                                           |
| Effect        | clears device reputation, suppresses new-device prompts | same, plus step-down of device-triggered actions, plus bounded quota/limit raise |
| Lifetime      | 30 d (renewable by claiming again)                      | 12 mo (renewable)                                                                |
| Quota effect  | **none**                                                | `write_quota_multiplier` + `ip_unit` budget raise (bounded)                      |
| Abuse ceiling | nuisance: challenge avoidance                           | bounded: one admin-approved device, revocable, logged                            |

### 2.1 Data model

```sql
-- m000012 (fold into the device_records CREATE TABLE; if m000012 has already shipped, m000013):
-- current shared-device state, read from the in-process map, never queried per request.
ALTER TABLE device_records ADD COLUMN shared_until TEXT;              -- NULL = not shared
ALTER TABLE device_records ADD COLUMN shared_grant_id INTEGER;        -- current grant
ALTER TABLE device_records ADD COLUMN context_last TEXT;              -- browser|webview|standalone (attribute only)
ALTER TABLE device_records ADD COLUMN reputation_cleared_at TEXT;     -- last self-claim/kiosk clear
ALTER TABLE device_records ADD COLUMN non_actionable INTEGER NOT NULL DEFAULT 0;

-- m000013: grant history + enrolment codes + audit. Append-only except state/revocation columns.
CREATE TABLE IF NOT EXISTS shared_device_grants (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,                    -- 'self_claim' | 'org_kiosk'
    state TEXT NOT NULL DEFAULT 'pending', -- pending|active|suspended|revoked|expired
    device_record_id INTEGER,              -- NULL until an org code is redeemed
    grantee_user_id INTEGER,               -- self-claim: claimant; org: staff who enrolled
    org_label TEXT,                        -- madrasa/school name (org only)
    org_contact TEXT,                      -- single monitored contact (org only)
    enrol_code_hash TEXT,                  -- HMAC(code); NULL for self_claim; single-use
    code_expires_at TEXT,                  -- 24 h to redeem an org code
    expected_accounts INTEGER,             -- org's declared student count (advisory)
    write_quota_multiplier INTEGER NOT NULL DEFAULT 1,  -- org only, 1..4
    ip_unit TEXT,                          -- network unit recorded at enrolment (org only)
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TEXT NOT NULL,              -- NOT NULL: no grant is indefinite
    revoked_at TEXT,
    revoked_by INTEGER,
    approved_by INTEGER,
    notes TEXT,
    FOREIGN KEY (device_record_id) REFERENCES device_records (id)
        ON UPDATE CASCADE ON DELETE SET NULL,
    FOREIGN KEY (grantee_user_id) REFERENCES users (id)
        ON UPDATE CASCADE ON DELETE SET NULL,
    FOREIGN KEY (approved_by) REFERENCES users (id)
        ON UPDATE CASCADE ON DELETE SET NULL,
    FOREIGN KEY (revoked_by) REFERENCES users (id)
        ON UPDATE CASCADE ON DELETE SET NULL
);
-- One-time code lookup (SQLite: NULLs distinct, so self-claims coexist).
CREATE UNIQUE INDEX IF NOT EXISTS idx_shared_grants_code ON shared_device_grants (enrol_code_hash);
-- One ACTIVE org grant per device.
CREATE UNIQUE INDEX IF NOT EXISTS idx_shared_grants_active_device
    ON shared_device_grants (device_record_id) WHERE state = 'active' AND kind = 'org_kiosk';
-- Admin list + sweeper.
CREATE INDEX IF NOT EXISTS idx_shared_grants_state_expires ON shared_device_grants (state, expires_at);
CREATE INDEX IF NOT EXISTS idx_shared_grants_device_created ON shared_device_grants (device_record_id, created_at);
-- Claim rate-limit count: distinct accounts per device per window.
CREATE INDEX IF NOT EXISTS idx_shared_grants_device_grantee ON shared_device_grants (device_record_id, grantee_user_id);
```

Notes: `enrol_code_hash` is `HMAC(server_key, code)` (same pattern as the appeal token in A8 §4) —
never the raw code. `org_label`/`org_contact` are the only PII in the table; they carry the 12 mo
retention class. `ALTER TABLE ADD COLUMN` must default NULL / literal (A10 finding 9).

### 2.2 In-process policy map (the `pool = 1` requirement)

```
SharedDevicePolicy { device_record_id, token_hash, kind, state, expires_at,
                     non_actionable, write_quota_multiplier, ip_unit }
static SHARED_POLICY: OnceLock<RwLock<HashMap<u64 /*device_record_id*/, SharedDevicePolicy>>>
static SHARED_IP:     OnceLock<RwLock<HashMap<String /*ip_unit*/, u32 /*multiplier*/>>
```

- Hydrated at boot from `shared_device_grants WHERE state='active' AND expires_at > now`
  (plus their `device_records` rows) and refreshed by a **60 s ticker** registered in
  `scheduler.rs` — the `route_blocker_service.rs` snapshot pattern with a longer TTL because
  grants change rarely. Mutations (create/suspend/revoke/renew) refresh eagerly, exactly like
  `refresh_snapshot_best_effort`.
- The limiter consults the map (in-process, no DB). **Stale map = fail-open for suppression**
  (the map's absence of a policy means "no suppression", i.e. the pre-existing behavior) — never
  fail-closed, because fail-closed on a shared-device map would recreate A2 F12's systemic
  false-positive class.
- A grant expiring is a no-op: `expires_at` in the map is checked at read time; the ticker also
  drops expired entries.

### 2.3 Admin flow (org kiosk)

1. **Request.** The madrasa/school contacts `support@` (or the appeal mailbox). No web form in v1
   (A8: no web admin UI).
2. **Light verification.** Operator checks: institutional domain on the contact address, or a
   one-line reply from a staff member on the school's published contact, or a known
   teacher/community reference. This is deliberately light — the grant is revocable and confers no
   exemption from account-level rules. Recorded in `notes`.
3. **Create.** `POST /admin/device-grants` with `{kind:'org_kiosk', org_label, org_contact,
expected_accounts, expires_in_days}` (Admin role; session + CSRF). The response contains the
   **one-time enrolment code** (24 h to redeem), returned exactly once — never listed again.
4. **Enrol.** A staff member signs in on the kiosk device and submits the code at
   `POST /device/v1/kiosk/enroll`. Server: hash lookup → `state='pending'` and not expired →
   binds `device_record_id` from the caller's session device record → `state='active'`,
   `enrol_code_hash=NULL`, `grantee_user_id=caller`, `ip_unit=current`, writes the
   `device_records` columns (`trust_state='kiosk'`, `shared_until=expires_at`,
   `non_actionable=1`), refreshes the map, writes a `device_evidence` row.
5. **Renew.** Admin issues a new code before expiry, or `POST .../renew` extends
   `expires_at` by 12 mo if no `kiosk_review` case is open. Renewal is not automatic in v1.
6. **Revoke/suspend.** Admin-only (§2.5).

### 2.4 What a grant does, precisely

- **(a) Device-triggered challenges are suppressed or pre-satisfied.** With an active grant, the
  device axis produces no new-device prompt and no device-match challenge for **any** account
  signing in on that device. This is the single most important behavior for the kiosk population:
  without it, every student's first write triggers a challenge, which is both a false positive and
  an unanswerable one (they cannot "clear" a challenge they never caused). The challenge path
  itself is unchanged for all other devices.
- **(b) No account's ban propagates to the device.** Enforced structurally: account bans are
  `user_bans` rows keyed by `user_id`; a registered device is `non_actionable=1` and the
  device-ban creation handler refuses it (P4). The kiosk survives every student's ban.
- **(c) Abuse is still attributable to the account.** Every write is authenticated; the session
  binds `(device_record_id, user_id, user_session_id)` in `device_sessions` (A10 §1.2), so the
  case view answers "which account did this on the kiosk" with an indexed query. A grant changes
  _attribution_, not _anonymity_.
- **(d) Quota/limit raise (bounded).** `write_quota_multiplier` (1–4) multiplies the per-account
  write budget for accounts active on the device, and `ip_unit` gets the same bounded multiplier on
  the IP budget — the `ip_legit_share` idea from R2 §3, applied only to admin-approved units. Never
  an exemption: the raise is bounded, expires with the grant, and is visible in the metrics
  (§8: "kiosk quota exhaustion").
- **(e) Step-down.** Any account-level action considered for an account whose current session is
  on a granted device steps down one ladder level and requires a second admissible corroborator
  before L5+ (A2 §5.1 row A2; R2 ladder row 10).

### 2.5 When the kiosk is abused

| Situation                                                     | Response                                                                                                                                                         | Never                                             |
| ------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- |
| One student spams/harasses from the kiosk                     | Normal account-level ladder on that student's account; device untouched                                                                                          | Device ban; whole-school block                    |
| ≥5 distinct accounts enforced from one granted device in 30 d | Admin `kiosk_review` case (auto-opened, queue-only); grant stays active                                                                                          | Auto-revoke                                       |
| The org is a front / device is a farm                         | `POST .../revoke`: grant → `revoked`; device → `trust_state='shared_unverified'`, `non_actionable=0`; its accounts are surfaced for review; org contact notified | Device ban; account bans without individual cases |
| Grant used to dodge per-account limits                        | Revoke + `kiosk_quota_abuse` reason code; quotas revert; the org can re-apply with a per-student plan                                                            | Silent quota cuts on unrelated accounts           |
| Leaked/replayed enrolment code                                | Code is single-use + 24 h; a replay finds `state != 'pending'` → rejected; a second redeem on a different device opens a `kiosk_review` case                     | Accepting a code twice                            |
| Abuse during a suspended grant                                | Device behaves as unregistered: challenges return (still L2 max, still self-clearable), never bans                                                               | Punitive device state                             |

---

## 3. Self-service "this is a shared device" claim

### 3.1 What it does

1. Sets `device_records.shared_until = now + 30 d` and `reputation_cleared_at = now`.
2. Clears device reputation: `risk_score = 0`, device-derived signals no longer count toward
   review priority for that record during the window.
3. Suppresses new-device prompts for the device for the window (same suppression as §2.4(a)).
4. Closes any open **device-sourced, account-free** case on that device with reason code
   `overturned_by_claim` (v1 cases are queue-only, so this is a record update, not an un-ban).
5. Records one `device_evidence` row: `subject='shared_claim'`, actor = the claiming account,
   plus the optional free-text "who else uses this device" (rendered to the operator, redacted
   copy in the notice).

### 3.2 What it must NOT do

- **Not clear an account ban.** `user_bans` is untouched. No path from the claim to
  `user_bans.revoke`. The claim response must say so (§9 copy).
- **Not launder a ban.** It cannot revoke a suspension, restore content, clear another account's
  record, or remove `device_sessions` links (the linkage asset survives — A2 §5.1).
- **Not change quotas or limits** (P5).
- **Not re-identify.** If the caller has no device record (storage cleared), respond
  `{cleared:false, reason:"no_device"}` and mint nothing — never rebuild a record from components
  to service a claim (A2 Part 4.3 #3).

### 3.3 Abuse potential and rate limits

| Attack                                                          | Why it fails                                                                                                                                                                          | Rate limit                   |
| --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------- |
| Evader claims "shared" to keep device reputation clean          | Device reputation is not a ban input; account bans are unaffected; the claim is exactly the intended use                                                                              | 1 per account / 30 d         |
| Claim spam from many accounts on one device to farm suppression | Device-level cap: >3 distinct claiming accounts / 90 d → device enters `kiosk_review`, **auto-clear stops**, admin sees the list                                                      | ≤3 distinct accounts / 90 d  |
| Claim immediately after a ban to look innocent                  | Recorded as `claim_after_ban` (claim within 7 d of a ban on the same device) and surfaced in the case; **not** punished automatically — a legitimate sibling/co-user is expected here | informational                |
| Kiosk-scale claim churn                                         | The org path exists; claim churn is the signal to use it (§7)                                                                                                                         | escalation to `kiosk_review` |

The endpoint requires an authenticated session and CSRF (mounted with the existing
`auth_guard::authenticated`, i.e. `not_banned()` applies — deliberate: a banned account cannot
clear device state, but the **appeal** path remains unauthenticated and ungated, so a banned user
is never without recourse). It does **not** require verified email or 2FA — those requirements are
what create the no-email lockout the prompt asks about.

---

## 4. New-device step-up UX for a shared-device population

### 4.1 When the prompt may appear

| Surface                                                                   | Prompt allowed?                                  |
| ------------------------------------------------------------------------- | ------------------------------------------------ |
| Reading any surah/page/juz, `/t/**`, reader APIs, `.md`/`.txt`/`llms.txt` | **Never**                                        |
| Login, logout, password reset, email verification, appeal                 | **Never**                                        |
| Session listing / account page                                            | Never                                            |
| Credential/email/phone change                                             | Only the existing `fresh_auth` grant (unchanged) |
| **Writes** (comment create/update, media upload, profile write)           | Yes — L2 challenge only                          |
| Kiosk/shared-granted device                                               | No device prompt at all (§2.4a)                  |

The only realistic device-triggered prompt for this product is on the comment/media write path
(the repo's publishing surface is `post_comment_v1` + `media_v1`; bookmarks are private). "New
device at login" is **not** a trigger (A2 §5.2 #4).

### 4.2 What the prompt says

Short, non-accusatory, in the reader's language, with the shared-device escape visible _in the
prompt itself_ (never a link to a help page):

> **You're on a new device.** To post from here, confirm it's you.
> [ Confirm ] [ This is a shared device (school, family, café) ]

The claim button opens a one-field confirm ("Anyone can use this device") and completes the §3
claim in place. No jargon, no "verify your identity", no mention of devices being tracked, no
"unusual activity detected".

### 4.3 Non-CAPTCHA fallback (the no-email / no-2FA / censored-network escape)

The challenge is never the only path. Fallback ladder, evaluated in order:

1. **Turnstile** if `challenges.cloudflare.com` loads within 5 s and siteverify succeeds.
2. **Email OTP link** if the account has a verified email and the mail path is up.
3. **Cooldown, then allow** — no email, no 2FA, no challenge host: the write is accepted into a
   short deferred state (UI: "Saved. Your comment will be posted in about 15 minutes.") or the
   user simply retries after the cooldown with a reduced write quota. Read-only never applies to
   reading; it applies only to the pending write.

**Turnstile host availability (censored networks, Tor):** `services/turnstile.rs` currently
fails closed on transport/config failure for registration. For the device step-up the semantics
must be inverted: **challenge-unreachable ⇒ fallback, never deny.** Implementation notes:

- Widget loaded with a hard timeout + `error-callback`; failure → jump to fallback 2/3 immediately,
  no retry loop.
- The server treats a missing/expired challenge token as "fallback" for the device path (the
  account-level limiter still applies), and logs a `challenge_unreachable` metric (§8).
- The shared-device claim and the appeal page are **never** gated by the challenge widget — a
  user who cannot reach `challenges.cloudflare.com` must still be able to clear device friction
  and to appeal.

---

## 5. Legitimate multiple accounts vs ban evasion — the operator's decision procedure

Doctrine: multiple accounts are permitted (Wikipedia legitimate uses: family, work/personal,
education/class accounts, privacy pseudonyms, security on public computers; Meta bans only
accounts created/repurposed to evade). **Abuse is defined functionally** (Stack Exchange): _the
second account lets the user do something the first account is prevented from doing._ On shared
hardware, the only admissible linkage families are person-level (A2 Part 3.1): credential/identity
reuse, identical **violating** content, declared/verified link. Device, network, locale, screen,
behaviour, and reading trail are **inherited by co-users** and are inadmissible.

### 5.1 Decision procedure (stored as case fields, not judgment)

| Step | Question                                                                                                                                                                             | Field stored                          | If NO                                       |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------- | ------------------------------------------- |
| 0    | Is there a **prior case** on the parent account (not just a shared device/IP)?                                                                                                       | `parent_case_id`                      | `no_action_inadmissible` — close            |
| 1    | **Functional test:** does the second account let the user do something their first account is prevented from doing? (First account not restricted → nothing to evade.)               | `functional_test` boolean             | `no_action_functional_test_failed` — close  |
| 2    | **Causal test:** was the second account created after the ban and does it carry a retained parent match?                                                                             | `parent_case_id` + created_at         | `no_action_no_causal_link` — close          |
| 3    | **Corroboration:** ≥1 admissible corroborator — credential/identity reuse ∨ identical violating content ∨ declared link? (Forgeable by a malicious co-user → never alone sufficient) | `corroborators[]` (enum, closed list) | friction only — close at L2/L3              |
| 4    | **Shared-device check (mandatory):** device fan-out ≥3 ∨ grant active ∨ kiosk annotation ∨ shared/CGNAT IP ∨ minor involved?                                                         | `shared_device_signals`               | proceed                                     |
| 5    | **Continuity (used only to close):** does the account look like a person continuing their own life?                                                                                  | `continuity`                          | if yes → `legitimate_multi_account` — close |
| 6    | **Decision**                                                                                                                                                                         | `decision` + `reason_code`            | —                                           |

Rules that fall out of the table:

- A case that reaches step 4 with a shared-device signal **steps down one level and can never
  reach L6**; the shared-device signal must be recorded even when the action is upheld.
- Steps 0–2 failing ⇒ close, always. Those are the legitimate-multi-account exits.
- Continuity may only _close_ a case, never open one (on shared hardware it is inherited).
- Upheld decisions act on the **account** and reference the parent **case**, never the device.

### 5.2 Reason codes (closed list)

`no_action_inadmissible`, `no_action_functional_test_failed`, `no_action_no_causal_link`,
`legitimate_multi_account`, `friction_challenge_only`, `action_account_suspension`,
`action_account_ban_proposed`, `device_flagged_kiosk_review`, `overturned_by_claim`,
`overturned_on_appeal`.

### 5.3 What the operator sees (bounded linked-accounts panel, A8 §6)

device → accounts (via `device_sessions`) → other devices → accounts, expansion capped at 20
accounts; grants/annotations shown inline; no clustering, no graph job, no auto-ban. The panel's
job is to answer "is this a family phone or a farm?" — and the procedure above is what turns the
answer into an action.

---

## 6. In-app browser / PWA / WebView fragmentation — the join rule

The failure to prevent (A2 F11/B3/B4): one human on one phone, arriving via a WhatsApp link into
an in-app WebView, then Safari, then a home-screen PWA, becomes three device records — and if one
context is ever flagged, the others look like evasion. iOS home-screen web apps are
storage-isolated from Safari (WebKit bug 181849); Android WebView is excluded from UA reduction
and UA-CH and has separate storage.

### 6.1 Concrete join rule (normative, evaluated on the login/link path only)

```
1. Valid device token present?
     -> device_record = lookup(token_hash). Update last_seen + context_last. STOP.
2. Authenticated session with a device_sessions binding?
     -> use that device_record_id for continuity. STOP.
3. Otherwise -> "unknown device".
     No record is minted from components on read paths.
     A record is minted only at login, marked trust_state='observe', and is non-actionable.
4. Component keys NEVER merge records and NEVER create records.
```

### 6.2 Rules that make the join hold

- `context` (`browser` | `webview` | `standalone`) is an **attribute** (`device_records.context_last`
  - evidence JSON), never part of any identity key. A context change updates the attribute; it
    never creates a record, never triggers a prompt, and never appears in any rule.
- **Account continuity is the join** across storage-isolated contexts: Safari and the home-screen
  PWA may have different tokens → two records → both bound to the same account. That is normal.
- `device_count_per_account` and `contexts_per_account` are **excluded from every rule** and from
  the review-priority score (A2 Part 4.3 #2). A machine guard asserts no rule reads them.
- No context handoff token in v1 (a signed token in a query param adds a tracking surface on
  shared/reader links for a marginal benefit). Revisit only if metrics show a real problem.
- A `webview` context never receives a device prompt (A2 F11 fix). If a link is opened in a
  webview and the user signs in there, that session's device record is a normal record; the
  account is the continuity.

---

## 7. Kiosk-scale operations: 200 students, one device

### 7.1 What breaks if all 200 share one account

| Break                                                             | Why                                                                                                                                             |
| ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| One student's violation blocks all 200                            | Enforcement is account-level (correct) and the account is shared                                                                                |
| Per-account write quota is 200× too small                         | Limits are per-human; the account is 200 humans                                                                                                 |
| School NAT (one `/32`) trips per-IP limits and could be IP-banned | A2 A2(ii) — the design must never IP-ban a school                                                                                               |
| **Device fan-out heuristics never fire**                          | fan-out counts _accounts per device_ = **1**; a shared account looks like one heavy user. This is a hole in the "≥3 accounts = shared" detector |
| Shared password, one recovery email, no attribution               | Any student can lock out the class; abuse is unattributable in practice                                                                         |
| Turnstile on 200 students behind one IP                           | challenges.stack, rate-limited; suppression is the only humane answer                                                                           |

### 7.2 Recommended posture (default): per-student accounts + one kiosk device token

- The device is enrolled once (§2.3). Every student has their own account and signs in per session.
- Writes are per-student → per-student limits, per-student enforcement, per-student attribution.
- Device prompts are suppressed for everyone on the kiosk (§2.4a) — this is what makes per-student
  accounts actually usable at 200/day.
- The kiosk is `non_actionable`; no ban propagates; the grant carries the bounded quota raise for
  the shared IP unit only.
- Copy shown at registration tells the org this is the default and why (§9).

### 7.3 If the org insists on one shared account

Do not silently fail. Detect and annotate:

- **Detector:** `sessions_per_device_per_day > 10` ∧ `writes_per_account_per_day` in the top
  percentile ∧ kiosk/grant annotation → propose `org_shared_account` on the account (admin
  annotation; queue-only in v1). No automatic friction.
- **Behavior:** with `org_shared_account`, the per-account write budget uses the grant's
  `write_quota_multiplier` (bounded 1–4) — still finite, still expiring with the grant.
- **Warn:** the org receives explicit copy at registration: one shared account means one student's
  violation blocks everyone; per-student accounts are the supported path.
- **On a ban of the shared account:** the device remains non-actionable, the grant can stay active,
  and the org is guided to create per-student accounts. The class is not IP-blocked.

### 7.4 Minors on kiosks

DESIGN-v1 B7 turns the collector off and deletes records for self-declared minors. Consequence for
this subsystem: the kiosk grant must be bound to the **device record**, not to any minor's session;
enrolment must be performed by a staff/adult account; minor sessions contribute no device evidence
and must not delete the kiosk record or the grant. This is stated so the two rules do not fight.

---

## 8. Metrics

One on-demand endpoint (`GET /admin/enforcement/metrics`, ROLE_ADMIN, A8 §5 pattern). All shared-
device numbers are **precision/quality** numbers; "bans issued" stays out (R2 §9).

| Metric                                | Definition                                                                                                 | Target                                              | Why                                                                   |
| ------------------------------------- | ---------------------------------------------------------------------------------------------------------- | --------------------------------------------------- | --------------------------------------------------------------------- |
| Device fan-out histogram              | devices by distinct accounts, 30 d rolling                                                                 | informational; modal 1–2; ≥3 expected, not alarming | distinguishes family phone from farm; drives the kiosk-review trigger |
| **Proposals from shared devices**     | L5+ proposals where fan-out ≥3 ∨ grant active ∨ shared annotation ÷ all proposals                          | **0**                                               | any >0 is a rule bug (P2/P4 violated)                                 |
| **Challenge rate on granted devices** | challenges shown on kiosk/claimed devices ÷ sessions on them                                               | **0** (suppression must be total)                   | >0 means the suppression path is broken                               |
| Challenge abandonment                 | challenges shown → not completed, by country / browser family / context (browser vs webview vs standalone) | flat across contexts; near-zero for shared devices  | A2 §2.2 #6 bias dashboard; context spikes mean the join rule broke    |
| Turnstile-unreachable fallback rate   | `challenge_unreachable` events ÷ challenges                                                                | tracked by country; fallback must carry the load    | censored networks must not be locked out                              |
| Claim rate + cap hits                 | self-claims/week; devices hitting the 3-account cap                                                        | cap hits → kiosk_review                             | turns claim abuse into a kiosk-review lead                            |
| Claim-after-ban rate                  | claims within 7 d of a ban on the same device ÷ claims                                                     | informational (expected >0 for families)            | tells evasion apart from siblings                                     |
| Unknown-device share                  | sessions with no token ÷ sessions                                                                          | high and steady = normal (SLO, not suspicion)       | A2 Part 4.3 #5                                                        |
| Shared-account detection              | accounts with `org_shared_account` + sessions/day                                                          | trending down after org onboarding                  | measures whether the per-student recommendation is working            |
| Kiosk quota exhaustion                | kiosk accounts hitting the raised write budget                                                             | ~0                                                  | a kiosk constantly exhausting 4× means it needs per-student accounts  |
| Grant lifecycle                       | active/suspended/revoked grants; median time-to-revoke after `kiosk_review`                                | ≤7 d                                                | keeps kiosk trust real                                                |

---

## 9. Compact spec

### 9.1 Data model (delta on DESIGN-v1)

- `users`: **no additions** (§1.3).
- `device_records`: add `shared_until TEXT`, `shared_grant_id INTEGER`, `context_last TEXT`,
  `reputation_cleared_at TEXT`, `non_actionable INTEGER NOT NULL DEFAULT 0`; `trust_state` gains
  values `shared_claimed` and `kiosk` (TEXT enum, fail-closed read).
- `shared_device_grants`: new table (§2.1). Active grants block the 30 d retention sweep for the
  bound `device_records` row (A10 §7: retention must be blockable by active state); on expiry the
  record returns to the normal ladder.
- In-process: `SHARED_POLICY` + `SHARED_IP` maps, boot-hydrated, 60 s ticker, eager refresh on
  mutation, fail-open on stale (§2.2).
- `device_evidence`: `shared_claim` / `kiosk_enrolled` / `kiosk_revoked` subjects (append-only).

### 9.2 API surface

| Method + path                              | Auth / role                    | Effect                                                                     |
| ------------------------------------------ | ------------------------------ | -------------------------------------------------------------------------- |
| `POST /device/v1/shared/claim`             | session (authenticated) + CSRF | §3 claim; body `{note?}`; returns `{sharedUntil, cleared}`                 |
| `POST /device/v1/shared/release`           | session + CSRF                 | revoke own claim early (state `revoked`, reason `user`)                    |
| `GET /device/v1/shared/status`             | session                        | `{granted, kind, expiresAt, suppressed}` for the caller's current device   |
| `POST /device/v1/kiosk/enroll`             | session + CSRF                 | redeem one-time org code `{code}`; binds device; `{granted, expiresAt}`    |
| `GET /admin/device-grants`                 | ROLE_MODERATOR                 | list, filters `state`, `kind`, `expires_before`                            |
| `GET /admin/device-grants/{id}`            | ROLE_MODERATOR                 | detail + bound device + org fields                                         |
| `POST /admin/device-grants`                | ROLE_ADMIN                     | create `org_kiosk`; returns the one-time code (once)                       |
| `POST /admin/device-grants/{id}/renew`     | ROLE_ADMIN                     | +12 mo if no open `kiosk_review`                                           |
| `POST /admin/device-grants/{id}/suspend`   | ROLE_ADMIN                     | suppression off, challenges return (L2 max)                                |
| `POST /admin/device-grants/{id}/revoke`    | ROLE_ADMIN                     | revoke; device `shared_unverified`; reason code required                   |
| `POST /admin/device-records/{id}/annotate` | ROLE_MODERATOR                 | existing A8 action gains `shared`/`kiosk`/`org_shared_account` annotations |
| `GET /admin/enforcement/metrics`           | ROLE_ADMIN                     | §8 numbers                                                                 |

Never: an export token on any mutation (A8 finding 11); device bans on granted devices (P4); a
device route on the reader path.

### 9.3 UX copy (English source; must exist in the reader's languages)

| Key                         | Copy                                                                                                                                  | Notes                                          |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| `device.prompt.title`       | You're on a new device.                                                                                                               | No accusation, no "unusual activity"           |
| `device.prompt.body`        | To post from here, confirm it's you.                                                                                                  | Writes only                                    |
| `device.prompt.sharedCta`   | This is a shared device (school, family, café)                                                                                        | Escape visible in the prompt                   |
| `device.claim.confirm`      | Anyone can use this device.                                                                                                           | One-field confirm                              |
| `device.claim.done`         | Got it. We won't ask about this device again for 30 days.                                                                             | Honest scope                                   |
| `device.claim.noBanNote`    | This doesn't change your account status. If your account is restricted, this won't lift the restriction.                              | Mandatory; prevents the "clear my ban" misread |
| `device.claim.noDevice`     | Nothing to clear — this device isn't stored.                                                                                          | Storage-cleared case                           |
| `device.fallback.deferred`  | We can't reach the verification service right now. Your post is saved and will go out in about 15 minutes.                            | Turnstile-unreachable fallback                 |
| `kiosk.enroll.prompt`       | Enter the code your school gave you.                                                                                                  | Staff-only                                     |
| `kiosk.enroll.done`         | This device is registered as a shared school device. Students can sign in normally.                                                   | —                                              |
| `kiosk.register.warning`    | If all your students share one account, one student's violation blocks everyone. We recommend one account per student on this device. | Shown to the org at registration               |
| `enforcement.notice.shared` | This device is used by more than one person. If you share this device, tell us who else uses it and we will re-review.                | R2 §5 pattern; categories only                 |
| `device.prompt.failed`      | Something went wrong. You can still read, and you can try posting again later.                                                        | Never a dead end                               |

### 9.4 Machine guards (tests that must fail the build if violated)

1. **Precedence guard:** no path from `shared_device_grants` / `device_records` into
   `check_ban` or `user_bans` writes (P1). Assert call-site count of `NewUserBan::permanent()` is
   unchanged (A8 §1.3).
2. **Non-actionable guard:** the device-ban creation handler rejects `non_actionable=1` (P4).
3. **No-quota-from-claim guard:** `shared_device_grants.kind='self_claim'` cannot produce a
   multiplier >1 (P5).
4. **Context-attribute guard:** `context_last` appears in no rule module; `device_count_per_account`
   is unread by the rule engine (§6.2).
5. **Zero-DB-on-request guard:** the request path reads only `SHARED_POLICY`/`SHARED_IP`; a test
   with a broken DB still answers suppression from the map.
6. **Retention guard:** the sweeper skips `device_records` bound to active grants (§9.1).
7. **Step-up scope guard:** no device-challenge call site exists on reader, login, recovery, or
   appeal routes (§4.1).

---

## Findings (10, all NEW)

### 1. NEW — Shared _accounts_ are invisible to the design's shared-device detector; fan-out counts accounts-per-device, and a madrasa on one shared account shows fan-out = 1

**Severity: Critical**
**Evidence.** DESIGN-v1/A2 ladder row 2 and A2 §5.1 define the shared-device signal as "device token
seen with ≥3 accounts". A madrasa that puts 200 students through one account produces
`accounts_per_device = 1` and is indistinguishable from a single heavy user; the entire shared-
device protection set (step-down, kiosk suppression, non-actionable) never fires, while per-account
write limits and per-IP limits (school NAT = one `/32`) throttle all 200. A2 A2(ii) already warned
the IP axis takes out the school.
**Fix.** Add the second signal (§7.3): `sessions_per_device_per_day` + `writes_per_account_per_day`

- grant/annotation → propose `org_shared_account` (queue-only). Recommend per-student accounts +
  one kiosk device token as the supported posture; warn the org with explicit copy; allow a bounded
  `write_quota_multiplier` only via admin-approved grants.

### 2. NEW — Precedence (account ban > device challenge > nothing) is unasserted in DESIGN-v1 and needs a machine guard, or the shared-device mechanisms become an un-ban path

**Severity: Critical**
**Evidence.** The only account-ban enforcement is `check_ban` → `auth_guard::not_banned()`
(`services/auth.rs:530`, `auth_guard.rs:31/66/84`). DESIGN-v1 B3 adds `device_bans` and grant-like
state with no stated ordering; A2 F3 requires "a shared-device claim must never clear an account
ban" but nothing enforces it. Any future handler that consults a grant before `check_ban` silently
creates an exemption.
**Fix.** Encode P1–P6 (§1.2) and the §9.4 guard 1: no call path from `shared_device_grants` /
`device_records` into the ban decision; grant tables are read only by the limiter and the admin
surface.

### 3. NEW — The self-service claim needs a device-level cap, or it is an unbounded "clear my device reputation" button

**Severity: High**
**Evidence.** A2 §3.2 requires device-linked actions to be self-service clearable; without a
device-level limit, an evader on a rotating device claims on every new record. Conversely, a real
kiosk (200 students, each on their own account) would generate claims from many accounts on one
device — which must _escalate_, not auto-clear.
**Fix.** 1 claim/account/30 d; ≤3 distinct claiming accounts per device per 90 d; the 4th →
`kiosk_review`, auto-clear stops; claims within 7 d of a ban recorded as `claim_after_ban`
(informational only — siblings legitimately do this). Claim does not require verified email (§3.3).

### 4. NEW — Context (in-app browser / PWA / WebView) must be an attribute with a fixed 4-step join rule, or one phone becomes three device records and the third looks like evasion

**Severity: High**
**Evidence.** A2 F11/B3/B4: iOS home-screen web apps are storage-isolated from Safari (WebKit bug
181849); Android WebView is excluded from UA reduction and UA-CH and has separate storage;
~31% of mobile web sessions are in-app (secondary). DESIGN-v1 B3's `device_records` columns do not
include context explicitly (only "evidence JSON tail"), so a literal implementation can key on
components and split.
**Fix.** §6: context is `device_records.context_last`, never an identity input; the 4-step join
rule (token → session binding → unknown → never components); `device_count_per_account` and
context counts excluded from every rule; machine guard 4.

### 5. NEW — Kiosk strength must be admin-issued, single-use, expiring and quota-carrying; a self-serve kiosk flag would launder the per-account limits that A8 ranks as the #1 defense

**Severity: High**
**Evidence.** A8 §9 ranks per-credential/per-account rate limits first; R2 §3 recommends
org-linked device registration and `ip_legit_share`; A2 A2 requires "kiosk/organisation
registration from day one"; DESIGN-v1 has no mechanism. If the self-claim carried a quota or IP
raise, an attacker claims "school" to bypass the primary defense.
**Fix.** Split strengths (§2): self-claim = suppression only, 30 d, no quota change (P5); org kiosk
= admin-approved, one-time HMAC'd code, 24 h redemption, 12 mo, bounded multiplier 1–4, revocable,
logged; `enrol_code_hash` single-use; replay → `kiosk_review`.

### 6. NEW — The no-email/no-2FA escape must be a deferred write, and Turnstile must never be the only path; today's `turnstile.rs` semantics are fail-closed and would lock out censored networks

**Severity: High**
**Evidence.** A2 F8: the CSP allows `challenges.cloudflare.com`, the widget is blocked/throttled on
censored/filtered networks and cannot complete in Tor; low-literacy users and minors without email
have no recovery path. `services/turnstile.rs` denies on config/transport failure
(`captcha_unavailable()` in production, `verify_registration` fail-closed) — correct for signup,
wrong for a device step-up.
**Fix.** §4.3: Turnstile → email OTP → cooldown-then-allow with a reduced quota; challenge-
unreachable ⇒ fallback, never deny; widget timeout + error callback; the claim and appeal paths are
never challenge-gated; log `challenge_unreachable` by country (§8).

### 7. NEW — Kiosk grants must be bound to the device record, not a session, or the minor rule (collector off + records deleted) churns the grant and un-registers the school

**Severity: Medium-High**
**Evidence.** DESIGN-v1 B7 deletes device records for self-declared minors; a madrasa kiosk is a
minors-majority population. A grant bound to a session or to per-minor records would be deleted
with them, silently reverting the kiosk to unregistered behavior mid-term.
**Fix.** §7.4: grant binds `device_records.id`; enrolment by a staff/adult account; minor sessions
write no device evidence and never delete the kiosk record or grant; retention sweeper skips
grant-bound records (§9.4 guard 6).

### 8. NEW — The multi-account decision procedure must be a stored checklist with closed reason codes, or "looks linked" cases will ban legitimate family accounts

**Severity: High**
**Evidence.** A2 F4 (inheritance lemma: device, network, locale, behaviour, reading trail are all
inherited by co-users; only credential reuse, identical violating content, and declared links
survive, and even those are forgeable). R2 §3 (Stack Exchange functional test; Wikipedia
legitimate uses; Meta permits multiple accounts). A8 §8 already establishes the checklist pattern
for L6.
**Fix.** §5: the 7-step procedure with stored fields; inadmissible signal families excluded by
construction; steps 0–2 failing always close; shared-device signal forces step-down and bars L6;
continuity may only close; closed reason-code list.

### 9. NEW — Metrics must include shared-device false-ban proposals and per-context challenge rates; otherwise the failure mode is invisible until users leave

**Severity: Medium-High**
**Evidence.** R2 §9 (measure precision, never ban count); A2 §2.2 #6 (bias dashboard by browser
family/country/device class); A8 §5 (one on-demand metrics endpoint). No existing metric would
detect "kiosk devices are being challenged" or "a shared device produced a ban proposal".
**Fix.** §8 metric set with two zero-tolerance numbers (proposals from shared devices = 0;
challenge rate on granted devices = 0), the fan-out histogram, claim-cap hits, claim-after-ban
rate, `challenge_unreachable` rate, and the unknown-device SLO; all served by the single
`GET /admin/enforcement/metrics` endpoint.

### 10. NEW — The policy map must follow the `route_blocker_service` snapshot pattern (boot-hydrated, 60 s ticker, fail-open) because the repo has one SQLite connection and A2 F12 already flagged fail-closed device lookups as a systemic false-positive class

**Severity: Medium**
**Evidence.** `pool = 1`; DESIGN-v1 B3 mandates "0 DB ops on the request path"; `route_blocker_service.rs`
proved the snapshot + TTL + eager-refresh pattern (5 s TTL, stale-but-present serves last known
state on DB error); A2 F12: per-request device writes plus fail-closed guards turn DB latency into
user-facing denials.
**Fix.** §2.2: `SHARED_POLICY`/`SHARED_IP` maps hydrated at boot from active grants, 60 s ticker in
`scheduler.rs` (matching `TICK_INTERVAL_SECS = 60`), eager refresh on mutation, and **fail-open on
stale** (absence of policy = no suppression = pre-existing behavior, never a denial). Machine guard 5.

---

## Repo anchors used (verified in-tree at the time of writing)

- `rust/backend/api/src/db/sea_models/user_ban/{model,actions,slice}.rs` — `create`, `revoke`,
  `get_active_ban`, `admin_list`, `NewUserBan::permanent()`; `expires_at`/`revoked_at` semantics.
- `rust/backend/api/src/services/auth.rs:530` — `check_ban`; `crates/rux-auth/src/traits.rs`
  `BanStatus`; `crates/rux-auth/src/middleware/guard.rs:46` — the `not_banned` gate;
  `src/middlewares/auth_guard.rs:18–22` — role consts incl. the dead `ROLE_MODERATOR`.
- `rust/backend/api/src/modules/device_v1/{controller,validator,mod}.rs` — `/device/v1`
  register/list/delete, `MAX_DEVICES_PER_USER = 20`, oldest-evict, same-token ownership transfer
  test ("shared devices are non-suspicious" already encoded).
- `rust/backend/api/src/db/sea_models/device/*` — the FCM push-token `devices` table
  (`m000005`/`m000011`); naming collision to avoid (A8 finding 12).
- `rust/backend/api/src/modules/admin_bans_v1/{mod,dto}.rs` — session-admin vs export-token split;
  `BanUnit = IpNet` only; mutations never accept the bearer token.
- `rust/backend/api/src/services/route_blocker_service.rs` — snapshot + TTL + stale-serving
  precedent; `src/services/scheduler.rs` — 60 s ticker to extend.
- `rust/backend/api/src/services/turnstile.rs` — production fail-closed semantics.
- `rust/backend/api/src/services/fresh_auth.rs` — 5-minute session-bound grants (unchanged).
- `rust/backend/api/src/modules/auth_v1/controller.rs` — `create_bound_session` (writes raw UA/IP
  into `user_sessions`), `sessions_list`/`sessions_terminate`.
- `web/src/routes/(account)/account/+page.svelte` — the Sessions section ("Devices currently
  signed in"); where the shared-device UI hooks in (claim button + status), with
  `account-view.ts`'s `sessionDeviceLabel` as the existing device-label precedent.
- `migration/src/m000001_init.rs:58` — `user_bans` DDL; `m000001:11–28` — `users` DDL (no new
  columns needed).
