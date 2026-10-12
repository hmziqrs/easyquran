# A1 — Adversarial threat model of DESIGN-v0 (device identity for EasyQuran)

Auditor: adversarial security review. Date: 2026-10-12.
Design under audit: `DESIGN-v0.md`. Repo: `/Users/hmziq/os/easyquran`.
Inputs read: DESIGN-v0, `R2-evasion-counters.md`, `R2-network-transport.md`, `R2-session-binding-rba.md`,
`R2-fingerprint-science.md`, plus repo source (cited inline).

Premise of this review: the attacker has read DESIGN-v0 (the project is open source, and even if it were not,
a design document is not a secret). Therefore every mechanism whose only protection is "the attacker does not
know how it works" is treated as absent.

**What I did not find new problems in** (stated up front, per the brief): the session-binding primitives are
sound — `cycle_id()` on login (`crates/rux-auth/src/session/extractor.rs:101,121`), `session_auth_hash`
recomputed per request (`:53`), process-local revocation set plus durable `revoked_at`, opaque
`ruxlog.sid` cookie with 14-day absolute TTL. The client-IP resolution path is also well built: forwarded
headers are peer-gated, and a missing `CF-Connecting-IP` in production is a hard 400
(`src/middlewares/client_ip.rs:113-148`). The problems below are in how DESIGN-v0 _uses_ these primitives,
not in the primitives.

---

## 1. The kill chain: what the attacker actually does, layer by layer

The attacker's objective is not "look like a bot" — it is "keep an account that can post illegal content,
scrape the corpus, or harass a specific community, while the defender cannot identify or remove me." Three
personas: **T2 funded scripter** (patched Chromium + residential proxies), **T3 organiser** (fresh VM per
identity), and **XSS/malware attacker** (steals another user's credentials and device state).

### Layer 0 — collection policy

No attack. The policy list is honest. The attacker benefits from it: it removes canvas/audio entropy from
the server's inputs while leaving the _ban key_ (device_key, DESIGN:35) built from hardware strings the
attacker can freely choose (WebGL renderer family, extension-set hash). See F3.

### Layer 1 — client collector

1. Read the collector source. Learn the exact field list and the exact coherence rules.
2. Because the coherence booleans and tamper flags are **computed in the collector and shipped as data**
   (DESIGN:29-30), send `{coherence: {ua_ch: true, gpu_platform: true, hardware: true, screen: true},
tamper: {webdriver: false, native_tamper: false, cdp: false}}` regardless of the real browser state.
   One line of JS, and Layer 1's entire detection value is gone. (F1.)
3. Choose components that hash to a **new** device_key whenever the design's ban would otherwise apply —
   i.e. run each abusive identity in a fresh anti-detect profile (R2-fingerprint-science §1.1: an
   anti-detect browser clears everything per profile). New device_key = new device record = "new device",
   which the design treats as a _reason to challenge_, not a reason to block (DESIGN:45, shared-device
   safety).
4. If the attacker wants to be cheap rather than thorough: do nothing. The device payload is only
   transmitted by `AuthClient` calls, and the anonymous reader never makes one (F5). Just fetch
   `/t/**`/`/api/quran` with a plain HTTP client and the device layer never sees the request at all.

### Layer 2 — server storage

5. Farm the collector. Each distinct component set produces a distinct `device_key`
   (`UNIQUE(device_key)`, DESIGN:35). Components are attacker-chosen strings in the collector payload, so
   the key space is unbounded by construction. Send N random payloads → N rows in `device_records`, each
   with `last_seen`/`seen_count` maintained (DESIGN:35). On a single-writer SQLite pool this is storage
   exhaustion plus write amplification on the 10-second L2 flush path. (F2.)
6. Or make the attacker's own device record _look legitimate_: present a stolen token (F4) or a token that
   was "aged" by many benign requests, so the risk engine's device-trust input is favourable.

### Layer 3 — risk engine

7. Read the rule weights from the open-source design and keep the score at 79. The engine is deterministic
   and the thresholds are published (`ban >= 80`, `restore <= 40`, DESIGN:41). The attacker calibrates by
   observing the L1→L2→L3 transitions. (F10.)
8. Sit at L1/L2 forever: throttle and Turnstile are costs the attacker has already budgeted for
   (R2-session-binding-rba §7.2: solver services defeat challenges at scale; 65% of ATO attacks already use
   advanced automation).
9. If a ban does land, wait. `restore <= 40` means a ban is a score threshold, not a decision; unless the
   design adds a hard floor, the attacker decays back in. The design's "never auto-permanent-ban" (DESIGN:44)
   is the correct compliance posture and simultaneously the correct attacker posture — the attacker just
   waits out the automated layer.
10. File an appeal and read the durable notice. If the notice names the triggering signal, the attacker
    changes exactly that one thing. (F9.)

### Layer 4 — edge

11. Note that the signed header set does **not** include `CF-Connecting-IP`, the query string, or the body
    (DESIGN:48; the field list is enumerated in R2-network-transport:318-324). Nothing the attacker does
    changes the IP the Worker reports, so this is not directly exploitable _from the internet_ — but it
    means the edge signature authenticates the header values only, and any process that can reach the origin
    from inside the default trusted network (`10/8`, `172.16/12`, `192.168/16`, `fd00::/8`,
    `docker-compose.yml:143`) and knows the key can present a fully "verified" request with a spoofed
    `CF-Connecting-IP`. (F6.)
12. Smuggle headers. Cloudflare forwards client headers through to the origin
    (R2-network-transport §7 item 1). If the Worker only sets the headers for which `request.cf` actually
    has a value (JA4 and Bot-Score are absent on non-Enterprise plans — DESIGN:48 says "JA4 only when the
    plan exposes it"), a client-supplied `X-Eq-Edge-JA4` survives the Worker. The origin then either
    (a) includes it in the canonical string → every such request 400s (self-DoS, or a denial-of-service
    against other users if the header is injectable into a shared path), or (b) ignores it → the attacker
    has injected a header the Worker never signed. (F6.)

### Layer 5 — ops

13. Treat `/admin/devices` as the highest-value target in the system. It is a new endpoint family that can
    ban and unban, returns evidence, and (per DESIGN:51) has no stated separate credential, no per-action
    audit, and no step-up requirement. One stolen admin session — which the design does not bind to an
    admin device — is full control of the enforcement system. (F9.)
14. Use the appeal flow as a laundering path: it is a human-mediated, unbounded, unauthenticated-by-design
    input that resets enforcement state.

---

## 2. The design's own mechanisms as attack surfaces

| Mechanism                                    | How it is attacked                                                                                                                                                                                                                                                                                                                                                                                                                                |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Device token (stable, server-minted, opaque) | Bearer credential. Stolen via any script the CSP allows (`script-src` includes `gstatic.com`, `googletagmanager.com`, `challenges.cloudflare.com` — `web/src/hooks.server.ts:80-85`), transferred to another machine, or left on a shared device. Possession ⇒ device-trust transfer and a forged linkage edge. (F4)                                                                                                                              |
| HMAC pepper (device_key)                     | Single global key with no versioning. Leak ⇒ offline candidate testing + pre-computed collisions. Restore-from-backup of the wrong secret ⇒ every device_key stops matching ⇒ mass "new device" events. (F8)                                                                                                                                                                                                                                      |
| Edge signing key                             | Same secret-store question as the pepper; lives in the api container's env (`env_file: [.env]`, `docker-compose.yml:119`). A leak lets an in-network attacker forge the entire "verified" header set. (F6)                                                                                                                                                                                                                                        |
| Coherence booleans                           | Client-computed ⇒ attacker-authored. Also a probing oracle if ever surfaced to the client. (F1, F10)                                                                                                                                                                                                                                                                                                                                              |
| Risk thresholds (80/40)                      | Published, deterministic, and observable through the enforcement ladder ⇒ a calibration target. (F10)                                                                                                                                                                                                                                                                                                                                             |
| Appeal flow + durable notice                 | Oracle (tells the attacker which signal fired) and laundering path (unbounded human-mediated reset). (F9)                                                                                                                                                                                                                                                                                                                                         |
| Admin API                                    | New high-value target: ban/unban/evidence with no separate auth boundary, no per-action audit, no step-up. `BAN_EXPORT_TOKEN` is a static bearer (`src/config/settings.rs:415`, `src/modules/admin_bans_v1/controller.rs:188-222`) and would become a device-ban enumeration oracle if device units enter the export. (F9)                                                                                                                        |
| `BanUnit::Device(device_key)`                | The ban unit is a fingerprint hash. Today `BanUnit` is IP-only and rejects everything else (`src/modules/admin_bans_v1/dto.rs:17-38`), the admin list filter drops non-IP units (`controller.rs:44-49`), and escalation is IP-only (`src/services/escalation.rs:72-78,133,172,207`). Adding a device unit therefore has no escalation path, no list/export representation, and inherits the fingerprint's instability and cohort collisions. (F3) |
| `device_records.last_seen` / `seen_count`    | A write on every request, on a single-writer SQLite pool, against an explicit repo invariant: "Do NOT add a DB write to the request hot path" (`src/services/rate_limit_store.rs:1`). (F2)                                                                                                                                                                                                                                                        |
| `audit_logs` reuse                           | The table already carries raw `ip_address` and `user_agent` (`migration/src/m000001_init.rs:252-263`), contradicting "raw signals never stored" and turning a general audit table into a device-evidence store with no retention job. (F11)                                                                                                                                                                                                       |
| Step-up (fresh-auth grants)                  | Not a challenge primitive: grants are records of a completed authentication bound to `session_id` + credential secrets (`src/services/fresh_auth.rs:36-45,111-129,246-290`), capped globally at 10,000 entries (`:16,100-102,176-178`) in a process-local map. (F7)                                                                                                                                                                               |

---

## 3. Where the design overclaims

A careful reader of DESIGN-v0 would assume the following. None of them hold.

1. **"Coherence booleans detect spoofing."** They are client-authored. The server cannot recompute them
   because Layer 0 forbids storing the raw values it would need (DESIGN:19,55 vs :29,35). This is an
   internal contradiction, not a tuning problem. (F1)
2. **"Device identity makes banning effective."** The device axis covers only clients that run the
   collector _and_ make an `AuthClient` call. The anonymous reader — and therefore the scraper — never does
   (`web/src/lib/auth/auth-state.svelte.ts:15-23`; `ensureAnonymousSession` is reached only from
   `unsafeRequest`, `auth-client.ts:257-259`, and the OAuth flow, `oauth-flow.svelte.ts:102,145`). A ban
   axis that requires the attacker's cooperation is not a ban axis. (F5)
3. **"The device signal can only challenge freely, and ban only with corroboration"** (DESIGN:45) is a
   _policy sentence_, not a mechanism. Nothing in the design makes corroboration a precondition of the ban
   write; the enforcement surface is a single threshold. (F3)
4. **"The edge signature makes `X-Eq-Edge-*` trustworthy."** It makes the _values it covers_ unforgeable to
   anyone who did not transit the Worker and does not know the key. It does not attest the eyeball's TLS
   fingerprint (the client chooses its own TLS stack; R2-network-transport §5.2 shows browser-identical
   TLS/h2 is commodity), it does not bind the client IP, the query string, or the body, and it is replayable
   for ±30s. (F6)
5. **"Purpose limitation in code"** (DESIGN:54) has no enforcement: one SQLite file holds `device_records`,
   `device_sessions`, `user_sessions`, and the analytics tables, so the forbidden join is one `SELECT`.
   (F11)
6. **"Raw signals never stored"** (DESIGN:55) contradicts the stored `evidence JSON` (DESIGN:35) and the
   reuse of `audit_logs.ip_address`/`user_agent` (DESIGN:38). (F11, F12)
7. **"Step-up on new device at login"** (DESIGN:43) cannot be implemented with the existing fresh-auth
   grant: at login there is no grant yet, and the grant is bound to the session, not the device. The design
   presents an existing record-keeping primitive as if it were a challenge primitive. (F7)

---

## 4. Specific questions

### (a) Can the collector be replayed / farmed to poison or dilute the device store?

**Yes to all three, with one correction on the mechanism.**

- **Replay:** the payload has no nonce, no session binding, and no server-issued challenge. A payload is
  idempotent by design (it is a fingerprint). Replaying it is harmless _for the attacker_ and harmless to
  the store (same key → same row), so "replay" is not the attack. The attack is **farming**.
- **ID exhaustion:** the components are attacker-chosen values in a client payload. The server HMACs them
  into `device_key`; `UNIQUE(device_key)` then makes each distinct component set a new row. The key space is
  effectively unbounded, so an anonymous attacker can create rows without limit. The design has no
  per-identity cap on device-record creation, and `device_records.user_id` is nullable (DESIGN:35) — i.e.
  creation is deliberately possible before authentication.
- **Storage exhaustion + write amplification:** this is the _real_ DoS, and it is worse than the row count
  suggests. The repo's persistence design flushes a full snapshot of the rate-limit store to SQLite every
  10 seconds, upserting **every** row (`src/services/rate_limit_store.rs:180-229`), and loads the whole
  table at boot (`:75-110`). Adding a device axis to the ban/limit key space means the flush cost scales
  with the number of device units the attacker has created. Separately, if `last_seen`/`seen_count` is
  maintained per request, that is a write on the request hot path, which the codebase explicitly forbids
  (`rate_limit_store.rs:1`) and which the single-connection pool cannot absorb.
- **HMAC cost DoS: no.** HMAC-SHA256 over a bounded payload is microseconds; it is not a usable lever.
  The levers are the DB write, the row growth, and the JSON parse of a body up to the 64 KiB default limit
  (`src/config/mod.rs:9-13`). Do not spend design effort on HMAC cost; spend it on the write path.

### (b) Does a stable device token make things worse than no fingerprint?

**Yes, in four specific ways — and the design has the fixes available but does not use them.**

1. **It is a transferable bearer credential.** Unlike DBSC (non-exportable key, R2-session-binding-rba §2),
   the token is a string in `localStorage` + IDB. Anyone who reads it can present it. The CSP on the web
   origin explicitly trusts three third-party script origins (`web/src/hooks.server.ts:80-85`), one of
   which — `challenges.cloudflare.com` — is the design's own step-up challenge provider (DESIGN:42). A
   compromise anywhere in that set is a device-token compromise, and the token is stable for its lifetime.
2. **It forges high-weight linkage edges.** The R2-evasion report names the attack: the "ant trap" — an
   attacker deliberately shares a resource with victims to poison the clustering graph
   (R2-evasion-counters §6.2). A stolen token lets the attacker attach their abuse to a victim's device
   record. The correct defence is already stated there (asymmetric edge weights, never one low-precision
   edge), and DESIGN-v0 does not adopt it for the token: the token is treated as device identity, not as a
   weighted edge.
3. **On shared devices it accumulates liability.** One device record, many accounts (family tablet, mosque
   kiosk, library). The design's "shared-device safety" clause limits _bans_, not _linkage_ — so a single
   abuser on a shared device still creates a device record that is correlated with every legitimate user of
   that device, and any future enforcement that touches the device axis hits all of them.
4. **It is unstable on exactly the browsers the design promises not to penalise.** Safari ITP caps
   script-written storage, and iOS 26 explicitly denies long-lived script-written storage to known
   fingerprinting scripts (R2-fingerprint-science §4.2). The token will vanish; the user will be "new"
   again; the design's own rule "step-up on new device" then applies to them repeatedly.

Net: as specified (stable, `localStorage`, and the ban key), the token is worse than no token for the
XSS/shared-device case, and no better for the evasion case. It becomes net-positive only with
HttpOnly + rotation + additive-evidence-only semantics. The clean version of that fix is a
`__Host-`-style cookie with `Path=/api`: cookies are path-scoped, so a `/api`-scoped cookie is never sent on
the `/t/**` document request and the 7-day disk cache (DESIGN:32) is untouched — the design's stated reason
for choosing `localStorage` does not actually require `localStorage`.

### (c) Does the edge signing scheme add security, given Traefik forwards client headers untouched?

**It adds one real property and three gaps, and it introduces a new availability risk.**

Real property: it stops a request that did not transit the Worker from presenting plausible
`X-Eq-Edge-*` values — _provided_ the origin is otherwise unreachable and the key is secret from the
attacker's network position. That is a genuine hardening step, and it is the pattern CloudFront/Vercel/
Fastly use (R2-network-transport §7 item 10).

Gaps:

1. **The signed set omits the client IP, the query string, and the body** (DESIGN:48; field list at
   R2-network-transport:318-324). So the signature does not bind the request to the network identity the
   rest of the system keys on, and any security decision that depends on a query parameter or body content
   is outside the signature.
2. **Header smuggling through Worker omission.** The Worker sets headers only where `request.cf` has a
   value. On non-Enterprise plans JA4 and Bot-Score are absent, so a client-supplied `X-Eq-Edge-JA4` /
   `X-Eq-Edge-Bot-Score` reaches the origin. Whether that is a 400 or a silently-ignored value depends on
   how the origin builds the canonical string — and the design does not specify that. This is the classic
   canonicalisation bug class (duplicate headers, case folding, comma-joining).
3. **The key is only as secret as the container environment.** `env_file: [.env]`
   (`docker-compose.yml:119`) feeds the api container; with the default all-RFC1918 trusted-proxy set
   (`docker-compose.yml:143`), any process on the Docker network that can also read the key can forge a
   complete "verified" request including `CF-Connecting-IP`. (The all-RFC1918 default is already a known
   finding; the new point is that the edge signature does not close it, because it does not cover the IP.)

New availability risk: a key rotation mistake, a clock skew beyond ±30s, or a Worker deploy that changes
the canonical string will 400 **all** traffic. The design says "reject unknown kid" (DESIGN:48) — that is
fail-closed on the whole site.

Verdict: worth building, but it is _transport integrity_, not _device attestation_. The design should say
that explicitly, because a reader currently reads "signed JA4" as "the client's TLS stack is attested."

### (d) What happens if the pepper / HMAC key leaks or is restored from a backup?

- **Leak (device pepper):** the attacker can compute `device_key` for any candidate component set. The
  component space for _real_ devices is small (browser family × version × platform × GPU family × locale ×
  screen bucket), so the attacker can (i) test whether a given device is already known or banned before
  acting — an enforcement oracle; (ii) pre-compute a collision with a target device so their abuse lands on
  someone else's record; (iii) if the same key is reused for edge signing, forge edge identity outright.
  There is no rotation story in DESIGN-v0, so the leak is permanent and unrecoverable without re-keying
  every row.
- **Restore from backup:** two distinct failures. (i) Restoring the _database_ to an earlier snapshot loses
  recent device rows → those devices are "new" → mass step-up. (ii) Restoring the _pepper_ from an older
  secret while the database is current makes every existing `device_key` unmatchable → every device is
  "new" → mass step-up, and if the risk engine treats new devices as risk, mass false positives. A routine
  ops action becomes a site-wide enforcement incident.
- The R2 report already prescribes the fix (R2-session-binding-rba §8.2: dual-key read window; §8.4:
  per-purpose key separation; §8.5: deleting salts destroys the ability to locate a subject's records —
  decide before launch whether erasure is supported). DESIGN-v0 adopts none of it.

### (e) Can the step-up challenge be used as a DoS against a legitimate user?

**Yes — two independent paths, and the design's own trigger list makes both cheap.**

Path 1 — **make the victim's device look new.** The device identity is defined by the _token_, which is
script storage. Any attacker who obtains the victim's session cookie (the design has no DBSC on Firefox/
Safari — already known) can present a different device payload from the stolen session. Under
"device change mid-session" (DESIGN:43) that marks the session as a device change; if step-up is required
to complete sensitive actions, the attacker can pin the victim into a challenge loop. The reverse also
works: the attacker rotates their _own_ device binding so the real user returns to a mismatched session.

Path 2 — **step-up fatigue without any attacker at all.** Safari's storage cap and iOS 26's denial of
long-lived script storage mean the token disappears on a timer; browser/OS updates change the fingerprint
(R2-fingerprint-science §2.3: 45.52% of fingerprints change after one day; §2.4: GPU driver and OS updates
change canvas/WebGL wholesale). A legitimate user on Safari or Firefox FPP will generate "new device"
events repeatedly. With no per-user prompt cap and no way for the legitimate user to satisfy the challenge
except by _being_ the same device, this is a lockout loop dressed as a security control.

Path 3 — **global grant-map saturation.** `MAX_GRANTS = 10_000` is a process-global cap on the fresh-auth
grant map (`src/services/fresh_auth.rs:16,100-102,176-178`). An attacker with many live sessions (logins are
100/min/IP; proxies are cheap) can saturate it so legitimate users receive `RateLimited` when they need a
grant (password change, 2FA setup, OAuth link). The map is also process-local, so the design's step-up
breaks across multiple API instances. Expanding step-up to device events makes this path far more common.

---

## 5. Top 8 attacks ranked by likelihood × damage, with the minimal fix

| #   | Attack                                                                                                                                                                           | L×D                 | Minimal design change                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | Client-authored coherence/tamper booleans ⇒ Layer 1 detection bypassed by one line of JS (F1)                                                                                    | Certain × high      | Server recomputes coherence from a **closed, typed** payload (enums and small ints only, no free-form strings); accept that a coarse typed subset _is_ stored under the 30-day keyed retention and amend Layer 0's "no raw values" claim accordingly. Never score a client-supplied boolean.                                                                                                                                                                             |
| 2   | Anonymous device-record farming ⇒ row growth + per-request writes + 10-second flush amplification on single-writer SQLite (F2)                                                   | High × high         | (a) No per-request DB write: fold `last_seen` into the existing 10s snapshot/flush, or drop `seen_count` entirely; (b) hard cap new device records per (IP unit, day) and per user; (c) validate the payload against a fixed schema _before_ any insert, and return a cheap 400 for invalid payloads; (d) never key the ban/limit store by an attacker-influenced cardinality.                                                                                           |
| 3   | Device signal only exists on account-touching flows ⇒ the scraping abuse the design exists to stop is never seen (F5)                                                            | Certain × high      | Make the collector send its own unauthenticated, rate-limited beacon from all pages (including reader routes) keyed by IP unit + `device_key`, independent of `AuthClient`. A client-side beacon does not touch the `/t/**` disk cache. If that is unacceptable, state plainly that anonymous abuse is addressed only by the edge signal + IP-unit cost, and remove the claim that the device axis covers it.                                                            |
| 4   | Bearer device token in `localStorage`/IDB on an origin whose CSP trusts third-party script origins ⇒ token theft ⇒ device-trust transfer + forged linkage (F4)                   | Med-high × high     | Move the token to a server-set `HttpOnly; Secure; SameSite=Lax; Path=/api` cookie (path-scoped ⇒ `/t/**` cache untouched). Rotate on login and privilege change; bind the token to the session at login; treat possession as **additive evidence only**, never as authority; drop `gstatic`/`googletagmanager` from `script-src` if they are not actually used.                                                                                                          |
| 5   | Edge signature does not bind IP/query/body; Worker omits headers it cannot fill ⇒ header smuggling; key in shared container env + default trusted CIDR ⇒ in-network forgery (F6) | Med × high          | Sign `CF-Connecting-IP` + path-with-query + a body digest; have the Worker **strip** all `X-Eq-Edge-*` before setting its own; verify a closed, mandatory, versioned header set; require Authenticated Origin Pulls or a Cloudflare Tunnel so the origin is unreachable otherwise; keep the edge key in a separate secret, not the general `.env`; make unknown-kid/clock-skew fail _open for read-only GETs_ with an alarm, so a rotation mistake is not a site outage. |
| 6   | Device-key ban axis: unstable under browser/driver updates (evasion) and cohort-wide (collateral bans) (F3)                                                                      | Certain × med-high  | Remove `device_key` as a standalone ban unit. Keep it as a weighted edge in the graph and make the durable ban **account-scoped**; require a second independent edge (credential or network or content) before any device-correlated enforcement; make the corroboration requirement a code precondition of the ban write, not a policy sentence.                                                                                                                        |
| 7   | Step-up is not a challenge primitive; triggers are attacker-influenceable; global grant cap ⇒ lockout and fatigue (F7)                                                           | Med-high × med-high | Implement step-up as a real session-bound challenge (WebAuthn UV or TOTP) with a **per-user** rate limit and no global cap; cap prompts (e.g. 1 per 5 minutes per session); a device mismatch may only _raise_ the requirement and must be satisfiable by the legitimate user; never let a client-asserted device signal gate a user action; re-bind the session's device only after a successful challenge.                                                             |
| 8   | Pepper has no versioning/rotation; leak or backup restore breaks or reveals the entire device store (F8)                                                                         | Low-med × high      | Per-purpose keys (`device:v1`, `edge:v1`); per-row key id + dual-key read window; the pepper lives in a secret store outside the DB backup; a documented "restore → re-key" runbook; and a launch decision on whether the pepper is retained for erasure (R2-session-binding-rba §8.5) — either answer is fine, an implicit answer is not.                                                                                                                               |

Remaining (below the top 8, still worth closing): appeal/notice as an oracle and laundering path plus an
unaudited admin surface (F9); published deterministic thresholds as a calibration target (F10); purpose
limitation asserted but not enforced, and `audit_logs` reuse (F11); retention ladder and DSAR path that
cannot locate the anonymous records it covers (F12).

---

## 6. Findings

### F1 — Coherence and tamper signals are client-authored, and the design forbids the server from recomputing them

**Severity: blocker. NEW.**
Why it breaks: DESIGN:29-30 has the collector emit `coherence booleans` and `spoof/tamper flags`; DESIGN:35
stores `coherence booleans, evidence JSON`; DESIGN:19 and :55 say no raw UA/UA-CH/screen values are stored
and "raw signals never stored". A server that only receives booleans must trust them, and they are one line
of JS to forge (`{webdriver:false, cdp:false, coherence:true}`). The server cannot recompute them from the
typed payload because the design forbids the raw inputs. This is not a tuning issue — it removes the entire
Layer 1 detection value against any attacker who reads the design. There is no server-side coherence code
in the repo today (`grep -rn "coherence" rust/backend/api/src web/src` → no matches), so this is a
greenfield design decision and the cheap moment to fix it.
Concrete fix: define a **closed** collector schema (fixed enums, small bounded integers, no free-form
strings), have the server recompute every coherence predicate from those typed fields, and amend Layer 0 to
say "we store the typed subset needed for coherence under the 30-day keyed retention" instead of "no raw
values". Never score a client-supplied boolean. Where a signal is only meaningful in raw form (UA vs
UA-CH), take it from the edge-signed header set rather than the client payload.

### F2 — Anonymous device-record farming + a per-request write on a single-writer SQLite pool

**Severity: blocker. NEW.**
Why it breaks: `device_key = HMAC-SHA256(components)` with `UNIQUE(device_key)` (DESIGN:35) and components
supplied by the client means the row count is attacker-controlled and unbounded. `user_id NULL` (DESIGN:35)
makes record creation possible pre-auth, so no account is needed. If `last_seen`/`seen_count` is maintained
per request, that is a write on the request hot path against an explicit repo invariant
(`src/services/rate_limit_store.rs:1`: "Do NOT add a DB write to the request hot path") on a pool of one
connection. Adding the device axis to the ban/limit key space also multiplies the 10-second L2 flush, which
upserts every snapshot row (`rate_limit_store.rs:180-229`) and loads the whole table at boot (`:75-110`).
Concrete fix: (a) no per-request write — update `last_seen` inside the existing 10s snapshot/flush, or drop
`seen_count`; (b) hard caps on new device records per (IP unit, day) and per user, with a metric on
cap-hits; (c) schema-validate the payload _before_ any insert and return a cheap 400 for anything that does
not match; (d) keep the ban/limit key space bounded by construction (never key a store by a value whose
cardinality the client controls).

### F3 — The device ban axis is a fingerprint hash: free evasion, cohort collateral, and no escalation path

**Severity: high. NEW.**
Why it breaks: DESIGN:37 bans on `device_key`, which DESIGN:35 derives from hardware/platform/locale/screen
components. Fingerprints are not stable (R2-fingerprint-science §2.3: 45.52% change after one day; §2.4:
driver/OS/browser updates change WebGL and canvas wholesale), so a browser update is ban evasion, and
anti-detect profiles (R2-evasion-counters §1.1) make it free. In the other direction the components are
coarse (mobile uniqueness 18.5%, overall 33.6% — R2-fingerprint-science §2.1), so identical hardware +
locale + screen buckets collapse to one key and a device ban is a cohort ban. DESIGN:45 says device may
"only ban with corroboration", but nothing makes corroboration a precondition of the ban write. In the repo,
`BanUnit` is IP-only and rejects non-IP strings (`src/modules/admin_bans_v1/dto.rs:31-38`), the admin list
filter drops non-IP units (`src/modules/admin_bans_v1/controller.rs:44-49`), and escalation handles IPs only
(`src/services/escalation.rs:72-78,133,172,207`) — so a device unit would have no escalation, list, or
export representation without new typed paths.
Concrete fix: make the durable ban **account-scoped** and treat `device_key` as one weighted edge, never a
standalone ban unit. Encode the corroboration rule as a precondition in the ban-write path (refuse the write
without a second independent edge). If a device unit must exist for admin inspection, give it an explicit
typed parse and keep it out of the escalation ladder.

### F4 — The device token is a bearer credential stored in `localStorage` on an origin that trusts third-party scripts

**Severity: high. NEW.**
Why it breaks: DESIGN:31 stores the token in `localStorage` + IDB; DESIGN:32 attaches it to every
`AuthClient` request. The web CSP allows `https://www.gstatic.com`, `https://www.googletagmanager.com`, and
`https://challenges.cloudflare.com` in `script-src` (`web/src/hooks.server.ts:80-85`), and
`challenges.cloudflare.com` is the design's own Turnstile step-up provider (DESIGN:42). Any script from
those origins runs first-party and can read the token. Because the token is stable and server-minted, theft
transfers device trust and forges a high-weight linkage edge — the "ant trap" poisoning pattern
(R2-evasion-counters §6.2) — and a shared device accumulates one device record across many accounts.
`AuthClient.#request` is also the CSRF bootstrap path (`auth-client.ts:232`), so the token is transmitted
by anonymous visitors too.
Concrete fix: server-set `HttpOnly; Secure; SameSite=Lax; Path=/api` cookie (path-scoped, so `/t/**` document
requests never carry it and the disk cache is untouched — the design's stated reason for `localStorage` does
not hold). Rotate on login and privilege change; bind to the session at login; treat possession as additive
evidence only. Remove `gstatic`/`googletagmanager` from `script-src` if nothing loads them (the repo has no
GTM/gtag usage — `grep googletagmanager web/src` → only the CSP line).

### F5 — The device signal exists only where `AuthClient` is used, so anonymous scraping is invisible

**Severity: high. NEW.**
Why it breaks: DESIGN:32 restricts transport to `AuthClient.#request` (all `/api/**`). In the repo,
`ensureAnonymousSession` is reached only from `unsafeRequest` (`web/src/lib/auth/auth-client.ts:257-259`)
and the OAuth flow (`oauth-flow.svelte.ts:102,145`); the root layout gates the session probe behind a
`localStorage` hint (`web/src/lib/auth/auth-state.svelte.ts:15-23`), so an anonymous cold load makes no
`AuthClient` call. A pure reader — and therefore every scraper fetching `/t/**` or `/api/quran` — never
transmits a device payload, and a scraper that does not run JS never loads the collector at all. The design's
stated goal is effective banning of abusive actors, but the device axis covers only clients that already
have an account, which are exactly the clients already bannable by `user_id`.
Concrete fix: give the collector its own unauthenticated, rate-limited beacon sent from all pages, including
reader routes (a client-side beacon does not affect the `/t/**` disk cache). Key it by IP unit +
`device_key`, cap it, and make it cheap. Otherwise, state explicitly in the design that anonymous abuse is
addressed only by edge signals and IP-unit cost, and drop the implied coverage claim.

### F6 — Edge signing does not bind the client IP, query, or body; Worker-omitted headers are smuggleable; the key shares the container env

**Severity: high. NEW (the all-RFC1918 trusted-proxy default is already known; the new part is that the signature does not close it).**
Why it breaks: DESIGN:48 signs `method + pathname + the X-Eq-Edge set + ts`. The field list
(R2-network-transport:318-324) does not include `CF-Connecting-IP`, so the signature does not bind the
network identity the system keys on, nor the query string or body. Cloudflare forwards client headers
through (R2-network-transport §7 item 1), and the Worker sets only the headers it has values for — JA4 and
Bot-Score are absent without Enterprise Bot Management (DESIGN:48 concedes this), so a client-supplied
`X-Eq-Edge-JA4`/`-Bot-Score` reaches the origin. Whether that is a 400 or a silently ignored value depends
on an unspecified canonicalisation step. The HMAC key lives in the api container's env
(`env_file: [.env]`, `docker-compose.yml:119`), and with the default all-RFC1918 trusted-proxy set
(`docker-compose.yml:143`) any process on the Docker network that can read the key can present a fully
"verified" request including a spoofed `CF-Connecting-IP`.
Concrete fix: sign `CF-Connecting-IP` + path-with-query + a body digest; have the Worker strip **all**
`X-Eq-Edge-*` before setting its own; verify a closed, mandatory, versioned header set with a spec'd
canonicalisation (reject duplicate/case-variant headers); require Authenticated Origin Pulls or a Cloudflare
Tunnel; keep the edge key in a dedicated secret rather than the general `.env`; make unknown-kid and
clock-skew fail _open for read-only GETs_ with an alert, so a rotation mistake is not a site-wide 400.

### F7 — Step-up is not a challenge primitive, its triggers are attacker-influenceable, and the grant map is a global-capped process-local bottleneck

**Severity: high. NEW.**
Why it breaks: DESIGN:43 calls the existing 5-minute fresh-auth grants "step-up". They are records of a
completed authentication bound to `session_id` + credential secrets
(`src/services/fresh_auth.rs:36-45,111-129,246-290`) — they cannot challenge anything, and at login there is
no grant yet. The map is process-global with `MAX_GRANTS = 10_000` (`:16,100-102,176-178`), so an attacker
with many live sessions (login is 100/min/IP; proxies are cheap) can saturate it and make legitimate users
receive `RateLimited` on password change / 2FA setup / OAuth link; it is also process-local, so step-up
breaks across instances. On the user side, "device change mid-session" is defined by a client-asserted
payload, so a stolen session can force the victim's session into a device-change state; and Safari's
script-storage cap plus normal browser updates produce "new device" events for legitimate users with no
prompt cap to stop the resulting fatigue loop.
Concrete fix: implement step-up as a real session-bound challenge (WebAuthn `userVerification: required` or
TOTP) with per-user rate limits and no global cap; cap prompts per session (e.g. one per five minutes);
device mismatch may only _raise_ the requirement and must be satisfiable by the legitimate user; never let a
client-asserted device signal gate a user action; re-bind the session's device only after a successful
challenge.

### F8 — No pepper versioning or rotation; a leak or a backup restore is unrecoverable and can mass-false-positive

**Severity: high. NEW.**
Why it breaks: DESIGN:35 specifies one server pepper with no key id per row and no rotation; DESIGN:48
specifies a `kid` for the edge key but no rotation schedule or dual-key window. A pepper leak permits
offline candidate testing over a small real-device component space (enforcement oracle + collision
pre-computation), and if the same key material signs edge headers, full edge forgery. A restore of the DB
loses recent rows (devices become "new"); a restore of the pepper from an older secret makes **every**
existing `device_key` unmatchable (every device becomes "new"). R2-session-binding-rba §8.2 and §8.5 already
prescribe the dual-key read window, per-purpose separation, and the erasure/retention decision.
Concrete fix: per-purpose keys (`device:v1`, `edge:v1`); per-row key id with a dual-key read window; the
pepper lives in a secret store outside the DB backup path; a tested "restore → re-key" runbook; and an
explicit launch decision on whether epoch keys are retained for erasure (either answer is acceptable;
an implicit one is not).

### F9 — The appeal flow and durable notice are an oracle and a laundering path; the admin surface is unaudited and unbound

**Severity: medium. NEW.**
Why it breaks: DESIGN:51 adds `/admin/devices` (list/lookup/ban/unban/evidence), a minimal UI, and an appeal
flow with a "durable notice"; DESIGN:44 forbids auto-permanent bans. Nothing in the design gives the admin
surface a separate credential, a step-up requirement, an admin-device binding, or a per-action audit trail —
and the repo's admin ban routes are session-ACL only (`src/modules/admin_bans_v1/mod.rs:17-33`) with
`audit_logs` dormant and repurposed by the design for _device evidence_ rather than admin actions
(`migration/src/m000001_init.rs:252-263`). A durable notice that names the triggering signal teaches the
attacker exactly which signal to change; an unbounded appeal flow is both a queue-DoS and a score-reset
mechanism if "restore <= 40" can be reached by appealing. `BAN_EXPORT_TOKEN` is a static bearer
(`src/config/settings.rs:415`, `src/modules/admin_bans_v1/controller.rs:188-222`) and becomes a device-ban
enumeration oracle if device units enter the export.
Concrete fix: generic notices (no signal detail); hard per-device and per-IP appeal caps with a cooldown;
restore requires new evidence rather than score decay; admin actions write an append-only audit row with
actor and before/after state; admin routes require step-up + role + a bound admin device; never export
device units through the static token.

### F10 — Deterministic published thresholds are a calibration oracle

**Severity: medium. NEW.**
Why it breaks: DESIGN:41 fixes `ban >= 80` / `restore <= 40` in a deterministic, versioned rule engine on an
open-source design. The attacker reads the weights, computes a request pattern that scores 79, and probes
the enforcement ladder (L1→L2→L3 transitions are observable via status codes and latency). The hysteresis
makes the boundary stable and repeatable, which is good for audit and bad for adversarial robustness.
Concrete fix: never expose a per-request score or reason to the client (and keep notices generic — see F9);
keep decision thresholds server-side and coarse; require a server-derived, non-forgeable signal class
(edge-signed TLS/ASN, IP unit, credential state) for any enforcement above L2; and add a small randomised
enforcement delay so the boundary is not a precise measurement instrument.

### F11 — Purpose limitation is asserted, not enforced; `audit_logs` reuse contradicts "raw signals never stored"

**Severity: medium. NEW.**
Why it breaks: DESIGN:54 promises device records are never joined to reading/recitation/search data, but
`device_records`, `device_sessions`, `user_sessions`, and the analytics tables live in one SQLite file
(single `DATABASE_URL`, `docker-compose.yml:128`), so the forbidden join is one `SELECT`. DESIGN:55 says
"raw signals never stored" while DESIGN:35 stores `evidence JSON` and DESIGN:38 reuses `audit_logs`, which
already carries raw `ip_address` and `user_agent` (`migration/src/m000001_init.rs:252-263`).
Concrete fix: separate the device store from the application DB (its own file/pool, or at minimum a schema
boundary with no joinable FK exposed to feature code) and add a machine guard that fails the build if
device tables appear in a join with reader/analytics tables — the repo already uses this pattern for other
invariants. Do not put device evidence in `audit_logs`; use a dedicated table with its own retention job.

### F12 — The retention ladder and DSAR path cannot locate the anonymous records they cover

**Severity: low-medium. NEW.**
Why it breaks: DESIGN:55 sets "keyed device key 30d → ban record 12mo → aggregates forever" and DESIGN:56
promises a DSAR path for device records. Because `device_key` is a keyed digest of values that are never
stored, a subject can only be located via `user_id` — and the anonymous majority of device records have
`user_id NULL` (DESIGN:35). So the DSAR path is decorative for exactly the records that hold the most
anonymous data, and "aggregates forever" derived from a retained pepper are re-identifiable (F8).
Concrete fix: state explicitly that anonymous device records are unlinkable by design and are excluded from
DSAR, recording that decision in the DPIA; or store a per-record random salt that is destroyed at the 30-day
boundary (which also bounds the ban record's matchability — so if the 12-month ban record must remain
matchable, key it by account where one exists and treat the anonymous case as an accepted 30-day window).

---

## 7. Bottom line

The design is a good sketch of the _data model_ and the _compliance posture_, and it correctly rejects
canvas/audio identifiers and behavioural biometrics. It fails as a _security control_ for three structural
reasons:

1. **It trusts the client for the one thing that matters** — the coherence and tamper verdicts (F1), while
   forbidding the server the raw inputs to check them.
2. **It keys enforcement on a value the attacker controls** — the device record's cardinality (F2) and the
   device ban unit's fingerprint components (F3), while the one identifier the defender _does_ control (the
   token) is stored where any trusted script can read it (F4).
3. **It covers the wrong traffic** — device identity is transmitted only by account-touching flows (F5),
   while the abuse the design targets is anonymous and server-side (F6/F5).

The three changes that buy the most: recompute coherence server-side from a closed typed payload (F1); never
write to the DB on the request path and cap device-record creation (F2); move the token to a path-scoped
HttpOnly cookie and demote it to additive evidence (F4), with the ban itself account-scoped (F3).
