# B3 — Anonymous and scraping abuse on the reader path (DESIGN-v1 OPEN Q3)

Auditor: adversarial abuse review, reader path only. Date: 2026-10-12. Repo: `/Users/hmziq/os/easyquran` (working tree at audit time; all line refs verified against it).
Inputs: `DESIGN-v1.md` (Q3), `A1-threat-model.md` F5, `A8-ops-realism.md` §9 + F1, `R2-evasion-counters.md` §1–2, §8, §10, `R2-network-transport.md` §6–7, plus repo source (cited inline).

**Question answered.** Readers are logged out. The device payload is attached only to `/api/**` auth calls (`web/src/lib/auth/auth-client.ts:257-259`), so the reader path never carries a device signal — by design and correctly. What defends the reader path _without_ fingerprinting readers?

**One-line answer.** Nothing does today, and the reason is not that fingerprinting is missing — it is that **the reader path has no limiter of its own on the web origin, its only cache is defeated by one cookie header, and all translated rendering funnels through a single global 600/min internal bucket.** The reader-path abuse that matters here is not scraping (the corpus is public and _should_ be copied); it is **cost and availability**, and both are trivially attackable today by a single anonymous IP.

---

## 0. What is actually being defended (the delivery contract)

From `docs/quran-system.md` and the code, these must stay public, unauthenticated, crawler-accessible and cheap:

| Asset                                                                | Contract                                                          | Evidence                                                                                                 |
| -------------------------------------------------------------------- | ----------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Arabic corpus (Uthmani family in prod)                               | public, freely distributable, immutable                           | `docs/quran-system.md:15-37`; `rust/.../quran_v1/controller.rs:47-62` (IndoPak/Tajweed withheld in prod) |
| Translation corpus (378 entries, 96 languages, **616 MB** of SQLite) | reader-free; the _defensible_ asset (production cost + licensing) | `db/quran/translations/index.min.json` (378 entries); `R2-evasion-counters.md:379`                       |
| Reader HTML (Arabic)                                                 | prerendered; `.md` twin prerendered                               | `web/build/prerendered/*.md` (814 files)                                                                 |
| Reader HTML (translations)                                           | SSR + 7-day disk cache; never SSG                                 | `docs/quran-system.md:292-293`; `web/src/lib/server/quran-disk-cache.ts:11-12`                           |
| `/llms.txt`, `/llms-full.txt`, `/*.md`, `/*.txt`                     | deliberate LLM/crawler surface                                    | `web/src/routes/llms.txt/+server.ts`; `web/src/lib/accept-parse.ts:28-48`                                |
| `/offline/manifest.json` + `/offline/pack.<id>.json`                 | offline install path, immutable                                   | `web/scripts/gen-offline-pack.ts:152-196`; `web/static/_headers`                                         |
| `/_quran/**`                                                         | allowlisted same-origin R2 gateway, immutable                     | `web/src/routes/_quran/[...artifact]/+server.ts:9,49-61`                                                 |
| `r2.easyquran.fyi`                                                   | public R2 custom domain; full DBs handed out as `download_url`    | `web/src/lib/quran/environment.ts:9`; `controller.rs:858,978-990`                                        |
| `/api/quran/**`                                                      | edge-cacheable, weak id-based ETags                               | `docs/quran-system.md:146`; `quran_v1/cache.rs:5-8`                                                      |

**Consequence.** The Arabic text is public-domain and _supposed_ to be copyable. Content-scraping defence is self-defeating if pushed far (R2 §8.3). So the defensible surfaces are: (1) availability of the SSR/translation path, (2) bandwidth/egress and origin cost, (3) the community layer (out of scope here), (4) the _translation_ corpus as a licensing asset. Rank accordingly.

---

## 1. What exists today (verified inventory, no speculation)

### 1.1 Limiters on the content path

| Limiter                                                   | Value                                                         | Key                                                                             | Where                                                                                                           |
| --------------------------------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| Public content ceiling                                    | **600 req / 60 s**                                            | `ratelimit:{ip_unit}:quran-v1` — canonical unit: IPv4 `/32`, IPv6 `/64`         | `rust/backend/api/src/middlewares/rate_limit.rs:57-63`; key built at `crates/rux-request-gate/src/layer.rs:278` |
| Public search                                             | **30 req / 60 s**                                             | `ratelimit:{ip_unit}:/quran/search` (Matched)                                   | `rust/backend/api/src/main.rs:899-901`                                                                          |
| Internal SSR (web → api)                                  | **600 req / 60 s** (env `QURAN_INTERNAL_REQUESTS_PER_MINUTE`) | `ratelimit:internal-webssr:quran-v1` — **one global bucket for the whole site** | `middlewares/rate_limit.rs:68-79`                                                                               |
| Readiness                                                 | 120 req / 60 s                                                | `ratelimit:{tcp_peer}:quran-health`                                             | `middlewares/rate_limit.rs:107-115`                                                                             |
| `/api/sitemap.xml` (ruxlog route, not the reader sitemap) | 30 req / 60 s                                                 | `ratelimit:{ip_unit}:/sitemap.xml`                                              | `rust/backend/api/src/router.rs:53-56`                                                                          |
| `/api/csrf/v1/generate`                                   | 60 req / 60 s                                                 | Matched route                                                                   | `router.rs:57-60`                                                                                               |
| `/api/auth/v1/**`                                         | 100 req / 60 s per matched route                              | Matched route                                                                   | `router.rs:72-76`                                                                                               |
| Login (per-account)                                       | **3 attempts / 360 s → 1 h block**; 5 / 900 s → 24 h          | `login:{normalized_email}`                                                      | `modules/auth_v1/controller.rs:31-38,133-140`                                                                   |

`docs/quran-system.md:277-281` states the same contract: "Fixed limits: 600/min general, 30/min search, 600/min internal SSR, 120/min readiness."

**There is no other limiter on the content path.** Grep-verified: no per-route, per-source, per-key, per-ASN, per-session or concurrency limiter anywhere under `/quran`.

### 1.2 The web origin has no limiter at all

`web/server.ts` (881 lines) and `web/src/hooks.server.ts` (543 lines) contain **zero** rate-limiting, throttling, concurrency or byte-budget code (grep for `rate|limit|throttle|budget|concurren` → only unrelated matches). Every reader-HTML request, every `.md`, every `/offline/*`, every `/_quran/*`, every `/quran-meta/*` is unlimited at the origin. The only brakes are Cloudflare (deployment-owned; no cache rules, WAF rules or rate-limiting rules are committed in this repo) and Traefik (no limiter middleware; only `compress=true`, `docker-compose.yml:108-110`).

### 1.3 The two caches, and how they fail

**API responses** — `quran_v1/cache.rs:5-8`:

```
public, max-age=300, s-maxage=86400, stale-while-revalidate=604800, stale-if-error=604800   (ARABIC_CACHE)
public, max-age=60, s-maxage=300                                                            (SEARCH_CACHE)
```

Correct headers — but **Cloudflare does not cache `.json` by default**; only a Cache Rule ("Cache Everything") makes `s-maxage` real. No Cache Rule is committed or documented. So today every `/api/quran/**` request is an origin render bounded only by 600/min/IP unit.

**Translated reader HTML** — disk cache, `web/src/lib/server/quran-disk-cache.ts:11-12,46-51`: 7-day TTL, **256 MB budget**, key `build-{id}__{sourceId}__{kind}__{a}[__b]__ui-{en|ar}`, mtime-LRU eviction (`:162-211`). It is **only used when the request carries no cookie** (`hooks.server.ts:345-351`: `!requestHasCookie`), and a cookie-bearing reader response is stamped `Cache-Control: private, no-store` (`hooks.server.ts:130-137`). A cache hit consumes **zero** internal SSR budget; a miss consumes one of the global 600/min.

### 1.4 Content API surface (what a scraper can ask for)

`modules/quran_v1/mod.rs:11-53` — 26 GET routes, all unauthenticated, `CorsLayer::allow_origin(Any)` (`quran_v1/cors.rs:7-22`), all under the single 600/min bucket.

| Endpoint                                                                | Yield per request                                                                                 | Cost to fetch the corpus       |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | ------------------------------ |
| `GET /quran/ayahs?keys=…`                                               | ≤ 50 verse keys (`controller.rs:474-495`)                                                         | 125 requests for all Arabic    |
| `GET /quran/surahs/{n}/ayahs`, `/juzs/{n}/ayahs`, `/pages/{n}/ayahs`, … | `RESPONSE_CAP = 300` (`quran/store.rs:5`); `limit` clamped (`controller.rs:239-251`)              | ~21 requests for all Arabic    |
| `GET /quran/sources/{id}/range?from=&to=`                               | ≤ 300 ayahs, **all** sources (`controller.rs:1272-1275`)                                          | 21 requests per source         |
| `GET /quran/sources/{id}/surah/{n}`                                     | whole surah (`controller.rs:1216-1256`)                                                           | 114 requests per source        |
| `GET /quran/search?q=`                                                  | ≤ 50 hits (`controller.rs:1163-1167`), 30/min                                                     | —                              |
| `GET /quran/sources`                                                    | **every source with a direct `download_url` to `r2.easyquran.fyi`** (`controller.rs:858,978-990`) | 1 request → 378 direct DB URLs |
| `GET /quran/scripts`                                                    | 4 (prod: 1) Arabic DB URLs                                                                        | 1 request                      |

So the full-corpus paths already exist and are _cheap_: **21 requests for all Arabic**, and **378 direct R2 GETs for 616 MB of translations** with the URLs handed out by one API call.

### 1.5 The bulk path is already cheaper than scraping (in two places)

1. `GET /quran/sources` → `download_url` = `https://r2.easyquran.fyi/tanzil/translations/sqlite/<id>.sqlite` (and `tanzil/arabic/<file>`), one per source.
2. `/offline/pack.<buildId>.json` — the postbuild pack contains **every prerendered `__data.json`**, i.e. the whole Arabic corpus. Measured on this working tree: `14,158,643` bytes raw, **`589,573` bytes brotli**, 1,635 entries, and juz bodies carry the Arabic verse text (e.g. `/juz/30/__data.json` = 80,723 bytes, 19,579 Arabic code points). Served immutable, unauthenticated, from the un-limited web origin (`web/server.ts:508-521`; `web/static/_headers`).

**This is the single most important fact for Q3.** A scraper needs **one 590 KB request** to obtain the entire Arabic corpus, and **378 requests** to obtain all translations. The "scraping" threat is therefore not a crawl-shaped problem at all — it is already a bulk-download problem, and the correct response is to _formalize_ it (§4.4), not to fight it.

### 1.6 Markdown / LLM delivery

`Accept: text/markdown` on a reader path serves the `.md` sibling (`hooks.server.ts:428-460`; families in `accept-parse.ts:28-48`). Arabic `.md` = **prerendered static file** (`web/server.ts:673-688`; 814 files in `build/prerendered`). Translated `.md` = **`prerender = false` → SSR → internal API call** (`web/src/routes/(application)/t/[lang]/[translator]/page/[n].md/+server.ts:10-22` → `loadTranslationRangeData` → `INTERNAL_QURAN_API_BASE` with the internal token). Response is `no-cache` (not an immutable asset, not a private response) → revalidate at the edge every time.

**So: LLM clients asking for translated markdown consume the global internal SSR budget, one call per request, uncached.**

### 1.7 Turnstile

`services/turnstile.rs:38-56` exposes exactly one entry point, `verify_registration`, whose sole caller is `modules/auth_v1/controller.rs:470` (registration). There is no challenge on the content path, no challenge-pass cookie, and no generic `verify_token`. CSP already allows `challenges.cloudflare.com` (`web/src/hooks.server.ts:84`).

### 1.8 Edge capability (what is and is not available)

From `R2-network-transport.md:261-276` and `deploy/README.md:347-392`:

- **Available**: `CF-Connecting-IP` (already the identity source, `IP_SOURCE=CfConnectingIp`), `CF-IPCountry`, `Cf-Ray`, and — via a Worker on any plan — `request.cf.asn`, `asOrganization`, `tlsVersion`, `tlsCipher`, `tlsClientHelloLength`, `clientTcpRtt`, `httpProtocol`.
- **Not available off Enterprise**: JA3/JA4/`ja4Signals`, Bot Score, `verifiedBot`, Logpush-inline. No Worker exists in this repo, so today the origin gets **nothing** but `CF-Connecting-IP`.
- **Definitively lost behind Cloudflare** (R2 §7): eyeball TCP/IP fingerprint (JA4T, TTL, MSS), eyeball HTTP/2 frame fingerprint, raw ClientHello bytes/extension order, **eyeball header order/casing**. CloudFront exposes `CloudFront-Viewer-Header-Order`/`-JA4-Fingerprint`; Cloudflare does not. So the "header-order/UA coherence at the edge" idea in the brief is **not implementable here** except in its weak form (coherence between the forwarded `User-Agent` and the forwarded `sec-ch-ua*` values, both of which CF passes through).

---

## 2. Abuse cases on the reader path, ranked by damage for THIS app

Rank = (likelihood for this app) × (damage), where damage = availability + cost + mission, not "did data leave".

| #       | Abuse case                                                                                                                                                                                                                                                                           | Likelihood | Damage                                 | Why this rank                                                                                                                                                                                                                                                                                                                                                                 |
| ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------- | -------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A1**  | **SSR-budget starvation**: one anonymous IP drives translated reader/`.md` SSR until the global 600/min internal bucket is exhausted → translated pages 429 for _everyone_                                                                                                           | Certain    | **High**                               | One IP, ~10 req/s, no fingerprint needed, no credential, no cost. The internal bucket is the site's translated-reader ceiling and the web origin has no limiter (§1.2, §1.3, §1.6).                                                                                                                                                                                           |
| **A2**  | **Offline-pack bandwidth amplification**: `/offline/pack.<id>.json` is 14.1 MB raw / 590 KB brotli, unauthenticated, unlimited at the web origin, and `.json` is not edge-cached by default                                                                                          | High       | **High**                               | 590 KB × unlimited req/min from one IP; 14 MB per request if the client omits `Accept-Encoding: br`. Direct egress + origin cost, no fingerprint, no auth.                                                                                                                                                                                                                    |
| **A3**  | **Disk-cache working-set eviction (cache-busting)**: enumerate translation cache keys (`sourceId` × kind × index × ui-locale ≈ 611 k possible, 256 MB budget ⇒ ~1.7 k resident entries) to evict the working set → every legit translated page becomes an SSR miss → converges on A1 | Likely     | **Medium-High**                        | Under 3 minutes at 600/min/IP to flush; damage is sustained CPU + latency + the A1 ceiling for everyone.                                                                                                                                                                                                                                                                      |
| **A4**  | **Full-corpus scrape / competing-site republishing**                                                                                                                                                                                                                                 | Certain    | **Medium** (mission, not availability) | Already trivial: 21 requests (Arabic) or 1 × 590 KB pack; 378 R2 GETs (616 MB) for translations. Irreducible: the Arabic text is public-domain and must stay distributable (R2 §8.3).                                                                                                                                                                                         |
| **A5**  | **Hotlinking / third-party bundling of the R2 artifacts**                                                                                                                                                                                                                            | Likely     | **Medium-Low**                         | `r2.easyquran.fyi` is a public custom domain; `/quran/sources` hands out direct URLs. R2 egress is free and 378 GETs cost pennies; the real loss is observability and licensing control over the _translation_ corpus.                                                                                                                                                        |
| **A6**  | **Edge-cache defeat on `/api/quran/**`** (no committed Cache Rule ⇒ uncached JSON at CF) + **cache-key fragmentation** (query permutation, `Vary: Accept`)                                                                                                                           | Likely     | **Medium**                             | With N IPs the origin serves N × 600/min uncached 50–200 KB JSON bodies. `deny_unknown_fields` blocks junk params (`dto.rs:283-359`) but **param reordering/duplication still fragments the CF cache key**, and `Vary: Accept` is emitted on every reader path (`hooks.server.ts:152-154`) which CF ignores → both cache misses and HTML/markdown confusion.                  |
| **A7**  | **Crawl / sitemap storm**                                                                                                                                                                                                                                                            | Medium     | **Low**                                | Sitemap ≈ 808 reader entries × 2 alternates (`web/src/routes/sitemap.xml/+server.ts`), static + `no-cache` → revalidate per request. Serving is cheap; the cost is revalidation traffic and `Vary: Accept` fragmentation. No cap on crawl rate is committed (robots.txt = `Disallow:` empty).                                                                                 |
| **A8**  | **LLM training-corpus harvesting**                                                                                                                                                                                                                                                   | Certain    | **Low / mission-positive**             | `llms.txt`/`llms-full.txt`/`.md` exist deliberately. `llms-full.txt` is marketing pages only (no scripture) — good. The real cost is A1/A6 (translated `.md` = uncached SSR), not the data leaving.                                                                                                                                                                           |
| **A9**  | **API-key / credential stuffing against the content API**                                                                                                                                                                                                                            | N/A today  | **None**                               | The content API has **no credentials and no API keys** — there is nothing to stuff. The analogous risk materializes only if a bulk key is introduced (§4.4), at which point the key becomes a stuffable/leakable credential and must be hashed, limited and revocable. Stuffing against `/api/auth/v1/log_in` is already covered per-account (3/360 s → 1 h) plus 100/min/IP. |
| **A10** | **Volumetric DDoS (L3/L4)**                                                                                                                                                                                                                                                          | Certain    | **Out of scope**                       | Cloudflare absorbs it. The app-layer variant is A1/A6 and is _not_ absorbed, because the web origin has no limiter.                                                                                                                                                                                                                                                           |

**The reordering versus the brief's list matters.** "Full-text scraping for a competing site" and "LLM harvesting" are the _least_ damaging items here because the cheapest copy path already exists and is legitimate. "Sitemap/crawl storms" and "hotlinking" are cost items. The two cases that actually threaten the product are **A1 (one IP starves all translated rendering)** and **A2 (one IP burns bandwidth)**, both of which are consequences of the same two facts: **no limiter on the web origin, and one global internal bucket behind it.**

---

## 3. For each abuse case: what stops it today, and the gap

### A1 — SSR-budget starvation

**Stops it today:** nothing at the web origin (§1.2). The API's internal bucket caps it at 600/min — which _is_ the damage, not a defence: the cap is global, so the attacker's 600 requests/min and the entire legitimate readership's requests compete for the same 600.

Code path, end to end:

1. Attacker: `GET /t/en/sahih/page/7.md` (or `/t/en/sahih/page/7` with a cookie) → Traefik → web:8080.
2. `web/server.ts:690-714` → no prerendered `.md` for translations → kit SSR.
3. `hooks.server.ts:345-351` → `cacheable=false` (either a cookie, or `.md`).
4. Route handler → `loadTranslationRangeData(...)` → `INTERNAL_QURAN_API_BASE` (`http://api:8888/quran`) with `X-EasyQuran-Internal-Token`.
5. `middlewares/rate_limit.rs:68-79` → key `ratelimit:internal-webssr:quran-v1`, max 600/min.
6. Request 601 → `blocked_response` → **429 for every subsequent internal call from the whole site**, including SSR of every legitimate translated page and every translated `.md`.

**Gap:** the internal bucket is a _global_ backstop that is reachable by an anonymous external client, and the web origin has no per-IP limiter, no concurrency cap, and no byte budget. A single IP at 10 req/s removes translated reading for the entire site. This is the highest-value finding in this audit.

**Fix (specific):**

1. **Restore the disk cache for anonymous traffic** — the biggest multiplier, because a cache hit consumes zero internal budget. Distinguish "has a cookie" from "is authenticated": the API mints `ruxlog.sid` for anonymous CSRF (`main.rs:841-843` session layer; `router.rs:57-60` CSRF route), so today the _default_ state for any client that touched `/api` is `private, no-store` + cache bypass. Add a second, HttpOnly, non-PII marker cookie set **only on login** (`ruxlog.auth=1`) and gate `requestHasCookie` on _that_ (`hooks.server.ts:346`), or keep a "cookie present but session unauthenticated" fast path. Reader cache-safety is unaffected (the disk cache never contains user state — `docs/quran-system.md:249-251`).
2. **Per-IP limiter at the web origin.** In-memory token bucket in `web/server.ts`'s fetch pipeline: 240 reader-page requests/min/IP (a real reader is < 60/min; the reader's own chunked fetcher does 1–3 requests per view), 600/min for static/`.md`/`_quran`, and a separate **byte budget** for `/offline/*` (§A2). No DB, no dependency on the API's store.
3. **Split the internal bucket by route class** so a `.md` flood cannot consume the HTML budget: `ratelimit:internal-webssr:quran-v1-md` and `…:quran-v1-range` alongside the existing global key. Keep the global key as the backstop.
4. **Raise the global internal backstop** to match the container's real capacity (measure first: the API serves Arabic from memory; the constraint is the web's SSR CPU and the api's single connection). The 600/min value is documented as a policy, not measured capacity (`docs/quran-system.md:277-281`).

### A2 — Offline-pack bandwidth amplification

**Stops it today:** nothing. `web/server.ts:508-521` serves `/offline/*` from disk; `hooks.server.ts:135` marks `packPattern` immutable; `web/static/_headers` sets `public, max-age=31536000, immutable`. Compression negotiation exists (`server.ts:357-369`) so a well-behaved client gets 590 KB — but a client that omits `Accept-Encoding` gets **14.1 MB**, and the request rate is unlimited.

**Gap:** no byte budget, no per-IP counter, no cache rule committed for `/offline/pack.*.json` (immutable headers make it _safe_ to cache at the edge, but nothing says the edge _does_). The pack is also the cheapest full-corpus exfil path (§1.5), so the same request is simultaneously a cost vector and the intended bulk path.

**Fix:** per-IP token bucket on `/offline/*` — e.g. burst 5 downloads / day with a 24 h refill, and a hard cap of 20 per IP/day (a family or kiosk shares an IP; the SW downloads once per install per build id). Commit the CF Cache Rule for `/offline/pack.*.json` (`Cache Everything`, immutable, long edge TTL) so the origin sees one request per POP per deploy. Do **not** put a challenge in front of it: the SW install path must not break, and the content is public.

### A3 — Disk-cache working-set eviction

**Stops it today:** nothing. The eviction policy is mtime-LRU with a 256 MB budget (`quran-disk-cache.ts:195-210`) and the key space is `sourceId` × `{surah,page,juz,hizb}` × index × `ui-{en|ar}` — with 378 sources that is ~611 k keys for ~1.7 k resident slots. Fill order is attacker-controlled; the attacker can refresh their own entries to keep them hot.

**Gap:** no cost to the attacker (600/min/IP × a handful of IPs), no detection, and the failure mode is _silent_: legitimate pages become misses and consume the A1 budget.

**Fix:** (a) the per-IP limiter from A1.1 makes the fill rate the same as the legitimate rate, which bounds but does not stop it; (b) **separate the cache by source class**: pin the top-N translation sources (the demand store already ranks them — `services/translation_popularity_store.rs`, `main.rs:594-646`) in a protected partition that eviction cannot touch, and let the long tail be evictable. Attackers can then only churn the tail; (c) expose `quran_ssr_cache hits/misses/evictions` in a metrics endpoint and alert on eviction spikes (the stats struct already exists, `quran-disk-cache.ts:23-33`).

### A4 — Full-corpus scrape

**Stops it today:** the 600/min/IP content bucket (`rate_limit.rs:57-63`). That is the whole defence: 21 requests for Arabic, 21 per translation source, 378 R2 GETs for everything.

**Gap:** none that is _fixable_, and trying to fix it is a mission error (R2 §8.3: the Arabic text "must be freely distributable"). What is genuinely missing is not a wall — it is a **signposted door** (§4.4). Today the cheapest legitimate path is undocumented, so a scraper's rational choice is to hit the API (and, incidentally, to consume A1/A3 resources).

### A5 — Hotlinking the R2 artifacts

**Stops it today:** nothing. `QURAN_R2_UPSTREAM_BASE = "https://r2.easyquran.fyi"` (`web/src/lib/quran/environment.ts:9`) is a public domain, and `download_url` is returned to every caller of `/quran/sources` (`controller.rs:858,978-990`). `/_quran/[...artifact]` is an allowlisted same-origin gateway with no limiter (`+server.ts:12-62`), so hotlinking can also be laundered through our own origin (at our expense) rather than the R2 domain.

**Gap:** no per-IP limit on `/_quran/*`, no Referer/Origin policy, no observability of direct-R2 traffic. Damage is low in dollars (R2 egress is free; 378 GETs is negligible ops) but the translation corpus is the licensing asset, and a bundler that points at `r2.easyquran.fyi` gets the data with zero contact with us.

**Fix:** (a) per-IP limiter on `/_quran/*` (it is a same-origin gateway for _our_ reader; no third party needs it); (b) keep `download_url` public — that is the bulk path — but add `X-Robots-Tag: noindex` and a licence/attribution field to `SourceDto` so a bundler cannot claim it did not know; (c) do **not** add signed URLs: they break the documented bulk path and buy nothing (the corpus is public).

### A6 — Edge-cache defeat / cache-key fragmentation

**Stops it today:** the origin sets correct `Cache-Control` (`cache.rs:5-8`) and `Vary: Accept-Encoding` only. `#[serde(deny_unknown_fields)]` on every query DTO (`dto.rs:283-359`) means junk params (`&_=1`) are a **400**, not a cache miss — a real, already-shipped anti-cache-busting property worth preserving and testing.

**Gap:** three concrete holes.

1. No committed CF Cache Rule for `/api/quran/**` ⇒ `.json` is uncached by default ⇒ `s-maxage=86400` is decorative.
2. **Query permutation** still fragments: `?from=1&to=300&script=uthmani` and `?script=uthmani&to=300&from=1` are different cache keys for identical content (CF does not sort query params off Enterprise). 8 permutations × 21 ranges × N sources = unbounded edge-cache entries and origin misses.
3. `Vary: Accept` is emitted on **every** reader path (`hooks.server.ts:152-154`, `appendVaryAccept`) but Cloudflare ignores `Vary: Accept` in its cache key — so an HTML response can be served to a markdown-preferring client (correctness), and the markdown variant is _also_ obtainable at a distinct URL (`/x.md`), so the header adds fragmentation without adding capability.

**Fix:** (a) commit and document the Cache Rules: `Cache Everything` for `/api/quran/**` and `/offline/pack.*`; (b) for the endpoints whose query is non-semantic (`NoQuery` with `deny_unknown_fields`: `/surahs`, `/juzs`, `/pages`, `/rukus`, `/hizb-quarters`, `/manzils`, `/sajdas`, `/sources`, `/scripts`) use "Ignore query string" — safe because the DTO rejects every query param; (c) for the parameterized range endpoints, **301 to a canonical query order** at the origin (the code already has a `canonical()` helper for ETags at `controller.rs:145-156` — reuse it for URL canonicalization) so one cache entry per content object; (d) drop `Vary: Accept` from the HTML response and rely on the `Link: rel="alternate"` + `/x.md` URLs (keep `Vary: Accept` on the 404 and the negotiated markdown responses).

### A7 — Crawl / sitemap storms

**Stops it today:** nothing beyond the static-file path being cheap. The web sitemap is prerendered and streamed (`web/src/routes/sitemap.xml/+server.ts`), ~808 reader entries; `web/static/robots.txt` is `Disallow:` (allow all) with the correct sitemap URL. The API's `robots.txt` (`router.rs:236-246`) is ruxlog residue — it advertises `ruxlog.com/sitemap.xml` and disallows `/api/` — and is only reachable at `/api/robots.txt`, so it is dead weight, not a policy.

**Gap:** no crawl-rate policy for the reader site, no `Crawl-delay`, no separation between "indexable HTML" and "expensive `.md`/SSR variants". `.txt`/`.md` variants are already `X-Robots-Tag: noindex, follow` (`web/static/_headers`, `hooks.server.ts:148-151`) — good.

**Fix:** low priority. Add a `Crawl-delay`-free but explicit policy in `web/static/robots.txt` disallowing `/offline/` (the pack should be fetched deliberately, not crawled), and add a `Sitemap:`-adjacent `llms.txt` line pointing at the bulk-data doc. Do not attempt to throttle Googlebot.

### A8 — LLM harvesting

**Stops it today:** nothing, deliberately. `llms.txt` → `renderLlmsIndex()`, `llms-full.txt` → marketing pages only, `.md` twins for every reader path.

**Gap:** translated `.md` is `prerender = false` SSR (uncached) → each LLM fetch of a translated page consumes the A1 budget. A polite crawler fetching 378 × 808 `.md` URLs would exceed 300 k internal calls.

**Fix:** either (a) prerender the translated `.md` variants (they are locale-independent and deterministic — the de-localization machinery already exists at `web/server.ts:670-688`), or (b) give translated `.md` the same 7-day disk cache as the HTML (it is currently excluded by `hooks.server.ts:351` `!event.url.pathname.endsWith(".md")`), or (c) serve a bounded `.md` set (the surah-level pages only) and 404 the rest. Option (b) is the smallest change with the largest effect: it makes the LLM path cost one render per (source, kind, index, ui-locale) per build.

### A9 — Credential stuffing against the content API

**Stops it today:** the question does not arise — the content API has no keys, no sessions, no auth (`quran_v1/mod.rs:11-53`; `public_cors_layer().allow_credentials(false)` at `cors.rs:20`). The auth API has a per-account limiter (`login:{email}`, 3/360 s → 1 h) _plus_ 100/min/IP.

**Gap:** none today. **Forward risk:** if the bulk path (§4.4) introduces an API key, that key becomes a credential in a system with no credential-limiter infrastructure on the content path. Mitigations to bake in from day one: store only an HMAC of the key, never log it (note `crates/rux-request-gate/src/layer.rs:323-330` logs `identity = %id_key` at debug — a raw key in a bucket key would be logged), make it revocable, and give it a per-key bucket and a per-key concurrency cap.

### A10 — DDoS

**Stops it today:** Cloudflare at L3/L4. At L7 the API is capped per IP unit at 600/min; the web origin is capped at nothing.

**Gap:** as A1. A single IP can saturate the translated-reader path; a botnet of 100 IPs gets 60 k req/min at the origin. Mitigation is the A1/A6 work, not a WAF.

---

## 4. The defence design that does not fingerprint readers

Design constraint from `DESIGN-v1.md:35`: **no device payload on `/t/**`, `/{surah}/t/**`, the quran content API, or internal SSR** — machine-guarded. Everything below respects that. Nothing below needs a reader-side script, a cookie the reader must accept, a canvas probe, or a JA4.

### 4.1 Per-credential / per-API-key limits

The content API has no credentials, and adding auth to the reader path is a product regression (the whole point is anonymous reading, and `docs/quran-system.md:249-251` promises auth-neutral shared output). So:

- **Do not** add a session requirement to the reader path. Do **not** add "anonymous session required" either: that is a cookie-based fingerprint with extra steps and it breaks `.md`/crawler delivery.
- **Do** reuse the existing per-credential primitive where a credential exists: `services/abuse_limiter.rs` already implements per-key counters (`login:{email}` at `auth_v1/controller.rs:133-140`, `totp:{user_id}` at `:40-46`, `email_verify:{user_id}` at `email_verification_v1/controller.rs:48-49`). Extend that pattern to authenticated surfaces (bookmarks, notifications) — not to reading.
- **Do** introduce a key only for the _documented bulk path_ (§4.4), with: HMAC-only storage, per-key bucket, per-key concurrency cap, revocation, and a contact email. That converts the highest-volume consumer from anonymous to identified.

### 4.2 Per-ASN / network-class limits

**Implementable, with a caveat.** The origin has no ASN today: `resolve_client_ip` reads `CF-Connecting-IP` (`middlewares/client_ip.rs`), and Cloudflare sends no ASN header. Two options:

**(a) Worker-injected (recommended, ~10 lines + deploy).** A Cloudflare Worker on the reader + API paths sets `X-Eq-Edge-ASN: <request.cf.asn>` and `X-Eq-Edge-ASN-Org: <request.cf.asOrganization>` and **strips any client-supplied `X-Eq-Edge-*` first** (the canonicalisation bug class called out in A1-F6). `request.cf.asn` is available on all plans, IPv6 included. No signature is needed _for this purpose_: the value is a cost-multiplier hint, not an identity — a forged value lets an attacker widen their own bucket, never impersonate another user. (Signature/HMAC is a separate question owned by DESIGN B4; if the Worker is built, follow that design's strip-and-sign discipline anyway.)

**(b) No Worker: baked IP→ASN table.** Build-time generated from a free daily feed (Team Cymru `iptoasn` TSV or the RIR split DBs), shipped as a sorted `u32` array + binary search — pure in-process, zero DB ops, consistent with the repo's "baked id maps, never remote metadata" rule (`AGENTS.MD`). Cost: ~1 day, ~5 MB v4 table. IPv6 is the gap (a radix trie over ~1 M v6 prefixes is real work); mitigate with a small hand-maintained datacenter-prefix list (AWS/GCP/Azure/DO/Hetzner/OVH/Cloudflare) which is where the high-value scraping actually comes from.

**Values (see §6).** ASN buckets must be _much looser_ than the IP bucket and must never ban. R2 §2.2 is decisive: 4 of 5 requests from residential-proxy IPs are legitimate household traffic, and a Quran app's core audience is mobile/CGNAT/shared phones. Treat the ASN bucket as a **cost multiplier and correlation edge only** (R2 §2.4 verbatim).

### 4.3 Response shaping (progressive disclosure) — and the honest limit

The research's prescription ("page-level rather than full-text dump endpoints") is **already implemented**: `RESPONSE_CAP = 300`, `cursor`-paginated, `keys` capped at 50. Further narrowing has to be chosen carefully:

- **Do not narrow Arabic.** It is the public-domain asset and the mission. Narrowing it raises scraper cost by a constant factor while raising _our_ request count by the same factor, and the corpus is already obtainable in one 590 KB request (§1.5).
- **Do narrow by concurrency, not by size.** A reader fetches 1–3 range chunks per view; a scraper opens 20+ parallel connections. A **per-identity in-flight cap** (4 for anonymous, 16 for a bulk key) is the single best response-shaping lever: it cuts scraper throughput by the parallelism factor without touching legitimate latency, and it needs no size changes, so no reader regression. Implement as a small `tower` layer or an in-flight counter inside the gate crate next to `Escalation` (bounded map, in-memory, no DB).
- **Do shape translations** (the defensible asset): keep reader-facing reads at ≤300 ayahs, but make full-source traversal go through the documented download (§4.4) rather than 378 × 21 API calls. Concretely: add `nextCursor`-style bounded pagination to `/quran/sources/{id}/range` (already present) and have `/quran/sources` advertise `download_url` more loudly (add a `bulk` object with `bytes`, `sha?`—no, not sha (hard rule)—`bytes`, `licence`, `updatedAt`, `bulkHint`).
- **Do not** add a "burst then 429" cliff: prefer shaping (slower responses, narrower pages) over blocking, so a wrong decision costs a real reader a second and a scraper a dollar (R2 §10 item 8).

### 4.4 A documented bulk-data path that is cheaper than scraping (the research's #5)

**Most of it already exists; it is just not documented.** Deliverables:

1. `docs/bulk-data.md` + a `Bulk data` section in `web/src/lib/seo/render.ts`'s `renderLlmsIndex()` (so `/llms.txt` points at it) and in `web/static/robots.txt`-adjacent docs.
2. `GET /.well-known/quran-bulk.json` (static, immutable-ish): for each Arabic script and each of the 378 translations — `id`, `language`, `bytes`, `url` (`https://r2.easyquran.fyi/...`), `licence`, `attribution`. Generated at build time from the same baked catalogue the app uses (`db/quran/translations/index.min.json` → `web/src/lib/data/translations.json`), never from remote metadata.
3. A stated policy: "bulk download is free; do not scrape the API; if you need more, contact `X`; we publish what we host and why." This is the identification mechanism — a hostile anonymous scraper becomes an identified, contactable consumer, which is the _only_ lever that survives IP rotation (R2 §10 item 5).
4. Optionally a key with a generous quota (6,000 req/min, 16 concurrent) for API-shaped consumers, so the API is not the slow path. Key only if demand appears; the R2 path is better for everyone.

Cost: ~1 day. Impact: removes the _motive_ for stealth tooling (R2 §10 item 5) and gives a legal/contact handle. It does not and cannot stop a determined copier — and should not try.

### 4.5 Bot-only challenges on expensive endpoints

Turnstile exists (`services/turnstile.rs`), is registration-only, and must **not** go on the reader path (R2 §9 row 2: false positives against real users with DevTools open; and `.md`/crawler delivery must stay scriptless). Where it can go:

1. **Cold, expensive server-side work**: `/quran/sources` and `/quran/scripts` when the verification memo is cold (`controller.rs:808-844, 898-938`). A challenge there is invisible to readers (the memo is warm in practice) and blocks the HEAD fan-out (F6 below).
2. **Above-budget translation bulk**: not on the reader, but on `/quran/sources/{id}/range` for anonymous callers who exceed the "polite" budget (e.g. > 40 range requests / 10 min from one identity) — challenge, then allow, and set an HMAC'd `Path=/api` pass cookie valid 24 h (path-scoped so it never touches `/t/**` document requests or the disk cache — the same trick DESIGN-v1 uses for the device token).
3. **Never** on `/offline/pack.*` (breaks the SW install), never on Arabic reader routes, never on `.md`/`.txt`.

Implementation notes for this stack: `verify_registration` must be generalized to `verify_token(token, remote_ip)`; the verify call is a 5 s-timeout outbound POST, so it belongs only on cold/challenged paths; the pass cookie should be `HttpOnly; Secure; SameSite=Lax; Path=/api` and HMAC'd with an HKDF subkey off `COOKIE_KEY` (per A1-F8 discipline) — never a raw boolean.

### 4.6 Caching / CDN strategy (the largest available win)

All of this is deployment-owned and currently undocumented; it must become a committed contract because it _is_ the reader-path defence.

| Rule                                  | Target                                                                                                        | Why                                                                                                                                                            |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Cache Everything, respect origin TTL  | `/api/quran/**`                                                                                               | `s-maxage=86400` becomes real; 21 Arabic range objects + ~808 page objects absorb essentially all read traffic, and a cached request costs the origin **zero** |
| Ignore query string                   | `/quran/surahs`, `/juzs`, `/pages`, `/rukus`, `/hizb-quarters`, `/manzils`, `/sajdas`, `/sources`, `/scripts` | DTOs are `NoQuery` + `deny_unknown_fields`, so the query is non-semantic; kills permutation fragmentation                                                      |
| Canonical 301 on parameterized routes | `/quran/ayahs`, `/quran/sources/*/range`, `/quran/search`                                                     | one cache entry per content object instead of 2^k permutations (reuse `controller.rs:145-156`)                                                                 |
| Cache Everything, immutable, long TTL | `/offline/pack.*.json`, `/_app/immutable/*`, `/_quran/*`                                                      | A2 bandwidth; the pack is already immutable and content-addressed by build id                                                                                  |
| Do not cache                          | `/offline/manifest.json`, `/quran-meta/*`, `/*.html` (SSR), `/api/**` private                                 | already `no-cache`/`private`                                                                                                                                   |
| Rate-limiting rule (CF)               | `/offline/*` and `/t/**`                                                                                      | the edge is the cheapest place to hold a per-IP bucket; free-plan rate limiting is limited, so treat the origin limiter (A1.2) as the primary                  |

**Cookie handling must be fixed at the origin, not the edge**: an anonymous `ruxlog.sid` should not be `private, no-store` and should not bypass the disk cache (§A1 fix 1). That single change converts the majority of translated reader traffic from SSR to disk-cache hits.

### 4.7 Detecting harvest _patterns_, not identities

Everything here is visible at the API today, costs no DB write, and needs no client cooperation. All state is in-memory and bounded (the existing precedent is `EscalationEngine`'s `max_tracked_identities = 10_000`, `config/settings.rs:374`).

| Signal                                              | Visible at  | Implementation                                                                                                                                                                                                                   | Verdict                                                                                                                                                 |
| --------------------------------------------------- | ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Sequential enumeration**                          | API         | per-identity ring of the last K request shapes (`from/to`/`cursor`/`surah`/`page`/`source`); score monotonic progression, ascending sweeps, sorted `keys=` lists                                                                 | implementable, cheap, **high value** — the FP-Inconsistent lesson is "change detection over what the sender already sends" (R2 §3)                      |
| **Interval uniformity / impossible sustained rate** | API         | EWMA of inter-arrival variance per identity; compare against the identity's own baseline                                                                                                                                         | implementable; **challenge-only** (CGNAT bursts are legitimate)                                                                                         |
| **ETag indifference**                               | API         | count requests with no `If-None-Match` from an identity that has received ETags (the API already emits weak ETags, `cache.rs:16-27`)                                                                                             | implementable; a real browser revalidates, a scraper does not                                                                                           |
| **Referrer / navigation-chain absence**             | API         | count requests with no `Referer` and no `Sec-Fetch-*`                                                                                                                                                                            | implementable; **weak** (curl and fetch both omit it; and CF passes client headers through, so it is real data)                                         |
| **No-asset-requests**                               | **not** API | requires correlating `/api/quran/**` with `/_app/immutable/*`; Traefik routes `/api` to the api container directly (priority 100, `docker-compose.yml:184-189`), so the web never sees API traffic and the API never sees assets | **not implementable at the origin.** Options: CF Logpush → sidecar (async, not inline, R2 §7 item 4), or accept the loss. Recommend **defer**           |
| **UA ↔ `sec-ch-ua*` coherence**                     | API         | both are forwarded by CF; check for ≥2 independent inconsistencies (headless UA with `Chromium` brand, empty `userAgentData`, missing `sec-ch-ua-form-factors`)                                                                  | implementable but **a floor, not a wall** (defeated by Patchright/rebrowser in one release — R2 §9 row 1). Use only as a _scoring_ input, never a block |
| **Header order / casing**                           | —           | **impossible** behind Cloudflare (R2 §7: "definitively lost")                                                                                                                                                                    | do not attempt                                                                                                                                          |
| **JA4/JA3, Bot Score, verified bot**                | —           | Enterprise-only; no Worker in repo                                                                                                                                                                                               | do not attempt                                                                                                                                          |

**The rule that makes this safe:** every one of these is an _input to a shape score_, never a verdict. Escalation stays what it is today — external-IP-only, default-off, 4xx-history-driven, never-permanent (`config/settings.rs:349-378`, `main.rs:867-895`) — and the reader path's response to a high score is **narrowing and challenge**, never a block. A wrong call on a Quran reader in a country where reading is risky is worse than a scrape.

---

## 5. Attacking my own design

| Measure                                                     | How a determined scraper beats it                                                                                                                                                                                                                               | Residual                                                                                                                                                                                            |
| ----------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Per-IP limiter at the web origin                            | Rotate IPs. A $10 residential pool gives thousands of exits; R2 §2.1 documents 30–100 M IPs available.                                                                                                                                                          | 600/min/IP × N IPs. The limiter bounds _one_ IP's blast radius; it does not bound a pool. **Residual: high.**                                                                                       |
| Restore disk cache for anonymous cookies                    | Nothing to beat — the attacker is no worse off; this only removes _our own_ self-inflicted bypass. An attacker who presents an _auth_ cookie still bypasses, which is now a _detectable signal_ (an auth cookie on a request whose session is unauthenticated). | Cache restores ~all legitimate load; the attack surface shrinks to "attacker forces SSR", which the per-IP limiter then bounds. **Residual: low for A1, medium for A3.**                            |
| ASN / network-class bucket                                  | Choose a pool spanning many ASNs. Residential pools are _built_ for ASN diversity (44.1% of IPs appear in more than one provider's pool, R2 §2.2), and mobile CGNAT puts hundreds of subscribers behind one IP _and_ one ASN.                                   | The bucket mostly catches datacenter/cloud scrapers (few ASNs) and single-carrier farms. **Residual: high against funded T2/T3, low against T1.** It is a cost multiplier, exactly as R2 §2.4 says. |
| Worker-injected `X-Eq-Edge-ASN`                             | Set the header themselves. Worker strips it first; if the origin is not locked to CF ranges (`deploy/README.md:377-392`, an owner-run procedure, not code), everything here is void.                                                                            | Forgery only widens the attacker's own bucket — it cannot impersonate another user. **Residual: acceptable**, contingent on the CF-range lockdown being real.                                       |
| Progressive disclosure (concurrency cap, no size narrowing) | Open more connections across more IPs.                                                                                                                                                                                                                          | 4 in-flight/IP is a 5–10× throughput cut per IP for a scraper, ~0 for a reader. **Residual: medium** — bounded by the IP bucket.                                                                    |
| Canonical query redirect + Cache Everything                 | Nothing (it is a pure win). Only risk: a redirect loop or a 301 on a query the app's own fetcher uses (`docs/quran-system.md:161` says the app chunks to 300 ayahs) — must be tested against the app's own fetcher.                                             | **Residual: none.**                                                                                                                                                                                 |
| Documented bulk path                                        | Ignore it and scrape anyway; copy without attribution.                                                                                                                                                                                                          | The Arabic corpus is public-domain — nothing to enforce. Translations are licensed per source; the _only_ lever is a contact + a takedown request. **Residual: irreducible.**                       |
| Turnstile on cold/bulk endpoints                            | Solve with a service (R2 §7.2: solver services defeat challenges at scale; FP-Inconsistent measured ~45–53% evasion against two best-in-class commercial vendors).                                                                                              | Stops naive tooling and raises T1 cost. **Residual: high against funded attackers** — which is why it must never be the primary defence.                                                            |
| Harvest-pattern scoring                                     | Add jitter (`sleep 2-5s`) and randomize order. A single full-corpus pass (21 requests, or 378 R2 GETs) is _nothing_ to a jittered scraper.                                                                                                                      | Real value is against _sustained_ harvesting (378 × 808 keys) and naive tooling. **Residual: high for a single pass, medium for sustained harvest.**                                                |
| Offline-pack byte budget                                    | Distribute across IPs; each pull is 590 KB from the edge.                                                                                                                                                                                                       | The attacker pays more than we do. **Residual: low.**                                                                                                                                               |
| Split internal bucket by route                              | Flood both classes.                                                                                                                                                                                                                                             | Damage is now partial (`.md` flood no longer kills HTML). **Residual: medium** — the global backstop still exists.                                                                                  |

**Two things I would not build**, on the evidence: (1) any client-side JS coherence battery for the reader path — the design forbids it (`DESIGN-v1.md:35`), R2 §9 row 3 shows it decays after each round, and A1-F1 shows client-authored booleans are one line of JS; (2) header-order/TLS-fingerprint work — unavailable behind Cloudflare off Enterprise and explicitly listed as lost (R2 §7).

---

## 6. What cannot be defended, and the honest posture

**Cannot be defended (state this in the design, do not pretend otherwise):**

1. **Copying the Arabic corpus.** It is public-domain, immutable, and must stay freely distributable (`AGENTS.MD`; `docs/quran-system.md:15-37`; R2 §8.3). Today it is one 590 KB request. The target is _cost per copy > cost of an existing dataset_ (R2 §9 row 17) — and even that is mostly about making the honest path the fast path, not about prevention.
2. **Copying the translation corpus.** 616 MB, public URLs, one API call to enumerate. Licensing is the lever, not access control.
3. **Reading with automation at a rate indistinguishable from a fast human, from many IPs.** R2 §10 item 8: on a Quran reader the false-positive population _is_ the audience — mobile users, CGNAT, shared phones, low-end devices, national filters, Tor users in jurisdictions where reading is risky.
4. **Identifying a reader.** No fingerprint, no JA4, no Bot Score, no header order. The reader path is anonymous by design and must stay so (`DESIGN-v1.md:35`).
5. **Volumetric DDoS.** Cloudflare's problem.

**The honest posture:**

- The reader path is a **cost-management and availability problem, not an identity problem.** The defender's goal is: _a wrong decision costs a real reader a second; a hostile actor costs a dollar_ (R2 §10 item 8).
- The abuse that actually threatens the product is **not scraping** — it is a single anonymous IP turning off translated reading (A1) and burning bandwidth (A2). Both are fixable this week, cheaply, without any fingerprint.
- Scraping is answered by **making the legitimate bulk path the fastest path** and by **cost multipliers**, not walls. Success is measured as: origin requests per read (down), cache hit ratio (up), cost per full-corpus copy (up), and identified bulk consumers (up) — never "scrapers blocked".
- Any enforcement above throttling stays off the reader path. The existing escalation engine is external-IP-only, default-off, and 4xx-history-driven (`config/settings.rs:349-378`) — keep it that way, and keep it away from anonymous reading.

---

## 7. Recommended rate-limit key scheme and values (Q6)

### 7.1 The existing scheme, exactly

`crates/rux-request-gate/src/layer.rs:278`:

```rust
let key = format!("ratelimit:{}:{}", id_key, path);
```

- `id_key` (`:239-243`): `canonical_unit_key(ip)` for a verified external IP — IPv4 `a.b.c.d/32`, IPv6 `2001:db8::/64` (`:378-390`) — else `internal-webssr`, else `unknown`.
- `path`: `PathKey::Raw | Fixed(label) | Matched` — `Fixed("quran-v1")` on all three public limiters, `Fixed("quran-health")` on readiness, `Matched` on the private API nests.

Observed key forms: `ratelimit:203.0.113.5/32:quran-v1`, `ratelimit:2001:db8::/64:quran-v1`, `ratelimit:internal-webssr:quran-v1`, `ratelimit:203.0.113.5/32:/quran/search`, `ratelimit:unknown:/auth/v1/log_in`.

### 7.2 Proposed scheme — additive, no collisions

| Tier                         | Key                                                                                         | Value                                                                                                             | Persisted?      | Escalation?       |
| ---------------------------- | ------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | --------------- | ----------------- |
| 0 (existing)                 | `ratelimit:{ip_unit}:quran-v1`                                                              | **600/min**                                                                                                       | yes             | yes (default-off) |
| 1 (new) network class        | `ratelimit:net:{class}:{asn}:quran-v1` where class ∈ `hosting` \| `mobile` \| `residential` | `hosting` **1,200/min**; `mobile` **3,000/min**; `residential` **3,000/min**; ASN unknown ⇒ no bucket (fail open) | **no**          | **no**            |
| 2 (new) harvest shape        | `harvest:{ip_unit}` in a **separate in-memory map**, not the gate store                     | not a rate — a shape score with hysteresis; action = narrow (`limit≤30`, disable 300-ayah pages) for 10 min       | no              | no                |
| 3 (new, only with bulk keys) | `key:{hmac16}` where `hmac16` = first 16 hex of `HMAC(subkey, api_key)`                     | **6,000/min**, 16 concurrent                                                                                      | yes (revocable) | no                |
| 4 (new) internal split       | `ratelimit:internal-webssr:quran-v1-md`, `…:quran-v1-range`                                 | 300/min each, global key unchanged at 600/min (raised after measurement)                                          | yes             | no                |

**Values rationale.**

- `hosting` 1,200/min = 2× the per-IP ceiling: a datacenter ASN is effectively one operator, so a generous aggregate still caps a 10-IP cloud scrape at 1,200/min instead of 6,000.
- `mobile`/`residential` 3,000/min: a carrier ASN legitimately carries thousands of readers behind CGNAT; this is a _cost multiplier_, never a ban (R2 §2.4, §6 row 7).
- The ASN tier is deliberately **looser than the IP tier** and must never be the reason a request is blocked for a long time — 429 with `Retry-After` and no ban.

### 7.3 Collisions — verified, not assumed

1. **String collisions: none.** No existing key starts with `net:` or `key:`; `unknown`, `internal-webssr`, and canonical IP units are disjoint from those prefixes. IPv6 units contain `:`, but keys are only ever compared as opaque strings — nothing splits on `:` to recover a unit (`rate_limit_store.rs:180-229` upserts by exact key; `delete_ban_rows` takes exact strings; `admin_bans_v1` export filters `quran-ban:` rows by `BanUnit::parse`, and `asn:64500`/`net:mobile:64500` fail `IpNet` parsing — `dto.rs:31-38`, with `totp:42` pinned as non-parseable at `controller.rs:304`, and the export filter at `controller.rs:36-49`). **So ASN/credential keys can never enter the ban export or be mistaken for ban units.**
2. **Bucket collisions: none by construction**, because each tier uses its own `id_key` and `PathKey::Fixed("quran-v1")` composes a distinct full key. `ratelimit:net:mobile:64500:quran-v1` ≠ `ratelimit:64500…` etc.
3. **Escalation collisions: none, but this needs a code change.** Escalation is gated on `RequestIdentity::is_external_ip()` at three call sites (`layer.rs:253, 304, 351`). If the ASN tier reuses `RequestIdentity::External(ip)`, its keys would enter the escalation state machine and could become bannable units — **a ban on a /64-shaped ASN bucket is exactly the collateral-ban failure A1-F3 warns about.** Fix: extend the closed enum with a non-IP variant (`ExternalNetwork { ip, asn }` or `NetworkClass(String)`), have `bucket_key()` return `net:{class}:{asn}`, and have `is_external_ip()` return `false`. Add a test mirroring `internal_identity_bucket_key_is_not_an_ip` (`ip.rs:139-148`) asserting the new label never parses as an IP and never escalates.
4. **Persistence collision: real, and it is the one that matters.** `rate_limit_store::flush` upserts **every live snapshot row** in one transaction on the shared connection (`rate_limit_store.rs:199-219`), and `InMemoryStore::snapshot()` returns every bucket with a live expiry (`crates/rux-request-gate/src/store.rs:123-150`). Adding ASN + credential + harvest dimensions multiplies the 10 s flush. **Requirement: the new tiers must be in-memory-only.** Add a `persist: bool` on the bucket (or filter `ratelimit:net:`/`key:`/`harvest:` prefixes out of `snapshot()`), and pin it with a test. The existing IP tier must keep persisting, because the admin un-ban clears `ratelimit:{unit}:quran-v1` by exact string (`deploy/README.md:418`).
5. **Log collision: avoid the raw key.** `layer.rs:323-330` logs `identity = %id_key` at debug. A raw API key placed in `id_key` would be logged. Use the HMAC form.

### 7.4 What the new tiers do NOT do

They do not identify anyone. They do not touch the reader's client. They do not add a cookie, a script, a challenge or a block to the reading path. They are aggregate cost ceilings plus a shape score, and the only reader-visible effect of a false positive is a slightly slower page.

---

## 8. Findings

### F1 — The web origin has no limiter, and every translated render funnels through one global 600/min internal bucket, so one anonymous IP can starve translated reading site-wide

**Severity: blocker. NEW.**
Evidence: `rust/backend/api/src/middlewares/rate_limit.rs:68-79` (`quran_internal_layer`: `settings.rate_limit.internal_requests_per_minute` = 600 default, `PathKey::Fixed("quran-v1")`, one global bucket for the whole site); `docs/quran-system.md:277-281`; `web/src/routes/(application)/t/[lang]/[translator]/page/[n].md/+server.ts:10-22` (`prerender = false` → SSR → internal API); `web/server.ts` + `web/src/hooks.server.ts` contain no rate limiting at all (grep-verified); Traefik routes `/api` straight to the api container so the web never sees API traffic (`docker-compose.yml:184-189`).
Why it breaks: an attacker sends ~600 requests/min to any translated reader URL (or its `.md` twin) from one IP. Requests 1–600 consume the entire site-wide internal budget; request 601+ returns 429 for _every_ legitimate translated page and `.md` on the site. No credential, no fingerprint, no cost. The 600/min content limiter does not apply (it is a different identity, `rate_limit.rs:57-63`).
Secondary hazard on the same line: `web/src/lib/server/quran-translation-page.ts:34,51,63` falls back to `PUBLIC_QURAN_API_BASE` **without** the internal token when `INTERNAL_QURAN_API_BASE` is unset. In that state every SSR call arrives as an external request from the web container's single egress peer, so all reader SSR collapses into one 600/min IP bucket — the same global ceiling by a different route, plus it is indistinguishable from abuse.
Fix: (1) per-IP in-memory limiter + byte budget in the web fetch pipeline (`web/server.ts`), 240 reader-page req/min/IP, 600/min for static/`.md`/`_quran`; (2) split the internal bucket by route class (`…:quran-v1-md`, `…:quran-v1-range`) with the global key as backstop; (3) raise the global backstop after measuring real capacity; (4) restore the disk cache for anonymous traffic (F2) so a cache hit consumes zero internal budget; (5) make a missing `INTERNAL_QURAN_API_BASE` a production boot error in the web image, not a silent fallback.

### F2 — Any cookie defeats both the disk cache and the edge cache, and the anonymous CSRF cookie is the _default_ state

**Severity: high. NEW.**
Evidence: `web/src/hooks.server.ts:345-351` (`cacheable = … && !requestHasCookie`); `:130-137` (`privateMode = requestHasCookie || responseSetsCookie(response)` → `Cache-Control: private, no-store`); `web/src/lib/server/quran-disk-cache.ts:11-12` (7-day TTL, 256 MB); `router.rs:57-60` (`/csrf/v1/generate` mints an anonymous session, `main.rs:781-788` sets `ruxlog.sid`).
Why it breaks: the API mints `ruxlog.sid` for anonymous CSRF, and the session cookie is host-scoped, so the _default_ state after any `/api` call is "cookie present" → `private, no-store` + disk-cache bypass + full SSR on every reader page. The single most effective cache in front of the SSR path is off for most real users, and an attacker needs one header to force the expensive path on demand. This is the multiplier that turns F1 from "theoretical ceiling" into "practical outage".
Fix: distinguish _authenticated_ from _cookie-present_. Set a second HttpOnly, non-PII marker cookie only on login (`ruxlog.auth=1`) and gate `requestHasCookie` on that; keep the disk cache auth-neutral (`docs/quran-system.md:249-251` guarantees no user state in shared output). Treat an auth-cookie request with an unauthenticated session as a harvest signal (F9).

### F3 — The offline pack is a 14.1 MB unauthenticated download served by the un-limited web origin, and `.json` is not edge-cached by default

**Severity: high. NEW.**
Evidence: `web/scripts/gen-offline-pack.ts:152-196` (pack = every prerendered `__data.json`; measured on this tree: `14,158,643` bytes raw, `589,573` bytes brotli, 1,635 entries); `web/server.ts:508-521` (`lateStaticResponse` for `/offline/*`); `web/src/hooks.server.ts:135` + `web/static/_headers` (`public, max-age=31536000, immutable`); no limiter anywhere in the web origin.
Why it breaks: unlimited requests; 14.1 MB per request for any client that omits `Accept-Encoding`, 590 KB otherwise. One IP at 600 req/min = up to 8.4 GB/min of origin egress. It is also the cheapest full-Arabic-corpus exfil path (one request), so cost and bulk-path concerns coincide.
Fix: per-IP byte budget on `/offline/*` (burst 5, cap 20/IP/day — the SW downloads once per install per build id); commit a CF Cache Rule (`Cache Everything`, immutable, long edge TTL) so the origin sees one fetch per POP per deploy. No challenge — the SW install path must not break.

### F4 — `/api/quran/**` has no committed edge-cache contract, and the cache key is attacker-fragmentable by query permutation and `Vary: Accept`

**Severity: high. NEW.**
Evidence: `docs/quran-system.md:146` ("Quran responses are edge-cacheable"); `quran_v1/cache.rs:5-8` (`s-maxage=86400`); no Cache Rule in the repo or in `deploy/README.md`; `hooks.server.ts:152-154` (`appendVaryAccept` on every reader path, which Cloudflare does not honour in its cache key); `#[serde(deny_unknown_fields)]` on every query DTO (`quran_v1/dto.rs:283-359`) — which correctly 400s junk params but does not stop param reordering/duplication.
Why it breaks: Cloudflare does not cache `.json` by default, so the origin renders every content request and the only brake is 600/min/IP unit. With N IPs the origin serves N × 600/min of 50–200 KB JSON. Permuted valid params produce distinct cache entries for identical content; `Vary: Accept` adds a second class that the edge ignores (correctness risk: HTML served to a markdown-preferring client).
Fix: commit the Cache Rules — `Cache Everything` on `/api/quran/**` and `/offline/pack.*`; `Ignore query string` on the `NoQuery` endpoints; **301 to a canonical query order** on the parameterized range endpoints (reuse `controller.rs:145-156`); drop `Vary: Accept` from HTML responses and rely on the existing `/x.md` URLs + `Link: rel="alternate"`. Test the canonical redirect against the app's own chunked fetcher (`docs/quran-system.md:161`).

### F5 — The cheapest bulk path exists but is undocumented: 21 requests for all Arabic, one 590 KB pack, and 378 direct R2 URLs for 616 MB of translations, with no bulk contract, no key, no contact

**Severity: medium. NEW.**
Evidence: `quran/store.rs:5` (`RESPONSE_CAP = 300`); `quran_v1/controller.rs:1272-1275`; `controller.rs:858,978-990` (`download_url` = `{public_url}/tanzil/…`); `web/src/lib/quran/environment.ts:9` (`https://r2.easyquran.fyi`); `db/quran/translations/index.min.json` (378 entries, 616 MB); `renderLlmsIndex()` has no bulk section; no `docs/bulk-data.md`.
Why it breaks: the research's #5 countermeasure ("a published, cheap, legitimate path so scrapers identify themselves", R2 §10 item 5) is 80% built and 0% signposted. A rational scraper's cheapest discovery path is the API, so they hit the API — consuming the F1/F3 resources — instead of downloading from R2. And there is no contact/attribution handle for the _translation_ corpus, which is the licensed asset.
Fix: `docs/bulk-data.md` + a `Bulk data` section in `/llms.txt` + a build-generated `/.well-known/quran-bulk.json` (id, language, bytes, url, licence, attribution per source) from the baked catalogue. State the policy: bulk download is free, do not scrape the API, contact X. Add a `licence`/`attribution` field to `SourceDto`.

### F6 — `/quran/sources` and `/quran/scripts` memoize only when _every_ artifact HEAD-verifies, so a partially published bucket turns one request into a 378-HEAD outbound fan-out

**Severity: medium. NEW.**
Evidence: `quran_v1/controller.rs:808-844` and `:898-938` (`if resolved.len() == advertised/total { *guard = Some(…) }`, else `NO_STORE` and recompute); `:945-1020` (`resolve_sources`: one HEAD per catalogue entry, `Semaphore::new(32)`); catalogue = 378 entries + advertised scripts.
Why it breaks: the memo is all-or-nothing. With a partially published R2 bucket (or one size-mismatched artifact), the guard never trips and **every** request re-runs the fan-out — up to 379 outbound HEADs per request, ×600/min/IP unit = ~227 k outbound R2 requests/min from a single IP, plus 32-way concurrency contention on the shared `http_client` (also used by mail/billing/FCM, per the code comment at `:996-997`).
Fix: memoize **per artifact** (a verified-set that grows) instead of all-or-nothing, keep `NO_STORE` while partial, and rate-limit the cold path (the existing content bucket is too loose). Optionally challenge the cold path (F7). Add a metric for cold-resolve count.

### F7 — Turnstile is registration-only; no challenge exists on any expensive endpoint and there is no challenge-pass mechanism

**Severity: medium. NEW.**
Evidence: `services/turnstile.rs:38-56` (`verify_registration` is the only entry point); sole caller `modules/auth_v1/controller.rs:470`; CSP already permits `challenges.cloudflare.com` (`web/src/hooks.server.ts:84`); no pass-cookie code exists.
Why it breaks: the only anti-automation primitive in the repo is unreachable from the paths that actually need cost control (cold source/script resolution, above-budget translation bulk, sustained harvest).
Fix: generalize to `verify_token(token, remote_ip)`; add challenges only on cold/expensive paths (F6, above-budget bulk) with an HMAC'd `HttpOnly; Secure; SameSite=Lax; Path=/api` pass cookie valid 24 h (path-scoped so `/t/**` and the disk cache are untouched — same reasoning as DESIGN-v1's device token). Never on Arabic reader routes, `.md`/`.txt`, or `/offline/*`.

### F8 — The limiter key space is IP-unit-only, so a residential/mobile pool defeats the sole content limit for free

**Severity: high. NEW.**
Evidence: `crates/rux-request-gate/src/ip.rs:38-57` (`RequestIdentity` = `External(IpAddr) | InternalService`); `layer.rs:239-243, 378-390` (canonical /32, /64); `rate_limit.rs:57-63` (the only content limiter); no ASN/network dimension anywhere; `R2-evasion-counters.md:88,96` (per-credential counters are the #1 countermeasure; per-IP is what a residential pool defeats for free; IP must be a cost multiplier, never allow/deny).
Why it breaks: the entire reader-path defence is 600/min per IP unit. A $10 residential pool multiplies it by the pool size, and CGNAT/mobile collapses many users into one unit (which is why it cannot be tightened). A1-F5 already established that the anonymous reader never carries a device signal, so the IP unit is _all_ there is today.
Fix: add the network-class tier (§7.2) — Worker-injected `X-Eq-Edge-ASN` (strip client-supplied `X-Eq-Edge-*` first) or a build-time baked IP→ASN table from a free daily feed. Extend `RequestIdentity` with a non-IP variant that returns `false` from `is_external_ip()` so ASN buckets can never escalate or be exported. Values: `hosting` 1,200/min, `mobile`/`residential` 3,000/min, unknown ⇒ no bucket. In-memory only, never persisted, never bans.

### F9 — No harvest-pattern detection exists, even though the signals are already on the wire

**Severity: medium. NEW.**
Evidence: no sequential-enumeration, interval-variance, ETag-indifference or referrer logic anywhere under `quran_v1/` or the gate crate; the API emits weak ETags (`quran_v1/cache.rs:16-27`) that a scraper ignores; `quran_v1/controller.rs:239-263` makes `cursor`/`from`/`to` the enumeration surface; the escalation engine tracks only 4xx history (`services/escalation.rs`, `config/settings.rs:349-378`).
Why it breaks: the FP-Inconsistent result (R2 §3) is that consistency/change detection over what the sender already sends halved evasion at ~48% with no new fingerprinting. Here nothing observes the cheapest, most robust signals (monotonic sweeps, uniform intervals, ETag indifference), so the only signal in play is raw volume — the one signal a pool defeats and CGNAT users trigger innocently.
Fix: an in-memory, bounded (10 k identities, mirroring `EscalationEngine`) shape observer on the content router: per-identity ring of the last K request shapes + EWMA interval variance + ETag-indifference count + referrer presence. Score only; action = narrow (`limit≤30` for 10 min) and, above a second threshold, challenge (F7). Never a block, never a ban, never persisted.

### F10 — Any new limiter dimension multiplies the 10-second flush because the store persists every live bucket

**Severity: medium. NEW.**
Evidence: `crates/rux-request-gate/src/store.rs:123-150` (`snapshot()` returns every bucket with a live expiry); `services/rate_limit_store.rs:180-229` (one transaction, `DELETE` expired + `INSERT … ON CONFLICT` **per row**, on the shared SeaORM connection); `main.rs:648-652` (flush task every 10 s); `rate_limit_store.rs:1` ("Do NOT add a DB write to the request hot path").
Why it breaks: the design's own rule is "0 DB ops on the request path" (`DESIGN-v1.md:12`), but the _flush_ is proportional to live key cardinality. Adding ASN (~thousands), per-credential, and harvest keys to the same map grows the 10 s transaction linearly and pushes write amplification onto the single connection.
Fix: mark the new tiers non-persistent (`persist: bool` on the bucket, or filter `ratelimit:net:` / `key:` / `harvest:` in `snapshot()`), keep the IP tier persisted (the admin un-ban clears `ratelimit:{unit}:quran-v1` by exact string, `deploy/README.md:418`), and add a test asserting the new prefixes never appear in a snapshot.

### F11 — One IP can cycle 378 translation ids and thrash the 8-entry / 48 MB corpus pool

**Severity: medium. NEW.**
Evidence: `config/settings.rs:299-301` (`QURAN_MAX_RESIDENT_TRANSLATIONS=8`, `QURAN_MAX_RESIDENT_TRANSLATION_BYTES=48 MiB`, idle TTL 1800 s); `quran_v1/controller.rs:1258-1351` (`source_range` cold-loads via `get_or_build`); 378 catalogue entries at 1.1–1.6 MB each (`db/quran/translations/index.min.json`); `main.rs:594-646` (demand flush rides the same 10 s task every 6th tick).
Why it breaks: 378 cold builds per cycle evict each other, so every request pays a disk read + parse, and the demand counter inflates the popularity flush — all from one anonymous IP at 600 req/min.
Fix: per-identity concurrency cap (§4.3) plus a per-identity cold-source budget (e.g. 5 distinct source ids per 10 min for anonymous callers); keep `no-store` on partial resolves; expose pool churn in metrics.

### F12 — The API's `robots.txt` is ruxlog residue, and no crawl policy covers the reader site's expensive variants

**Severity: low. NEW.**
Evidence: `rust/backend/api/src/router.rs:236-246` serves `Disallow: /admin/`, `/api/`, `/auth/` and `Sitemap: https://ruxlog.com/sitemap.xml` (reachable only at `/api/robots.txt`); the reader site's policy is `web/static/robots.txt` (`Disallow:` = allow all) with the correct sitemap; `.md`/`.txt` carry `X-Robots-Tag: noindex, follow` (`web/static/_headers`, `hooks.server.ts:148-151`); `/offline/` is not disallowed.
Why it breaks: an external observer reading `/api/robots.txt` sees another product's sitemap and a stale policy; crawlers are free to fetch `/offline/` (14 MB) and the translated `.md` SSR variants, which is the expensive class (F1/F3).
Fix: fix or delete the API's `robots.txt` (it is not the site's policy file); add `Disallow: /offline/` to `web/static/robots.txt`; add a `Bulk data` pointer to `/llms.txt`.

---

## 9. Bottom line for DESIGN-v1 Q3

1. **No fingerprint, and none is needed.** The reader path's real problem is cost and availability, and every mechanism that fixes it is server-side, credential-free, and invisible to readers: a web-origin limiter, a restored cache, split internal buckets, a committed edge-cache contract, an ASN cost multiplier, a concurrency cap, and a shape score.
2. **The three highest-value changes are not "abuse features"** — they are fixing the cache bypass for anonymous traffic (F2), adding a limiter to the web origin (F1), and committing the edge cache rules (F4). Together they remove the practical version of A1/A2/A3 without touching the reader.
3. **Scraping is answered by a door, not a wall** (F5): the bulk path already exists at 590 KB / 378 URLs; document it, publish the licence, and put a contact on it. That is the only measure that survives IP rotation.
4. **The device layer in DESIGN-v1 must not be extended to the reader path to solve this.** A1-F5 is right that the anonymous scraper never sends a payload; the correct conclusion is not "add a beacon", it is "the reader path is defended by cost multipliers and cache discipline, and the device layer stays where it is".
5. **Say the irreducible part out loud** in the design: the Arabic corpus is public-domain and copyable in one request, the translation corpus is public and licensed, and a determined scraper behind a residential pool will always be able to read faster than a human. The honest posture is cost management, not prevention — and the metric is cost per full copy, not bans issued.
