# A4 — Web/SSR correctness collisions: DESIGN-v0 (device identity) vs the shipped delivery contract

Auditor: frontend/SSR correctness. Scope: `web/` + the web-relevant rust contract points.
Method: read the actual sources (no guessing). Every claim carries file:line.
Already-known items excluded (cookie bypass, IDB_VERSION pin, happy-dom gaps, nonce/wasm-unsafe-eval, nested-ternary, hotkeys dynamic-import).

---

## 1. Device token in localStorage/IDB does NOT break `/t/**` — but three specific shapes do

**Cache decision (verbatim).** `web/src/hooks.server.ts:346-351`:

```ts
const cacheable =
  event.request.method === "GET" &&
  !event.isDataRequest &&
  key !== null &&
  !requestHasCookie &&
  !event.url.pathname.endsWith(".md");
```

`requestHasCookie` is `!!event.request.headers.get("cookie")` (`hooks.server.ts:427`). The disk cache
itself (`web/src/lib/server/quran-disk-cache.ts:51-57`) keys on
`build-<appBuildId>__<sourceId>__<kind>__<index>[__<b>]` plus the `__ui-<locale>` suffix
(`hooks.server.ts:45-53`). It has **no request-header component and no `Vary` awareness**.

So:

- A device token in `localStorage` + IDB never enters a request → `/t/**` stays `cacheable`,
  hit path at `hooks.server.ts:361-372` intact, write path at `376-389` intact.
- **Break condition A:** the token becomes a cookie that reaches reader paths (`document.cookie = ...`,
  or an API cookie with `Path=/`) → `cacheable === false` (line 350) and `applyHeaders` forces
  `private, no-store` (`hooks.server.ts:130-137`). The predicate is cookie _presence on the request_, so a
  cookie scoped to `Path=/api` is never sent with `/t/**` and leaves the cache intact. (`docs/locale-suggest.md`
  §1 rule 2, "No cookies, ever", is that feature's own rule for client-written cookies.)
- **Break condition B:** SSR on a reader route ever _varies_ by a device header. Because the disk
  key and the SW page key are header-blind (see §4), the first writer's HTML is served to everyone
  else; a poisoned entry lives up to 7 days (`quran-disk-cache.ts:12`, TTL via `stat.mtimeMs`).
- **Break condition C:** the collector is _invoked_ from the reader path in a way that delays SSR?
  No — collection is client-side only; the risk is only client CPU (see §3).

**Design must avoid:** (1) any device cookie that reaches reader paths — if the token is a cookie it must be
server-set with an explicit `Path=/api` (never `Path=/`, never `document.cookie`);
(2) any server-side branch in `/t/**` (or any cached HTML path) that reads a device header/param;
(3) relying on `Vary` — `applyHeaders` only appends `Vary: Accept` for markdown/404
(`hooks.server.ts:148-154`), and the disk cache ignores `Vary` entirely. If the server ever needs a
device signal for a reader route, it must not render it into cached HTML.

**Guard:** assert `src/lib/device/**` contains no `document.cookie`/`Set-Cookie`, and that
`/t/**` HTML is byte-identical with and without the device header (fixture test in the
`service-worker-pending.test.ts` W8a style).

---

## 2. CSP: exact diff needed (and it is not the file you'd think)

Three CSP definitions exist and they are **not** identical:

| file                                     | nonce?                 | script-src                                                                                  | frame-src                                          | connect-src                                                         |
| ---------------------------------------- | ---------------------- | ------------------------------------------------------------------------------------------- | -------------------------------------------------- | ------------------------------------------------------------------- |
| `hooks.server.ts:59-101` (SSR)           | yes (`:81`)            | `'self' 'wasm-unsafe-eval'` + nonce + gstatic + gtm + **challenges.cloudflare.com** (`:84`) | `'self' https://challenges.cloudflare.com` (`:96`) | self + firebase + gtm + **challenges.cloudflare.com** (`:85`) + api |
| `server.ts:210-246` (prerendered/static) | no (hash tokens)       | `'self' 'wasm-unsafe-eval'` + hashes + gstatic + gtm — **no challenges.cloudflare.com**     | **absent** (falls back to `default-src 'self'`)    | self + firebase + gtm + api — **no challenges.cloudflare.com**      |
| `static/_headers` (static-host mirror)   | no (one hardcoded sha) | has `'wasm-unsafe-eval'` + hash + gstatic + gtm + challenges.cloudflare.com                 | `'self' https://challenges.cloudflare.com`         | includes challenges.cloudflare.com                                  |

Answers to the specific questions:

- **WebAssembly timing:** allowed. `'wasm-unsafe-eval'` is in all three (`hooks.server.ts:80`,
  `server.ts:230`, `static/_headers`). `performance.now()` arithmetic needs nothing. No diff.
- **WebGL / WebGPU / `navigator.userAgentData`:** no CSP directive exists for these. No diff.
  (`WEBGL_debug_renderer_info` is a WebGL extension, not CSP-gated.)
- **Worker / `OffscreenCanvas`:** no diff. `worker-src 'self' blob:` already ships
  (`hooks.server.ts:93`, `server.ts:239`, `static/_headers`), so both a module worker
  (`new Worker(new URL(...))`) and a blob worker are allowed. `OffscreenCanvas` is not CSP-gated.
  Only a _cross-origin_ worker script or `importScripts` of a remote URL would need `script-src`/
  `worker-src` changes.
- **New origin fetch (e.g. a wasm asset on another host):** add to `connect-src` in **all three**.
- **Inline script:** must be exactly `<script>` with no attributes. Nonce tagging is
  `html.replaceAll("<script>", ...)` (`hooks.server.ts:177-179`); hash scanning is
  `/<script>([\s\S]*?)<\/script>/` (`server.ts:160`, `src/lib/server/preview-headers.ts:3`).
  `<script type="module">` or `<script nonce=...>` escapes both → blocked on prerendered pages.
  Static hosts additionally require regenerating the manual sha in `static/_headers`
  (the comment block in `static/_headers` documents the one-liner); adapter-bun rescans at boot
  (`server.ts:171-203`), so `server.ts` needs no hash edit.
- **Concrete gap the design will hit:** Layer 3 L2 is "challenge (Turnstile)". `/login`,
  `/register`, `(account)` are `prerender = true` (`(auth)/+layout.ts:1`, `(account)/+layout.ts:1`),
  and prerendered HTML is served by `server.ts`'s static layer **before hooks run**
  (`server.ts:165-170`, `480-497`). `server.ts:210-246` has no `frame-src` and no
  `challenges.cloudflare.com` → a Turnstile challenge rendered on any prerendered auth/account page
  is CSP-blocked (script + iframe + frame→parent connect). Required diff:
  `server.ts` `buildCsp` must gain `https://challenges.cloudflare.com` in `script-src` and
  `connect-src` and a `frame-src 'self' https://challenges.cloudflare.com` line, and
  `static/_headers` must match. (Register already loads Turnstile via
  `TurnstileWidget.svelte` + `turnstile.ts:30` — the same gap is latent there today.)
- **Turnstile tokens in the collector's flow:** if the challenge is embedded in a _new_ route
  (e.g. `/challenge`), it must not be prerendered, or it must carry its own CSP-compatible shell.

---

## 3. First-paint path: the collector must never gate `ready`

Trace:

1. Root layout `onMount` (`web/src/routes/+layout.svelte:86-119`): hydrates stores, then latches
   first paint behind **double rAF** (`:114-119`) and only then boots the offline engine.
2. `(auth)/+layout.svelte:63-78`: `installPurgeHook`, `authState.hydrate({ force: true })`,
   `await authState.probe()`, optional `goto`, then `ready = true`.
   Children render only under `{#if ready}` (`:126-132`); otherwise the user sees
   "Checking your session…" (`:129-131`).
3. `(account)/+layout.svelte:13-27, 32-36` is the same shape.
4. `authState.probe()` (`web/src/lib/auth/auth-state.svelte.ts:87-99`) always issues a real
   `GET /api/user/v1/get`. `hydrate({ force: true })` can early-return when the root layout already
   hydrated (`:107`), but `probe()` is not gated by the session hint — so **every** `/login` cold
   load performs a network probe before the form renders.

Therefore: any `await` inside `AuthClient.#request` (`auth-client.ts:167-229`) for the collector
delays the probe → delays `ready` → delays the login form. Two failure shapes:

- await-before-`fetch`: `setTimeout(..., timeoutMs)` at `auth-client.ts:192` starts _after_ the
  collector, so the effective deadline becomes collector-time + 12 s;
- await-after-timer-start: a slow collector (WebGL context creation + WASM compile + timing loops)
  can trip the 12 s abort → `AuthTransportError` on a request that was never slow.

Also: the reader boot path already competes for main thread on mobile — `ensureOfflineEngine` +
`quranWorker` (`(application)/+layout.svelte:86-103, 147-158`). A WebGL/WASM timing loop fired
eagerly after first paint degrades INP on the very routes the disk cache serves.

**Correct sequencing (normative):**

1. `#request` reads a **synchronous, already-collected** payload (`deviceHeader()` reading an
   in-memory value backed by localStorage). No `await`, no import inside `#request`.
2. Collection is kicked off from the root layout **after** the existing double-rAF latch
   (`+layout.svelte:114-119`), via dynamic `import("#lib/device/collector.js")`, `browser`-gated —
   same pattern as the hotkeys registration (`(application)/+layout.svelte:160-182`).
3. Only the _security-relevant submit_ paths (login/register/2FA/passkey/credential change) may
   `await` the collector, with a bounded deadline (≤1.5 s) and a **send-without-payload** fallback.
   Server semantics: missing payload = "unknown device" (no step-up, no ban signal), never
   "new device".
4. The collector must not run on `/t/**` reader routes at all; gate on the auth/account surface set
   (inverse of the `NON_READER_FIRST_SEGMENTS` set at `+layout.svelte:38-63`) or on idle.

---

## 4. Service worker: `/api/**` is fully bypassed; the SW needs no device-token knowledge

- Non-GET returns immediately: `service-worker.ts:615` (`if (req.method !== "GET") return;`).
- Same-origin `/api/` GET bypass: `service-worker.ts:623`
  (`if (url.origin === sw.location.origin && url.pathname.startsWith("/api/")) return;`).
- Cross-origin API base (`PUBLIC_API_BASE_URL` absolute): never reaches `respondWith` — only
  same-origin non-`/api` paths fall to `swrApp` (`service-worker.ts:654-656`).
- Pinned by tests: `service-worker-api-bypass.test.ts:242-283` asserts `respondWith` is never
  called and no cache bucket receives `/api/` bytes, for GET and POST, any depth.

Consequence: a device payload attached to `/api/**` is **not cached, not replayed, not duplicated**
by the SW. No SW change is needed for transport. The SW must **not** be taught the token:
`PAGES_CACHE` navigation entries are keyed by `normalizeDataKey(url)` (`service-worker.ts:106-117`,
`674-716`) — pathname+search only, header-blind — so any device-header-dependent document would be
served wrong from cache. `isCacheable` refuses `private`/`no-store` (`:123-128`), which is what
keeps authenticated documents out today; that is a second reason the token must never become a
cookie on reader paths (any cookie on the request makes reader documents `private, no-store` per
`hooks.server.ts:136-137`; a `Path=/api` cookie is not sent there).

If offline device-announce is ever wanted, it is a **new SW responsibility**, not a free by-product
of the fetch path: new message type in `lib/offline/messages.ts` (union at `:21-32`), and a new IDB
database name because `IDB_VERSION` is pinned at 1 (`lib/workers/idb.ts:2`, one-store-per-db
invariant at `:19-31`). Recommend: do not queue device announces at all (see §8).

---

## 5. Prerender + hydration

`(auth)/+layout.ts:1` and `(account)/+layout.ts:1` are `export const prerender = true;`
(marketing and design layouts too). During prerender:

- `onMount` and `$effect` never run, so a collector invoked from them cannot execute at build time.
- A **statically imported** collector module whose top level touches `navigator`/`window`/
  `document`/`localStorage` will crash prerender (or the SSR render of `/login`). `svelte-check`
  will not catch it; the build will.
- Hydration mismatch: any device-derived value written into `$state` that markup reads **before**
  hydration completes produces a mismatch. `bun run check` runs with `--fail-on-warnings`, so a
  Svelte hydration warning is a build failure (AGENTS.MD; `package.json:25`).

**Correct guard:** `import { browser } from "$app/env"` inside the collector + dynamic
`import("#lib/device/collector.js")` from a client-only lifecycle hook, never a static import in any
`+layout.svelte` that renders on a prerendered route. All browser API access inside
try/catch + `browser` check (the `safe-storage.ts:5-13` mold). No device value may appear in SSR
markup on `/login`, `/register`, `(account)` — including "new device" notices, which must be
resolved client-side after the session probe.

---

## 6. i18n gate: exact files

Gates that run `i18n:check` before check/lint/test/build: `package.json:12` (`prebuild`),
`:24` (`precheck`), `:29` (`prelint`), `:33` (`pretest`). `i18n:check` = `check-i18n.mjs` +
`i18n:compile` + `i18n:namespaces` (`package.json:22`).

Rules enforced (`scripts/check-i18n.mjs:105-139`): per-domain en/ar key parity, same parameter
signature, non-empty values, and **no duplicate keys across domains**. Namespace generation
(`scripts/gen-message-namespaces.ts:87-108`) fails if any catalog key has no namespace claim, and
`--check` fails on stale barrels.

To add any user-facing copy (challenge screen, "new device" notice, appeal UI):

1. `web/messages/en.json` + `web/messages/ar.json` (general copy), or
   `web/messages/auth/en.json` + `web/messages/auth/ar.json` (auth-flow copy; the `auth` namespace
   is claimed by prefix `auth_` — `i18n-namespaces.json:126-130`).
2. `web/i18n-namespaces.json` — claim the new prefix or exact keys (e.g. `device_` or a new
   `device` namespace). Unclaimed keys fail generation.
3. `bun run i18n:namespaces` → regenerates `web/src/lib/i18n/m/*.ts` (auto-generated; never edit).
4. If the design adds a **new catalog domain** (e.g. `messages/device/{en,ar}.json`):
   `web/project.inlang/settings.json` (`pathPattern`), `scripts/check-i18n.mjs:4` (`DOMAINS`),
   `scripts/gen-message-namespaces.ts:21` (`CATALOGS`).
5. Copy must be resolved through a namespace barrel, never `#lib/paraglide/messages.js`
   (`message-barrel-guard.test.ts:42-61`).
6. A held/anonymous user's appeal route needs its own copy namespace + the durable notice text —
   same four steps.

---

## 7. `AuthClient.#request` attachment: all three risks are real

(a) **CSRF bootstrap.** `fetchCsrf()` is `#request("POST", "/csrf/v1/generate")`
(`auth-client.ts:231-238`), and `unsafeRequest()` auto-bootstraps before the first unsafe call
(`:257-260`). A payload attached inside `#request` therefore rides the CSRF request — every
anonymous visitor's first unsafe action would mint a `device_records` row (GDPR/abuse surface and a
useless row for a visitor who never authenticates).

(b) **Before the collector has run.** `getUser()` → `#request("GET", "/user/v1/get")`
(`auth-client.ts:269-283`) is fired by `authState.probe()` from `(auth)/+layout.svelte:66` and
`(account)/+layout.svelte:16`, plus the root-layout hydrate path when a session hint exists
(`auth-state.svelte.ts:106-114`). A payload built "if missing" inside `#request` would be attached
to that probe before collection finished.

(c) **Timeout / rotation.** The 12 s timer starts at `auth-client.ts:192`, before `fetch` at `:196`.
Awaited collection before that line extends the real deadline; awaited collection after it can trip
the abort. The rotated path (`:211-227`) re-enters `#request` via `fetchCsrf()` while
`#refreshing` guards only the CSRF refetch — a collector invocation inside `#request` can run twice
per rotated response. Also, the retry is only triggered on `res.ok` (`:218`), so a payload-induced
abort skips the recovery path entirely.

**Correct ordering and fallback (normative):**

- Attach via a **header** (`x-eq-device: <token>.<ts>.<sig>` or similar), never by merging into
  `opts.body` — DTOs are strict (`quran_v1/dto.rs` uses `deny_unknown_fields`; auth/passkey bodies
  are asserted in `passkey-flow.test.ts:321`, `register-flow.test.ts:246-285`) and the FCM endpoint
  `/device/v1/register` already takes `{token, expected_user_id}` (`messaging.ts:136-152`).
- Attach only on an explicit path allowlist: login, register, 2FA verify, passkey, password/email
  change, step-up. **Never** on `/csrf/v1/generate`, never on `/user/v1/get`.
- Header value comes from a synchronous cache; collection never awaits inside `#request`.
- Cross-origin deployments: a new request header needs
  `rust/backend/api/src/main.rs:799-805` `allow_headers` (currently only `csrf-token`, `accept`,
  `content-type`, `accept-encoding`, `content-encoding`); add `expose_headers` only if the server
  answers with a device header. Same-origin `/api` (the normal prod/dev path —
  `vite.config.ts:97-102`, docker `strip-api`) does not preflight, so this bites only when
  `PUBLIC_API_BASE_URL` is absolute.
- Fallback: no payload ⇒ request proceeds unmarked; server treats it as unknown-device.

---

## 8. Offline/outbox: a queued device payload replays stale, forever

`Outbox.enqueue` persists `{id, domain, payload, queuedAt}` (`lib/sync/outbox.ts:275-279`;
decoder `:36-46`). There is **no expiry field** and the engine deliberately never drops work:
"data stays queued forever; only `lastError` surfaces the problem" (`lib/sync/engine.svelte.ts:55-61`),
replay loop at `:296-354`. `SyncPausedError` (`lib/sync/types.ts:58-63`) is a _skip_, not a drop.

So: offline device payload enqueued at T, replayed at T+3 days → a device record/step-up computed
from a stale payload, possibly after a token rotation or on a different network.

**Required behavior:**

1. Preferred: never enqueue device payloads. The payload is cheap and always recomputable; send it
   on the next online request. (This also removes the need for any SW/outbox awareness — §4.)
2. If queued anyway: the domain's `sync()` must filter on `Date.now() - mutation.queuedAt > WINDOW`
   **before any network call** and treat expired mutations as drained (remove them), not failed.
   The design's ±30 s edge replay window (`DESIGN-v0` Layer 4) does not cover the client outbox.
3. Logout/account switch: `purge-hook.ts:19-24` purges caches only (`purgeUserCaches`), and the
   device domain must be explicitly cleared (`SyncEngine.clear(domain)`, `engine.svelte.ts:357-364`)
   on `logout`/`current-session-terminated` so a shared browser does not re-associate a device
   record with the next account.
4. The outbox DB is a second, unrelated IDB database (`easyquran-sync`, v2,
   `outbox.ts:9-19`) — do not reuse it for the device token store.

---

## 9. Tests: existing files to update + new guards

Existing (will need changes if the design lands as sketched):

| file                                                                            | why                                                                                                                                                |
| ------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `web/src/routes/(application)/__tests__/service-worker-pending.test.ts:377-504` | pins cookie→disk-cache bypass + `applyHeaders`; add device-header cases (anonymous + device header must still hit the disk cache)                  |
| `web/src/lib/firebase/__tests__/messaging.test.ts:32-44`                        | pins the exact `/api` sequence `csrf → device/v1/register → device/v1/delete` and the body shape; any new announce request or body field breaks it |
| `web/src/lib/auth/__tests__/register-flow.test.ts:246-285`                      | pins body inclusion/omission of `turnstile_token`                                                                                                  |
| `web/src/lib/auth/__tests__/passkey-flow.test.ts:321`                           | pins `device_type` body field                                                                                                                      |
| `web/src/lib/auth/__tests__/account-client.test.ts:51-57`                       | decodes session rows with a `device` field (name collision, §11)                                                                                   |
| `web/src/routes/(account)/account/__tests__/account-view.test.ts:102-122`       | `sessionDeviceLabel` UA behavior; a device-record UI next to it must not leak raw UA                                                               |
| `web/src/routes/(application)/__tests__/auth-cache-purge.test.ts`               | purge path gains the device token on logout                                                                                                        |
| `web/src/lib/auth/__tests__/route-guard.test.ts`                                | only if a challenge/held route is added to `route-guard`                                                                                           |

New guards (house "machine guard" style — raw-source assertions like
`nav-guard.test.ts:42-79` and `catalogue-sha-guard.test.ts:6-51`):

1. `web/src/lib/device/__tests__/device-cache-guard.test.ts`
   - `src/lib/device/**` source contains no `document.cookie`, no `Set-Cookie`, no `cookie` write.
   - the device module never imports `#lib/data/quran` reader helpers (no reader-path coupling).
   - `hooks.server.ts` source has no device-header read (regex over the source, the
     `catalogue-sha-guard` readFileSync pattern).
2. `web/src/lib/device/__tests__/device-transport-guard.test.ts`
   - `auth-client.ts` attaches the device header only from the allowlist; assert the literal
     `/csrf/v1/generate` and `/user/v1/get` call sites carry no device header (source regex +
     a unit test with a stubbed fetch, `messaging.test.ts:15-23` style).
   - assert no static import of `#lib/device/` in any `+layout.svelte` under
     `src/routes/**` (raw-glob scan like `nav-guard.test.ts:44-54`), i.e. dynamic-import-only.
3. `web/src/lib/device/__tests__/device-offline-guard.test.ts`
   - assert `src/lib/device/**` never calls `createOutbox`/`enqueue`; if a queue is added, assert an
     explicit `DEVICE_PAYLOAD_WINDOW_MS` constant exists and the sync fn filters on `queuedAt`.
4. `web/src/lib/device/__tests__/device-prerender-guard.test.ts`
   - assert every browser-API entry point in `src/lib/device/**` is behind a `browser` check
     (source assertion), and that prerendered layouts contain no device import.
5. Extend `docs-divergence-guard.test.ts` style: assert `documentation/quran-system.md` records "device
   payload never rides `/t/**`, no device cookie" so the contract can't silently drift.

---

## 10. Lint: which collector lines violate the anti-slop rules, and the house fix

Rules live in `web/vite.config.ts:31-62` (`vp lint --deny-warnings`, `package.json:30`).

| typical collector line                                                                    | rule (`vite.config.ts`)                                                                    | house-compliant rewrite                                                                                                                                                                                                                                                                                                     |
| ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `typeof navigator.webdriver === "boolean"`                                                | `anti-slop/no-runtime-typeof` (`:54`) — bans **every** `typeof`                            | feature-detect with `"webdriver" in navigator`; decode boundary values with `asString`/`asNumber`/`asObject` from `#lib/storage/decoders.ts:16-46`; if truly at an I/O boundary, the sanctioned escape is `// eslint-disable-next-line anti-slop/no-runtime-typeof -- <reason>` (see `hooks.server.ts:114`, `outbox.ts:35`) |
| `function collect(raw: unknown): Fingerprint`                                             | `no-unknown-parameters` (`:56`)                                                            | take a named domain param; for a boundary decoder, one disable comment with the reason (outbox `decodeMutation` pattern, `outbox.ts:35-46`)                                                                                                                                                                                 |
| `function evidence(): unknown` / `Promise<unknown>`                                       | `no-unknown-returns` (`:57`)                                                               | return a named payload interface (`DevicePayload`) or `DevicePayload \| null`                                                                                                                                                                                                                                               |
| `const bag: Record<string, unknown> = {}`                                                 | `no-unsafe-dictionary-type` (`:59`)                                                        | explicit interface with typed optional fields; `Record<string, number>` / `StringRecord` / `NumberRecord` are safe (`decoders.ts:9-13`)                                                                                                                                                                                     |
| `const gpu = navigator as Navigator & { gpu?: GPU }`                                      | `require-safety-comment-for-type-assertion` (`:61`) + `no-chained-type-assertions` (`:47`) | one assertion with `// SAFETY:` immediately above (the `hooks.server.ts:110-115` mold); prefer a guard function that returns the narrowed type                                                                                                                                                                              |
| `renderer.includes("Apple") ? "apple" : renderer.includes("Adreno") ? "adreno" : "other"` | `no-nested-ternary` (`:35`) + `unicorn/no-nested-ternary` (`:36`)                          | lookup table or a named function with early-return `if`s (AGENTS.MD rule)                                                                                                                                                                                                                                                   |
| `{ ...(flag ? { a: 1 } : {}) }`                                                           | `no-conditional-empty-object-spread` (`:48`)                                               | explicit field assignment after the conditional                                                                                                                                                                                                                                                                             |

Also: the collector module must not be a top-level side-effect importer — `no-floating-promises`
(`:45`) means every collection promise is `await`ed or `void`ed with a `.catch`, and
`no-misused-promises` (`:46`) bans async callbacks where a `void` handler is expected.

---

## 11. Naming collision: "device" already means the FCM push token

- Existing API surface: `/device/v1/register`, `/device/v1/delete`
  (`rust/backend/api/src/router.rs:167`, consumed at `web/src/lib/firebase/messaging.ts:136-152`).
- Existing client field: `user_sessions.device` (UA-derived) read by
  `account-client.ts:133-136` and rendered by `sessionDeviceLabel` (`account-view.ts:57-61`).

The design's `device_records` / `device_sessions` / `/admin/devices` must use a distinct vocabulary
(e.g. `client_records`, `/identity/v1/...`) or every future reader will conflate a fingerprint
identity with a push subscription. Rename now, before the device migration is written.

---

## 12. Held-session appeal UI and the prerender/ready gates

If the design's L4 "hold for review" or an appeal notice must be visible, note:

- `(account)/+layout.svelte:32-36` gates all children behind `ready`, which is set only after the
  session probe (`:13-23`). A notice rendered inside the gate is absent from the prerendered HTML
  and invisible while status is `unknown`.
- The precedent is explicit: `(auth)/+layout.svelte:121-124` moved `main#main` **outside** the
  `ready` gate because prerendered auth pages otherwise failed the build (`no element with id
"main" exists`), and the footer links at `:133-147` were deliberately placed outside the gate for
  the same reason.
- A held/banned session may have no usable session at all → the appeal route must render
  unauthenticated, must not depend on `authState.probe()` succeeding, and must not be prerendered
  with user-specific content. That means a new route + its own copy namespace (§6) + a
  `route-guard` entry (§9).

---

## 13. Docs: the cited non-negotiables live at a different path (and are machine-guarded)

The design cites `docs/plan/09-locale-suggest.md`. That path does not exist in this repo:
`docs/plan/` is gone; the compacted doc is `docs/locale-suggest.md`, whose "No cookies, ever"
rule is §1 rule 2.
Additionally, `docs-divergence-guard.test.ts:96-151` fails the test run if any doc under `docs/`
claims translated reader routes are prerendered/SSG, so the device design must not describe
`/t/**` as prerendered when it lands. Update `documentation/quran-system.md` (Part 5 divergence record) and
`docs/locale-suggest.md` §7's amendment list in the same change as the device layer.

---

## Severity summary

| #   | finding                                                                     | severity    |
| --- | --------------------------------------------------------------------------- | ----------- |
| 1   | cache contract: cookie or header-variant breaks/bypasses `/t/**` disk cache | high        |
| 2   | CSP: prerendered CSP lacks Turnstile origins; inline-script tagging rules   | high        |
| 3   | collector must not gate the auth `ready` probe / first paint                | high        |
| 7   | `#request` attach: CSRF, probe, timeout, rotation + CORS allowlist          | high        |
| 8   | stale device payload replays forever via outbox                             | medium-high |
| 5   | prerender crash / hydration mismatch                                        | medium-high |
| 4   | SW is safe; header-blind caches must not see device-varied HTML             | medium      |
| 6   | i18n gate: exact files to edit                                              | medium      |
| 9   | tests: existing + new guards                                                | medium      |
| 10  | anti-slop lint rewrites                                                     | medium      |
| 11  | "device" naming collision with FCM push tokens                              | medium      |
| 12  | held-session appeal UI vs ready/prerender gates                             | medium      |
| 13  | stale doc path + doc guards                                                 | low         |
