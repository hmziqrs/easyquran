# EasyQuran — web PWA & offline plan

> Status: **Implemented — Phases 0–4 landed on `feat/web-pwa-offline`**
> (written 2026-08-03, against `master` @ `fbb220e`).
>
> Goal: the whole web app installable and readable offline — every Surah, every
> page — with a boring, atomic update path when a new build ships.
>
> Serving assumption: the static Caddy container is **interim**. Translated SEO
> pages (deferred item 8 of the translations plan) will be rendered on demand
> with a TTL — ISR — which requires a dynamic origin. Every rule in this plan
> is therefore a **host-agnostic contract** (§6), not a Caddy config; §6.3
> specifies the target origin and §4/§5 are written so ISR routes slot in
> without changing the offline architecture.
>
> Companion docs: [`quran-web-delivery.md`](./quran-web-delivery.md) (how the
> Quran databases reach the browser), [`quran-ssg-optimization-plan.md`](./quran-ssg-optimization-plan.md)
> (page/data shape this plan builds on), [`quran-translations.md`](./quran-translations.md)
> (translation packs; its deferred "translated SEO pages" item is the ISR
> driver here).

---

## 1. Outcomes and non-goals

Four independent outcomes:

1. **Installable PWA that boots offline.** Launching the installed app (or any
   previously visited URL) with no network renders the reader, not a browser
   error page.
2. **Complete offline coverage on request.** A "Download for offline" action
   makes *every* route readable offline — not just visited ones — for roughly
   the cost of the Quran databases the app already stores (~10 MB extra, not
   the 40 MB of prerendered HTML).
3. **An atomic update system.** A deploy is detected within minutes by open
   tabs, applied as one all-or-nothing cache swap, and surfaced to the user as
   a single unobtrusive prompt. Reading position survives the update reload.
4. **Honest HTTP delivery.** Whatever serves responses — Caddy today, the
   ISR-capable origin later — sends the cache headers and compression this
   plan depends on. Today nothing is sent; §6 defines the contract once and
   each host implements it.

Non-goals:

- Offline write features (bookmarks/notes sync, auth) — local-first storage of
  those is a separate effort.
- Offline translations and audio — translation *text* offline is owned by the
  pack/OPFS design in `quran-translations.md`; this plan only reserves how the
  SW must treat translated (ISR) routes so the two don't collide (§4.2, §6.3).
  The SW already bypasses `/translations/`.
- Changing the Rust API, the R2 artifact layout, or the two Arabic databases.
- Adopting Workbox or any SW framework (see decision log, §12).

---

## 2. Current state (verified inventory)

### 2.1 What already exists

| Piece | Where | State |
|---|---|---|
| Web app manifest | `web/static/manifest.webmanifest` | Good: standalone, 192/512 icons, `any` + `maskable`. Missing `id`, shortcuts; `start_url` is `/`. |
| Service worker | `web/src/service-worker.ts` | Hand-rolled, registered in prod only by `web/src/lib/boot/service-worker.ts` (`kit.serviceWorker.register: false`, manual `register("/service-worker.js")`). Also owns FCM `push` / `notificationclick`. |
| SW precache | `eq-precache-${version}` | `build` + `files` (excludes `quran-meta/`, `_headers`, `_redirects`, `robots.txt`, `og.png`) + `/404.html`. **No prerendered pages, not even `/`.** |
| SW runtime cache | `eq-runtime-v1` | Stale-while-revalidate for other same-origin GETs. Never versioned, never swept, and cache keys keep query strings (SvelteKit's `__data.json?x-sveltekit-invalidated=…` params create duplicate entries). |
| Navigations | `networkFirstNav()` | Network → cached copy → `404.html`. Successful navigations are cached **into the version-keyed precache**, so the "visited pages" library is wiped on every deploy. No network timeout. |
| Quran databases | `web/src/lib/workers/opfs-cache.ts` | Downloaded from R2 (or `/_quran/` locally), sha256+size verified, stored in OPFS `easyquran/<contentVersion>/<id>.sqlite`, IDB fallback, in-memory last resort. `navigator.storage.persist()` requested. Old content-version directories are **never deleted**. |
| Content versioning | `web/src/lib/quran/manifest.ts` | `contentVersion` / `searchVersion` from the API (`/version`, `/scripts`, 3 s timeout) with baked fallback — already offline-safe. |
| App version file | `build/_app/version.json` | Emitted by SvelteKit (currently `{"version":"<build timestamp>"}`). **Unused**: no `kit.version` config, no polling, `updated` store never read. |
| Deploy | `deploy/Dockerfile.web` | `caddy file-server` — **no Caddyfile, no Cache-Control on anything, no compression**. `static/_headers` / `_redirects` are Cloudflare Pages/Netlify conventions and are inert on Caddy. Interim by decision: replaced by an ISR-capable origin when translated SEO pages land (§6.3). |

### 2.2 Measured weight (production build of `fbb220e`)

| Asset class | Count | Raw size | Notes |
|---|---|---|---|
| HTML pages | 1,311 (1,296 under `/app`) | 39.3 MB | ~30 KB avg; full ayah text prerendered per Surah-local page |
| `__data.json` | 1,308 | 8.3 MB | ~6.4 KB avg, max 100 KB (`/app/juz/30`); contains the same ayah text + route metadata |
| `_app/immutable/` | 77 files | 2.5 MB | includes `sqlite3-*.wasm` (848 KB) and fonts (532 KB, Amiri Arabic 108 KB) |
| Quran databases | 2 | 2.4 MB | 1,593,344 B + 929,792 B, already stored in OPFS |
| `quran-meta/quran-data.json` | 1 | 16 KB | boot-critical: coordinates, catalog, page ranges |
| Whole `build/` | — | 50 MB | |

Two conclusions fall out of the numbers:

- **Do not precache all HTML.** 39 MB of HTML restates 2.4 MB of database
  sixteen times over, and it all re-downloads on every deploy (the precache is
  version-keyed). The offline unit of truth for *content* is the database +
  `__data.json`; HTML is a delivery format.
- **`__data.json` is the cheap path to full coverage.** The SPA can render any
  route from the fallback shell + route JS (precached) + that route's
  `__data.json`. 8.3 MB raw ≈ 1.5–2.5 MB brotli for the *entire* app.

### 2.3 What is broken or missing today

1. No cache headers and no compression from Caddy (30 KB/page raw on the wire;
   heuristic browser caching of things that must never be stale).
2. No update loop: a new SW installs but **waits until every tab is closed**.
   A reader app lives in long-lived tabs and installed windows — users can sit
   on a stale build indefinitely. No UI exists to tell them.
3. Offline coverage is "pages visited since the last deploy", because visited
   pages are cached into the version-keyed precache.
4. `start_url` (`/`) and `/app` are not precached — the installed app does not
   reliably launch offline.
5. `quran-meta/quran-data.json` is only cached after the first `/app` visit
   (runtime SWR); it is deliberately excluded from the `files` precache but
   never explicitly added back.
6. Old OPFS content-version directories and legacy IDB entries accumulate
   forever.
7. Unknown URLs return Caddy's unstyled 404 (the `404.html` SPA fallback is
   only used by the SW when offline).

---

## 3. Version model

### 3.1 Three version domains — keep them separate

| Domain | Identity | Changes when | Invalidates |
|---|---|---|---|
| **App build** | `kit.version.name` → `_app/version.json` and `$service-worker`'s `version` | every deploy | SW precache (`eq-app-*`), triggers resumable page/data maintenance |
| **Quran content** | `contentVersion` (API `/version` or baked constant) | ~never (corrected artifacts) | OPFS/IDB database entries |
| **Search corpus** | `searchVersion` | search normalization changes | in-worker corpus rebuild |
| **Translation content** *(future)* | per-translation pack version (translations catalog) + the origin's ISR TTL for rendered pages | a translation pack is corrected / catalog changes | that translation's OPFS pack; ISR entries simply expire by TTL |

An app deploy must **not** touch OPFS databases, and a content-version bump
must not require an app deploy (the API manifest already delivers it). This
separation already exists — preserve it. ISR extends the same principle:
**freshness of translated pages is owned by the origin's TTL, not by the app
version** — a deploy neither implies nor requires re-rendering them, and the
SW never treats them as part of the app shell.

### 3.2 The proposed `version.json` — assessment

The proposal on the table: generate a `version.json` from `web/package.json`
at build time, serve it with a no-cache header, fetch it on boot, and if the
version changed, purge caches and refetch.

**The mechanism already exists.** SvelteKit emits `_app/version.json` on every
build, exposes it as `updated` in `$app/state`, polls it on a configurable
interval, and embeds the same string into the service worker bundle as
`version`. We adopt the proposal by configuring that mechanism rather than
building a parallel one:

- `kit.version.name` becomes `"<release tag>+<build ref>"` in deployed images.
  If a non-container build supplies only `BUILD_REF`, `web/package.json`
  supplies the release part; a direct build with neither variable uses
  SvelteKit's default version identity.
- The no-cache header requirement is real and is **currently unmet** (Caddy
  sends no headers at all) — fixed in §6.

**One correction: nothing should purge caches at boot.** The boot-time
"fetch version → if changed, invalidate → refetch" loop is strictly worse than
the SW lifecycle that already ships:

- *Atomicity.* The SW `install` step downloads and verifies the complete new
  precache while the old one keeps serving; a failed fetch rejects the install,
  so the old worker remains active. Activation changes control only after that
  invariant holds. A manual boot-time purge has a window where the old cache is
  gone and the new one is partial — a refresh mid-purge strands the user, and
  offline users who purge on a stale version flag would delete their only
  working copy.
- *No boot penalty.* Boot never blocks on a network check; the version poll is
  a background concern.
- *Free change detection.* A new `version.name` changes the compiled SW bytes;
  the browser's own SW update check (on registration, navigation, and pushes)
  detects it without us diffing anything.

So: **version.json is the *detector* (when to prompt the user), the SW
lifecycle is the *invalidator* (how caches actually swap).** Boot code never
deletes caches.

### 3.3 Concrete configuration

In `web/vite.config.ts`, inside the existing `sveltekit({ … })` options:

```ts
import { readFileSync } from "node:fs";

const pkg = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8"));
const release = process.env.BUILD_VERSION ?? pkg.version;
const buildRef = process.env.BUILD_REF;

sveltekit({
  version: {
    // SvelteKit requires an explicitly supplied name to be deterministic.
    // Local builds without BUILD_REF use SvelteKit's own build-timestamp default.
    ...(buildRef ? { name: `${release}+${buildRef}` } : {}),
    pollInterval: 5 * 60_000, // long-lived reader tabs learn about deploys
  },
  // …existing serviceWorker/adapter config
});
```

`deploy/Dockerfile.web` must declare and export both arguments in the **build
stage, before** `pnpm --filter web build` (Docker arguments are stage-scoped).
They may be redeclared in the final stage for OCI labels:

```dockerfile
FROM node:24-slim AS build
# …
ARG VERSION=dev
ARG REVISION
ENV BUILD_VERSION=${VERSION}
ENV BUILD_REF=${REVISION}
RUN test -n "$BUILD_REF" || { echo "REVISION build-arg is required for container builds" >&2; exit 1; }
RUN pnpm install && pnpm --filter web build

FROM caddy:2-alpine
ARG VERSION=dev
ARG REVISION=unknown
# …OCI labels only; this declaration cannot affect the build stage
```

`docker-compose.yml` passes `REVISION` alongside the existing `VERSION`, and
`deploy/deploy.sh` exports the checked-out commit before building:

```yaml
args:
  VERSION: ${VERSION:-dev}
  REVISION: ${REVISION:?REVISION must be set for a container build}
```

```bash
export VERSION="$LATEST_TAG"
export REVISION="$(git rev-parse --short=12 HEAD)"
```

Notes:

- The suffix is mandatory for container builds. `deploy.sh` supplies both the
  release tag and commit; it does **not** rely on `web/package.json` being
  bumped. Run a manual build as
  `REVISION="$(git rev-parse --short=12 HEAD)" docker compose up --build`.
  If building modified uncommitted sources, supply an explicit unique suffix
  such as `<sha>-local1`; two different outputs must never share a version.
- **Dokploy:** it runs `docker compose up` from its own checkout and does not
  export `REVISION`, so the compose `:?` guard fails that build by design.
  Add `REVISION` (the deployed commit) in the Dokploy application's
  Environment tab, and note this in `deploy/README.md`'s Dokploy section when
  this lands.
- Do not use `Date.now()` or randomness as an explicit `version.name` fallback.
  SvelteKit's [configuration contract](https://svelte.dev/docs/kit/configuration#version)
  requires configured names to be deterministic. Omitting `name` for a direct
  local build deliberately delegates to SvelteKit's default.
- Poll is belt; add braces: call `updated.check()` on
  `visibilitychange → visible` (a tab resumed after days should not wait up to
  5 minutes) and call `registration.update()` at the same moment so the SW
  update check runs too.

---

## 4. Caching architecture (service worker v2)

`web/src/service-worker.ts` keeps its shape (hand-rolled, same registration
path, same push handlers) and changes its cache layout and routing.

### 4.1 Caches

| Cache name | Keyed by version? | Contents | Why |
|---|---|---|---|
| `eq-app-${version}` | yes | `build` + `files` + shell routes: `/`, `/app`, the SPA shell (§4.5) + `/quran-meta/quran-data.json` + `/manifest.webmanifest` | The app shell. Version-keyed so a deploy is an atomic swap; immutable assets never revalidate. |
| `eq-pages-v1` | **no** | HTML of successfully fetched navigations | The "pages I visited stay readable" library. Survives deploys (fixes §2.3-3). |
| `eq-data-v1` | **no** | `__data.json` responses encountered during normal navigation | The visited runtime-data library. Survives deploys and takes precedence over the optional pack. |
| `eq-pack-<hash>` | content-addressed | Tier-2 `__data.json` responses synthesized from one downloaded pack (§5) | Isolates explicit downloads from runtime entries, permits fail-closed staging, and makes disable an exact cache deletion. Only the hash named by `eq-meta-v1` is active. |
| `eq-meta-v1` | **no** | Active pack hash, pack state, page/data recency, maintenance cursor, and per-client app-version acknowledgements | Small SW-readable bookkeeping. Do not use `localStorage` as the source of truth because service workers cannot read it. |

**Install invariant:** all required `eq-app-${version}` entries must be present
before installation succeeds. Replace the current `Promise.allSettled` behavior
with `cache.addAll()` or an equivalent `Promise.all()` that rejects on a failed
fetch, non-2xx response, or `cache.put()`. On failure, delete the partial new
cache and rethrow so the previous worker and cache stay active. This is what
makes the app-cache update atomic; it is also a required fault-injection test.

Previous `eq-app-*` caches and the legacy `eq-precache-*` / `eq-runtime-v1`
caches are retained through the multi-tab handoff (§4.3). After every old client
has moved, migrate missing cached navigation HTML and `__data.json` entries into
the new persistent caches before deleting the legacy names; never overwrite a
newer entry. Persistent page/data caches and the active pack are never deleted
merely because the app version changed.

### 4.2 Request routing

| Request | Strategy |
|---|---|
| non-GET | pass through |
| `/quran/v1/*`, `/_quran/*`, `*.r2.easyquran.fyi`, `/firebase-config.js`, `/translations/*` | pass through (unchanged; the worker layer owns DB caching) |
| `/_app/version.json`, `/service-worker.js`, `/offline/manifest.json`, `/offline/pack.*.json` | network-only/pass through — detectors and the bulk-transfer artifact must never enter Cache Storage as ordinary runtime entries. Fetch the pack with `cache: "no-store"`; its expanded route responses are the persistent copy. |
| `/_app/immutable/*` | cache-first in `eq-app-*` (content-hashed, safe forever) |
| navigations (`request.mode === "navigate"`) | network-first **with a ~3.5 s timeout** → `eq-app-*` shell-route match → `eq-pages-v1` match → the SPA shell (§4.5). Successful responses are written to `eq-pages-v1`. |
| translated reader routes *(future ISR; URL scheme TBD in `quran-translations.md` deferred-8)* | same navigation strategy — network-first into `eq-pages-v1` — but **never precached, never in the tier-2 pack**. The origin's TTL is the freshness authority; the SW copy is only an offline last-known-good. **Hard requirement on the future URL scheme:** translated routes must be distinguished by *path* (e.g. `/app/<surah>/tr/<id>`), not by query string, so the SW can select the ISR strategy without interpreting product query parameters. |
| prerendered `*/__data.json` | stale-while-revalidate in `eq-data-v1`; on a runtime miss, fall back to the active `eq-pack-<hash>`. Normalize only SvelteKit's reserved `x-sveltekit-invalidated` parameter, not the entire query string. Preserve application query parameters and the trailing-slash marker. ISR routes' data responses use their exact request key and follow the network-first rule above, not SWR. |
| other same-origin GET | stale-while-revalidate in `eq-app-*`'s runtime section — practically: icons, favicons, og image; small and safe to re-fetch per version. Never store a response whose `Cache-Control` contains `no-store`. |

Details that make this bulletproof rather than merely plausible:

- **The shell fallback is the offline renderer.** An SPA shell + precached
  route chunks + a cached `__data.json` render any route offline with full
  interactivity. This is what makes precaching 1,311 HTML files unnecessary.
  (§4.5 defines what "the shell" is on each origin.)
- **Navigation timeout.** Today's network-first blocks on lie-fi until the
  browser gives up. 3.5 s then cache keeps repeat visits usable on bad
  connections without making fresh content unreachable (post-activation
  maintenance refreshes the stale copy later).
- **Stale HTML degrades to readable, not broken.** After a deploy, a cached
  page in `eq-pages-v1` references immutable chunks that no longer exist.
  Offline, scripts fail to load — but the page is fully server-rendered
  Arabic text with working plain links. Readable > hydrated. Online, the
  resumable maintenance job (below) replaces it before this matters.

### 4.3 Activation and post-activation maintenance

Service-worker activation is scope-wide, not tab-local: `skipWaiting()` activates
the new worker and [`clients.claim()`](https://developer.mozilla.org/en-US/docs/Web/API/Clients/claim)
transfers every live client in scope. The update flow therefore treats accepting
an update in one tab as accepting it for every open EasyQuran tab (§7):

1. **Activate only a complete app cache.** The install invariant in §4.1 has
   already proven every required entry. Activation calls `clients.claim()` and
   announces the new `version` to every window client. It does **not** delete
   older `eq-app-*` or legacy cache names yet.
2. **Reload every live tab once.** Each client synchronously persists its reader
   position, sets a per-tab reload guard, and reloads on the update-takeover
   message or `controllerchange`. A first-ever SW install does not trigger this
   flow; the worker announces a takeover only when a prior `eq-app-*` or legacy
   `eq-precache-*` cache was present.
3. **Acknowledge the new build.** After boot, each client posts
   `{ type: "APP_READY", version }`. `eq-meta-v1` records acknowledgements by
   `Client.id` and prunes IDs no longer returned by `clients.matchAll()`. Only
   when every live window client reports the current version may the SW migrate
   missing navigation HTML and data entries from legacy cache names (never
   overwrite a newer persistent entry), then delete those names and older
   `eq-app-*` caches. A frozen tab can delay cleanup, costing temporary storage
   but never losing its runnable build.
4. **Run maintenance outside `activate`.** The first ready client asks the SW to
   revalidate `eq-pages-v1` and runtime-only `eq-data-v1` with small concurrency
   (e.g. 6). Store a cursor in `eq-meta-v1` so termination or lost connectivity
   is harmless and the next ready client can resume. Replace only on success and
   keep the stale copy on failure. Never sweep `eq-pack-*` entry-by-entry; §5's
   manifest/hash protocol owns pack freshness.
5. **Trim `eq-pages-v1`** to a sane bound (e.g. 300 entries). Record
   `lastUsedAt` in `eq-meta-v1`; Cache Storage does not provide a portable LRU
   timestamp. `eq-data-v1` remains bounded by the set of prerendered routes.

### 4.4 Database storage hygiene (worker layer, not SW)

After the worker reaches `ready` on `contentVersion` **V**:

- OPFS: delete `easyquran/<anything ≠ V>/` directories.
- IDB fallback store: delete keys not prefixed `V:`.

One-shot, fire-and-forget, logged on failure. This is the only cache the app
deletes outside the SW lifecycle, and only *after* the replacement is proven
to load.

### 4.5 The SPA shell, on both origins

The SW needs one precached HTML document that boots the router for any URL.
Its identity changes with the origin migration; keep it behind a single
constant in the SW so nothing else cares:

- **Static origin (today):** `/404.html` — adapter-static's `fallback` page,
  which already exists.
- **ISR-capable origin (adapter-node):** adapter-node has no `fallback`
  concept, so add a dedicated prerendered shell route (e.g. `/shell`):
  `export const prerender = true; export const ssr = false; export const csr = true;`
  — SvelteKit's documented SPA-shell recipe. It prerenders to a bare document
  that client-renders whatever URL it is served under. Excluded from sitemap,
  `noindex`, and precached in `eq-app-*`.

Migration is a one-line SW change plus the new route; the offline behavior is
identical.

---

## 5. Full offline coverage ("Download for offline")

### 5.1 Tiers

| Tier | Trigger | What works offline | Storage cost |
|---|---|---|---|
| 0 — shell | automatic on first visit | app boots; installed app launches to `/app`; every *visited* page renders; reader infinite-scroll works across the whole Surah (worker reads OPFS DB) | ~2.7 MB + 2.4 MB DB |
| 1 — visited library | automatic while browsing | everything in tier 0 + visited pages readable even without JS | +~36 KB per visited page |
| 2 — full app | explicit user action in Settings | **every** route (114 Surahs, all local pages, 604 global pages, 30 ajzāʼ, marketing pages) navigable and renderable offline | +8.3 MB stored, ~1.5–2.5 MB transfer |

### 5.2 The offline pack

Bulk-fetching 1,308 individual `__data.json` files works but is chatty and
non-atomic. Instead the build emits one artifact:

- A post-build step (in `web/`, after `vite build`) walks `build/**/__data.json`
  and writes `build/offline/pack.<sha256-12>.json`:

  ```jsonc
  {
    "version": 1,
    "entries": { "/app/al-kahf/__data.json": 0, … },  // route → index
    "bodies": [ /* raw __data.json strings */ ]
  }
  ```

- The generator sorts normalized route keys before serialization so identical
  route data produces byte-identical output on every filesystem. Do **not** put
  app/build identity inside this artifact: it would change the content hash on
  every deploy. If useful for diagnostics, put `appVersion` in the unhashed
  manifest instead.
- …plus `build/offline/manifest.json`
  (`{ "pack": "/offline/pack.<hash>.json", "bytes": …, "entries": 1308,
  "appVersion": "0.3.2+a1b2c3d" }`), served `no-cache`. The pack itself is
  content-hashed → served immutable.
- **The hash makes re-downloads nearly free.** `__data.json` bodies change only
  when the Quran data derivation or a route's load shape changes — not on
  ordinary deploys. Same hash → the active `eq-pack-<hash>` is still valid → the
  "re-download on update" step is a no-op manifest check.

Client flow (Settings → "Offline" section):

1. Show current state via `navigator.storage.estimate()` plus the active-pack
   record (`{ hash, entries, savedAt }`) in `eq-meta-v1`. Confirm that the
   referenced cache exists with the recorded entry count; clear a stale pointer
   if eviction made it incomplete. `localStorage` may mirror this for instant UI
   paint, but it is never authoritative and the SW cannot read it.
2. On enable: fetch the manifest network-only → if its hash is already active,
   stop → otherwise fetch the pack with streaming progress and
   `cache: "no-store"` (reuse the `DownloadBar` pattern).
3. Populate an **inactive** `eq-pack-<hash>` cache. For each deterministically
   ordered entry, `cache.put()` a synthesized `Response` with
   `content-type: application/json` under the same normalized key used by the
   request router. Validate the schema, declared entry count, every index/body,
   and the final cache entry count. Any failure deletes this staged cache and
   leaves the previous active pack untouched.
4. Commit with one `eq-meta-v1` write that changes the active hash. The request
   router ignores every pack cache except the referenced hash, so this pointer
   write is the atomic visibility boundary. Then delete the previously active
   pack cache and update the optional UI mirror.
5. On boot after an app update, if a pack is active, re-check the manifest from
   client code (not the SW `activate` event). Refresh through steps 2–4 only
   when the hash changed. Runtime cache maintenance never walks pack entries.
6. Disable by clearing the active pointer first and then deleting exactly its
   `eq-pack-<hash>` cache. `eq-data-v1` and the visited-page library are
   untouched.

The pack deliberately contains **data, not HTML**: offline hard navigations to
never-visited routes land on the SPA shell (§4.5), whose data request misses
`eq-data-v1`, falls back to the active `eq-pack-<hash>`, and client-renders the
route. That asymmetry is fine — it is exactly how a SvelteKit soft navigation
works anyway.

Because the pack step walks `build/**/__data.json`, it inherently contains
only prerendered (Arabic core) routes — future ISR translation routes never
exist in `build/` and are excluded by construction.

### 5.3 Rejected alternative for tier 2 (recorded for later)

Route data could be synthesized *client-side* from what's already offline —
`quran-data.json` (16 KB) has every range/link, OPFS has every ayah — by
migrating the `/app` routes' `+page.server.ts` to universal loads that fall
back to the worker in the browser. Zero extra download, perfect coverage. It
is also a real refactor of the SSG data path (build-time node SQLite vs
browser worker behind one interface) with fragile SSR/browser conditional
imports. The pack ships value now; revisit synthesis if pack maintenance ever
hurts. (§12-e)

---

## 6. HTTP delivery contract (prerequisite for everything above)

The origin **will change** — static Caddy now, an ISR-capable origin when
translated SEO pages land. So the deliverable of this section is a *contract*,
enforced by an assertion script (§11), that every origin must satisfy. Hosts
come and go; the contract and the script stay.

### 6.1 The contract

| Path | Cache-Control | Notes |
|---|---|---|
| `/_app/immutable/*` | `public, max-age=31536000, immutable` | content-hashed |
| `/_app/version.json`, `/offline/manifest.json` | `no-cache` | update detectors; allow storage only for conditional revalidation/cheap 304s, but the SW always passes them through |
| `/offline/pack.*.json` | `public, max-age=31536000, immutable` | content-hashed |
| `/service-worker.js`, `/manifest.webmanifest`, `/quran-meta/*`, HTML pages, `*/__data.json` (prerendered) | `no-cache` | revalidate with ETag; cheap 304s |
| translated (ISR) pages + their `__data.json` *(future)* | `public, max-age=0, s-maxage=<TTL>, stale-while-revalidate=<TTL>` | browsers revalidate every time (`max-age=0`), shared caches serve for the TTL (`s-maxage`); set by the route itself via `setHeaders`. The TTL *is* the "translation lives for some amount of time" rule, expressed in standard HTTP so any proxy/CDN can enforce it |
| `*.md`, `*.txt` | — | `X-Robots-Tag: noindex, follow` (parity with `static/_headers`) |
| everything | compressed | brotli/zstd/gzip; prerendered files via `precompress: true` siblings, dynamic responses via the proxy |

Also required of every origin: unknown URLs return the branded 404 page
(§2.3-7), and correct `ETag`/`304` behavior on the `no-cache` class.

### 6.2 Interim implementation: Caddy (only if the ISR migration is not imminent)

A ~25-line checked-in `deploy/Caddyfile` (replacing the bare
`caddy file-server` CMD) implements the static rows: `encode zstd gzip`,
`file_server { precompressed br gzip }` (+ flip the adapter's
`precompress: false → true`), one `header` matcher per contract row, and
`handle_errors { rewrite * /404.html }`. Compression alone cuts the ~30 KB
pages to single-digit KB on the wire and makes the tier-2 pack and §4.3
maintenance cheap.

If the adapter-node migration is scheduled within the next couple of releases,
skip this and land the contract there directly — do not build the same rules
twice. (Open question §13-6.)

### 6.3 Target implementation: the ISR-capable origin

Constraints it must satisfy, given the existing infra (Docker + external
Traefik, Rust API alongside):

- **Adapter:** `adapter-static → adapter-node`, hybrid rendering. Every
  existing route keeps `prerender = true` (the Arabic core stays a build-time
  artifact — nothing in §2–§5 changes). Translated reader routes are the only
  non-prerendered ones: rendered on demand, with popularity-based exceptions
  (top translations may be prerendered later) — thresholds belong to the
  translations project, not this plan.
- **ISR semantics, self-hosted:** on-demand render + TTL is expressed as the
  `s-maxage`/`stale-while-revalidate` contract row, set per-route with
  `setHeaders`. Enforcement starts as an **in-process LRU render cache** in
  the node origin (URL → rendered response + expiry; modest cardinality:
  115 translations × page space, evicted by size). Because freshness is in
  standard headers, a real shared cache (Souin/Varnish/Cloudflare) can be
  layered in front later *without touching the app*. Do not invent a bespoke
  invalidation protocol; TTL expiry is the design.
- **Header contract placement:** Traefik already routes this stack, so the
  static rows of §6.1 live in Traefik middlewares (path-matched `headers`
  + `compress` middleware for dynamic responses) — adapter-node's own static
  serving handles `_app/immutable` correctly, and ISR rows come from the
  routes themselves. The assertion script is the referee, wherever a rule
  physically lives.
- **SW interplay (already fixed above):** ISR routes are ordinary navigations
  to the SW (§4.2) — cached as last-known-good in `eq-pages-v1`, never
  precached, never packed, TTL owned by the origin. The SPA shell switches to
  the dedicated `/shell` route (§4.5). No other SW change.

### 6.4 Other hosts

If the site also deploys to Cloudflare Pages/Netlify, mirror the static
contract rows in `web/static/_headers` (the robots rules there today are the
template). Rule of thumb anywhere: **`_app/immutable/*` and hashed packs are
immutable-forever; `version.json`, the pack manifest, and ordinary static
responses are no-cache; ISR rows use `s-maxage`.**

### 6.5 Registration hardening

- `navigator.serviceWorker.register(SW_URL, { …, updateViaCache: "none" })` —
  belt-and-braces against any intermediary caching the SW script.
- Drop the stale `sw.js` entry from the `serviceWorker.files` exclusion list
  (no such file exists).

---

## 7. Update UX

### 7.1 Flow

1. `updated.current` is an **early detector**, not proof that a worker is ready.
   When it flips true (poll / visibility check), call and await
   `registration.update()`. Follow `updatefound` and the installing worker's
   `statechange` until `registration.waiting` exists. If a waiting worker was
   already present, skip directly to step 2.
2. Only after a worker is waiting, show one generic app toast: *"A new version
   is ready · Reload open tabs"*. Extend the toast UI with an explicit action
   API or add a small update-toast store; do not put synthetic FCM
   `MessagePayload` values into the existing push-notification store. If SW
   registration is unavailable but `updated.current` is true, offer a plain
   `location.reload()` action instead.
3. On accept, use a `BroadcastChannel` plus a message from the waiting worker to
   send update preparation to every window client. Each tab synchronously
   persists reader position and arms a once-only reload guard. Then post
   `{ type: "SKIP_WAITING" }` to the confirmed `registration.waiting` worker.
4. The new worker activates, calls `clients.claim()`, and announces the version.
   Every armed tab reloads once on the takeover message or `controllerchange`,
   then posts `{ type: "APP_READY", version }` after boot (§4.3). Activation is
   global, so keeping other tabs on the old build is not a supported state.
5. Before the user accepts, retain the documented SvelteKit hard-navigation
   guard so an attempted client navigation cannot mix old route metadata with
   a newly deployed origin:

   ```ts
   beforeNavigate(({ willUnload, to }) => {
     if (updated.current && !willUnload && to?.url) location.href = to.url.href;
   });
   ```

6. The SW adds the matching message listeners (`skipWaiting()` only on the
   explicit request, update-takeover broadcast, and `APP_READY` bookkeeping).
   No auto-`skipWaiting` on install. Until acceptance, the old worker and cache
   continue serving every old tab; after acceptance, every tab reloads onto the
   already-complete new cache.

### 7.2 Reading position survives

`SurahReader` already persists position to `sessionStorage`
(`easyquran.reader-position`) and restores it on reload-type navigations. Add a
synchronous handler for the update-preparation message (or expose the existing
`writeHistoryState()` through a small app event) so every tab writes its latest
anchor before the coordinator reloads it. Test two reader tabs at different
positions; both must return to their own Surah page and anchor.

---

## 8. Manifest & install polish

- Add `"id": "/"` (stable identity regardless of future `start_url` tweaks).
- Decide `start_url`: recommend `"/app"` — an installed EasyQuran is a reader,
  not a marketing site. (Open question §13-1.)
- Add `shortcuts`: `[{ "name": "Continue reading", "url": "/app" }, { "name": "Juz index", "url": "/app/juz" }]`
  (long-press / jump-list entries).
- Optional, low priority: `screenshots` (richer install sheet on Android),
  `categories: ["books", "education"]`.
- Keep the push/notification handlers in the single SW — FCM already binds to
  this registration (`lib/firebase/messaging.ts`); do not introduce a second
  `firebase-messaging-sw.js`.
- Add a lightweight offline indicator in the app header (`navigator.onLine` +
  `online`/`offline` events) and audit copy: the reader's existing "More ayahs
  are unavailable right now" should distinguish *offline* from *error* when
  `navigator.onLine === false`.

---

## 9. Failure modes & edge cases

| Scenario | Behavior under this plan |
|---|---|
| First visit ever, offline | Nothing we can do (no SW yet) — browser error. |
| Installed app launched offline | `/app` served from `eq-app-*`; DB from OPFS; worker `resolveManifest` times out in 3 s → baked manifest → reader boots. |
| Offline hard-nav to never-visited route, tier 2 on | SPA shell (§4.5) → route chunks from precache → runtime-data miss → active `eq-pack-<hash>` fallback → full render. |
| Offline, tier 2 off, route never visited | Shell renders, data fetch fails → route-level error UI with the offline indicator explaining why. Acceptable; tier 2 is the fix. |
| Deploy while N tabs open | New SW installs completely and waits; every tab gets the toast. Acceptance in any tab persists all reader positions, activates globally, and reloads every live tab once. Old app caches remain until every live client acknowledges the new version. |
| Deploy removes old immutable chunks, old tab lazy-imports one | Before acceptance, the old worker/cache remains intact. After acceptance there is no intentionally old tab: every live client reloads, acknowledges the new build, and only then permits old app-cache cleanup. A frozen/unacknowledged tab delays cleanup rather than losing chunks. |
| Required precache request fails during install | Installation rejects and deletes its partial `eq-app-*`; the old worker/cache remains active and no update-ready toast is shown. |
| `__data.json` shape changes in a deploy | Resumable maintenance (§4.3) refreshes runtime entries when online, while a changed pack hash stages and swaps the full tier-2 dataset (§5.2). Residual risk: offline across a shape-breaking deploy → shell render fails for stale entries → error UI. Rare and self-heals online. |
| Translated (ISR) route, offline *(future)* | Visited: served stale from `eq-pages-v1`, offline indicator showing; TTL staleness offline is accepted — the alternative is nothing. Unvisited: shell + error UI pointing at the translation pack flow (`quran-translations.md`), which is the real offline path for translation text. |
| ISR entry expires at the origin | Purely an origin/proxy concern: next request re-renders. The SW neither knows nor cares — it never caches by TTL, only replaces on successful fetch. |
| Lie-fi (connected, 0 throughput) | 3.5 s navigation timeout → cached page/shell. |
| Storage eviction (quota pressure, Safari) | `persist()` already requested (grants are near-automatic for installed PWAs). Everything is re-downloadable; sha256 verification catches truncated DBs; worst case is a re-download, never corruption. |
| Safari/iOS specifics | SW + manifest install supported (16.4+ for push). OPFS `createWritable` missing on older Safari → existing IDB fallback already covers it. Test matrix in §11. |
| Private browsing | OPFS/IDB may be unavailable → existing in-memory (`session`) fallback path already handles it; SW may be denied → app works as a plain site. |
| SW registration fails / unsupported | Current behavior preserved: fully functional online site, no offline layer. |
| Push received while SW updated | Handlers live in the SW file itself; subscription rides the registration across updates — no action needed. |

---

## 10. Rollout phases

Each phase ships independently and leaves the site strictly better. The
adapter-node/ISR migration is **its own project** (owned by the translations
work); when it lands, this plan changes only in §6's implementation column
and the one-line shell swap of §4.5 — phases 1–4 are origin-agnostic by
construction.

**Phase 0 — delivery contract (no app code).**
Write `web/scripts/assert-headers.sh` from the §6.1 table, then implement the
static rows on the current origin (interim Caddyfile, §6.2) **or**, if the
node migration is imminent, defer implementation to it and keep only the
script + `precompress: true` + `updateViaCache: "none"`.
*Accept:* assertion script passes against the serving origin; pages arrive
brotli-compressed; `_app/immutable/*` shows `immutable`; `version.json` shows
`no-cache`; unknown URL returns branded 404.

**Phase 1 — version identity & update loop.**
deterministic `kit.version { name, pollInterval }` from `BUILD_VERSION` +
build-stage `BUILD_REF`; detector → `registration.update()` → waiting-worker
state machine; generic action toast; all-tab preparation/`SKIP_WAITING`/
`APP_READY` protocol; `beforeNavigate` hard-nav guard; visibility-triggered
`updated.check()` + `registration.update()`. The SW side lands against the
**legacy cache layout**: swap the precache fill to fail-closed and implement
§4.3 steps 1–3 (takeover, reload, acknowledgements), introducing `eq-meta-v1`
early for the acknowledgement records; the rest of the §4.1 cache layout is
Phase 2.
*Accept:* deploy a trivial change with the old tab open → toast within
5 min (or instantly on tab refocus) but only after the worker is waiting →
accept in one of two reader tabs → both reload exactly once, return to their own
reader positions, and report the same new app version; the superseded app cache
(still named `eq-precache-*` in this phase) is deleted only after both
acknowledgements. A forced precache failure leaves the old build active and
shows no ready toast.

**Phase 2 — cache architecture v2.**
Versioned app cache plus persistent page/data/meta caches; fail-closed shell-route
precache (`/`, `/app`, the SPA shell of §4.5, `quran-data.json`, manifest);
navigation timeout; reserved-parameter-only data-key normalization; resumable
post-activation maintenance + metadata-backed trim; OPFS/IDB old-version cleanup.
*Accept:* DevTools-offline: installed app launches to `/app`; a visited Surah
renders; an *unvisited* Surah soft-navigates successfully after its data was
cached by a prior session; after a deploy, previously visited pages still
render offline (survived the version bump); OPFS contains exactly one
content-version directory.

**Phase 3 — full offline.**
Pack build step + manifest, Settings UI with progress + storage estimate,
staged `eq-pack-<hash>` cache + atomic active-pointer commit, boot-time manifest
check, and exact disable path.
*Accept:* enable on wifi (< 30 s), kill network, cold-launch installed app,
navigate via search/juz/global-page routes to Surahs never visited — all
render; deploy without data changes → one manifest revalidation, zero individual
pack-entry requests, and no pack re-download (hash unchanged). Inject a quota
failure halfway through staging → the previous pack remains fully active and
the partial candidate is deleted. Disable → visited runtime data still works.

**Phase 4 — polish.**
Manifest `id`/`start_url`/shortcuts, offline indicator + copy audit, iOS
manual pass, Lighthouse installability ≥ green, update this doc's status line.

---

## 11. Testing

- **Header assertions** — `web/scripts/assert-headers.sh` (curl -sI against a
  locally built `docker compose up web`) checking the §6.1 matrix; run in the
  deploy checklist. This is the regression net for the whole update system.
- **Unit (vitest)** — pure pieces: cache-name/version derivation, request →
  strategy routing decision function (extract it as a pure function for this),
  reserved-parameter data-key normalization, deterministic pack serialization,
  pack index encode/decode, active-pointer transitions, acknowledgement cleanup,
  and metadata-backed trim ordering.
- **SW lifecycle integration** — simulate one required precache failure and
  assert the install rejects without deleting the old cache; simulate two
  clients through waiting → takeover → `APP_READY` and assert old app caches are
  retained until both acknowledge; terminate/resume a maintenance sweep from
  its stored cursor.
- **Pack fault injection** — fail schema validation, an intermediate
  `cache.put()`, and the final count check. In every case the active pointer and
  previous pack remain unchanged and the staged cache is removed.
- **Manual matrix (per release of phases 2–3)** — Chrome desktop, Chrome
  Android (installed), Safari iOS (installed): the §10 acceptance lines plus
  DevTools → Application → "Offline" walkthrough. Keep the checklist at the
  bottom of this doc as phases land.
- **Local SW loop** — `just web-build && cd web && pnpm preview` exercises the
  real SW; two direct local builds exercise SvelteKit's default timestamp
  version. Container-loop tests pass explicit, distinct `REVISION` values when
  the built output changes.

---

## 12. Decision log

- **(a) Reuse SvelteKit's `_app/version.json` instead of a custom
  `version.json`.** Same file the framework already generates, polls, and
  embeds in the SW. Production combines the deploy's release tag and commit
  into a deterministic `kit.version.name`; direct local builds may use
  SvelteKit's default. The original intent (human-meaningful release id,
  no-cache delivery, boot/interval checks) is preserved with zero parallel
  machinery.
- **(b) SW lifecycle as the only cache invalidator; no boot-time purge.**
  Atomic swap vs. a hand-rolled delete-then-refill window; see §3.2.
- **(c) App-shell + data offline model; no full-HTML precache.** 40 MB
  (re-downloaded every deploy) vs ~11 MB total that survives deploys. HTML
  stays a runtime-cached nicety for no-JS readability.
- **(d) Keep the hand-rolled SW; no Workbox.** The existing worker is ~150
  lines from the target architecture; Workbox would add a dependency, its own
  update semantics, and no capability we lack.
- **(e) Content-hashed offline pack now; client-side data synthesis later
  (maybe).** §5.3.
- **(f) Persistent (unversioned) page/data caches with resumable post-activate
  maintenance** — chosen over version-keyed runtime caches, which
  silently delete the user's offline library at every deploy (today's
  behavior).
- **(g) HTTP rules are a host-agnostic contract, not server config.** The
  origin is scheduled to change (static Caddy → ISR-capable node origin for
  on-demand translated pages); encoding the rules as a table + assertion
  script means the migration re-implements, never re-designs. Caddy work is
  explicitly throwaway-priced (§6.2) and skippable.
- **(h) ISR pages live outside the app-version lifecycle.** Their freshness is
  the origin's TTL (`s-maxage`/`stale-while-revalidate` set per route); the SW
  treats them as ordinary network-first navigations — never precached, never
  packed, cached only as an offline last-known-good. Offline translation
  *text* remains the pack/OPFS design in `quran-translations.md`; this plan
  does not duplicate it in Cache Storage.
- **(i) An accepted SW update reloads every live tab.** `skipWaiting()` plus
  `clients.claim()` is scope-wide. Pretending activation is tab-local would
  leave old JavaScript executing against a new cache. All tabs persist position,
  reload once, and acknowledge the new version; old app caches remain until the
  acknowledgement set covers every live client.
- **(j) Tier-2 data has its own content-addressed cache.** Runtime data and an
  explicit full download have different freshness, rollback, and deletion
  semantics. An inactive `eq-pack-<hash>` is fully populated and verified, then
  one SW-readable metadata pointer makes it active. Runtime sweeps and disable
  can no longer corrupt each other's guarantees.

---

## 13. Open questions

1. `start_url`: `/app` (recommended) or `/`? Affects store listings and what
   "installed" means.
2. `pollInterval` 5 min vs 15 min — cost is one 27-byte 304 per tab per
   interval; 5 min recommended for faster fleet convergence.
3. Should tier 2 also pack the ~15 marketing/app-index HTML pages for no-JS
   offline parity? (Cheap: +~300 KB. Default: no.)
4. `eq-pages-v1` trim bound (proposed 300 entries ≈ ~9 MB worst case).
5. Auto-enable tier 2 for installed apps on wifi without asking? (Default: no
   — keep it an explicit, labeled download.)
6. Timing of the adapter-node/ISR migration relative to phase 0: if it lands
   within a couple of releases, skip the interim Caddyfile entirely (§6.2).
7. ISR TTL values and the popularity threshold for prerendering top
   translations — owned by the translations project; this plan only requires
   they be expressed as `s-maxage`/`stale-while-revalidate` and that the URL
   scheme be path-distinguished (§4.2 hard requirement).
8. ISR render-cache backend: in-process LRU (recommended start) vs Redis vs a
   caching proxy (Souin/Varnish) — revisit only if hit rates or multi-replica
   deployment demand it.
