# E6 — Reader-path cost/availability: claim verification + phase-0 fix set

Auditor: E6 (verification of `B3-anonymous-abuse.md` F1–F12; execution of the phase-0 scope set by
`D2-v1-attack.md` F10 / §12 item 3). Date: 2026-10-12. Repo: `/Users/hmziq/os/easyquran`.

Method: every B3 claim re-read at source (`file:line` below), every load-bearing number re-measured,
plus read-only live probes of `easyquran.fyi` (headers + one manifest GET only). Nothing written to
prod. B3's own claims are treated as hypotheses, not inputs.

**Answer up front.** B3 is directionally right and its headline is real: **the web origin has no
limiter, the only internal SSR bucket is global, and one anonymous IP can turn translated reading
off site-wide.** Two corrections and two sharpenings change the fix set:

1. **Any cookie also kills the service-worker caches.** The cache predicate is "any cookie", and
   GA4's default-on `_ga` puts _every returning reader_ on the uncached path (B3 F2). The same predicate
   also disables the **service-worker caches** — a third cache. (Verified: `hooks.server.ts:427,130`; `service-worker.ts:123-128`;
   `consent.svelte.ts:16,70-80`.)
2. **The `.md` bucket needs a render-class signal (B3 F1 fix 3).** HTML and `.md` SSR call the _same_ API route
   (`/quran/sources/{id}/range`); the API cannot tell them apart. A render-class signal from the web
   is required before any `…-md` bucket can exist.
3. **The artifact fan-out serializes.** Production advertises 2 scripts → **380** HEADs (non-prod 382),
   and the memo mutex is held across the fan-out, so N concurrent requests run N _sequential_
   sweeps, not one (B3 F6).
4. **The failure is silent (B3 F1).** A 429 from the internal API does
   not surface as a 429; `loadTranslationRangeData` swallows it and the reader gets an **empty 200
   page** marked `x-eq-translation-pending`. Worse for observability (invisible in logs), better for
   UX, and it is why this outage can happen without anyone noticing.

One detail is load-bearing for the _cost model_: **translated `.md` is partly cached**. A 512-entry in-process range cache (`translation-range-cache.ts`) serves repeats of the
same `(sourceId, from, to)` with zero internal calls. The flood still works — it needs distinct
`(source, range)` keys, of which there are ~7.9 k ≫ 512 — but the per-request internal cost of a
`.md` crawl is lower than a per-request call.

---

## 1. The seven verification questions (answers)

### Q1 — Is the internal SSR bucket really global (one key for all translated pages)?

**Yes. Exact key: `ratelimit:internal-webssr:quran-v1`.**

- Key construction: `format!("ratelimit:{}:{}", id_key, path)` — `crates/rux-request-gate/src/layer.rs:278`.
- `id_key` for a trusted internal request is the fixed label `internal-webssr`
  (`layer.rs:239-243`; label from `RequestIdentity::InternalService(InternalServiceId::WebSsr)`,
  inserted at `middlewares/client_ip.rs:97-100`).
- `path` is `PathKey::Fixed("quran-v1")` — a constant, **not** the route
  (`middlewares/rate_limit.rs:75`). So every internal call to every `/quran/**` route shares one
  bucket.
- Limit: `state.settings.rate_limit.internal_requests_per_minute.max(1)`,
  `QURAN_INTERNAL_REQUESTS_PER_MINUTE`, default **600**, window 60 s
  (`rate_limit.rs:69-74`; `config/settings.rs:462`). Documented as policy, not measured capacity
  (`documentation/quran-system.md:277`).
- The identity is granted only by constant-time match of `x-easyquran-internal-token`
  (`client_ip.rs:92-106`); present-but-invalid falls through to external resolution and warns.
- The layer stack attaches it to the public Quran router only (`main.rs:910-912`), alongside the
  content and health layers; each self-skips via `applies`.

### Q2 — Can one anonymous IP really starve the translated reader site-wide?

**Yes — trace confirmed, with two corrections to the mechanism.**

Full path: browser → Cloudflare → Traefik (`docker-compose.yml:184-192`, `/api` prefix stripped,
priority 100) → web container → `web/server.ts` `handleRequest` → kit SSR → `+page.server.ts` /
`.md/+server.ts` → `loadTranslationRangeData` → `fetchRangeChunks` → internal base
`http://api:8888/quran` with `x-easyquran-internal-token` → `quran_internal_layer` → shared bucket.

- Confirmed: the web origin has **no limiter of any kind**. `web/server.ts` and
  `web/src/hooks.server.ts` contain zero rate/concurrency/byte-budget code (grep verified: the only
  matches are the offline-pack path regex and header names). Traefik adds only `compress=true`
  (`docker-compose.yml:104-110`).
- Confirmed: no per-route/per-source/per-ASN/concurrency limiter exists on the Quran content path —
  the content layer is `PathKey::Fixed("quran-v1")`, so all 29 public routes share one bucket per IP
  unit (`rate_limit.rs:57-63`; `quran_v1/mod.rs:11-53`).
- **Correction 1 (cost):** two caches sit in front of the internal call, so the attacker must
  enumerate distinct keys, not just repeat a URL:
  - disk HTML cache: key `build-{id}__{sourceId}__{kind}__{a}[__b]__ui-{en|ar}`, 7-day TTL, 256 MB,
    mtime-LRU (`quran-disk-cache.ts:11-13,51-57,162-211`; `hooks.server.ts:45-53`);
  - in-process range cache: 512 entries + single-flight, key `sourceId:from-to`
    (`translation-range-cache.ts:4,24-46,62-78`).
    Distinct attack keys: 378 sources × ~21 ranges ≈ 7.9 k ≫ 512 → every request misses. At 600/min
    the attacker exhausts the global bucket in one minute and keeps it pinned indefinitely.
- **Correction 2 (reader-visible effect):** `loadTranslationRangeData` catches every error and
  returns `ayahs: []` (`quran-translation-page.ts:199-211`); the page then renders an empty shell
  marked `x-eq-translation-pending: 1` + `noindex` + `no-store`
  (`reader-route-guards.ts:33-45`). So the site-wide effect is _every translated page renders
  empty_, not a 429 page — and nothing logs the swallowed 429.
- The circuit breaker does **not** rescue it: `QuranApiAvailability` treats any `FetchHttpError` with
  status < 500 as a _neutral_ failure and resets (`api-availability.ts:28-31,80-83`) — a 429 storm
  never opens the breaker.

### Q3 — Does any cookie really kill the disk cache? Is the anonymous CSRF cookie the default?

**Any cookie kills the disk cache AND the service-worker caches. The CSRF cookie is not the
default, but GA4's `_ga` is.**

- Predicate: `requestHasCookie = !!event.request.headers.get("cookie")` (`hooks.server.ts:427`) →
  `cacheable = … && !requestHasCookie && …` (`:346-351`) and
  `privateMode = requestHasCookie || responseSetsCookie(response)` → `private, no-store`
  (`:130-137`). It never reads cookie _names_.
- `ruxlog.sid` is minted by `POST /csrf/v1/generate` only (`csrf_v1/controller.rs:10-19`; session
  layer `main.rs:781-788`, default `Path=/` — no `with_path` anywhere in the API).
- **Is it minted on a first visit to a translated page? No.** `authState.hydrate()` is gated on the
  localStorage hint `eq.auth.session-hint` (`auth-state.svelte.ts:106-114`); `getUser()` is a GET and
  mints nothing; `unsafeRequest`/`ensureAnonymousSession` are only reached from auth flows, FCM
  registration and bookmarks sync (`auth-client.ts:231-260`; grep: no reader-path caller). So a
  _first-time_ reader has no cookie → cache works (live-confirmed below).
- **But GA4 guarantees a cookie for returning readers.** `initAnalytics()` runs unconditionally on
  every page boot (`firebase/analytics.ts:11-28`; `boot/analytics.ts:20-31`), consent defaults
  `analytics: true` (`consent.svelte.ts:16`) → `analytics_storage: granted` (`:70-80`) → gtag.js
  writes `_ga`/`_ga_<id>` on `easyquran.fyi`. Any `_ga` alone flips the predicate. `__cf_bm` (if Bot
  Fight Mode is on) does the same.
- **Third victim:** the service worker's `isCacheable()` rejects any response containing
  `private`/`no-store` (`service-worker.ts:123-128`), so navigations and `__data.json` are never
  stored in `PAGES_CACHE`/`DATA_CACHE` for cookie-bearing clients (`:674-716`). The site loses three
  caches with one header.
- Live proof (read-only, anonymous curl → no cookies): first GET `/t/en/sahih/page/7` returned
  `x-easyquran-quran-cache: miss` + `server-timing: …desc="miss"`; second GET returned `hit`. The
  disk cache is live in production and does work — for cookie-less clients only.

### Q4 — Is the offline pack really 14.1 MB and unauthenticated? Would Cloudflare cache it?

**Yes to size/auth. No to the edge cache — live-confirmed `DYNAMIC` on an immutable pack.**

- Generator: pack = every prerendered `__data.json`, `JSON.stringify({version,entries,bodies})`
  (`gen-offline-pack.ts:152-196`). Measured on this tree:
  `14,158,643` bytes raw, **`589,573`** bytes brotli, 2,879,498 gzip, 1,635 entries
  (`web/build/client/offline/manifest.json`); the deployed manifest reports the same
  `bytes: 14158643`, `entries: 1635`.
- Serving: `lateStaticResponse` for `/offline/*` (`server.ts:511-522`), `immutable =
packPattern.test(pathname)` → `serveFile` sets `public, max-age=31536000, immutable`
  (`server.ts:467`), `vary: accept-encoding` when a compressed sibling exists. No auth, no limiter.
- Live probe: `HEAD /offline/pack.1791752085271.json` →
  `cache-control: public, max-age=31536000, immutable`, `vary: accept-encoding`, and
  **`cf-cache-status: DYNAMIC`**. With `Accept-Encoding: br` the response is
  `content-encoding: br`, `content-length: 589573`. A client that omits `Accept-Encoding` (or sends
  `identity`) gets the 14.1 MB identity body (`negotiateEncoding` returns null, `server.ts:357-369`).
- So the A2 arithmetic stands (600/min × 14.1 MB ≈ 8.4 GB/min per IP worst case), and the cache rule
  B3 asks for is genuinely absent: no Cache Rule text exists anywhere in the repo
  (`deploy/README.md` mentions Cloudflare only for Turnstile, ingress ranges, mail, ban-export).

### Q5 — Is there really no edge-cache contract for `/api/quran/**`?

**Correct — no rule in the repo, and the live origin is uncached.**

- Response headers are correct at the origin: `ARABIC_CACHE = public, max-age=300, s-maxage=86400,
stale-while-revalidate=604800, stale-if-error=604800` (`quran_v1/cache.rs:5-8`), weak id-based
  ETags (`:16-27`). Live `GET /api/quran/surahs` confirmed exactly those headers **plus**
  `cf-cache-status: DYNAMIC` and a second `vary: origin, access-control-request-method,
access-control-request-headers` header from the CORS layer (B3 said "`Vary: Accept-Encoding` only"
  — it is `Accept-Encoding` + the CORS triple).
- Cloudflare does not cache `.json` by default; with no committed rule, `s-maxage=86400` is
  decorative and every content request reaches the origin, bounded only by 600/min/IP unit. Verified
  live on both `/api/quran/surahs` and the immutable offline pack.
- Error responses carry **no** `Cache-Control` at all (`error/response.rs:88-120`), so if a Cache
  Everything rule is added without also stamping `no-store` on errors, 4xx responses become
  edge-cacheable depending on the rule's TTL setting. That is a NEW finding (E6-F7).
- Live probes also confirm the reader HTML is uncached at the edge (`cf-cache-status: DYNAMIC`,
  `cache-control: no-cache`) and carries `vary: Accept` — the fragmentation B3 F4 describes.

### Q6 — Verify the HEAD fan-out

**Confirmed: 380 (prod) / 382 (non-prod), and it serializes.**

- `resolve_sources` builds one candidate per advertised script + every catalogue translation, then
  spawns a `JoinSet` of HEADs bounded by `Semaphore::new(32)`, each verified against
  `CONTENT_LENGTH == expected_size` (`controller.rs:945-1020`, `:875-891`).
- `advertised_script_count(Production)` = 2 (`Uthmani | SimpleClean`, `quran/store.rs:58-63`;
  `controller.rs:43-45`) + 378 catalogue entries (`db/quran/translations/index.min.json`, measured 378) = **380**. Non-production = 4 + 378 = 382. `/scripts` fans out 2 (prod).
- Trigger: the memo is all-or-nothing — `if resolved.len() == total { *guard = Some(…) }`
  (`controller.rs:915`; scripts `:821`). With any single unverified/missing/size-mismatched artifact,
  the guard never trips, so **every** request re-runs the full sweep and stamps `no-store`
  (`:924-928`).
- **Amplification:** the mutex guard is held across the `resolve_sources(...).await`
  (`controller.rs:901-920`), so concurrent requests serialize and _each_ performs its own sweep.
  N inbound requests → N sequential 380-HEAD sweeps through the shared `http_client` (also used by
  mail/billing/FCM, `:995-997`). The effective inbound rate collapses to ~1.5 req/s while this
  state persists, and 32-way concurrency saturates the shared client.
- Fix direction (B3's is right, sharpen it): a **per-artifact verified set** that grows, a retry
  floor so a permanently missing artifact is re-probed at most once per interval, single-flight
  (the mutex already serializes — make the waiters share the in-flight result instead of re-sweeping),
  `no-store` while partial, plus a cold-resolve counter.

### Q7 — Verify the translation-id cycling thrash

**Confirmed with a size correction.**

- Pool: Moka, `max_resident_translations = 8`, `max_resident_bytes = 48 MiB`,
  `translation_idle_ttl_secs = 1800` (`config/settings.rs:277-301`); single-flight build via
  `try_get_with` with a build semaphore (`quran/translation_pool.rs:228-297`); byte bound enforced
  after each build (`:275`).
- Catalogue sizes measured: min 790,528 B, median 1,327,104 B, p90 2,985,984 B, max 13,029,376 B;
  total 645,926,912 B (616.0 MiB). The **byte cap binds once large corpora are resident** — the pool holds ~4 entries for a 12 MB corpus mix, ~8 for typical 1.3 MB
  ones.
- One IP can enumerate all 378 ids through `/quran/sources/{id}/range` (or `/surah/{n}`) at
  600/min: 378 cold builds per cycle, each a disk read + parse, evicting each other; every request
  also increments the durable demand counter flushed every 6th tick (`main.rs:594-646`;
  `translation_popularity_store.rs`), inflating the popularity ranking with attack demand.

---

## 2. B3 F1–F12 verification table

Legend: **C** confirmed at source · **C\*** confirmed with correction · **W** wrong (with the
correct value) · **U** understated (real scope is larger).

| B3  | Claim                                                                                                                                               | Verdict | Correction / evidence                                                                                                                                                                                                                                                                                                                                                                   |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F1  | web origin has no limiter; one global 600/min internal bucket; one IP starves translated reading                                                    | **C**   | Key/limit/attach confirmed (Q1/Q2), including: (a) the attack needs distinct `(source,range)` keys because of the 512-entry range cache; (b) the reader-visible effect is an empty 200 pending page (Q2); (c) the `…:quran-v1-md` bucket needs a render-class signal — HTML and `.md` hit the same API route; (d) the web→API fallback bucket is _escalatable_, not just shared (E6-F5) |
| F2  | any cookie defeats disk + edge cache; `_ga` (every returning reader) and `ruxlog.sid` (after any `/api` call) make cookie-present the default state | **C\*** | Predicate confirmed verbatim (`hooks.server.ts:427,130,346-351`). Also: the SW caches die by the same predicate (`service-worker.ts:123-128`), and the CSRF cookie itself is _not_ minted on a first reader visit                                                                                                                                                                       |
| F3  | 14.1 MB unauthenticated pack, unlimited, `.json` not edge-cached                                                                                    | **C**   | Measured 14,158,643 / 589,573 / 1,635; serving path + headers confirmed; live `cf-cache-status: DYNAMIC` on the immutable pack                                                                                                                                                                                                                                                          |
| F4  | no committed edge-cache contract; permutation + `Vary: Accept` fragmentation                                                                        | **C**   | Contract absence + live DYNAMIC confirmed. Range permutations are 2 (`RangeTextQuery` has only `from`/`to`; junk params 400); `/ayahs?keys=` is unbounded (order permutations of ≤50 keys); API `Vary` is `Accept-Encoding` + the CORS triple. NEW: error responses carry no `Cache-Control` (E6-F7)                                                                                    |
| F5  | cheapest bulk path exists, undocumented, no contract/contact                                                                                        | **C**   | `RESPONSE_CAP=300` (`quran/store.rs:5`), `download_url` per source (`controller.rs:960,978-990`), 378 entries / 616.0 MiB measured, no bulk doc, `renderLlmsIndex()` has no bulk section (`seo/render.ts:53`)                                                                                                                                                                           |
| F6  | 380-HEAD fan-out (prod; 382 non-prod) on partial R2 state, serialized                                                                               | **C**   | 2 advertised scripts + 378 catalogue entries; memo mutex held across the sweep → N sequential sweeps for N requests; shared `http_client` saturation (`controller.rs:893-938,945-1020`)                                                                                                                                                                                                 |
| F7  | Turnstile is registration-only; no challenge/pass mechanism                                                                                         | **C**   | `services/turnstile.rs` single entry point; sole caller `auth_v1/controller.rs:470`; no pass-cookie code                                                                                                                                                                                                                                                                                |
| F8  | limiter key space is IP-unit-only; residential pool defeats it free                                                                                 | **C**   | `RequestIdentity` enum, canonical `/32` `/64` (`layer.rs:239-243,378-390`); no ASN/network dimension. D2 already cut this from phase 0                                                                                                                                                                                                                                                  |
| F9  | no harvest-pattern detection                                                                                                                        | **C**   | No sequential/variance/ETag-indifference logic; escalation is 4xx-history only (`config/settings.rs:306-378`)                                                                                                                                                                                                                                                                           |
| F10 | new limiter dimensions multiply the 10 s flush                                                                                                      | **C**   | `snapshot()` returns every live bucket (`store.rs:123-155`); flush upserts per row in one txn (`rate_limit_store.rs:180-229`). Any new _persisted_ dimension pays here; phase-0 limiters are web-side in-memory, so unaffected                                                                                                                                                          |
| F11 | one IP cycles 378 ids and thrashes the 8-entry/48 MB pool                                                                                           | **C**   | Config confirmed. Sizes are 0.77–12.4 MB (median 1.27, mean ≈1.6) → ~8 resident for typical corpora, ~4 once a large one is resident (byte cap). Demand-counter inflation confirmed (`main.rs:594-646`)                                                                                                                                                                                 |
| F12 | API robots.txt is ruxlog residue; no crawl policy for expensive variants                                                                            | **C**   | `router.rs:236-246` (`Sitemap: https://ruxlog.com/…`); `web/static/robots.txt` has `Disallow:` empty; `/offline/` crawlable                                                                                                                                                                                                                                                             |

**Not in B3, found here:** the SW caches die with the disk cache (F2 scope); the public-fallback bucket is the _escalatable_ one
(F1 secondary, sharpened); the pack is fetched with `cache: "no-store"` by the offline engine, so the
browser never reuses it; disk-cache stats exist and have no reader; error responses carry no
`Cache-Control`.

---

## 3. The ordered phase-0 fix set (WS7a, exact changes)

D2 §12 puts WS7a third in phase 0 (after gates WS0a and the truthful interim policy WS1a). The
ordering below is inside WS7a: cheapest-and-safest first, and every item is independent enough to
ship alone. Estimates are engineering-only.

### P0.1 — Restore the reader caches (highest value; do first) — 0.5–1 d

**Files:** `web/src/hooks.server.ts`, `rust/backend/api/src/main.rs`, new
`rust/backend/api/src/middlewares/auth_marker.rs`, `web/src/lib/server/__tests__/localized-reader-hook.test.ts`.

1. API: path-scope the session cookie.
   `main.rs:781-788` — add `.with_path("/api")` to `SessionManagerLayer`. The browser then never
   attaches `ruxlog.sid` to `/t/**` or any reader path. Verified safe: all browser→API traffic is
   under `/api` (Traefik strips the prefix only upstream of the browser), and the web SSR never reads
   `ruxlog.sid`. This is the first half of C2 §2's fix; the marker
   and the name-specific predicate below are the second.
2. API: mint an auth marker (the predicate must stay _correct_, not just permissive).
   New `middlewares/auth_marker.rs`, layered on the private router next to `private_no_store`:
   ```rust
   // rux_auth key is the tower-session payload written by rux-auth (SESSION_KEY = "rux_auth",
   // crates/rux-auth/src/session/extractor.rs:30). Presence == authenticated; no DB read.
   let authed = session.get::<serde_json::Value>("rux_auth").await.ok().flatten().is_some();
   // Set only on transitions: authed && request lacks ruxlog.auth=1 -> append Set-Cookie;
   // !authed && request has it -> expire it. Max-Age = 1209600 (session OnInactivity 14 d).
   // Attributes: HttpOnly; Secure; SameSite=Lax; Path=/
   ```
   One `Set-Cookie` per login and one per logout; zero churn on steady-state requests.
3. Web: name-specific predicate, not bare presence.
   `hooks.server.ts:427` →
   ```ts
   const AUTH_COOKIE_RE = /(?:^|;\s*)ruxlog\.auth=1(?:;|$)/u;
   const cookieHeader = event.request.headers.get("cookie");
   const requestHasAuthCookie = cookieHeader !== null && AUTH_COOKIE_RE.test(cookieHeader);
   ```
   Thread `requestHasAuthCookie` through `resolveRequest` (`:343,346-351,500,514`) and `applyHeaders`
   (`:118-137,541`). `privateMode = requestHasAuthCookie || responseSetsCookie(response)`.
   Every other cookie (`_ga`, `__cf_bm`, `ruxlog.sid` if the path change ever regresses, analytics
   junk) is ignored.
4. Keep `responseSetsCookie` as the second half of `privateMode` — it is the guard that keeps a
   credentialed response out of the shared cache if a future route ever sets one.
5. Drop `Vary: Accept` from plain HTML (see P0.6): `hooks.server.ts:152-154` and
   `server.ts:314-321` — remove the `mdSiblingRequest(pathname) !== null` clause, keep
   `textVariant`/404/negotiated responses.

**Why this is the fix for A1:** a disk-cache hit consumes **zero** internal budget and zero API
calls (`resolveRequest` returns before `resolve`). Restoring the cache converts the majority of
translated reader traffic from SSR to a disk read. It is a product-wide perf fix, not abuse work.

### P0.2 — Web-origin limiter on SSR **misses** + a global miss budget — 1–1.5 d

**Files:** new `web/src/lib/server/origin-limiter.ts`; `web/server.ts`; `web/src/hooks.server.ts`;
`docker-compose.yml`.

There is no middleware in `Bun.serve`; the repo's request pipeline is the single `handleRequest`
function passed as `fetch:` (`server.ts:718-789,845`). Two placement points, because they see
different sets of requests:

- `handleRequest` (every request): per-IP buckets for request classes — `reader` HTML 240/min,
  `md/txt` 120/min, `/_quran/*` 60/min, `/offline/*` byte budget (P0.5), plus a global in-flight cap
  around `kitServer.respond`. Immutable `/_app/immutable/*` and `/health/*` exempt.
- `resolveRequest` in hooks (only the SSR path, where `cacheable` is known): **the per-IP SSR-miss
  limit (60/min/IP) and the global internal-call budget** (`QURAN_SSR_MISSES_PER_MINUTE`, default
  300 < the API's 600 backstop). Increment only when about to `resolve()`; **hits never consume it.**
  This is what removes the CGNAT/school false positive: 100 readers behind one NAT hitting cached
  pages consume nothing.

Client identity: `cf-connecting-ip` **only when the TCP peer is private** (Docker network), else the
peer address; never `X-Forwarded-For` (Traefik's trust config is external and unverifiable here, and
CF's XFF semantics vary). Same trust model as the API's `IP_SOURCE=CfConnectingIp` + peer gate.
`ADDRESS_HEADER` is currently unset in compose (`docker-compose.yml:52-78`) so `getClientAddress`
returns Traefik's address — the limiter must not reuse it unguarded.

On exhaustion: cache hits always serve; misses get `503` + `Retry-After` (clearer than the current
empty 200 pending page) and a WARN line. In-memory, bounded LRU (50 k keys), no persistence, no DB.
Exempt the Docker healthcheck `/health/quran`.

### P0.3 — Split the internal bucket by class (needs a render-class signal) — 0.5–1 d

**Files:** `web/src/lib/server/quran-translation-page.ts` (+ the 8 reader route handlers),
`rust/backend/api/src/middlewares/rate_limit.rs`, `rust/backend/api/src/main.rs`,
`rust/backend/api/src/modules/quran_v1/mod.rs`.

1. Web: plumb `renderClass: "html" | "md"` from each route handler into
   `loadTranslationRangeData` / `loadTranslationSurahRouteData` → `fetchTranslationRange` →
   `serverJsonFetcher`, and send `x-eq-render-class: html|md` alongside the internal token
   (`quran-translation-page.ts:43-77`). `.md` handlers (`(application)/t/[lang]/[translator]/*.md`,
   `[surah]/t/[lang]/[translator].md`) pass `"md"`; `+page.server.ts` handlers pass `"html"`.
2. API: `quran_internal_layer` becomes three layers, all `applies(identity_is_internal)`:
   - `Fixed("quran-v1-md")` — internal calls carrying `x-eq-render-class: md` (allowlist only;
     absent/unknown → the default bucket). This is the bucket B3 F1 intended and cannot exist
     without the header.
   - `Fixed("quran-v1-range")` — `/sources/{id}/range` (bulk read, where a flood starves).
   - `Fixed("quran-v1")` — the existing global backstop, unchanged, applied last.
3. Keep the global backstop at 600 and raise it only after measuring real capacity
   (`documentation/quran-system.md:277` documents it as policy). The web's global miss budget (P0.2) is the
   primary control; the API buckets are damage-class separation.
4. Do **not** add these to persistence: they reuse the existing `PathKey::Fixed` mechanism on the
   in-memory gate store; the flush only sees the same key space as today (B3 F10 unaffected).

### P0.4 — Fail boot when the internal SSR env is missing (production only) — 0.25 d

**Files:** `web/server.ts`, `web/src/lib/server/__tests__/quran-translation-page.test.ts`.

`server.ts` boot block (`:828-846`), before `bun.serve`:

```ts
export function assertInternalSsrEnv(env: Record<string, string | undefined>): void {
  if (env.NODE_ENV !== "production") return; // Dockerfile.web sets NODE_ENV=production
  if ((env.PUBLIC_ENV ?? "").trim() === "local") return; // explicit local-mode deploy opt-out
  const base = (env.INTERNAL_QURAN_API_BASE ?? "").trim();
  const token = (env.INTERNAL_QURAN_API_TOKEN ?? "").trim();
  if (base === "" || token === "")
    throw new Error(
      "[server] INTERNAL_QURAN_API_BASE and INTERNAL_QURAN_API_TOKEN are required in production; refusing to boot into the public-API fallback",
    );
}
```

Keep the dev/test fallback (`quran-translation-page.ts:33-41`) — it is pinned by a test
(`quran-translation-page.test.ts:230-239`) and is the correct local behaviour. Only production
changes: it now crashes instead of silently collapsing all SSR into the web container's single
egress IP — and that bucket is the **escalatable** content bucket (`main.rs:912`
`.with_escalation(escalation)`), so with escalation enabled the host's egress IP could be banned for
7 days and take the whole site's translated rendering with it.

### P0.5 — Offline-pack byte budget + committed edge cache rule — 0.5–1 d

**Files:** `web/server.ts`, `web/src/lib/offline/offline-store.svelte.ts`, `deploy/README.md`.

1. Budget in the P0.2 limiter: `/offline/*` gets a per-IP byte counter (charge the negotiated size —
   read `.br`/`.gz` sizes into `AssetMeta` at first touch in `lateAssetMeta`, `server.ts:420-440`),
   burst 5 / day cap 20 requests **and** 100 MB/IP/day. The SW/offline engine downloads once per
   install per build id (`offline-store.svelte.ts:203-268`); 20/day covers family/kiosk reinstalls.
   Never challenge this path (breaks install).
2. Client: change the pack fetch from `cache: "no-store"` to `cache: "default"`
   (`offline-store.svelte.ts:226`) — the pack is content-addressed by build id and served immutable,
   so the browser can reuse it; keep `no-store` for the manifest (`:185,208`).
3. `deploy/README.md` new section "Edge cache contract (Cloudflare)":
   - `Cache Everything` for `/api/quran/**` (origin `s-maxage=86400` becomes real) and
     `/offline/pack.*.json` (immutable, long edge TTL).
   - `Bypass Cache` for `/offline/manifest.json`, `/*.html`, `/t/**`, `/quran-meta/*`.
   - **Do not** cache reader HTML at the edge while Accept-based negotiation is same-URL (P0.6).
   - A Cloudflare rate-limiting rule on `/t/*` + `/offline/*` as a backstop (free plan: one rule,
     10 s window) — the origin limiter remains primary.
   - Note the owner-run ingress-range procedure already required at `deploy/README.md:377-409`.

### P0.6 — Canonical query order + honest `Vary` + no-store on errors — 0.5–1 d

**Files:** `rust/backend/api/src/modules/quran_v1/controller.rs`, `src/error/response.rs`,
`web/src/hooks.server.ts`, `web/server.ts`, `documentation/quran-system.md`.

1. `source_range` (`controller.rs:1258-1276`): after validation, build the canonical query from the
   _effective_ clamped values (`from={f}&to={t}`, both always present) and, when the raw query
   differs (`axum::extract::RawQuery`), return `308` to `{path}?{canonical}`. Idempotent by
   construction. The in-repo fetcher already emits exactly `?from=&to=` (`range-fetch.ts:86`), so no
   legitimate reader ever sees the redirect.
2. `/ayahs` (`controller.rs:466-514`): sort + dedupe the parsed `keys` before building `keys_csv`,
   canonicalize `fromGlobal`/`toGlobal`/`script`/`cursor`, and 308 when the raw query differs.
   Document the semantic change ("keys are returned in ascending global order; duplicates removed")
   — no in-repo client uses `keys=` (grep: 0 URL builders), and the current order is undocumented.
   This closes an _unbounded_ cache-key space (≤50! orders) that B3 called "8 permutations".
3. `Vary`: drop the `mdSiblingRequest(pathname) !== null` clause from `applyHeaders` in both
   `hooks.server.ts:152-154` and `server.ts:314-321`. Keep `Vary: Accept` on `.md`/`.txt`, 404, and
   the negotiated-markdown responses (they set it explicitly). Safe only together with the committed
   rule "reader HTML is never edge-cached" — record that coupling in `deploy/README.md`.
4. `error/response.rs:88-120`: set `Cache-Control: no-store` on every `ErrorResponse` (covers 400,
   404, 429, 5xx) so a future Cache Everything rule cannot pin an error. Also stamp `no-store` on
   the rate-limited `blocked_response` path (it already routes through `ErrorResponse`).

### P0.7 — Per-artifact verified set for `/quran/sources` and `/quran/scripts` — 0.5–1 d

**Files:** `rust/backend/api/src/modules/quran_v1/controller.rs`, `src/state.rs` (field type),
`src/main.rs` (init).

Replace `Mutex<Option<Vec<SourceDto>>>` with:

```rust
struct ArtifactMemo { verified: HashMap<String /*artifact id*/, SourceDto>, last_sweep: Instant, sweeping: bool }
```

- Serve the union of `verified` + freshly verified candidates; `no-store` while
  `verified.len() < advertised` (unchanged).
- Sweep only the **missing** candidates, under the same mutex, but with a **retry floor**
  (`SOURCE_SWEEP_MIN_INTERVAL`, e.g. 60 s) so a permanently absent artifact cannot be re-probed per
  request. Concurrent requests wait for the in-flight sweep (single-flight) instead of each running
  one.
- Add `quran_sources_cold_resolves_total` (counter) + WARN on sweep failure, so a partial bucket is
  visible instead of silent.
- Keep `NO_STORE` semantics and the `verified={n}` ETag canonical exactly as today.

### P0.8 — Bulk-data documentation and manifest — 1 d

**Files:** new `docs/bulk-data.md`, new `web/static/.well-known/quran-bulk.json` (build-generated),
`web/src/lib/seo/render.ts`, `web/static/robots.txt`, `rust/backend/api/src/modules/quran_v1/dto.rs`.

1. `docs/bulk-data.md`: the three paths (21 API requests for Arabic; one 590 KB pack; 378 direct
   `https://r2.easyquran.fyi/tanzil/translations/sqlite/<id>.sqlite` for 616 MiB), the policy
   ("bulk download is free; do not scrape the API; contact X"), and the licence/attribution rule.
2. `/.well-known/quran-bulk.json` generated at build from the same baked catalogue
   (`db/quran/translations/index.min.json` → `web/src/lib/data/translations.json`): per source
   `id`, `language`, `languageCode`, `bytes`, `url`, `licence`, `attribution`. **No hash field** —
   the repo's hard rule forbids SHA-256 over Quran data in any automated path; a DB's identity is
   its id.
3. `renderLlmsIndex()` (`seo/render.ts:53`) gets a `Bulk data` section pointing at both.
4. `web/static/robots.txt`: `Disallow: /offline/` (the pack should be fetched deliberately, not
   crawled) and keep `Sitemap:`.
5. API `SourceDto` (`dto.rs`): add `licence`/`attribution` fields (serde defaults) so the data
   carries its own terms; keep `download_url` public.

### P0.9 — Reader-path observability (small, ships with P0.2) — 0.25 d

**Files:** `web/src/routes/health/quran/readiness.ts` (or a private stats route),
`web/src/hooks.server.ts`, `rust/backend/api/src/modules/quran_v1/controller.rs`.

- The disk-cache stats struct exists and **has no caller** (`quran-disk-cache.ts:138-149,223`);
  surface `hits/misses/writes/evictions/entries/bytes` on the existing `/health/quran` readiness
  payload (it is a Docker-network endpoint, not the public reader surface) and WARN when
  `evictions/min` spikes.
- Log the swallowed SSR failure (`quran-translation-page.ts:99-110,199-211`) as WARN with source
  id + range + status, and count it; today a site-wide 429 outage is invisible.
- Log every limiter rejection (the API precedent is `layer.rs:323`, currently `debug!` — see C3 F4:
  promote it to `warn!` so the API side is visible too).

### P0.10 — What is explicitly _not_ in phase 0 (D2 already cut it)

ASN/network tier, harvest-shape scoring, in-flight per-identity caps, Turnstile on cold endpoints,
and `.md` HTML-cache extension. Reasons in D2 §6.2; the `.md` disk cache is a P2 nicety now that the
range cache (512 entries) amortizes the API cost.

---

## 4. Per-fix blast radius and verification

| Fix  | What breaks for a legitimate reader                                                                                                                                                                                                                                                                                                                      | Test                                                                                                                                                                                                                                                                                                                                                                    | Manual                                                                                                                                                                                                         |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P0.1 | Logged-in readers keep the uncached path (marker cookie present) — no regression. Anonymous returning readers get _faster_ pages. Risk: if any future code puts user state in reader HTML, it would now be shared among anonymous clients (it already was shared among cookie-less ones) — the `responseSetsCookie` guard + a content assertion cover it | Rewrite `localized-reader-hook.test.ts:304-320` into three cases: `cookie: "session=private"` → cache used + `no-cache`; `cookie: "_ga=GA1.1.1"` → cache used; `cookie: "ruxlog.auth=1"` → bypass + `private, no-store`; `cookie: "ruxlog.sid=x"` → cache used. Add a Rust test that login sets `ruxlog.auth` and logout clears it, and that `with_path("/api")` is set | `curl -sS -o /dev/null -D - -H 'Cookie: _ga=GA1.1.1' https://…/t/en/sahih/page/7` twice → `x-easyquran-quran-cache: hit` on the second; repeat with `ruxlog.auth=1` → no cache headers and `private, no-store` |
| P0.2 | Shared-NAT users: hits are unlimited; only _misses_ are capped at 60/min/IP and the global 300/min. 100 readers behind one NAT each opening a cold page simultaneously could see a 503 + Retry-After (retry succeeds)                                                                                                                                    | Unit tests for the bucket (window, burst, LRU cap, hit-exemption) + a hooks test asserting a cache hit never touches the miss limiter                                                                                                                                                                                                                                   | `for i in $(seq 1 80); do curl -s -o /dev/null -w '%{http_code} ' https://…/t/en/sahih/page/$((i%30+1)); done` → expect hits to stay 200 while misses 429/503 past the cap; `/health/quran` always 200         |
| P0.3 | None — the backstop is unchanged; `.md` clients get their own bucket                                                                                                                                                                                                                                                                                     | Rust layer tests mirroring the existing three (content/internal/health isolation), one per new class + a test that an unknown render-class header lands in the default bucket                                                                                                                                                                                           | Internal-only: `curl -H 'x-easyquran-internal-token: …' -H 'x-eq-render-class: md' http://api:8888/quran/sources/en.sahih/range?from=1&to=10`; check the bucket count via `/health/quran`/logs                 |
| P0.4 | A misconfigured production deploy now fails to boot instead of degrading — intended. Rollback: set the two env vars (they are already in `docker-compose.yml:63-66`)                                                                                                                                                                                     | Unit test for `assertInternalSsrEnv` (prod+missing → throws; prod+local → ok; dev+missing → ok)                                                                                                                                                                                                                                                                         | `docker compose up web` with `INTERNAL_QURAN_API_BASE` removed → container exits non-zero with the message                                                                                                     |
| P0.5 | A client that reinstalls the offline pack >20×/day or pulls >100 MB/day/IP gets 429 until the next day. SW install path unaffected (one download per build)                                                                                                                                                                                              | Unit test for the byte budget (charge = negotiated size); a test that `/offline/manifest.json` is not byte-charged                                                                                                                                                                                                                                                      | `curl -sS -D - -o /dev/null https://…/offline/pack.<id>.json` ×6 → 6th is 429; with `Accept-Encoding: br` the charge is 589,573 not 14,158,643                                                                 |
| P0.6 | Third-party API clients that permute `from`/`to` or `keys` order get one 308 (browsers/curl follow). The app's own fetcher already uses canonical order                                                                                                                                                                                                  | Rust tests: canonical URL idempotence (no redirect loop), `from>to` still 400, `keys` sort/dedupe, and an ETag test that permuted and canonical URLs share one ETag                                                                                                                                                                                                     | `curl -sSI 'https://…/api/quran/sources/en.sahih/range?to=10&from=1'` → `308` with `location: …?from=1&to=10`; then the canonical URL → `200` + `cf-cache-status: HIT` once the rule is live                   |
| P0.7 | `/quran/sources` may still return a partial list while R2 is incomplete (same as today), but converges instead of re-sweeping; no reader impact                                                                                                                                                                                                          | Rust test: memo with one missing artifact → first request sweeps, second request does zero HEADs within the retry floor; after the artifact appears, the list is complete and cached                                                                                                                                                                                    | In a dev env, point one catalogue entry at a missing object and watch the logs/metric for one sweep per minute instead of one per request                                                                      |
| P0.8 | None                                                                                                                                                                                                                                                                                                                                                     | Build test: the generated manifest matches the baked catalogue count and contains no `sha256` field (extend the existing catalogue-sha guard)                                                                                                                                                                                                                           | `curl -sS https://…/.well-known/quran-bulk.json \| head`; `/llms.txt` shows the Bulk data section; `robots.txt` disallows `/offline/`                                                                          |
| P0.9 | None (adds fields to a Docker-internal readiness payload)                                                                                                                                                                                                                                                                                                | Readiness test updated for the new fields; a WARN assertion on the swallowed-error path                                                                                                                                                                                                                                                                                 | `curl -sS http://localhost:8080/health/quran \| jq` after a translated page → `entries`/`bytes` move; force a 429 with a local API stub and confirm the WARN                                                   |

---

## 5. Attacking the fixes (what a determined scraper does; residual)

| Fix                                    | Attack                                                                                                                                                                                                                   | Residual                                                                                                                                                                                                                                                                                                                                                                                     |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P0.1 cache restore                     | Send `Cookie: ruxlog.auth=1` on every request → cache bypass, full SSR. The web cannot validate the marker (it has no `COOKIE_KEY`; validating would need a shared secret and a DB/session read on the reader hot path). | The marker only fixes _legitimate_ readers; an attacker's cache-busting is unchanged. Bounded by P0.2's miss limiter and the global budget. **Accepted.** (Optional hardening: HMAC the marker with a shared subkey and verify in the web — rejected for phase 0: new secret in the web container, new failure mode, and the attacker can still force misses by rotating source/range keys.) |
| P0.2 per-IP miss limit + global budget | Rotate IPs (a $10 residential pool = thousands of exits); each IP gets 60 misses/min.                                                                                                                                    | With N IPs, misses scale N×60/min; the global 300/min budget then sheds load site-wide → degraded (503 on cold pages) but **cache hits still serve**, which is the real defence. One IP can no longer starve the site; a funded pool can still degrade cold paths. **Residual: medium.**                                                                                                     |
| P0.3 bucket split                      | Flood both classes.                                                                                                                                                                                                      | Damage is partial (a `.md` flood cannot kill HTML) but both are global. Backstop unchanged. **Residual: medium.**                                                                                                                                                                                                                                                                            |
| P0.4 boot error                        | Nothing (ops-only).                                                                                                                                                                                                      | A production deploy that intentionally runs without the internal base must set `PUBLIC_ENV=local` explicitly — documented.                                                                                                                                                                                                                                                                   |
| P0.5 pack byte budget                  | Rotate IPs; each pull is 590 KB (brotli) from the edge once the rule lands, 14.1 MB from origin without it.                                                                                                              | Attacker pays more than we do; origin sees one fetch per POP per deploy. **Residual: low.**                                                                                                                                                                                                                                                                                                  |
| P0.6 canonical 308                     | Permute _values_ (`from=1&to=300` vs `from=1&to=299`) — legitimate content variance, not fragmentation. `keys=` permutations collapse.                                                                                   | Unbounded distinct valid content objects are inherent to the API. **Residual: none for the fix.**                                                                                                                                                                                                                                                                                            |
| P0.7 verified set                      | Keep one artifact permanently missing (out-of-band, attacker can't) → one sweep per minute, bounded.                                                                                                                     | 380 HEADs/min worst case, down from 380/request. **Residual: low.**                                                                                                                                                                                                                                                                                                                          |
| P0.8 bulk doc                          | Ignore it and scrape anyway.                                                                                                                                                                                             | The Arabic corpus is public-domain; translations are licensed per source; the lever is contact + takedown. **Residual: irreducible.**                                                                                                                                                                                                                                                        |
| P0.9 observability                     | Nothing to attack.                                                                                                                                                                                                       | Without it every fix ships blind.                                                                                                                                                                                                                                                                                                                                                            |

**Cross-cutting residual:** a residential/mobile pool defeats every per-IP dimension (B3 F8), and
CGNAT means the per-IP limit cannot be tightened. The honest metric is cost per full copy and cache
hit ratio, not "scrapers blocked".

---

## 6. What cannot be fixed (must be accepted and written down)

1. **Public text is public.** Arabic is one 590 KB request (pack) or ~21 API requests; translations
   are 378 direct R2 GETs enumerated by one `/quran/sources` call. The mission requires the Arabic
   corpus to stay freely distributable.
2. **A determined scraper with a residential pool** can always read faster than a human. Per-IP is
   the only identity the reader path has, by design (no device payload on reader routes).
3. **CGNAT/shared IPs** mean the per-IP limit must stay loose enough that a household, madrasa, or
   cyber-café never notices; that is why the limiter is on misses + a global budget, not on reads.
4. **No reader identification.** No fingerprint, JA4, Bot Score, or header order is available off
   Cloudflare Enterprise, and none is wanted.
5. **Volumetric DDoS** is Cloudflare's problem; the app-layer variant is what P0.1–P0.3 bound.
6. **Cloudflare config is deployment-owned.** The repo can document the edge contract (P0.5) but
   cannot enforce it; a rule that is not applied in the console does not exist.

---

## 7. Findings (11)

### E6-F1 — "Any cookie" disables the disk cache and the service-worker caches; GA4 makes that the default for every returning reader. **NEW (extends B3 F2). Severity: high.**

Evidence: `web/src/hooks.server.ts:427` (`requestHasCookie = !!event.request.headers.get("cookie")`), `:346-351` (`cacheable = … && !requestHasCookie`), `:130-137` (`private, no-store`); `web/src/service-worker.ts:123-128` (`isCacheable` rejects `private`/`no-store`) → `:674-716`; `web/src/lib/firebase/analytics.ts:11-28` (unconditional `initAnalytics`); `web/src/lib/boot/analytics.ts:20-31`; `web/src/lib/stores/consent.svelte.ts:16,70-80` (`analytics: true` → `analytics_storage: granted`). The CSRF cookie (`csrf_v1/controller.rs:10-19`, `main.rs:781-788`) is _not_ minted on a first reader visit (`auth-state.svelte.ts:106-114` gates the probe on a localStorage hint; no reader-path `unsafeRequest`), but `_ga` is set after the first page load and rides every subsequent request. Live proof: anonymous GET is `hit` on the second request; the SW additionally stores nothing for cookie-bearing clients.
Fix: P0.1 — `.with_path("/api")` on the session cookie, a login-only `ruxlog.auth` marker, and a name-specific predicate; tests for `_ga`/`ruxlog.sid`/`ruxlog.auth`.

### E6-F2 — Internal-budget exhaustion is silent: the 429 becomes an empty 200 "pending" page, with no log, no metric, and no cache. **NEW. Severity: high (detection).**

Evidence: `web/src/lib/server/quran-translation-page.ts:199-211` (catch-all → `ayahs: []`), `:99-110` (error mapped then thrown, swallowed by the caller); `web/src/lib/server/reader-route-guards.ts:33-45` (`x-eq-translation-pending: 1`, `noindex`); `hooks.server.ts:380` (pending pages refused by the disk cache); `quran/api-availability.ts:28-31,80-83` (429 is a _neutral_ failure → the breaker never opens). Result: one IP pins the global internal bucket and every translated page renders empty — indistinguishable from "translation not published" to a reader and invisible to the operator.
Fix: P0.9 (WARN + counter on the swallowed failure; surface disk-cache stats), P0.2 (shed with 503 + Retry-After instead of rendering empty when the global budget is exhausted).

### E6-F3 — The internal SSR bucket is global (`ratelimit:internal-webssr:quran-v1`), the web origin has no limiter, and B3's md/range split is not implementable without a new signal. **CONFIRMS B3 F1. Severity: blocker.**

Evidence: `rust/backend/api/src/middlewares/rate_limit.rs:68-79` (limit `QURAN_INTERNAL_REQUESTS_PER_MINUTE` default 600, `PathKey::Fixed("quran-v1")`); `crates/rux-request-gate/src/layer.rs:239-243,278` (key construction); `client_ip.rs:92-106` (token → `InternalServiceId::WebSsr`); `main.rs:910-912` (attach); `web/server.ts` + `web/src/hooks.server.ts` (no limiter — grep-verified); `web/src/lib/server/quran-translation-page.ts:82-98` (both HTML and `.md` fetch `/sources/{id}/range`); `translation-range-cache.ts:4,24-46` (512-entry range cache changes the attack key space); live `x-ratelimit-limit: 600` on the public API.
Fix: P0.2 (per-IP miss limit + global web budget) and P0.3 (render-class header + route-class buckets), keeping the global backstop.

### E6-F4 — The artifact fan-out is 380 HEADs in production and serialized by the memo mutex, so N requests run N sequential sweeps. **CONFIRMS B3 F6. Severity: medium-high.**

Evidence: `controller.rs:945-1020` (`resolve_sources` + `Semaphore::new(32)` + shared `http_client` note at `:995-997`); `:915` (`if resolved.len() == total` — all-or-nothing memo); `:901-920` (mutex held across the await); `:821` (scripts equivalent); `quran/store.rs:58-63` + `controller.rs:43-45` (2 advertised scripts in prod) + 378 catalogue entries (`db/quran/translations/index.min.json`) = 380.
Fix: P0.7 — per-artifact verified set, retry floor, single-flight waiters, cold-resolve metric.

### E6-F5 — With `INTERNAL_QURAN_API_BASE` unset, all reader SSR collapses into the **escalatable** external content bucket. **NEW (sharpens B3 F1's secondary hazard). Severity: high (config-dependent).**

Evidence: `quran-translation-page.ts:33-41` (`requireApiBase` falls back to `QURAN.apiBase`), `:51` (`usingInternal` false → no token), `:63` (token only sent to the internal base — correct); `main.rs:912` (the content layer is the only one carrying `.with_escalation(escalation)`); `config/settings.rs:306-378` (escalation: Temp 1 h after 5 blocks, Long 7 d after 20, allowlist-gated, default-off); `middlewares/rate_limit.rs:57-63` (600/min). A missing env var therefore routes the whole site's SSR through one external IP bucket that can, when escalation is enabled, be banned for 7 days.
Fix: P0.4 — production boot error; the test at `quran-translation-page.test.ts:230-239` keeps the dev fallback.

### E6-F6 — The translation pool binds at ~4–8 entries (byte cap first), and one IP can cycle all 378 ids, evicting the working set and inflating the durable demand counter. **CONFIRMS B3 F11 (with measured sizes). Severity: medium.**

Evidence: `config/settings.rs:277-301` (`QURAN_MAX_RESIDENT_TRANSLATIONS=8`, `QURAN_MAX_RESIDENT_TRANSLATION_BYTES=48 MiB`, idle TTL 1800 s); `quran/translation_pool.rs:228-297` (`try_get_with` single-flight, `enforce_byte_bound` at `:275`); measured sizes min 0.77 MB / median 1.27 MB / p90 2.85 MB / max 12.4 MB, total 616.0 MiB; `controller.rs:1258-1296` (`get_or_build` on `source_range`); `main.rs:594-646` (demand flush every 6th tick).
Fix: P0.2 (miss limiter bounds the cycle rate); optionally a per-identity distinct-source budget and a metric on pool churn (P0.9).

### E6-F7 — The edge contract is missing, error responses carry no `Cache-Control`, and HTML is negotiated on the same URL with `Vary: Accept` — so a Cache Everything rule on `/api/**` could pin 4xx and one on HTML would be a correctness bug. **CONFIRMS B3 F4 + NEW. Severity: high.**

Evidence: `quran_v1/cache.rs:5-8` (correct `s-maxage`), live `cf-cache-status: DYNAMIC` on `/api/quran/surahs` and on the immutable pack; `rust/backend/api/src/error/response.rs:88-120` (no `Cache-Control` on errors); `hooks.server.ts:152-154` + `server.ts:314-321` (`Vary: Accept` on every reader HTML); `web/src/lib/server/markdown-negotiation.ts` + `hooks.server.ts:444-457` (same URL returns markdown for `Accept: text/markdown`); `deploy/README.md` (no Cache Rule text anywhere); `documentation/quran-system.md:146` ("Quran responses are edge-cacheable" — intent without a rule).
Fix: P0.5 (Cache Rules + the HTML exclusion) and P0.6 (canonical queries, `Vary` cleanup, `no-store` on errors).

### E6-F8 — The offline engine fetches the pack with `cache: "no-store"`, and the edge does not cache it either, so every install re-downloads from the origin. **NEW. Severity: medium.**

Evidence: `web/src/lib/offline/offline-store.svelte.ts:226` (`fetch('/offline/pack.${id}.json', { cache: "no-store" })`), `:185,208` (manifest, correctly no-store); `server.ts:467,511-522` (immutable headers); live `cf-cache-status: DYNAMIC` on `HEAD /offline/pack.<id>.json` with `cache-control: public, max-age=31536000, immutable`. The pack is content-addressed by build id, so `no-store` is self-inflicted cost.
Fix: P0.5 — `cache: "default"` for the pack fetch + the committed Cache Rule.

### E6-F9 — Disk-cache statistics are collected and never read; there is no signal for evictions, internal-budget exhaustion, or cold resolves. **NEW. Severity: medium.**

Evidence: `web/src/lib/server/quran-disk-cache.ts:138-149,223` (`getQuranDiskCacheStats` has no caller — grep: only the definition), `:23-33` (stats struct); `web/src/routes/health/quran/+server.ts` (readiness checks only manifest + writability); `controller.rs` (no cold-resolve counter); `crates/rux-request-gate/src/layer.rs:323-330` (`debug!` — per-route 429s invisible at `RUST_LOG=info`, C3 F4).
Fix: P0.9 — stats on the internal readiness payload, WARN on eviction spikes and swallowed SSR failures, cold-resolve counter, and the `debug!`→`warn!` promotion (shared with C3 F4).

### E6-F10 — The cheapest bulk path (21 requests / one 590 KB pack / 378 R2 URLs) is undocumented and has no licence or contact handle. **CONFIRMS B3 F5. Severity: medium.**

Evidence: `quran/store.rs:5` (`RESPONSE_CAP=300`), `controller.rs:1272-1275` (range cap), `:960,978-990` (`download_url`), `web/src/lib/quran/environment.ts:9` (`https://r2.easyquran.fyi`), 378 entries / 616.0 MiB measured, `web/src/lib/seo/render.ts:53` (`renderLlmsIndex()` has no bulk section), no `docs/bulk-data.md`.
Fix: P0.8.

### E6-F11 — The API's `robots.txt` is ruxlog residue and `/offline/` is crawlable. **CONFIRMS B3 F12. Severity: low.**

Evidence: `router.rs:236-246` (`Sitemap: https://ruxlog.com/sitemap.xml`, reachable only at `/api/robots.txt`); `web/static/robots.txt` (`Disallow:` empty); `/offline/pack.*` = 14.1 MB per crawl fetch, `/offline/manifest.json` = `no-cache` (revalidation traffic).
Fix: P0.8 — `Disallow: /offline/`, bulk-data pointer in `/llms.txt`, and delete or correct the API robots handler.

---

## Appendix A — exact key strings and limit values (verified)

| Layer                             | Key                                                                | Limit                                      | Where                                                              |
| --------------------------------- | ------------------------------------------------------------------ | ------------------------------------------ | ------------------------------------------------------------------ |
| Public content (external IP unit) | `ratelimit:{ip_unit}:quran-v1` (`203.0.113.5/32`, `2001:db8::/64`) | 600/60 s, escalatable                      | `rate_limit.rs:57-63`; `layer.rs:239-243,278`                      |
| Internal SSR (all routes)         | `ratelimit:internal-webssr:quran-v1`                               | `QURAN_INTERNAL_REQUESTS_PER_MINUTE` (600) | `rate_limit.rs:68-79`; `settings.rs:462`                           |
| Readiness                         | `ratelimit:{tcp_peer}:quran-health`                                | 120/60 s                                   | `rate_limit.rs:107-115`                                            |
| Public search                     | `ratelimit:{ip_unit}:/quran/search`                                | 30/60 s                                    | `main.rs:899-901`                                                  |
| `/api/sitemap.xml`                | `ratelimit:{ip_unit}:/sitemap.xml`                                 | 30/60 s                                    | `router.rs:53-56`                                                  |
| `/api/csrf/v1/generate`           | `ratelimit:{ip_unit}:/csrf/v1/generate`                            | 60/60 s                                    | `router.rs:57-60`                                                  |
| Auth nest (per matched route)     | `ratelimit:{ip_unit}:/auth/v1/*`                                   | 100/60 s                                   | `router.rs:72-76`                                                  |
| Disk cache entry                  | `build-{buildId}__{sourceId}__{kind}__{a}[__b]__ui-{en\|ar}`       | 7 d TTL, 256 MB, mtime-LRU                 | `quran-disk-cache.ts:11-13,51-57,162-211`; `hooks.server.ts:45-53` |
| Range cache entry                 | `{sourceId}:{from}-{to}`                                           | 512 entries, single-flight, in-process     | `translation-range-cache.ts:4,24-46,62-78`                         |
| Translation pool                  | Moka, count 8 / bytes 48 MiB / TTL 1800 s                          | —                                          | `settings.rs:299-301`; `translation_pool.rs:228-297`               |
| Offline pack                      | `/offline/pack.{buildId}.json`                                     | 14,158,643 B raw / 589,573 B br            | measured; `gen-offline-pack.ts:152-196`                            |

## Appendix B — live probe evidence (read-only, 2026-10-11/12)

```
GET /api/quran/surahs           -> 200, cache-control: public, max-age=300, s-maxage=86400,
                                   stale-while-revalidate=604800, stale-if-error=604800,
                                   etag: W/"quran-corpus:surahs", x-ratelimit-limit: 600,
                                   vary: Accept-Encoding + vary: origin, access-control-request-method,
                                   access-control-request-headers, cf-cache-status: DYNAMIC
GET /t/en/sahih/page/7 (1st)    -> 200, cache-control: no-cache, x-easyquran-quran-cache: miss,
                                   server-timing: quran_ssr_cache;desc="miss", vary: Accept,
                                   link: </t/en/sahih/page/7.md>; rel="alternate", cf-cache-status: DYNAMIC
GET /t/en/sahih/page/7 (2nd)    -> x-easyquran-quran-cache: hit, server-timing: …desc="hit"
GET /offline/manifest.json      -> {pack: /offline/pack.1791752085271.json, bytes: 14158643, entries: 1635}
HEAD /offline/pack.1791752085271.json
                                -> cache-control: public, max-age=31536000, immutable,
                                   vary: accept-encoding, cf-cache-status: DYNAMIC
   + Accept-Encoding: br        -> content-encoding: br, content-length: 589573
```

## Appendix C — B3 claims re-verified, one line each

1. F1 fix 3: `…:quran-v1-md` cannot exist without an `x-eq-render-class` header — HTML and `.md`
   call the same API route.
2. F1 impact: the reader sees an empty 200 pending page (the internal 429 is swallowed).
3. F4: range-query permutations are 2 (only `from`/`to`); the unbounded permutation space is
   `/ayahs?keys=` (≤50! orders). API `Vary` is `Accept-Encoding` + the CORS triple.
4. F11: translation sizes 0.77–12.4 MB (median 1.27) → the 48 MiB byte cap binds first.
5. §1.4: the public Quran router has 29 routes (+search = 30).
6. §1.6/A8: the 512-entry range cache amortizes the API call for translated `.md`; the HTML disk cache
   still excludes it.
7. §1.2: `web/server.ts` is 881 lines and `hooks.server.ts` 543 — confirmed; zero limiter code.
8. §0/A5: `r2.easyquran.fyi` is public and hands out 378 URLs — confirmed; the live pack/API probes
   add the missing edge-cache evidence (`DYNAMIC` on both).
