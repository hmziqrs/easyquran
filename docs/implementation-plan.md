# Implementation Plan — closing the gap to `my-plan-raw.md`

**Status:** proposed (not started)
**Source of intent:** `docs/my-plan-raw.md` (owner-authored, never edited by agents)
**Source of current state:** `docs/quran-system.md`, plus the code ledger in §0

Each workstream states: **goal · files · steps · invariants · tests · done-when**.

---

## 0. Gap ledger

| # | Gap | Domain | Workstream |
|---|---|---|---|
| 1 | No durable cross-restart API-demand signal for translation prewarm | Rust | W1 |
| 2 | `IP_SOURCE=ConnectInfo` in deploy config; wrong behind CF→Traefik | Rust | W2 |
| 3 | Quran routes never escalate to a ban; no export, no un-ban path | Rust | W3 |
| 4 | Engagement counter is a sessionStorage int | Web | W4 |
| 5 | Arabic `readRange` has no API fallback; validator dropped on API path | Web | W5 |
| 6 | `RangeReader` has no client data path | Web | W6 |
| 7 | Translated routes intentionally have no SSG tier; degradation and recovery are incomplete | Web | W7 + W9 |
| 8 | Zero web-side auth; cache and origin prerequisites are incomplete | Both | W8 |
| 9 | Doc divergences unrecorded | Docs | W9 |
| 10 | Cross-cutting delivery and cache safety requirements | Both | W10 |

### Already satisfied — no workstream needed

- **`my-plan-raw.md:4`** Arabic fully in memory at boot. True (`quran/loader.rs`;
  `main.rs:438,448`; `exit(1)` at `:455`).
- **`my-plan-raw.md:7`** no Redis. True and deliberate (L1 in-mem + L2 SQLite).
  **Standing constraint** — W1 must not erode it.
- **`my-plan-raw.md:18`** "SSG pages are hydrated once the cache is available." True for surah
  routes today: `SurahReader` renders server data, then fills further pages from the worker
  once `onStatus` reports ready (`SurahReader.svelte:611,617-620`). **Not** true for range
  routes — that is W6.

### Planning facts

- **"SQLite database is 1.5 MB"** — true for the pinned Arabic DB (`quran-uthmani.sqlite` =
  1,593,344 B). But time-to-SPA also pays `sqlite3-*.wasm` ≈ 864,752 B → real cold cost
  ≈ **2.4 MB**. Size any "how long is the degraded window" reasoning off 2.4 MB.
- **`my-plan-raw.md:5`** literally reads "We *do* load all translated database on boot…". In
  context and against the code the intent is the opposite. This document records that reading;
  the owner-authored file remains unchanged.
- Reader route count is ≈ **1,410** (114 surah + 662 surah-local-page + 604 global page +
  30 juz).

### Global invariants

1. **Quran DBs immutable.** 2. **No SHA-256 over Quran data** (identity is the id; guarded by
`tests/quran_v1.rs` + `catalogue-sha-guard.test.ts`). 3. **Arabic pinned, only translations
evictable.** 4. **Reader hrefs use the `*For(ctx, …)` family** (guarded by `nav-guard.test.ts`).
5. **No per-user content in any shared cache.** 6. **No Redis.**
7. **`my-plan-raw.md` is read-only** — divergences go in W9.

---

## W1 — Durable API-demand prewarm (Rust)

**Goal:** preserve the existing bounded TTL + TinyLFU pool while carrying enough API-demand
evidence across restarts to prewarm likely cold-start translations (`my-plan-raw.md:5`).
**Files:** `quran/translation_pool.rs`, `services/translation_popularity_store.rs` (new),
`migration/src/m000003_translation_popularity.rs` (new), `config/settings.rs`, `main.rs`,
`modules/quran_v1/{controller.rs,dto.rs}`, `quran/mod.rs`, `deploy/.env.example`.

**Today:** TinyLFU already supplies in-process frequency-aware admission, with an idle TTL and a
post-build byte-bound LRU prune (`translation_pool.rs:76-97,168-192`). That satisfies bounded
runtime popularity. Missing piece is durable evidence for restart prewarm.

### What the durable signal means

The pool only sees requests reaching the Rust process. Translated pages are SSR behind a 7-day
HTML disk cache, and warm clients read their local worker DB. So pool hits measure **API
demand** — a more popular translation may produce *fewer* pool hits once caching works.

The pool exists to serve API demand, so name the persisted value `api_demand_score`, not
"reader popularity" (recorded in W9).

**Design decision:** durable demand governs **restart prewarm only**. Moka TinyLFU continues to
govern runtime admission; TTL/LRU/byte-bound govern eviction unchanged. If prewarm fails the
evidence threshold in step 8, remove the durable store too — a score with no consumer is dead
state.

### Steps

1. **Definition of a hit:** one increment per `get_or_build` call — i.e. per API request that
   needs the corpus. Not per ayah, not per surah.
2. **Counter with no second lock, and no lost counts on eviction.** Widen the existing map to
   `residents: Mutex<HashMap<TranslationId, ResidentMeta>>`,
   `ResidentMeta { tick: u64, hits: u64, evicted: bool }`, incremented at the acquisition that
   already happens (`:162-163`).
   The eviction listener currently *removes* the entry (`:90`); instead it **tombstones**
   (`evicted = true`) so undrained hits survive. This keeps the "one lock" property that a
   separate accumulator would break — but tombstones share the map that `enforce_byte_bound`
   scans, so three rules are mandatory:

   - **Victim selection must skip tombstones.** `enforce_byte_bound` picks
     `residents.iter().min_by_key(tick)` (`:179-186`). A tombstone is not in the cache, so
     `cache.invalidate(victim)` is a no-op, `resident_bytes` never drops, and the loop spins
     forever while holding `prune_sem` **on the request hot path** (`:164`). Filter
     `!evicted` when selecting, and `break` when no live candidate remains.
   - **Do not stamp `tick = 0` on a tombstone.** Prewarm also uses tick 0 (step 7); sharing the
     sentinel makes the two indistinguishable in `min_by_key`. Leave the tick untouched and
     rely on the `evicted` flag alone.
   - **Define resurrection.** If `get_or_build` reloads a tombstoned id, the insert at `:163`
     must *merge* (`evicted = false`, fresh tick, **`hits` preserved**), not overwrite — an
     unconditional `insert` would drop exactly the undrained counts the tombstone exists to
     protect. Symmetrically, the flush must not blindly remove-by-key after the DB write:
     re-acquire the lock and, in that **single** acquisition, remove only rows that are still
     tombstoned **and fully drained** (`hits == 0` after subtracting what committed) — the
     check and the removal under one guard closes the resurrect-then-re-evict race between
     snapshot and cleanup, and the `hits == 0` condition keeps counts accrued during a brief
     resurrection. (Re-acquire, don't hold the snapshot guard across the SQLite write — see
     the lock-ordering rule below.) A blind remove after a resurrection deletes the live
     entry's meta, which both stops its hits accruing and makes it unselectable as a
     byte-bound victim (silently weakening the byte bound to moka's count ceiling and TTL).

   **Lock-ordering rule:** never hold the `residents` guard across a SQLite write — copy out,
   drop the guard, then write. The listener runs inside `run_pending_tasks()` on the request
   hot path (`:175,192`), so coupling it to DB latency would put request latency behind disk.
3. **Durable store, owned by migrations.** Add
   `m000003_translation_popularity.rs`, after W3b's reserved
   `m000002_rate_limit_state.rs` — **not** raw DDL at boot.

   ```sql
   CREATE TABLE translation_popularity (
     id TEXT PRIMARY KEY, score REAL NOT NULL DEFAULT 0,
     hits_total INTEGER NOT NULL DEFAULT 0,
     last_hit_at INTEGER, updated_at INTEGER NOT NULL
   );
   ```

4. **Flush wiring — this is not free, budget for it.** The existing rate-limit flush is spawned
   at `main.rs:136`; the pool is built at `main.rs:459-492`, so the task closure cannot capture
   a pool that does not exist yet. Move the spawn below pool construction and pass a third
   clone (note `sea_db` is moved into `AppState` at `:495`, so clone before that).
   Only the spawn moves — `restore()` is a separate statement at `:135` and stays put, so
   restore-before-serve ordering is unaffected. Nothing between `:136` and `:492` depends on
   the flush task running (only `gate_store` clones at `:171`; no requests are served yet).
   Run popularity on every 6th tick (≈60 s) of the existing 10 s task rather than spawning a
   second task — a second flush task on the one shared SeaORM connection adds write
   amplification and cuts against invariant 6.
5. **Snapshot, don't drain; commit as one transaction.** Copy counts out, drop the residents
   guard, then execute one SQLite transaction for all rows. Subtract the snapshot only after the
   transaction commits. A failed transaction leaves every count intact; per-row autocommits on
   the single shared connection are forbidden.
6. **Persist one precise score invariant.** `score` is the value at `updated_at`. On a committed
   flush at `now`, compute in Rust:
   `score_now = score * 0.5f64.powf((now - updated_at) / HALF_LIFE_SECS) + committed_hits`, then
   set `updated_at = now`, increment `hits_total`, and set `last_hit_at = now` only when
   `committed_hits > 0`. Clamp negative elapsed time to zero. Reads apply the same decay from
   `updated_at` to read time. This prevents one new hit from making an old score fresh again.

   `pow()` / `exp()` / `ln()` **do not exist in this SQLite build** —
   `sqlx-sqlite` bundles `libsqlite3-sys 0.30.1` without `SQLITE_ENABLE_MATH_FUNCTIONS`, so
   `ORDER BY score * pow(...)` fails at runtime with `no such function: pow`. The table is
   ≤115 rows, so `SELECT` all and decay in Rust:
   `decayed = score * 0.5f64.powf(elapsed_secs / HALF_LIFE_SECS)`, `HALF_LIFE = 7 days`
   (const, not env). Decay **at read**, so rows that stopped being hit don't keep a frozen
   score and outrank recently-popular ones forever.
7. **Prewarm — bounded, non-self-reinforcing, yielding.** `QURAN_PREWARM_TRANSLATIONS` defaults
   to `2`; `0` disables both collection and prewarm. Background `tokio::spawn` after pool
   construction:
   - `N = min(QURAN_PREWARM_TRANSLATIONS (default 2), max_resident_translations)`;
   - **must not increment `hits`** (else prewarmed ids gain score every boot with zero traffic
     and the top-N freezes) **nor `lookups`** (`:131` — it would corrupt `hit_rate`);
   - **must stamp `tick = 0`** — `enforce_byte_bound` evicts the lowest tick (`:179-186`), so a
     fresh prewarm tick would evict a corpus a real user is reading;
   - the post-build merge may set `tick = 0` only when metadata is absent or tombstoned; a real
     request racing the warm must keep its newer tick and hit count;
   - **must yield** while `build_sem.available_permits() < BUILD_CONCURRENCY`: each load holds
     one of two permits (`:149-152`) and each iteration takes `prune_sem` and runs
     `run_pending_tasks` twice;
   - **must not block or fail boot.** Arabic boot is fail-fast; translations are not.
   Implement as a private `warm(id)` path that bypasses the metric increments rather than
   calling `get_or_build` directly.
8. **Keep/drop threshold with collectable evidence.** `time_to_idle` defaults to 1800 s
   (`settings.rs:162`), so prewarmed entries evaporate 30 min after boot unless real traffic
   arrives — in which case the first request would have built them anyway. Net value: "first
   request after each restart is fast, for N translations, for 30 minutes."
   Keep prewarm only if `cold_builds_within_30m_of_boot / total_cold_builds >= 0.20` over at
   least seven complete daily windows. Emit both counters through OTLP with a boot identifier;
   do not infer them from the reset-on-restart health counter. If the threshold fails, remove
   prewarm, collection, and the table together.
9. **Observability contract.** Add `prewarmed: Vec<String>` and
   `top_demand: Vec<(String, f64)>` (cap 10) to `PoolStats`, `TranslationPoolHealth`, its OpenAPI
   shape, and `/quran/health/ready`. This drops `PoolStats: Copy`; keep `Clone`. Change
   `hit_rate` in both pool and DTO to `Option<f64>` serialized as `null` when `lookups == 0`,
   instead of today's misleading `1.0`.

**Invariants:** eviction semantics unchanged; no new lock; no second flush task; no Quran-DB
writes (`translation_popularity` lives in the app DB, `sqlite:./data/easyquran.db`, entirely
separate from `db/quran/tanzil/**` — the immutability rule is not touched).

**Tests:** decay halves across one half-life; a 30-day-old score plus one hit does not become a
fresh undiminished score; ordering prefers a recent lower score over an old high one; failed
transaction leaves every count intact; eviction tombstones rather than dropping hits;
prewarm clamps to `max_resident_translations`, increments neither `hits` nor `lookups`, stamps
tick 0; prewarm on a bogus id warns without panicking; existing pool tests pass unchanged.

**Done when:** restart warms top-N by decayed API demand, metrics can evaluate the keep/drop
threshold, `/quran/health/ready` reports the explicit nullable contract, and no eviction or
memory-bound behaviour changed.

---

## W2 — Production client-IP correctness (Rust)

**Goal:** external rate limiting keys on the Cloudflare client IP while trusted Docker-internal
SSR and health traffic use separate, bounded identities.
**Files:** `config/settings.rs`, `middlewares/{route_blocker.rs,client_ip.rs}`, `state.rs`,
`main.rs`, `web/src/lib/server/quran-translation-page.ts`, `deploy/.env.example`,
`docker-compose.yml`, `deploy/README.md`, `web/scripts/assert-headers.sh`.

**Today:** `settings.rs:98` defaults to `ConnectInfo` and `deploy/.env.example:42` ships
`IP_SOURCE=ConnectInfo` **uncommented**. Behind CF→Traefik every request carries the proxy's
IP, so per-IP limiting collapses into one global bucket. **The bug is set-but-wrong, not
unset** — a guard firing only on "unset" catches nothing.

### Steps

1. **One `is_production()` helper.** Three spellings exist today: `state.rs:107-114` reads
   `RUST_ENV → NODE_ENV → APP_ENV`; `route_blocker.rs:69` reads **only** `APP_ENV`, defaulting
   to `"development"`; `telemetry.rs:101` reads `DEPLOYMENT_ENVIRONMENT`. Since
   `deploy/.env.example:36` sets `RUST_ENV=production` and never `APP_ENV`, **the route blocker
   is disabled in production right now. Convert `route_blocker` to the shared helper.
2. `HttpSettings::from_env` returns `Result`, never panics on `IP_SOURCE`. Define environment
   precedence once: `RUST_ENV → NODE_ENV → APP_ENV`; production is `production`, accepted
   non-production values are `development|dev|test|testing|ci|local`, and unset/unknown values
   are configuration errors outside tests.
3. **Three identities, resolved before rate limiting:**
   - external request: require `CF-Connecting-IP` in production and use it;
   - Bun SSR request: require a constant-time match on server-only
     `X-EasyQuran-Internal-Token`, assign `service:web-ssr`, and use a separate non-escalating
     limiter configured by `QURAN_INTERNAL_REQUESTS_PER_MINUTE`;
   - `/healthz` and `/quran/health/ready`: mount outside every content limiter and never require
     a client-IP header.
   Missing/invalid internal token never grants internal treatment. Missing CF header on an
   external production request is rejected, never collapsed into `unknown`.
4. Generate `INTERNAL_QURAN_API_TOKEN` per deployment. Bun reads it from private runtime env and
   sends it only to `INTERNAL_QURAN_API_BASE`; it never enters a public env variable, response,
   or log. Compose passes it to web and API containers.
5. Production boot with `ConnectInfo`, missing internal token, or a non-positive internal limit
   returns a configuration error and exits non-zero. Log selected external source and internal
   policy without logging the token.
6. Set `deploy/.env.example` to `IP_SOURCE=CfConnectingIp` — **exact PascalCase**;
   `cf-connecting-ip` is invalid. `docker-compose.yml` passes the environment to the API through
   `env_file: [.env]`.

### Trust, not just parsing

`CfConnectingIp` trusts the header. Before this configuration reaches production, Traefik or the
host firewall must restrict public origin ingress to current Cloudflare ranges.
`deploy/README.md` owns the range-update procedure and direct-origin negative test.
`assert-headers.sh` proves that a public caller cannot choose `CF-Connecting-IP`, an external
request without the header is rejected at origin, internal SSR succeeds only with the token,
and health works without either header. W3a remains disabled until this contract passes.

**Tests:** serialized env tests for production/dev/unset/unknown values; prod + `ConnectInfo` →
`Err`; prod + `CfConnectingIp` → `Ok`; dev + `ConnectInfo` → `Ok`; route blocker enabled under
`RUST_ENV=production`; external missing/spoofed CF header rejected; valid internal token gets
the service bucket; invalid token gets no privilege; internal and external buckets are isolated;
health never enters either bucket.

**Done when:** production boots only with the complete ingress contract, route blocker is live,
external requests carry verified client identities, internal SSR has its own bounded identity,
and health traffic cannot enter a content or escalation bucket.

---

## W3 — Ban escalation, persistence, inspection, and export (Rust)

**Goal:** `my-plan-raw.md:7` — persist longer IP bans, make them operable, and expose a safe
machine-readable feed for a deployment-owned proxy adapter.
**Files:** `crates/rux-request-gate/src/{layer.rs,store.rs,abuse.rs}`,
`services/rate_limit_store.rs`, `migration/src/m000002_rate_limit_state.rs`,
`modules/admin_bans_v1/` (new), `modules/quran_v1/error.rs`, `config/settings.rs`, `main.rs`,
`deploy/.env.example`, `deploy/README.md`.

**Today:** Quran routes have fixed-window limiting only. Existing Temp/Long abuse buckets lose
scope on restart, no side-effect-free ban lookup exists, `RateLimitStore::del` removes dedup
claims rather than limit buckets, and no operator API exists.

**Dependency order:** W3b → W3c → W3a. W3a stays disabled until W2 and W3c are complete.

### W3b — Migration-owned persistence

1. Add `block_scope` to `BucketSnapshot`, snapshot, flush, load, and restore. Unknown stored
   values fail closed as `Temp` and emit a warning; valid values round-trip exactly.
2. Add reserved migration `m000002_rate_limit_state.rs`. If `rate_limit_state` is absent, create
   the five-column table. If it already exists, inspect
   `PRAGMA table_info(rate_limit_state)` and conditionally add
   `block_scope TEXT NOT NULL DEFAULT 'Temp'`. Register the migration before replacing the
   boot-owned table creation, and keep migration-before-`load()` ordering.
3. Introduce one async persistence-operation lock shared by periodic snapshot/flush and admin
   mutations. Never hold the in-memory bucket mutex during SQLite I/O.
4. Expired rows are deleted on the next successful flush; active keys remain only until their
   fixed or block expiry. This retention rule covers IP, email, and user-id key classes. Only
   successfully parsed active IP ban keys may appear in an IP export.

**Tests:** fresh migration; legacy table with rows gains scope without data loss; migration is
idempotent; `Long` survives restart; invalid scope restores as `Temp`; expired rows purge.
**Done when:** fresh and existing installs share one migration-owned schema and scope survives
restart without changing Quran DBs.

### W3c — Inspect, export, and lift bans

1. Add distinct store operations: `ban_status(key)` performs a read-only active-ban lookup;
   `clear_limit(key)` removes a limit bucket; dedup `del(key)` keeps its current claim semantics.
2. `GET /admin/bans` is admin-ACL gated, `no-store`, paginated, and returns active
   `{ip, scope, blockUntilAt}` rows only.
3. `DELETE /admin/bans/{ip}` acquires the persistence-operation lock, clears exact
   `quran-ban:{ip}` and `ratelimit:{ip}:quran-v1` L1 buckets, deletes their L2 rows, then releases
   the lock. This prevents stale periodic snapshots from resurrecting a lifted ban and prevents
   the old fixed count from immediately recreating it. State explicit error behavior: any DB
   failure returns failure and leaves a retryable, logged operation.
4. `GET /admin/bans/export` returns neutral JSON containing active parsed IPs, scope, and expiry.
   Human access uses admin ACL. Machine access uses a separate read-only `BAN_EXPORT_TOKEN`
   compared in constant time; the token is never accepted by mutation routes. Never export
   `totp:*`, email, user-id, fixed-rate, or unparseable keys.
5. Traefik/Cloudflare mutation stays outside this repository. `deploy/README.md` documents the
   JSON contract and requires any deployment adapter to preserve expiry and un-ban semantics;
   this plan does not claim a proxy block exists merely because an export exists.

**Tests:** list/export exclude email and user-id keys; IPv4/IPv6 round-trip; invalid export token
fails; delete clears L1/L2 and fixed count; barrier-controlled flush/delete race cannot recreate
the row; restart after delete does not restore it.
**Done when:** operator can inspect and lift bans without DB edits and a proxy adapter has a
stable, non-PII-leaking feed.

### W3a — Escalate suspicious repeated rate blocks (default off)

`on_block` remains a synchronous response hook and is not used for store mutation. Escalation
runs in `RateLimitMiddleware::call`, where resolved identity and store are available.

1. Add explicit config:
   ```rust
   struct EscalationConfig {
     enabled: bool,                    // QURAN_BAN_ESCALATION_ENABLED, default false
     key_prefix: &'static str,         // "quran-ban"
     temp_after: u32,                  // 5 qualifying blocked windows
     temp_window_secs: u64,            // 3600
     temp_duration_secs: u64,          // 3600
     long_after: u32,                  // 20 qualifying blocked windows
     long_window_secs: u64,            // 86400
     long_duration_secs: u64,          // 604800
     suspicious_4xx_per_window: u32,   // 20
     allowlist: Vec<IpNet>,
   }
   ```
   Parse `QURAN_BAN_ALLOWLIST` as CIDRs at boot; enabled + missing/invalid allowlist is a boot
   error. Internal service identities and health routes can never enter this state machine.
2. Before fixed-window increment, call side-effect-free `ban_status(quran-ban:{ip})`. An active
   Temp/Long ban immediately returns 429 for every request, including after fixed-window rollover.
3. Record suspicious Quran 4xx events in a bounded per-IP/window counter for a closed set:
   unknown source id, invalid range bounds, and unknown Quran route. Exclude normal search
   validation, ordinary not-found content, 5xx, and successful reads.
4. A rate window qualifies only when `count == max_requests + 1` **and** the suspicious counter
   meets its threshold. That equality is the one-event-per-window primitive; do not use
   `dedup_nx` or append on every 429. Raw volume alone never creates a ban.
5. `record_block_event` prunes old qualifying events, evaluates Long before Temp, and upgrades
   to Long when the long threshold is met. Active-ban lookup never appends events. Escalation
   history is L1-only; active ban scope/expiry is L2-persistent.
6. Wire only the outer `quran-v1` branch limiter. Search remains under its own 30/min limiter and
   the outer 600/min ceiling but contributes at most one outer qualifying event per window.
7. Prune expired `claims` in the existing store as W10.5; escalation itself does not use claims.

**Hard gates:** W2 ingress contract · W3b migration · W3c un-ban · valid CIDR allowlist · exact
key namespace · default-off flag.

**Tests:** flag off preserves existing behavior; active ban blocks request 1 after rate-window
rollover and after restart; volume without suspicious shape never bans; qualifying windows reach
Temp then Long; Long wins when both thresholds match; active Temp can later upgrade after new
qualifying windows; flood stores one event per window; allowlisted/internal identities never ban.

**Done when:** suspicious sustained abuse creates enforceable persistent bans, raw legitimate
volume cannot ban a CGNAT range, and every ban is visible and safely removable.

---

## W4 — Engagement: one-time viewer vs continuous reader (Web)

**Goal:** `my-plan-raw.md:21`.
**Files:** `lib/quran/engagement.ts`, `lib/quran/engagement-state.ts` (new),
`lib/workers/download.ts`, engagement tests.

**Today** (`engagement.ts:15-20,83`): one `sessionStorage` integer, threshold 2. Resets every
tab, so a daily reader never accumulates while two clicks in one tab reads as "continuous".
Arabic views inflate the counter toward a *translation* download (`:68-70`).

### Design

- **`readerViews`** (all sources, `localStorage`): is this person a reader at all?
- **`sourceViews[id]`** (`localStorage`): has this person used *this* translation?

```ts
interface EngagementState {           // localStorage "eq:engagement"
  v: 1; totalViews: number; distinctDays: number;
  lastDay: string;                    // YYYY-MM-DD local
  firstSeen: number; lastSeen: number;
  sourceViews: Record<string, number>;
}
isEngagedReader() = distinctDays >= 2 || totalViews >= 4 || sessionViews >= 3;
```

`sessionViews` lives under `sessionStorage "eq:reader-session-views"`. The legacy
`eq:reader-views` key is read once during migration, seeds both `totalViews` and
`sessionViews`, then is deleted. A `localStorage "eq:engagement-migrated-v1"` marker makes the
durable seed idempotent while the session counter continues normally.

**Gate:** `isEngagedReader() && preBumpSourceViews[id] >= 1` — explicitly the **pre-bump**
value; reading post-bump would make it always true on first exposure and the second signal
would do nothing.

### Steps

1. New `lib/quran/engagement-state.ts` — load/save/bump. **Never read storage at module
   scope**: `readJSON` no-ops under SSR/prerender (`safe-storage.ts:4`), so a module-scope read
   would freeze `undefined` into the bundle. Read lazily inside the call.
2. **Validate the shape explicitly.** `isFutureSchema` returns `false` for an object with no
   `v` key (`safe-storage.ts:56-59`), so a legacy or garbage blob passes as valid current
   schema. Check field types.
3. **Keep the read-modify-write synchronous** — no `await` between `readJSON` and `writeJSON`,
   re-read immediately before writing. `localStorage` RMW is non-atomic across tabs; the
   current sessionStorage design has no such hazard, so this migration introduces it. (Do not
   add a `storage`-event sync; last-writer-wins on a view counter is acceptable.)
4. **Monotonic day guard:** increment `distinctDays` only when `today > lastDay`
   lexicographically. DST is harmless; a clock set forward then back would otherwise
   double-count.
5. `sessionViews` uses the new session key above. Migration order is: read validated durable
   state → read legacy session value → seed durable total once if marker absent → initialize new
   session counter → write marker → delete legacy key.
6. Bump `readerViews` always; `sourceViews[id]` only for translation sources.
7. Keep `PREFETCH_PREFIX` settle markers in `sessionStorage` and every existing guard:
   `saveData`/2g (`:27-32`), `whenIdle` (`:34-38`), one retry per source per tab (`:45-50`).
8. `noteTranslationChosen` (`:96`) keeps bypassing the gate — an explicit pick beats a heuristic.
9. **Migration is low-stakes and atomic within one synchronous call.** The legacy
   `eq:reader-views` counter is **sessionStorage** (`engagement.ts:6,16-18`) — per-tab,
   dies with the tab — so there is no durable history to protect. Seed `totalViews` from it
   if present and delete it in the same call. Settle markers (`eq:tprefetch:*`) are untouched.
10. **Give `downloadBytes` a full-transfer timeout and byte ceiling.** One AbortController stays
    active through headers and complete body streaming, not only until `fetch()` resolves. Abort
    when elapsed budget expires or bytes exceed declared `sizeBytes`; always cancel reader and
    clear timer. One retry remains governed by existing per-source session logic.

**Do not** write a "`readJSON` throws" test — `safe-storage.ts:3-11` catches internally and
returns `undefined`. Test the `undefined` path.

**Tests:** one view/one day → no prefetch; four views same day → prefetch; two views across two
distinct days → prefetch; Arabic-only views never prefetch; `readJSON` → `undefined` falls back
to session-only without throwing; legacy key seeds `totalViews` once; gate
reads pre-bump `sourceViews`; `downloadBytes` aborts on timeout.
Repeat-load migration seeds durable history once while the new session counter continues; an
oversized stream aborts before unbounded allocation.

**Done when:** a reader returning on a second day gets the translation prefetched, a one-time
visitor does not, and no code path can hang forever on a download.

---

## W5 — Unify and repair the fallback chain (Web)

**Goal:** `my-plan-raw.md:14` — "fall back to API first".
**Files:** `lib/quran/{worker-client.ts,api-client.ts,fetch.ts,range-fetch.ts}`,
`lib/server/quran-translation-page.ts`, `app/_reader/SurahReader.svelte`,
`src/service-worker.ts`, browser + server loader tests.

**Today:** translations get an ordered chain (`worker-client.ts:108-156`); Arabic gets ad-hoc
early-returns — `readSurah:259-269` hits the API only if the worker was never constructed;
`readRange:286-291` is worker-only with no fallback. Translated range SSR bypasses
`quranApi.readRange` and sends one request from `quran-translation-page.ts:32-49`, so juz
19/23/27/29/30 exceed the backend cap and degrade to empty data even while the API is healthy.

### API cache boundary

**The SW `/quran/` bypass is already dead in production.** `service-worker.ts:252` matches
`/quran/`, but prod `PUBLIC_QURAN_API_BASE=https://easyquran.fyi/api/quran`
(`deploy/.env.example:20`) → real paths are `/api/quran/…`, which falls past all five bypasses
into `swrApp()` and lands in **`eq-app-${version}`**. Quran API JSON is being cached there
today. Add same-origin `/api/` to the bypass list before adding fallback traffic. API response
caching belongs to HTTP/edge policy, never `eq-app-*` or `eq-data-v1`.

### Steps

1. Generalise `withTranslationFallback` into `withSourceFallback` with a `hasLocal` probe:
   translations → `hasTranslation(id)`; Arabic → worker readiness. `quranWorker.ready` is a
   synchronous getter, so type the probe `() => boolean | Promise<boolean>`.
2. Preserve tier diagnostics even when fallback succeeds. Each read records
   `{servedBy: "local"|"api", workerFailure?: ReadFailure, apiFailure?: ReadFailure}` through a
   typed status callback. `ReadFailure` distinguishes timeout, transport, HTTP, malformed data,
   and worker failure without exposing URLs or response bodies. Final failure retains both tier
   causes rather than only `lastErr`.
3. **Fix the boot window.** When worker startup has begun, wait up to
   `LOCAL_BOOT_BUDGET_MS = 1500` for readiness before choosing API. Merely checking
   `startPromise !== null || worker !== null` is insufficient because the worker rejects reads
   until initialization completes. Timeout falls through to API; worker readiness within budget
   prevents network use when local DB already exists.
4. **One shared range fetcher for browser and SSR.** `range-fetch.ts` accepts base URL, source,
   bounds, validator, and fetch implementation. Both `quranApi.readRange` and
   `fetchTranslationRange` use it, so no server path can bypass chunking.
5. Chunk reads to ≤300 ayahs (`RESPONSE_CAP = 300`, enforced at
   `controller.rs:1237-1239`).
   **Five juz exceed the cap — 19 (339), 23 (357), 27 (399), 29 (431), 30 (564)** — computed
   from the juz start table in `web/static/quran-meta/quran-data.json`. Global pages max at 42,
   so they are safe. Max chunk count is 2; every chunk is at most 300.
6. **Stitch exact complete results.** Per chunk: apply coordinate validator; require first/last
   indices to equal requested chunk bounds; require exact count. Across chunks: require adjacency;
   concatenate ayahs; merge normalizations by surah; accept byte-equivalent duplicates created
   by a split inside one surah and reject conflicting duplicates. Final result must cover
   `[from,to]` exactly. Any failure rejects the whole read; partial renders are forbidden.
7. `fetchJsonWithTimeout` owns AbortController through headers **and complete body decoding**.
   Range chunks use `RANGE_CHUNK_TIMEOUT_MS = 10_000`; timeout cancels body consumption. The
   30-second worker request timeout remains unrelated.
8. **Thread `validateCoordinate` through `quranApi`.** `api-client.ts` takes no validator, so
   the API path decodes without the check the worker path performs (`worker-client.ts:288`,
   `SurahReader.svelte:417-419`). W5/W6 make the API path routine, silently weakening the
   invariant.
9. **Remove both reader gates.** Neither `quran.status === "error"` nor
   `!quranWorker.ready` may prevent a read. Worker status drives UI only; `loadPage` always calls
   the fallback chain. A successful API read clears network degradation but may leave worker
   degradation visible.
10. Resulting order for both source kinds: `local → API → local re-check → typed failure`.

**Invariant:** a ready worker always beats the API; the API is strictly a fallback.

**Tests:** worker fatal/store status error + API healthy → API result; worker healthy → API never
called; worker becomes ready inside boot budget → no network; boot budget expires → API; all
five oversized juz work through browser and translated server loaders; split-inside-surah merges
normalization; conflicting duplicate/truncated/wrong-boundary chunk rejects; chunk failure → no
partial result; timeout covers stalled body; API path receives validator; `/api/` never enters
any Cache Storage bucket; translation surah behavior unchanged.

**Done when:** Arabic and translation browser reads share one fallback ladder, browser and SSR
share one exact chunk contract, oversized juz return complete data, tier failures stay
observable, and API responses never enter service-worker caches.

---

## W6 — Client data path for range routes (Web)

**Goal:** upgrade range routes from server first paint to exact local/API data without blanking,
mixing route state, or allowing unbounded SvelteKit data cache growth.
**Files:** `app/_reader/RangeReader.svelte`, `lib/quran/worker-client.ts`,
`src/service-worker.ts`, `app/__tests__/nav-guard.test.ts`, component + service-worker tests.

**Today:** `RangeReader` renders server `data` and nothing else — no worker read, no
`onStatus`.

**Scope:** the component serves **four** routes — `app/juz/[n]`, `app/page/[n]`
(`prerender = true`) and `app/t/[lang]/[translator]/juz/[n]`,
`app/t/[lang]/[translator]/page/[n]` (**`prerender = false`**). Both source kinds must be
designed for, and the prerender flag differs between them.

### Mechanism: component-level post-paint swap

Keeping `+page.server.ts` means SvelteKit resolves `__data.json` before new route props reach the
component. A universal `+page.ts` does not remove that dependency: it receives server data and
runs after the server node resolves. Therefore this workstream is a post-paint data upgrade, not
network-free warm navigation. Removing that dependency requires a separate route architecture;
W9 records the divergence from `my-plan-raw.md:13,22` explicitly.

For Arabic this costs little: the route is prerendered, so `__data.json` is a **static build
artifact** served from the CDN/`eq-data-v1`, not an API call.

### Steps

1. Define route key `{sourceId, kind, index}` and one `$state.raw` display snapshot containing
   `{ayahs, normalizations, surahs}`. These fields always change atomically; never combine client
   ayahs with server normalization or metadata.
2. When props change, immediately install the matching server snapshot, capture its route key,
   then start `quranWorker.readRange` through W5 **without a worker-ready gate**. Install the
   result only if captured key still equals current key. A slow result for an old route is
   discarded.
3. When first paint is complete, skip duplicate client read. When first paint is degraded
   (`ayahs=[]`) or a later navigation occurs, run the chain. Worker readiness events may retry
   only the current degraded key.
4. Derive canonical surah metadata from `loadQuranData()` for every represented surah; never use
   a non-null assertion against possibly empty `data.surahs`.
5. Arabic total failure keeps matching server snapshot. Translation worker failure reaches API;
   total failure keeps matching server snapshot and typed degradation state. No path blanks or
   restores data from a previous route.
6. Compute ranges from `loadQuranData()` and preserve translation context on every reader link:
   use `juzPathFor`/`globalPagePathFor` and the `surah*For(ctx, ...)` family with `ctx` derived
   from `surahRouteContext(sourceId)` or `routeContextFromParams(page.params)`. Components never
   call Arabic-only path helpers or hand-build `/app/` navigation strings.
7. Bound `eq-data-v1` with count `DATA_MAX = 400` and
   `DATA_BUDGET_BYTES = 32 * 1024 * 1024`. Track data-cache recency separately in IDB, touch on
   hit/write, and evict least-recent entries until both bounds hold. Offline-pack entries live in
   their separate versioned cache and do not count against this budget.
8. Extend `nav-guard.test.ts` to reject component calls to Arabic-only reader path helpers and
   navigational `/app/` literals used by links, `goto`, or `resolve`. Add translated range-route
   fixtures that prove the active source context survives page, juz, surah, and ayah navigation.

**Tests:** complete first paint does not duplicate-read; empty translated SSR recovers complete
ayahs/normalizations/surah metadata; worker unavailable reaches API; Arabic total failure keeps
server data; delayed old result after index/source navigation is discarded; juz 30 works on all
paths; data cache enforces count and byte caps in true LRU order; offline pack untouched;
translation-context navigation assertions and `nav-guard.test.ts` pass.

**Done when:** range-route props upgrade to matching client data without mixed metadata, stale
results, blanks, or unbounded `eq-data-v1`; remaining server-load dependency is documented rather
than presented as network-free SPA navigation.

---

## W7 — Pagination-aware degradation and recovery (Web)

**Goal:** implement API-first recovery and Arabic static-data fallback without document reloads;
state the translated-route divergence from `my-plan-raw.md:22` exactly.
**Files:** `src/service-worker.ts`, `hooks.server.ts`,
`app/_reader/SurahReader.svelte`, reader status UI, server-loader and SW tests.

### Delivery contract

- Arabic reader routes are prerendered. Their `__data.json` is a static build artifact, so a
  SvelteKit data load already provides the Arabic SSG-last tier while the static origin is up.
  Offline pack supplies the same data keys where staged. No document reload or second
  prerendered-cache mechanism is added.
- Translated routes are SSR + bounded disk cache and have **no SSG artifact**. Prerendering all
  115 translations would create at least 13,110 surah routes before local-page/range routes.
  Translation recovery is local DB → API → matching server data; it is never called SSG-last.
  W9 records this deliberate divergence.
- Navigation/reload is not a fallback. Reload discards the worker, re-pays the ~2.4 MB Arabic
  cold cost, and still cannot manufacture a translated SSG artifact.

### Steps

1. A translated server load that returns empty pending data sets
   `X-EQ-Translation-Pending: 1` and `Cache-Control: no-store` on both document and
   `__data.json` responses. `hooks.server.ts` preserves `no-store` instead of overwriting it.
2. Service worker treats `no-store` **or** `X-EQ-Translation-Pending` as uncacheable and deletes
   any existing matching `eq-pages-v1`/`eq-data-v1` entry. API recovery must make the next retry
   observe fresh data, never a cached empty 200.
3. Multi-page surah failure keeps the current page, shows an inline retry, and retries the
   missing page through W5. Single-page empty server data first performs one SvelteKit
   `invalidateAll()` retry, then uses W5 for the same page if server data remains empty. Neither
   branch performs document navigation.
4. Consume W5 typed outcomes. Track worker and API degradation independently:
   - worker failure + API success: content renders; local-offline warning remains;
   - worker success: clear worker warning;
   - API failure + matching server/worker content: content remains; network warning shows;
   - any successful API read clears network warning.
   Never infer API health from `navigator.onLine`.
5. Retry operations are single-flight per route/page. Every degraded state has an explicit
   success-clearing rule; route-key changes discard old state.

**Tests:** pending document/data response is `no-store`; SW evicts old pending key; API outage →
empty pending → API recovery → first retry returns content; multi-page failure retries in-page;
single-page failure invalidates once then uses W5; no document navigation; worker-down/API-up,
worker-up/API-down, both-down, and recovery produce distinct clearable states.

**Done when:** cached failure cannot survive recovery, worker/API failures remain distinct,
pagination behavior never reloads the document, Arabic static fallback is truthful, and no
translated route is described as SSG.

---

## W8 — Web authentication and cache isolation (Web + Rust)

**Goal:** client-hydrated web authentication for email/password, OAuth, passkeys, and account
management without placing user-specific content in shared server or browser caches.
**Files:** `deploy/.env.example`, `docker-compose.yml`, `src/service-worker.ts`,
`web/src/hooks.server.ts`, new `web/src/lib/auth/`, new auth/account routes,
`middlewares/{security_headers.rs,cors.rs}`, `utils/cors.rs`, existing Rust auth/user/OAuth/
passkey modules.

### W8a — Origin and cache prerequisites

1. **Set and validate `ALLOWED_ORIGINS`.** The private router runs `origin_guard`
   (`main.rs:663`), which rejects any request whose `Origin` is not allowlisted
   (`middlewares/cors.rs:20-28`; list at `utils/cors.rs` = localhost + `hmziq.rs` +
   `hzmiqrs.com` + `blog.hmziq.rs`). `easyquran.fyi` is **not** among them and the variable
   appears in neither `deploy/.env.example` nor `docker-compose.yml`. Same-origin POSTs still
   send `Origin`, so **every login and register would 403.** Add
   `ALLOWED_ORIGINS=https://${DOMAIN}` to both.
   **Sharp edge:** `utils/cors.rs` does `.parse::<HeaderValue>().unwrap()` per entry. Entries
   are trimmed first and `HeaderValue` accepts any byte in `0x20..0x7E` plus tab, so spaces and
   trailing commas do **not** panic — a trailing comma yields an empty, silently useless entry,
   which is the more likely failure. What *does* panic at boot is any control byte (a newline
   from a copy-paste) or any non-ASCII byte (a smart quote, NBSP). Validate for both: reject
   empty elements loudly, and reject non-ASCII instead of unwrapping.
2. W5's `/api/` SW bypass is a hard dependency. No authenticated API response may enter Cache
   Storage.
3. **Authenticated API `no-store` without harming public Quran caching.** Capture before
   `next.run`: request contains exact parsed cookie `ruxlog.sid`. After `next`, also inspect
   `Set-Cookie` for that exact name. Either condition sets `Cache-Control: private, no-store`;
   otherwise preserve handler cache headers unchanged. Never overwrite anonymous Quran public
   cache policy.
4. **Authenticated web document/data isolation.** `translationRouteCacheKey` keys on
   `(sourceId, kind, index)` only and `cacheable` never inspects cookies
   (`hooks.server.ts:11-27,104-105`), while the cached artifact is the **full HTML document
   including the app shell**. Cookie-bearing or session-setting web responses are never read
   from/written to SSR disk cache and receive `Cache-Control: private, no-store`. This includes
   SvelteKit documents and `__data.json`.
5. On successful login, logout, or account switch, send an explicit SW message that deletes
   `eq-pages-v1` and `eq-data-v1`. OPFS Quran DBs and versioned offline pack remain untouched.
   SW also refuses to cache any request/response marked `private` or `no-store`.

**Tests:** production Origin passes; malformed origin config returns boot error; anonymous Quran
headers remain public; cookie-bearing and session-setting API/document/data responses are
private no-store; SSR disk cache never contains user token; login/logout purge page/data caches;
OPFS/offline pack remain.

### W8b — Session and CSRF foundation

1. Add `auth-client.ts` as the only web API wrapper for auth. Requests use credentials and the
   same-origin `/api` base; errors decode existing API envelopes without treating 401 as a crash.
2. Add request-scoped client state in `auth-state.svelte.ts`: `unknown | anonymous |
   authenticated`, user profile, verification/2FA state, and one in-flight session probe.
   Nothing reads browser storage or session at module scope during SSR.
3. Hydrate after mount with `GET /api/user/v1/get`; 200 authenticates, 401/403 becomes anonymous,
   transport/5xx remains retryable unknown. `/app/**` stays prerendered and never embeds user
   data in build output.
4. Fetch CSRF through `POST /api/csrf/v1/generate`. Token stays in memory, never localStorage.
   Re-fetch after login, logout, OAuth/passkey login, session termination affecting current
   session, or any response that rotates `ruxlog.sid`, because token is HMAC(session id).
5. Auth transitions invoke W8a cache purge before new user state renders.

### W8c — Email/password, verification, 2FA

1. Build login and registration UI over `/api/auth/v1/log_in` and `/api/auth/v1/register`, with
   field errors, pending state, keyboard/focus behavior, and uniform credential failure copy.
2. Support TOTP continuation through `/api/auth/v1/login/totp`; do not mark session authenticated
   before continuation succeeds.
3. Add email-verification request/confirm flow through
   `/api/email_verification/v1/resend` and `/api/email_verification/v1/verify`. Verified-only
   account actions explain the required next step instead of looping on 403.
4. Logout posts `/api/auth/v1/log_out`, clears in-memory CSRF/auth data, purges SW caches, then
   probes session once to confirm anonymous state.

### W8d — OAuth and passkeys

1. Web OAuth supports Google, Apple, Facebook, and GitHub through existing
   `/api/auth/{provider}/v1/login|callback|exchange` routes. Mobile `/token` endpoints remain
   outside web flow; GitHub remains web-only.
2. Implement `/auth/{provider}/success`, the exact target used by backend callbacks. Return
   targets are same-origin allowlisted paths, stored transiently, and cleared after use. OAuth
   failure/cancel returns to login with safe error state; success refreshes CSRF, purges caches,
   probes profile, then consumes the return target.
3. Passkey login uses `/api/passkey/v1/login/begin|finish`; verified account registration uses
   `/api/passkey/v1/register/begin|finish`, and management uses
   `/api/passkey/v1/list|remove`. Challenges stay in memory; cancellation is not reported as
   server failure.

### W8e — Account UI and session lifecycle

1. Add prerendered account shell hydrated from W8b. Anonymous users see login CTA; unknown state
   shows neutral loading/retry; authenticated users see profile and security controls.
2. Profile update uses `/api/user/v1/update`; session list/termination uses
   `/api/auth/v1/sessions/list|terminate/{id}`; 2FA/passkey controls use existing verified routes.
3. Terminating current session follows logout transition. Terminating another session refreshes
   list without clearing current auth.
4. No server-rendered user name, email, token, CSRF value, OAuth payload, or passkey challenge may
   enter HTML disk cache, Cache Storage, localStorage, logs, analytics, or error URLs.

**Tests:** session probe states; CSRF refresh after every session transition; email/password +
TOTP flows; all four OAuth success/cancel/error flows; passkey login/register/remove; current vs
other session termination; anonymous prerender contains no user data; account-switch cache
isolation; accessibility and `pnpm check` pass.

**Done when:** every supported backend auth path has a web flow, session transitions are
consistent, CSRF tracks session rotation, and neither server nor browser shared caches can replay
one user's content to another.

---

## W9 — Record the divergences (Docs)

**Files:** `docs/quran-system.md`, `AGENTS.MD`.

`quran-system.md:112` already records "translated pages = SSR on Bun". Record the exact
boundaries that implementations must not blur:

1. **Translated pages are SSR + 7-day disk cache, not SSG.** Prerendering would be
   114 × 115 ≈ 13,110 pages for the surah route alone. SEO is served by hreflang alternates
   (`routes/sitemap.xml/+server.ts:31-42`). This deliberately diverges from
   `my-plan-raw.md:19,22`; no translated path may be described as SSG.
2. **Translated delivery is not surah-only.** The same source context spans surah,
   surah-local-page, global-page, and juz routes so reader navigation cannot fall back to
   Arabic. The sitemap groups those route shapes with language alternates. This is the exact
   runtime boundary behind the shorter intent in `my-plan-raw.md:19`.
3. **"SSG last" holds for Arabic only,** because those routes are prerendered and their
   `__data.json` is a static artifact. Translated routes recover through local DB, API, or
   matching server data only.
4. **The durable pool metric is API demand across process restarts, not reader popularity.**
   Moka TinyLFU already owns in-process popularity admission (W1).
5. **Range routes keep `+page.server.ts`,** so `__data.json` is fetched on client navigation;
   the worker read is an upgrade after paint, not a replacement (W6).
6. **Authentication is client-hydrated.** `/app/**` remains prerendered; user state never enters
   build output, translated HTML disk cache, or Cache Storage (W8).

**Do not edit `my-plan-raw.md`.**
**Done when:** every divergence above is recorded.

---

## W10 — Cross-cutting delivery and cache safety requirements

1. **Remote-controlled artifact URL.** `catalogue.ts:96-113`: a catalogue
   entry with no baked match keeps `t.downloadUrl` unrewritten and the worker fetches it
   verbatim; the `/_quran` allowlist only gates requests that reach `/_quran`.
   **`manifest.ts:37-42` has the identical hole** for Arabic script specs from `/scripts`.
   Accept only ids with an exact baked id→delivery-path mapping and always replace API URLs with
   that baked same-origin path. Unknown ids, path disagreement, non-HTTP schemes, credentials,
   or non-baked origins reject the whole API payload. Existing catch blocks must emit a
   structured warning + OTLP counter before falling back to baked data; rejection may not be
   silent. Tests cover malicious origin, same-origin wrong path, unknown id, and visible fallback.
2. **`downloadBytes` timeout and byte ceiling** — W4 step 10.
3. **Health outside content limiters** — W2.
4. **Bound `eq-data-v1`** (unbounded today; `trimPages` covers `eq-pages` only,
   `service-worker.ts:525-539`). W6 owns explicit count/byte LRU bounds.
5. **Prune `InMemoryStore.claims`.** `prune()` removes expired entries from both `limits` and
   `claims`; add a high-cardinality expiry regression test. This fixes existing dedup retention
   independently of W3 escalation.
6. **Crash-safe OPFS replacement without hashing Quran data.** Download to an id-scoped temporary
   file, close it, verify declared size/content shape, then switch the id's active-file pointer in
   one IDB transaction and remove the old file afterward. Failure keeps old active file and
   removes temp. Identity remains the Quran source id; temp names never become versions or cache
   identities. Tests interrupt download/write/pointer switch and prove old corpus stays readable.

---

## 11. Dependencies and verification

```
W10.1 ─► W4
W3b ─► W3c
W2 + W3c ─► W3a
W5 ─► W6 ─► W7
W5 + W7 ─► W8a ─► W8b ─► W8c/W8d ─► W8e
W10.5
W10.6
W1 (after m000002; owns m000003)
W9
```

**Hard constraints:** W2 ingress proof + W3b + W3c + valid allowlist before W3a. W5 before W6
before W7. W5 API bypass and W7 pending-response rules before auth. `m000002_rate_limit_state`
always precedes `m000003_translation_popularity`.

### Operational requirements

- **Feature flags:** only W3a needs a kill switch because it bans clients. W1 prewarm is disabled
  with `QURAN_PREWARM_TRANSLATIONS=0`. Recovery/cache correctness does not get a second behavior
  branch.
- **Rollback:** W4's legacy key is per-tab sessionStorage — nothing durable to protect (see
  W4 step 9); W3b's column is additive and readable by old binaries.
- **Migration ordering:** boot runs migrations before loading rate-limit/popularity state.
  `m000002` upgrades legacy rows; `m000003` is new-table-only. Test old binary/new schema and new
  binary/legacy fixture behavior. Read `migration/README.md` into each schema change.
- **Partial deploy:** web and API are separate images. W5 must tolerate an older API; W2's
  hard-fail must not land while an older web depends on the API booting. API-first for additive
  changes, web-first for anything relaxing a contract.
- **Observability:** add counters for escalation/bans (W3), API-fallback rate and degraded-mode
  entry (W5/W7), auth failures (W8). There is **no `/metrics` endpoint** — counters leave via
  OTLP push only. Provision the corresponding OTLP dashboard queries with each counter.
- **Verification per workstream:** backend — `cargo fmt --check`, `cargo test`,
  `cargo clippy --all-targets -- -D warnings`; web — `pnpm check`, `pnpm test`, `pnpm lint`,
  `pnpm format:check`. Always include `tests/quran_v1.rs`, `catalogue-sha-guard.test.ts`, and
  `nav-guard.test.ts`. Schema work runs fresh + legacy migration fixtures; cache work runs
  service-worker/browser tests. Nothing is done while any required check fails.

---
