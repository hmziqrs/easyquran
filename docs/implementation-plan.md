# Implementation Plan — closing the gap to `my-plan-raw.md`

**Status:** proposed (not started)
**Baseline commit:** `bad793f`
**Source of intent:** `docs/my-plan-raw.md` (owner-authored, never edited by agents)
**Source of current state:** `docs/quran-system.md`, plus the code audit in §0
**Revision:** v3.2 — after four audit rounds (seven auditors). §12 records what changed.

Each workstream states: **goal · files · steps · invariants · tests · done-when**.

---

## 0. Gap ledger (audited at `bad793f`)

| # | Gap | Domain | Workstream |
|---|---|---|---|
| 1 | No popularity signal for translation hot-cache | Rust | W1 |
| 2 | `IP_SOURCE=ConnectInfo` in deploy config; wrong behind CF→Traefik | Rust | W2 |
| 3 | Quran routes never escalate to a ban; no export, no un-ban path | Rust | W3 |
| 4 | Engagement counter is a sessionStorage int | Web | W4 |
| 5 | Arabic `readRange` has no API fallback; validator dropped on API path | Web | W5 |
| 6 | `RangeReader` has no client data path | Web | W6 |
| 7 | No SSG-last tier for translated routes; degradation not pagination-aware | Web | W7 |
| 8 | Zero web-side auth; live cache-poisoning holes in its way | Web | W8-0 (+ separate doc) |
| 9 | Doc divergences unrecorded | Docs | W9 |
| 10 | Security riders this plan makes load-bearing | Both | W10 |

### Audited and already satisfied — no workstream needed

- **`my-plan-raw.md:4`** Arabic fully in memory at boot. True (`quran/loader.rs`;
  `main.rs:438,448`; `exit(1)` at `:455`).
- **`my-plan-raw.md:7`** no Redis. True and deliberate (L1 in-mem + L2 SQLite).
  **Standing constraint** — W1 must not erode it.
- **`my-plan-raw.md:18`** "SSG pages are hydrated once the cache is available." True for surah
  routes today: `SurahReader` renders server data, then fills further pages from the worker
  once `onStatus` reports ready (`SurahReader.svelte:611,617-620`). **Not** true for range
  routes — that is W6.

### Corrections to figures the intent doc relies on

- **"SQLite database is 1.5 MB"** — true for the pinned Arabic DB (`quran-uthmani.sqlite` =
  1,593,344 B). But time-to-SPA also pays `sqlite3-*.wasm` ≈ 864,752 B → real cold cost
  ≈ **2.4 MB**. Size any "how long is the degraded window" reasoning off 2.4 MB.
- **`my-plan-raw.md:5`** literally reads "We *do* load all translated database on boot…". In
  context and against the code the intent is clearly the opposite. Flagged here rather than
  silently rewriting the owner's sentence.
- Reader route count is ≈ **1,410** (114 surah + 662 surah-local-page + 604 global page +
  30 juz), not the ~1,300 used in earlier drafts.

### Global invariants

1. **Quran DBs immutable.** 2. **No SHA-256 over Quran data** (identity is the id; guarded by
`tests/quran_v1.rs` + `catalogue-sha-guard.test.ts`). 3. **Arabic pinned, only translations
evictable.** 4. **Reader hrefs use the `*For(ctx, …)` family** (guarded by `nav-guard.test.ts`).
5. **No per-user content in any shared cache.** 6. **No Redis.**
7. **`my-plan-raw.md` is read-only** — divergences go in W9.

---

## W1 — Popularity-driven translation residency (Rust)

**Goal:** implement "residency based on TTL and popularity" (`my-plan-raw.md:5`).
**Files:** `quran/translation_pool.rs`, `services/translation_popularity_store.rs` (new),
`migration/src/` (new migration), `main.rs`, `modules/quran_v1/controller.rs`, `quran/mod.rs`.

**Today:** TinyLFU count ceiling + idle TTL + post-build byte-bound LRU prune
(`translation_pool.rs:76-97,168-192`). No popularity signal.

### What "popularity" can honestly mean here

The pool only sees requests reaching the Rust process. Translated pages are SSR behind a 7-day
HTML disk cache, and warm clients read their local worker DB. So pool hits measure **API
demand** — a more popular translation may produce *fewer* pool hits once caching works.

That is not a defect: the pool exists to serve API demand, so optimising for API demand is
correct. But name it honestly — `api_demand_score`, not "reader popularity" (recorded in W9).

**Design decision:** popularity governs **admission** (prewarm); TTL/LRU/byte-bound govern
**eviction**, unchanged. A popular-but-idle entry resisting eviction would trade a bounded
latency cost for an unbounded memory cost, and the byte bound is what protects the box.

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
3. **Durable store, owned by migrations.** New migration in `migration/src/` (next after
   `m000001_init.rs`) — **not** a raw `CREATE TABLE IF NOT EXISTS` at boot. `rate_limit_state`
   inherited that wart; do not copy it.

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
5. **Snapshot, don't drain.** Copy counts out, upsert, then subtract only what committed. A
   drain-first design loses every count if the upsert errors.
6. **Decay in Rust, not in SQL.** `pow()` / `exp()` / `ln()` **do not exist in this build** —
   `sqlx-sqlite` bundles `libsqlite3-sys 0.30.1` without `SQLITE_ENABLE_MATH_FUNCTIONS`, so
   `ORDER BY score * pow(...)` fails at runtime with `no such function: pow`. The table is
   ≤115 rows, so `SELECT` all and decay in Rust:
   `decayed = score * 0.5f64.powf(elapsed_secs / HALF_LIFE_SECS)`, `HALF_LIFE = 7 days`
   (const, not env). Decay **at read**, so rows that stopped being hit don't keep a frozen
   score and outrank recently-popular ones forever.
7. **Prewarm — bounded, non-self-reinforcing, yielding.** Background `tokio::spawn` after pool
   construction:
   - `N = min(QURAN_PREWARM_TRANSLATIONS (default 2), max_resident_translations)`;
   - **must not increment `hits`** (else prewarmed ids gain score every boot with zero traffic
     and the top-N freezes) **nor `lookups`** (`:131` — it would corrupt `hit_rate`);
   - **must stamp `tick = 0`** — `enforce_byte_bound` evicts the lowest tick (`:179-186`), so a
     fresh prewarm tick would evict a corpus a real user is reading;
   - **must yield** while `build_sem.available_permits() < BUILD_CONCURRENCY`: each load holds
     one of two permits (`:149-152`) and each iteration takes `prune_sem` and runs
     `run_pending_tasks` twice;
   - **must not block or fail boot.** Arabic boot is fail-fast; translations are not.
   Implement as a private `warm(id)` path that bypasses the metric increments rather than
   calling `get_or_build` directly.
8. **Decide prewarm on evidence, with a stated threshold.** `time_to_idle` defaults to 1800 s
   (`settings.rs:162`), so prewarmed entries evaporate 30 min after boot unless real traffic
   arrives — in which case the first request would have built them anyway. Net value: "first
   request after each restart is fast, for N translations, for 30 minutes."
   **Threshold:** keep prewarm only if, over one week, `builds` in the first 30 min after each
   restart exceeds 20% of daily `builds`. Otherwise ship the popularity table for observability
   and drop prewarm — an acceptable outcome, not a failure.
9. **Observability.** Add `prewarmed: Vec<String>` and `top_demand: Vec<(String, f64)>` (cap 10)
   to `/quran/health/ready` (`controller.rs:985`). This drops `PoolStats: Copy`
   (`translation_pool.rs:33`) — keep `Clone`. The one consumer (`controller.rs:989`) reads
   scalar fields only, so it should compile untouched; verify rather than assume.
   Fix the `hit_rate` wart while here: `Option<f64>`, `None` when `lookups == 0`, instead of
   today's misleading `1.0` (`:217-218`).

**Invariants:** eviction semantics unchanged; no new lock; no second flush task; no Quran-DB
writes (`translation_popularity` lives in the app DB, `sqlite:./data/easyquran.db`, entirely
separate from `db/quran/tanzil/**` — the immutability rule is not touched).

**Tests:** decay halves across one half-life; ordering prefers a recent lower score over an old
high one; failed upsert leaves counts intact; eviction tombstones rather than dropping hits;
prewarm clamps to `max_resident_translations`, increments neither `hits` nor `lookups`, stamps
tick 0; prewarm on a bogus id warns without panicking; existing pool tests pass unchanged.

**Done when:** restart warms top-N by decayed demand, `/quran/health/ready` reports it, and no
eviction or memory-bound behaviour changed.

---

## W2 — Production client-IP correctness (Rust)

**Goal:** per-IP rate limiting actually keys on the client.
**Files:** `config/settings.rs`, `middlewares/route_blocker.rs`, `state.rs`, `main.rs`,
`deploy/.env.example`, `docker-compose.yml` (repo root, not `deploy/`), `deploy/README.md`.

**Today:** `settings.rs:98` defaults to `ConnectInfo` and `deploy/.env.example:42` ships
`IP_SOURCE=ConnectInfo` **uncommented**. Behind CF→Traefik every request carries the proxy's
IP, so per-IP limiting collapses into one global bucket. **The bug is set-but-wrong, not
unset** — a guard firing only on "unset" catches nothing.

### Steps

1. **One `is_production()` helper.** Three spellings exist today: `state.rs:107-114` reads
   `RUST_ENV → NODE_ENV → APP_ENV`; `route_blocker.rs:69` reads **only** `APP_ENV`, defaulting
   to `"development"`; `telemetry.rs:101` reads `DEPLOYMENT_ENVIRONMENT`. Since
   `deploy/.env.example:36` sets `RUST_ENV=production` and never `APP_ENV`, **the route blocker
   is disabled in production right now.** Convert `route_blocker` to the shared helper — a real
   bug fix riding along.
2. `from_env` must return `Result` instead of panicking (`settings.rs:97-107` currently
   `unwrap_or_else(|e| panic!(…))`), or step 3 is untestable. List its call sites (boot +
   tests) in the change.
3. Boot check: `is_production() && ip_source == ConnectInfo → error + exit(1)`, naming
   `CfConnectingIp` and `deploy/.env.example:42`.
4. Log the resolved IP source at startup unconditionally.
5. Set `deploy/.env.example:42` to `IP_SOURCE=CfConnectingIp` — **exact PascalCase**;
   `cf-connecting-ip` would hard-panic. `docker-compose.yml` (repo root) already passes the
   variable through to the API container via `env_file: [.env]` (`:80`) — verified.

### Trust, not just parsing

`CfConnectingIp` trusts the header unconditionally. If the origin is reachable without going
through Cloudflare, anyone can spoof `CF-Connecting-IP`. **Precondition for W3a:** Traefik (or
the host firewall) must restrict origin ingress to Cloudflare ranges; document in
`deploy/README.md`. Until then escalation stays disabled — a spoofable header plus automated
banning is a remote "ban anyone" primitive.

### Rollout

Two releases. **R1:** warn loudly at boot and log a structured event (there is no `/metrics`
endpoint — counters leave via OTLP only, so "emit a metric" means an OTLP attribute, not a
scrape target). **R2, one week later:** hard-fail. Gives operators a window without an outage.

**Tests:** `from_env` — prod + `ConnectInfo` → `Err`; prod + `CfConnectingIp` → `Ok`; dev +
`ConnectInfo` → `Ok`. Route blocker enabled under `RUST_ENV=production`.

**Done when:** production boots only with a correct IP source, the route blocker is live in
prod, and startup logs name the resolved source.

---

## W3 — Ban escalation, scope persistence, export (Rust)

**Goal:** `my-plan-raw.md:7` — ban bad actors longer, feed them to proxy level.
**Files:** `crates/rux-request-gate/src/{layer.rs,store.rs,abuse.rs}`,
`services/rate_limit_store.rs`, `migration/src/`, `modules/admin_*`, `main.rs`,
`docker-compose.yml`.

**Today:** the abuse limiter (Temp/Long scopes, `store.rs:194,203`) is wired only to `auth_v1`,
`newsletter_v1`, `post_comment_v1`, `forgot_password_v1`, `email_verification_v1`. Quran routes
get fixed-window layers only — 429s forever, never a ban, no export, no un-ban.

**Ship order: W3b → W3c → W3a.** Cheapest correctness fix first; the un-ban path before the
thing that creates bans; the dangerous part last, flagged off.

### W3b — Persist the ban scope (independent bug fix)

`store.rs:111-114` restores every ban as `Temp`, so `Long` downgrades on restart.

1. Add `scope` to `BucketSnapshot` (`store.rs:46-52`) and to `snapshot()`/`restore()`.
2. **Decision (made, not deferred): move `rate_limit_state` into the migration graph.** It is
   currently created by raw `CREATE TABLE IF NOT EXISTS` at boot
   (`services/rate_limit_store.rs:10-33`) while `migration/src/` holds only `m000001_init.rs`,
   so an `ALTER TABLE` migration would race `ensure_table`. Add the table *and* the new
   `block_scope TEXT` column in one migration, and delete `ensure_table`'s DDL. This matches
   W1 and stops the wart spreading.
   **Old-binary behaviour:** an old binary re-running `ensure_table` against the
   migration-owned table is a no-op (`IF NOT EXISTS`) and ignores the extra column — safe
   during a rolling deploy.
3. Write the scope on flush, read on restore, default `Temp` for pre-existing rows.

**Tests:** a `Long` ban survives restart as `Long`.
**Done when:** scope round-trips across restart and the table is migration-owned.

### W3c — Inspect and lift bans (must precede W3a)

1. `GET /admin/bans` — admin-ACL gated, `no-store`, paginated, returns
   `{ip, scope, block_until_at}`.
2. **`DELETE /admin/bans/{ip}`** — clears **both** L1 and L2. Without it the only way to lift a
   wrong ban is a DB edit, and after W3b not even a restart clears it. A ban system with no
   un-ban is not shippable.
3. `GET /admin/bans/traefik.yml` — deny fragment for a file-provider mount or polling sidecar.
4. **Whitelist key prefixes on export; never parse keys generically.** The same `InMemoryStore`
   holds abuse buckets keyed by **user id**, not IP — `totp:{user.id}`,
   `email_verify:{user_id}`, `email_verification:{user_id}`
   (`auth_v1/controller.rs:223,431`; `email_verification_v1/controller.rs:49,130`) alongside
   `ratelimit:{ip}:{path}` (`layer.rs:168`) and `forgot_password:{ip}`. Generic parsing would
   put **user IDs into a Traefik deny list** — a PII leak and a garbage ACL. Export must
   accept only known IP-keyed prefixes and parse IPv6 from the right.
5. Raw client IPs are PII: admin-gated, `no-store`, and subject to a stated retention rule.

**Cloudflare sync stays out of scope** — an outward-facing, hard-to-reverse change on a
third-party account needing its own decision on token scope, list id, blast radius, and
un-ban. These endpoints give an external job everything it needs. **Needs the owner's explicit
go-ahead before anyone builds it.**

**Done when:** an operator can list, export, and lift a ban without touching the DB.

### W3a — Escalate repeated blocks into a ban (last, default off)

**`on_block` cannot do this.** `BlockFn` is `Arc<dyn Fn(BlockInfo) -> Response + Send + Sync>`
— synchronous (`layer.rs:28`) — while `abuse::check` is `async` and needs a store handle
(`abuse.rs:46`); and `BlockInfo` (`layer.rs:19-26`) carries no client IP.

1. Escalate **inline in `RateLimitMiddleware::call`** (`layer.rs:176-206`) where `ip`
   (`:158`) and `layer.store` are in scope, behind opt-in `EscalationConfig`:
   ```rust
   struct EscalationConfig {
     key_prefix: &'static str,   // must match W3c's export whitelist
     temp_after: u32,            // default 5 window-blocks
     temp_window_secs: u64,      // default 3600
     long_after: u32,            // default 20
     long_window_secs: u64,      // default 86400
     allowlist: Vec<IpNet>,
   }
   ```
   (`IpNet` means adding the `ipnet` crate — not currently a dependency anywhere in the
   workspace.)
2. **Escalate at most once per window rollover, never per request.**
   `InMemoryStore::abuse_check` pushes an attempt on *every* call, including while already
   blocked (`store.rs:172`, inside the `until > now` branch), pruning only past
   `cfg.block_range` (`:165-168`). Recording per-429 means a flood stores one `i64` per blocked
   request for the whole `block_range` — turning the ban into a memory amplifier for the exact
   attack it should stop, making thresholds meaningless (crossed in milliseconds), and
   auto-escalating every temp-blocked IP to `Long`.
   *(The 24 h `MAX_ATTEMPT_WINDOW`, `store.rs:13`, applies to `prune()` — the retention here is
   each caller's `block_range`. Conclusion unchanged.)*
3. **The dedup primitive must be written fail-closed.** `abuse::dedup_nx` (`abuse.rs:109-117`)
   fails **open** by design, which contradicts the fail-closed stance everywhere else in this
   workstream. Either write a fail-closed variant or drop the fail-closed claim — do not ship
   the contradiction.
4. **`InMemoryStore.claims` pruning is a hard precondition, not a rider.** `claims`
   (`store.rs:66`) is never touched by `prune()` (`:121-131`, which walks `limits` only), so
   every dedup `set_nx_ex` leaks a `String` + `SystemTime` forever — the same memory
   amplification, moved one map over. Fix `prune()` before enabling escalation.
5. **Unify the key namespace with W3c.** Escalation keys and export keys must be the same
   prefix; otherwise `GET /admin/bans` will not show W3a's bans and `DELETE` will not clear
   them — and the un-ban path was the whole reason W3c ships first.
6. Wire to the `quran-v1` branch limiter **only** (`main.rs:682-684`). `/quran/search` is
   merged into `quran` at `:673-676` *before* the branch layer, so wiring both double-counts
   every blocked search.
7. **CIDR allowlist required.** The branch limiter collapses all quran paths into one bucket
   per IP (`middlewares/rate_limit.rs:23-30`) at 600/min. A university or carrier CGNAT exit
   exceeds that legitimately — today a recoverable 429, after W3a a `Long` ban for thousands.
   Escalation should also key on abuse *shape* (repeated 4xx, unknown ids, traversal attempts)
   rather than raw volume; **that detection design is out of scope here and must be specced
   before it ships** — until then, volume + allowlist only, with conservative thresholds.
8. **Split the healthcheck out of the escalating bucket.** `/quran/health/ready` lives in
   `quran_v1::routes()` (`mod.rs:50`) inside the 600/60 bucket, and `docker-compose.yml:92`
   healthchecks it — a busy box could rate-limit its own healthcheck into a restart loop. Give
   it its own `PathKey`.

**Preconditions, all hard:** W2 enforcing · Cloudflare-range ingress restricted · W3c un-ban
live · `claims` pruning fixed · allowlist configured · key namespace unified. **Default off.**

**Tests:** escalation off by default (existing limiter tests unchanged); N blocks → `Temp`,
M → `Long`; a sustained flood records **one** attempt per window (memory regression test);
allowlisted CIDR never escalates; `DELETE /admin/bans/{ip}` clears an escalation-created ban;
`/quran/health/ready` not in the escalating bucket.

**Done when:** a sustained abuser is banned, an operator can see and lift it, and no
legitimate CGNAT range can be banned by volume alone.

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
5. `sessionViews` stays in `sessionStorage`.
6. Bump `readerViews` always; `sourceViews[id]` only for translation sources.
7. Keep `PREFETCH_PREFIX` settle markers in `sessionStorage` and every existing guard:
   `saveData`/2g (`:27-32`), `whenIdle` (`:34-38`), one retry per source per tab (`:45-50`).
8. `noteTranslationChosen` (`:96`) keeps bypassing the gate — an explicit pick beats a heuristic.
9. **Migration is low-stakes — one release, not two (corrected in v3.2).** The legacy
   `eq:reader-views` counter is **sessionStorage** (`engagement.ts:6,16-18`) — per-tab,
   dies with the tab — so there is no durable history to protect. Seed `totalViews` from it
   if present and delete it in the same release. A web rollback then merely restarts the old
   per-tab counter at 0, indistinguishable from opening a new tab; settle markers
   (`eq:tprefetch:*`) are untouched, so no downloads re-trigger. (v3.1's two-release scheme
   guarded a cross-session history that never existed.)
10. **Rider (required): give `downloadBytes` a timeout.** `workers/download.ts` has no timeout,
    abort, or retry — the one fetch not wrapped by `fetchWithTimeout`. W4 *increases* how often
    `ensureTranslation` fires, multiplying the frequency of a hung fire-and-forget fetch.

**Do not** write a "`readJSON` throws" test — `safe-storage.ts:3-11` catches internally and
returns `undefined`. Test the `undefined` path.

**Tests:** one view/one day → no prefetch; four views same day → prefetch; two views across two
distinct days → prefetch; Arabic-only views never prefetch; `readJSON` → `undefined` falls back
to session-only without throwing; legacy key seeds `totalViews` once; gate
reads pre-bump `sourceViews`; `downloadBytes` aborts on timeout.

**Done when:** a reader returning on a second day gets the translation prefetched, a one-time
visitor does not, and no code path can hang forever on a download.

---

## W5 — Unify and repair the fallback chain (Web)

**Goal:** `my-plan-raw.md:14` — "fall back to API first".
**Files:** `lib/quran/worker-client.ts`, `lib/quran/api-client.ts`,
`app/_reader/SurahReader.svelte`, `src/service-worker.ts`.

**Today:** translations get an ordered chain (`worker-client.ts:108-156`); Arabic gets ad-hoc
early-returns — `readSurah:259-269` hits the API only if the worker was never constructed;
`readRange:286-291` is worker-only with no fallback.

### Precondition (moved forward — this is not an auth-only problem)

**The SW `/quran/` bypass is already dead in production.** `service-worker.ts:252` matches
`/quran/`, but prod `PUBLIC_QURAN_API_BASE=https://easyquran.fyi/api/quran`
(`deploy/.env.example:20`) → real paths are `/api/quran/…`, which falls past all five bypasses
into `swrApp()` and lands in **`eq-app-${version}`**. Quran API JSON is being cached there
today. W5/W6 turn API reads from rare into routine, so **add `/api/` to the bypass list as part
of W5**, not later in W8-0.

### Steps

1. Generalise `withTranslationFallback` into `withSourceFallback` with a `hasLocal` probe:
   translations → `hasTranslation(id)`; Arabic → worker readiness. `quranWorker.ready` is a
   **synchronous getter** (`:159`), so type the probe `() => boolean | Promise<boolean>`.
2. **Fix the boot-window branch.** The current first branch is `if (!worker && QURAN.apiBase)`
   (`:124`) and `worker` is `null` until `start()` runs — so a read during boot goes to the
   network even on a device holding the full local DB. Probe
   `startPromise !== null || worker !== null`, or `await whenReady()` with a short budget.
3. **Chunk API range reads to ≤ 300 ayahs** (`RESPONSE_CAP = 300`, `quran/store.rs:3`, enforced
   `controller.rs:1237-1239`). **Decision: chunk inside `quranApi.readRange`** (not "gate the
   fallback on span"), because W6 needs whole-juz reads to work.
   **Five juz exceed the cap — 19 (339), 23 (357), 27 (399), 29 (431), 30 (564)** — computed
   from the juz start table in `web/static/quran-meta/quran-data.json`. Global pages max at 42,
   so they are safe. Max chunk count is 2. The client already knows the span before calling
   (`loadQuranData().rangeByIndex(kind, index)` → `{startGlobal, endGlobal}`), so there is no
   discovery round-trip.
   **Stitching rules:** apply the coordinate validator **per chunk**; concatenate in order;
   **any chunk failure fails the whole read** (no partial renders). Timeout (corrected in
   v3.2): API reads run under `fetchWithTimeout` with `FETCH_TIMEOUT_MS = 3000` (`fetch.ts:1`)
   — `DEFAULT_TIMEOUT_MS = 30_000` (`worker-client.ts:32`) is the **worker-request** timeout
   and never applies to the API path. Two sequential chunks worst-case ≈ 6 s, so the real risk
   is inverted: 3 s may be too *tight* for a 431–564-ayah chunk on a slow network. Pass an
   explicit per-chunk budget sized for the largest chunk rather than inheriting the 3 s
   default.
4. **Thread `validateCoordinate` through `quranApi`.** `api-client.ts` takes no validator, so
   the API path decodes without the check the worker path performs (`worker-client.ts:288`,
   `SurahReader.svelte:417-419`). W5/W6 make the API path routine, silently weakening the
   invariant.
5. **Relax the reader's pre-gate.** `SurahReader.loadPage` returns early on
   `!quranWorker.ready` (`:397-403`), queuing into `pendingPages` and retrying when
   `onStatus === "ready"` (`:617-620`) — so the read is *deferred*, not permanently dead, but
   it never reaches the fallback chain while the worker is unhealthy. Let `loadPage` attempt
   the read and let the chain decide. (Write the test against "deferred", not "dead".)
6. Resulting order for both source kinds: `local → API → local re-check → throw`.

**Invariant:** a ready worker always beats the API; the API is strictly a fallback.

**Tests:** Arabic `readRange` — worker throws + API healthy → API result; worker healthy → API
never called; read during boot with a local DB → no network; each of juz 19/23/27/29/30 via API
→ chunked, no 400; chunk failure → whole read fails, no partial render; API path receives the
validator; `/api/` never enters `eq-app-*`; translation behaviour byte-identical.

**Done when:** every Arabic read has the same fallback ladder as translations, oversized juz
work through the API, and API responses are not cached by the SW.

---

## W6 — Client data path for range routes (Web)

**Goal:** `my-plan-raw.md:13` — stop relying on server data once the local DB is warm.
**Files:** `app/_reader/RangeReader.svelte`, `lib/quran/worker-client.ts`.

**Today:** `RangeReader` renders server `data` and nothing else — no worker read, no
`onStatus`.

**Scope:** the component serves **four** routes — `app/juz/[n]`, `app/page/[n]`
(`prerender = true`) and `app/t/[lang]/[translator]/juz/[n]`,
`app/t/[lang]/[translator]/page/[n]` (**`prerender = false`**). Both source kinds must be
designed for, and the prerender flag differs between them.

### Mechanism decision: component-level swap, not a universal `+page.ts`

An earlier draft proposed moving the decision into a universal `+page.ts` to avoid the
`__data.json` fetch. **That does not work.** SvelteKit gates the fetch on *the node having a
server load*, not on the presence of a universal load — so with `+page.server.ts` present,
`load_data()` runs on every client navigation regardless. Worse, a universal load is *fed*
`server_data_node.data`, so it cannot start until `__data.json` resolves — putting the worker
read there would serialize it **behind** the network hop it was meant to avoid.

So: render server data immediately, then upgrade from the worker off the critical path.
Eliminating the `__data.json` fetch entirely would require deleting `+page.server.ts` for these
nodes — a much larger change (Arabic prerender currently needs `node:sqlite` at build time,
which a universal load cannot do). **Out of scope here; recorded in W9.**

For Arabic this costs little: the route is prerendered, so `__data.json` is a **static build
artifact** served from the CDN/`eq-data-v1`, not an API call.

### Steps

1. Server `data` is the initial paint; hold `ayahs` in `$state`; subscribe via
   `quranWorker.onStatus` (pattern at `SurahReader.svelte:611`).
2. On index change **and** worker ready → `quranWorker.readRange(startGlobal, endGlobal,
   validator, source)`, validator pattern from `SurahReader.svelte:417-419`.
3. **Failure behaviour differs by source kind:** Arabic → keep server data (complete and
   prerendered). Translation → fall through to the API via `withSourceFallback` (W5); freezing
   on server HTML is wrong there, because the server render may be exactly what is stale.
4. Compute ranges from `loadQuranData()` — do not re-derive tiling in the component.
5. Nav links keep `juzPathFor`/`globalPagePathFor` (`:42-44`) — invariant 4.
6. Do **not** re-fetch on first paint when server data is complete; client reads are for
   navigation.

**Tests:** worker ready + client nav → worker read used; Arabic + worker unavailable → server
data rendered, no blank page; translation + worker unavailable → API fallback, not a frozen
render; juz 30 works on both paths; `nav-guard.test.ts` passes.

**Done when:** navigating juz→juz and page→page with a warm DB renders from the local DB, and
neither source kind can blank the page on failure.

---

## W7 — SSG-last tier and pagination-aware degradation (Web)

**Goal:** `my-plan-raw.md:15-17` — API first, **SSG last**, but only where the view is not
paginated.
**Files:** `src/service-worker.ts`, `app/_reader/SurahReader.svelte`, reader status UI.

### What is actually true (v2 got this half wrong)

- **For Arabic, an SSG-last tier already exists** — but not for the reason v2 gave. Arabic
  reader routes are `prerender = true`, so their `__data.json` is a **static build artifact**,
  not an API response. A SPA navigation to a prerendered Arabic route already gets SSG data
  with no document reload and no API involvement. That is the real "SSG last".
- **For translations there is no SSG artifact at all.** Both translated range routes and all
  translated surah routes are `prerender = false`; their `__data.json` comes from the Bun SSR
  server. **The intent at `my-plan-raw.md:22` is therefore genuinely unmet, and v2 wrongly
  asserted it was covered.**
- **Forcing document navigation is the wrong mechanism** (v2's refutation of v1 stands and was
  verified): SW navigation is network-first with a 3500 ms timeout
  (`service-worker.ts:25,304-338`); `eq-pages-v1` gains new entries **only** via
  `handleNavigation` (`:320-321`) — `revalidateCache` (`:495-521`) refreshes existing keys but
  never adds one — and is capped at `PAGES_MAX=300` of ~1,410 routes; the offline pack is read only by
  `handleData` (`:370-381`) and never serves navigations; and a reload discards the worker,
  re-paying ~2.4 MB.

### The unexplored option

`src/service-worker.ts:4` imports `{ base, build, files, version }` from `$service-worker` —
**`prerendered` is never imported anywhere in `web/src`.** SvelteKit exposes the full list of
prerendered routes. A list-driven precache or on-demand fetch of prerendered `__data.json`
would be a *real* SSG-last tier that does not require a document reload. This was never
evaluated before earlier drafts declared it unnecessary.

### Steps

1. **Evaluate `prerendered` as the Arabic SSG-last tier.** Measure the byte cost of staging
   prerendered `__data.json` for the reader routes (the offline pack already stages ~7 MiB /
   1,308 entries, so much of this may already exist — check for overlap before adding a
   second mechanism).
2. **Decide translations explicitly.** Options, pick one and record it in W9:
   (a) accept that translated routes have no SSG tier and degrade to the API + local DB only;
   (b) prerender a small high-traffic subset (e.g. top-N translations × 114 surahs) to create
   one. **Do not assert a tier exists when it does not.**
3. **Make the pagination distinction explicit in code.** In `SurahReader.loadPage`, keep the
   inline banner for multi-page surahs with the intent-doc rationale as a comment, and branch
   on `initial.pageCount === 1` for the single-page case.
4. **Retry, don't reload.** On a failed in-page load, retry through the fallback chain (W5) and
   the SW data cache. Explicitly: the "whole-route retry" for the single-page case is a
   `load`-level retry, **not** a document navigation.
5. **Distinguish the two failure kinds.** Worker-unhealthy and network-unhealthy must not
   collapse into one flag — a wasm/sqlite failure says nothing about the network, and the
   owner's stated failure model (`my-plan-raw.md:14-15`) is *API down, static origin up*, which
   only a two-signal design can represent. Surface them separately in the reader's status line
   (copy decision belongs with whoever owns reader UI).
6. **Any degraded flag must have a clearing rule** — clear on the next successful read, or make
   it a timestamp decaying over 60 s. A sticky flag with no reset turns one transient failure
   into a permanently degraded session.

**Tests:** multi-page in-page failure → banner + retry, no navigation; single-page failure →
`load`-level retry; degraded flag clears after one success; healthy → SPA navigation unchanged;
worker-down + network-up and network-down + worker-up produce different states.

**Done when:** the two failure kinds are distinguishable, degradation never forces a document
reload, and the translated-route SSG question has a recorded decision.

---

## W8-0 — Auth prerequisites (Web + Rust)

Full web auth is **deferred to its own document** — it spans ~10 Rust modules and five UI
phases (session foundation, email/password, OAuth ×4, passkeys, account UI), and folding it
here would put four unspecified placeholders into the sequencing graph.

**What stays here** are four prerequisites, three of them one-liners, one a live
cache-poisoning hole worth fixing regardless of when auth ships.
**Files:** `deploy/.env.example`, `docker-compose.yml` (repo root), `src/service-worker.ts`,
`middlewares/security_headers.rs`, `web/src/hooks.server.ts`.

1. **`ALLOWED_ORIGINS` is unset in deploy config.** The private router runs `origin_guard`
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
2. **SW `/api/` bypass** — already moved into W5 as a precondition (it is breaking Quran API
   caching today, not just auth). Listed here for completeness.
3. **`no-store` on authenticated API responses.** `security_headers.rs:44-70` sets
   CSP/HSTS/nosniff and nothing cache-related. Defense in depth behind (2).
4. **SSR disk cache must bail on authenticated requests.** `translationRouteCacheKey` keys on
   `(sourceId, kind, index)` only and `cacheable` never inspects cookies
   (`hooks.server.ts:11-27,104-105`), while the cached artifact is the **full HTML document
   including the app shell**. The moment any auth-derived markup renders during SSR on a `/t/`
   route, one user's HTML is served to everyone. Skip caching when the request carries a
   session cookie or the response sets one, plus a test asserting cached HTML contains no user
   tokens. This is the enforcement point for invariant 5.

**Notes for the auth document (do not lose these):** the session probe is
`GET /api/user/v1/get` (`user_v1/mod.rs:17-19`, behind `auth_guard::authenticated`, CSRF-exempt
as a GET) — **`auth_v1` is 100% POST with no `/me`**; the CSRF token is `HMAC(session_id)`
(`static_csrf.rs:50-55`), so it must be re-fetched after login/logout if login rotates the
session id; `/app/**` is prerendered, so auth must hydrate client-side; and nothing currently
decides what happens to `eq-pages-v1`/`eq-data-v1` on login and logout.

**Tests:** production-shaped `Origin` passes `origin_guard`; malformed `ALLOWED_ORIGINS` fails
validation instead of panicking; `/api/` never cached by the SW; a cookie-bearing `/t/` request
is not disk-cached.

**Done when:** an authenticated request cannot poison any shared cache, and a POST from the
production origin is not rejected.

---

## W9 — Record the divergences (Docs)

**Files:** `docs/quran-system.md`, `AGENTS.MD`.

`quran-system.md:112` already records "translated pages = SSR on Bun". Add the *why* and the
divergences this plan surfaced:

1. **Translated pages are SSR + 7-day disk cache, not SSG.** Prerendering would be
   114 × 115 ≈ 13,110 pages for the surah route alone. SEO is served by hreflang alternates
   (`routes/sitemap.xml/+server.ts:31-42`). `my-plan-raw.md:19` says SSG; the code is right.
2. **"SSG last" holds for Arabic only,** because those routes are prerendered and their
   `__data.json` is a static artifact. Translated routes have no SSG artifact — record W7
   step 2's decision here once made.
3. **The pool's popularity metric is API demand, not reader popularity** (W1).
4. **Range routes keep `+page.server.ts`,** so `__data.json` is fetched on client navigation;
   the worker read is an upgrade after paint, not a replacement (W6).
5. `docs/quran.md`, `docs/caching-architecture.md`, and `docs/quran-normalization-reasoning.md`
   were consolidated into `quran-system.md` at `bad793f`, and the old §4 known-gaps list did
   not survive — W10 carries the survivors. (`AGENTS.MD` was repointed at `quran-system.md` in
   that same commit; v3.1's "fix that pointer" instruction was stale — verified no dangling
   pointer exists at `bad793f`.)

**Do not edit `my-plan-raw.md`.**
**Done when:** every divergence above is recorded.

---

## W10 — Riders this plan makes load-bearing

1. **Remote-controlled fetch origin (security; do first).** `catalogue.ts:96-113`: a catalogue
   entry with no baked match keeps `t.downloadUrl` unrewritten and the worker fetches it
   verbatim; the `/_quran` allowlist only gates requests that reach `/_quran`.
   **`manifest.ts:37-42` has the identical hole** for Arabic script specs from `/scripts`.
   **Decision: reject non-allowlisted origins** (rather than silently dropping entries), so a
   drifted API response is visible instead of quietly shrinking the catalogue. Fix both files
   in one change.
2. **`downloadBytes` timeout** — folded into W4 step 10.
3. **`/quran/health/ready` own `PathKey`** — folded into W3a step 8.
4. **Bound `eq-data-v1`** (unbounded today; `trimPages` covers `eq-pages` only,
   `service-worker.ts:525-539`). W5/W6 increase `__data.json` churn. **Scheduled with W6.**
5. **Prune `InMemoryStore.claims`** — promoted to a hard precondition of W3a (step 4).
6. **`verifyBytes` size-only + non-atomic OPFS `put`.** Deferred — but it is the second half of
   rider 1's hole, so if rider 1 is ever resolved by *trusting* URLs rather than restricting
   them, this must ship with it. **Not scheduled; revisit after W10.1.**

---

## 11. Sequencing and operations

```
W10.1 ─► W2(R1 warn) ─► W2(R2 fail) ─┐
W3b ─► W3c ──────────────────────────┼─► W3a   [+ CF-range ingress, CIDR allowlist,
W1                                   │          claims-prune — all hard gates]
W5 ─► W6 (+W10.4) ─► W7 ─────────────┘
W4
W8-0
W9
```

**Hard constraints:** W2 enforcing + CF-range ingress + W3c + `claims` pruning + allowlist —
**all** before W3a. W5 before W6 before W7. W10.1 before anything that adds cookies to the page.

**Suggested first slice:** **W10.1 → W5 → W3b → W2(R1) → W9.**
W10.1 is security and self-contained; **W5 is the cheapest item that delivers actual owner
intent** (`my-plan-raw.md:14`, "API first") and also fixes the live `/api/` SW caching bug;
W3b is a correctness fix with near-zero blast radius (three layers, so not zero *effort*);
W2 R1 only warns; W9 is docs. Nothing in the slice can ban a user or break a reader.

### Operational requirements

- **Feature flags:** W3a (bans users) and W7 (changes degradation) ship behind flags with a
  documented kill switch. W1's prewarm is env-gated.
- **Rollback:** W4's legacy key is per-tab sessionStorage — nothing durable to protect (see
  W4 step 9); W3b's column is additive and readable by old binaries.
- **Migration ordering:** W1 and W3b both touch schema, and W3b moves an existing
  boot-created table into migrations. State per PR whether the migration runs before or after
  the binary, what an old binary does with a new column, and what a new binary does with
  pre-migration rows. Read `migration/README.md` into the change.
- **Partial deploy:** web and API are separate images. W5 must tolerate an older API; W2's
  hard-fail must not land while an older web depends on the API booting. API-first for additive
  changes, web-first for anything relaxing a contract.
- **Observability:** add counters for escalation/bans (W3), API-fallback rate and degraded-mode
  entry (W5/W7), auth failures (W8). There is **no `/metrics` endpoint** — counters leave via
  OTLP push only, so dashboards are extra scheduled work, not a free side effect.
- **Verification per workstream:** `cargo test`, `pnpm test`, plus `tests/quran_v1.rs`,
  `catalogue-sha-guard.test.ts`, `nav-guard.test.ts`. Nothing is done while any fail.

---

## 12. Revision history

**v3.2** — after round 4 (independent full re-verification at `bad793f`). Two substantive
fixes and one cut:

- **W5's chunk-timeout note cited the wrong constant.** API reads run under
  `fetchWithTimeout`'s 3 s default (`FETCH_TIMEOUT_MS`, `fetch.ts:1`); `DEFAULT_TIMEOUT_MS =
  30_000` (`worker-client.ts:32`) is the worker-request timeout and never touches the API
  path. The risk is therefore a too-*tight* budget for a 431–564-ayah chunk, not a 60 s hang
  — the guidance now says to set an explicit per-chunk budget instead of "use a shorter one".
- **W9 item 5's pointer instruction was stale.** The `bad793f` consolidation commit itself
  repointed `AGENTS.MD` at `quran-system.md`; no dangling `docs/quran.md` pointer exists at
  the plan's own baseline. (The consolidation also folded in
  `quran-normalization-reasoning.md` — three docs, not two.)
- **W4's two-release migration cut.** The legacy `eq:reader-views` counter is per-tab
  sessionStorage; the "rollback wipes every reader's history" rationale protected a history
  that never existed. One release: seed, delete.

Also corrected: the compose file lives at the repo root, not `deploy/` (W2/W8-0 file lists;
W2 step 5's passthrough is now verified — `env_file: [.env]` at `:80`); `origin_guard` is
layered at `main.rs:663`, not `:672`; W1's flush-removal rule restated as re-acquire +
check-and-remove in one guard with a `hits == 0` drain condition, closing a
resurrect-then-re-evict race the v3.1 wording left open and removing a literal reading that
would hold the lock across the SQLite write; W3a's `IpNet` allowlist requires the `ipnet`
crate (new dependency, previously unstated); line-ref drift fixed (`abuse.rs:109-117`,
`store.rs:172`, `:165-168`). Everything else re-verified exactly as written, including: all
W1 pool line refs; no `SQLITE_ENABLE_MATH_FUNCTIONS` anywhere in `libsqlite3-sys 0.30.1`'s
build script (not even as an opt-in feature); 115 catalogue entries; the five oversized juz
(339/357/399/431/564), the 42-ayah page max, and the 662 surah-local pages — all recomputed
from `quran-data.json`; the five abuse-wired modules and the user-id key prefixes
(`totp:{user.id}` at `auth_v1/controller.rs:223,431`); live `/api/` caching into `eq-app-*`
via `swrApp`; `prerendered` unimported anywhere in `web/src`; 1,593,344 B Arabic DB +
864,752 B wasm; the 6.84 MiB / 1,308-entry offline pack; `auth_v1` 100% POST; and W8-0's
origin-allowlist, CSRF-exemption, and cache-key facts.

**v3.1** — after round 3 (convergence audit). One fix-induced blocker and one major, both in
W1 step 2's tombstone design, which v3 introduced to fix a round-2 blocker without re-checking
it against the victim-selection loop it shares state with: tombstones stamped `tick = 0` were
permanently the `min_by_key` victim, and `cache.invalidate` on an already-absent key is a
no-op, so `enforce_byte_bound` would spin forever holding `prune_sem` on the request hot path.
Victim selection now skips tombstones, the tick sentinel is no longer overloaded with prewarm's
`tick = 0`, and resurrect/flush-remove semantics are defined so a reload between snapshot and
removal cannot delete a live entry's meta. Also corrected: `ALLOWED_ORIGINS` panics on control
and non-ASCII bytes, not on spaces or trailing commas (the latter silently yields an empty
entry); `eq-pages-v1` is also touched by `revalidateCache`, though only for keys already
present, so the 300-of-1,410 conclusion is unchanged; `restore()` at `main.rs:135` does not
move with the spawn; and the `PoolStats: Copy` break is smaller than claimed.
Round 3 independently re-derived the five oversized juz, the 42-ayah page max, the absent
SQLite math functions, the SvelteKit server-load gating, the live `eq-app-*` caching of Quran
API JSON, and all four rate-limit-store facts — all confirmed exactly as written.

**v3** — after round 2 (adversarial re-audit + executability audit). Four blockers fixed:

- **W1 step 6 would have failed at runtime.** `pow()`/`exp()`/`ln()` do not exist in the
  bundled SQLite (`libsqlite3-sys 0.30.1` without `SQLITE_ENABLE_MATH_FUNCTIONS`). Decay moved
  into Rust over the ≤115-row table.
- **W1's flush fold is not free.** The rate-limit flush is spawned at `main.rs:136`, 300+ lines
  before the pool exists; the spawn must move and `sea_db` must be cloned before `AppState`
  takes it. Also resolved the contradiction between "no second mutex" and "pending accumulator"
  by tombstoning in the existing map, and added the lock-ordering rule.
- **W6's mechanism was refuted.** SvelteKit gates the `__data.json` fetch on the node having a
  *server* load, so a universal `+page.ts` does not avoid it — and a universal load is fed
  server data, so the worker read would have been serialized behind the very fetch it was meant
  to dodge. Reverted to the component-level swap. Also corrected the claim that all four routes
  are `prerender = true` (the two translated ones are not).
- **W7 was rationalizing away real work.** v2 concluded the SW caches constitute an SSG-last
  tier; they do not (`eq-pages-v1` holds only previously-visited documents, 300 of ~1,410
  routes). The correct Arabic argument is that prerendered routes serve `__data.json` as a
  *static build artifact*. For translated routes there is no SSG artifact at all, so the intent
  at `my-plan-raw.md:22` is genuinely unmet — now stated instead of asserted away, with
  `prerendered` from `$service-worker` (never imported anywhere in `web/src`) flagged as the
  unexplored option.

Also: W5's oversized-juz list corrected from 2 to **five** (19, 23, 27, 29, 30) with chunk
stitching and partial-failure rules specified; the `/api/` SW bypass moved forward from W8 to
W5 after finding prod Quran API JSON is cached in `eq-app-*` today; W3c export now whitelists
key prefixes after finding `totp:{user_id}` in the same store (a deny list would have shipped
user IDs); W3a's key namespace unified with W3c's, `claims` pruning promoted from rider to hard
precondition, `dedup_nx`'s fail-**open** behaviour flagged against the fail-closed claim,
concrete escalation thresholds supplied, abuse-shape detection explicitly deferred as unspecced;
W3b's deferred "pick one" decision made (table moves into migrations); W1 notes the
`PoolStats: Copy` break and `lookups` pollution and adds a measurable prewarm keep/drop
threshold; W8a-e split into its own document, leaving W8-0's four prerequisites here with the
`ALLOWED_ORIGINS` parse-panic edge; per-workstream **files** and **done-when** restored
throughout (the header promised them and v2 dropped them); `my-plan-raw.md:18` added to the
already-satisfied ledger; route count corrected to ~1,410; line refs corrected
(`catalogue.ts:96-113`, `settings.rs:162`, `SurahReader:397-403`, `layer.rs:168`,
`store.rs:111-114`).

**v2** — after round 1 (facts / soundness / completeness). Retargeted docs to
`quran-system.md` after `bad793f` consolidation; W2 rediagnosed (set-but-wrong, not unset; the
prescribed value would have panicked; found the live `APP_ENV` bug disabling the route blocker
in prod); W3a's `on_block` premise refuted (sync hook, no IP) and escalation moved inline, with
the per-429 memory amplification, search double-count, CGNAT risk, and mandatory un-ban path
added; W1 corrected in five ways; W7's `data-sveltekit-reload` mechanism refuted; W5 gained
four blockers (`RESPONSE_CAP`, dropped validator, upstream reader gate, boot-window probe);
W6 rescoped to four routes; W8 gained W8-0; W4 gained pre-bump gate ordering, monotonic day
guard, cross-tab RMW hazard, two-release migration; added W10 and the operations section.
