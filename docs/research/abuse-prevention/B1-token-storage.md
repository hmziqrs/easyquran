# B1 — Q1 resolution: device token storage (HttpOnly `Path=/api` cookie vs localStorage+IDB)

Auditor: storage/transport correctness. Date: 2026-10-12. Repo: `/Users/hmziq/os/easyquran`.
Inputs: `DESIGN-v1.md` (Q1, line 69), `A1-threat-model.md` (§4b + F4), `A4-web-collisions.md` (§1, §4, §7, §8),
and the repo sources cited inline. Method: read the code, no guessing. Every claim carries file:line.

**Verdict: cookie wins.** `ruxlog.device` — `HttpOnly; SameSite=Lax; Secure; Path=/api; Max-Age=2592000`
(30 days). The cache-safety objection to cookies is false for a path-scoped cookie (§1), and the localStorage
option's only claimed advantage is already forfeited by the shipped `ruxlog.sid` cookie (§2).
The losing option's strongest surviving argument is the cross-site API base (§5) — real, but it is
not the shipped topology and it is guardable.

---

## 1. If the token is a `Path=/api` cookie, does it reach `/t/**`, and does the cache survive?

**No and yes.** Full trace.

### 1a. The cache decision reads cookie _presence on the request_, not cookie name or path

`web/src/hooks.server.ts:346-351`:

```ts
const cacheable =
  event.request.method === "GET" &&
  !event.isDataRequest &&
  key !== null &&
  !requestHasCookie &&
  !event.url.pathname.endsWith(".md");
```

`web/src/hooks.server.ts:427`:

```ts
const requestHasCookie = !!event.request.headers.get("cookie");
```

So the decision is a bare presence test on the request's `Cookie` header. Path is never read —
**which is exactly why the browser's path-matching decides the outcome.** A cookie the browser does
not attach cannot be seen.

`applyHeaders` then forces the response private (`hooks.server.ts:130-137`):

```ts
const privateMode = requestHasCookie || responseSetsCookie(response);
...
if (privateMode && !isImmutableAsset) {
  response.headers.set("Cache-Control", "private, no-store");
}
```

### 1b. The disk cache key has no request component

`web/src/lib/server/quran-disk-cache.ts:51-57`:

```ts
export function diskCacheKey(sourceId: string, kind: DiskCacheKind, a: number, b?: number): string {
  const parts = [`build-${appBuildId}`, sourceId, kind, String(a)];
  if (b !== undefined) parts.push(String(b));
  return parts.map((part) => sanitizeComponent(part)).join("__");
}
```

plus the UI-locale partition `hooks.server.ts:45-53` (`${diskCacheKey(...)}__ui-${uiLocale}`). No
header, no `Vary` awareness. TTL is 7 days (`quran-disk-cache.ts:12`). Hit path `hooks.server.ts:361-372`,
write path `:373-392`.

### 1c. A `Path=/api` cookie is never attached to a reader request

The browser attaches cookies by RFC 6265 path-match against the request URI it issues. A cookie with
`Path=/api` matches `/api` and `/api/…`, and nothing else. `GET /t/en/sahih/juz/5` or
`GET /al-baqarah/t/en/sahih` does not match → the cookie is not in that request's `Cookie` header →
`requestHasCookie === false` → `cacheable === true` → disk cache intact.

### 1d. The `/api` cookie would not even reach `hooks.server.ts`

`hooks.server.ts` only ever handles requests routed to the **web** container. `/api/**` is routed to
the api container by Traefik and the prefix is stripped before it arrives
(`docker-compose.yml:184-192`):

```yaml
- "traefik.http.routers.api.rule=Host(`${DOMAIN}`) && PathPrefix(`/api`) && !PathPrefix(`/api/healthz`)"
- "traefik.http.routers.api.middlewares=strip-api@docker"
- "traefik.http.middlewares.strip-api.stripprefix.prefixes=/api"
```

Dev mirrors it in `vite.config.ts:97-102` (proxy `/api` → `127.0.0.1:8888`, `rewrite` strips `/api`).
There is no `/api` route anywhere under `web/src/routes/` (verified by directory listing). Server-side
SSR talks to the API container directly (`INTERNAL_QURAN_API_BASE: http://api:8888/quran`,
`docker-compose.yml:63`) and sends a service token, not cookies (`web/src/lib/server/quran-translation-page.ts:34-51`).

So there are two independent reasons the device cookie cannot affect the reader cache: the browser does
not send it, and the SSR server never receives that request class at all.

### 1e. The rule is "no cookie on reader paths", not "no cookie"

"A device cookie breaks the reader cache" is true only for a cookie the browser attaches to reader
requests — `document.cookie = ...` or any `Path=/` cookie — and false for a `Path=/api` cookie. The
cache-protection guards (no `document.cookie` in `src/lib/device/**`, no device header or device-cookie read in
`hooks.server.ts`) should still ship; they protect exactly the reader-path variant.

---

## 2. Does the browser honor `Path=/api` given the API is stripped by Traefik?

**Yes. The strip is invisible to the browser.** Cookie Path is matched by the browser against the URL
the browser requests (`https://easyquran.fyi/api/csrf/v1/generate`), not against what the origin sees
after Traefik rewrites. `strip-api` runs after the browser has already attached cookies and before the
API reads them. The API sees `/csrf/v1/generate`; the browser's jar sees `/api/...`. Both are correct
and independent.

**Confirmation from the repo's own `ruxlog.sid`.** It is `Path=/` and it _does_ ride reader requests:

- `rust/backend/api/src/main.rs:781-788` builds the session layer with `.with_name("ruxlog.sid")` and
  **no `.with_path(...)`** (grep: no `with_path` anywhere in `rust/backend/api/src`).
- tower-sessions 0.14.0 default is `path: "/"` and it always sets it —
  `~/.cargo/registry/src/index.crates.io-*/tower-sessions-0.14.0/src/service.rs:135-141`
  (`path: "/".into()`), `:108-110` (`Cookie::build(...).path(self.path)`), `:243`
  (`cookie.set_path(session_config.path)`), `:113` (`Expiry::OnInactivity(duration) => cookie_builder.max_age(duration)`).
- Result: `Set-Cookie: ruxlog.sid=…; Max-Age=1209600; SameSite=Lax; HttpOnly; Secure; Path=/`.

`Path=/` matches every path on the host, so `ruxlog.sid` **is** sent on `/t/**` and
`/{surah}/t/**` → `requestHasCookie === true` → disk cache bypassed. This is the already-known
"cookie bypass" (A4 preamble). Its consequence for Q1 is new and decisive: **the localStorage option's
entire cache-safety claim rests on a property the shipped app does not have.** The marginal cache cost
of adding a `Path=/api` device cookie is exactly zero; the marginal cost of a `Path=/` one is also zero,
because the cache is already gone for anyone holding a session.

(Latent trap, not currently live: `web/src/lib/components/ui/sidebar/sidebar-provider.svelte:32` writes
`document.cookie = \`${SIDEBAR_COOKIE_NAME}=…; path=/; max-age=604800\``— a`Path=/`cookie on reader
routes. It is dead code today:`setOpen` has no callers (`context.svelte.ts:39-41` `toggle()`only flips`openMobile`; `isMobile`is hardcoded`true`at`:24-26`). It is exactly the class of write the guard in
§7 must forbid outside `src/lib/device/**`.)

**The default-path trap is the real hazard.** If the API omits `Path`, the browser computes the
default-path from the request URI (RFC 6265 §5.1.4 step 4: the URI path up to, but not including, the
rightmost `/`). For a mint endpoint at `POST /api/csrf/v1/generate` that is **`/api/csrf/v1`** — the token
would then be attached only to `/api/csrf/v1/**` and silently absent from `/api/user/v1/get`,
`/api/bookmark/v1/sync`, etc. For a mint endpoint at `/api/device` (no trailing segment) it is `/` — the
cache-killer. `Path=/api` must be set explicitly, never inferred. (Note `strip-api` has
`forceslash=false`, `docker-compose.yml:192`, so `/api` alone stays `/api` — but that is the API's view,
not the browser's.)

---

## 3. XSS: read vs use

CSP `script-src` trusts `https://www.gstatic.com`, `https://www.googletagmanager.com`,
`https://challenges.cloudflare.com` (`hooks.server.ts:80-85`). A compromised script from any of them runs
first-party. What it can do to the token:

| Capability                                                                      | localStorage token         | HttpOnly cookie token    |
| ------------------------------------------------------------------------------- | -------------------------- | ------------------------ |
| Read the token value (`localStorage.getItem`)                                   | yes                        | no                       |
| Exfiltrate the value to an allowed `connect-src` host                           | yes                        | no                       |
| Copy it to another machine and present it there                                 | yes — full bearer transfer | no                       |
| Use it in-place: same-origin `fetch` to `/api/**` (cookies attached by default) | yes                        | yes                      |
| Mint/refresh device trust as the victim's browser                               | yes                        | yes                      |
| Persist the capability after the page unloads                                   | yes (value copied)         | no (needs re-compromise) |

The `connect-src` allowlist (`hooks.server.ts:61-88`) includes `google-analytics.com`, `firebaseio.com`,
`firebaselogging*.googleapis.com`, and `challenges.cloudflare.com` — so exfiltration has channels. That is
the whole quantified difference: **HttpOnly removes theft-and-transfer; it does not remove in-place use.**
A1 §4b says this ("becomes net-positive only with HttpOnly + rotation + additive-evidence-only
semantics"); the table above is the number behind it.

Two consequences the design must encode:

1. **Possession is never authority** (already DESIGN B4: "never an auth factor"). With in-place use
   possible, a token-driven trust bump must be additive evidence only, never a step-up bypass.
2. **The token must never be echoed in a response body.** Any endpoint that returns the token value
   re-creates the localStorage threat model inside an HttpOnly deployment. Guard in §7.

Also note the asymmetry that makes the cookie strictly better here: the _session_ cookie is already
HttpOnly, so a script that can read `ruxlog.sid` does not exist — the device token in localStorage would
have been the single most valuable script-readable credential on the origin.

---

## 4. Offline/PWA: does anything need to read the token while offline?

**No. Cookie is not disqualified; it is simpler.**

- The outbox stores payloads only: `{id, domain, payload, queuedAt}` (`web/src/lib/sync/outbox.ts:275-279`,
  decoder `:36-46`). No credential, no token. Its own IDB DB is separate (`outbox.ts:9-19`, `SYNC_DB_VERSION = 2`).
- The engine never drops queued work (`web/src/lib/sync/engine.svelte.ts:55-61`) and replays through the
  domain's `sync()`, which goes through `auth.unsafeRequest` (`web/src/lib/bookmarks/domain.ts:40-46`) →
  `AuthClient.#request` → `fetch(..., credentials: "include")` (`web/src/lib/auth/auth-client.ts:196-202`).
  The browser attaches the cookie at replay time. Replay is online by definition, so "read the token while
  offline" never happens.
- The service worker bypasses `/api/` entirely (`web/src/service-worker.ts:623`, non-GET at `:615`) and
  page/data keys are header-blind (`normalizeDataKey`, `:106-117`). Nothing to teach it.
- Logout purges caches only (`web/src/lib/auth/purge-hook.ts:19-24`); it has no token access and needs none.

Corollary the design should keep: A4 §8's "never enqueue device payloads" rule stands. With a cookie,
there is no payload to enqueue — the strongest form of that rule.

---

## 5. Cross-origin: `PUBLIC_API_BASE_URL` absolute

Shipped config is same-origin (`docker-compose.yml:69`):

```yaml
PUBLIC_API_BASE_URL: https://${DOMAIN}/api
```

and the client default is `/api` (`auth-client.ts:3-4`). In that topology the cookie is first-party and
nothing changes.

If the API base is ever an absolute **cross-site** URL (different registrable domain):

- `SameSite=Lax` is not sent on a cross-site `fetch`/XHR, so the device cookie simply never arrives.
- Fixing it requires `SameSite=None; Secure`, which makes it a third-party cookie: **Safari/ITP blocks
  third-party cookies by default**; Firefox Total Cookie Protection partitions them per top-level site;
  Chrome's third-party-cookie policy is policy-dependent and not something a security control should
  assume. `Set-Cookie` from a cross-site fetch response is likewise a third-party set and is blocked
  in the same places — the token would never even be stored.
- CORS itself is not the blocker: `allow_credentials(true)` and an explicit origin list are already set
  (`main.rs:813-815`), and the client already sends `credentials: "include"` (`auth-client.ts:201`).

An absolute **same-site** URL (e.g. `https://api.easyquran.fyi`) is fine for `SameSite=Lax` (SameSite
compares sites, not origins); only the truly cross-site case breaks. So: **cookie requires a same-site
API base — a deployment constraint to guard, not a reason to pick localStorage.** If a cross-site base is
ever required, that deployment must switch the token to a request header (`allow_headers` at
`main.rs:799-805` currently lists only `csrf-token`, `accept`, `content-type`, `accept-encoding`,
`content-encoding` — a header transport needs that list extended; the cookie path needs no change).

One transport bonus for the cookie: no request header is added, so no `allow_headers` change and no
preflight change anywhere.

---

## 6. Lifetime and rotation

Current primitives:

- `ruxlog.sid`: `Expiry::OnInactivity(24 * 14 hours)` → `Max-Age=1209600`, `SameSite=Lax`, `HttpOnly`,
  `Secure` gated on `settings.http.cookie_secure` (`main.rs:779-788`; `COOKIE_SECURE` defaults true and
  prod refuses false — `config/settings.rs:221-229`), `Path=/`.
- Anonymous session: `POST /csrf/v1/generate` inserts a marker and calls `session.save()`, minting
  `ruxlog.sid` for a visitor with no account (`rust/backend/api/src/modules/csrf_v1/controller.rs:10-25`).
- Login rotates the session id: `self.session.cycle_id()` (`crates/rux-auth/src/session/extractor.rs:101`,
  and `:121` for `login_with_metadata`), which re-issues `ruxlog.sid` and flips the client into a CSRF
  refetch via `x-eq-session-rotated` (`auth-client.ts:211-227`).

Rules for the device cookie:

1. **Independent of the session.** tower-sessions manages exactly one cookie (`ruxlog.sid`); `cycle_id()`
   cannot touch a differently named cookie. Do not fold the device token into the session payload — it
   would then inherit `cycle_id()`, logout teardown, and the 14-day inactivity clock.
2. **Persistent, not a session cookie.** A session cookie dies at browser close, resetting device identity
   constantly — record churn and "new device" noise, the exact failure A1 §4e(2) warns about. `Max-Age=30d`
   aligned to the design's 30-day keyed-record retention (DESIGN B3), so the cookie never outlives its
   record. No rolling refresh: bumping `Max-Age` per request is a hot-path write, and the record's
   `expires_at` is the authority anyway. On expiry, the server sees an unknown token → "unknown device",
   never "new device" (DESIGN B2).
3. **Do not rotate on login; keep the record.** Rotation has exactly two triggers: a fixed lifetime cap
   (re-write `token_hash` in place on the same `device_records.id`) and revoke. A per-login rotation races
   two tabs into two `token_hash` writes with one surviving cookie — an unknown device — and buys nothing,
   because the token is not a privilege (F5-token-lifecycle F3). Rotating the _record_ per login would be
   worse still: every login a new device. On each login, re-set the **same** cookie value with a fresh
   `Max-Age` (it rides the login response that already re-issues `ruxlog.sid`) so the cookie tracks the record.
4. **`/csrf/v1/generate` must not mint a device record.** It is CSRF-exempt
   (`static_csrf.rs:56-70`) and every anonymous visitor's first unsafe action passes through it
   (A4 §7a). Minting there creates a device row per drive-by and puts the mint behind an exempt path.
5. **Mint/refresh must be CSRF-guarded POSTs.** The guard exempts safe methods (`static_csrf.rs:92-98`)
   and a fixed list (`:56-70`). Because the browser now attaches the token automatically, a GET that
   mints or refreshes device trust would be forgeable by a cross-site navigation (`<img>`/`<a>`). Keep
   the device endpoints on POST + `csrf-token`.
6. **Logout does not clear it, and the web client cannot clear it anyway.** It is HttpOnly on the API's
   path; `purge-hook.ts` has no way to touch it. Revocation is the server-side `revoked_at` + revoke
   endpoint (DESIGN B4). This is a feature (account switch does not reset the device) that must be
   stated explicitly, because a reviewer will expect logout to clear it.

---

## 7. Decision, exact attributes, guards

### 7a. Attributes

```
Set-Cookie: ruxlog.device=<128-bit base64url token>; Max-Age=2592000; HttpOnly; SameSite=Lax; Secure; Path=/api
```

| attribute  | value                                              | why exactly this                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ---------- | -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| name       | `ruxlog.device`                                    | matches the `ruxlog.sid` namespace. Do **not** use a `__Host-` prefix — it requires `Path=/` (spec), which is the one thing that must not happen. A `__Secure-` prefix requires `Secure` unconditionally and would break LAN-http dev (the repo acknowledges that workflow: `outbox.ts:22`), so gate `Secure` on `cookie_secure` like `ruxlog.sid` and keep the plain name. If A4 §11's vocabulary rename lands first, use `ruxlog.client`. |
| `Path`     | `/api`                                             | the cache-safety invariant. Explicit, never defaulted (§2). If the public API prefix is ever configurable, this becomes a config value with a boot assertion that it equals the API base's path prefix.                                                                                                                                                                                                                                     |
| `Max-Age`  | `2592000` (30d)                                    | equals the 30-day keyed-record retention; token never outlives the record. Not a session cookie (§6.2).                                                                                                                                                                                                                                                                                                                                     |
| `SameSite` | `Lax`                                              | same-site (shipped same-origin) requests always carry it; cross-site does not, which is the desired fail-closed direction.                                                                                                                                                                                                                                                                                                                  |
| `HttpOnly` | on                                                 | removes theft-and-transfer (§3).                                                                                                                                                                                                                                                                                                                                                                                                            |
| `Secure`   | `cookie_secure` (default true; prod refuses false) | same gate as `ruxlog.sid` (`main.rs:779-788`, `settings.rs:221-229`).                                                                                                                                                                                                                                                                                                                                                                       |
| `Domain`   | absent                                             | host-only; do not widen. Accept that a sibling subdomain could shadow it with a `Domain=.easyquran.fyi` cookie — the server treats a bad token as unknown-device (fail-safe), and `__Host-` is unavailable here.                                                                                                                                                                                                                            |

### 7b. The losing option's strongest argument (stated fairly)

localStorage+IDB is the more robust transport in three ways, and they are real:

1. **Cross-site API base** (§5): a header-attached token works from any origin (given `allow_headers`);
   the cookie is blocked outright in Safari and partitioned in Firefox.
2. **Fail-safe by construction on the cache**: localStorage has no path attribute to get wrong. The
   cookie's failure mode is a silent one — omit `Path`, and the disk cache dies (or the token silently
   stops being sent, §2) with no error, only a perf/cost cliff and a broken device layer.
3. **No new cookie on an origin whose own docs say "No cookies, ever"**
   (`docs/locale-suggest.md` §1 rule 2; `documentation/quran-system.md:157` records the cookie→bypass rule).

Rebuttal, in order: (1) is not the shipped topology and is guardable (§7c-5); if a cross-site deployment
ever appears, it must change transport deliberately — a documented, testable boundary, not a silent one.
(2) is inverted by what each failure costs: a Path mistake is a cache miss (performance, detectable by
guard §7c-1/§7c-4); a localStorage token is a transferable bearer credential on a CSP that trusts three
third-party script origins (security, undetectable). (3) is already violated by `ruxlog.sid` itself
(§2) — the rule's real content is "don't add cookies to _reader_ requests", which `Path=/api` satisfies.
Additionally, the cookie is _more_ durable than localStorage on the browsers A1 worried about: Safari
ITP's 7-day cap targets script-written storage; a server-set HttpOnly cookie is not script-written.

### 7c. Guards — the assertion, not the intent

**1. Rust: the attributes are what makes it safe** (in the device-token endpoint's `#[cfg(test)]`,
house style; `cookie` crate is already reachable via `axum_extra::extract::cookie`):

```rust
let raw = res.headers().get(http::header::SET_COOKIE).unwrap().to_str().unwrap();
let cookie = axum_extra::extract::cookie::Cookie::parse(raw.to_owned()).unwrap();
// Cache-safety invariant: the browser must never attach this cookie to /t/** or /{surah}/t/**.
assert_eq!(cookie.path(), Some("/api"));
// XSS invariant: no script can read the token value.
assert_eq!(cookie.http_only(), Some(true));
assert_eq!(cookie.name(), "ruxlog.device");
assert_eq!(cookie.same_site(), Some(axum_extra::extract::cookie::SameSite::Lax));
assert_eq!(cookie.max_age(), Some(Duration::days(30))); // cookie::time::Duration
assert!(raw.contains("Secure"));
```

**2. Web raw-source guard** (`web/src/lib/device/__tests__/device-cache-guard.test.ts`, the
`readFileSync(path.resolve(process.cwd(), rel))` pattern of
`web/src/lib/quran/__tests__/catalogue-sha-guard.test.ts:6-8`):

```ts
it("the device layer never writes a cookie and never touches the outbox", () => {
  for (const [file, src] of Object.entries(deviceSources)) {
    expect(src, file).not.toMatch(/document\.cookie/u);
    expect(src, file).not.toMatch(/set-cookie/iu);
    expect(src, file).not.toMatch(/createOutbox|\benqueue\b/u);
  }
});
it("the SSR cache decision never names the device cookie", () => {
  expect(read("src/hooks.server.ts")).not.toMatch(/ruxlog\.device/u);
});
it("the auth transport never attaches a device header", () => {
  expect(read("src/lib/auth/auth-client.ts")).not.toMatch(/x-eq-device/iu);
});
```

The second assertion is the load-bearing one: `hooks.server.ts` must keep making the decision from
generic cookie presence, so that a path-scoping mistake fails **closed** (cache bypassed) instead of
open (device-varied HTML served from a header-blind cache).

**3. Behavior guard, fail-safe direction** (extend the existing `W8a handle bypasses the SSR disk cache`
block, `web/src/routes/(application)/__tests__/service-worker-pending.test.ts:413-504`):

```ts
it("bypasses the disk cache if a device cookie ever reaches a reader request (fail-safe)", async () => {
  disk.cachedHtml = "<html>cached</html>";
  const res = await runHandle(
    { pathname: "/t/en/sahih/juz/5", cookie: "ruxlog.device=tok" },
    new Response("<html>fresh</html>", { headers: { "content-type": "text/html" } }),
  );
  expect(disk.reads).toBe(0);
  expect(res.headers.get("cache-control")).toBe("private, no-store");
});
```

This cannot test browser path matching (happy-dom drops the cookie header — see the comment at
`service-worker-pending.test.ts:99-101`), which is precisely why guard 1 is the real one.

**4. Deployment guard** (raw-source, `read("../../docker-compose.yml")`):

```ts
it("the shipped API base stays same-site so the device cookie is first-party", () => {
  expect(read("../../docker-compose.yml")).toMatch(
    /PUBLIC_API_BASE_URL: https:\/\/\$\{DOMAIN\}\/api/u,
  );
});
```

**5. Negative assertion for the transport rule**: no `web/src/routes/api/**` exists and
`hooks.server.ts` contains no device-cookie read (guard 2's second case) — i.e. the web server can never
be the place the token is minted or cleared.

---

## Findings

### B1-F1 — A `Path=/api` cookie does not break the reader cache; the cookie wins the cache argument

**Severity: high. NEW.**
Evidence: `web/src/hooks.server.ts:346-351` (cache decision), `:427` (`requestHasCookie` = bare presence),
`:130-137` (`private, no-store`); `web/src/lib/server/quran-disk-cache.ts:51-57` (key has no request
component); `docker-compose.yml:184-192` (Traefik routes `/api` to api:8888, strips the prefix);
`vite.config.ts:97-102` (dev proxy); no `/api` route under `web/src/routes/`.
Path is the variable that decides: a `Path=/api` cookie is never attached to a `/t/**` navigation — and
`/api/**` never reaches `hooks.server.ts` at all. Fix: adopt the cookie; keep A4's guard set, which
protects the reader-path variant.

### B1-F2 — The disk cache is already bypassed for session-holders, so localStorage's cache-safety claim is moot

**Severity: high. NEW (the implication; the bypass itself is the known item in A4's preamble).**
Evidence: `rust/backend/api/src/main.rs:781-788` (session layer, no `with_path`); no `with_path` in
`rust/backend/api/src` (grep); tower-sessions 0.14.0 `service.rs:135-141` (`path: "/".into()`),
`:243` (`cookie.set_path`), `:108-113`; `documentation/quran-system.md:157` ("Cookie-bearing or
session-setting responses bypass it").
`ruxlog.sid` is `Path=/` and rides every reader request, so the 7-day cache only serves cookie-less
visitors today. Adding a `Path=/api` device cookie changes nothing; the "keep the cache" argument for
localStorage defends a property that no longer exists. Fix (separate change, not Q1): if the cache is
meant to serve session-holders, set `.with_path("/api")` on the session layer — the SSR never reads
`ruxlog.sid` (only presence, `hooks.server.ts:427`) and internal SSR uses the service token.

### B1-F3 — `Path=/api` must be explicit; the browser default-path trap breaks the token or the cache

**Severity: high. NEW.**
Evidence: RFC 6265 §5.1.4 (default-path = request URI up to the rightmost `/`); the repo's mint points —
`rust/backend/api/src/modules/csrf_v1/controller.rs:10-25` (`POST /csrf/v1/generate` after strip);
`docker-compose.yml:191-192` (`stripprefix.prefixes=/api`, `forceslash=false`).
Omitting `Path` at `/api/csrf/v1/generate` scopes the cookie to `/api/csrf/v1` (token silently absent from
every other API call); at `/api/device` it scopes to `/` (cache-killer). Fix: hardcode `Path=/api` in the
`Set-Cookie`, and make it a config value with a boot assertion if the API prefix ever becomes
configurable. Guard: B1-F1's Rust assertion (`cookie.path() == Some("/api")`).

### B1-F4 — A `__Host-` prefix is unimplementable with `Path=/api`

**Severity: medium. NEW.**
Evidence: the `__Host-` prefix requires `Secure`, `Path=/`, and no `Domain`.
The prefix and the path scoping are mutually exclusive, so the design cannot have both. Fix: plain
`ruxlog.device` (or `__Secure-ruxlog.device` only if `Secure` is unconditionally on, which breaks LAN-http
dev — `outbox.ts:22`), `Secure` gated on `cookie_secure` exactly like `ruxlog.sid`; record the reasoning in
the endpoint so the next reader does not "fix" the missing prefix.

### B1-F5 — The cookie makes the device endpoints CSRF-relevant; mint/refresh must be guarded POSTs and the token must never be echoed

**Severity: medium. NEW.**
Evidence: `rust/backend/api/src/middlewares/static_csrf.rs:56-70` (exempt list incl. `/csrf/v1/generate`),
`:92-98` (safe methods exempt), `:106-113` (no session id → reject); `csrf_v1/controller.rs:10-25`.
With a cookie the browser attaches the token automatically, so any GET that mints/refreshes device trust
is forgeable by a cross-site navigation, and minting at the CSRF-exempt `/csrf/v1/generate` would create a
device row per drive-by. Fix: mint/refresh only on CSRF-guarded POSTs off the exempt list; never return the
token value in a response body (an echo re-creates the localStorage threat model).

### B1-F6 — Cross-site `PUBLIC_API_BASE_URL` breaks the cookie; the shipped topology is same-site and must be guarded

**Severity: medium. NEW.**
Evidence: `docker-compose.yml:69` (`PUBLIC_API_BASE_URL: https://${DOMAIN}/api`), `:15-16` (routing
comment); `auth-client.ts:3-4,201` (`/api` default, `credentials: "include"`); `main.rs:813-815`
(`allow_credentials(true)` + explicit origin list), `:799-805` (`allow_headers` — a header transport would
need an addition, the cookie does not).
Safari/ITP blocks third-party cookies; Firefox partitions them; `SameSite=Lax` is not sent cross-site.
An absolute _same-site_ base (subdomain) is fine. Fix: keep the same-site base, assert it in a deployment
guard, and state that a cross-site deployment must switch to a header-attached token.

### B1-F7 — Lifetime: independent persistent cookie, token rotation ≠ record rotation, anonymous session untouched

**Severity: medium. NEW.**
Evidence: `main.rs:779-788` (14-day inactivity session, `SameSite=Lax`, `HttpOnly`, `Secure` config);
`crates/rux-auth/src/session/extractor.rs:96-124` (`cycle_id()` on login, both paths);
`csrf_v1/controller.rs:10-25` (anonymous session mint).
Fix: `Max-Age=30d` aligned to the 30-day record retention, re-set (same value) at each login and never per
request, no `cycle_id()` coupling, no device record minted at `/csrf/v1/generate`, and no rotation on login —
only a lifetime cap (re-writing `token_hash` in place on the stable record) and revoke. Logout does not clear it (and the web client cannot — it is HttpOnly on
a path the web server never serves).

### B1-F8 — Offline is not a disqualifier: nothing reads the token offline, and replay attaches it automatically

**Severity: low-medium. NEW.**
Evidence: `web/src/lib/sync/outbox.ts:275-279` (payload-only records), `:9-19` (separate IDB DB);
`web/src/lib/sync/engine.svelte.ts:55-61` (never drops work), `:296-354` (replay loop);
`web/src/lib/bookmarks/domain.ts:40-46` (replay via `auth.unsafeRequest`);
`auth-client.ts:196-202` (`credentials: "include"`); `service-worker.ts:615,623` (non-GET and `/api/`
bypass); `purge-hook.ts:19-24` (logout purges caches only).
The token is needed only at request time, and request time is online. Fix: keep A4 §8's "never enqueue
device payloads" rule in its strongest form (nothing to enqueue with a cookie) and add the
`createOutbox|enqueue` raw-source assertion (guard 2).

---

## Severity summary

| #     | finding                                                                          | severity   | new?              |
| ----- | -------------------------------------------------------------------------------- | ---------- | ----------------- |
| B1-F1 | a `Path=/api` cookie does not break the reader cache; cookie wins                | high       | NEW               |
| B1-F2 | disk cache already bypassed by `ruxlog.sid` (`Path=/`) — localStorage claim moot | high       | NEW (implication) |
| B1-F3 | explicit `Path=/api` mandatory; default-path trap                                | high       | NEW               |
| B1-F4 | `__Host-` incompatible with `Path=/api`                                          | medium     | NEW               |
| B1-F5 | device endpoints become CSRF-relevant; no token echo                             | medium     | NEW               |
| B1-F6 | cross-site API base breaks cookie; shipped base is same-site                     | medium     | NEW               |
| B1-F7 | lifetime/rotation vs 14-day session, `cycle_id()`, anonymous mint                | medium     | NEW               |
| B1-F8 | offline needs nothing; replay attaches automatically                             | low-medium | NEW               |
