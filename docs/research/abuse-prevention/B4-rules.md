# B4 — Concrete v1 detection rule set: thresholds, precision estimates, evasion analysis

Answers Q4 of `DESIGN-v1.md` (§C). Inputs: `DESIGN-v1.md` B2/B3/B5; `A1-threat-model.md` F10 (published
deterministic thresholds = calibration oracle) and F1/F2/F3/F5; `A2-false-positives.md` F5–F10 (coherence
false positives, release-day waves, proxy-rendered browsers, in-app browsers, storage clearing);
`R2-fingerprint-science.md` §1 (entropy table), §2 (uniqueness/stability), §5 (coherence beats uniformity),
§7.2 (Tier A–C); `R2-evasion-counters.md` §3.2 (consistency lattice), §6 (linkage), §9 (master table),
§10 (top 8 countermeasures); plus the repo (`rate_limit_store.rs`, `escalation.rs`, `quran_v1/mod.rs`,
`middlewares/rate_limit.rs`, `device_v1/controller.rs`).

**Status: design. Every number here is a v1 default to be recalibrated in shadow mode. No rule in this
document may be enforced above L2 before it passes the §6 gate. The v1 enabled subset is §8.**

---

## 0. Scope and method

1. A rule may consume **only** (a) server-recomputed predicates over the closed typed payload, and (b)
   server-side data the client does not author. Client-supplied _verdicts_ are never inputs (§1.3).
2. "Deterministic" means the trigger is a non-forgeable server-side fact or a counter. Deterministic rules
   may auto-act, but "auto-act" is bounded: L1/L2 friction is automatic; L3+ requires a human or a
   deterministic rule whose action is capped at L4 (queue-only, no restriction).
3. "Scored" means the trigger depends on a distribution, a correlation, or a page-supplied value. Scored
   rules run in shadow mode (L0) until §6's gate is met.
4. Every rule must state its **cheapest evasion** and its **residual**. A rule with no residual is a
   liability (A1 F10): the attacker reads it, changes one field, and the detection is gone.
5. Thresholds are **not published** and never returned to the client (A1 F10). Notices name categories,
   never weights (R2-ban-operations §5).

---

## 1. What the server may use

### 1.1 Closed typed payload (client-collected, server-recomputed)

Fixed enums and bounded integers only. No free strings, no raw UA/screen/plugins, no client booleans
(`DESIGN-v1` B2; `A1` F1's fix). Each field carries a state, per `A2` §2.2: `{value, masked, unavailable}`.
The server stores the _derived_ predicates and the HMAC'd key, not the raw tuple.

```
p.v                 = 1                      payload version
p.ctx               = {browser|webview|standalone|unknown}
p.engine            = {chromium|gecko|webkit|unknown}
p.engine_major      = u16 (0 = unknown)
p.ua_family         = {chrome|edge|firefox|safari|samsung|opera|webview|other|unknown}
p.ua_major          = u16
p.ua_ch             = {present|absent|unknown}   // navigator.userAgentData existed
p.os_family         = {android|ios|windows|macos|linux|chromeos|other|unknown}
p.os_major          = u16
p.platform          = {desktop|mobile|tablet|unknown}
p.hw_gpu_family     = {apple|nvidia|amd|intel|qualcomm|arm|mali|imagination|masked|unknown}
p.hw_gpu_class      = u8   // 0 unknown, 1..8 perf bucket
p.hw_ext_hash       = u32  // WebGL extension-set hash
p.hw_shader_prec    = u8
p.hw_cores          = u8   // bucketed {0,1,2,4,8,16}
p.hw_mem            = u8   // bucketed {0,1,2,4,8}
p.touch             = u8   // {0,1,5,10}
p.screen_class      = u8   // coarse bucket; session-stable set, max/modal
p.dpr_bucket        = u8
p.lang_js_primary   = u8   // index into a server-side BCP-47 allowlist
p.lang_js_list_hash = u32
p.intl_locale       = u8   // index
p.intl_calendar     = u8
p.speech_lang       = u8   // index; 0 = unavailable
p.tz_offset_min     = i16  // bucketed to 30 min
p.features          = u64  // version-gated API presence bitmask (server-side matrix)
p.perms_notif       = {granted|denied|prompt|unknown}
p.display_mode      = {browser|standalone|fullscreen|minimal-ui|unknown}
```

`device_key = HMAC-SHA256(DEVICE_KEY_PEPPER, canonical(payload))` — the key is a _cohort/evidence_ key,
never an enforcement unit (`DESIGN-v1` B1/B3, `A1` F3, `A2` F1/F2).

### 1.2 Server-side data (existing or cheap to add)

| Source                                         | Fields                                                                                                               | Status                                                                                                                                                |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Request headers (page JS **cannot** set these) | `User-Agent`, `Sec-CH-UA`, `Sec-CH-UA-Platform`, `Sec-CH-UA-Mobile`, `Accept-Language`, `Sec-Fetch-*`, HTTP version  | present                                                                                                                                               |
| Edge-signed headers (Ed25519, `DESIGN-v1` B4)  | `country`, `asn`, `JA4` when the plan exposes it, `cf-ray`, client IP                                                | designed                                                                                                                                              |
| IP unit                                        | `/32` v4, `/64` v6 (canonical `BanUnit`); an ASN bucket would need a Worker header or a baked IP→ASN table (B3 §4.2) | IP unit present; ASN missing                                                                                                                          |
| Gate store (in-memory, flushed every 10 s)     | fixed-window counters, block state, escalation suspicious counters                                                   | present                                                                                                                                               |
| Auth events                                    | credential id, outcome, session id, timestamp                                                                        | needs an aggregate ring in the gate store                                                                                                             |
| Ban state                                      | `user_bans` (wired), `device_bans`, `device_sessions`                                                                | `user_bans` exists with zero callers (`DESIGN-v1` B1)                                                                                                 |
| Content                                        | server-computed normalized hash of post/comment bodies                                                               | **missing** (only `media.content_hash` exists) — R04b/R10/R11 need it added                                                                           |
| Quotas                                         | per-credential and per-API-key counters                                                                              | per-credential counters exist (`login:`, `totp:`, `reauth:`, `forgot_password_*`, `email_verify:` — B2 §0) but count successes; no API keys exist yet |
| Request shape                                  | matched route template, status, response bytes, timestamp                                                            | present in metrics; needs a bounded per-subject rolling sketch                                                                                        |
| Registration                                   | creation time, normalized identifier domain, `device_key`                                                            | creation time + email present; `device_key` is designed, not built                                                                                    |

**Hot-path discipline (`rate_limit_store.rs:1`):** no rule may add a DB write or read to the request path.
Scored rules evaluate from the gate store + session payload on the existing ticker; observations are
written by the existing 10 s snapshot / 60 s slow-flush cadence. `DESIGN-v1` B3 already states "0 DB ops on
the request path".

### 1.3 The four prohibitions

**(a) Client-asserted booleans — forbidden.** No rule may consume `spoof_webdriver`, `spoof_native_tamper`,
`spoof_cdp`, `coherence_*`, or any `{coherence: true}`-shaped verdict. These columns may be **stored as
evidence** for a human; they may never be a rule input. This is `A1` F1: one line of JS otherwise sets them.

**(b) Privacy-browser / VPN / VM / DevTools presence — forbidden.** No rule may key on Tor, VPN use,
incognito, a VM, a software renderer (`SwiftShader`, `llvmpipe`, `Microsoft Basic Render Driver`), a
privacy-hardened OS, or an open DevTools session. This forces two drops from the `R2-evasion` §9 catalogue:
the software-renderer/VM tell (row 3.1) and the CDP `Runtime.enable` trap (row 2). Both are genuinely
useful against the median bot and both are forbidden here, because their false-positive population is
"developers, VMs, and the privacy-conscious" — DevTools open is ~6% of desktop sessions (P10), and the
Canvas-Blocker-class extension population is ~20K users (P11). Record the drop; do not quietly re-add.

**(c) Single-signal evidence — forbidden above L2.** No rule may reach L3 or above on one signal. Scope
note, stated explicitly rather than assumed: **L0–L2 rules may be single-signal**, because L0–L2 is
reversible friction (observe / throttle / challenge) with self-service clearing (`A2` §5.2 items 2–8).
A single-signal _ban_ is the v0 flaw. The algebra in §2 makes the distinction arithmetic, not policy.

**(d) Behavioural biometrics — forbidden.** No keystroke dwell/flight, mouse curvature/velocity, scroll
kinematics, touch dynamics, or form-fill timing. GDPR Art. 9 exposure and an 80% F1 on lab data
(`R2-evasion` §5.2, row 14). **Distinction that matters:** _server-side request-interval entropy,
navigation-graph template, and asset-completeness ratio_ are admissible — they are properties of the HTTP
request stream, not of a person's motor signature, and `R2-evasion` §10 item 6 names them as the
Art-9-clean version. `R07` is admissible; keystroke capture is not.

### 1.4 Admissibility test applied to every rule below

A rule input is admissible iff the server computes the predicate **and** it is one of:

- **H — header-anchored.** The value comes from an HTTP header the page cannot set (`User-Agent`,
  `Sec-CH-UA*`, `Accept-Language`, `Sec-Fetch-*`) or from an edge-signed field. Page JS can read
  `navigator.userAgent` and lie about it; it cannot change the `User-Agent` header the engine sends. This
  is the only non-forgeable coherence anchor available in v1 (`R2-evasion` §3.1 "UA vs Client Hints:
  highest yield, zero client JS").
- **S — server state.** Counters, ban state, credential identifiers, content hashes, quotas, account
  history. The attacker does not author it.
- **T — request shape.** Inter-arrival gaps, route mix, asset ratio, enumeration coverage. Server-observed,
  distribution-level.
- **P — page payload.** Typed enums from §1.1. Forgeable by a patched client, but only _consistently_: a
  P-class signal must be cross-checked against H, S, or T to be worth anything, and P alone caps at L2
  (see §2).

Every rule below is tagged with the classes it consumes. A rule that is P-only and has no H/S/T
cross-check is not in this document.

---

## 2. Evidence-class algebra — the structural fix to v0's flaw

The v0 flaw: one attacker-controlled input could push a score across the ban threshold. A weight cap does
not fix it (a single input can still be the marginal contributor). The fix is to make class composition a
precondition of the _threshold_, not a policy sentence (`A1` F3: "the design says corroboration is a
precondition; nothing makes it one"):

```
score = Σ over classes C ∈ {H, S, T, P} of min(CLASS_CAP, best_rule_in_family(C))
CLASS_CAP = 40
```

| Level                    | Enter     | Exit | Class precondition (enforced in code)                                                                                                                     |
| ------------------------ | --------- | ---- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| L1 throttle              | 20        | 10   | any                                                                                                                                                       |
| L2 challenge             | 40        | 20   | any (so **P alone maxes out at L2** — arithmetic, not policy)                                                                                             |
| L3 cooldown              | 60        | 30   | ≥2 distinct classes, ≥1 of them H or S                                                                                                                    |
| L4 proposal (queue-only) | 80        | 40   | ≥2 classes with both contributing ≥20, ≥1 H or S; or a deterministic rule                                                                                 |
| L5/L6                    | human     | —    | never automatic; permanent needs a recorded second look — two-person where a second reviewer exists, otherwise auto-expiry + recorded self-review (B9 F4) |
| L7                       | statutory | —    | CSAM hash / court order / NEA                                                                                                                             |

Consequences, all deliberate:

- A payload-only attacker can reach L2 and no further. That is the price of using forgeable input at all.
- Two classes at cap (40 + 40) reach exactly the L4 proposal line and not one point more. There is no
  arithmetic path from "one signal" to a restriction.
- The ban threshold (80) and restore threshold (40) from `DESIGN-v1` B5 remain the outer pair; the ladder
  thresholds above are nested inside them.
- Hysteresis is per-level (enter/exit in the table) **and** per-rule (§4). The outer pair is never
  exposed to the client, and enforcement takes effect after a fixed, documented grace period — not a
  randomized delay, which would break reproducibility, DSAR answers and Art 22(3) contestation (B9 F10).

**Evidence families** (dedup unit — only the best rule in a family contributes, never the sum):

| Family            | Members             | Why they must not stack                                                                                                   |
| ----------------- | ------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| credential        | R01, R02            | both read the same failure stream                                                                                         |
| registration      | R05                 | single source                                                                                                             |
| device-population | R03, R04c           | both read `device_sessions`                                                                                               |
| payload-coherence | R12–R17             | one page payload cannot be five independent facts (`R2-fingerprint-science` §5.1: correlated surfaces add almost nothing) |
| request-shape     | R07, R08, R09-shape | one request stream                                                                                                        |
| content           | R10, R11            | same bodies                                                                                                               |
| linkage           | R04a, R04b          | superseding, not additive                                                                                                 |

---

## 3. Rule table (compact)

Classes: H header-anchored · S server state · T request shape · P page payload.
Det = deterministic (auto-act, bounded) · Scored = shadow mode first.
Hysteresis shown as enter/exit. Action is the **ceiling** the rule may reach alone.

| ID       | Rule                                    | Trigger (exact)                                                                                                                                                                                     | Cls   | Action                                                                    | Kind         | Hyst.                   | TP / FP                                                                                           |
| -------- | --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----- | ------------------------------------------------------------------------- | ------------ | ----------------------- | ------------------------------------------------------------------------------------------------- |
| **R01**  | Per-credential failure burst            | `count(fail(credential_id), 600s) ≥ 5`; at `≥ 20/24h` → queue                                                                                                                                       | S     | L1 progressive delay; L4 queue at 24h tier                                | Det          | — / 24h dedup           | TP unmeasured; FP high by base rate (typos, password managers) → **never lock**                   |
| **R02**  | Credential-stuffing shape               | per IP unit, 900s: `distinct_credentials ≥ 10` ∧ `failure_ratio ≥ 0.9` ∧ (`gap_cv ≤ 0.35` ∨ `gap_entropy ≤ 0.5b` over ≥10 gaps) ∧ (`distinct_device_keys ≥ 2` ∨ `payload_absent`)                   | S T P | L2 login challenge + L1; ≥3 windows/24h → L4 queue                        | Scored       | 60/25                   | unmeasured — shadow first                                                                         |
| **R03**  | One-account-many-devices fan-out        | `distinct device_record_id per user in 30d ≥ 6` ∧ `≥3 within one 24h` ∧ `writes ≥ 1 in that 24h`                                                                                                    | S     | **L0**; L4 queue only if a Class-H/S abuse rule also fired on the account | Scored       | n/a (no action)         | FP ≫ TP if punitive: 32% of instances clear cookies in 8mo (P4); ~31% mobile sessions in-app (P5) |
| **R04a** | Prior ban + identifier reuse            | normalized recovery identifier (email, phone E.164, OAuth subject) HMAC-equals one on an account with an active or ≤12mo ban for {spam, harassment, threats, CSAM, evasion}                         | S     | **L4 queue-only, hard expiry ≤72h + human.** Never auto-ban               | Det          | — / 12mo per identifier | TP unmeasured; FP real: family-shared email/phone (`A2` §3.1)                                     |
| **R04b** | Prior ban + identical violating content | HMAC(normalized body) equals the body that was the basis of a ban; CSAM-class → statutory path                                                                                                      | S     | L4 queue + human; CSAM → L7                                               | Det          | —                       | unmeasured; framing is possible (`A2` §3.1) → human mandatory                                     |
| **R04c** | Prior ban + retained device             | new account binds a `device_record_id` linked to a banned account ≤30d ago; `G01` passes                                                                                                            | S P   | **L4 queue-only.** Never a ban input alone                                | Scored       | 50/20                   | unmeasured; both FP and FN expected (P1: 45.52%/day change)                                       |
| **R05**  | Mass account creation burst             | per IP unit, 3600s: `new_accounts ≥ 5` ∧ (`distinct_device_keys ≥ 3` ∨ `payload_absent`) ∧ (disposable domain ∨ `registration_gap_cv ≤ 0.3`)                                                        | S T P | L2 registration challenge + L1; ≥10/24h → L4 queue                        | Scored       | 65/30                   | unmeasured; school/NAT bursts are the FP class → `G07`                                            |
| **R06**  | Per-credential / per-key quota          | credential or API key over its published quota                                                                                                                                                      | S     | L1 → 429 + `Retry-After`; ≥3 windows → L2 on the key. Never ban           | Det          | —                       | FP ≈ 0 **if** the quota is published; mitigation = the public bulk path                           |
| **R07**  | Scraping cadence                        | per IP unit/key, 3600s, browser-claiming subject only: `content_reqs ≥ 100` ∧ `asset_ratio ≤ 0.02` ∧ (`gap_entropy ≤ 0.5b` ∨ `gap_cv ≤ 0.2`) ∧ (`adjacent_id_rate ≥ 0.8` ∨ `corpus_coverage ≥ 0.5`) | T H   | L1 rate-shape (1 req/s); L2 on the bulk path only. Reading never blocked  | Scored       | 60/25                   | unmeasured; IP reputation forbidden (P8: 4/5 residential-proxy reqs legitimate)                   |
| **R08**  | Sequential-ID enumeration               | per IP unit, 300s: numeric/ULID-param reqs ≥ 30 ∧ `404+403 ratio ≥ 0.5` ∧ `distinct_params ≥ 20` ∧ `monotonic_adjacency ≥ 0.6`                                                                      | T     | L1 on that route; ≥3 windows → L2                                         | Scored       | 55/20                   | unmeasured; exempt declared crawlers + `.md`/`llms.txt` (`G06`)                                   |
| **R09**  | Search abuse                            | quota: `/search` > 20/min per IP unit → L1. Shape: ≥100 queries ∧ `distinct_ratio ≥ 0.9` ∧ `median_len ≥ 3 words` → L2                                                                              | S T   | L1 / L2                                                                   | Det + Scored | 60/25                   | unmeasured; low risk (search is not a read gate)                                                  |
| **R10**  | Write-spam burst                        | per account, 600s: `writes ≥ 8` ∧ `distinct_body_hash_ratio ≤ 0.3` ∧ `write_gap_cv ≤ 0.25` ∧ (`age < 24h` ∨ `no read before first write`)                                                           | S T   | L1 (writes only); ≥3 windows → L4 queue                                   | Scored       | 70/30                   | unmeasured                                                                                        |
| **R11**  | Cross-account duplicate content         | same HMAC(body) posted by ≥3 distinct accounts in 3600s, **and the body was previously human-confirmed violating**                                                                                  | S     | L4 queue (cluster review ≤20 accounts) + L1 writes until reviewed         | Det + human  | —                       | **collides with canonical Quran text if unscoped** → human-confirmed-only scoping is mandatory    |
| **R12**  | UA-CH impossible                        | `p.ua_ch = present` ∧ no `Sec-CH-UA` header ∧ `ctx = browser` ∧ `ua_family ∉ {opera, webview}` ∧ not a known header-stripping class                                                                 | H P   | L1, cap L2                                                                | Scored       | 40/10                   | unmeasured; logical basis strong (`navigator.userAgentData` ⇒ Chromium ⇒ header sent)             |
| **R13**  | Engine/header mismatch                  | `p.engine ∉ unknown` ∧ header family ⇒ different engine ∧ not in the fork allowlist (Samsung, Yandex, UC, Opera, CriOS, WebView, ChromeOS)                                                          | H P   | L1, cap L2                                                                | Scored       | 50/15                   | unmeasured; forks are the FP class (`A2` F6)                                                      |
| **R14**  | Platform/header mismatch                | `p.os_family` vs `Sec-CH-UA-Platform`/UA OS token disagree ∧ not in the masked allowlist (Brave, Safari AFP, Firefox FPP, Tor, Lockdown, iPad desktop, frozen `Android 10`)                         | H P   | **L1 cap** (advisory)                                                     | Scored       | 60/25                   | naive form flags 90.62% of Indian mobile (P6) → directional only                                  |
| **R15**  | Hardware/platform impossible            | `touch = 0` ∧ `os_family ∈ {ios, android}` ∧ `ctx = browser` ∧ not desktop-mode iPad; or `hw_gpu_family = apple` ∧ `os_family ∈ {windows, android}`; Firefox FPP clamp signature excluded           | P     | L1, cap L2                                                                | Scored       | 55/20                   | unmeasured; `A2` F6 lists these exact FP configs                                                  |
| **R16**  | Language quintuple                      | server `Accept-Language` primary ∉ payload language set, or `intl_locale` ≠ primary                                                                                                                 | H P   | **L0 advisory only**                                                      | Scored       | n/a                     | high FP: VPN, translation, Firefox RFP (`A2` B9)                                                  |
| **R17**  | Session field drift                     | same `user_session_id`, two beacons ≥60 s apart: `engine`, `engine_major` (downward), `os_family`, `platform`, `hw_gpu_family` (≠masked), or `lang_js_primary` changes                              | P     | L1, cap L2                                                                | Scored       | 50/15                   | release-day waves (`A2` F7) → `G05` damping                                                       |
| **G01**  | Cohort non-actionability                | `device_key`/`cohort_key` with ≥4 distinct accounts or ≥6 tokens in 30d                                                                                                                             | S     | suppresses all device-punitive rules for that key; log only               | Det          | always on               | guard only                                                                                        |
| **G02**  | Action budget                           | ≤1 review case per key per 7d; ≤1 challenge per session per 5 min                                                                                                                                   | S     | suppresses                                                                | Det          | always on               | guard only                                                                                        |
| **G03**  | Shared-device / org claim               | registered kiosk/org device or self-service shared-device claim                                                                                                                                     | S     | step down one level; never clears an account ban                          | Det          | always on               | guard only                                                                                        |
| **G04**  | Bias circuit breaker                    | per-family challenge rate (Safari/Brave/Firefox/WebView/Tor) > 3× Chrome baseline over 24h                                                                                                          | S     | auto-suspend the offending rule to L0 + alert                             | Det          | always on               | guard only                                                                                        |
| **G05**  | Release-day damping                     | 72h after an observed `engine_major` mode shift                                                                                                                                                     | S     | +15 to all coherence enter thresholds; update-shaped deltas = same device | Det          | always on               | guard only                                                                                        |
| **G06**  | Reader-path exemption                   | `/t/**`, `/{surah}/t/**`, quran content API, search, `.md`/`llms.txt`                                                                                                                               | —     | no device rule may read or gate these; machine-guarded                    | Det          | always on               | guard only                                                                                        |
| **G07**  | Shared-IP suppression                   | IP unit with ≥20 distinct accounts each active ≥3 days in 30d                                                                                                                                       | S     | suppress all IP-unit-scoped scored rules for that unit                    | Det          | always on               | guard only                                                                                        |

---

## 4. Rule detail

### R01 — Per-credential failure burst

- **Trigger.** Fixed window per `credential_id` (account): `count(outcome = failure, 600s) ≥ 5` → L1
  progressive delay on that credential (1 s, 2 s, 4 s, 8 s, cap 8 s) plus L2 challenge on the _session_
  that is failing. `count ≥ 20 in 24h` → L4 queue-only case (the account is probably under a targeted
  attack, not the attacker).
- **Data.** `(credential_id, outcome, ts, ip_unit, session_id)` ring in the gate store; no DB.
- **Kind.** Deterministic (a counter), action bounded at L1/L4-queue.
- **Precision.** TP unmeasured — shadow-mode first. FP base rate is high and known: password managers,
  keyboard-layout confusion, and an expired password produce 5-in-10-min failures for real users. That is
  why the action is **delay, never lock** (see F2).
- **Cheapest evasion.** Spread one attempt per credential across many accounts — which is exactly the
  shape R02 detects; the two rules are a pair, not alternatives.
- **Hysteresis.** None (fixed-window reset). L4-queue case dedup 24h.

### R02 — Credential-stuffing shape

- **Trigger.** Per IP unit, one 900 s window: `distinct_credentials_attempted ≥ 10` **and**
  `failure_ratio ≥ 0.9` **and** `gap_cv = σ(gaps)/μ(gaps) ≤ 0.35` or `H_bins(gaps) ≤ 0.5 bits` over ≥10
  gaps (8 log-spaced bins: <50 ms, <100, <200, <400, <800, <1.6 s, <3.2 s, ≥3.2 s; Shannon entropy of the
  bin distribution) **and** (`distinct_device_keys ≥ 2` or no payload).
- **Data.** Bounded per-IP-unit ring buffer (64 entries) of `(credential_id, outcome, ts, device_key)`.
- **Kind.** Scored. Shadow first.
- **Precision.** TP unmeasured — shadow-mode first. `R2-evasion` §10 ranks per-credential counters #1
  impact/effort but publishes no precision; there is no source for this rule's precision on our traffic.
- **Cheapest evasion.** Residential proxy rotation with one attempt per credential per exit (P8 shows why
  IP reputation cannot be the backstop). Residual: R01's per-credential counters, and the identity
  provider's own lockout on the accounts that are actually being attacked.
- **Hysteresis.** Enter 60 / exit 25; minimum hold 30 min.

### R03 — One-account-many-devices fan-out

- **Trigger.** `distinct device_record_id` bound to one `user_id` in 30 d ≥ 6, of which ≥3 fall inside a
  single 24 h window, and the account wrote at least once in that window.
- **Data.** `device_sessions` (`idx_device_sessions_user_linked`).
- **Kind.** Scored, **evidence-only**.
- **Precision.** This rule is a _false-positive generator by construction_ if made punitive. Sourced base
  rates: 32% of browser instances cleared cookies in 8 months (P4); ~31% of mobile web sessions are
  in-app-browser sessions with separate storage (P5); iOS home-screen apps are storage-isolated from
  Safari (`A2` F11). `A2` F5 states the conclusion directly: "one account, many device records is normal;
  no fan-out feature" and "device churn is never a risk feature".
- **Cheapest evasion.** Not applicable — the rule never acts alone.
- **Hysteresis.** n/a; observation dedup 7 d.

### R04a — Prior ban + identifier reuse (deterministic)

- **Trigger.** `HMAC(id_pepper, normalize(identifier))` for the new account's recovery email, phone
  (E.164), or OAuth subject matches an identifier on an account whose ban is active or expired ≤12 months
  ago, with reason in {spam, harassment, threats, CSAM, ban_evasion}. Comparison is exact on the HMAC.
- **Data.** `users`, `user_bans` (wire it — `DESIGN-v1` B1), identifier HMAC set.
- **Kind.** Deterministic. Safe to auto-act **at L4 only** (queue-only, hard expiry ≤72 h, no restriction,
  human decides).
- **Precision.** TP unmeasured — shadow-mode first. FP is real and must be budgeted: family-shared email
  and phone are common in this audience (`A2` §3.1). That is why the action is a hold, not a ban.
- **Cheapest evasion.** New email and phone per identity. Residual: R04b (content) and R04c (device).
- **Hysteresis.** n/a; case dedup per identifier 12 months.

### R04b — Prior ban + identical violating content (deterministic)

- **Trigger.** `HMAC(content_pepper, normalize(body))` equals the normalized body that was the recorded
  basis of a prior ban, where the prior ban's reason is content-based. CSAM-class hash match goes straight
  to L7 (statutory; `DESIGN-v1` B5, `R2-ban-operations` §7).
- **Data.** `posts`/`comments`, ban records, the frozen evidence snapshot.
- **Kind.** Deterministic; human review mandatory before L5+.
- **Precision.** Unmeasured. FP mechanism is documented: a malicious co-user can reproduce the payload to
  frame another account (`A2` §3.1). Human review is the control, not a threshold.
- **Cheapest evasion.** Paraphrase or rewrite. Residual: none from this rule — accept it; the rule exists
  to catch the naive re-poster.
- **Hysteresis.** n/a.

### R04c — Prior ban + retained device (scored, evidence-only)

- **Trigger.** A new account binds a `device_record_id` that is linked (via `device_sessions`) to an
  account banned ≤30 d ago, **and** G01 passes for that key.
- **Data.** `device_sessions` (`idx_device_sessions_record_user`), `device_bans`.
- **Kind.** Scored, L4 queue-only, **never a ban input alone** (`A2` §3.3; `DESIGN-v1` B1 "device signals
  may challenge; they may never be the sole cause of a ban").
- **Precision.** Unmeasured. Sourced priors bound the error in both directions: 45.52% of fingerprints
  change after one day (P1, on FP-Stalker's larger attribute set), so "same device" is a weak positive; mobile uniqueness was 18.5% on a 2018 French population (P3), so
  "same key" is partly "same model + locale + screen bucket" and the FP is a _cohort_, not a person.
- **Cheapest evasion.** Anti-detect profile or a different phone (`R2-evasion` §1.1). Residual: R04a and
  R04b — which is why the linkage family is designed around credentials and content, not devices.
- **Hysteresis.** Enter 50 / exit 20.

### R05 — Mass account creation burst

- **Trigger.** Per IP unit, 3600 s: `new_accounts ≥ 5` **and** (`distinct_device_keys ≥ 3` or no payload)
  **and** at least one of: normalized email domain ∈ local disposable-domain list; `registration_gap_cv ≤
0.3`.
- **Data.** Registration events, the local disposable-domain list (no external calls), gate-store counters.
- **Kind.** Scored.
- **Precision.** Unmeasured — shadow-mode first. The FP class is named in advance: a madrasa, a school lab,
  or a cyber-café produces exactly this shape. G07 (`ip_legit_share`) exists because of it
  (`R2-ban-operations` §3: "a madrasa IP with 40 daily active readers is evidence of a school, not a farm").
- **Cheapest evasion.** Residential proxy, one account per exit. Residual: identifier linkage (R04a) and
  content (R04b/R11).
- **Hysteresis.** Enter 65 / exit 30.

### R06 — Per-credential and per-API-key quota (deterministic)

- **Trigger.** A credential or API key exceeds its published quota. Concrete v1 defaults: content API
  already 600/min/IP (`middlewares/rate_limit.rs`); add per-key daily byte quota (start 50 MB/day for free
  keys), login 100/min/IP (exists), search 20/min/IP unit (new, tighter — `quran_v1/mod.rs:56` already
  argues search must not inherit the coarse limit).
- **Data.** Per-key and per-credential counters in the gate store.
- **Kind.** Deterministic.
- **Precision.** FP ≈ 0 by construction _provided the quota is published_. The real risk is a quota too low
  for legitimate bulk consumers, and the mitigation is `R2-evasion` §10 item 5: publish a cheap bulk path
  and make the honest route faster than the scrape.
- **Cheapest evasion.** New key or account. Residual: R05 (creation burst) and per-credential counters.
- **Hysteresis.** None (window reset).

### R07 — Scraping cadence (payload-free)

- **Trigger.** Per IP unit or API key, 3600 s, **only for subjects that claim to be a browser** (UA family ∈
  browser set or payload present): `content_requests ≥ 100` **and** `asset_ratio = asset_requests /
content_requests ≤ 0.02` **and** (`H_bins(gaps) ≤ 0.5 bits` or `gap_cv ≤ 0.2`) **and**
  (`adjacent_id_rate ≥ 0.8` or `corpus_coverage ≥ 0.5`). Declared non-browser clients (curl/python/etc.)
  are routed to R06 instead — the honest API path, not the shape path.
- **Data.** Per-subject rolling sketch of `(route_template, ts, bytes, status)`; bounded 64 gaps. No
  payload, no cookies needed — this is the only rule class that works on the anonymous reader path
  (`A1` F5; `DESIGN-v1` B2 forbids the collector on reader routes).
- **Kind.** Scored. Shadow first.
- **Precision.** Unmeasured — shadow-mode first. Two sourced priors shape it: full-text scrape is the
  "very likely" attack for this corpus (`R2-evasion` §8.2), and 4 of 5 requests from residential-proxy IPs
  are legitimate household traffic (P8), so the rule is shape-based and never IP-reputation-based.
- **Cheapest evasion.** Randomize delays, fetch a few assets, spread across exits. Residual: the quota
  (R06) plus the published bulk path — the goal is cost, not prevention (`R2-evasion` §8.2: "target: cost
  per full copy > cost of getting it from an existing dataset").
- **Hysteresis.** Enter 60 / exit 25.

### R08 — Sequential-ID enumeration

- **Trigger.** Per IP unit, 300 s: requests carrying a numeric or ULID path param ≥ 30 **and**
  `(404 + 403) / total ≥ 0.5` **and** `distinct_param_values ≥ 20` **and** monotonic adjacency ≥ 0.6.
- **Data.** Route-template + status + param sketch.
- **Kind.** Scored.
- **Precision.** Unmeasured. FP class: link checkers, search-engine crawlers, and deep-link previews —
  exempt declared crawlers and G06 routes.
- **Cheapest evasion.** Randomize the id order. Residual: corpus-coverage term in R07.
- **Hysteresis.** Enter 55 / exit 20.

### R09 — Search abuse

- **Trigger.** Quota tier (deterministic): `/search` > 20/min per IP unit → L1. Shape tier (scored):
  ≥100 queries in 3600 s with `distinct_query_ratio ≥ 0.9` and median query length ≥ 3 words → L2
  challenge.
- **Data.** Search bucket counters (the module already isolates search for a tighter limit).
- **Kind.** Deterministic quota + scored shape.
- **Precision.** Unmeasured; low blast radius because search is not a read gate.
- **Cheapest evasion.** Distribute across IP units. Residual: per-key quota (R06).
- **Hysteresis.** Quota none; shape enter 60 / exit 25.

### R10 — Write-spam burst

- **Trigger.** Per account, 600 s: `writes ≥ 8` **and** `distinct_body_hash_ratio ≤ 0.3` **and**
  `write_gap_cv ≤ 0.25` **and** (`account_age < 24 h` or `no read activity before the first write`).
- **Data.** Write events + body hashes + session start.
- **Kind.** Scored.
- **Precision.** Unmeasured.
- **Cheapest evasion.** Unique text per post (LLM paraphrase). Residual: burst rate, account age, and
  read-before-write — three cheap constraints an agent must satisfy simultaneously.
- **Hysteresis.** Enter 70 / exit 30.

### R11 — Cross-account duplicate content

- **Trigger.** The same `HMAC(content_pepper, normalize(body))` posted by ≥3 distinct accounts inside
  3600 s, **where that body has previously been human-confirmed violating on at least one account**.
- **Data.** Body hashes + human-confirmed violation records.
- **Kind.** Deterministic trigger, human-reviewed cluster action.
- **Precision.** The unscoped version of this rule is a trap on this specific product: identical text is
  _normal_ here — Quran verses, hadith, du'a, and translations are identical by design (`A2` §3.1 warns
  explicitly: "never Quran text, which is identical by design"). Restricting the rule to
  human-confirmed violating bodies is what makes it safe, and it is also what keeps it out of the
  project's no-SHA-over-Quran-data rule: the rule never hashes the corpus.
- **Cheapest evasion.** Reproduce the body from a different account. Residual: none; the cluster review is
  the control.
- **Hysteresis.** n/a.

### R12–R17 — Coherence (only what is computable server-side)

Design rules shared by all six: (i) every signal has state `{value, masked, unavailable}` — only positive,
self-contradictory evidence can flag, never absence or a vendor clamp (`A2` §2.2); (ii) checks are
directional; (iii) a versioned per-family support matrix plus a known-masked allowlist is required;
(iv) coherence is **excluded from ban thresholds entirely** (`A2` F6/F4) — hence the L2 cap; (v) release-day
damping (G05); (vi) bias instrumentation (G04).

- **R12 UA-CH impossible.** The strongest logical basis available: `navigator.userAgentData` exists only
  in Chromium, and Chromium always sends `Sec-CH-UA` on the request. So "payload says UA-CH present, the
  request carried no `Sec-CH-UA` header" is a positive contradiction, not an absence. FP requires an
  intermediary that strips the header while the page still reads the API — exclude proxy-rendered clients
  (Opera Mini: P7, 3.51% of Indian mobile traffic) and WebView, both of which are already classified as
  non-devices.
- **R13 Engine/header mismatch.** Page JS cannot change the `User-Agent` header, so a payload `engine`
  that contradicts the header family is a contradiction. FP class = Chromium forks that report a
  non-Chromium family string (Samsung Internet, Yandex, UC, Opera, CriOS, WebView, ChromeOS) — all
  allowlisted. `A2` F6's rule applies: check "contains Chromium", never brand-list equality.
- **R14 Platform/header mismatch.** The naive form of this check flags 90.62% of Indian mobile traffic
  (P6, StatCounter Sept 2026): Chrome's reduced UA says `Android 10; K` forever while UA-CH returns the
  real version. Directional only: flag a payload platform that the header platform _cannot_ be; never flag
  a version older than claimed; never flag `Linux x86_64` on ChromeOS or `MacIntel` + touch on an iPad in
  desktop mode. Capped at L1.
- **R15 Hardware/platform impossible.** `touch = 0` on a browser-claimed iOS/Android context; `apple` GPU
  family on a Windows/Android claim; hardware triple outside the platform's possible set. Explicitly
  excludes the Firefox FPP clamp signature (`hardwareConcurrency = 2`, `maxTouchPoints ∈ {0,1,5}`,
  `height − 48`) — treat the clamp as "Firefox FPP", which _lowers_ risk (`A2` §2.1).
- **R16 Language quintuple.** DataDome's five-source check, implemented with the header as the anchor:
  `Accept-Language` primary tag must be present in the payload's language set (strict variant: first).
  **L0 advisory only.** VPN, translation extensions, and Firefox RFP (which reports `en-US`) all produce
  legitimate disagreement (`A2` B9: "never a challenge trigger").
- **R17 Session field drift.** Time consistency: the same typed field at two points in one session. Only
  session-stable fields; excludes screen class, DPR, display mode (rotation, foldable, monitor change).
  Needs two beacons ≥60 s apart. FP class = browser/OS release days → G05 damping.

**Not computable in v1, stated rather than faked:** update-churn coherence (Li & Cao's "a device that
claims Chrome 131 for 18 months while never churning is not a device that has been running Chrome 131 for
18 months", `R2-fingerprint-science` §5.3-I) needs ≥3 months of history and a corpus of release dates.
Defer to v2. Do not approximate it with a version-age check, which `A2` F7 already shows produces a
release-day FP wave.

---

## 5. Ordering, interaction, supersession, double-counting

**Evaluation order (per subject, on the ticker):**

```
1. G06 path exemption          → device rules never see reader routes
2. G01 / G07 population guards → mark subject non-actionable if it fails either
3. deterministic rules         → R01(24h tier), R04a, R04b, R06, R09-quota, R11
4. scored rules                → R02, R03, R04c, R05, R07, R08, R09-shape, R10, R12–R17
5. family dedup                → keep max per evidence family (§2)
6. class algebra               → Σ min(40, best-in-family) per class
7. threshold + hysteresis      → ladder table §2
8. G02 action budget           → ≤1 case/key/7d, ≤1 challenge/session/5min
9. G03 shared-device step-down → one level down, never up
10. G05 damping                → already applied at threshold time
11. G04 bias breaker           → may suspend any rule to L0 globally
```

**Supersession.**

| When                             | Then                                                                                                                |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| R04a or R04b fires for a subject | R04c is suppressed for that subject; the case goes to the deterministic path (fixed action, no score accumulation)  |
| G01 or G07 marks a subject       | every device-punitive rule is suppressed for that key/unit; log only                                                |
| L7 (CSAM, court order, NEA)      | supersedes everything; statutory process owns the case                                                              |
| R11 opens a cluster case         | per-account actions for cluster members are held at L1 until the cluster is reviewed (≤20 accounts, `DESIGN-v1` B6) |
| G03 applies                      | the action steps down one level; the account-scoped ban is untouched                                                |
| A rule has been suspended by G04 | its observations continue at L0; no action until the bias alert clears                                              |

**How double-counting is prevented.** Three independent mechanisms, because one is not enough:

1. **Family dedup** — rules that read the same source cannot stack (table in §2). This is what stops
   R12–R17 (six rules, one payload) from being six independent facts; `R2-fingerprint-science` §5.1 is
   explicit that correlated surfaces add almost nothing.
2. **Class cap** — a class contributes at most 40 regardless of how many families fire. This is what
   stops an attacker who trips five distinct request-shape rules from reaching L4 on shape alone.
3. **Class precondition on the threshold** — L3+ needs ≥2 classes with ≥1 non-P. This is what stops the
   v0 failure mode where one attacker-controlled input was the marginal contributor to a ban.

Worked example of the v0 flaw vs the fix: attacker spoofs a coherent-but-false payload (all coherence
checks pass, GPU class claims a rare high-end device, locale mismatched). v0: this is 2–3 scored signals
that sum. v1: the payload is one class (P), the family dedup keeps the best coherence rule, the class cap
holds P at 40, and the threshold precondition means the attacker can reach L2 (a challenge) and no further,
regardless of how many payload-derived rules fire.

---

## 6. Shadow-mode plan

**Mechanism.** Every scored rule runs at L0 with `would_action` computed and recorded but not executed.
Observations are written by the existing 60 s slow-flush path (`rate_limit_store.rs` `SLOW_FLUSH_EVERY_N_TICKS`)
— never the request path.

```sql
-- append-only, no FKs, self-contained, expiry enforced by the sweeper
CREATE TABLE IF NOT EXISTS rule_observations (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    rule_id       TEXT    NOT NULL,
    rule_version  INTEGER NOT NULL,
    subject_kind  TEXT    NOT NULL,   -- account | device_key | ip_unit | api_key | session
    subject_hash  TEXT    NOT NULL,   -- HMAC, never raw
    observed_at   TEXT    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    window_start  TEXT    NOT NULL,
    window_end    TEXT    NOT NULL,
    score         INTEGER NOT NULL,
    class_bitmap  INTEGER NOT NULL,   -- H|S|T|P contributions
    would_action  TEXT    NOT NULL,   -- L0..L4
    guards        TEXT    NOT NULL DEFAULT '[]',
    evidence      TEXT    NOT NULL DEFAULT '{}',
    payload_ver   INTEGER NOT NULL DEFAULT 0,
    expires_at    TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_rule_obs_rule_time ON rule_observations (rule_id, observed_at);
CREATE INDEX IF NOT EXISTS idx_rule_obs_subject   ON rule_observations (subject_hash, observed_at);
CREATE INDEX IF NOT EXISTS idx_rule_obs_expires   ON rule_observations (expires_at);
```

Retention: 30 d for observations not promoted to a case; a promoted case moves to `device_evidence` /
`enforcement_cases` (`DESIGN-v1` B3) and inherits the case retention.

**What to measure, per rule, per day.**

| Metric             | Definition                                                                                                                                                     | Query shape                                                                           | Gate                                                                                |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `fire_rate`        | observations per day, per rule_version                                                                                                                         | `count(*) group by rule_id, date(observed_at)`                                        | sanity: not >1% of eligible subjects                                                |
| `precision_proxy`  | confirmed / observed, where confirmed = the subject has a human-confirmed violation, an upheld ban, or a rejected appeal within 30 d **after** the observation | `rule_observations ⋈ enforcement_cases` on `subject_hash`, `observed_at` within ±30 d | ≥0.90 for L2 actions; ≥0.95 for L4 proposals (P13: ≥95% permanent, ≥90% suspension) |
| `benign_share`     | share of observations whose subject was never actioned by any other rule **and** had ≥3 distinct active days in the 30 d after                                 | `rule_observations` left join activity rollup                                         | ≤0.05                                                                               |
| `bias_ratio`       | fire rate per browser family / country / device class / display mode / ASN type ÷ Chrome baseline                                                              | same query, grouped by the attribute                                                  | no group >3× baseline (G04's gate)                                                  |
| `replay_stability` | re-run the last 30 d of logs with the current rule_version weekly; Δ fire rate                                                                                 | offline harness over stored observations                                              | ≤±20%                                                                               |
| `appeal_overturn`  | for anything promoted to a case and actioned: reversed ÷ appeals                                                                                               | `appeals ⋈ enforcement_cases`                                                         | <5% (yellow at 10%, P13)                                                            |

**Window.** Minimum 30 continuous days **or** ≥200 observations per rule, whichever is later. For rare
rules (R04c, R11) the sample gate is 90 days or ≥50 observations. A rule that cannot reach its sample
gate in 180 days is either dead code or mis-specified — retire it rather than enabling it blind.

**Gate to enable a scored rule (all required, checked in code, not by an operator's judgement):**

1. sample gate met;
2. `precision_proxy` ≥ target for the action level;
3. `benign_share` ≤ 0.05;
4. `bias_ratio` ≤ 3 for every group;
5. `replay_stability` within ±20%;
6. the rule's cheapest evasion and residual are written down (the §7 table is that record);
7. a golden-vector unit test exists (`A2` F12: happy-dom has no canvas/WebGL/UA-CH, so coherence must be
   pure functions over golden vectors);
8. a kill switch exists: a per-rule env flag **and** a global `DEVICE_RULES_ENABLED=false` that reverts
   every rule to L0 without a deploy.

Until all eight pass, the rule's `would_action` is discarded and only `rule_observations` is written.

**The measurement caveat, stated because it is load-bearing.** There is no ground truth for "this was a
false positive" on a subject that was never actioned — no one appeals an L0 observation. `precision_proxy`
and `benign_share` are _proxies_ built from post-hoc adverse signals and activity continuity. They are
biased toward under-counting FPs on subjects that look active and over-counting on subjects that go quiet
(the storage-clearing user, the seasonal reader). Log them as proxies, never as precision; and hold the
targets above the published platform benchmarks precisely because the proxy is optimistic.

---

## 7. Red team: attacker counter + residual, per rule

| ID      | Attacker's counter (one sentence)                                                                                                                                                  | Residual detection                                                                                                                                                                                                                             |
| ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R01     | Fail one attempt on each of 1000 accounts instead of five on one.                                                                                                                  | R02's IP-unit stuffing shape; and the accounts being attacked still generate downstream signals the attacker does not control.                                                                                                                 |
| R02     | Rotate residential exits, one attempt per credential per exit, jitter the delays.                                                                                                  | R01 per-credential counters are unaffected by rotation; the attacker now needs a proxy per attempt, which is the cost target (P14: ~$8.85/account is the bar to beat).                                                                         |
| R03     | Not applicable — evidence-only; an attacker gains nothing by tripping it.                                                                                                          | The combination requirement (a Class-H/S abuse rule must also fire) means tripping it alone does nothing.                                                                                                                                      |
| R04a    | Buy a fresh email and a fresh phone number per identity.                                                                                                                           | R04b content and R04c device; and the cost of fresh identifiers is the lever, not the identifier itself.                                                                                                                                       |
| R04b    | Rewrite the violating body each time (LLM paraphrase).                                                                                                                             | Nothing from this rule. Accept: R04b catches the naive re-poster, and the honest accounting is that a determined evader is uncatchable (`R2-ban-operations` §3.3).                                                                             |
| R04c    | Run each identity in a fresh anti-detect profile or use a different phone.                                                                                                         | R04a/R04b only. This is why the linkage family is credential- and content-anchored, not device-anchored.                                                                                                                                       |
| R05     | One account per residential exit, unique emails, human-speed gaps.                                                                                                                 | R04a linkage and content rules; registration friction stops being the control and identifier cost becomes it.                                                                                                                                  |
| R06     | Mint a new API key.                                                                                                                                                                | R05 creation burst + the fact that key minting is account-scoped and rate-limited.                                                                                                                                                             |
| R07     | Add jitter, fetch a few assets, split the traversal across many exits, and revalidate with ETags.                                                                                  | Quota (R06) and the published bulk path: the goal is to make the legitimate route faster and cheaper than the scrape. Detection is not the control here.                                                                                       |
| R08     | Randomize the ID order and accept the 404s.                                                                                                                                        | R07's corpus-coverage term (coverage is order-independent).                                                                                                                                                                                    |
| R09     | Distribute queries across IP units and keys.                                                                                                                                       | R06 per-key quota.                                                                                                                                                                                                                             |
| R10     | Post unique text per write (LLM paraphrase) and read a few pages first.                                                                                                            | Burst rate remains (8 writes/600 s is above human writing cadence); three simultaneous constraints raise the cost.                                                                                                                             |
| R11     | Reproduce a confirmed-violating body from a different account.                                                                                                                     | None. The cluster review is the control; the rule exists to make the _cluster_ visible, not to catch the individual.                                                                                                                           |
| R12     | Omit the payload field (`p.ua_ch = unknown`).                                                                                                                                      | R13 engine mismatch and R17 drift — the attacker must now be consistent across fields, which is the whole point (`R2-evasion` §3.2 rank 1: space consistency requires a per-version knowledge base).                                           |
| R13     | Claim a different engine than the one running (e.g. patched Chromium claiming Firefox).                                                                                            | Engine-dialect behaviour tests (`R2-evasion` §3.1) — **not built in v1**, because they require client-side probes whose results are page-asserted and therefore forbidden by §1.3(a). Honest residual: none in v1 for an engine-level spoofer. |
| R14     | Keep the claimed OS consistent with the header.                                                                                                                                    | None server-side. `R2-fingerprint-science` §5.1: a consistent spoof is out of reach for coherence checks; accept and rely on the H/S/T classes.                                                                                                |
| R15     | Report touch points consistent with the claimed platform.                                                                                                                          | None from this rule; same acceptance as R14.                                                                                                                                                                                                   |
| R16     | Match the locale to the proxy's geography.                                                                                                                                         | None, and the rule is L0 by design — language disagreement is not evidence of abuse.                                                                                                                                                           |
| R17     | Freeze the payload for the duration of a session.                                                                                                                                  | Cross-session inconsistency (update-churn) — deferred to v2, needs months of history.                                                                                                                                                          |
| G01–G07 | Guards only reduce action; the "counter" is to look like a school, a shared device, or a bias victim — which is precisely what a real school/shared device/bias victim looks like. | The residual is that a sophisticated attacker can hide behind a legitimate-looking population. That is the accepted cost of not banning populations (`A2` F1–F3).                                                                              |

**Summary of the red team.** Six rules have a _real_ residual (R04b, R04c, R13, R14, R15, R17), and in
every one of those cases the attacker's counter is "be consistent" or "rewrite the content" — which is
cheap. The honest conclusion: the rules that survive an adaptive attacker are the deterministic,
non-forgeable, server-state ones (R01, R04a, R04b, R06, R11) plus the shape rules as cost multipliers
(R07–R09). The coherence rules buy at best the 45–48% evasion reduction measured against commercial vendors (`R2-evasion` §3, P9) and
nothing more; they must never be the sole basis of a restriction, which is why every one of them is capped
at L2.

---

## 8. v1 ENABLED subset (smallest defensible set without measurement)

Enforced from day one — deterministic, non-forgeable, action bounded and reversible:

| ID                        | Action ceiling                                  | Why it is defensible without measurement                                                                                                                          |
| ------------------------- | ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **R01**                   | L1 progressive delay; L4 queue at the 24 h tier | A counter; the action is delay, not lock; a mistyped password costs 8 s                                                                                           |
| **R04a**                  | L4 queue-only + human, ≤72 h hard expiry        | The design's own deterministic class (`DESIGN-v1` B5: "prior ban + credential reuse"); no restriction is imposed before a human sees it                           |
| **R04b**                  | L4 queue + human; CSAM → L7                     | Same; statutory path for CSAM is not optional                                                                                                                     |
| **R06**                   | L1 → 429; L2 on a key after 3 windows           | Quota, published; `R2-evasion` §10 ranks it #1 impact/effort. Per-credential counters already exist (B2 §0) — R06 makes them failure-only and adds per-key quotas |
| **R09 (quota tier only)** | L1                                              | Existing search-isolation intent (`quran_v1/mod.rs:56`)                                                                                                           |
| **G01–G07**               | suppress / step-down only                       | Guards can only reduce action, never increase it                                                                                                                  |

Everything else — R02, R03, R04c, R05, R07, R08, R09-shape, R10, R11, R12–R17 — runs in **L0 shadow mode**
under §6 and is enabled only by passing the gate.

**What this subset does not do, stated plainly.** It does not detect a determined evader; it does not
fingerprint anyone; it does not challenge a reader; it does not ban on a device signal. It catches naive
ban evasion (identifier reuse, re-posted content), credential stuffing against real accounts (per-credential
counters), and bulk extraction (quota). That is the defensible floor. The scored rules are the _next_
release, not the launch.

The v1 property worth naming as a design invariant: **the enabled subset contains zero rules that consume
a page-supplied payload and zero rules that can ban.** The first scored rule to be enabled will be the
first time a page-supplied value affects a user's experience on this system, and that should be a
deliberate, measured decision — not a launch default.

---

## 9. Prohibited rules — proposed and rejected

| Rejected rule                                                        | Prohibition                 | Why                                                                                                                                                  |
| -------------------------------------------------------------------- | --------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `spoof_webdriver`, `spoof_cdp`, `spoof_native_tamper` as rule inputs | (a) client-asserted boolean | One line of JS sets them (`A1` F1). Stored as evidence only.                                                                                         |
| `coherence_*` client verdicts                                        | (a)                         | The v0 flaw; the server recomputes from §1.1 instead.                                                                                                |
| Software-renderer / `SwiftShader` / VM tell                          | (b) VM presence             | FP population is VMs and CI, and "VM" is not abuse. Dropped from `R2-evasion` §9 row 3.1 despite being useful.                                       |
| CDP `Runtime.enable` / DevTools trap                                 | (b) DevTools presence       | DevTools open is ~6% of desktop sessions (P10); the tell also breaks quarterly (row 2). Dropped.                                                     |
| Tor / VPN / incognito / privacy-hardened OS as a risk input          | (b)                         | `A2` F8: a challenge is a hard block behind national filters and in Tor; `A2` §5.2 item 7: matching a masking signature _lowers_ risk.               |
| IP reputation / ASN block / residential-range block                  | (c) + (b)                   | 4 of 5 residential-proxy requests are legitimate household traffic (P8); blocking would hit the app's own mobile/CGNAT audience (`R2-evasion` §2.4). |
| `device_key` match alone → ban                                       | (c)                         | Cohort key (P3: low mobile uniqueness even with richer signals); a device ban is a cohort ban (`A1` F3, `A2` F1/F2/F3).                              |
| New-device event → step-up                                           | (c)                         | 32% of instances clear cookies in 8 months (P4); iOS home-screen and in-app contexts are separate storage (P5).                                      |
| Low interval entropy alone → block                                   | (c)                         | A single distribution is one signal; R07 requires four terms and caps at L1/L2.                                                                      |
| Disposable email domain alone → block                                | (c)                         | Domain lists have real FPs (plus-addressing, small ISPs, universities); it is one term inside R05.                                                   |
| Token/payload mismatch → risk                                        | —                           | Per `A2` F5: mismatch means "unknown device", never risk. Explicitly not built.                                                                      |
| Keystroke / mouse / scroll / touch dynamics                          | (d)                         | Art. 9 exposure, 80% F1 (`R2-evasion` §5.2, row 14).                                                                                                 |
| Client-side WASM/JS timing signature                                 | (a) + unstable              | Measured 2–4 bits and unstable (`DESIGN-v1` A table); a client-asserted value.                                                                       |
| Unscoped cross-account duplicate-content detection                   | product-specific            | Identical text is normal on a Quran app (Quran/hadith/du'a are identical by design). Restricted to human-confirmed violating bodies (R11).           |

---

## 10. Sourced priors (the numbers behind the FP/TP reasoning)

| #   | Number                                                                                                                                          | Source                                                                          |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| P1  | 45.52% of fingerprints change after one day; average trackable 51.8 days; 26% trackable >100 days (larger attribute set than this design's key) | FP-Stalker, Vastel et al., IEEE S&P 2018, via `R2-fingerprint-science` §2.3     |
| P2  | FP-Stalker rule-based F1 drops 86.1% → 75.9% as the DB grows 100K → 1M fingerprints; learning variant cannot scale past ~300K                   | Li & Cao, IMC 2020, via `R2-fingerprint-science` §2.2                           |
| P3  | 33.6% overall unique; 18.5% mobile unique (one French commercial site, with canvas/WebGL/audio)                                                 | Gómez-Boix et al. 2018 (Hiding in the Crowd), via `R2-fingerprint-science` §2.1 |
| P4  | 32% of browser instances cleared cookies in 8 months                                                                                            | Li & Cao, IMC 2020, via `R2-fingerprint-science` §2.3                           |
| P5  | ~31% of mobile web sessions are in-app-browser sessions (secondary, flagged)                                                                    | MobiLoud 2026, via `A2` F11                                                     |
| P6  | Naive UA↔UA-CH check flags 90.62% of Indian mobile traffic                                                                                      | StatCounter Sept 2026, via `A2` F6                                              |
| P7  | Opera is 3.51% of Indian mobile traffic                                                                                                         | StatCounter Sept 2026, via `A2` F10                                             |
| P8  | 4 of 5 requests from residential-proxy IPs are legitimate household traffic                                                                     | Cloudflare, via `R2-evasion` §2.2                                               |
| P9  | 52.93% avg evasion vs DataDome, 44.56% vs BotD; consistency rules cut evasion 48.11% / 44.95%                                                   | FP-Inconsistent, arXiv 2406.07647, ACM IMC'25, via `R2-evasion` §3              |
| P10 | DevTools open ≈6% of desktop sessions                                                                                                           | FingerprintJS, via `R2-fingerprint-science` §7.2                                |
| P11 | ~20K users of Canvas-Blocker-class extensions                                                                                                   | Vastel 2024, via `A2` F6                                                        |
| P12 | No platform publishes account-ban precision; no vendor publishes a device-fingerprint ban FP rate                                               | `R2-ban-operations` §4                                                          |
| P13 | Normative targets (no platform publishes achieved rates): ban-proposal precision ≥95% permanent, ≥90% suspension; appeal overturn <5%           | `R2-ban-operations` §9                                                          |
| P14 | ~$8.85 per account is the unit cost of the abuse being defended against                                                                         | Meta Q1 2025, via `R2-evasion` §7.1                                             |

---

## 11. Findings

### F1 — The v0 flaw cannot be fixed with weights; it needs a class algebra with a class precondition on the threshold

**Severity: blocker. NEW.**
Evidence: the brief's own statement of the v0 flaw; `A1` F1 (client-authored verdicts), F2 (attacker-controlled
cardinality), F3 (device as ban unit). A weight cap still lets one input be the marginal contributor, and
`A1` F3 notes that "corroboration" in v0 is a policy sentence with no mechanism behind it.
Fix: `score = Σ min(40, best_rule_per_class)` over H/S/T/P, with L3+ requiring ≥2 classes and ≥1 non-P. This
makes "one signal cannot escalate" arithmetic. Ban 80 / restore 40 stay the outer pair; the ladder nests
inside; enforcement takes effect after a fixed, documented grace period (B9 F10).

### F2 — Per-credential failure counters must throttle, never lock, or the counter becomes an account-lockout DoS

**Severity: high. NEW.**
Evidence: `R2-evasion` §10 item 1 recommends per-credential counters as the #1 countermeasure. Implemented as
a lock, they let an attacker lock any account by failing logins on it — a targeted DoS on a specific reader, from any IP, for free. The existing repo
escalation path is IP-only and default-OFF (`escalation.rs`), so this would be new attack surface.
Fix: R01 caps at progressive delay (1–8 s) plus a session-scoped challenge; never an account lock, never a
credential suspension. The 24 h tier opens an L4 queue case _for the account being attacked_, which is a
protective action, not a punitive one.

### F3 — Page JS cannot set HTTP headers; that asymmetry is the only non-forgeable coherence anchor and must be the admissibility contract

**Severity: high. NEW.**
Evidence: `A1` F1 (the collector's booleans are attacker-authored), `R2-evasion` §3.1 ("UA vs Client Hints:
highest yield, zero client JS"), §3.2 rank 1 (space consistency is the hardest lie to maintain).
Fix: §1.4 — an input is admissible iff the server computes the predicate and it is H (header/edge-signed),
S (server state), T (request shape), or a P value cross-checked against H/S/T. P alone caps at L2. This is
what makes R12/R13 real checks rather than v0's client-authored booleans, and it is why R12 (payload claims
`navigator.userAgentData` while no `Sec-CH-UA` header arrived) is a positive contradiction rather than an
absence check.

### F4 — The UA↔UA-CH coherence check must be directional-only or it flags ~90.6% of Indian mobile traffic

**Severity: high. NEW.**
Evidence: `A2` F6 — Chrome's reduced UA reports `Android 10; K` for every Android Chrome user forever while
UA-CH returns the real version; StatCounter Sept 2026 puts the naive-flag rate at 90.62% of Indian mobile
traffic. `navigator.platform` is likewise a fixed per-OS string (`Linux x86_64` on both ChromeOS and Linux).
Fix: R14 direction only ("payload platform that the header platform cannot be"), plus a per-family support
matrix and a known-masked allowlist (Brave, Safari AFP, Firefox FPP, Tor, Lockdown, iPad desktop mode,
frozen Android token). Coherence is excluded from ban thresholds entirely (`A2` §2.2 item 4) — which the L2
cap enforces in code.

### F5 — Unscoped cross-account duplicate-content detection collides with the product's own canonical text

**Severity: high. NEW.**
Evidence: `A2` §3.1 — "Identical **violating** content … never Quran text, which is identical by design";
`R2-ban-operations` §2. A Quran app's normal traffic contains large volumes of byte-identical text (verses,
hadith, du'a, shared translations). Any "same body, ≥3 accounts" rule without a scope gate fires
continuously on legitimate sharing.
Fix: R11 is restricted to bodies that have already been **human-confirmed violating** on at least one
account. This also keeps the rule outside the project's no-SHA-over-Quran-data hard rule, because it never
hashes the corpus — it hashes confirmed user content only.

### F6 — One-account-many-devices is a false-positive generator and must be evidence-only, not a "fan-out" feature

**Severity: high. NEW.**
Evidence: `A2` F5 (32% of browser instances cleared cookies in 8 months — P4 — and "one account, many
device records is normal; no fan-out feature"); `A2` F11 (~31% of mobile sessions are in-app-browser
sessions with separate storage; iOS home-screen apps are storage-isolated from Safari — P5).
Fix: R03 is L0 by default and reaches L4 queue only when a Class-H/S abuse rule also fired on the same
account in the same window. Any "device fan-out score" is prohibited (§9).

### F7 — Device rules cannot see the reader path, so anonymous scraping is a request-shape problem, not a fingerprint problem

**Severity: high. NEW.**
Evidence: `A1` F5 (the collector transmits only on `AuthClient` calls; a pure reader never sends a payload);
`DESIGN-v1` B2 ("Never on reader routes … Machine-guarded"); `DESIGN-v1` B6 (no entity-graph clustering in
v1). Therefore R12–R17 have zero coverage of the anonymous reader path by construction.
Fix: R07/R08/R09 are payload-free and use only T-class data (route template, gaps, asset ratio, coverage).
`DESIGN-v1` Q3 must be answered with rate-shaping plus a published bulk path, not with a reader fingerprint.
State the coverage boundary in the design: the device axis covers authenticated writes only.

### F8 — A device/cohort rule needs a population non-actionability guard or one attacker becomes a population-wide action

**Severity: high. NEW.**
Evidence: `A1` F3 (device_key is a cohort key); `A2` F1/F2/F3 (cohort bans; attacker-chosen key; the
"frame the kiosk"/"frame a stranger by cohort" attacks); `A2` §3.1 (the inheritance lemma: a co-user
inherits every device/network/locale/behaviour signal).
Fix: G01 (≥4 accounts or ≥6 tokens on a key → non-actionable), G02 (≤1 case per key per 7 d), G03
(shared-device/org step-down), G07 (`ip_legit_share`). Guards can only reduce action; they are always on and
need no shadow period.

### F9 — Shadow mode needs a defined false-positive proxy, because no ground truth exists for a non-action

**Severity: medium. NEW.**
Evidence: `R2-ban-operations` §4 — no platform publishes account-ban precision and no vendor publishes a
device-fingerprint ban FP rate (P12); §9 sets precision targets but they are measured on _proposals that
were actioned_. An L0 observation is never appealed, so the standard metric is blind.
Fix: two proxies with explicit gates — `precision_proxy` (post-hoc adverse signal within 30 d) and
`benign_share` (no adverse signal **and** ≥3 active days in the following 30 d). Document both as optimistic
proxies, not precision; hold L2 at ≥0.90 and L4 at ≥0.95, above the published benchmarks, precisely because
the proxy is optimistic.

### F10 — The linkage rule's weakest link is retention, not detection: the ban-evasion window must be ≤ the retention of the linkage tables

**Severity: medium. NEW.**
Evidence: `DESIGN-v1` B3 retention (keyed device record 30 d, ban row 12 mo, `device_sessions` denormalized
so account deletion cannot destroy linkage); A10's `expires_at` sweeper deletes idle+unlinked records.
R04c's 30 d window and R04a's 12 mo window are therefore exactly at the retention boundary — a sweeper run,
a batch lag, or a backup restore (`A1` F8: pepper restore makes every key unmatchable) silently degrades the
rule to zero without any error.
Fix: assert `retention(device_sessions) ≥ linkage_window + 30 d` in a migration contract test (the repo
already uses the m000004 pattern for exact-column assertions), emit a metric on sweeper deletions of records
that are linked to a ban, and add a boot-time check that the active-ban map is hydrated before any linkage
query runs.

---

## 12. Bottom line

- The rule set is real, but only the deterministic server-state rules are defensible at launch. Everything
  behavioural or payload-derived must earn its way out of shadow mode with a measured proxy, per rule.
- The v0 flaw is fixed structurally, not by tuning: class caps plus a class precondition on the threshold.
  A payload-only attacker reaches a challenge and stops.
- The four prohibitions cost three genuinely useful detectors (software-renderer/VM, CDP/DevTools, IP
  reputation). That is the correct trade for this audience; the drops are recorded in §9 rather than
  quietly re-added.
- The honest residual: an adaptive attacker who rewrites content and stays consistent across fields is not
  caught by v1. The response is cost (quota, identifier cost, the published bulk path), not a stronger
  fingerprint — which is what `R2-ban-operations` §3.3 already concluded.
