# B10 — Falsification audit of the round-3 reports and DESIGN-v0/v1

Auditor: research falsification pass. Date: 2026-10-12.
Method: read every report in this directory; re-check load-bearing claims against the working tree and
against each other. No new research. Every citation below is to a file in this directory or to a
`file:line` in `/Users/hmziq/os/easyquran`.

**Scope.** This pass audited `DESIGN-v0.md`, `DESIGN-v1.md`, the seven `R2-*.md` reports and `A1..A10`.
The round-4 `B1..B9` reports ran in parallel and are outside its scope; `DESIGN-v1.md` itself cites only
"A1..A10".

**Repo facts re-verified for this audit** (so the contradictions below have a ground truth):

- `max_connections(1)` — `rust/backend/api/src/db/sea_connect.rs:35-44`. Confirmed.
- sea-orm-migration 1.1.20 wraps **only Postgres** in a transaction; Sqlite/MySql run bare
  (`~/.cargo/registry/.../sea-orm-migration-1.1.20/src/migrator.rs:261-272`). Confirmed by reading the
  registry source. The `seaql_migrations` row is inserted after `up()` returns (`:386-397`). Confirmed.
- `BanUnit` is a newtype over `IpNet`, v4→/32, v6→/64 (`rust/backend/api/src/modules/admin_bans_v1/dto.rs:17-35`). Confirmed.
- `device_v1` exists with `MAX_DEVICES_PER_USER = 20` and an ownership-transfer test
  (`rust/backend/api/src/modules/device_v1/controller.rs:17,167`). Confirmed.
- happy-dom 20.11.1 (`web/package.json:70`), `environment: "happy-dom"` (`web/vitest.config.ts:12`).
  Probed directly: `navigator.webdriver === true`, `WebAssembly === undefined`, `navigator.userAgentData === undefined`,
  `navigator.gpu === undefined`, `navigator.storage === undefined`, `canvas.getContext("2d") === null`,
  `navigator.plugins.length === 0`, `hardwareConcurrency = 8`, `platform = "X11; Darwin arm64"`.
  A6's probe is **accurate**.
- CSP: `hooks.server.ts:82-96` has gstatic + googletagmanager + challenges.cloudflare.com + frame-src;
  `server.ts:210-246` has gstatic/gtm, **no** challenges.cloudflare.com, **no** frame-src. A4's CSP diff is
  **accurate**. `static/_headers` carries all three.
- Disk cache: 7-day TTL (`web/src/lib/server/quran-disk-cache.ts:12,40`); `cacheable` predicate verbatim at
  `hooks.server.ts:346-351`; `requestHasCookie = !!headers.get("cookie")`. A4 accurate.
- `AuthSessionState` derives `Serialize, Deserialize` and has **no** `#[serde(...)]` field attributes
  (`rust/backend/api/crates/rux-auth/src/session/state.rs:5-26`). A3's "no serde attributes" is imprecise
  wording; its substantive finding (adding a field without `#[serde(default)]` breaks live sessions) stands.
- `MAX_GRANTS = 10_000`, process-local (`src/services/fresh_auth.rs:16,100,176`). Confirmed.
- `user_sessions.device` = raw `user-agent` on login (`modules/auth_v1/controller.rs:182-187`). Confirmed.
- `ROLE_MODERATOR` (`middlewares/auth_guard.rs:20`) gates no route, but inline `User::is_moderator()` checks
  already grant delete-any-media, read-any-email and paywall bypass (`post_v1/controller.rs`,
  `media_v1/controller.rs:540`; F3 §3.5).
- `NewUserBan` exists only in `db/sea_models/user_ban/` — no production caller. Confirmed.
- `audit_logs` — model only, zero writers. Confirmed.
- `post_v1` and `post_comment_v1` are mounted **outside** the `WEB_AUTH_ENABLED` gate (`router.rs:117,122`).
  Confirmed. A5's DSA finding is evidence-backed.
- CI: `53d3d421` deleted "Web security gates", "Rust security gates", `bun audit`, `cargo-audit` from
  `.github/workflows/images.yml`. Confirmed by `git show`.
- `sqlx-sqlite-0.8.6` inserts `synchronous` with value `None` (SQLite default = FULL)
  (`~/.cargo/registry/.../options/mod.rs:187-189`). Confirmed. A7's fsync premise is correct.
- Firebase Analytics is live and initialised (`web/src/lib/firebase/analytics.ts:18-20`, consent defaults
  `analytics: true` — `web/src/lib/stores/consent.svelte.ts:16`). Load-bearing for §1.10.
- `verify_ed25519` exists (`rust/backend/api/crates/rux-webhook-crypto/src/lib.rs:101`), `MAX_SKEW_SECS = 300` (`:6`).
- `static_csrf` key is `OnceLock<Vec<u8>>`, HKDF from `COOKIE_KEY`, info `ruxlog-csrf-v1`, not zeroized
  (`middlewares/static_csrf.rs:15-47`). A9 accurate.

---

## 1. Contradictions between reports

Each entry: a claim that conflicts with the code or with another report, and what the code or the adopted
design supports.

### 1.1 Migrations are not transactional on SQLite

- **Claim to avoid:** "sea-orm wraps each migration in a transaction", and its corollary that
  `PRAGMA foreign_keys` is a no-op inside a migration.
- **Fact** (A10 §0 + F8, read from the registry source): sea-orm-migration 1.1.20 opens a transaction
  only for Postgres (`migrator.rs:261-272`: `MySql | Sqlite => f(&manager)`), and the version row is
  inserted **after** `up()` returns (`:386-397`), so a crash mid-migration leaves DDL applied but
  unrecorded and `up()` re-runs on the next boot. `configure_sqlite` sets `PRAGMA foreign_keys=ON` before
  `Migrator::up` (`sea_connect.rs:46-56,87-93`), so FK actions are live during DDL.
- Both facts lead to the same rules: never rebuild a table; idempotent `IF NOT EXISTS` DDL, re-runnable,
  optional explicit `conn.transaction` (A3 §8, A10).

### 1.2 `device_sessions` must not cascade from `user_sessions`

- **Claim to avoid:** `device_sessions.user_session_id … REFERENCES user_sessions(id) ON DELETE CASCADE`
  (and `device_record_id … ON DELETE CASCADE`).
- **Fact** (A10 F1, A3 F10): any such FK erases the device↔account linkage exactly when the offender's
  account is deleted (`user_sessions.user_id … ON DELETE CASCADE`, `m000001:55-56`; hard delete at
  `user/actions.rs:486-497`; boot reconcile `auth.rs:401-406`). Fix: mirror `auth_session_binding` — no
  FK. `DESIGN-v1:43` adopts this shape ("no FKs to users/user_sessions; denormalized user_id").

### 1.3 Device token storage: cookie or localStorage — A1 vs A9

- **A1 F4** (high): "Move the token to a server-set `HttpOnly; Secure; SameSite=Lax; Path=/api` cookie
  (path-scoped ⇒ `/t/**` cache untouched)".
- **A9 §3.4(5)**: "One storage location … **Pick localStorage**".
- **DESIGN-v1:16,78**: adopts "HttpOnly cookie, Path=/api (OPEN Q1)".
- **Ground truth**: `requestHasCookie = !!event.request.headers.get("cookie")` (`hooks.server.ts:427`). A
  `Path=/api` cookie is **not sent on a `/t/**` document request**, so the disk cache predicate is
  untouched — the rule is "no cookie on reader paths", not "no cookie" (A4 §1, A6 G2). A9's
  localStorage choice is contradicted by A1's XSS analysis (the CSP trusts three third-party script
  origins) and by DESIGN-v1.

### 1.4 Edge attestation: fail-open or fail-closed — A3 vs A1 vs A9 (three incompatible answers)

- **A3 F12**: the new middleware must "**fail closed** in production when the header is present but invalid".
- **A9 F11**: "Sig mismatch → **drop the attestation, keep the request.** Alert with the kid. **Never 401/403 the request**"; enforcement only behind a default-off flag.
- **A1 F6**: "make unknown-kid/clock-skew **fail open for read-only GETs**" (implying fail-closed for writes).
- **DESIGN-v1:19** adopts A9's observe-only default.
- No two of these can be implemented at once. A3's rule turns one Worker deploy or one clock-skew event into
  a site-wide 4xx (A1's own availability warning); A1's read/write split is a third policy with no spec.

### 1.5 Edge signing algorithm — A1 vs A9 (**A1's HMAC fix superseded**)

- **A1 F6 / §(c)**: keeps the design's HMAC construction; fix = "sign `CF-Connecting-IP` + path-with-query + a body digest … keep the edge key in a separate secret"; verdict "worth building".
- **A9 F10 (high)**: "**Use Ed25519 for edge attestation, not a shared HMAC key** … a shared HMAC key … gives the origin the ability to forge edge claims"; `verify_ed25519` already exists.
- **DESIGN-v1:18** adopts Ed25519.
- A1's stated security property ("stops a request that did not transit the Worker") fails under its own
  threat model, because the origin holds the same key and the origin is not the attacker's only target —
  an in-network process with the `.env` key forges everything (A1's own F6 gap 3). A9's construction is the
  only one that satisfies A1's requirement. A1 and A9 therefore give contradictory recommendations for the
  same control.

### 1.6 Edge layer verdict: build / optional / defer — A1 vs A7 vs A9

- **A1 §(c)**: "worth building, but it is _transport integrity_, not _device attestation_".
- **A7 F7 (major)**: "A Cloudflare Worker on every request is **unjustified in v0** … make Layer 4 optional; ship origin-only".
- **A9 §8**: "**Defer** edge attestation entirely. There is no Worker, no wrangler config".
- **DESIGN-v1:19** = A9/A7's shape. A1's positive verdict is unsupported by the cost analysis it did not do
  (A7 did it: +1–5 ms p50, per-request billing, and "without Bot Management the edge contributes
  geography/ASN/colo — all derivable from the IP").

### 1.7 `UNIQUE(device_key)` is wrong

- **Claim to avoid:** `CREATE UNIQUE INDEX … ON device_records (device_key)` as the upsert conflict target.
- **Fact** (A2 F1, A9 §2.4, DESIGN-v1:11): `device_key` is a cohort key — 1:1 is false in both directions.
  It stays a non-unique, indexed cohort attribute; `token_hash` (UNIQUE) is the identity and the conflict
  target (A10 §1.2).

### 1.8 Two "normative" device-migration schemas that cannot both be written — A9 vs A10

Beyond the UNIQUE question, A9 and A10 each present a complete DDL as _the_ device migration, and they differ on:
`id` (TEXT ULID vs INTEGER AUTOINCREMENT), `token_hash` (BLOB vs TEXT), `trust_state` default (`'new'` vs
`'observe'`), retention column (`expires_at` absent in both vs DESIGN-v1's requirement), and the presence
of a `device_bans` table (A10 has one; A9 has none and defers device bans entirely). DESIGN-v1:42-46
requires `expires_at NOT NULL` and `device_bans`; **neither A9's nor A10's DDL has `expires_at`**, which
A5 F2 (high) and DESIGN-v1:23 both make a launch requirement. Three reports assert a schema; none matches
the adopted one.

### 1.9 Anonymous visitors mint device records — DESIGN-v1 vs A4 (internal contradiction)

- **DESIGN-v1:72**: "no fingerprint for logged-out readers".
- **A4 §7(a)**: the payload attaches inside `AuthClient.#request`, and
  `fetchCsrf()` is `#request("POST", "/csrf/v1/generate")` (`auth-client.ts:231-238`) fired by
  `unsafeRequest()` for anonymous visitors — "every anonymous visitor's first unsafe action would mint a
  `device_records` row". A6 §1(b) reinforces it: "The token must ride the existing `/api/**` round trip …
  never a side fetch."
- The adopted design states a policy its own attachment point violates. The fix is A4 §7's path allowlist
  ("Never on `/csrf/v1/generate`, never on `/user/v1/get`"), which DESIGN-v1 does not adopt.

### 1.10 Removing gstatic/googletagmanager from the CSP breaks shipped Firebase Analytics

- **Claim to avoid:** "remove `gstatic`/`googletagmanager` from `script-src` — nothing loads them
  (`grep googletagmanager web/src` → only the CSP line)".
- **Fact:** `web/src/lib/firebase/analytics.ts` initialises Firebase Analytics (`getAnalytics`, `logEvent`)
  and consent defaults `analytics: true` (`consent.svelte.ts:16`). GA4 loads `gtag/js` from
  `googletagmanager.com`/`gstatic.com` at runtime — the URLs are inside the Firebase bundle, so a source
  grep cannot find them. The CSP entries exist **because** the app loads them; removing them breaks
  Analytics on every page.
- Firebase Analytics is itself a third-party-origin script that could read a script-readable token, so the
  fix for token exposure is HttpOnly (A1 F4), not deleting the CSP entries.

### 1.11 Nothing strips client-supplied edge headers

- **Claim to avoid:** "origin strips client-supplied headers — good", credited as an existing control.
- **Fact** (A3 F12, A9 F7): nothing strips client-supplied `X-Eq-Edge-*`; Traefik's only api-router
  middleware is `stripprefix /api`; `HeaderMap::get` is first-value-wins; `rg X-Eq-Edge` → 0 hits
  (re-verified). DESIGN-v0:54 says the origin _should_ strip; no code does.

### 1.12 The repo already violates Layer 0

- **Fact** (A8 F6, A10 F10): raw UA and IP are already stored durably — `user_sessions.device` = raw UA,
  `user_sessions.ip_address` = client IP, written on every login (`auth_v1/controller.rs:182-187`), with no
  retention policy. Layer 0's refusal stays aspirational until that is fixed (A5 §1 L0).

### 1.13 `#request` must read a synchronous payload

- **Claim to avoid:** an awaited collector inside `#request`
  (`const payload = await this.device.payload().catch(() => null);`).
- **Fact** (A4 §3/§7, normative): `#request` reads a **synchronous, already-collected** payload — no
  `await`, no import inside `#request`; an awaited collector either extends the 12 s deadline or trips it
  (`auth-client.ts:192,196`) and breaks the login `ready` gate. A6 §2 sketches the synchronous shape.

### 1.14 DESIGN-v1's per-request `cf-ray` dedupe vs A7's request-path budget

- **DESIGN-v1:52**: "`cf-ray` single-use dedupe" (a short-TTL store makes replay strictly single-use).
- **A7 §1**: request path hard budget "**≤10 µs CPU and 0 fsyncs per request**" and "**no new global
  lock** on the request path".
- A per-request insert/check into a TTL store is a state mutation on the hot path (the repo's own store is
  behind a global `Mutex`, A7 §0). The control is not budgeted and contradicts A7's constraint; A9 drops
  it.

### 1.15 A5's India verdict vs R2-legal-privacy's India verdict (severity, not direction)

Both say "consent or don't process", but **A5 F5 (high)** recommends a jurisdiction gate + "no device mode"
while **R2-legal-privacy §9.7(7)** recommends a consent-gated mode for India. They are different products
(gate = never collect; consent = collect with a Rule 3 notice), and DESIGN-v1 has **neither** — B7's minors
clause is the only population control, and no report notices that the adopted design has no India path at
all.

---

## 2. Quantitative claims: MEASURED / SOURCED / ESTIMATED / UNSOURCED

### 2.1 A7 (performance & cost)

| Claim                                                                                                                                                                                                   | Verdict                                                                | Basis                                                                                                                                                                                                                   |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Session UPSERT median 56 µs, p95 85 µs, max 162 µs (WAL+FULL)                                                                                                                                           | **MEASURED**                                                           | A7 §0 table; no script committed in this directory — self-reported                                                                                                                                                      |
| SQLite PK read 2.7–7.2 µs; 10 s-flush shape 1.4 ms/2000 upserts; HMAC 1.50 µs (Python); SHA-256 0.31 µs; Intl 5.62 ms; WASM 3.78 ms; JS loop 12.84 ms/3M; feature matrix 1.93 ms; payload 0.67 ms/426 B | **MEASURED**                                                           | A7 §0; self-reported, machine-class anchors (bun/JSC for client items)                                                                                                                                                  |
| **0.5 ms per fsync on the VPS**                                                                                                                                                                         | **ESTIMATED — no basis**                                               | A7 §1: "a cheap VPS ext4/NVMe is 0.1–0.5 ms … Use **0.5 ms** as the working number". No measurement, no citation, no hardware spec                                                                                      |
| **~950 req/s ceiling**                                                                                                                                                                                  | **ESTIMATED (derived)**                                                | 1/(1.0 ms + 0.05 ms) from the 0.5 ms estimate; DESIGN-v1:13 labels it an estimate                                                                                                                                       |
| ~1 800 / ~3 300 / ~8 900 req/s variants                                                                                                                                                                 | **ESTIMATED (derived)**                                                | same arithmetic; the "today" row adds an unexplained 0.1 ms to a 0.2 ms read cost                                                                                                                                       |
| **60–200 ms flush lock hold, every 10 s**                                                                                                                                                               | **ESTIMATED**                                                          | N = 2000 buckets is an assumption (no traffic data in repo); 30–100 µs per `txn.execute().await` is an estimate (engine floor measured at 1.4 ms). A3 §1.4 independently describes the same flush but gives no duration |
| "2–5× slower on CPU … much slower on fsync" for the VPS                                                                                                                                                 | **ESTIMATED**                                                          | A7 §0; no citation                                                                                                                                                                                                      |
| "low-end Android multiplier 5–15×"                                                                                                                                                                      | **ESTIMATED**                                                          | A7 §3, labelled                                                                                                                                                                                                         |
| Workers pricing ($5/10M + $0.30/M); Logpush ≈ $0.05/GB                                                                                                                                                  | **SOURCED (Workers) / UNSOURCED (Logpush $/GB)**                       | A7 §6 cites the Cloudflare Workers pricing page; the Logpush transfer figure has no URL                                                                                                                                 |
| Logpush 0.5–1.5 KB/record, 30 s–minutes latency, no backfill                                                                                                                                            | SOURCED in R2-network-transport §7 item 4 (Cloudflare docs, URL given) | A7 repeats without the URL                                                                                                                                                                                              |
| JA4 = Enterprise + Bot Management                                                                                                                                                                       | SOURCED                                                                | R2-network-transport §4.1 (Cloudflare docs URLs)                                                                                                                                                                        |
| WebGPU availability is uneven; Safari 26 ships WebGPU                                                                                                                                                   | **SOURCED (Safari)**                                                   | R2-fingerprint-science §4.2; no per-version Android/iOS threshold has a source                                                                                                                                          |
| Storage/row-size/MAU assumptions (25k MAU, 1.4 devices, 40k rows/mo, 16–36 MB)                                                                                                                          | **STATED ASSUMPTION**                                                  | A7 §7 explicitly labels it ("the repo has no traffic figures"). Fair, but every storage conclusion inherits it                                                                                                          |
| Metrics cardinality 6k → 36k series; ~100–300 ns per histogram                                                                                                                                          | ESTIMATED                                                              | A7 §8                                                                                                                                                                                                                   |
| WebGL cold 50–200 ms; WebGPU 10–150 ms                                                                                                                                                                  | ESTIMATED                                                              | A7 §3, labelled "est."                                                                                                                                                                                                  |

**Cross-check with A3**: A3 §3 and A7 F1 agree that a per-request DB device read takes 60–95 % of the
authenticated-API DB budget (pool = 1); the fix is L1 + a session field, never a per-request `SELECT`
(DESIGN-v1:13).

### 2.2 A2 (false positives)

| Claim                                                                                                 | Verdict                                                            | Basis                                                                                                                                                                                                                                                                                                                                                            |
| ----------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **18.5 % mobile uniqueness** (and "≥81.5 % shared")                                                   | **SOURCED but population-transferred**                             | Gómez-Boix et al., WWW 2018, one top-15 **French** commercial site (R2-fingerprint-science §2.1, §3.2). A2 applies it to a "phone-first, locale-concentrated Arabic/Urdu/Indonesian/Bengali" audience and calls C1 _Critical_ on that basis. The study's own thesis is that uniqueness is population-dependent; the transfer is an assumption, not a measurement |
| **90.62 % Chrome, India mobile, Sept 2026** (and the 3.61/3.51/0.97/0.79/0.27 tail)                   | **SOURCED**                                                        | StatCounter, cited inline three times (§2.0, B5, F6) and in A2's source list with URL. Arithmetic checks out (non-Chrome = 9.38 %)                                                                                                                                                                                                                               |
| 45.52 % of fingerprints change after one day                                                          | **SOURCED**                                                        | FP-Stalker, IEEE S&P 2018 (R2-fingerprint-science §2.3)                                                                                                                                                                                                                                                                                                          |
| 32 % of browser instances cleared cookies in 8 months                                                 | **SOURCED**                                                        | Li & Cao, IMC 2020                                                                                                                                                                                                                                                                                                                                               |
| Pew 2019 (India 30 % non-owners; 48 % median / 56 % India borrowing; women 20 % vs men 5 %)           | **SOURCED**                                                        | URL given                                                                                                                                                                                                                                                                                                                                                        |
| IAMAI/Kantar 2024 (+24 % rural shared-device; 76 % under 19; 58 % female; 62 % NCCS CDE; ~80 % rural) | **SOURCED**                                                        | URLs given (report + The Hindu summary)                                                                                                                                                                                                                                                                                                                          |
| ~31 % of mobile web sessions in in-app browsers (MobiLoud 2026)                                       | **SOURCED (secondary, self-flagged)**                              | A2 labels it "secondary, flagged; treat as indicative"                                                                                                                                                                                                                                                                                                           |
| VMs 13 % of Chromium-desktop, tampering 4.4 %, VPN ~20 %                                              | **SOURCED**                                                        | FingerprintJS 2026 (vendor)                                                                                                                                                                                                                                                                                                                                      |
| "666 accounts held the IPBE flag"                                                                     | SOURCED via R2-ban-operations §1 (Wikipedia, "at time of reading") | no snapshot                                                                                                                                                                                                                                                                                                                                                      |
| "No published rate for kiosks"                                                                        | honest gap                                                         | —                                                                                                                                                                                                                                                                                                                                                                |
| Severity "Critical" on C1/C2/C4                                                                       | **ESTIMATED**                                                      | Severity is judgement; the _rate_ is the sourced part                                                                                                                                                                                                                                                                                                            |

### 2.3 A1 (threat model)

| Claim                                                      | Verdict              | Basis                                                                                                                                                                                    |
| ---------------------------------------------------------- | -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 45.52 % one-day change                                     | SOURCED              | as above                                                                                                                                                                                 |
| Mobile 18.5 %, overall 33.6 % uniqueness                   | SOURCED              | as above (same population caveat)                                                                                                                                                        |
| "65 % of ATO attacks already use advanced automation"      | SOURCED (vendor)     | Kasada via R2-session-binding §6.2                                                                                                                                                       |
| `MAX_GRANTS = 10 000`; login 100/min/IP; 64 KiB body limit | **MEASURED in repo** | `fresh_auth.rs:16`; `router.rs` nest limiter; `config/mod.rs` (not re-checked)                                                                                                           |
| "45.52 % … a browser update is ban evasion"                | inference            | The number measures _any_ change in a large attribute set; the design's key is deliberately coarse. Using it to size churn for the design's key is a general-to-specific leap (see §3.1) |

### 2.4 A6 (testability)

| Claim                                                                                                                                                                                          | Verdict                                 | Basis                                              |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------- | -------------------------------------------------- |
| happy-dom has no WebGL/WASM/`navigator.gpu`/`userAgentData`/`storage`; `canvas.getContext("2d") === null`; `navigator.webdriver === true`; hc = 8; platform "X11; Darwin arm64"; plugins empty | **MEASURED — reproduced by this audit** | direct probe of happy-dom 20.11.1                  |
| "Master CI runs zero tests"                                                                                                                                                                    | **MEASURED (git)**                      | `git show 53d3d421`                                |
| "≈70 tests", "11 new files", "1700-line quran_v1.rs", "699 messages"                                                                                                                           | ESTIMATED / not re-counted              | no script; the count claims are planning estimates |

### 2.5 A5 / A8 / A9 / A10

- A5's legal numbers are all **SOURCED with URLs** (EDPB 2/2023 v2.0, Planet49 ¶70 via EDPB, PECR Sch A1
  text, ICO SAT 29 Apr 2026, Uber €824,990,000, DPDP Rules commencement table, COPPA amendments,
  CCPA §§). Its uncertainty register is honest. No unsourced numeric claim found.
- A8's KPI targets (≥95 % precision, <5 % overturn, <0.1 % FP) are **SOURCED as _targets_** from
  R2-ban-operations §9 — which itself labels them as chosen bars ("no platform publishes this"). They are
  normative, not empirical; treating them as achievable rates is an overread.
- A9's "~500 ns per short ClientHello" is SOURCED (Cloudflare blog via R2-network-transport §8.1).
- A10's SQLite facts (`SQLITE_ENABLE_UPDATE_DELETE_LIMIT` off, 3.46.0, JSON1 on) are SOURCED to the
  vendored header/build.rs; not re-checked here.

### 2.6 Numbers in the design docs labelled more strongly than their source

- **DESIGN-v1:14** "measured 3.78 ms compile, 50–400 ms low-end" — the first is measured, the second is an
  estimate; the sentence labels the pair as measured.

---

## 3. True in general, false (or unsupported) for THIS repo

1. **"45.52 % of fingerprints change after a day ⇒ the device key churns" (A1 F3, A2 B6, DESIGN-v1:9).**
   FP-Stalker measured a large attribute set including canvas/audio/UA. The design's key is _deliberately
   coarse_ (screen bucket, hardware family, no canvas/audio) — the churn rate for a coarse key is lower,
   and no report estimates it. The direction is right; the magnitude is imported from a study of a
   different object. This matters because the anti-churn fix ("fuzzy re-link", A2 F7) was justified by
   the inflated number.
2. **"Server-side collection (headers/TLS) is the consent-free architecture" (A5 F1, R2-legal-privacy §8.4, §9.1-9.2).**
   Behind Cloudflare: Traefik has **no TLS hook** (R2-network-transport §4.2 — plugin model is
   Yaegi/WASM middleware, no ClientHello); the origin sees Cloudflare's rewritten headers, so origin-side
   header fingerprinting describes Cloudflare, not the eyeball (R2-network-transport §0 item 3); JA3/JA4
   are **Enterprise + Bot Management only** (§4.1); the all-plans TLS fields (`tlsClientCiphersSha1`,
   `tlsClientExtensionsSha1`) exist **only inside a Worker**, and the repo has no Worker (A3 F12, A9 F7,
   A6 §1(h)). So the "clean" server-side path that removes the ePrivacy problem **is not available on this
   stack without building the exact edge component A7 and A9 recommend against**. Three reports recommend
   it; none notices the conflict.
3. **"Edge-signed headers make JA4 trustworthy" / "sign request.cf fields" (DESIGN-v0:54, A5 L4).**
   JA4 is absent below Enterprise+BM, so the Worker can only sign fields the origin could derive from
   `CF-Connecting-IP` anyway (A7 F7). Signing a field that is never present is dead code; the design's
   stated value ("signed JA4") is not purchasable on the actual plan.
4. **"Purpose limitation in code" (DESIGN-v0:62).** One SQLite file, one `DATABASE_URL`
   (`docker-compose.yml:128`) — the forbidden join is one `SELECT` (A1 F11). A machine guard on _source
   text_ (A6 G3) cannot prevent a runtime join added later through a query builder; A1 F11's "separate
   store" is the only enforcement, and DESIGN-v1:70 adopts the weaker source-scan guard.
5. **"`device_key`/token HMAC cost is a non-issue" (A7 §5) — true, and irrelevant.** A1 §4(a) already
   concedes it; the actual levers are the DB write and the row growth. No contradiction, but the two
   reports spend different amounts of ink on it.
6. **"Cloudflare Worker adds 1–5 ms p50" (A7 F7).** Plausible in general, but this deployment is a single
   container behind Traefik on one VPS; the Worker also introduces a second secret store and a new deploy
   path that the repo has no tooling for (A6 §1(h): "There is no edge code in this repo"). The _cost_ is
   understated for a one-operator project; the _benefit_ is zero below Enterprise.
7. **"Appeals at ≤72 h first response / ≤7 days final" (A8 §4, R2-ban-operations §5).** True for a staffed
   platform. With one operator and (per A8 §6) no observed threat, the SLA is a promise no one is
   scheduled to keep. A8's own dashboard admits the queue may be empty — the SLA is untestable here.
8. **"`ROLE_MODERATOR` is the natural seat for the device console" (A8 F10).** True in the ACL, but the
   console is being built for an operator who is the Admin; seating a second role does not create a second
   human, and DESIGN-v1's "two-person for permanent" (line 19) depends on exactly that missing human.
9. **"`WEB_AUTH_ENABLED` is false by default" (A10 §8) — and `post_v1`/`post_comment_v1` are unconditional
   (verified).** So the repo today is _both_ a public content platform with user comments **and** a
   deployment where auth can be off. A5's DSA finding (Art 17 applies now) and A10's "device axis is
   auth-scoped and near-useless with auth off" are simultaneously true; no report reconciles what the
   product actually is (comments live, auth possibly off). That ambiguity changes both the legal analysis
   and the value of the feature.

---

## 4. Recommendations that cannot be implemented here

1. **"Compute the fingerprint server-side from TLS/JA4 via your reverse proxy"** (R2-legal-privacy §9.2,
   §8.4; A5 F1 option (a)). Traefik cannot compute JA3/JA4 (no TLS hook — R2-network-transport §4.2);
   Cloudflare exposes JA4 only on Enterprise+Bot Management (§4.1); the all-plans TLS digests require a
   Worker that does not exist (A6 §1(h)). The compliance-clean alternative is not buildable on this stack.
2. **"Encrypt `device_key` at rest with a separate KMS key; restrict decryption to the abuse-review role"**
   (A5 §1 L2). There is no KMS in this stack (A7 §5: "there is no KMS in this stack"), no role-scoped
   decryption path, and the only at-rest tool is the single env-held `FIELD_ENC_KEY`
   (`state.rs:108-179`). A5's control requires infrastructure the project does not have and A5 never
   budgets.
3. **"Two-person for permanent bans" / "reviewed by someone not involved in the original decision"**
   (DESIGN-v1:20; R2-ban-operations §5.4; A8 §8). One operator. A8's compensating control
   (`single_operator_control=true`) is the implementable version; the design text asserts a control the
   project cannot staff, and a control that is not exercised is the same Uber-pattern exposure A5 F4
   calls CRITICAL.
4. **"Monthly out-of-band `VACUUM INTO 'easyquran.new.db'` on a copy, then swap"** (A7 §7.3). `VACUUM INTO`
   needs only a read lock; the **swap** of the file the running container holds open (pool = 1, WAL) does
   not. A single-container deployment has no atomic swap path without stopping the API; A7 does not budget
   the downtime, and the repo has no blue/green for the DB volume.
5. **Precision/overturn KPIs as designed** (A8 §5; R2-ban-operations §9): "precision = upheld ÷ (upheld +
   overturned) ≥ 95 %", "overturn < 5 %". At this scale the denominator is the number of _human-adjudicated_
   proposals, and R2-evasion §8.1/A8 §6 found no evidence of an abuse population to generate them. A
   dashboard over N≈0 cases cannot distinguish a good rule set from a broken one; the metric is
   unmeasurable, not merely hard. (Same class: A5's 17-artefact compliance set — DPIA, LIA, ROPA, per-MS
   ePrivacy analysis, DPAs, TIA, legal-hold SOP, DSAR SOP, appeal SOP, breach runbook, children's memo —
   needs legal capacity the project does not have; the artefacts are _doable_ but not by the team that
   exists.)
6. **Bonus — "dedupe `cf-ray` per request"** (DESIGN-v1:52) conflicts with A7 §1's
   "no new global lock / ≤10 µs" budget. Either the dedupe is a new hot-path lock or the single-use
   guarantee is dropped; both are unstated.

---

## 5. Overclaiming in DESIGN-v0 / DESIGN-v1

**DESIGN-v0 (asserted guarantee → evidence)**

1. "**make banning of abusive/illegal actors effective**" (line 3) → contradicted by A1 F5 (device signal
   exists only where `AuthClient` is used; anonymous readers/scrapers never send it) and A2 §3.3
   ("challenge spam on co-users plus a small recapture gain on naive evaders").
2. "**coherence booleans** … **spoof/tamper flags**" (32-33) presented as detection → A1 F1: client-authored,
   one line of JS, and the server is forbidden the raw inputs to recompute them.
3. "**Purpose limitation in code**: device records never joined to reading/recitation/search data" (62) →
   one SQLite file, one `SELECT` (A1 F11).
4. "**raw signals never stored**" (63) → contradicted by `evidence JSON` (39), the `audit_logs` reuse (42),
   and the repo's existing raw UA/IP in `user_sessions` (A8 F6, A10 F10).
5. "**Edge** … origin strips client-supplied `X-Eq-Edge-*` and verifies" (54) → nothing strips anywhere in
   the stack (A3 F12, A9 F7; `rg X-Eq-Edge` → 0).
6. "**Step-up (existing 5-min fresh-auth grants)**" (48) → login itself issues the grant, so a grant
   cannot serve as a challenge; process-global cap (A1 F7, A3 §6).
7. "**Cloudflare Worker adds signed `X-Eq-Edge-*` headers (request.cf fields …)**" (54) → `request.cf`
   includes city/lat/long/postal/timezone (A5 F6: precise-geolocation injection), and JA4 is not available
   on the plan.
8. "**L4 hold for review (<=72h)**" and "**L6 ban + second look**" (47) → no timer, no queue, no case
   record; with one operator both become silent-permanent or theatre (A8 F3, F9).
9. "**Retention ladder**" (63) → no `expires_at` in the schema (A5 F2); the ladder is a sentence.
10. "**/admin/devices API … minimal UI**" (58) → no admin UI exists in `web/src/routes`, and the repo's
    convention is API + scripts (A8 §0, §2). The UI is a new build, not a "minimal" addition.

**DESIGN-v1 (asserted guarantee → evidence)**

11. "**measured 3.78 ms compile, 50–400 ms low-end**" (14) → only the first is measured; the sentence presents
    both as measured.
12. "**0 DB ops on the request-evaluation path**" (13) → holds for evaluation only: the login/link write and
    the session-payload TTL refresh (A7 §2 allows "≤1 read") are DB operations, so the zero must not be
    quoted for the request path as a whole.
13. "**two-person for permanent**" (20) → one operator (A8 §8).
14. "**`cf-ray` single-use dedupe**" (52) → conflicts with A7's hot-path budget; not budgeted.
15. "**no fingerprint for logged-out readers**" (72) → contradicted by the chosen attachment point (A4 §7(a):
    the CSRF bootstrap is an anonymous `AuthClient.#request`).
16. "**Purpose limitation enforced by schema boundary + a machine guard that fails the build on any device
    x reader-data join**" (70) → the guard A6 sketches is a substring scan of source text; it cannot fail
    the build on a runtime join through the ORM.
17. "**minors: self-declared DOB -> minor flag -> collector off**" (72) → self-declaration is not
    verification (A5 §8 says so); in India the honest posture A5 recommends is _no accounts for
    self-declared under-18s_, which DESIGN-v1 does not adopt.
18. "**Where an audit found a blocker, the blocker's fix is folded in**" (line 3) → false for at least
    A3 F1 (serde default — v1 does not mention session wire compat), A5 F1 (no consent path), and A4's
    "no await in `#request`" (v1 B2 does not state the attachment allowlist).

---

## 6. Questions nobody asked (and that matter)

1. **What is the measured abuse base rate for _this_ app?** No report produced one. A8 §6 ("no observed
   threat"), R2-evasion §8.1 ("I found no public threat report … against Quran apps"), R2-ban-operations
   §4 ("no vendor publishes a device-fingerprint ban false-positive rate"). Every threshold (80/40), every
   KPI, and the entire cost/benefit case is calibrated against an unmeasured population. One query over
   existing logs (bans issued, spam comments actioned, scrape volume in the last 12 months) would settle
   whether the feature has a target at all.
2. **What happens on a deployment change?** (a) Origin change / domain move: the first-party device token is
   origin-scoped — every device becomes "new" on any domain move, and A4's guards forbid the cookie that
   would survive it. (b) Cloudflare removed: `IP_SOURCE=CfConnectingIp` + "400 if absent in prod"
   (`client_ip.rs:113-148`, DESIGN-v0:11) means the API stops answering; JA4/geo vanish; the edge design is
   dead. (c) DB moved off SQLite: A10's partial UNIQUE upsert (`ON CONFLICT … WHERE revoked_at IS NULL`),
   `id IN (SELECT … LIMIT)`, and the `SQLITE_ENABLE_UPDATE_DELETE_LIMIT` workaround are SQLite-shaped; the
   pool-of-1 argument evaporates; A9's TEXT PK vs A10's INTEGER PK becomes a portability decision nobody
   made. (d) `PUBLIC_API_BASE_URL` made absolute: A4 §7 and A3 §7 both flag the CORS `allow_headers`
   update, but nobody asked what a cross-origin device token means for the HttpOnly cookie vs localStorage
   decision. (e) Quran API becomes the only product: A10 §8 shows the device axis is auth-scoped and
   near-useless without accounts.
3. **Change of control / open-sourcing.** A1's premise is "the project is open source", yet no report asks
   who holds the pepper, who answers the DSAR and the 12-month ban ledger on an acquisition, or what
   publishing the rule weights and thresholds does to their effectiveness. A1 F10 treats published
   thresholds as a tuning problem; on an open-source project it is a permanent one, and the _compliance_
   artefacts (DPIA, ROPA, retention schedule) are non-transferable without a named controller — which no
   report names.
4. **TLS-inspecting middleboxes.** Corporate proxies, antivirus TLS interception, and enterprise
   break-and-inspect present the _proxy's_ TLS/h2 fingerprint (R2-network-transport §3.2 notes proxies
   present the proxy's TCP fingerprint; §5.2 notes TLS/UA mismatch is a _detector_). A2's D-row lists
   privacy browsers, Tor, WebView and proxy-rendered browsers but **not** the enterprise-middlebox
   population — which will trip every coherence rule and is not a privacy-protected cohort that A2's
   mitigations ("known-good masking allowlist") cover. A whole false-positive class is missing.
5. **Falsification criteria for the project itself.** Q2 of DESIGN-v1 asks "is fingerprinting the right tool
   at all", but no report answers it with a decision rule. Nobody wrote down what observation would make the
   team stop (e.g. "if <5 ban-worthy events/quarter, ship nothing"), nor what the total build cost is
   (A5's 17 artefacts + A8's case/appeal/legal-hold/metrics surface + A6's ~70 tests + A9's crypto surface +
   A7's ops jobs) against the _demonstrated_ threat. Without a kill criterion, the audits function as
   optimisation of a decision that was never tested.

---

## 7. The strongest counterargument to the entire project (steelmanned)

The project is a large, legally exposed, operationally demanding mechanism built to defend a resource that
is deliberately public, against a threat that has never been measured, for an audience on which the
mechanism is most likely to misfire — and it does not even cover the abuse it names. Start with the
evidence the reports themselves produced: the Arabic Quran text is immutable, public-domain and _meant_ to
be copied (R2-evasion §8.3); the documented attacks on Quran apps are app-store takedowns and report-bombing,
not account abuse (R2-evasion §8.1-8.2); and no report, vendor, platform or study publishes a ban precision
or a false-positive rate for device fingerprinting (R2-ban-operations §4). Against that, the fingerprint
layer's own audits conclude that it cannot see anonymous scraping at all (A1 F5), that its key is a cohort
key (18.5 % mobile uniqueness on one French site in 2018, A2 F1), that its coherence signals are client-authored and unforgeable
one-liners (A1 F1), that the ban key churns with browser updates (A1 F3), that its token is a bearer
credential on a CSP that trusts three third-party script origins (A1 F4), and that the honest accounting of
its marginal value is "challenge spam on co-users plus a small recapture gain on naive evaders" (A2 §3.3) —
while the same reports rank _per-credential rate limits_ as the single highest impact-per-effort
countermeasure, needing no fingerprint at all (A8 §9, R2-evasion §10). The costs are not hypothetical: on
this stack the per-request variant alone consumes 60–95 % of the authenticated-API DB budget on a
pool-of-1 SQLite connection (A7 F1), the schema and retention machinery is four new tables plus a sweeper
plus a rollback story that permanently loses every device identity (A10 §4, F11), the collector cannot be
tested by any gate the repo has (A6 headline: happy-dom has no WebGL/WASM/canvas; master CI runs zero
tests), the legal work is a DPIA, an LIA, a ROPA, a per-Member-State ePrivacy analysis, processor DPAs, a
retention schedule, a DSAR SOP, an appeal SOP, a legal-hold SOP and a children's memo (A5 §10) — and the
audience that pays for all of it is the audience the system is most likely to flag: shared family phones,
madrasa labs, CGNAT mobile, low-end Android, in-app browsers, privacy-hardened and censorship-country
readers (A2 Parts A–D), the same population the design promises in Layer 0 never to penalise. Meanwhile
the one genuinely defensible asset — the account and its credential — is already bannable with `user_id`
through a `user_bans` table that exists and that `check_ban` already enforces at login, but that nothing
ever writes (A8 F1); wiring that, plus failure-only per-credential limits (the existing ones also count
successes), plus the existing moderation queue, delivers most of the abuse reduction
at a fraction of the cost and none of the Art 9/ePrivacy/DPIA surface. The correct v1 is therefore: wire
`user_bans`, make the per-credential limits failure-only, add per-API-key rate limits, ship the appeal path, and spend the rest of the
budget on the threats the evidence actually shows (takedown contingency, review-bombing defence, scraping
tolerated with a cheap legitimate bulk path) — with device identity revisited only if a measured abuse
base rate and a working server-side signal source (not available behind this Cloudflare plan) appear.

---

## 8. Deliverable summary

**Top 10 unsupported / contradicted claims** — see §1.1–1.11:

1. "Each migration is wrapped in a transaction" — false on SQLite; the version row is written after `up()` (§1.1).
2. `device_sessions … ON DELETE CASCADE` to `user_sessions` — erases ban-evasion linkage on account deletion; no FK (§1.2).
3. "No device cookie, ever" — a `Path=/api` cookie cannot reach `/t/**`; the rule is "no cookie on reader paths" (§1.3).
4. A3 F12 "fail closed" vs A9 F11 "never reject" vs A1 F6 "fail open for reads" (three policies, one control).
5. A1 F6 HMAC fix vs A9 F10 Ed25519 (DESIGN-v1 adopts Ed25519; A1's property requires it).
6. A1 §(c) "worth building" vs A7 F7 "unjustified" vs A9 §8 "defer" (one component, three verdicts).
7. `UNIQUE(device_key)` — a cohort key; `token_hash` is the identity (§1.7).
8. A9 vs A10 both present _the_ device migration with incompatible `id`/`token_hash`/`trust_state`/`device_bans`; neither has the `expires_at` that A5 F2/DESIGN-v1:23 require (§1.8).
9. DESIGN-v1:72 "no fingerprint for logged-out readers" vs the adopted attachment point (A4 §7(a) CSRF bootstrap).
10. "Remove gstatic/googletagmanager from the CSP" — Firebase Analytics is live and loads them (§1.10). Runner-up: "origin strips edge headers" — nothing strips (§1.11).

**Top 5 unimplementable recommendations** — see §4:

1. Server-side TLS/JA4 collection via the reverse proxy as the consent-free architecture (Traefik has no TLS hook; CF JA4 = Enterprise+BM; no Worker in repo).
2. KMS-backed field encryption with role-scoped decryption (no KMS, no roles; only env-held `FIELD_ENC_KEY`).
3. Two-person permanent-ban review / independent appeal reviewer (one operator; A8's `single_operator_control` is the only real option).
4. Monthly `VACUUM INTO` + file swap on a live single-container SQLite volume (the swap needs downtime nobody budgeted).
5. Precision/overturn KPIs as designed (unmeasurable at N≈0 human-adjudicated cases) — and, same class, the 17-artefact compliance set with no legal capacity.

**Top 5 missing questions** — see §6: (1) measured abuse base rate; (2) deployment-change resilience
(origin/CF/SQLite/API-base/only-Quran-API); (3) change-of-control and open-source custody of pepper,
DSARs, ban ledger, rule weights; (4) the TLS-middlebox false-positive population; (5) falsification
criteria and total cost/benefit for the project itself.

**Steelmanned counterargument** — see §7: public text, unmeasured threat, misfiring audience, uncovered
abuse, high legal/ops/DB cost; the account+credential path already exists and is unwired; correct v1 is
`user_bans` + per-credential limits + appeals, device identity only if a base rate and a server-side
signal source appear.

_End of B10._
