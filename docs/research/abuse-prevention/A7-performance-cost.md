# A7 — Performance & cost envelope of DESIGN-v0 (device identity)

Audit of `DESIGN-v0.md` against the _actual_ EasyQuran stack. Every stack fact below was read
from source (file:line cited). Measurements are from this dev machine (Apple Silicon, APFS,
page-cache warm) and are labelled; the deploy target is a small arm64 Ubuntu VPS
(`deploy/README.md:547` — "≥2 GB RAM"), which is 2–5× slower on CPU and much slower on fsync.

## 0. Verified stack facts that bound everything

| Fact                                                                                                                                                                                      | Source                                                                                                           |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| sea_db pool = **1 connection**, acquire timeout 8 s, WAL, busy_timeout 5000, FK on. No `synchronous` override.                                                                            | `src/db/sea_connect.rs:35-55`                                                                                    |
| sqlx pins `PRAGMA synchronous` to the SQLite default = **FULL** ("The `synchronous` pragma defaults to FULL"). WAL + FULL = **one fsync per write commit**.                               | `~/.cargo/registry/.../sqlx-sqlite-0.8.6/src/options/mod.rs:187-189`                                             |
| Sessions: `load` = 1 PK SELECT, `save` = 1 UPSERT; tower-sessions saves only when the record was modified (`always_save` is NOT set).                                                     | `src/services/session_store.rs:166-210`; `tower-sessions-0.14.0/src/service.rs:225,251`                          |
| Per authenticated request the app already does **2 reads**: session SELECT + `backend.get_user` SELECT.                                                                                   | `rux-auth/src/session/extractor.rs:43-48`; `src/services/auth.rs:520-527`                                        |
| Ban check = 1 read + 1 session write, gated by a **5-min staleness cache inside the session payload** (`ban_checked_at`).                                                                 | `rux-auth/src/middleware/guard.rs:46-56`; `session/state.rs:59-63`; `requirements/builder.rs:16`                 |
| Revocation = process-local HashSet (Mutex), no DB.                                                                                                                                        | `src/services/auth.rs:583-595`                                                                                   |
| Rate limiter = 1 global `Mutex<HashMap>` (`limits`) + 1 (`claims`) + 1 atomic; sliding-log `VecDeque` per bucket. In-memory only.                                                         | `rux-request-gate/src/store.rs:116-120, 287-350`                                                                 |
| Rate-limit L2 flush every 10 s: 1 DELETE + **N separate `txn.execute().await` calls in a loop** (not a batch), under `PERSISTENCE_LOCK`.                                                  | `src/services/rate_limit_store.rs:126-229`                                                                       |
| Password work = semaphore, **2 slots default**, `spawn_blocking`.                                                                                                                         | `src/services/password_work.rs:8-15, 51-65`                                                                      |
| `http.server.*` labels = `http.method` + `http.route` (**MatchedPath template**) + `http.status_code`.                                                                                    | `src/middlewares/http_metrics.rs:8-13, 17-46`                                                                    |
| **Quran content path is a separate router with no session layer**, rate-limited only; its SQLite access uses its own read-only `immutable(true)` sqlx connections, not sea_db.            | `src/main.rs:897-927`; `src/quran/loader.rs:208-222`                                                             |
| docker-compose sets **no cpu/memory limits** on any service.                                                                                                                              | `docker-compose.yml` (no `deploy.resources`/`mem_limit`)                                                         |
| No `auto_vacuum` / `VACUUM` anywhere in the repo.                                                                                                                                         | grep: 0 matches                                                                                                  |
| No CF Worker / Logpush / JA4 code anywhere in the repo.                                                                                                                                   | grep `Logpush\|JA4`: 0 matches                                                                                   |
| Client budgets: `DEFAULT_TIMEOUT_MS = 12_000` (auth), worker `DEFAULT_TIMEOUT_MS = 30_000`, `LOCAL_BOOT_BUDGET_MS = 1500`, `LOCAL_HEDGE_BUDGET_MS = 400`, `DOWNLOAD_BUDGET_MS = 120_000`. | `web/src/lib/auth/auth-client.ts:7`; `quran/worker-client.ts:61`; `quran/fetch.ts:3,7`; `workers/download.ts:14` |
| Worker infra exists; IDB is a shared helper with `IDB_VERSION = 1`.                                                                                                                       | `web/src/lib/workers/*`; `workers/idb.ts:2`                                                                      |
| CSP already allows wasm (`'wasm-unsafe-eval'`) and workers (`worker-src 'self' blob:`); `'unsafe-eval'` is dev-only. sqlite-wasm already runs in `quran.worker.ts`.                       | `web/src/hooks.server.ts:80-93`; `web/server.ts:230`; `web/static/_headers:16`                                   |

### Measurements (this machine; label = floor, not the VPS)

| Op                                                                        | Result                                                       |
| ------------------------------------------------------------------------- | ------------------------------------------------------------ |
| SQLite PK read (200k-row table, WAL)                                      | 2.7–7.2 µs                                                   |
| Session UPSERT, WAL + `synchronous=FULL` (what sqlx uses)                 | **median 56 µs, p95 85 µs, max 162 µs**                      |
| Session UPSERT, `synchronous=NORMAL`                                      | 22 µs                                                        |
| 10 s-flush shape: 1 DELETE + 2000 upserts in one txn, raw `executemany`   | 1.4 ms (≈1 µs/stmt, engine only)                             |
| HMAC-SHA256 over 200 B                                                    | 1.50 µs (Python incl. overhead); raw SHA-256 200 B = 0.31 µs |
| `Intl.DateTimeFormat().resolvedOptions()` + `Locale.maximize()` first use | 5.62 ms                                                      |
| `WebAssembly.compile` + `instantiate` of an **8-byte** module             | 3.78 ms                                                      |
| JS fixed loop, 3M iterations                                              | 12.84 ms                                                     |
| 16-property feature matrix                                                | 1.93 ms                                                      |
| 426-byte payload JSON build+parse                                         | 0.67 ms                                                      |

---

## 1. Per-request device evaluation: cost and the req/s ceiling

### What §1+§2 of the design implies per request

Server receives the client payload → `device_key = HMAC-SHA256(components, pepper)` →
`SELECT … FROM device_records WHERE device_key = ?` (UNIQUE) → `UPDATE device_records SET last_seen, seen_count`
→ upsert `device_sessions`. That is **1 indexed read + 2 commits + 1 HMAC + 1 rule-engine eval**.

### Cost per request (VPS arithmetic)

- Commit cost = 1 fsync. Local APFS floor 56 µs; a cheap VPS ext4/NVMe is 0.1–0.5 ms and
  network-backed block storage 0.5–3 ms. Use **0.5 ms** as the working number — an assumption, not a VPS measurement, so every ceiling below is an estimate to recalibrate on the real host.
- 2 writes = 2 commits = **1.0 ms/request** of pure commit time. Batch both into one txn → 0.5 ms.
- 1 read through sea-orm/sqlx (statement build + param encode + row decode + pool acquire/release)
  ≈ **30–100 µs** (raw engine is 2.7–7.2 µs; the app stack is 10–30× the engine cost).

### Ceiling for the pool of 1 (all DB work serialized) — estimates derived from the 0.5 ms fsync assumption

```
ceiling = 1 / (DB time per request)
per-request device eval, 2 commits:  1/(1.0 ms + 0.05 ms)  ≈  950 req/s
per-request device eval, 1 txn:      1/(0.5 ms + 0.05 ms)  ≈  1 800 req/s
today (2 reads, 1 write/5 min):      1/(0.2 ms + 0.1 ms)   ≈  3 300 req/s   ← and the 10 s flush is not in this number
local floor (56 µs commits):                               ≈  8 900 req/s
```

The device feature would consume **60–95% of the entire authenticated-API DB budget** in the
per-request variant. On the deploy VPS the whole authenticated API tops out at roughly
**250–1000 req/s**, and the device write is the majority of it.

### The request budget that remains

Everything else shares that one connection: session load (1 read/request), user load (1 read/request),
ban refresh (1 read + 1 write per session per 5 min), the 10 s ban flush, the 60 s
translation-popularity flush, admin/ban mutations, and the hourly session sweep.

The 10 s flush is the sharp edge. It holds the connection for
`N × per-await-overhead`:

```
N = 2 000 live rate-limit buckets (assumed), 30–100 µs per txn.execute().await (estimated)
  → 60–200 ms of exclusive connection time, every 10 s (0.6–2% duty cycle) — estimate
```

Any request that lands in that window waits up to 200 ms — _before_ the device feature adds anything.
Adding 1–2 DB ops to every request increases the probability of colliding with it and doubles the
queue behind it. (Raw engine floor for the same 2000 upserts is 1.4 ms; the gap is the per-await
round trip, i.e. the app's own overhead is 40–140× the engine cost. Batching the flush into one
multi-row upsert is a free win worth taking regardless.)

### Maximum acceptable cost per request

**0 DB round-trips and 0 commits on the request path.** Concretely, per authenticated request the
device axis may spend:

- 1 in-memory map read (bounded LRU, `Arc<DeviceRisk>` clone) — ~100–200 ns;
- ≤1 HMAC verification — **0.3–1.0 µs** (3 SHA-256 compressions of ≤200 B);
- ≤1 counter bump;
- **no new global lock** on the request path. The `limits` Mutex is already acquired 2–4× per
  request by the rate limiter; a second global device lock would serialize every request behind
  it. Use `RwLock`/sharded map with copy-out-on-read (the same discipline `snapshot()` uses).

Hard budget: **≤10 µs CPU and 0 fsyncs per request.** Anything above that is a regression on a
pool of 1.

---

## 2. Where to amortize (concrete design + staleness + security consequence)

**Primary: session payload cache, mirroring the existing 5-min ban cache.**
`AuthSessionState` already has `ban_checked_at` + `is_banned` and `check_requirements` re-reads only
when stale (`rux-auth/src/middleware/guard.rs:46-56`). Add the same pair for the device axis:
`device_eval_at: Option<DateTime<FixedOffset>>`, `device_risk: u8`, `device_trust_state: u8`,
`device_id: Option<[u8;16]>` (an opaque id, never the fingerprint). The session row is loaded on
every authenticated request anyway, so reading these fields costs **0 extra DB ops**.

- Refresh policy: re-evaluate only when `device_eval_at` is stale (> 5 min, same knob family as
  `ban_cache_duration`) or when the request mutates the session (login, credential change,
  2FA change, device change) — and coalesce that refresh with the ban refresh that is already
  writing the session row in that same request (`guard.rs:49`), so it adds **1 read and 0 extra writes**.
- Invalidate immediately on: login, password change, session rotation (`cycle_id`), and on a
  `device_id` that differs from the one in the payload.

**Secondary: in-memory device cache in front of `device_records`.** Bounded LRU/TTL
(`HashMap<[u8;32], (Arc<DeviceRow>, Instant)>`, 10k entries ≈ 1–2 MB, 5-min TTL) so even the
5-min refresh is free. Miss → 1 DB read. Never the source of truth; eviction is safe.

**Tertiary: async writes on the existing ticker pattern.** `last_seen`, `seen_count`, coherence
history and `device_sessions` go through a dirty-set drained by a ticker — precedent exists twice
(`spawn_flush_task`, 10 s; translation popularity every 6th tick ≈ 60 s,
`src/services/rate_limit_store.rs:30-37, 126-178`). Add the device drain as the third companion
(same task, outside `PERSISTENCE_LOCK`) or a 60 s ticker. One commit per tick instead of one per
request: **1440 commits/day instead of 1 per request**.

**Push, don't poll, for bans.** A ban created at L5/L6 must be inserted into the in-memory device
cache immediately (the same way `revoked_sessions` is a HashSet and `clear_limit` clears L1
state), so enforcement latency is 0 for the ban action itself; the TTL is only a fallback for
out-of-band DB edits.

### Staleness windows and their security consequence

| Path                  | Staleness | Consequence                                                                                                                                                                                                                                                                                                                          |
| --------------------- | --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Session payload cache | ≤ 5 min   | A device flagged/banned at T is enforced at T+≤5 min on API requests. A mid-session device _swap_ (cookie theft + signature change) is invisible until the next boundary — the same latency class as today's ban cache, so no regression, but it means the device axis cannot gate a "ban-relevant action" without a forced refresh. |
| In-memory LRU         | ≤ 5 min   | Same as above; a miss (eviction/restart) costs 1 read, not a security gap.                                                                                                                                                                                                                                                           |
| Ticker persistence    | ≤ 60 s    | Cosmetic only (`last_seen`/`seen_count` accuracy).                                                                                                                                                                                                                                                                                   |
| Ban push              | 0         | Immediate.                                                                                                                                                                                                                                                                                                                           |

Rule: any action in §3 that is _itself_ ban-relevant (credential change, ban/unban, legal-referral
evidence, appeal) forces `device_eval_at = None` in the same request → 1 read + 0 extra writes,
because the session row is written by that action anyway.

---

## 3. Client cost per signal

Measured numbers are from this machine (bun/JSC — _not_ a browser; treat as engine-class anchors).
Low-end Android multiplier for CPU-bound work: **5–15×**. Estimates (marked "est.") are for a
mid-range Android browser.

| Signal                                         | Cost (measured / est.)                                                                    | Low-end Android | Must be a Worker?                                                                                                  |
| ---------------------------------------------- | ----------------------------------------------------------------------------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------ |
| WebGL context + `getParameter` + extension set | warm 3–20 ms; **cold GPU-process start 50–200 ms (est.)**                                 | 50–300 ms       | **Yes** (OffscreenCanvas WebGL in worker). Fallback to main thread = `requestIdleCallback` + hard abandon timeout. |
| WebGPU `requestAdapter()`                      | 10–150 ms (est.); availability is uneven across Android browsers (Safari 26 ships WebGPU) | 20–250 ms       | **Yes**, and must be optional (never a required component).                                                        |
| WASM compile+instantiate                       | **3.78 ms for an 8-byte module (measured)**                                               | 20–60 ms        | **Yes**                                                                                                            |
| JS timing loop                                 | **12.84 ms / 3M iters (measured)**                                                        | 65–190 ms       | **Yes**                                                                                                            |
| Feature matrix (16 probes)                     | **1.93 ms (measured)**                                                                    | 10–30 ms        | Main thread OK if deferred                                                                                         |
| UA-CH `getHighEntropyValues()`                 | 0.5–20 ms (est., browser-process IPC); absent in Firefox/Safari                           | 2–30 ms         | Main thread OK (async), optional                                                                                   |
| `Intl` resolver                                | **5.62 ms first use (measured)** — ICU data load                                          | 20–60 ms        | Prefer Worker, or reuse an existing instance                                                                       |
| `navigator.webdriver` / native-fn tamper check | 0.1–5 ms (stringifying functions is the cost)                                             | ≤ 25 ms         | Main thread OK                                                                                                     |
| Payload build + JSON                           | **0.67 ms / 426 B (measured)**                                                            | ~3 ms           | anywhere                                                                                                           |

**Main-thread budget for a first-paint-adjacent task on low-end Android:** no single task > **50 ms**
(that is the Long Task / RAIL threshold), and for INP health the whole collector should stay
**≤ 20–30 ms total in ≤ 5 ms chunks**, deferred past first paint. This repo's own convention
supports that: worker boot gets a 1500 ms budget and reads hedge after 400 ms — nothing
user-visible waits on background work.

**Where it must live:** a **separate, lazily-spawned** `new Worker(new URL(...), {type:"module"})`.
CSP `worker-src 'self' blob:` already allows it (`hooks.server.ts:93`). Do **not** reuse
`quran.worker.ts`: its boot is artifact-staging work with its own budget
(`worker-client.ts:576-618`), and adding GL/WASM probes to it would put device work behind a
multi-MB download. The `web/src/lib/workers/` helpers (`idb.ts`, `storage.ts`) are reusable.
Storage: `localStorage` first (sync, ~µs), IDB as backup only — opening a new IDB connection is
5–30 ms cold, and `IDB_VERSION` is pinned at 1 with no `onupgradeneeded` path
(`workers/idb.ts:2,20`), so a new DB _name_ is required (the design says this; confirmed correct).

---

## 4. Is the WASM/JS timing signature viable here?

- **CSP: yes, technically.** `'wasm-unsafe-eval'` is present in all three CSP sources
  (`hooks.server.ts:80`, `server.ts:230`, `static/_headers:16`), and sqlite-wasm already compiles
  and runs in `quran.worker.ts` — the toolchain is proven. No CSP change needed.
- **Cost: 3.78 ms just to enter the wasm compiler with an 8-byte module (measured).** A real
  signature (compile a 20–100 KB module, warm it up, measure a loop) is 10–80 ms on this Mac and
  **50–400 ms on a low-end Android**, plus 100 KB–1 MB of memory per compiled module. That is a
  long task, and it must be in a worker.
- **Watchdog/anti-jank risk:** a few hundred ms will not trigger Chrome's "Page Unresponsive"
  dialog (~10 s of blocked main thread), but it _is_ a Long Task and it feeds INP. In a worker it
  is safe from jank — but then it is also subject to timer/background throttling, thermal
  throttling and CPU frequency scaling, which is exactly what makes the measurement unstable.
- **Entropy: ~0.** The timing bucket distinguishes "fast/modern" from "slow/legacy" — 2–4 bits
  that are already covered by `navigator.deviceMemory` and `navigator.hardwareConcurrency`
  (1 property read each, ~1 µs, no compile, no memory).
- **Worse than useless for a _key_:** the bucket is **not stable across sessions on the same
  device** (thermal/background state, browser version, JIT tier-up). The design HMACs components
  into `device_key` with a 30-day retention → an unstable component means a **new `device_record`
  and a false "new device" step-up** every time the bucket shifts. This is a self-inflicted
  device-churn generator and a storage-growth driver.
- **Verdict: drop it.** If a performance cohort is wanted, use `deviceMemory` + `hardwareConcurrency`
  buckets, tagged "cohort input, never a key".

---

## 5. HMAC cost and correct placement

- **Cost:** HMAC-SHA256 over a ≤200 B payload = 2 key-block compressions + 1 message compression
  ≈ **0.3–1.0 µs in Rust** (measured proxy: SHA-256 200 B = 0.31 µs in C; HMAC 200 B = 1.50 µs in
  Python incl. interpreter overhead). At 1000 req/s that is 0.1% of one core. CPU-wise it is a
  non-issue.
- **Does it belong on the request path?** Only as a _verification_ of a server-minted opaque token
  (constant-time compare — the `rux-webhook-crypto` pattern, `MAX_SKEW_SECS = 300`). It must **not**
  be a per-request re-derivation of `device_key` from the full client component set: that re-parses
  client data, re-HMACs it, and — combined with an unstable component (see §4) — changes the key
  under the client's feet.
- **Correct placement:**
  1. **Mint time (first sight of a device / at login):** `device_key = HMAC(pepper, canonical_components)`,
     computed once, stored; the server also returns its own opaque 128-bit token. Thereafter the
     token is the primary lookup key and the HMAC value is the dedup/ban key.
  2. **Ingest boundary:** verify the token with HMAC + constant-time compare (~1 µs). This is the
     only per-request crypto.
  3. **Ban/link time:** the same key is used for `BanUnit::Device(device_key)`.
- **Two constraints:** (a) never spend a `PASSWORD_WORK` permit on this — that semaphore has 2
  slots by default and couples device eval to Argon2/login capacity (`password_work.rs:8-15`);
  (b) the pepper is an env secret with the `COOKIE_KEY`-style rotation story — there is no KMS in
  this stack, and a boot-time remote fetch would violate the offline-boot invariant.

---

## 6. Edge cost

- **Today:** exactly one edge-derived header is trusted (`CF-Connecting-IP`, peer-gated by
  `TRUSTED_PROXY_CIDRS`, 400 if absent in prod). **Zero** CF Worker, Logpush or JA4 code exists in
  the repo.
- **Worker on every request:** +1–5 ms p50 TTFB, up to +20 ms p99 on cold isolates, plus edge CPU
  (~0.1–1 ms for an HMAC + header signing). Pricing (Cloudflare Workers pricing page, developers.cloudflare.com/workers/platform/pricing — verify current
  rates): Workers free = 100k requests/day; paid = $5/mo for 10M requests + $0.30/M after → 10M req/mo ≈ $5, 100M req/mo ≈ $32. That is billed on _all_
  site traffic if the Worker sits in front of the zone, not just `/api/**`. A Worker that only adds
  `X-Eq-Edge-*` to `/api/*` (route-filtered) avoids most of that.
- **Header-only alternative:** no Worker at all. `request.cf` fields are **not** exposed to the
  origin as headers by default; you need either a Worker or a Managed Transform / Transform Rule.
  Managed transforms can add country/colo-ish fields but **not JA4**.
- **JA4 gating (as the design states):** JA4 is Enterprise + Bot Management only. **The design is
  viable without it, but only if it is honest about what remains:** origin-visible signals are
  `CF-Connecting-IP`, plus whatever a Worker forwards (`asn`, `colo`, `clientTrustScore` only with
  Bot Management). Without Bot Management the edge contributes geography/ASN/colo — all derivable
  from the IP — so the edge layer adds cost and no new signal. **Recommendation: make Layer 4
  optional in v0; ship origin-only; add the Worker only when the plan exposes a field the origin
  cannot see.** The ladder must not require JA4 above L2, or the feature silently degrades on the
  plan the site is actually on.
- **Logpush as an origin pipeline:** volume = requests × 0.5–1.5 KB/record (a full `cf` object is
  ~2 KB; NDJSON with selected fields ≈ 0.5 KB). 1M req/mo = 0.5–1.5 GB; 10M = 5–15 GB;
  100M = 50–150 GB. Transfer billing is plan-dependent (≈ $0.05/GB on Enterprise) → 100M req/mo
  ≈ $2.5–7.5 plus destination storage. **Latency: batched, typically 30 s to several minutes**
  (configurable batch size/interval), and delivery is best-effort: **gaps are not backfilled**.
  Therefore Logpush can feed _aggregates and dashboards only_ — never enforcement, never the
  request path, never a complete DSAR export.

---

## 7. Storage growth and retention

**Assumption (stated, since the repo has no traffic figures):** 25k authenticated MAU, 1.4
devices/user, 3 sessions/user/month, ~40% of MAU active in any 30-day window, ~1M authenticated
API requests/month (≈0.4 req/s average, peak 20–50 req/s).

| Table                            | Rate                                         | Row size                                                                      | Retention               | Steady state                                  |
| -------------------------------- | -------------------------------------------- | ----------------------------------------------------------------------------- | ----------------------- | --------------------------------------------- |
| `device_records`                 | ~40k devices/month (25k × 1.4 × 1.15 churn)  | 0.4–0.9 KB (32 B key + ids/timestamps + 6 booleans + 300–800 B evidence JSON) | 30 d                    | **16–36 MB** + 3.5 MB indexes                 |
| `device_sessions`                | 75k rows/month (write-on-login-only)         | ~100 B                                                                        | session lifetime (14 d) | **~4 MB** (grows to ~90 MB/yr if never swept) |
| ban records (keyed, no evidence) | ~500 bans/yr                                 | ~150 B                                                                        | 12 mo                   | < 1 MB                                        |
| `audit_logs` evidence            | ~10k rows/yr (2k flagged devices × 5 events) | ~1 KB                                                                         | policy                  | ~10 MB/yr                                     |
| **Total**                        |                                              |                                                                               |                         | **~30–50 MB steady state**                    |

Context: the existing `sessions` table already carries a 300–500 B msgpack payload per row and
runs ~15–20 MB steady state, so the device feature roughly doubles the _logical_ DB footprint.

**Does the ladder keep it bounded?** Live rows: yes. File size: **no, not by itself.**
`auto_vacuum` is not set anywhere (grep: 0 matches), so SQLite's default `auto_vacuum=NONE` means
deleted pages are reused but **never returned to the OS** — the file sits at a high-water mark.
And a plain `VACUUM` cannot run in-process on this stack without stalling every request for the
whole rewrite (2× the file size of I/O through the single connection).

Recommended strategy:

1. Enforce retention with **delete-on-write inside the existing ticker** (the 60 s companion), not a
   new job — one transaction per tick, matching `rate_limit_store::flush`.
2. Sweep `device_sessions` in the **existing hourly session sweep** (`main.rs:764-775`) by joining on
   `user_session_id` — otherwise the join table outlives the sessions it describes.
3. Monthly out-of-band compaction via **`VACUUM INTO 'easyquran.new.db'`** (SQLite 3.27+; needs only a read
   lock), then swap the file in a short maintenance window — replacing the live file requires closing the
   single connection, so the swap is not zero-downtime (C5 §4 #25). Set an explicit high-water-mark target (e.g. ≤ 200 MB)
   and alert on it. Do **not** try `PRAGMA incremental_vacuum` — it requires
   `auto_vacuum=INCREMENTAL` to have been set before the tables existed, and switching needs a full
   VACUUM anyway.
4. Keep the ban record **independent** of the 30-day `device_records` row (see finding 10): the
   12-month ban row must own its own copy of `device_key`, or the ban axis dies when the record is
   swept.
5. "Aggregates forever" needs a pre-aggregated table (day × trust_state × cohort counts) or the
   aggregates become a scan of retained rows.

---

## 8. Observability cost

Verified: labels are `http.method` + `http.route` (MatchedPath **template**, chosen precisely to
avoid cardinality explosion) + `http.status_code`, on `http.server.duration`,
`http.server.request.count`, `http.server.response.status`
(`src/middlewares/http_metrics.rs:8-13`; instrument names `src/utils/telemetry.rs:542-553`).

Adding a device dimension to those three instruments is wrong on two counts:

- **Cardinality:** ~120 routes × 5 methods × ~10 statuses ≈ 6k series today. `device.trust_state`
  (6 values) → ~36k. Anything keyed by `device_key` → **unbounded** (one series per device) — the
  classic OOM/bill explosion.
- **PII:** a `device_key` label exports a device identifier into a third-party metrics pipeline,
  contradicting the design's own Layer 6 purpose limitation.

What to add instead:

1. A separate `device.*` family with **closed-enum labels only**: `device.trust_state` (≤6),
   `device.coherence` (≤3), `device.eval.result` ∈ {fresh, cached, miss, error},
   `device.ladder.level` ∈ L0..L7. Never `device_key`, never `user_id`, never a raw component.
2. `device.eval.duration` (histogram, request path) and `device.persist.flush.duration` (ticker) —
   these are the instruments that _prove_ the per-request cost stayed at 0 DB ops.
3. `device.cache.hits` / `device.cache.misses` counters → hit-ratio guard; alert if hit ratio
   drops (that is the early warning that the feature started touching the pool).
4. `device.records.count` gauge (storage growth), plus `db.pool.connections` already exists
   (`telemetry.rs:356`).
5. Per-device detail goes in **traces**, not labels — and only if the trace backend is first-party.
6. Cost of the extra instruments: ~100–300 ns/request for one more histogram. Add **one**, not
   three; fold `eval.result` into its attributes.

---

## 9. Performance budget table

| Component                                          | Budget                                          | Measured / estimated                                                                        | Where it must live                                                |
| -------------------------------------------------- | ----------------------------------------------- | ------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| Device risk lookup                                 | ≤ 1 µs, 0 DB ops                                | LRU read ~100–200 ns                                                                        | API request path (in-memory)                                      |
| Device token HMAC verify                           | ≤ 1 µs                                          | 0.3–1.0 µs (est. Rust)                                                                      | API request path                                                  |
| Device eval refresh                                | ≤ 1 read, 0 extra commits, ≤ 5 min cadence      | 30–100 µs read; session row already being written                                           | API request path, only when stale                                 |
| Session payload additions                          | ≤ 40 B                                          | 4 fields in existing msgpack record                                                         | API request path (already loaded)                                 |
| `device_records` write (`last_seen`, `seen_count`) | 1 commit / 60 s total                           | 56 µs local / 0.5 ms VPS per commit                                                         | Async ticker (existing 60 s companion)                            |
| `device_sessions` link                             | 1 row at login only                             | ~100 B                                                                                      | Login handler (async-safe)                                        |
| Ban push to L1 cache                               | 0 latency, no DB                                | in-memory insert                                                                            | Ban mutation handler                                              |
| Client collector total                             | ≤ 20–30 ms low-end, ≤ 5 ms/chunk, ≤ 50 ms/task  | WebGL 50–200 ms cold, WASM 3.8 ms (measured), JS 12.8 ms (measured), Intl 5.6 ms (measured) | Dedicated lazy Worker (`web/src/lib/workers/`), after first paint |
| Client payload                                     | ≤ 1 KB                                          | 426 B for a representative payload (measured)                                               | Attached to `AuthClient.#request` only                            |
| Edge Worker                                        | +0 ms in v0; ≤ 5 ms p50 if enabled              | 1–5 ms p50 / 20 ms p99 est.                                                                 | Cloudflare Worker, `/api/*` route-filtered — **optional in v0**   |
| Logpush                                            | analytics only, 30 s–min latency, no backfill   | 0.5–1.5 KB/record                                                                           | Off-path; never enforcement                                       |
| Metrics                                            | 1 extra histogram, closed-enum labels           | ~100–300 ns/request                                                                         | API observability                                                 |
| DB file                                            | ≤ 200 MB high-water mark; monthly `VACUUM INTO` | +30–50 MB steady state                                                                      | Ops job                                                           |

---

## Findings

1. **Per-request device evaluation breaks the single-writer budget** — **blocker.** Design §1/§2 implies 1 indexed read + 2 commits + 1 HMAC per authenticated request. sqlx pins `PRAGMA synchronous` to the SQLite default FULL (`sqlx-sqlite-0.8.6/src/options/mod.rs:187-189`; `sea_connect.rs` never overrides it) → **1 fsync per commit**. At an assumed 0.5 ms/fsync on the VPS target (not measured there): 1.0 ms of commit time per request → **an estimated ~950 req/s ceiling for the entire authenticated API**, with the device feature consuming 60–95% of it (local floor 56 µs median / 162 µs max per commit; 250–1000 req/s range on slower disks). **Fix:** zero DB ops on the request path — write-on-login-only + the 5-min session-payload cache (§2), 60 s ticker for `last_seen`/`seen_count`, in-memory LRU in front of `device_records`.

2. **The existing 10 s ban flush already monopolizes the pool for 60–200 ms** — **major** (pre-existing, but the design must budget around it). `rate_limit_store.rs:199-219` issues **N separate `txn.execute().await`** in a loop: raw engine floor 1.4 ms for N=2000 (measured), but an estimated 30–100 µs per await through sea-orm/sqlx → **an estimated 60–200 ms of exclusive connection time every 10 s at an assumed N = 2 000** (0.6–2% duty cycle, up to 200 ms tail for any request that lands in it). **Fix:** add no DB reads to the request path; separately, batch the flush into one multi-row upsert (free win).

3. **WASM/JS timing signature is negative-value** — **major.** Measured: **3.78 ms** just to compile+instantiate an **8-byte** module, **12.84 ms** for a 3M-iteration loop (fast Mac) → 50–400 ms on low-end Android as a long task. Entropy is ~2–4 bits already covered by `deviceMemory`/`hardwareConcurrency` (1 property read each), and the bucket is **unstable across sessions** (thermal/throttling/JIT) → an unstable component inside the HMAC'd `device_key` **mints a new `device_record` and a false "new device" step-up** whenever it shifts, defeating the 30-day keyed retention. **Fix:** drop it; use `deviceMemory` + `hardwareConcurrency` as cohort inputs, never key material.

4. **Client collector must not run on the main thread** — **major.** Cold WebGL context 50–200 ms (est.), WebGPU `requestAdapter()` 10–150 ms (est.) with uneven availability, WASM compile 3.8 ms measured. Budget: **no task > 50 ms, total ≤ 20–30 ms in ≤ 5 ms chunks, deferred past first paint** (the repo's own convention: `LOCAL_BOOT_BUDGET_MS = 1500`, hedge at 400 ms). **Fix:** a dedicated, lazily-spawned Worker under `web/src/lib/workers/` (CSP `worker-src 'self' blob:` already permits it, `hooks.server.ts:93`); do **not** reuse `quran.worker.ts` (its boot is artifact staging with its own budget, `worker-client.ts:576-618`); main-thread GL fallback only via `requestIdleCallback` with a hard abandon timeout.

5. **`Intl` resolver is the sneaky client cost** — **minor.** Measured **5.62 ms** first use (ICU data load) → 20–60 ms on low-end Android. **Fix:** run it in the collector worker, or reuse an already-constructed formatter instead of `Intl.DateTimeFormat().resolvedOptions()` + `Locale.maximize()` per collection.

6. **HMAC belongs at mint/link time, not per request** — **minor.** 0.3–1.0 µs CPU is negligible, but re-deriving `device_key` from client components on every request re-parses client data and — with any unstable component — changes the key. **Fix:** mint `device_key = HMAC(pepper, canonical_components)` once, return an opaque 128-bit token, and per request do only an HMAC + constant-time token compare (the `rux-webhook-crypto` pattern); never consume a `PASSWORD_WORK` permit (2 slots, `password_work.rs:8-15`) for this.

7. **A Cloudflare Worker on every request is unjustified in v0** — **major.** +1–5 ms p50 TTFB (up to 20 ms p99 cold isolate) and per-request billing (free 100k/day; $5 per 10M + $0.30/M after). JA4 requires Enterprise + Bot Management, and without Bot Management the edge forwards only ASN/colo/country — all derivable from `CF-Connecting-IP`, which the origin already trusts. **Fix:** Layer 4 optional; ship origin-only; add the Worker (route-filtered to `/api/*`) only when the plan exposes a field the origin cannot see; the ladder must not require JA4 above L2.

8. **Logpush cannot be an origin/enforcement pipeline** — **major.** Batched delivery, typical end-to-end **30 s to several minutes**, and **gaps are not backfilled**. Volume 0.5–1.5 KB/record → 5–15 GB per 10M requests, 50–150 GB per 100M (transfer billing plan-dependent, ≈ $0.05/GB on Enterprise). **Fix:** use it for aggregates/dashboards only; state explicitly that it can never gate a request, never retry a lost record, and never produce a complete DSAR export.

9. **Storage growth is bounded in rows but not in file size** — **minor/major.** ~40k `device_records`/month × 0.4–0.9 KB = 16–36 MB/month; 30 d retention → **20–40 MB steady state**; `device_sessions` 7.5 MB/month must be swept with sessions. **No `auto_vacuum` is set anywhere** (0 grep matches) → default `auto_vacuum=NONE` leaves the file at a high-water mark, and a plain `VACUUM` through the pool of 1 stalls every request. **Fix:** retention deletes inside the existing 60 s ticker; add `device_sessions` to the hourly session sweep (`main.rs:764-775`); monthly out-of-band `VACUUM INTO` plus a file swap in a maintenance window, with an explicit ≤200 MB high-water target.

10. **The 12-month ban record conflicts with the 30-day `device_records` sweep** — **major.** If the ban row FKs to (or is derived from) a `device_records` row deleted at 30 d, the ban axis dies or cascades away — silently un-banning devices. **Fix:** the ban record owns its own copy of `device_key` with no FK dependency on the swept table; assert this in a test that deletes a 31-day-old record and then checks the ban still resolves.

11. **A device dimension in `http.server.*` explodes cardinality and leaks PII** — **major.** Labels are method + MatchedPath template + status (`http_metrics.rs:8-13`) ≈ 6k series; `device.trust_state` × 6 → ~36k; a `device_key` label is unbounded (one series per device) and exports a device identifier to the metrics backend, contradicting Layer 6's purpose limitation. **Fix:** a separate `device.*` family with closed-enum labels only; one `device.eval.duration` histogram plus cache hit/miss counters as the guard that the request path stayed DB-free; per-device detail in traces, not labels.

12. **Quran hot path is provably out of reach — keep it that way, and guard it** — **info.** `quran_v1` is mounted on the _public_ router with only rate-limit/CORS/IP/security layers and no session layer (`main.rs:897-927`), and its SQLite access uses separate read-only `immutable(true)` connections (`quran/loader.rs:208-222`), so a device middleware attached to the private router cannot touch it. The subtle hole is the translated-reader SSR path: web SSR calls the API's internal route with `INTERNAL_QURAN_API_TOKEN`, and a device header attached too broadly (e.g. to `quranApi`) would poison the 7-day disk cache and add per-request work to the highest-traffic path. **Fix:** attach the device payload **only** inside `AuthClient.#request` (`auth-client.ts:167-229`); add a test asserting no device header/payload is emitted by `quranApi` or the internal SSR client.
