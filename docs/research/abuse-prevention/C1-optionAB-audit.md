# C1 — Adversarial audit of the recommended A+B plan (the round-4 convergence)

Auditor: C1, adversarial pass. Date: 2026-10-12.
Target: the recommendation three round-4 reports converged on — **Option A now, Option B next, C deferred**
(`B2-contrarian.md` §5; `B9-second-order.md` finding 12; `B10-falsification.md` §7).
Method: read the eight named reports, then re-verify every load-bearing repo claim at HEAD
(`/Users/hmziq/os/easyquran`), then attack A and B exactly as round 3 attacked the fingerprint design —
assume each measure is implemented **as written** and ask what it breaks, what it leaves open, and what
an attacker does next. Every repo citation below is a `file:line` read in this session.

**Verdict up front.** The A+B _direction_ is right and the three reports are right to converge on it: the
account/credential axis is where the enforceable abuse reduction lives, and the device token is the only
durable identifier worth having. But the plan as written has **four load-bearing defects** that the
round-4 reports did not catch, each of which changes the work:

1. The login-limiter "fix" is under-specified and, implemented naively (failures-only), still leaves a
   **credential-level lockout DoS** — the correct design puts the _penalty_ on the source IP and the
   _challenge + notification_ on the credential, and never blocks on the credential.
2. Registration admission control, as costed, **cannot** stop mass creation against a residential proxy
   pool, and the one cheap win nobody found is a **Gmail dot-alias bypass** of the existing alias policy
   (one inbox → unlimited verified accounts) that makes every per-domain/disposable-list control moot.
3. Option B's token is described as a limiter; as a _security_ mechanism its real properties are
   different — it is a **second 12-month bearer identifier with no authentication power** whose only
   defensible product is a **low-collision linkage edge for evidence**, and its "one token = one device"
   claim fails on shared browsers exactly where the fingerprint's FP problem lives.
4. Ban wiring is not a 2–3 day isolated module: **nothing revokes sessions today**, the ban bites with a
   ≤5-min lag through the session cache, the client shows a banned user "Email or password is incorrect",
   and shipping a ban without notice + appeal turns a 3-line admin route into a silent permanent lockout.

Corrected estimate: **A core ≈ 12–17 dev-days, B ≈ 9–14** (vs B2's 8–12 + 6–10), plus 1–2 policy days.
Corrected PR order in §8; abort gate in §8.3.

---

## 0. Scope note

The eight reports named in the brief are `B2`, `B9`, `B10`, `B3`, `B4`, `B5`, `B8`, `A8`; all exist
and were read in full.
Where a finding here **confirms** an existing report it is labelled _confirms_; where it is new it is
labelled **NEW**. The task's "mark NEW" is honoured per finding in §9.

---

## 1. Verification of B2's repo claims (the ground truth for the costing)

| #   | B2 claim                                                                                                              | Verdict                                                | Evidence (read this session)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| --- | --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `login:<normalized_email>` throttle exists, runs before password verification, 3/360 s → 1 h; 5/900 s → 24 h          | **CONFIRMED**                                          | `rust/backend/api/src/modules/auth_v1/controller.rs:31-38` (config), `:133-140` (key + limiter, before `authenticate_password` at `:142-145`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| 2   | The limiter counts **every** attempt, success included; nothing clears on success                                     | **CONFIRMED**                                          | `crates/rux-request-gate/src/store.rs:322` (unconditional `attempts.push_back` on the allowed path), `:305` (also appends on the blocked path); `crates/rux-request-gate/src/abuse.rs:64-125`; `clear_limit` has **no** auth caller — only `admin_bans_v1/controller.rs:152-153,448-449,545-546`                                                                                                                                                                                                                                                                                                                              |
| 3   | The counter design: temp block at 3/360 s, long at 5/900 s                                                            | **CONFIRMED with a correction**                        | `store.rs:326-343`. The **long tier is not reachable by "5 requests" alone**: while a temp block is live, every request returns early at `:303-315` and never evaluates the long threshold; the 24 h block fires only when the block expires and ≥5 attempts remain in the trailing 900 s — i.e. from attempts accumulated **during** the block. The practical weapon is therefore: 3 requests → 1 h block → the victim's own retries append attempts → one retry after expiry → 24 h. (§2.2)                                                                                                                                 |
| 4   | Router limit table: auth 100/min, post 200, comment 100, user/billing/oauth 60, media/search 30, quran 600            | **CONFIRMED**                                          | `src/router.rs:72-76` (auth 100), `:110-113` (user 60), `:115-118` (post 200), `:120-123` (comment 100), `:129-132` (media 30), `:134-137` (search 30), `:160-177` (billing/device/notification/bookmark 60/100); `src/main.rs:897-909` (quran content 600, search 30); `src/middlewares/rate_limit.rs:57-79`                                                                                                                                                                                                                                                                                                                 |
| 5   | Registration has no per-identity limiter; only Turnstile; Turnstile is "a no-op unless `TURNSTILE_SECRET_KEY` is set" | **CONFIRMED on the first half; REFUTED on the second** | `auth_v1/controller.rs:459-535` — no `abuse_limiter` call; only `turnstile::verify_registration` at `:470-474`. Turnstile: `services/turnstile.rs:17-19` (`turnstile_enabled()` is true in prod even unconfigured), `:42-47` (prod + no secret → `captcha_unavailable()` = **503**; dev + no secret → `Ok`). So in production registration is either **Turnstile-gated** or **entirely broken (503)** — never a silent no-op. The web gates its widget on `PUBLIC_TURNSTILE_SITE_KEY` (`web/src/lib/auth/turnstile.ts:4-7`, `RegisterForm.svelte:51-55`), so an API secret without a web site key = registration dead. (§3.1) |
| 6   | `user_bans` fully dormant: zero writers, no admin route; `check_ban` → `not_banned()` wired                           | **CONFIRMED**                                          | `db/sea_models/user_ban/slice.rs:5-43` (`NewUserBan`, `permanent()`), `actions.rs:11-29,31-54,56-71`; grep `NewUserBan` → only `slice.rs:12`; `services/auth.rs:530-547`; `crates/rux-auth/src/middleware/guard.rs:46-56`                                                                                                                                                                                                                                                                                                                                                                                                     |
| 7   | 5-minute ban cache in `AuthSessionState`                                                                              | **CONFIRMED**                                          | `crates/rux-auth/src/requirements/builder.rs:16` (`Duration::minutes(5)`), `crates/rux-auth/src/session/state.rs:50-63` (`ban_checked_at`, `ban_cache_stale`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| 8   | The client misclassifies 403 AccountLocked as "Email or password is incorrect"                                        | **CONFIRMED, with the exact mechanism**                | `crates/ruxlog-types/src/error.rs:20-21` (AccountLocked = AUTH_006), `:231` (403); `web/src/lib/auth/auth-copy.ts:80-88` — **AUTH_006 is in `CREDENTIAL_TYPES`** and 403 is in `CREDENTIAL_STATUSES`, `:137-139` → `CREDENTIAL_FAILURE`; consumed at `web/src/lib/auth/flows.svelte.ts:118-134`                                                                                                                                                                                                                                                                                                                               |
| 9   | The limiter is skipped outside production; fails closed on store error                                                | **CONFIRMED**                                          | `services/abuse_limiter.rs:47-49` (non-prod `Ok(())`), `:69-73` (503); `crates/rux-request-gate/src/abuse.rs:215-218`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| 10  | Other per-identity keys exist (`totp`, `reauth`, forgot-password, email-verify, comment)                              | **CONFIRMED**                                          | grep `abuse_limiter::limiter` → `auth_v1/controller.rs:135,299,411,443,582,813,918`; `forgot_password_v1/controller.rs:327,337,430,455,503`; `email_verification_v1/controller.rs:49,91`; `post_comment_v1/controller.rs:62`                                                                                                                                                                                                                                                                                                                                                                                                  |

**Two facts B2 did not state that change the plan:**

- **`user_bans` has no index.** `migration/src/m000001_init.rs:58-73` creates the table with FKs and no
  `CREATE INDEX` (repo-wide grep: the only `user_bans` matches are the CREATE at `:58` and the DROP at
  `:340`). `get_active_ban` filters `user_id + revoked_at + expires_at` (`actions.rs:59-65`) — a full scan
  on a pool-of-1 SQLite connection, executed at every login **and** for every authenticated user every
  5 minutes once the guard's cache expires. Ban wiring must add the index in the same migration. **NEW**
- **The alias policy does not canonicalize dots.** `utils/email.rs:7-9` normalizes trim+lowercase only;
  `utils/email_alias.rs:8-18` rejects only `+` in the local part, and only when
  `EMAIL_ALIASES_ENABLED` is off (prod default off — `config/settings.rs:48-53`). `a.b@gmail.com`,
  `ab@gmail.com`, `a..b@gmail.com` are three distinct accounts that all deliver to one inbox. (§3.2)

---

## 2. Attack the login-limiter fix

### 2.1 What the current code actually does (end to end)

```
log_in (auth_v1/controller.rs:122)
  └─ abuse_limiter::limiter(gate_store, "login:<trim+lowercase(email)>", 3/360→1h, 5/900→24h)   :133-140
       └─ rux_request_gate::check → store.abuse_check
            ├─ blocked branch:  attempts.push_back(now); return Blocked                   store.rs:303-315
            └─ else:            attempts.push_back(now); evaluate 3/360 then 5/900        store.rs:322-349
  └─ authenticate_password                                                                   :142-145
  └─ check_ban → AccountLocked 403 (only with a correct password)                            :153-166
```

Consequences, all verified:

1. **Successful logins count.** Three logins by one human inside 6 minutes (phone + laptop + tablet, or
   a shared family account) → 1-hour block. A family sharing one account trips it in normal use.
2. **The block is on the credential, not the source.** Any anonymous POST with a known email consumes
   the victim's budget. There is no per-IP penalty for the _attacker_.
3. **The blocked path appends attempts** (`store.rs:305`). Retrying during the block is not neutral — it
   is evidence. Combined with the long tier (`:335-343`), a victim's own retries can escalate to 24 h.
4. **The ban check is after the limiter** (`:133` before `:153`), so a banned user's attempts also burn
   the credential's budget and eventually produce 429 (AUTH_007) instead of the ban message.
5. **The same success-counting bug exists on `totp:<user_id>` (`:299`), `reauth:<user_id>` (`:411`),
   `totp_setup:<user_id>` (`:582`), `email_verify:<user_id>` (`email_verification_v1:49`) and
   `forgot_password_*:account` (`forgot_password_v1:337,503`)** — every one of them increments before the
   thing it is guarding, and none clears on success. Fixing only `login:` leaves five clones.

### 2.2 The lockout DoS, precisely

- **Cheap tier (confirmed):** 3 requests → 1 h credential block. No credential, no cost. This alone is a
  free denial-of-login for any known email (all of them are enumerable: the register endpoint answers
  duplicates with the same success envelope — `auth_v1/controller.rs:519-525` — so existence is _masked_,
  but the lockout does not require knowing that an account exists: the block is created regardless).
- **Expensive tier (corrected):** 5 requests → 24 h is **not** one-shot. It requires attempts _inside_
  the 900 s window when a temp block expires. An attacker can force it: send 3 (temp block), wait until the last 15 min
  of the block, send 4 more (blocked but appended), then one request after expiry → long block (≈8 timed
  requests; 3 + 3 + 1 leaves `long_count = 4`, one short). Or simply let the
  victim's retries do it. So the 24 h weapon exists, but the mechanism is the **blocked-path append**, and
  a fix that only switches to "failures-only" without touching `store.rs:303-315` leaves the escalation
  intact (a blocked request is not a password failure, yet it counts as one).

### 2.3 The correct counter design

**Principle: detect on the credential, penalize the source, challenge the credential, never lock it.**

| Key                                 | Window | Threshold   | Action                                                             | Cleared on success?                           |
| ----------------------------------- | ------ | ----------- | ------------------------------------------------------------------ | --------------------------------------------- |
| `login_fail:<email>` (detection)    | 600 s  | 5 failures  | **require Turnstile** on the next attempt + `notify owner`         | **yes** — `del` on success                    |
| `login_fail:<email>` (attack state) | 3600 s | 10 failures | keep the challenge for 24 h, notify, surface in metrics            | yes                                           |
| `login_fail_ip:<ip_unit>` (penalty) | 600 s  | 20 failures | **429, `Retry-After: 900`** — the attacker's own source is delayed | **no** (must not clear on any single success) |
| `login_fail_pair:<email>:<ip_unit>` | 600 s  | 5 failures  | 429 for that pair only                                             | no                                            |

Rules that make it safe:

1. **Never a 429 on the credential.** A credential-level lock is a DoS primitive by construction; a
   challenge is passable by the legitimate owner and expensive for the attacker. The only block is on the
   observable the attacker owns (IP unit), where the collateral is CGNAT — hence the high threshold and
   the 15-minute cap, not 24 h.
2. **Peek before, record after.** The pre-check must be read-only (`abuse_peek`), the increment must
   happen on the failure path only, and the blocked path must not append (fix `store.rs:305`). Exact
   store change: split `abuse_check` into `abuse_peek(key, cfg) -> LimiterDecision` (no mutation) and
   `abuse_record(key, cfg) -> LimiterDecision` (append + evaluate); keep `abuse_check` as
   `record` for the existing non-auth callers (comments, resends) where counting throughput is intended.
3. **Success clears the credential key only.** `store.del("login_fail:<email>")`; never clear the IP key
   (otherwise the attacker logs into their own account between attempts to reset the counter).
4. **Never escalate a credential block.** The long tier is deleted for credential keys; keep the two-tier
   escalation only for `login_fail_ip` (and even there cap at 1 h).
5. **Ban check moves before the limiter** (or banned attempts skip the counter): a banned user retrying
   should get the ban message, not contribute to a lockout they cannot see.
6. **The challenge must be satisfiable everywhere** (censored networks, Tor): Turnstile → email OTP →
   progressive delay fallback, exactly the semantics `B5-shared-devices.md` §4.3 specifies for the device
   path; today `turnstile.rs` is fail-closed (`:91-94`) which is correct for signup and wrong here.
7. Apply the same split to `totp`, `reauth`, `totp_setup`, `email_verify` — or at minimum stop counting
   successes on them (they are all the same bug).

**Residual after the fix, stated honestly.** An attacker with rotating IPs can still force the _challenge_
state on a victim's credential (5 failures → Turnstile required for 24 h). That is an annoyance, not a
lockout: the owner passes the challenge, is notified, and can log in. A determined attacker can also
exhaust `login_fail_ip` for a shared IP (CGNAT) — accepted residual, bounded by the 15-min window and the
existing allowlist pattern. What is _eliminated_ is the free 1 h / 24 h credential lockout, which is the
availability bug.

---

## 3. Attack registration admission control

### 3.1 What exists (verified)

- No per-identity limiter anywhere in the register handler (`auth_v1/controller.rs:459-535`).
- One shared per-IP, per-matched-route bucket: 100/min (`router.rs:72-76` + `rate_limit.rs` Matched
  keying) → **144,000 registrations/day/IP**.
- Turnstile: `verify_registration` is the only entry point (`turnstile.rs:38-56`); it is **fail-closed in
  production** (503 when unconfigured, `:42-47`) and the web only renders the widget when
  `PUBLIC_TURNSTILE_SITE_KEY` is set (`web/src/lib/auth/turnstile.ts:4-7`, `RegisterForm.svelte:51-55`).
- CSRF is required (`static_csrf.rs:88-141`; register is not in the exempt list `:56-85`), so a script
  needs one `POST /csrf/v1/generate` bootstrap per session — one extra request, no barrier.
- Duplicate emails are masked as success (`auth_v1/controller.rs:519-525`), so no enumeration leak, but
  also no signal.

### 3.2 The bypass nobody found: Gmail dot-aliases (NEW)

`EMAIL_ALIASES_ENABLED` (prod default off, `settings.rs:48-53`) rejects `+` tags only
(`email_alias.rs:8-18`). `normalize_email` is trim+lowercase (`email.rs:7-9`), so **dots are not
canonicalized**: `a.b@gmail.com`, `ab@gmail.com`, `a..b@gmail.com`, `a.b.c@gmail.com` are distinct
accounts (`find_by_email` uses the same normalization, `user/actions.rs:280`) and all deliver to one
Gmail inbox. One real inbox therefore yields an unbounded number of **verified** accounts (the
verification email reaches the same inbox), defeating:

- any per-email-domain cap (same domain),
- any disposable-domain list (gmail.com is never on it),
- any per-inbox cap (the inbox is not identified),
- the "verified email raises the cost to one inbox per account" claim (B2 §2, A2/A8's costing) — it is
  one inbox for _n_ accounts.

Fix: canonicalize the **verification identity** (not the stored address) for the known dot-insensitive
providers (gmail.com/googlemail.com: strip dots in the local part; strip `+tag` where the provider
supports it). Store a `verification_identity_hash` on the user row (HMAC, `secret_key`-derived, same
pattern as `code_hash`) and cap distinct accounts per identity (e.g. 3 pending/unverified, 5 lifetime),
and feed it into the registration burst rule. This is ~60 LOC and is the only control in the list that
actually touches the real cost model.

### 3.3 What per-IP and per-domain controls break (the FP classes)

| Control                    | Legitimate population it hits                                                                                                                                           | Severity                                                                                                                            |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| Per-IP/day cap (e.g. 5–20) | **CGNAT mobile** (thousands of subscribers per public IP — the app's core audience), **school/madrasa labs**, cyber-cafés, corporate NAT, family sharing one connection | High — this is the same population `A2`/`B5` protect everywhere else; a hard cap is the school-block failure A2 A2(ii) warned about |
| Per-ASN/day cap            | Mobile carriers (one ASN = a country), large ISPs; and it is bypassable by the 44% of residential-proxy IPs that span providers                                         | Medium — useful as a _challenge_ trigger, never a block                                                                             |
| Per-email-domain/day       | Schools/companies on one Workspace domain; and **useless** for the attacker who uses gmail.com — while a gmail.com cap would be a global ceiling                        | High FP + zero evasion value; only a disposable-domain _list_ is defensible                                                         |
| Turnstile required         | Censored networks where `challenges.cloudflare.com` is blocked/throttled; low-literacy users; Tor                                                                       | Medium — needs the §4.3 fallback ladder, not a wall                                                                                 |
| Invite-only                | Growth; it is an emergency switch, not a control                                                                                                                        | n/a                                                                                                                                 |

### 3.4 The exact control, and its residual

Ship, in order of effect-per-day:

1. **Turnstile enforced in production with a boot/deploy check that the web site key is present** (a
   smoke test that registers a user with a valid token in staging; a boot log line is not enough). Cost:
   0.5–1 d, mostly deploy wiring. Abandonment: measure — the widget is on the _soft_ path (first 3
   signups/IP/24 h pass without a challenge if the widget is invisible/managed; the challenge is required
   only past the soft cap or on the existing always-on prod policy).
2. **Per-IP soft cap:** 3 registrations/24 h without challenge, 5 → Turnstile, 20 → 24 h block; plus the
   existing `QURAN_BAN_ALLOWLIST`-style CIDR allowlist for known schools. In-memory, non-persisted (the
   `B3` §7.2/§7.3 discipline: never in `snapshot()`, never escalatable).
3. **Per-ASN soft cap:** 50/24 h → challenge only. (Requires the ASN dimension from `B3` §7.2; if the
   Worker is not built, a baked IP→ASN table is the alternative. If neither ships, skip this item.)
4. **Verification-identity cap** (§3.2) — the only control that survives proxy rotation, because it caps
   _inboxes_, not networks.
5. **Fresh-account privilege limits** (the actual abuse lever): accounts <24 h get 1 comment / no links /
   no media / no profile writes; everything else queued for moderation. This is where the harm reduction
   is, not in the signup wall — B2's own ranking ("registration ≥ ban wiring > limiter fix") is right
   about _naive_ attackers and wrong about a funded one.
6. **Invite mode** (A9 in B2's list) as a config flag only, off by default.

**Residual mass-creation capacity, honestly.** With per-IP 5 + per-ASN 50 + Turnstile: a residential pool
of 10 k IPs across many ASNs yields ~10⁵ accounts/day at a proxy cost of cents per account; free inboxes
and the dot-alias trick make verification free. **Admission control raises the floor from "free" to
"cheap" — it does not close it.** The things that actually bound the damage are (a) fresh-account
privilege limits, (b) moderation, (c) ban wiring, (d) the verification-identity cap. Any plan that
budgets "1–2 days for registration admission" and expects the substrate to disappear is over-crediting
the control; the honest statement is "removes the naive 144 k/day/IP case and makes bulk creation
visible in metrics".

---

## 4. Attack Option B's device token as a security mechanism

Option B as defined: server-minted opaque token (≥128-bit), `HttpOnly; Secure; SameSite=Lax; Path=/api`
cookie, bound at login/register, no component fingerprint; token-keyed limits; account-linked device list;
ban-evasion evidence via token reuse across accounts.

### 4.1 Token theft — HttpOnly does not mean "not usable"

HttpOnly prevents `document.cookie` reads. It does **not** prevent **use**: any script executing on the
origin can `fetch("/api/...")` and the browser attaches the cookie automatically. The repo's CSP trusts
three third-party script origins (`gstatic`, `googletagmanager`, `challenges.cloudflare.com` — B10 §1.10
verified, Firebase Analytics is live), and XSS anywhere on the origin yields full token use. So:

- **Against XSS, B adds nothing over the session cookie.** The session cookie is HttpOnly too
  (`main.rs:781-788`), same theft properties, same CSP dependency.
- **B adds a second, longer-lived credential.** The token lives ~12 months (B2 B1/B2 policy: "token 12 mo,
  links 12 mo"); the session expires after 14 days of inactivity (`main.rs:782`). An XSS or a shared-computer visitor who
  captures the token holds a 12-month identifier that survives logout and password change.
- **Consequence is attribution poisoning, not account takeover** — today. The token is not an auth
  credential: stealing it does not grant access. But it _does_ let the holder make the victim's device
  appear on the attacker's accounts (`device_account_links`), i.e. it can **frame the victim's device**
  and consume its evidence slot. If any future feature makes the token enforcement-bearing (a device
  ban, a quota), theft becomes account-affecting. Write this as a normative constraint now: **the token
  must never gate a privilege** — the same rule B9 imposed on the fingerprint's device layer.
- **XSS is not the only capture path:** a shared/library computer (the attacker uses the victim's browser
  with the victim's token present), a malicious extension, or a network attacker on plaintext (mitigated
  by Secure + HSTS).

### 4.2 "One token = one device" — false on exactly the population that matters

The token is per **browser profile**, not per device or person:

- A family phone with one browser profile → every account that signs in on it shares one token.
- A madrasa/library/café computer → all students share one token; the first staff login binds it, then
  every subsequent account is "linked to a banned account" if any one of them is banned.
- iOS Safari vs home-screen PWA vs in-app WebView are **storage-isolated** (B5 §6, WebKit bug 181849) →
  one human, up to three tokens (the reverse failure: linkage split). B5's join rule is the mitigation,
  and B2's B plan **omits B5 entirely** (B2 B5 is only "device list on the account page").

So B's "one token → N fresh accounts is the strongest cheap signal" (B2 §2, the KUNAI pattern) is
**exactly the school/family shape** that `A2`/`B5` identified as the false-positive core. The difference
from the fingerprint is real but narrow: a token collision means "same browser profile", which has far
fewer strangers than "same hardware class" — but it is not zero, and on shared computers it is the norm.
**If B ships, B5's self-claim + kiosk suppression is not optional**; without it the linkage edge will
produce shared-device FPs on the first investigation. Add B5's `POST /device/v1/shared/claim` and the
`non_actionable` rule to the B plan, or B's evidence panel is a trap for the operator.

### 4.3 Token sharing / framing — precise account

- **Sharing your own token to implicate someone else** is _not_ a real vector: the token identifies the
  browser you are on. Sharing it moves _your_ linkage to the recipient's device, which implicates _you_.
- The real framing vectors are (a) **using a victim's browser** (shared computer, XSS, extension) so the
  victim's token gets bound to the attacker's account, and (b) **the victim's own token being linked to a
  banned co-user's account** through a shared profile — the honest shared-device case.
- Compare with the fingerprint: the component key is **attacker-chosen** (A1 F2: a payload can be crafted
  to match a target cohort), so the fingerprint is _more_ weaponizable for deliberate framing; B requires
  obtaining the token. **B is strictly better than C on this axis** — this is the one place B2 undersells
  B, and it should be stated in B's favour.
- The mitigation is the same as B5's: linkage is **evidence only**, never auto-action; a second
  independent admissible corroborator (credential reuse or identical violating content) is required for
  any action; the panel types the edge ("same token" vs "same account" vs "same IP").

### 4.4 Token farming and storage clear

- **Farming:** every fresh browser profile mints a token at registration. N incognito windows → N tokens →
  N accounts. **The per-token registration cap is therefore ~no cap** against a deliberate attacker; it
  only slows a naive script that forgets to clear cookies. B2 concedes this parenthetically ("cookie clear
  resets"); the plan should not count it as a mass-creation control.
- **Storage clear:** clears the token; the evader is unlinked; documented in B2/A2. 32% of browser
  instances clear cookies in 8 months (Li & Cao) — mostly legitimate privacy clearers, which is why the
  token must never punish absence.
- **Token-keyed limits** are thus a marginal add-on: they beat per-IP limits only for the non-clearing
  client, and any client that clears defeats them for free. Their genuine value: **a client that rotates
  IPs but keeps its browser** (rare — the sophisticated attacker does both, the naive one does neither).

### 4.5 What B actually stops that A does not (precise)

| #   | Capability                                                                             | Real?    | Notes                                                                                                                                                   |
| --- | -------------------------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | **Ban-evasion evidence for the naive evader** who keeps the browser and clears nothing | Yes      | The single defensible product. Needs B5's shared-device guards and the "evidence only, never auto-action" rule.                                         |
| 2   | **A low-collision linkage edge** (browser-profile-scoped, not hardware-class-scoped)   | Yes      | Genuinely better than the fingerprint's cohort key on this audience — fewer strangers collide. This is B's real security delta over C, not the reverse. |
| 3   | Per-token rate limits surviving IP rotation                                            | Marginal | Defeated by one line of storage clearing; useful only against non-clearing clients.                                                                     |
| 4   | Mass-creation cap                                                                      | ~None    | Fresh profile = fresh token.                                                                                                                            |
| 5   | Anything about scraping, CSAM, impersonation, CIB                                      | No       | Same as C.                                                                                                                                              |

**Therefore:** build B, but build it as **evidence + linkage**, not as a limiter, and not as a
"device identity" system. If B ships without the shared-device guards (§4.2) and without the "never gates
a privilege" constraint (§4.1), it will reproduce the fingerprint's FP problem with a smaller blast
radius and the same appeal burden.

---

## 5. Attack the ban wiring

### 5.1 What exists and what breaks when you wire it

`user_bans` + `create/revoke/get_active_ban/admin_list` exist (`user_ban/actions.rs`), `check_ban` is
wired into login (`auth_v1/controller.rs:153-166`), OAuth (`oauth/login.rs:321`), passkey
(`passkey_v1/controller.rs:243`) and the guard (`guard.rs:46-56`) with a 5-minute session cache
(`builder.rs:16`). Wiring an admin route is ~2 days. The defects are in the interactions:

1. **No session revocation on ban.** The ban takes effect only when the session's ban cache is stale
   (≤5 min) and only on routes that call `not_banned()`. A banned user's existing session keeps
   working — including any authenticated endpoint that does not carry the requirement — for up to five
   minutes, and reading is unaffected forever (reader routes are public by design, which is correct).
   The repo already has the immediate primitive: `session_auth_hash` is recomputed per request and a
   mismatch deletes the session (`crates/rux-auth/src/session/extractor.rs:248-251`), and rotation on
   credential change is precedent (`user/actions.rs:146-154,224`). **Same-change requirement: the ban
   handler bumps `session_auth_secret`** (or the plan explicitly accepts and documents the 5-min window).
   **NEW**
2. **The 5-min cache is per-session, server-side, and not invalidatable.** Unban has the mirror problem:
   a user can be locked out for up to 5 minutes after unban. Accept and document, or add a
   `ban_epoch` to `AuthSessionState` and compare on read (a serde-compatible field with a default — A3 F1:
   adding a field without `#[serde(default)]` breaks every live session). **NEW**
3. **No index on `user_bans`** (§1, `m000001_init.rs:58-73`): the ban lookup becomes a table scan every
   5 min per active user on a pool-of-1 connection. Add
   `CREATE INDEX IF NOT EXISTS idx_user_bans_user_active ON user_bans (user_id, revoked_at, expires_at)`
   in the same migration. **NEW**
4. **`get_active_ban` returns the newest active ban only** (`actions.rs:59-65`). Create must be
   idempotent (refuse or revoke-then-create) or an operator can stack bans and revoke the wrong one.
   **NEW**
5. **`user_bans.expires_at` is TEXT/chrono RFC3339** (`m000001_init.rs:63`, `actions.rs:12,18`). Keep
   Rust-only writes and comparisons (do not copy B8's "INTEGER epoch" rule onto this table without a
   migration; do copy it for any new ban-adjacent table).
6. **No ban check on password reset or email verification** (grep: `forgot_password_v1/controller.rs`
   and `email_verification_v1/controller.rs` contain no `check_ban`). A banned user can reset their
   password and re-verify. Low harm today, but the reset flow must not become a ban-bypass if it ever
   issues a session (it does not today — reset grants are separate, `forgot_password_v1` reset_token
   module). Verify and state it; add a ban check if any future flow mints a session. **NEW**
7. **The admin route must not reuse the export-token pattern** (A8 F11): mutations = session + CSRF +
   `ROLE_ADMIN` only; the bearer `BAN_EXPORT_TOKEN` stays read-only (`admin_bans_v1/mod.rs:28-33`).
8. **Naming collision:** `/admin/bans` is IP bans (`admin_bans_v1`, `BanUnit = IpNet`). Use
   `/admin/users/bans` (B2's choice is correct) and never merge the two DTOs; `BanUnit::parse` cannot
   parse a user id, but the operator UI will confuse them unless the paths differ clearly.

### 5.2 The banned user's experience (verified, then fixed)

- **Login:** correct password → ban check → `AccountLocked` (AUTH_006, HTTP 403,
  `ruxlog-types/src/error.rs:20-21,231`) → **client shows "Email or password is incorrect."** because
  AUTH_006 is in `CREDENTIAL_TYPES` (`auth-copy.ts:80-88,137-139`; `flows.svelte.ts:118-134`). A banned
  user will retry, burn the login limiter (the limiter runs _before_ the ban check — `:133` vs `:153`),
  and after 3 attempts see "Too many attempts." — never the ban.
- **Fix (same PR):** remove `AUTH_006` from `CREDENTIAL_TYPES`, add a `banned` error kind with its own
  copy ("This account is suspended. You can still read. To appeal: <link>"), make the login flow stop
  retrying on it, and move the ban check before the limiter (or skip counting banned attempts).
  Web-side this is `auth-copy.ts` + `flows.svelte.ts` + tests; it must keep `bun run check/lint/test`
  green.
- **Notice + appeal:** `NotificationKind` has no `Enforcement` variant and the in-app channel is
  unreachable for a session that is 403'd; `is_transactional()` covers only verification + password_reset, so
  a notice to a suppressed address fails with `Err(Suppressed)` — and would be dropped silently with `Ok` if
  simply added to the transactional set (B9 Q4, verified against `services/mail/router.rs:241-258`). B9 finding 12's irreducible minimum applies to the **shipped** ban feature:
  a ban without a durable notice and an unauthenticated appeal route is a silent permanent lockout —
  the Art 17 / Art 22(3) exposure the reports assign to C lands on A1 instead. **Same-change
  requirements: (a) a durable notice channel with delivery proof, (b) `POST /appeal/v1/submit` +
  tokenized page, (c) an `audit_logs` row per ban/revoke.** **NEW (scoped to A1)**
- **Does the ban survive a session?** Yes, by design: guard check every 5 min (or immediately after the
  `session_auth_secret` bump), and the login path is uncached. Reading is unaffected (public routes) —
  correct, but say so in the notice so "banned but still reading" is not mistaken for a bypass.

---

## 6. What A+B misses that the fingerprint design would have caught

Honest list. "Catch" means _detect_, not _enforce_ (the design demotes the device to evidence only).

| #   | Population                                                                                                               | A                                                              | B                                                                            | C (fingerprint)                                                        | Verdict                                                                                                                                                                              |
| --- | ------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------- | ---------------------------------------------------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | Evader who clears storage but does not spoof components (Safari ITP users, privacy clearers, deliberate cookie-clearers) | ✗                                                              | ✗                                                                            | ✅ (if the key survives)                                               | **Acceptable.** This is the population dominated by legitimate users (B2 §3.1); C's catch rate here is unmeasured and its FP cost lands on the same people. Do not build C for this. |
| 2   | Farming with fresh storage per account (incognito-per-signup)                                                            | ✗                                                              | ✗                                                                            | ✅ cohort/continuity signal                                            | **Acceptable if §3.5 (fresh-account privilege limits) ships.** Otherwise it is the cheapest mass-abuse path and nothing sees it. The gate: measure whether it happens.               |
| 3   | Cohort detection with no account and no cookie (N accounts, one hardware class, storage cleared between signups)         | ✗                                                              | ✗                                                                            | ✅ (cohort key)                                                        | **Acceptable — and arguably desirable.** A2 F1/F2 established the cohort key is a cohort _ban_ on strangers; A8 F13 called clustering a no-go at this size. Do not build C for this. |
| 4   | Pre-auth anonymous activity linked to a later account (scrape now, register later)                                       | ✗                                                              | ✗                                                                            | ◐ (only where a payload is attached; reader routes excluded by design) | **Acceptable.** C explicitly excludes reader routes (DESIGN-v1 B2, B3 F4): it cannot see the scraper either. Reader defence is cost-shaping (B3), not identity.                      |
| 5   | Re-link after account deletion + storage clear (the durable-identifier case)                                             | ✗                                                              | ◐ (token survives deletion if not cleared; `device_account_links` retention) | ✅ (digest survives)                                                   | **Acceptable** — and B is the cheap 80% here. The digest's extra durability is exactly the privacy harm A5 flags.                                                                    |
| 6   | Cross-account content/credential linkage                                                                                 | ◐ (R04a/R04b in B4's v1 subset — identifier and content reuse) | ◐                                                                            | ◐                                                                      | **Already covered by the v1 enabled subset** (B4 §8), which is cheaper than C.                                                                                                       |
| 7   | CIB / coordination                                                                                                       | ✗                                                              | ◐ (token edges)                                                              | ✗ (no co-action pipeline)                                              | **A+B is equal or better.** B2 F9 is right.                                                                                                                                          |

Net: A+B misses (1)–(5); of those, only **#2** is worth a decision gate, and it is answered by
fresh-account privilege limits (§3.5), not by C. The fingerprint's unique delta over B remains
"evaders who clear storage but do not spoof hardware", whose composition is dominated by legitimate
users — B2's conclusion survives this audit.

---

## 7. Artifacts, migrations, tests, and the corrected estimate

### 7.1 Option A — artifact list

| #   | Artifact                                                                                                                                                                                                                                               | Files                                                                                                                 | Size                    | Days        |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------- | ----------------------- | ----------- |
| A1  | **Limiter correctness** (new `abuse_peek`/`abuse_record` on the store trait + in-memory impl + `PersistentStore` wrapper; callers `login`, `totp`, `reauth`, `totp_setup`; config split; blocked-path append removed; ban-check-before-limiter; tests) | `crates/rux-request-gate/src/store.rs`, `abuse.rs`; `services/abuse_limiter.rs`; `auth_v1/controller.rs`              | ~250 LOC + 120 test LOC | **1.5–2.5** |
| A2  | **Client banned/credential split** (remove AUTH_006 from `CREDENTIAL_TYPES`, add `banned` kind + copy, login flow stops retrying, tests)                                                                                                               | `web/src/lib/auth/auth-copy.ts`, `flows.svelte.ts`, `__tests__/*`                                                     | ~80 LOC                 | **0.5–1**   |
| A3  | **Ban wiring** (`modules/admin_user_bans_v1/{mod,dto,controller}.rs`, router mount, ROLE_ADMIN + CSRF, idempotent create, revoke, list; `session_auth_secret` bump on create; migration index)                                                         | new module ~350 LOC, `router.rs`, `user/actions.rs`, migration                                                        | ~450 LOC                | **2.5–3.5** |
| A4  | **Ban notice + appeal minimum** (mail template exempt from the suppression drop + delivery state + `NotificationKind::Enforcement`; `appeals` table + `POST /appeal/v1/submit` + tokenized page + rate limit + uniform responses)                      | `services/mail/*`, `ruxlog-types/enums/notification.rs`, new `appeal_v1`, web page                                    | ~450 LOC + migration    | **3–4**     |
| A5  | **Audit writers** (`services/audit.rs` + call sites: ban lifecycle, auth events, admin mutations; index; retention note)                                                                                                                               | new service ~150 LOC + ~15 call sites                                                                                 | ~300 LOC                | **1.5–2.5** |
| A6  | **Registration admission** (per-IP/ASN in-memory caps + allowlist, Turnstile prod wiring + deploy check, verification-identity canonicalization + cap, metrics)                                                                                        | `auth_v1/controller.rs`, new `services/registration_policy.rs`, config, migration (identity hash column), deploy docs | ~400 LOC + migration    | **2.5–3.5** |
| A7  | **Fresh-account privilege limits + comment hardening** (age-gated caps, link/repetition heuristic, flag auto-hide)                                                                                                                                     | `post_comment_v1/*`, config                                                                                           | ~250 LOC                | **1.5–2**   |
| A8  | **Session notifications + terminate-all** (new-session mail + `NotificationKind`, `sessions/terminate_all`)                                                                                                                                            | `auth_v1/*`, mail template, web account page                                                                          | ~300 LOC                | **2–3**     |
| A9  | **Real image moderation + CSAM path** (provider seam, hash-list interface, NCMEC SOP, retention class)                                                                                                                                                 | `services/image_moderation/*`, `media_v1`, migration, doc                                                             | ~400 LOC + doc          | **3–5**     |

**A core (A1–A7): ~12–17 dev-days.** Full list (A8–A9): **~17–25**. B2's "core 8–12" is low by ~4–5
days: it prices the limiter fix as a controller edit (it is a store-API change with five clone call
sites), prices ban wiring without session revocation, notice, or appeal, and does not price the
verification-identity work at all.

### 7.2 Option B — artifact list (corrected to include the shared-device guards)

| #   | Artifact                                                                                                                                                                                                               | Days  |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----- |
| B1  | Migration: `device_tokens` (token_hash UNIQUE, first/last seen, revoked_at) + `device_account_links` (+ indices, retention columns INTEGER epoch)                                                                      | 0.5–1 |
| B2  | `services/device_token.rs` (mint ≥128-bit, HMAC-SHA256 via HKDF subkey off the existing `secret_key`/`COOKIE_KEY` discipline, verify, bind, rotate, revoke) + cookie flags `HttpOnly; Secure; SameSite=Lax; Path=/api` | 2–3   |
| B3  | Hooks: bind at login/register; token → linked-accounts flag for review (never auto-action); **token-keyed limits deferred**                                                                                            | 1–2   |
| B4  | Admin evidence panel (one hop, cap 20, typed edges) on the existing case view                                                                                                                                          | 1–1.5 |
| B5  | **Shared-device guards** (B5 §3 self-claim + §2.4 suppression semantics + non_actionable) — **required if B ships**                                                                                                    | 2–3   |
| B6  | Web device list on the account page + new-device mail                                                                                                                                                                  | 1     |
| B7  | Guard tests (cookie flags, no token on reader routes, retention job, no privilege gated by token) + policy paragraph + retention decision                                                                              | 1.5–2 |

**B: ~9–14 dev-days** (B2's 6–10 omits B5's guards, which are the FP control for the linkage edge).
Policy: 1–2 days. **A+B total ≈ 22–33 dev-days + 1–2 policy**, vs B2's 14–22.

### 7.3 Migrations

- **First migration (next free number)**: `CREATE INDEX IF NOT EXISTS idx_user_bans_user_active ON user_bans (user_id, revoked_at, expires_at)`; `appeals` table (id, case ref, token_hash, state, created/expires); `users.verification_identity_hash TEXT` (nullable, additive — `ALTER TABLE ADD COLUMN` with NULL default only, A10 F9); `audit_logs` index if the retention/query plan needs one.
- **Second migration** (Option B): `device_tokens`, `device_account_links`; all retention columns INTEGER epoch (B8 F7); no CASCADE FK to users (evidence outlives accounts — B8 F12).
- Both: `up()` pure DDL, `IF NOT EXISTS`, idempotent — sea-orm-migration 1.1.20 does **not** wrap SQLite migrations in a transaction (B10 §1.1 verified), so a crash mid-run re-runs.

### 7.4 Tests (the minimum that pins the fixes)

1. `login_failure_counters_do_not_count_success` — 3 successful logins in 6 min → no block.
2. `login_blocked_path_does_not_append_attempts` — request during a block does not grow `long_count`.
3. `credential_never_gets_a_429_from_failure_thresholds` — 100 failures → challenge state, never 429 on the credential; 429 only on the IP key.
4. `success_clears_only_the_credential_key`.
5. `banned_login_returns_auth_006_before_limiter_increment`.
6. `ban_creation_bumps_session_auth_secret_and_kills_live_session` (extractor test with a real session).
7. `ban_revoke_restores_login_immediately` (login path uncached).
8. `second_active_ban_is_rejected` (idempotent create).
9. `get_active_ban_uses_index` — `EXPLAIN QUERY PLAN` contains `idx_user_bans_user_active`.
10. `register_rejects_dot_aliased_gmail_beyond_identity_cap` and `register_allows_distinct_identities`.
11. `registration_cap_allows_known_allowlisted_cidr`.
12. `turnstile_prod_unconfigured_is_503_and_configured_requires_token` (exists in `turnstile.rs` tests — extend with the web-coupling check).
13. Web: `banned_login_shows_ban_copy_not_credential_copy` (`auth-copy.test.ts`), and the existing login-flow tests stay green.
14. `appeal_submit_is_uniform_and_rate_limited`.
15. `device_token_cookie_flags` + `device_token_never_on_reader_routes` + `no_privilege_requires_the_token` (Option B).

---

## 8. Corrected PR order, and the A→B gate

### 8.1 The first five PRs, with the reason for each

| PR      | Content                                                                                                                                    | Why this position                                                                                                                                                                                                                                                                                 |
| ------- | ------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **PR1** | Limiter correctness (A1) + client banned/credential split (A2)                                                                             | Safety-critical and unblocks measurement: until the limiter stops counting successes and AUTH_006 stops masquerading as a credential failure, every ban/challenge in the logs is confounded with a lockout, and the ban UX actively misleads users. Smallest change, largest correctness gain.    |
| **PR2** | Ban wiring + `session_auth_secret` bump + index + idempotent create (A3)                                                                   | The only enforcement primitive that exists in schema; it is the prerequisite for measuring evasion (the C1/C2 gate) and for every later measure to mean anything. Ship the session kill in the same PR — a ban that takes 5 minutes and leaves the session alive is not an enforcement primitive. |
| **PR3** | Audit writers (A5) + ban notice/appeal minimum (A4)                                                                                        | These are _part of_ PR2's contract, not later polish: a ban with no audit row, no durable notice and no appeal is the silent-permanent-lockout failure B9 F12 already described, and it lands on A1 the moment it ships. If PR2 must be split for size, PR3 is PR2b.                              |
| **PR4** | Registration admission (A6) + fresh-account privilege limits (A7)                                                                          | Now that bans exist and are auditable, admission control is measurable (metrics) and its FP classes (CGNAT/schools) can be watched. Doing it before ban wiring would produce a wall with no consequence behind it.                                                                                |
| **PR5** | Option B token as **evidence + linkage only** (B1–B4, B6, B7), with B5's shared-device guards and the "never gates a privilege" constraint | The token's only defensible products are the naive-evader linkage and the low-collision edge; both need the ban machinery (PR2/PR3) to be actionable and the shared-device guards to be safe. Do not ship token-keyed limits in PR5 — defer them behind measurement.                              |

Deferred: A8 (session notifications — good but not abuse-reducing), A9 (image moderation — **ship
immediately if uploads are accepted from untrusted users**; it is the only CSAM control and should
probably be pulled forward to PR3/PR4 if media upload is live), invite mode, ASN dimension, edge Worker.

### 8.2 The A→B gate (abort/measurement), corrected

B2's gate (C1 ≥30% of confirmed evasion cases have no token; C2 token recapture <40%; C3 1 eng-day/week)
is directionally right but has two defects: it measures **only** the fingerprint's justification, and it
runs after B is already built. Corrected:

**Gate 0 — before PR5 (is B justified at all?):** 90 days after PR1–PR4, from the new audit log and
moderation queue:

- **G0.1 — base rate:** ≥5 ban-worthy events/quarter (bans issued + spam actioned + scrape incidents).
  If false, **stop** — neither B nor C is justified; the correct spend is moderation and cost-shaping.
- **G0.2 — the naive-evader population exists:** ≥10 confirmed ban-evasion cases in the window. If
  false, the linkage edge has no target.
- **G0.3 — IP linkage is actually insufficient:** in the confirmed cases, per-IP linkage produced
  ≥2 unrelated-account collisions (CGNAT) or failed to link the evader. If false, the IP axis plus
  identifier/content reuse (B4's v1 subset) already covers it and B's token adds little.
- **G0.4 — shared-device FP budget:** the moderation queue shows <20% of linked-account investigations
  resolve as "shared device / legitimate multi-account" (B5 §5.1). If above, the linkage edge is
  FP-dense and B5's guards must ship first (or B is postponed).

**Gate 1 — after B has run 90 days (is C justified?):** B2's C1/C2/C3 unchanged:

- **C1 population:** ≥30% of confirmed evasion cases have no/cleared token.
- **C2 headroom:** token recapture <40%.
- **C3 capacity:** ≥1 engineer-day/week committed indefinitely.
- Add **C4 (FP budget):** B's linkage edge produced <5 overturned/human-rejected linkage cases per
  quarter — if B itself is already FP-dense, C (whose key collides far more) is worse by construction.

Any false → C is net-negative; B2's "defer C" stands. **Do not run PR5 before Gate 0**; do not run any C
item before Gate 1.

### 8.3 Corrected dev-day estimate (summary)

|                       | B2        | C1 (corrected)                                      | Delta reason                                                                                                                                        |
| --------------------- | --------- | --------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| A core (A1–A7)        | 8–12      | **12–17**                                           | store-API limiter fix + five clone call sites; session revocation; notice+appeal folded in; verification-identity canonicalization; web client work |
| A full (incl. A8, A9) | 22–34     | **17–25** for A1–A9 as scoped here (A8 2–3, A9 3–5) | B2's "full" includes items C1 treats as deferred/optional; the comparison is scope-dependent                                                        |
| B                     | 6–10      | **9–14**                                            | B5 shared-device guards are mandatory for the linkage edge; guard tests; retention job                                                              |
| Policy                | 1–2       | 1–2                                                 | unchanged                                                                                                                                           |
| **A core + B**        | **14–22** | **21–31**                                           |                                                                                                                                                     |
| **Everything**        | **28–44** | **26–39**                                           |                                                                                                                                                     |

---

## 9. Findings (14)

### F1 — The login limiter counts successes and never clears; the correct fix is a store-API change, not a controller edit. _confirms B2 F2; the design is NEW_

**Severity: High.**
**Evidence:** `auth_v1/controller.rs:133-140` (runs before password verification); `store.rs:322` (appends
on every call), `:305` (appends on the blocked path); `clear_limit` has no auth caller (grep: only
`admin_bans_v1/controller.rs:152-153,448-449,545-546`). Five clone call sites: `auth_v1/controller.rs:299,
411,582,813,918`; `email_verification_v1/controller.rs:49`; `forgot_password_v1/controller.rs:337,503`.
**Fix:** `abuse_peek` (read-only) + `abuse_record` (append/evaluate) on `RateLimitStore`; failures only;
credential key → challenge + notify, never 429; IP key → 429 ≤15 min; success clears the credential key
only; blocked path must not append. §2.3.

### F2 — The 24-hour lockout weapon is real but not "5 requests": it is the blocked-path append + the long tier, so a failures-only patch that leaves `store.rs:305` intact does not fix it. **NEW**

**Severity: High.**
**Evidence:** `store.rs:303-315` (blocked requests append and return early — the long threshold is never
evaluated during a block), `:335-343` (long block when the temp block expires with ≥5 attempts in the
trailing 900 s). The victim's own retries during the hour are the evidence for the 24 h block.
**Fix:** remove the append on the blocked path; delete the long tier for credential keys; keep two-tier
escalation only on `login_fail_ip` capped at 1 h. Test `login_blocked_path_does_not_append_attempts`.

### F3 — Gmail dot-aliases bypass the alias policy entirely: one inbox → unlimited verified accounts, which voids per-domain/disposable/inbox controls and the "one inbox per account" cost model. **NEW**

**Severity: High.**
**Evidence:** `utils/email.rs:7-9` (trim+lowercase only), `utils/email_alias.rs:8-18` (`+` only),
`config/settings.rs:48-53` (prod default off), `user/actions.rs:280` (`find_by_email` uses the same
normalization). `a.b@gmail.com` ≠ `ab@gmail.com` in the DB, identical inbox at delivery.
**Fix:** canonicalize the _verification identity_ for dot-insensitive providers (gmail.com/googlemail.com
dot-stripping; `+`-stripping where supported), store an HMAC `verification_identity_hash`, cap accounts
per identity (3 pending / 5 lifetime), feed it into the registration burst rule.

### F4 — Registration admission control cannot stop a proxy-funded attacker; its residual capacity is ~10⁵ accounts/day, and the honest harm reduction is fresh-account privilege limits, not the wall. **NEW**

**Severity: High.**
**Evidence:** `auth_v1/controller.rs:459-535` (no per-identity limiter), `router.rs:72-76` (100/min/IP =
144 k/day), `turnstile.rs:42-47` (prod fail-closed when unconfigured); CGNAT/school FP classes from A2/B5;
residential-pool diversity from R2 §2.2 (44% of IPs in >1 pool) as cited in B3 §5.
**Fix:** per-IP soft cap (3 free, 5 → challenge, 20 → 24 h) + CIDR allowlist; per-ASN challenge tier;
disposable-domain list only; **fresh-account privilege limits (<24 h: 1 comment, no links/media)**;
invite mode as an emergency flag. State the residual in the plan.

### F5 — Turnstile's production semantics are the opposite of "optional/no-op", and the API-secret ↔ web-site-key coupling is an unguarded deploy trap. **NEW**

**Severity: Medium-High.**
**Evidence:** `turnstile.rs:17-19,42-47` (prod unconfigured → 503; configured → token mandatory),
`:91-94` (`captcha_unavailable`); web gates the widget on `PUBLIC_TURNSTILE_SITE_KEY`
(`web/src/lib/auth/turnstile.ts:4-7`, `RegisterForm.svelte:51-55`). API secret without web site key =
registration 400/503 for every user.
**Fix:** a deploy smoke test that registers with a real token; a boot log naming the site-key coupling;
the §4.3 fallback ladder (Turnstile → email OTP → delay) for censored networks on the _registration_
path too, or an explicit decision to accept the hard block.

### F6 — The B token is a second 12-month bearer identifier with no authentication power; HttpOnly prevents reads, not use, so it adds no XSS resistance over the session cookie and doubles the theft window. **NEW**

**Severity: High.**
**Evidence:** session cookie is HttpOnly with a 14-day inactivity expiry (`main.rs:781-788`); B2 B2 proposes
`HttpOnly; Path=/api` with 12-month token/links retention; CSP trusts gstatic/googletagmanager/
challenges.cloudflare.com with live Firebase Analytics (B10 §1.10 verified). Any same-origin script can
`fetch` `/api/**` and use the cookie; theft consequence is attribution poisoning (linking a victim's
token to attacker accounts), not account takeover.
**Fix:** normative constraint — **the token never gates a privilege** (mirrors B9's device-as-evidence
rule); shorten the token/links retention to the ban window (≤12 mo only while a case is live, 90–180 d
otherwise); treat the token as evidence-only in every rule.

### F7 — "One token = one device" is false on shared browsers; the linkage edge reproduces the school/family FP shape unless B5's self-claim/kiosk guards ship in the same plan. **NEW**

**Severity: High.**
**Evidence:** B2 §2 claims "one token → N fresh accounts is the strongest cheap signal"; B5 §7.1/§1
documents the shared-account/kiosk shape; storage-isolated contexts split one human into up to three
tokens (`B5` §6, WebKit bug 181849). B2's B-plan table has no shared-device guard (B5 is "device list on
the account page").
**Fix:** fold `B5-shared-devices.md` §2/§3 into Option B (self-claim 30 d, admin kiosk, `non_actionable`,
suppression, the 7-step decision procedure) and type the panel's edges ("same token" ≠ "linked
accounts"); never act on the token edge alone.

### F8 — Token-keyed limits and per-token creation caps are defeated by one line of storage clearing; B's defensible delta is the low-collision _linkage_ edge, and it should be built as evidence only. **NEW**

**Severity: Medium-High.**
**Evidence:** token minted per browser profile (B2 B2/B3); fresh profile = fresh token at register;
32% of browser instances clear cookies in 8 months (Li & Cao, via B2 §1/A2); the profile-scoped token
collides far less than a hardware-class cohort key (B2 §3.2).
**Fix:** ship B as evidence + linkage (PR5), defer token-keyed limits behind Gate 0/Gate 1 measurements;
never let the token suppress or gate a user action.

### F9 — Ban wiring without session revocation leaves the ban toothless for ≤5 minutes and there is no revocation primitive wired to it. **NEW**

**Severity: High.**
**Evidence:** `builder.rs:16` (5-min ban cache), `guard.rs:46-56` (cache path); the immediate primitive
already exists: `crates/rux-auth/src/session/extractor.rs:248-251` (hash mismatch → `session.delete()`),
rotation precedent `user/actions.rs:146-154,224`.
**Fix:** ban create/revoke bumps `session_auth_secret` in the same transaction as the ban row; document
the ≤5-min window only if the bump is rejected; add `idx_user_bans_user_active` (no index today —
`m000001_init.rs:58-73`); make create idempotent (`get_active_ban` returns one row, `actions.rs:59-65`).

### F10 — The banned-user login UX is wrong on both sides: AUTH_006 is classified as a credential failure, and the limiter runs before the ban check so a banned user's attempts become a 429. **NEW (mechanism)**

**Severity: Medium-High.**
**Evidence:** `ruxlog-types/src/error.rs:20-21,231` (AUTH_006 = 403); `web/src/lib/auth/auth-copy.ts:80-88,
137-139` (AUTH_006 inside `CREDENTIAL_TYPES`); `flows.svelte.ts:118-134`; `auth_v1/controller.rs:133`
(limiter) vs `:153` (ban check).
**Fix:** split AUTH_006 out, add a `banned` error kind + copy with the appeal link, stop the retry loop;
check the ban before the limiter (or skip counting banned attempts); keep `bun run check/lint/test` green.

### F11 — Shipping the ban without a durable notice and an unauthenticated appeal converts A1 into the silent-permanent-lockout liability the reports assigned to C. **NEW (scoped to A1)**

**Severity: High.**
**Evidence:** `NotificationKind` has no Enforcement variant (A8 §0 item 5; B9 F8); a notice to a
suppressed address is never delivered (`Err(Suppressed)` today; silent `Ok` if the template is made
transactional — `services/mail/router.rs:241-258`, B9 Q4); no appeal module (A8 F7); a banned
session is 403'd out of the in-app channel.
**Fix:** A4 (notice template exempt from the suppression drop, delivery state recorded, `appeals` table +
`POST /appeal/v1/submit` + tokenized no-referrer/noindex page + rate limit + uniform responses) ships in
PR2/PR3, not later.

### F12 — A+B misses five populations the fingerprint would catch; four are acceptable, one (incognito-per-signup farming) needs an explicit decision gate. **NEW**

**Severity: Medium.**
**Evidence:** §6 table; C's reader-path exclusion (DESIGN-v1 B2, B3 F4) means even C cannot see anonymous
scrapers; the cohort key's FP density (A2 F1/F2, B2 §3.2) makes #3 undesirable; #2 is answered by
fresh-account privilege limits.
**Fix:** accept #1, #3, #4, #5 with the stated rationale; gate #2 on G0.2/G0.4 metrics, and ship the
fresh-account privilege limits (A7) regardless.

### F13 — The A+B estimate is low by ~7–9 dev-days: the limiter fix is a store-API change with five clone call sites, ban wiring needs session revocation + notice + appeal + an index, and the verification-identity work is unpriced. **NEW**

**Severity: Medium.**
**Evidence:** §7.1/§7.2 artifact tables; `crates/rux-request-gate` is a separate crate consumed by five
modules; no `user_bans` index (`m000001_init.rs:58-73`); no appeal/notice artifacts exist.
**Fix:** budget A core 12–17 d, B 9–14 d, policy 1–2 d (A+B ≈ 22–33 d, not 14–22).

### F14 — Corrected PR order and gates: limiter/client first, ban+session-kill+audit+notice+appeal second, registration third, B as evidence-only fifth; B and C each get a code-enforced measurement gate. **NEW**

**Severity: High.**
**Evidence:** §8.1/§8.2; B2's gate measures only C's justification and runs after B is built; the
corrected Gate 0 measures whether B is justified at all (base rate, naive-evader population, IP-linkage
insufficiency, shared-device FP budget).
**Fix:** PR1 limiter+client → PR2 ban+session kill+index → PR3 audit+notice+appeal → PR4 registration +
fresh-account limits → PR5 B token (evidence only, B5 guards included). Gate 0 before PR5; Gate 1
(B2's C1–C3 + C4) before any C item.

---

## 10. Confidence notes

- Every repo claim above is a source read at HEAD in this session; the test suite was not run.
- Production env values (`TURNSTILE_SECRET_KEY`, `PUBLIC_TURNSTILE_SITE_KEY`, `EMAIL_ALIASES_ENABLED`,
  `QURAN_BAN_ESCALATION_ENABLED`) are not readable from the tree; the analysis uses the code defaults.
- The Gmail dot-alias bypass depends on Gmail's documented dot-insensitivity; other providers differ
  (Outlook/iCloud treat dots as significant). The fix should be provider-scoped, not universal.
- Dev-day estimates are engineering judgement calibrated to the module sizes cited in §7, not measured
  builds; the _relative_ corrections (limiter is a store change; B5 guards are mandatory for B) are the
  load-bearing part, not the absolute numbers.
- The "what A+B misses" table states detection capability only; enforcement capability is a separate
  (and for the device axis, deliberately empty) column.

_End of C1._
