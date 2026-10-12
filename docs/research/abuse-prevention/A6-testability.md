# A6 — Testability & CI-gate audit (DESIGN-v0: device identity)

Repo: `/Users/hmziq/os/easyquran` @ `53d3d421` (master). Worktree read: `.wt/next-features` @ `bed80b13` (only
for the deleted-CI-gate evidence).
Scope: can every part of `DESIGN-v0.md` be verified by this repo's gates, and what must change.

## 0. Headline

1. **Master CI runs zero tests.** `.github/workflows/images.yml` @ HEAD builds images, provisions Quran DBs,
   runs `bun run quran:check`, and nothing else. The gate blocks that existed in the parent commit (`37d90c41`)
   were deleted by HEAD (`53d3d421 fix(ci): remove security publication gates`). The pre-commit hook is
   `vp staged` → `"*": "vp check --fix"` (lint/fmt/types only; `vite.config.ts:4-6`). So the only test gates that
   exist today are the local `bun run check|lint|test` (web) and `cargo test` (rust) — nothing machine-enforced
   on push. Every "must stay green" claim in `AGENTS.MD` is currently a human promise.
2. **The collector cannot be written against real browser APIs and remain testable.** happy-dom 20.11.1
   (empirically probed) has no WebGL/WebAssembly/`navigator.gpu`, `navigator.userAgentData === undefined`,
   `canvas.getContext("2d") === null`, `navigator.storage === undefined`, and **`navigator.webdriver === true`**.
   The collector must take an injected, closed `DeviceSignals` value; the browser provider becomes a thin,
   deliberately-untested adapter.
3. **Three structural blockers in the Rust design**, all pre-existing shapes the design did not account for:
   `BanUnit` is a newtype over `IpNet` (not an enum); the gate's escalation path is gated on
   `RequestIdentity::is_external_ip()` and the gate crate _cannot import_ `BanUnit`; and `src/test_utils` is
   `#[cfg(test)]`-only, so integration tests must hand-roll the app/state/CSRF harness.

## 1. Per-component testability (a–k)

Notation: **T** = testable in-repo with existing runners; **S** = testable only after a specific seam is added;
**M** = manual only.

### (a) Client collector — `web/src/lib/device/`

- **Runner:** vitest via `cd web && bun run test` (`vp test run`), environment `happy-dom`
  (`web/vitest.config.ts:11-14`), include `src/**/*.{test,spec}.ts`. `pretest` = `bun run i18n:check`.
- **What happy-dom actually provides** (probed with `bun -e` against the installed happy-dom):
  `hardwareConcurrency = 8`, `platform = "X11; Darwin arm64"`, `languages = ["en-US","en"]`,
  `screen 1024×768×24`, `Intl` present (timeZone string). Absent: `userAgentData`, `deviceMemory`,
  `navigator.gpu`, `navigator.storage`, `WebGLRenderingContext`, `WebGL2RenderingContext`, `WebAssembly`,
  canvas 2D context (returns `null`), `navigator.plugins` (empty). `navigator.webdriver === true`.
- **T** for everything downstream of signal _capture_: bucketing, family derivation, extension-set hashing,
  coherence booleans, spoof flags, payload serialization, storage round-trip, single-flight token minting.
  Use hand-rolled `DeviceSignals` fixtures — the house style for fake browser APIs is a local in-file fake
  (`src/lib/workers/__tests__/idb.test.ts:47-144`, `opfs-cache.test.ts:41-411`), and `localStorage` works
  directly in happy-dom (`safe-storage.test.ts`).
- **S**: the browser provider (`browserDeviceSignals()`) itself. It must be _excluded_ from coverage by
  construction: the only testable claims are "never throws on missing APIs" and "returns the closed shape",
  which can be asserted with a stub `globalThis` that deletes the APIs (house style: `stripOpfs()` /
  `delete (globalThis as { indexedDB?: unknown }).indexedDB` in `opfs-cache.test.ts:392-411`).
- **M**: real GPU vendor/renderer families, real UA-CH high-entropy values, real WebGPU adapter info,
  Safari/Firefox farbling, real timing signatures. Procedure in §8.
- **Trap:** `navigator.webdriver` is `true` under happy-dom, so a collector that reads it directly makes every
  test look like a tampered browser. The `tamper` flags must come from the injected signals, not from a
  direct read inside the classifier.

### (b) Transport attachment in `AuthClient.#request`

- **Runner:** vitest; `web/src/lib/auth/__tests__/auth-client.test.ts` is the exact template:
  `vi.mock("$app/env/public", …)` at top, `vi.spyOn(globalThis, "fetch")` + real `Response` objects,
  assertions on `(url, init)` pairs.
- **T**: header attached only on the allowlisted security paths (login, register, 2FA, passkey, credential change — A4 §7), never on `/csrf/v1/generate` or `/user/v1/get`; absent on `exempt: true`; absent when
  the provider yields nothing; provider rejection never fails the request (mirror the existing
  `"rotation refresh is best-effort and does not fail the originating request"` test, line 252).
- **S**: the provider must be constructor-injected (`new AuthClient(signals)`), because `#request` is private
  and `authClient` is a module singleton (`auth-client.ts:286-290`). Today's only seam is the fetch spy.
- **Hard constraint from existing tests:** `auth-client.test.ts` asserts call _counts_ and call _indices_
  (`fetchMock.mock.calls[2]`, `toHaveBeenCalledTimes(2)`, the 5-element URL array at line 297). If the device
  token is minted with its own `fetch`, ~20 assertions across that file (and `account-client.test.ts`) need
  renumbering. The token must ride the existing `/api/**` round trip (response header/body), never a side fetch.
- **M**: real header size under CF→Traefik (`x-eq-device` must stay well under the proxy header limit), CORS
  allow-header acceptance in the deployed stack.

### (c) HMAC device key derivation

- **Runner:** `cargo test`; unit tests in the module + `tests/*.rs` with in-memory sqlite.
- **Precedent to copy exactly:** `rust/backend/api/src/utils/code_hash.rs` — `Hmac<Sha256>` over a server
  secret, hex-encoded, with `hash_is_deterministic` / `hash_differs_by_code_and_key` / `hash_is_hex` (len 64)
  tests. `static_csrf.rs:49` shows the same pattern for a derived key with `HKDF`-style separation.
- **T**: fixed pepper + fixed component list → fixed vector (assert the literal hex once); key differs when any
  component changes; key differs across peppers; component order canonicalized (sort before hashing, so the
  same signals in a different insertion order collide).
- **S**: pepper must be a parameter/`&[u8]` field on settings, not a `OnceLock` read inside the hash fn —
  otherwise tests cannot set it twice in one process (the `field_crypto::set_key` "already initialized with a
  different value" error is exactly this trap, `utils/field_crypto.rs:24-36`).
- **M**: pepper rotation in production (rotating the pepper re-keys every device → every device looks new →
  step-up storms; needs a staging run and an ops runbook).

### (d) `device_records` / `device_sessions` (device migration; next free number, `NN` below)

- **Runner:** `cargo test -p migration` (inline `#[cfg(test)] mod tests`) + `Migrator::up` from `tests/*.rs`.
- **T** (see §4): fresh `up` creates the table, columns, `UNIQUE(device_key)`, `(user_id)`/`(last_seen)` indexes,
  defaults, and the FK actions; `up` twice is idempotent; `down` drops; a full `Migrator::up` run creates the
  index (catches "forgot to register the migration in `migration/src/lib.rs`" — the m000010 bug class).
- **Gap the design must close:** `user_id … SET NULL` is a _foreign-key action_. SQLite enforces FKs only with
  `PRAGMA foreign_keys = ON`; production enables it at connect (`db/sea_connect.rs:50`), but a test harness that
  calls `Database::connect` directly does not. Unless the test explicitly turns it on, "user deleted → device
  record survives with NULL user" is unverified.
- **M**: applying the migration to the live DB (row counts, wall time).

### (e) Risk engine + hysteresis

- **Runner:** pure Rust unit tests — no HTTP, no DB. Precedent: `rux-request-gate/src/store.rs` abuse tests and
  `services/rate_limit_store.rs` tests, which exercise the scoring/flush functions directly.
- **T**: boundary triplets `79/80/81` and `39/40/41`; monotonic hysteresis (a device at 60 with a prior ban
  stays banned until ≤40 — assert the exact pair of transitions, not "score < threshold"); rule-version pinned
  in the input so a rules change is a visible test diff; a deterministic `now` parameter.
- **S**: rules must be _data_ (`struct RiskRule { version, weight, predicate }`) with the engine taking
  `&[RiskRule]`; a hardcoded `if` ladder can only be tested through the HTTP path.
- **M**: real precision/false-positive rate. That is the design's Layer-5 dashboard; no test can stand in.

### (f) Step-up trigger

- **Runner:** Rust integration (`tests/fresh_auth_security.rs` is the template: real `AuthSession`,
  `fresh_auth::issue/consume/available`, cookie-bound single-use grants, in-memory sqlite) + web vitest for the
  client side (`fresh-auth-client.test.ts` pattern: `isFreshAuthenticationRequired` reads
  `error.context.reason === "fresh_auth_required"`).
- **T**: the trigger as a pure predicate over `(device_record state, session state, action class)` — unit-test
  the four cases (new device at login; device change mid-session; credential change from a new device;
  ban-relevant action) without HTTP; then one integration test that a login from a new device yields the
  fresh-auth error context and the existing reauth flow satisfies it.
- **S**: the device key must be resolvable from the session/request inside `auth_v1`'s controller — i.e. the
  `AppState`/extension carries the resolved device record, not a re-query.
- **M**: the Turnstile challenge round-trip (needs the real CF site key + a real browser). happy-dom can only
  assert `turnstileEnabled()` gating and the challenge-unavailable decision.

### (g) Device ban unit in `rux-request-gate`

- **Runner:** `cargo test -p rux-request-gate` — the crate has its own fake `IdentitySource` (`StaticExternal`,
  `layer.rs:464`) and fake `Escalation` implementations, plus `OkService` as the inner service.
- **Blockers (measured, not guessed):**
  - `layer.rs:245-330`: both escalation call sites are wrapped in
    `identity.as_ref().filter(|i| i.is_external_ip())`. A device identity is not an external IP, so it can
    never reach `active_ban` today.
  - `ip.rs:39-57`: `RequestIdentity` has exactly two variants; `bucket_key()`/`is_external_ip()` are the only
    knobs.
  - `layer.rs:369-390`: the gate crate mirrors `BanUnit::from_ip` in a private `canonical_unit_key` **because
    the gate crate cannot import `BanUnit`** (api depends on the gate, not vice versa). A device ban unit has
    no such mirror, and the existing lockstep test `canonical_unit_key_mirrors_ban_unit` only pins IPs.
- **T after the seam:** a fake identity source yields a device identity → a fake escalation reports an active
  device ban → the layer short-circuits with the 429 shape; the device identity never produces an IP bucket
  key (`ratelimit:{device}:…` ≠ `ratelimit:{ip}:…`); the IP path's fixed counters are untouched.
- **M**: the real Traefik/CF behaviour of a device-ban response.

### (h) Edge-header verification

- **There is no edge code in this repo.** No wrangler config, no CF Worker directory; `web/src/service-worker.ts`
  is the PWA service worker and `web/tsconfig.worker.json` compiles only that file. `.github/workflows` has one
  workflow, `images.yml`.
- **S/T**: put the verifier in Rust as a pure function
  `verify_edge_headers(headers: &HeaderMap, secret: &[u8], now: i64) -> Result<EdgeFacts, EdgeError>`, plus a
  middleware that strips client-supplied `X-Eq-Edge-*`. Then unit tests: HMAC mismatch, `kid` unknown,
  timestamp at −31s/−30s/+30s/+31s (boundary), missing/duplicate headers, and a `oneshot` test that a
  client-supplied `X-Eq-Edge-Ip: 1.2.3.4` never reaches the handler.
- **M**: the Worker's own signing, real `request.cf` fields, JA4 (plan-gated), CF Logpush.

### (i) Admin API (`/admin/devices`)

- **Runner:** Rust integration; `tests/admin_bans_export.rs` is the template — in-memory sqlite +
  `Migrator::up`, real admin seeded via the sea-orm model, a real `AuthSession::login`, then a **forged private
  `ruxlog.sid` cookie** via `CookieJar::private_mut(&cookie_key)` (lines 366-383), and `oneshot` requests.
- **T**: unauthenticated → 401/403; admin session → 200; the read-only `BAN_EXPORT_TOKEN` must **not** unlock
  any device route (add the case, mirroring `export_token_does_not_authorize_delete_mutation`); ban/unban
  effects on L1 + L2 (`successful_delete_clears_l1_l2_history_and_fixed_count` pattern); evidence row written;
  pagination bounds.
- **M**: the "minimal UI" — there are **no admin UI routes in `web/src/routes` at all** (no `admin*` directory).
  Only component-level vitest is possible after it is built.

### (j) Retention / cleanup job

- **Runner:** Rust; precedent `services/rate_limit_store.rs` — the periodic task is `spawn_flush_task`, and its
  tests call the inner `flush(&db, snaps)` directly, never the ticker (`rate_limit_store.rs:126-180, 311`).
- **S/T**: expose `retention::cleanup(&db, now) -> RetentionReport`; test with seeded rows: key deleted at
  > 30d, ban row retained to 12mo, aggregates survive, nothing older than the ladder is left; assert the report
  > counts. Run it twice (idempotent).
- **M**: the spawned ticker, restart behaviour, and production row counts after a real 31-day window.

### (k) Appeal flow

- **Nothing exists.** No table, no route, no UI, no mail template. Testable parts: durable-notice row write +
  status transitions (`submitted → under_review → upheld/overturned`) + admin-decision audit, as pure functions
  over sqlite. The email delivery path is a noop in tests (`MailRouter::new(…, false)`) — delivery is **M**.
- **M**: the end-to-end journey (ban → notice → appeal → decision → unban) and the legal timeline.

## 2. The collector seam (exact shape)

Requirements from the enabled lint rules (`web/vite.config.ts:26-64`):
`anti-slop/no-unknown-parameters`, `no-unknown-returns`, `no-unknown-type-aliases`, `no-unsafe-dictionary-type`
(a `Record<string, unknown>` / `any` / `object` value is an error; `Record<string, string>` is fine),
`no-object-parameters` (only the literal `object` keyword is banned — a named interface is fine),
`no-runtime-typeof` (no `typeof x === "…"` without an eslint-disable + reason; use `browser` from `$app/env`),
`require-safety-comment-for-type-assertion` (any `as` needs a SAFETY comment).

```ts
// web/src/lib/device/signals.ts — the seam. Pure data, closed shape, no browser access.

export interface HardwareSignals {
  readonly glVendorFamily: string; // bucketed family, never the raw renderer string
  readonly glRendererFamily: string;
  readonly glExtensionSetHash: string; // hex digest of the sorted extension-name set
  readonly shaderPrecision: string; // e.g. "highp/highp/highp"
  readonly wasmTimingBucket: number; // coarse bucket, never the raw ms
}

export interface PlatformSignals {
  readonly platform: string;
  readonly featureBits: readonly string[]; // sorted, closed vocabulary
  readonly uaChBrandFamily: string; // "" when unavailable
  readonly uaChPlatformVersionBucket: string;
}

export interface LocaleSignals {
  readonly languageQuintuple: readonly string[];
  readonly resolvedLocale: string;
}

export interface ScreenSignals {
  readonly widthBucket: number;
  readonly heightBucket: number;
  readonly pixelRatioBucket: number;
}

export interface CoherenceSignals {
  readonly uaVsUaCh: boolean;
  readonly gpuVsPlatform: boolean;
  readonly hardwareTriple: boolean;
  readonly screenVsInner: boolean;
}

export interface TamperSignals {
  readonly webdriver: boolean;
  readonly nativeFnTampered: boolean;
  readonly cdpArtifacts: boolean;
}

/** Everything the classifier needs. `null` = that class was unavailable, never an error. */
export interface DeviceSignals {
  readonly hardware: HardwareSignals | null;
  readonly platform: PlatformSignals;
  readonly locale: LocaleSignals;
  readonly screen: ScreenSignals;
  readonly coherence: CoherenceSignals;
  readonly tamper: TamperSignals;
}

export type DeviceSignalsProvider = () => Promise<DeviceSignals>;
```

```ts
// web/src/lib/device/browser-signals.ts — the untested adapter. The ONLY file in the repo
// allowed to touch navigator/screen/canvas/WebGL/WebAssembly; machine-guarded (G1, §3).
import { browser } from "$app/env";
import type { DeviceSignals } from "./signals.js";

export async function browserDeviceSignals(): Promise<DeviceSignals> {
  if (!browser) return unavailableSignals();
  // ...every read in try/catch, every bucket derived here...
}
```

```ts
// web/src/lib/device/collector.ts — the tested core. Never reads a browser API.
import { readJSON, writeJSON } from "#lib/storage/safe-storage.js";
import type { DeviceSignals, DeviceSignalsProvider } from "./signals.js";
import { browserDeviceSignals } from "./browser-signals.js";

export interface DevicePayload {
  readonly v: 1;
  readonly hardware: string; // derived, compact
  readonly platform: string;
  readonly locale: string;
  readonly screen: string;
  readonly coherence: string; // bitfield
  readonly tamper: string; // bitfield
  readonly token: string | null; // server-minted, from storage; never derived here
}

export class DeviceCollector {
  constructor(private readonly provider: DeviceSignalsProvider = browserDeviceSignals) {}

  async payload(): Promise<DevicePayload | null> {
    // ...classify(await this.provider()), merge the stored token, return null on any failure...
  }
}

export const deviceCollector = new DeviceCollector();
```

```ts
// web/src/lib/auth/auth-client.ts — injection point.
export class AuthClient {
  constructor(private readonly device: DeviceCollector = deviceCollector) {}
  async #request<T>(method: string, path: string, opts: RequestOpts = {}) {
    // Synchronous read of an already-collected payload: never await the collector inside #request —
    // it would delay the session probe and can trip the 12 s abort (A4 §3). Allowlisted paths only (A4 §7).
    const payload = isDevicePath(path) ? this.device.cachedPayload() : null;
    if (payload) headers["x-eq-device"] = encodePayload(payload);
    // ...existing body unchanged...
  }
}
```

Storage conventions to follow: keys under `easyquran.device*` (cf. `easyquran.prefs`, `easyquran.reader`),
access only through `#lib/storage/safe-storage.js` (`browser`-gated + try/catch, `safe-storage.ts:1-70`), and a
**new single-store IDB DB** via `openIdb("easyquran-device", "deviceToken")` — `IDB_VERSION` is a global `1`
and `openIdb` hard-fails when a store is missing (`workers/idb.ts:1-36`), so the device DB gets exactly one
store forever at v1.

## 3. Machine guards this feature needs (house style, with assertion sketches)

House style: ordinary tests under `src/**/__tests__/` that read source text (`readFileSync` +
`path.resolve(process.cwd(), rel)` as in `catalogue-sha-guard.test.ts:1-8`) or raw module text via
`import.meta.glob(… { query: "?raw", import: "default", eager: true })` as in `nav-guard.test.ts:42-61`; on the
Rust side `include_str!` source scans as in `tests/security_tests.rs:343-380`. Each guard below names the
invariant that would otherwise rot silently.

**G1 — no fingerprinting API outside the device adapter.**
Rots silently when someone adds a quick `canvas.getContext(...)` in a reader component to "improve" the signal.

```ts
const FINGERPRINT_API =
  /\bgetContext\s*\(|toDataURL|OfflineAudioContext|\bAudioContext\b|userAgentData|\bnavigator\.gpu\b|requestAdapter|WEBGL_debug_renderer_info|navigator\.plugins/;
const ADAPTER = /^src\/lib\/device\/browser-signals\.ts$/;
it("only the device adapter touches fingerprinting APIs", () => {
  for (const [rel, src] of sourceFilesUnder("src")) {
    if (ADAPTER.test(rel)) continue;
    expect(src, `${rel} must not touch fingerprinting APIs`).not.toMatch(FINGERPRINT_API);
  }
});
it("the adapter never uses canvas/audio hashing at all (Layer 0)", () => {
  const adapter = read("src/lib/device/browser-signals.ts");
  expect(adapter).not.toMatch(/toDataURL|OfflineAudioContext|\bAudioContext\b|createOscillator/);
});
```

**G2 — device payload never attached to reader routes; no device cookie reaches reader paths.**
Rots silently: a device cookie visible on reader requests (`document.cookie`, or a `Path=/` cookie) flips
`requestHasCookie` in `hooks.server.ts:427` → `cacheable` false at line 346-351 → the 7-day SSR disk cache stops
caching for every reader. No error, just slower pages. (A server-set HttpOnly cookie scoped to `Path=/api` is never
sent on reader paths; client code must never write a cookie at all.)

```ts
it("reader fetch paths carry no device header", () => {
  for (const rel of ["src/lib/quran/fetch.ts", "src/lib/data/quran-data-client.ts"]) {
    expect(read(rel), `${rel} must stay device-free`).not.toMatch(
      /x-eq-device|deviceToken|deviceCollector/,
    );
  }
});
it("the device token is never a cookie and never in document.cookie", () => {
  const device = read("src/lib/device/token.ts") + read("src/lib/device/collector.ts");
  expect(device).not.toMatch(/document\.cookie|Set-Cookie|credentials: "include"/);
});
it("AuthClient is the only device-header writer", () => {
  expect(read("src/lib/auth/auth-client.ts")).toMatch(/x-eq-device/);
  for (const [rel, src] of sourceFilesUnder("src")) {
    if (rel === "src/lib/auth/auth-client.ts") continue;
    expect(src, `${rel} must not write x-eq-device`).not.toMatch(/"x-eq-device"|x-eq-device:/);
  }
});
```

**G3 — device records must never join to reading data.**
Rots silently: someone adds "devices per surah" to the admin dashboard and the Art-9 boundary is gone.

```ts
it("the device migration references no reader table", () => {
  const m = read("../rust/backend/api/migration/src/m0000NN_device_identity.rs");
  for (const t of ["bookmark", "reading", "recitation", "search_history", "post_view", "comment"]) {
    expect(m, `device migration must not reference ${t}`).not.toMatch(new RegExp(t, "i"));
  }
});
```

```rust
// tests/device_identity_regressions.rs — Rust-side scan, include_str! style.
#[test]
fn device_module_never_reads_reader_tables() {
    let srcs = [
        include_str!("../src/modules/device_identity_v1/controller.rs"),
        include_str!("../src/services/device_risk.rs"),
    ];
    for (i, s) in srcs.iter().enumerate() {
        for banned in ["bookmark", "post_view", "search", "reading_progress"] {
            assert!(!s.contains(banned), "device source[{i}] references reader data: {banned}");
        }
    }
}
```

**G4 — raw signals must never be persisted.**
Rots silently: a debugging commit adds `user_agent TEXT` or `renderer TEXT` to `device_records` and the
retention ladder ("raw signals never stored") is a lie.

```ts
it("the device payload wire shape carries no raw values", async () => {
  const collector = new DeviceCollector(async () => FIXTURE_SIGNALS);
  const wire = JSON.stringify(await collector.payload());
  for (const raw of [FIXTURE_SIGNALS.platform.platform, "1024", "8"]) {
    expect(wire, `raw value leaked into the wire payload: ${raw}`).not.toContain(raw);
  }
});
```

```rust
#[test]
fn device_schema_has_no_raw_signal_columns() {
    let m = include_str!("../migration/src/m0000NN_device_identity.rs");
    for banned in ["user_agent", "raw_signals", "renderer TEXT", "screen_width", "plugins TEXT"] {
        assert!(!m.contains(banned), "device_records must not carry raw column: {banned}");
    }
}
```

Also assert the reused `audit_logs.user_agent` column stays NULL for device evidence rows (the model has that
column, `sea_models/audit_log/model.rs:14`, and writing it would violate Layer 0).

**G5 — no device data in the disk-cache key, and the cacheable predicate keeps `!requestHasCookie`.**
Rots silently: cache keyed by device → cache fragmentation and a device-correlated store of rendered HTML.

```ts
it("the SSR disk cache stays device-free and cookie-gated", () => {
  const cache = read("src/lib/server/quran-disk-cache.ts");
  expect(cache).not.toMatch(/device|deviceKey|signal/i);
  expect(cache).toContain(
    "export function diskCacheKey(sourceId: string, kind: DiskCacheKind, a: number, b?: number): string",
  );
  const hooks = read("src/hooks.server.ts");
  expect(hooks).toMatch(/key !== null &&\s*!requestHasCookie/); // the exact cacheable predicate
});
```

**G6 — offline caches and the sync outbox stay device-free.**
Rots silently: a device token inside `offline/pack.json` is shipped to every other profile on a shared device.

```ts
it("the offline pack and sync outbox carry no device token", () => {
  for (const rel of [
    "src/lib/offline/pack.ts",
    "src/lib/sync/outbox.ts",
    "src/lib/offline/offline-store.svelte.ts",
  ]) {
    expect(read(rel), `${rel} must not reference device identity`).not.toMatch(
      /device|deviceToken/i,
    );
  }
});
```

## 4. Migration testing (the device migration)

What the repo's precedent demands (all three patterns exist; the new migration should combine them):

- **`m000003_translation_popularity.rs`** — the canonical shape: `manager.has_table(...)` idempotency, `down`
  drops, and an inline `#[cfg(test)] mod tests` with `mem_db()` (`sqlite::memory:`, `max_connections(1)`),
  `PRAGMA table_info` column assertions, a "defaults" insert (id-only row takes defaults), `up` twice, `down`.
- **`m000006_email_code_unique_user.rs`** — the _UNIQUE-on-populated-table_ pattern: `DEDUPE_*` statements run
  **before** `CREATE UNIQUE INDEX IF NOT EXISTS`, plus `dedupes_and_creates_unique_index` (assert the second
  duplicate insert errors) and `migration_is_idempotent`.
- **`m000008_bookmark_verse_unique.rs`** — the full-fat version: fixture tables mirroring the prior migration,
  `index_exists` via `PRAGMA index_list`, dedupe ordering tests (including ties), a "garbage rows survive"
  test, `down_drops_the_unique_index`, and **`full_migrator_run_creates_the_unique_index`** which runs the
  whole `Migrator::up` — this is the only test that catches a forgotten registration in `migration/src/lib.rs`.

**What breaks when a UNIQUE index is added to a table that already has rows:**
`CREATE UNIQUE INDEX` (SQLite cannot add a UNIQUE _constraint_ via `ALTER TABLE`) executes and **fails with
`UNIQUE constraint failed: <table>.<col>`**, which propagates out of `Migration::up`, which aborts
`Migrator::up`, which runs **at boot** — the API does not start. So:

- for a brand-new table (`device_records`, `device_sessions`) there are no duplicate rows _in that table_, but
  the m000005 precedent (out-of-band-provisioned DBs) means `up` must still be `has_table`-guarded and, if the
  table could pre-exist with rows, dedupe first — otherwise one stray duplicate from a partial earlier run
  bricks boot;
- `down` must be real: `DROP TABLE` for the new tables and `DROP INDEX IF EXISTS` for indexes added to
  existing tables (m000008's `down` only drops the index because the dedupe is irreversible — the new
  migration should say which parts are irreversible in the same comment style);
- register it: `mod m0000NN_…;` + `Box::new(m0000NN_…::Migration)` in `migration/src/lib.rs:3-56`, with the
  one-line "what it is for" comment the neighbours carry.

Minimum test list for the device migration (≈7 tests): fresh up columns+indexes; defaults; UNIQUE rejects a second
`device_key`; `device_sessions` UNIQUE(user_session_id) + FK; `up` twice; `down` drops table+indexes; full
`Migrator::up` creates the index.

## 5. The Rust HTTP harness (integration tests)

`src/lib.rs:20-21`:

```rust
#[cfg(test)]
pub mod test_utils;
```

Integration test binaries link `ruxlog` **without `cfg(test)`**, so `test_utils::{json_post, raw_post,
csrf_header_for_session}` do not exist for `tests/*.rs`. `static_csrf::compute_csrf_token` is `pub(crate)`
(`middlewares/static_csrf.rs:49`) — also unreachable. `csrf_signing_key()` additionally **panics** in a
non-`cfg(test)` build when `COOKIE_KEY` is unset (`static_csrf.rs:36-45`).

Three existing workarounds, all hand-rolled:

1. **Bootstrap CSRF over HTTP** — `tests/auth_rotation_header.rs:425-474` builds a router with the production
   layer stack and mints a session + token in one request:

```rust
fn login_router(state: AppState) -> Router {
    let cookie_key = tower_sessions::cookie::Key::derive_from(state.settings.cookie_key.as_bytes());
    let session_layer = tower_sessions::SessionManagerLayer::new((*state.session_store).clone())
        .with_secure(false).with_http_only(true).with_name("ruxlog.sid").with_private(cookie_key);
    Router::new()
        .nest("/auth/v1", auth_v1::routes())
        .route("/csrf/v1/generate", post(csrf_v1::controller::generate))
        .layer(middleware::from_fn(csrf_guard))
        .layer(middleware::from_fn(resolve_client_ip))
        .layer(Extension(axum_client_ip::ClientIpSource::CfConnectingIp))
        .layer(axum::Extension(state_ext))
        .layer(session_layer)
        .with_state(state)
}

async fn csrf_bootstrap(app: &Router) -> (String, String) {
    let res = app.clone().oneshot(
        Request::builder().method("POST").uri("/csrf/v1/generate").body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK, "csrf generate must succeed");
    let set_cookie = res.headers().get(header::SET_COOKIE).expect("generate sets a session cookie")
        .to_str().unwrap().to_string();
    let pair = set_cookie.split(';').next().unwrap_or(&set_cookie).trim();
    let bytes = to_bytes(res.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = json["token"].as_str().unwrap().to_string();
    (pair.to_string(), token)
}
```

2. **Forge the private cookie, skip CSRF** — `tests/admin_bans_export.rs:366-383`: derive the same
   `cookie::Key`, `jar.private_mut(&cookie_key).add(Cookie::new("ruxlog.sid", session_id))` after a real
   `AuthSession::login`. Valid for GET routes only.
3. **Full-app mirror** — `tests/quran_v1.rs:187-231` `full_app()` duplicates `main.rs` layering, with the
   load-bearing comment at lines 212-215:

```rust
// Extension(state) MUST be layered outer to origin_guard (mirrors main.rs):
// axum runs last-added-first, so this order makes the Extension insert visible
// to origin_guard's req.extensions().get::<AppState>(). Inner (as before) left
// the guard with no AppState -> it rejected every Origin header.
```

**Cost of this pattern:** each test file re-declares a ~115-line `AppState` literal + ~55-line `Settings`
literal + `quran_settings()` + a Quran store load. `tests/quran_v1.rs` is 1782 lines, largely for this reason.
For device tests the right move is a shared `tests/common/mod.rs` (a new file, since there is no existing
`tests/common/`) exposing `state()`, `app_with_device()`, `csrf_bootstrap()`, and `admin_cookie()` — otherwise
the device test file becomes the fifth copy of the same 170 lines and drifts.

## 6. Existing tests that break or must change (by file)

| File                                                                                   | What breaks                                                                                                                                                                                                                                                                                                                                                                           | Why                                                                                                                                                                                                                                                                                                                                                                                     |
| -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `rust/crates/ruxlog-types/src/error.rs`                                                | `status_code_map_is_total_and_stable` (asserts `expected_count == 55` **and** a full 55-row snapshot), `all_codes_have_non_empty_default_messages`, `status_code_mappings`, `serde_roundtrip`                                                                                                                                                                                         | Any new `ErrorCode` variant (device blocked / step-up required) must be added to the snapshot + count. `status_code()` is an exhaustive match, so the compiler catches the mapping first. Serde tokens are "client-matched — never renumber" (doc comment line 4).                                                                                                                      |
| `rust/backend/api/tests/quran_v1.rs`                                                   | `public_cors_table_covers_every_route` derives the expected route count from `include_str!("../src/modules/quran_v1/mod.rs").matches(".route(").count()`                                                                                                                                                                                                                              | Breaks **iff** a device endpoint lands under `quran_v1`. A separate module (e.g. `/device-identity/v1`) leaves it green — and that is the right call.                                                                                                                                                                                                                                   |
| `rust/backend/api/tests/quran_v1.rs`                                                   | `full_app()` / `app_over_public()` middleware-order mirror (lines 187-249)                                                                                                                                                                                                                                                                                                            | If `main.rs` grows a device middleware (or a header-strip layer), the mirror must be updated or the tests silently stop covering the new layer. Also asserts `SET_COOKIE` absent on public `/quran/*` — a device cookie on public routes breaks it.                                                                                                                                     |
| `rust/backend/api/src/modules/admin_bans_v1/controller.rs` (test mod, lines 254-440)   | `ban_unit_round_trips_through_json_without_url_encoding`, `ipv6_unit_truncates_host_bits_to_canonical_64`, `ipv4_unit_is_canonical_32`, `ban_unit_rejects_wrong_widths_and_non_ip_keys`, `filter_excludes_email_user_id_totp_and_fixed_rate_keys` (`rows.len() == 2`), `filter_dedupes_l1_and_l2_keeping_the_fresher_block`, `successful_delete_clears_l1_l2_history_and_fixed_count` | `BanUnit` is `struct BanUnit(IpNet)` (`dto.rs:18-48`). Turning it into an enum with a `Device` variant changes `parse`/`canonical`/`Serialize`; `filter_active_ban_rows` (`controller.rs:39-60`) **drops any key that does not parse as /32 or /64**, so a device ban keyed `quran-ban:{deviceKey}` would be invisible to list/export/unban — a banned device that cannot be un-banned. |
| `rust/backend/api/crates/rux-request-gate/src/layer.rs`                                | `canonical_unit_key_mirrors_ban_unit`; `flood_calls_on_first_block_once_per_window` and `escalation_none_preserves_exact_legacy_path` (fake-Escalation call counts: `active_bans == 10`, `observes == 5`)                                                                                                                                                                             | Adding a device escalation call site changes those counts; the IP mirror test must stay, plus a new device-side lockstep test.                                                                                                                                                                                                                                                          |
| `rust/backend/api/tests/admin_bans_export.rs`                                          | `valid_export_token_authorizes_export_route`, `export_token_does_not_authorize_delete_mutation`, `export_token_does_not_authorize_list_route`, `admin_session_fallthrough_authorizes_export_route`                                                                                                                                                                                    | New `/admin/devices*` routes must be proven to reject `BAN_EXPORT_TOKEN`; the export payload must keep round-tripping `banUnit` (if device bans enter it).                                                                                                                                                                                                                              |
| `rust/backend/api/tests/auth_round2_security_regressions.rs`, `fresh_auth_security.rs` | `user_session::Entity::find().count(...) == 1`, `auth.user.is_none()`                                                                                                                                                                                                                                                                                                                 | Unchanged today (they count `user_session`, not device tables), but a `device_records` write on the login path must not be asserted globally elsewhere; new device tests must count their own tables.                                                                                                                                                                                   |
| `web/src/lib/auth/__tests__/auth-client.test.ts` (+ `account-client.test.ts`)          | every call-count/index assertion (`toHaveBeenCalledTimes`, `mock.calls[2]`, the 5-URL array)                                                                                                                                                                                                                                                                                          | Safe if the device header is just added to the existing request (`toMatchObject` is used for headers). Breaks if the provider issues its own `fetch`.                                                                                                                                                                                                                                   |
| `web/src/lib/server/__tests__/server-headers.test.ts`                                  | `serves exactly the dynamic public vars declared in src/env.ts`                                                                                                                                                                                                                                                                                                                       | Any new `PUBLIC_*` var must be added to `web/src/env.ts` **and** `DYNAMIC_PUBLIC_ENV_VARS` in `web/server.ts`.                                                                                                                                                                                                                                                                          |
| `web/src/routes/(application)/__tests__/service-worker-*.test.ts` (6 files)            | each `vi.mock("../../../lib/workers/idb", …)` enumerates `IDB_VERSION`, `openIdb`, `idbGet/Put/Delete/Scan`                                                                                                                                                                                                                                                                           | Adding an export is fine; changing `openIdb`'s signature/version breaks all six mocks.                                                                                                                                                                                                                                                                                                  |
| `web/src/lib/quran/__tests__/catalogue-sha-guard.test.ts`                              | `SHA_FREE` file list                                                                                                                                                                                                                                                                                                                                                                  | If device code is placed in any listed file (e.g. `src/lib/quran/wire.ts`, `src/lib/data/quran-types.ts`), the sha ban applies. Keep device code out of the Quran tree; a device HMAC in the _web_ client is unrelated crypto but must not sit in a guarded file.                                                                                                                       |

Not affected: `web/src/routes/(application)/__tests__/nav-guard.test.ts` (reader URL grammar) and
`docs-divergence-guard.test.ts` — the device feature touches neither.

## 7. CI

- **Current state (master `53d3d421`):** `.github/workflows/images.yml` has three build jobs (`web`, `api`,
  `publish`) + `deploy`. It runs `bun run quran:check` and image builds. **No `bun run test`, no
  `cargo test`, no clippy, no fmt check.** The parent commit `37d90c41` had exactly those gates; `53d3d421`
  deleted them ("Web security gates", "Rust security gates", `bun audit`, `cargo-audit`).
- **What must change for this feature to be CI-verifiable:** restore the two gate blocks (the diff is a clean
  revert of `53d3d421`'s workflow hunks). Nothing else is missing for provisioning:
  - web job already runs `deploy/fetch-quran-db.sh` + `deploy/fetch-quran-db.sh catalogue` (needed: the
    sha-guard test reads `db/quran/translations/index.min.json`, and `vitest.config.ts` loads the
    `quranArtifacts()` plugin);
  - api job already caches + provisions `deploy/fetch-quran-db.sh all` (needed: `cargo test` opens the corpus
    through `CARGO_MANIFEST_DIR/../../../db/quran`).
- **New services / env vars:** none required for the tests as scoped. The device feature must not introduce a
  required env var for boot (a missing pepper must fail loud at boot, and the tests must set it explicitly —
  the `COOKIE_KEY` pattern).
- **`--test-threads=1`:** it does not exist in master CI (no test job). When restored it must be kept — the
  known global password semaphore serializes anyway. Timeout risk: GitHub's default job timeout is 360 min and
  the current suite fits, but **a new per-request DB write in the hot path is the real risk**: the pool is 1
  connection and tests are serialized, so every request that touches `device_records` blocks behind the single
  connection. A device upsert on every `/api/**` request would multiply the wall time of the existing
  1700-line `quran_v1.rs` + 9700 lines of integration tests. Mitigation: the device lookup must be L1-cached
  (the ban check already caches 5 min, `DESIGN-v0` line 9) and the cache must be injectable so HTTP tests can
  pre-seed it without the DB.
- **Web test job time:** `pretest` runs `i18n:check` (compiles 699 messages) on every `bun run test`; that is
  already the dominant fixed cost, not the new tests.

## 8. Minimum viable test suite

**New files: 11 (7 Rust, 4 web) + 2 extended; ≈70 tests.**

Rust:

1. `migration/src/m0000NN_device_identity.rs` (inline tests) — fresh up / defaults / UNIQUE / idempotent /
   down / full-migrator. ~7 tests.
2. `tests/device_identity_regressions.rs` — HMAC key vector + pepper isolation + component-order
   canonicalization; `device_records` UNIQUE + FK SET NULL (with `PRAGMA foreign_keys=ON`); `device_sessions`
   link; retention `cleanup(&db, now)`; the G3/G4 source scans. ~12 tests.
3. `tests/device_http_regressions.rs` — header accepted / garbage header rejected without 5xx / device ban
   blocks / step-up on new device / admin device routes reject the export token / admin device list+ban+unban
   round trip. ~8 tests.
4. `src/services/device_risk.rs` (or `src/modules/device_identity_v1/`) inline unit tests — thresholds
   79/80/81, 39/40/41, hysteresis pair, rule-version pin. ~8 tests.
5. `crates/rux-request-gate/src/layer.rs` + `ip.rs` — device identity call site, never-IP-key, IP counters
   untouched, mirror lockstep. ~5 tests.
6. `crates/ruxlog-types/src/error.rs` — updated snapshot + count (+1 variant). 0 new tests, 1 updated.
7. `tests/edge_headers.rs` — only if the verifier lands in-repo: HMAC/kid/replay boundary/strip middleware.
   ~6 tests.

Web: 8. `src/lib/device/__tests__/collector.test.ts` — injected `DeviceSignals`: bucketing, coherence, tamper,
missing-API no-throw (delete globals), payload contains no raw values. ~10 tests. 9. `src/lib/device/__tests__/token.test.ts` — safe-storage conventions, browser=false no-op, blocked-storage
no-throw, new IDB DB name, single-flight mint. ~6 tests. 10. `src/lib/auth/__tests__/auth-client-device.test.ts` — header attach on `unsafeRequest`/`get`, absent on
`exempt`, provider failure never blocks, no extra `fetch`. ~5 tests. 11. `src/routes/(application)/__tests__/device-guard.test.ts` — G1, G2, G5, G6. ~6 tests. 12. extend `src/lib/server/__tests__/quran-disk-cache.test.ts` — the device-free-key assertion (G5).

**Deliberately manual (with procedure):**

- _Real signal capture_ — deploy to staging; in Chrome/Firefox/Safari open the app, log only the payload's
  **field names and null-ness** (never values); confirm every field is present-or-null and no console error.
  Repeat with Brave (farbling) and Firefox RFP. Reason: no WebGL/WebGPU/UA-CH in happy-dom.
- _Turnstile step-up_ — staging with `PUBLIC_TURNSTILE_SITE_KEY`; log in from a fresh profile, complete the
  challenge, confirm the session is upgraded and the challenge is not re-asked inside the grant window.
- _Edge headers_ — deploy the Worker; `curl` the origin with (a) no headers, (b) a forged
  `X-Eq-Edge-Ip`, (c) a valid header captured from logs; confirm (a)/(b) are ignored and (c) is accepted;
  check `request.cf`/JA4 availability on the plan.
- _Device ban + appeal end-to-end_ — ban a test account, confirm the durable notice arrives, submit an appeal,
  admin decides, confirm unban propagates to the gate (L1 + L2) within one flush interval.
- _Retention_ — seed a row older than 30 days in staging, run the job once, verify the key is gone, the ban row
  survives, aggregates survive; then check production counts after 31 real days.
- _Proxy limits_ — capture a real request through CF→Traefik; assert `x-eq-device` is well under the header
  limit and that the API's CORS allow-headers accepts it.
- _Safari/ITP eviction_ — leave a profile idle >7 days; confirm the token is gone, re-minted silently, and the
  user is not challenged twice.
