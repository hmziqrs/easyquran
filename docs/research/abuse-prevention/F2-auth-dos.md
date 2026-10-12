# F2 — Authentication availability: semaphore, process-global maps, finish throttles, recovery, email change, single-instance

Auditor: F2 subagent. Scope: E5 fresh-eyes #7/#8/B1/B4/B5, A7 §5 constraint (a), plus the recovery
chain and email-change paths. Every claim below is verified against source at HEAD `53d3d421`
unless explicitly labelled "estimate".

**All findings in this report are marked NEW.** Method note: code was read first; measurements are
labelled. One measurement was taken on this dev machine (Apple Silicon, release build); VPS numbers
are scaled estimates, stated as such.

---

## 1. Password-work semaphore — quantification

### 1.1 The facts

| Fact                                    | Value                                                                                                                          | Source                                                                            |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------- |
| Capacity (slots)                        | `PASSWORD_WORK_CONCURRENCY`, **default 2**, accepted range 1..=32; `.env.example:31` sets `2`; docker-compose sets no override | `services/password_work.rs:8-15`, `.env.example:31`, `docker-compose.yml:120-152` |
| Acquire semantics                       | `try_acquire_owned()` — **non-blocking, zero queue**                                                                           | `password_work.rs:56-58`                                                          |
| On saturation                           | `Err(Busy)` → `ErrorCode::RateLimited` → **429, immediately, no `Retry-After`** (Busy has no `retry_after` set)                | `password_work.rs:25-32`, `:108-114`; `error/response.rs:103-116`                 |
| Permit lifetime                         | Acquired **before** `spawn_blocking`; held inside the blocking closure, survives HTTP cancellation                             | `password_work.rs:56-64`; test `:79-98`                                           |
| Argon2 params                           | `Argon2::default()`: m=19456 KiB (**19 MiB**), t=2, p=1 (Argon2id)                                                             | `password-auth-1.0.0/src/lib.rs:65,86`; `argon2-0.5.3/src/params.rs:42,52,61`     |
| Measured verify cost                    | **p50 = 11.6 ms, p95 = 17.5 ms** (20 iters, release, this Mac)                                                                 | measured for this report                                                          |
| VPS estimate (2–5× slower arm64)        | **p50 ≈ 25–60 ms**                                                                                                             | estimate from the measurement                                                     |
| Worst-case memory held by the semaphore | 2 × 19 MiB ≈ 38 MiB                                                                                                            | derived                                                                           |
| Telemetry on saturation                 | **none** — `password_work.rs` has no counter/gauge/histogram                                                                   | verified: no telemetry import in the file                                         |

### 1.2 Saturation threshold

With 2 slots and per-op time H, admitted arrivals above `2/H` saturate:

| H                | Saturation arrival rate          |
| ---------------- | -------------------------------- |
| 25 ms (fast VPS) | **80 admitted password-ops/sec** |
| 40 ms (mid)      | **50/sec**                       |
| 60 ms (slow VPS) | **33/sec**                       |

Once saturated, **every** request that needs a slot returns 429 instantly. The shared budget covers:
password login verify (`services/auth.rs:143`), **dummy verify for unknown emails and OAuth-only
accounts** (`auth.rs:86-89,115,136`), registration hash (`db/sea_models/user/actions.rs:81-84`),
password change / reset (`actions.rs:216-218`), admin create/update (`actions.rs:355-358,440-443`),
2FA backup-code hashing and consumption (`auth_v1/controller.rs:616,730-734,882-886`).

### 1.3 The dummy verify — confirmed, and it is the cheap vector

E5 is right: the unknown-email path calls `run_blocking_dummy_verify` → `password_work::run`
(`auth.rs:86-89`), i.e. the **same semaphore** and the same full Argon2id work (test
`dummy_verify_does_full_work_not_parse_short_circuit`, `auth.rs:602-618`). So an attacker needs **no
account, no valid email** — one arbitrary string per request is enough to consume a slot for H ms.

Two secondary observations:

- `DUMMY_VERIFY_HASH` is a `LazyLock` (`auth.rs:23-24`): the **first** dummy verify pays
  generate-hash + verify-hash (2 Argon2 ops) while holding a slot.
- `hash_backup_codes` = 10 sequential Argon2 hashes in one closure (`utils/twofa.rs:132-133`);
  `consume_backup_code` = up to 10 sequential verifies (`twofa.rs:136-138`). One backup-code
  request therefore holds a slot for **~10× H** (≈ 250–600 ms on the VPS) — a 10× amplifier for
  whoever triggers it, and a reason an authenticated 2FA user's request can look like a hang.

### 1.4 What the attacker must pay (login flood)

Admission controls in front of the semaphore on `POST /auth/v1/log_in`:

| Control                             | Limit                                                                                                             | Consequence for a flood                                                                               |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| Per-IP nest limiter (matched route) | 100/min/IP = **1.67 admitted req/s per IP**                                                                       | 20–48 IPs sustain saturation at H=25–60 ms                                                            |
| Per-account limiter `login:{email}` | **2 admitted attempts per 360 s**, 3rd sets a 1 h Temp block; the 24 h Long block needs ≈8 timed attempts (D1 §1) | Fresh random emails dodge it entirely; sustaining 50/s over 6 min costs ~9,000 distinct email strings |
| Turnstile                           | only on `register` (fail-closed in prod)                                                                          | **login has no captcha**                                                                              |

Attacker cost to fully deny login/register/reauth/2FA:

- **~33–80 admitted requests/sec** (2/H), i.e. **~20–48 IPs** at the per-IP cap, or **one IP** if the
  ingress hole (E5 #1: forgeable `CF-Connecting-IP` from any private-CIDR peer) is present.
- ~2,000–5,000 requests/min, each a tiny POST with a random email string. Any single VPS with a
  modest proxy pool does this; a botnet does it trivially.

Partial denial: duty cycle `d = A·H / 2`. At **10 IPs** (16.7 req/s, H=40 ms), d ≈ 33% → **one in
three legitimate login attempts gets 429**. At **6 IPs** with H=60 ms, d ≈ 100%.

### 1.5 Legitimate-user impact

- Login: 429 with a generic "Password service busy; retry later" body and **no `Retry-After`** — a
  client cannot even back off intelligently.
- Register: 429 (the error is preserved deliberately, `auth_v1/controller.rs:546-552`).
- **Reauth / fresh-auth**: `confirm_password_authentication` → `authenticate_password` → same
  budget (`auth_v1/controller.rs:416-428`). A pre-auth flood denies logged-in users the ability to
  change email/password, disable MFA, unlink providers, enroll passkeys — every fresh-auth-gated
  action.
- **Password reset**: `forgot_password::Entity::reset` → `change_password` → same budget; the code
  deliberately restores the reset grant after a transient 429
  (`forgot_password_v1/controller.rs:97-118,159-172`), but the user still cannot reset while the
  flood runs.
- 2FA login/verify/disable: same budget (TOTP code check itself is cheap, but backup-code paths and
  reauth are not).
- The web client has `DEFAULT_TIMEOUT_MS = 12_000` (A7 §0) — no special handling; users see 429s.

### 1.6 The documented reason the semaphore exists (and what a fix must preserve)

- The code states the design: one shared semaphore (default concurrency 2) over every Argon2 path,
  held inside the blocking closure; saturation returns 429; a transient 429 preserves the reset
  grant without extending it (`password_work.rs:8-64`; `forgot_password_v1/controller.rs:97-118`).
- Commit `bed80b13` ("fix(security): bound password work and gate hardened releases") added it; its
  now-deleted companion doc (`git show bed80b13:docs/security-hardening-2026-10-11.md`, absent at
  HEAD) states the intent: **"Admission budgeting follows OWASP's password hashing cost and
  denial-of-service guidance"** — bound concurrent Argon2 to cap memory (19 MiB/slot) and CPU on the
  ≥2 GB arm64 VPS.
- `deploy/README.md:221-222`: "Set it against container CPU/memory capacity; password and
  backup-code operations return 429 when full."

So the capacity bound is deliberate and correct. The defect is **fail-fast admission with one shared
budget and no lane separation** — not the bound itself.

### 1.7 Fix design (concrete)

Keep the memory/CPU bound; change admission:

1. **Queue-with-timeout instead of instant 429.** `tokio::time::timeout(PASSWORD_WORK_WAIT_MS
(default 1500), budget.acquire_owned())`; on timeout → `Busy`. Permit is still moved into
   `spawn_blocking` (hold semantics unchanged). Bursts queue for ≤1.5 s instead of failing the
   third concurrent user. Add a **global waiter cap** (e.g. 64): beyond it, immediate `Busy` again
   (bounds held tasks/connections under flood).
2. **Give the dummy verify its own budget.** `DUMMY_WORK_CONCURRENCY` (default 1) separate
   semaphore. Same Argon2 work (timing equalization preserved), but unknown-email floods no longer
   consume the login budget. Worst-case concurrent hashes become 2 + 1 + 1 (see lane split) = 4 ×
   19 MiB ≈ **76 MiB** — document the arithmetic next to the env var.
3. **Reserve an authenticated lane.** Split the real-work budget: `PREAUTH` (default 2; login,
   register) and `AUTHENTICATED` (default 1; reauth/fresh-auth, password change, reset, admin
   hashing). A pre-auth flood can then never starve security-changing actions. Total default
   concurrency 4 (or 3 if the dummy lane is merged back when memory is tight).
4. **Per-identity in-flight cap (fairness).** Optional, cheap: at most 1 concurrent hash per
   normalized email/account id; excess waits in the same queue. Stops one account from taking both
   pre-auth slots.
5. **Telemetry.** Add `password_work.saturated` counter, `password_work.wait` histogram,
   `password_work.in_flight` gauge (and the waiter gauge) — currently there is **zero** visibility
   on the component that is the single point of auth failure. Alert on sustained saturation.
6. **Split the backup-code work** onto the authenticated lane (it already is authenticated) and note
   the 10× hold; no param change needed.

Tradeoff summary: the fix cannot make a public CPU-bound endpoint unfloodable — a large enough
distributed flood still wins. It removes the **2-request** cliff, makes the attacker sustain
tens of requests/sec across dozens of identities/IPs, and guarantees recovery/reauth paths stay
usable during a login flood.

### 1.8 Attack the fix (new problems)

- **Queue = attacker-occupied slots.** Waiters hold tasks + sockets; the waiter cap bounds it, but a
  flood can keep all waiters full, so legit logins still time out to 429. Reserved lanes are what
  protect security actions; login itself degrades under sustained attack (honest).
- **Dummy lane doubles worst-case memory** (2 real + 1 auth + 1 dummy = 76 MiB). Must be set against
  the container limit; a 1 GB container with other memory could OOM. Document, and make the dummy
  lane shrinkable to 0 (then dummy verifies share preauth — original behavior) via env.
- **Reserved lane wastes one slot** when idle. Acceptable.
- **Per-identity cap** adds a PII-bearing in-memory map (normalized emails) needing TTL/eviction.
- **Timeout tuning**: too long → request pile-up; too short → no benefit. 1.5 s is below the web
  client's 12 s budget and above p95×10.

---

## 2. Process-global maps — capacity, eviction, exploitability

### 2.1 Facts per map

| Map                                          | Capacity             | TTL    | At capacity                       | Eviction code                                                                                |
| -------------------------------------------- | -------------------- | ------ | --------------------------------- | -------------------------------------------------------------------------------------------- |
| `passkey_state::STATES`                      | 10,000               | 300 s  | **evicts oldest entry globally**  | `min_by_key(\|(_, (_, issued_at))\| *issued_at)` → `map.remove` (`passkey_state.rs:104-112`) |
| `oauth::state::OAUTH_STATES`                 | 10,000               | 600 s  | **evicts oldest globally**        | `min_by_key(... stored_at)` → remove (`oauth/state.rs:142-150`)                              |
| `oauth::state::NATIVE_NONCES`                | 10,000               | 600 s  | **evicts oldest globally**        | `min_by_key(... issued_at)` → remove (`oauth/state.rs:71-79`)                                |
| `pending_login::GRANTS`                      | 10,000               | 300 s  | **429 fail-closed** (no eviction) | `pending_login.rs:85-87`                                                                     |
| `fresh_auth::GRANTS` (3 purposes incl. link) | 10,000               | 300 s  | **429 fail-closed** (no eviction) | `fresh_auth.rs:100-102,176-178`                                                              |
| `forgot_password::reset_token`               | 10,000 tracked users | 600 s  | **503 fail-closed**               | `forgot_password_v1/controller.rs:70-75`                                                     |
| `billing::checkout_intent::INTENT_STORE`     | **unbounded**        | 3600 s | grows until reaped on next access | `billing_v1/controller.rs:50-74`                                                             |

E5's claim is confirmed verbatim: the two eviction paths remove the **oldest** entry, which is
whoever issued earliest — i.e. the challenge most likely to belong to a legitimate user already
mid-flow. `passkey_state.rs:84-86` and `oauth/state.rs:46-52` reap expired entries first, but the
eviction has no owner/session scoping: one session's insert can delete another session's state.

### 2.2 What filling actually costs (precise, not hand-waved)

- **passkey_state**: `login/begin` is `unauthenticated`-guarded (`passkey_v1/mod.rs:17-19`) and
  issues one entry per call (`controller.rs:190-209`); it creates a session per cookie-less request
  (`passkey_session_id`, `:298-317`). The nest limiter is 60/min/IP **per matched route**
  (`router.rs:84-87`), plus one CSRF token per session from `/csrf/v1/generate` (60/min/IP). So one
  IP can insert **~60 entries/min**.
  - To reach 10,000 inside the 300 s TTL: **~34 IPs**.
  - At exactly 34 IPs, an entry survives ~4.9 min — i.e. a challenge is evicted just before its
    5-min TTL; a user who completes in <5 min is mostly safe.
  - To break a challenge issued **30 s ago**, the attacker must insert 10k in 30 s = 20k/min →
    **~333 IPs**, or one IP with the forgeable `CF-Connecting-IP` (E5 #1).
- **oauth state / native nonces**: same math, 600 s TTL, 60/min/IP (`router.rs:78-104`).
- **pending_login**: issuing a grant requires a **completed password (or passkey/OAuth) login on a
  2FA-enabled account** (`pending_login.rs:59-104`; `auth_v1/controller.rs:168-180`;
  `oauth/login.rs:331-339`). Filling 10k requires 10k successful password logins on 2FA accounts
  within 5 min, each also passing the per-account limiter (2/360 s per email). **Not practical.**
  If it ever fills, all new TOTP logins 429 (fail-closed).
- **fresh_auth**: issuing requires an authenticated user who **completed a password reauth**
  (`auth_v1/controller.rs:404-455` → `fresh_auth::issue`). 10k distinct sessions × completed
  reauth in 5 min is bounded by the same semaphore. **Not practical.** Fail-closed 429.
- **reset_token**: minting requires a valid emailed code (`verify` → `reset_token::mint`,
  `forgot_password_v1/controller.rs:424-443`). Not attacker-fillable at all. The 503 at the
  generations cap is an availability risk only under a bizarre mass-recovery event.
- **INTENT_STORE**: written after the server creates a provider checkout session; not an
  unauthenticated surface. The missing cap is a hygiene bug, not an attack.

### 2.3 Verdict per map

| Map                          | Exploitable today?           | Precise statement                                                                                                                                                                                                                                                                                              |
| ---------------------------- | ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| passkey_state                | **Conditional — MEDIUM-LOW** | Real cross-user eviction of in-flight challenges, but needs ~300+ IPs (or spoofing) to break a _fresh_ challenge; ~34 IPs to fill it. With E5 #1 fixed, this drops to "large proxy pool" territory. The bigger defects are: no per-session quota, oldest-first victim selection, and single-instance coupling. |
| oauth states + native nonces | **Conditional — MEDIUM-LOW** | Same shape, 10-min TTL. Native nonces are not session-bound, so eviction is fully cross-user.                                                                                                                                                                                                                  |
| pending_login                | **No**                       | Fail-closed; filling needs thousands of real 2FA logins.                                                                                                                                                                                                                                                       |
| fresh_auth                   | **No**                       | Same.                                                                                                                                                                                                                                                                                                          |
| reset_token                  | **No**                       | Requires email codes.                                                                                                                                                                                                                                                                                          |
| INTENT_STORE                 | **No**                       | Authenticated-ish surface; missing cap only.                                                                                                                                                                                                                                                                   |

### 2.4 Fix design

1. **No cross-session eviction. Ever.** On insert at cap: reap, then evict the oldest entry **of the
   requesting session only**; if the session has no entry, return 429 (`RateLimited`). Breaking a
   live challenge of another user is worse than refusing a new one: refusal is visible and
   retryable; silent eviction corrupts a security-critical single-use flow.
2. **Per-session quota** (e.g. ≤5 outstanding per session id) so one session cannot own the map.
   Note this does **not** by itself stop a flood (each cookie-less request gets a fresh session) —
   it bounds accidental/legitimate leakage and makes the "who owns the map" question answerable.
3. **Move the session-bound grants into the session payload** (shared SQLite, survives restart and
   a second replica):
   - `fresh_auth`: store the grant as an HMAC-signed payload in the session record instead of a
     process map. Single-use is preserved by deleting it from the session on consume — the session
     row is already written on issue/consume today (`fresh_auth.rs:181-187`), so this is **zero
     extra commits** and removes the strongest single-instance coupling.
   - `pending_login`: same treatment (token already lives in the session, `pending_login.rs:90-98`;
     the map only holds the credential snapshot). The DB reads needed to validate the snapshot stay.
   - `oauth` state: already keyed by session id + state secret (`oauth/state.rs:116-118`); store
     the PKCE verifier + nonce + link intent in the session payload. Removes one more map.
   - These three changes make 2FA logins, fresh-auth and OAuth survive rolling restarts and
     replica > 1 with no new storage.
4. **passkey_state stays a map** (the serialized `PasskeyRegistration`/`DiscoverableAuthentication`
   state is too large to put in every session row, and single-use remove-on-read is load-bearing,
   `passkey_state.rs:117-120`). Apply fix 1 + 2 + telemetry (insert/evict/reject counters).
5. **Native nonces**: either signed stateless tokens (HMAC over provider+nonce+iat, verify on
   consume — loses strict single-use, keep a tiny consumed-nonce set if needed) or keep the map with
   fix 1. Native flows are mobile-only; either is acceptable.
6. **INTENT_STORE**: add a cap + reject (hygiene).
7. **Telemetry** on every map: size gauge, rejected-at-cap counter, eviction counter.

### 2.5 Attack the fix

- Refusal-at-cap is itself a DoS on **new** passkey/OAuth begins under a flood — but only for the
  new flow, never for in-flight ones. This is the correct tradeoff; document it.
- Per-session quota is bypassed by cookie-less requests; it is a hygiene control, not the flood
  control (the per-IP limiter is).
- Session-payload grants grow the `sessions` row (~200–600 B each) and are bounded by the 14-day
  session sweep; existing sessions table is already 300–500 B/row (A7 §7). Fine.
- Moving grants into the session payload means a session-store failure at consume time now fails
  the security action — but the session store is already on every request path; no new SPOF.

---

## 3. Passkey/OAuth finish throttle — verified absent

**Verification:** `grep abuse_limiter::limiter` over all modules returns **zero** hits in
`passkey_v1/`, `google_auth_v1/`, `github_auth_v1/`, `discord_auth_v1/`, `apple_auth_v1/`,
`facebook_auth_v1/` and `services/oauth/`. The only brake on `passkey/v1/*` and `/auth/{provider}/v1/*`
is the 60/min/IP nest limiter (`router.rs:78-104`). `login_finish` (`passkey_v1/controller.rs:213-229`)
and `finish_oauth_login` (`services/oauth/login.rs:303-349`) contain no failure counter.

**What an attacker actually achieves (be precise):**

| Hypothesis                   | Verdict                                                                                                                                                                                                                                                                                                                |
| ---------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Credential probing           | **No.** A passkey assertion is verified against the stored public key; without the private key, every guess fails. Credential ids are opaque and useless alone.                                                                                                                                                        |
| Counter manipulation         | **No.** `finish_login` rejects non-advancing counters atomically (`webauthn.rs:188-202`, `touch_counter_if_unchanged`).                                                                                                                                                                                                |
| Timing oracle                | **Negligible.** A DB miss (unknown credential) vs a crypto failure differs in timing, but reveals only whether an opaque credential id exists — not secret, not exploitable.                                                                                                                                           |
| Account lockout              | **No.** There is no lockout on this path, so it cannot be weaponised for lockout (unlike the password path, E5 #4).                                                                                                                                                                                                    |
| Resource DoS                 | **Yes, mild.** Each unauthenticated `login/finish` costs 1 DB lookup + one signature verification (sub-ms to low-ms); 60/min/IP × N IPs. Not a serious amplifier, but it is unmonitored noise.                                                                                                                         |
| OAuth callback amplification | **Yes, bounded.** A callback with a valid session-bound state but an invalid code makes the server call the provider token endpoint (`google_auth_v1/controller.rs` → token exchange). State issuance is 60/min/IP; each callback burns one state and one outbound HTTPS request. Cost amplification, not auth bypass. |

**Minimum throttle and where:**

1. **Failure-only** counter (the current `abuse_limiter` counts _allowed_ calls too — using it
   verbatim would throttle normal users after 3 successes, E5 #4). Add a `note_failure(store, key,
cfg)` helper that pushes a timestamp, and an entry check via `store.ban_status(key)`. Keys:
   `passkey_finish_fail:{credential_id}` and `passkey_finish_fail:ip:{ip}` on `login_finish`;
   `oauth_cb_fail:ip:{ip}` on every provider callback failure. Threshold: 10 failures / 5 min per
   credential, 20 / 5 min per IP → 429.
2. **Alert on failure spikes** (counter per route) — currently invisible.
3. Optionally require Turnstile on `passkey/v1/login/begin` for cookie-less callers (the mobile
   challenge flood is the map-fill vector, §2).

**Attack the fix:** failure counters are themselves a per-credential DoS vector if they ever block
_valid_ assertions — keep them failure-only and short-windowed, and never let them gate a
successful assertion. Per-IP counters are bypassable with many IPs (spoofing, E5 #1), which is
exactly why the credential-keyed counter matters more.

---

## 4. Recovery chain — enumeration, grants, missing nest limiter

### 4.1 The enumeration step (confirmed)

`POST /forgot_password/v1/request` answers unknown emails with **404 `RecordNotFound` + "No account
exists for that email"** (`forgot_password_v1/controller.rs:343-349`). This is a **deliberate,
test-pinned product decision** (`:314-317` comment; `:541-552` test
`unknown_email_answers_explicit_record_not_found`; no committed doc records the decision, E2 §1.1). Every other auth path masks existence: register masks duplicates
(`auth_v1/controller.rs:519-535`), login uses the dummy verify (`services/auth.rs:86-89`). The
anti-enumeration defence exists everywhere **except here**.

### 4.2 What the chain actually buys the attacker (and what it doesn't)

Chain: enumerate an email → hit `generate` repeatedly → mail flood + reset-flow block.

| Effect                                                         | Real?                                                                                                                                                                                                                                                   | Evidence                                                                                |
| -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Confirm an email is an account (privacy leak, targeting input) | **Yes**                                                                                                                                                                                                                                                 | `:343-349`                                                                              |
| Flood a victim's inbox with reset mail                         | **Yes, throttled**: per-account limiter `forgot_password_request:account:{email}` (2 admitted per 360 s, then 1 h, then 24 h) plus a 60 s `is_in_delay()` send gate (`:326-373`). So ~2–3 mails/account/hour, then the account's reset path is blocked. | `:326-342`, `:357-365`; config `:21-28`                                                 |
| Deny the victim's **password reset** for up to 24 h            | **Yes** — the account key blocks 1 h after 3 attempts and 24 h after ≈8 timed attempts (blocked calls keep appending, `rux-request-gate/src/store.rs:303-320`)                                                                                          | `:337-342`                                                                              |
| Deny the victim's **login**                                    | **No** — separate key (`login:{email}`)                                                                                                                                                                                                                 | key names `auth_v1/controller.rs:134-140` vs `forgot_password_v1/controller.rs:337-342` |
| Burn mail-provider quota / reputation                          | **Yes** — one shared `MailRouter` for transactional + marketing (E5 B3); reset mail competes with verification mail                                                                                                                                     | `main.rs:29-124,648-652`                                                                |
| Timing oracle on unknown emails                                | **Yes, secondary** — unknown emails return fast (no mail send); known emails pay the mail transport. Detectable even when status is masked later.                                                                                                       | `:395-411`                                                                              |
| Scale of enumeration per IP                                    | **~2 requests/6 min then a 1 h block** (per-IP limiter `forgot_password:{ip}`). With E5 #1 (spoofable IP), unlimited.                                                                                                                                   | `:325-333`                                                                              |

### 4.3 Grants and the missing nest limiter (confirmed)

- Reset grants are **process-memory only**: `reset_token` map, TTL 600 s, single-use, bound to
  `session_auth_secret`, with `restore_after_rejection` for a transient password-work 429
  (`forgot_password_v1/controller.rs:30-172`). A restart between `verify` and `reset` silently
  invalidates the token — and the emailed code was already consumed by `verify_code`
  (`db/sea_models/forgot_password/actions.rs:67-78,122-133`), so the user must request a brand-new
  code. Safe-fail, but a user-visible recovery failure on every rolling restart.
- **No nest limiter**: `router.rs:81-83` mounts `/email_verification/v1` and `/forgot_password/v1`
  with **no `rate_limit_layer`** while every other auth nest has one. The only brakes are the
  in-handler abuse limiters. `/email_verification/v1/verify` and `/resend` have no per-IP ceiling at
  all (per-user limiters only, `email_verification_v1/controller.rs:48-49,90-97`).

### 4.4 Fix design

1. **Restore a uniform response for `request`** (200 with the same body for known/unknown, mail only
   for known) — or keep the honest answer but gate it behind Turnstile + a strict per-IP cap. This
   is an undocumented product decision; the audit position: mask it.
2. **Mount `rate_limit_layer` on both nests** (`router.rs:81-83`), matching the rest of the router.
3. **Failure-only counting**: the `verify` limiter currently counts allowed calls (a user who
   mistypes a code 3 times gets a 1 h block; that is intended for brute force but also punishes
   typos). Keep the request limiter as a _send_ budget; make the verify limiter count failures only.
4. **Make reset grants restart-safe**: persist them (or sign a token bound to
   `user_id + session_auth_secret + code-hash + expiry`, single-use enforced by deleting the row).
   This also removes a single-instance dependency (§6).
5. **Per-recipient send budget shared with registration mail** (E5 #6) so the reset path cannot be
   used to burn the provider quota.

**Attack the fix:** masking the response removes a product feature users apparently asked for
("accurate feedback", test-pinned) — needs explicit sign-off; the honest-answer variant keeps the
privacy leak but with a strict cap. A global send budget can be exhausted by an attacker to block
legit recovery mail — mitigate with a separate transactional lane (E5 B3).

---

## 5. Email change — self-lockout, verified (with one nuance E5 missed)

**Verified path, step by step:**

1. `POST /user/v1/update` is `verified`-guarded (`user_v1/mod.rs:14-15`).
2. `update_profile_with_auth` requires **fresh auth** when the email differs
   (`user_v1/controller.rs:228-234`).
3. `User::update` on email change (`db/sea_models/user/actions.rs:142-166`):
   - `is_verified = false`
   - `session_auth_secret` rotated (all sessions die)
   - `email_verification` rows **deleted**
   - `forgot_password` rows deleted
   - **no verification row is created, and no email is sent.**
4. Session death is immediate and total: the extractor compares the session's stored
   `session_auth_hash` with the fresh user row (`rux-auth/src/session/extractor.rs:53-56,248-251`)
   → mismatch → `session.delete()`. The current request returns 200; the **next** request is
   unauthenticated.
5. Recovery is possible only by discovering the resend endpoint: log in again (login does not
   require verification) → `/email_verification/v1/resend` is `unverified`-guarded
   (`email_verification_v1/mod.rs:12`) → the deleted row means the 60 s delay check is skipped
   (`controller.rs:73-88`) → a new code is generated and sent to the **new** address
   (`:99-101`).

**Nuances / additional defects beyond E5:**

- **Typo dead-end (worse than B5 states).** After the change the user is unverified, and
  `/user/v1/update` requires `verified` — so if the new address was mistyped, the user **cannot
  correct it**. The verification code goes to the typo address; only an admin (`admin_update`, which
  also sets `is_verified=false` and sends nothing, `actions.rs:415-429`) or direct DB access can fix
  it. This is a genuine permanent self-lockout for a mistyped email.
- **The web client cannot trigger it today.** `AccountClient.updateProfile` sends only
  `name`/`avatar_id` (`web/src/lib/auth/account-client.ts:224-235`; caller `(account)/account/+page.svelte:96`).
  The backend accepts `email`/`password` in the payload (`user_v1/validator.rs:20-40`), so the bug
  is reachable by direct API/mobile callers and would become reachable in the web UI the moment a
  field is added. (`password` in that payload is silently dropped by `into_update_user` — separate
  minor bug.)
- **No in-flow signal.** The response is a plain user object; nothing tells the client
  "verification required" — a client must infer it from `is_verified=false`.

**Fix:** make email change a verified two-step flow (code to the new address first; only flip the
email + rotate the secret after the code verifies), or at minimum: create the verification row and
send the code to the new address in the same flow (async, like register,
`auth_v1/controller.rs:495-517`), return an explicit `email_verification_required` marker, and allow
correcting the address while unverified (fresh-auth + rate-limited) so a typo is recoverable.
Tradeoff: sending mail to an attacker-chosen address requires the attacker to already hold a fresh
authenticated session (password + TOTP) — at that point they own the account; the change itself is
the risk, not the mail. Rate-limit the send.

---

## 6. Single-instance coupling — full inventory

### 6.1 Process-local state

| State                                                    | Where                                                                                  | Restart (rolling/one replica)                                                                                                                          | Second replica                                                                                                                                              |
| -------------------------------------------------------- | -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `passkey_state` map                                      | `passkey_state.rs:78`                                                                  | In-flight WebAuthn challenges lost → user restarts the flow. Safe-fail.                                                                                | `login/begin` on A, `finish` on B → `InvalidToken`. Flow breaks ~50% of the time.                                                                           |
| `pending_login` grants                                   | `pending_login.rs:44`                                                                  | In-flight TOTP logins lost → "Invalid or expired login session"; user re-logs-in. Safe-fail.                                                           | 2FA login broken whenever begin and finish hit different replicas.                                                                                          |
| `fresh_auth` grants                                      | `fresh_auth.rs:60`                                                                     | Grants lost → security action asks for reauth again. Safe-fail (documented, `deploy/README.md:225-227`).                                               | Security actions randomly demand reauth; never grant wrongly.                                                                                               |
| `oauth` states + native nonces                           | `oauth/state.rs:22-23`                                                                 | In-flight OAuth logins/links fail at the callback. Safe-fail.                                                                                          | Callback on the other replica → `InvalidToken`; link flows can strand identities mid-flow.                                                                  |
| `forgot_password` reset grants                           | `forgot_password_v1/controller.rs:121`                                                 | In-flight reset fails; code already consumed → request a new code. Safe-fail but user-visible.                                                         | `verify` on A, `reset` on B → "invalid or expired". Recovery breaks.                                                                                        |
| `revoked_sessions` HashSet                               | `main.rs:178-179`, `auth.rs:72-74`                                                     | Lost; but `terminate` also deletes the session row (shared SQLite), so revocation mostly survives. The set is never pruned — unbounded (small) growth. | Revocation is per-replica for the window where the row delete failed; normally the shared row delete covers it.                                             |
| Rate limiter L1 (`gate_store`)                           | `main.rs:170-174`; flush `rate_limit_store.rs:30,126-229`                              | L2 restore at boot; **up to 10 s of limiter/ban state lost** on a hard kill.                                                                           | Each replica enforces its own limits and bans: effective limits multiply by replica count, and a ban on A is invisible on B until (if ever) L2 round-trips. |
| Escalation history (`suspicious` map + ban-unit history) | `escalation.rs` (`EscalationEngine`, `max_tracked_identities`)                         | Lost; active bans live in the gate store (L2-flushed every 10 s).                                                                                      | Escalation counts split across replicas → thresholds effectively double; attacker alternates replicas.                                                      |
| Route-blocker snapshot                                   | `route_blocker_service.rs:24` (5 s TTL, re-synced by a 30-min task, `main.rs:693-758`) | Re-synced at boot; safe.                                                                                                                               | Per-replica caches, DB-backed; divergence ≤ TTL/sync window. Low risk.                                                                                      |
| Billing checkout intents                                 | `billing_v1/controller.rs:52`                                                          | In-flight checkout intents lost → webhook cannot grant → payment without entitlement (support burden).                                                 | Checkout on A, webhook on B → grant dropped. **Money-path**, not auth, but part of the invariant.                                                           |
| Device-ban map                                           | **does not exist** (no device ban implementation at HEAD; E5 #3)                       | N/A                                                                                                                                                    | N/A                                                                                                                                                         |
| Static CSRF key, field-crypto key, JWKS caches           | env-derived or DB-derived                                                              | Deterministic/rebuildable                                                                                                                              | Fine                                                                                                                                                        |

### 6.2 Deployment facts

- `docker-compose.yml:118` pins `container_name: ${PROJECT}_api` and there is no
  `deploy.replicas`; `restart: unless-stopped` (`:193`). One `api_data` volume (`:160`), one SQLite
  file, sea_db pool = **1 connection** (A7 §0).
- `main.rs:181-209` runs `reconcile_unbound_sessions` **on every boot**: it revokes unbound audit
  rows and deletes live auth sessions with no durable binding, deliberately making each restart a
  re-authentication boundary. With a second replica, a booting B could delete sessions created by A
  in the narrow window before their binding row commits — and every boot is a boundary by design, so
  this cannot be run by more than one instance without a leader lock.
- `deploy/README.md:225-227` admits the constraint in prose: "Sessions carrying in-memory proof must
  reach the same API process; a restart safely requires confirmation again."

### 6.3 Honest conclusion

**The app is structurally single-instance, and that is currently an accepted, documented design
point — but it is a hard invariant, not a preference.** Three independent couplings make replica
count > 1 incorrect today: (a) the process-global auth maps, (b) the boot reconcile treating every
boot as _the_ re-authentication boundary, (c) a single SQLite file with a 1-connection pool shared
by all state. Rolling restart is _safe-fail_ for security (no grant is wrongly accepted) but
breaks in-flight passkey/2FA/OAuth/recovery flows and the money path's checkout intents.

**What the plan must say:**

1. Replica count is **1**. HA is restart, not scale-out. State it in `deploy/README.md` next to the
   fresh-auth note, not just in prose.
2. Any scale-out requires, in order: session-payload grants for fresh-auth/pending-login/OAuth
   (removes most of (a)), persisted reset grants, a leader-locked reconcile (removes (b)), and a
   shared database (removes (c)). Do not add replica 2 before all four.
3. Add a boot-time log/metric with an instance identifier and an explicit "single-instance mode"
   banner so an accidental scale-out is visible.
4. Move the in-memory-proof note from prose to a tested invariant where possible: e.g. a startup
   assertion that `reconcile_unbound_sessions` is only run when a deployment-wide flag/lock is held.

---

## 7. Attack your own fixes (consolidated)

| Fix                                        | New problem                                                                                                   | Mitigation                                                                                |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| Queue + timeout on password work           | Waiters hold tasks/sockets; flood fills the queue; login still 429s under sustained load                      | Global waiter cap; reserved lanes; telemetry                                              |
| Dummy lane                                 | +19–38 MiB worst case; unknown-email latency degrades under a dummy-only flood                                | Make lane size env-tunable (0 = old behavior); document memory math                       |
| Reserved authenticated lane                | Wasted slot when idle                                                                                         | Accept; it is 1 slot                                                                      |
| Per-identity in-flight cap                 | PII map needs TTL/eviction                                                                                    | Bound + TTL + no logging of keys                                                          |
| Map: refuse-at-cap instead of evict-oldest | New passkey/OAuth begins 429 during a flood                                                                   | Correct tradeoff (in-flight state > new state); per-IP limiter already present; telemetry |
| Session-payload grants                     | Session rows grow; consume now depends on session write                                                       | Session store is already on every request path; +200–600 B/row                            |
| Failure-only finish throttle               | A bad threshold could block valid assertions; per-IP counters are bypassable                                  | Credential-keyed, failure-only, short window, never gate success                          |
| Uniform recovery response                  | Removes a product feature users asked for; needs sign-off                                                     | Turnstile-gated honest variant as fallback                                                |
| Email change sends code                    | Attacker with fresh auth can aim mail at an arbitrary address (they already own the account); extra mail cost | Rate-limit; two-step change is strictly better                                            |
| Signed reset/native nonce                  | Loses strict single-use (replay within TTL)                                                                   | Keep a small consumed set, or keep the map with no-cross-eviction                         |

---

## 8. Ranking and minimum fix set

### Ranking (realistic exploitability × impact)

| #   | Finding                                                   | Severity                     | Exploitability                                                      | Impact                                                                    |
| --- | --------------------------------------------------------- | ---------------------------- | ------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| 1   | Password-work fail-fast saturation                        | **HIGH**                     | 20–48 IPs or 1 spoofed IP; ~33–80 req/s; no account                 | Total auth outage: login, register, reauth, reset, 2FA                    |
| 2   | Email-change self-lockout (+typo dead-end)                | **HIGH (user-impact)**       | Any direct API client; web UI not yet wired                         | Locked out of verified routes, sessions killed, no code, typo = permanent |
| 3   | Recovery enumeration oracle + mail chain                  | **MEDIUM-HIGH**              | 2 req/6 min per IP, unlimited with E5 #1                            | Privacy leak; targeted reset DoS; mail-quota burn                         |
| 4   | Map cross-session eviction (passkey/OAuth)                | **MEDIUM-LOW (conditional)** | ~300+ IPs for fresh challenges; ~34 IPs to fill; trivial with E5 #1 | Breaks other users' in-flight logins; single-instance coupling            |
| 5   | Single-instance invariant undocumented as hard constraint | **MEDIUM (plan risk)**       | N/A (deployment decision)                                           | Scale-out silently breaks auth/recovery/money flows                       |
| 6   | No per-account finish throttle (passkey/OAuth)            | **LOW-MEDIUM**               | Trivial, but crypto-bound                                           | Noise/unmonitored grinding; OAuth outbound amplification                  |
| 7   | Missing nest limiters on forgot/verify nests              | **LOW**                      | Trivial                                                             | Removes a defence-in-depth layer                                          |
| 8   | pending/fresh-auth fail-closed 429 at cap                 | **LOW**                      | Not practical to fill                                               | Latent global 2FA/fresh-auth denial if ever filled                        |
| 9   | INTENT_STORE uncapped                                     | **LOW**                      | Not unauthenticated                                                 | Memory hygiene                                                            |

### Minimum fix set (ordered, smallest correct change per problem)

1. **Password-work admission redesign** (§1.7): queue-with-timeout + waiter cap + dummy lane +
   reserved authenticated lane + telemetry. Keeps the documented memory bound; removes the
   2-request cliff. _This is the single highest-leverage code change._
2. **Email change**: two-step verify-before-switch (or code-on-change + explicit response marker +
   typo correction path). _Highest user-facing impact._
3. **Recovery**: mask the unknown-email response (or Turnstile-gate it) + mount nest limiters +
   failure-only verify counting + restart-safe reset grants.
4. **Maps**: no-cross-session eviction + per-session quota + telemetry for passkey/OAuth; move
   fresh-auth/pending-login/OAuth state into the session payload (kills single-instance coupling for
   those flows).
5. **Single-instance invariant**: write it into `deploy/README.md` + a boot banner/metric; document
   the four preconditions for replica 2.
6. **Finish throttles**: failure-only per-credential/per-IP counters + alerts.

---

## Findings (numbered, NEW)

### F2-01 — NEW — `PASSWORD_WORK` fail-fast semaphore (2 slots) is a 2-request global auth outage — HIGH

**Evidence:** `rust/backend/api/src/services/password_work.rs:8-15` (default 2, env 1..=32),
`:56-58` (`try_acquire_owned` → Busy), `:25-32` (Busy → 429, no Retry-After); shared by
`services/auth.rs:86-89,115,136` (dummy verify — no account needed), `:143` (login),
`db/sea_models/user/actions.rs:81-84,216-218,355-358,440-443` (register/change/admin),
`auth_v1/controller.rs:616,730-734,882-886` (backup codes, 10 Argon2 ops per permit). Argon2
default m=19 MiB t=2 p=1 (`argon2-0.5.3/src/params.rs:42,52,61`; measured p50 11.6 ms locally,
VPS est. 25–60 ms). Documented intent: `git show bed80b13:docs/security-hardening-2026-10-11.md`, `deploy/README.md:221-222`.
**Fix:** queue-with-timeout (1.5 s) + global waiter cap + separate dummy lane + reserved
authenticated lane + saturation telemetry (see §1.7). Keep the memory bound.

### F2-02 — NEW — Email change sets `is_verified=false`, rotates the session secret, deletes the verification row, and sends no code — HIGH

**Evidence:** `db/sea_models/user/actions.rs:142-166`; guard `user_v1/mod.rs:14-15`; fresh-auth gate
`user_v1/controller.rs:228-234`; session kill `rux-auth/src/session/extractor.rs:53-56,248-251`;
recovery only via `email_verification_v1/controller.rs:73-101` (row deleted → delay skipped → resend
works); typo dead-end because `update` requires `verified`. Web client sends only `name`
(`web/src/lib/auth/account-client.ts:224-235`).
**Fix:** two-step change (verify new address before flipping) or code-on-change + explicit
`email_verification_required` response + typo-correction path while unverified.

### F2-03 — NEW — `forgot_password` is an explicit account-existence oracle with no nest limiter and an in-memory single-use grant — MEDIUM-HIGH

**Evidence:** `forgot_password_v1/controller.rs:343-349` (404 + explicit message), test-pinned
`:541-552`, product comment `:314-317`; no nest limiter `router.rs:81-83`; per-IP/per-account
limiters `:326-342` (2 admitted per 360 s); reset grants process-local `:30-172` (TTL 600 s).
**Fix:** uniform response (or Turnstile-gated honest answer), mount `rate_limit_layer`, failure-only
verify counting, persist/sign reset grants.

### F2-04 — NEW — Process-global passkey/OAuth maps evict the oldest entry, i.e. other users' in-flight challenges — MEDIUM-LOW (conditional)

**Evidence:** `services/passkey_state.rs:14-15,84-86,104-112`; `services/oauth/state.rs:15-18,
46-52,71-79,142-150`; issuance reachable pre-auth `passkey_v1/mod.rs:17-19`,
`passkey_v1/controller.rs:190-209`; 60/min/IP `router.rs:78-104`. Fill math: ~34 IPs to fill 10k in
300 s; ~333 IPs to break a 30 s-old challenge (or one spoofed IP per E5 #1).
**Fix:** reap → evict only the requesting session's oldest entry → else 429; per-session quota;
telemetry. Keep remove-on-read single-use (`passkey_state.rs:117-120`).

### F2-05 — NEW — `fresh_auth`, `pending_login` and OAuth state are process-local, hard-wiring replica count 1 — MEDIUM (plan risk)

**Evidence:** `fresh_auth.rs:60`; `pending_login.rs:44`; `oauth/state.rs:22-23`;
`deploy/README.md:225-227`; `main.rs:181-209` (boot reconcile deletes unbound sessions, assumes it
is the only instance); `docker-compose.yml:118,193` (fixed `container_name`, no replicas).
**Fix:** move the three session-bound grants into the session payload (signed, single-use via
session delete); persist reset grants; leader-lock the boot reconcile; document replica 1.

### F2-06 — NEW — No per-account failure throttle on passkey/OAuth finish paths — LOW-MEDIUM

**Evidence:** no `abuse_limiter` call in `passkey_v1/`, any `*_auth_v1/` or `services/oauth/`;
`passkey_v1/controller.rs:213-229,232-296`; `services/oauth/login.rs:303-349`. Counter/replay
controls are sound (`webauthn.rs:188-202`); impact is unmonitored grinding + OAuth outbound
amplification, not probing/lockout.
**Fix:** failure-only counters keyed by credential id and IP (10/5 min, 20/5 min) + alerts, placed
before/after the finish verification; never gate a successful assertion.

### F2-07 — NEW — `password_work` has zero telemetry; saturation is invisible — LOW-MEDIUM

**Evidence:** `services/password_work.rs` (no metrics imports); `utils/telemetry.rs` has limiter
metrics but no password-work instruments; contrast `abuse_limiter.rs:11-32`.
**Fix:** saturated counter, wait histogram, in-flight/waiter gauges; alert on sustained saturation.

### F2-08 — NEW — Nest-level rate limiter missing on `/forgot_password/v1` and `/email_verification/v1` — LOW

**Evidence:** `router.rs:81-83` vs every other nest (`:73-104`); `/email_verification/v1/verify`
and `/resend` have only per-user limiters (`email_verification_v1/controller.rs:48-49,90-97`).
**Fix:** mount `rate_limit_layer(&state, 60, 60)` on both nests.

### F2-09 — NEW — `pending_login` and `fresh_auth` fail closed with 429 at a 10k cap; no admission telemetry — LOW

**Evidence:** `pending_login.rs:85-87`; `fresh_auth.rs:100-102,176-178`. Filling requires thousands
of completed password/TOTP or reauth flows within the TTL, so not practical today; the residual
risk is a silent global 429 if it ever fills.
**Fix:** telemetry + map-size alert; the session-payload move (F2-05) removes the cap entirely.

### F2-10 — NEW — `billing_v1` checkout-intent map is uncapped and process-local — LOW

**Evidence:** `billing_v1/controller.rs:50-74` (TTL 1 h, no max, reap-on-access only); single
instance assumption shared with F2-05.
**Fix:** add a cap with reject + telemetry; fold into the F2-05 replica-2 precondition list.

### F2-11 — NEW — `revoked_sessions` HashSet is never pruned — LOW

**Evidence:** `services/auth.rs:72-74` (insert only), `:583-595` (read only); callers
`auth_v1/controller.rs:1070`. Entries are tower session ids (~43 chars) and are lost at restart.
**Fix:** bound the set (TTL/LRU) or note that the durable session delete makes the set a short-term
belt-and-braces only; add a size gauge.

### F2-12 — NEW — `User::update` silently drops `password` from the profile-update payload — LOW

**Evidence:** `user_v1/validator.rs:20-40` accepts `password`; `into_update_user` (`:30-39`) omits
it; `actions.rs:123-185` never reads it. A client that "changes" the password through this endpoint
gets a 200 and no change.
**Fix:** reject `password` on this endpoint (validation error) or route it to
`change_password` with fresh auth.

---

## Appendix — verification commands used

- Read at HEAD: `password_work.rs`, `auth.rs`, `passkey_state.rs`, `pending_login.rs`,
  `fresh_auth.rs`, `oauth/state.rs`, `oauth/login.rs`, `passkey_v1/controller.rs`,
  `forgot_password_v1/controller.rs`, `email_verification_v1/controller.rs`,
  `user_v1/controller.rs`, `user_v1/mod.rs`, `validator.rs`, `db/sea_models/user/actions.rs`,
  `db/sea_models/forgot_password/actions.rs`, `auth_v1/controller.rs`, `router.rs`, `main.rs`,
  `rate_limit.rs`, `abuse_limiter.rs`, `rux-request-gate/src/store.rs`,
  `rux-auth/src/session/{extractor,state}.rs`, `middleware/guard.rs`, `webauthn.rs`, `twofa.rs`,
  `docker-compose.yml`, `deploy/README.md`, `.env.example`.
- Greps: `password_work|PASSWORD_WORK`, `abuse_limiter::limiter` across modules,
  `OnceLock|LazyLock|static .*Mutex`, `.terminate(`, `PASSWORD_WORK_CONCURRENCY` repo-wide.
- History: `git show bed80b13:docs/security-hardening-2026-10-11.md` (the admission rationale);
  `git cat-file -e HEAD:docs/security-hardening-2026-10-11.md` (absent at HEAD).
- Measurement: standalone release build, `argon2 0.5.3` default params, 20 iterations:
  p50 11.6 ms / p95 17.5 ms (this Mac). VPS numbers are scaled estimates, labelled.
