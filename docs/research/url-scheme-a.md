# URL scheme A — drop both route prefixes (`/en` and `/app`)

Status: implemented (M1–M3 shipped; see Part 5 divergence #10 in docs/quran-system.md). Scope: navigation/addressing only — no rendering, typography, data, or
translation-model changes. Companion to
[`navigation-audit.md`](./navigation-audit.md) (status: implemented) whose shapes this
scheme replaces. Owner decision: exactly two UI locales exist — `en` (default,
**unprefixed**) and `ar` (**`/ar/`-prefixed**). The `/{ui}/app` double prefix dies.

Every claim below cites code read in this session (paths + lines). Nothing here touches
the Quran DBs or adds any hash over Quran data.

---

## 1. Goal grammar

| Family | Today (verified) | Target |
| --- | --- | --- |
| Surah (en) | `/en/app/al-baqarah` | `/al-baqarah` |
| Surah translated | `/en/app/ar-rum/t/ms/basmeih` | `/ar-rum/t/ms/basmeih` |
| Global page | `/en/app/page/13` | `/page/13` |
| Juz / hizb / rub | `/en/app/juz/1`, `/en/app/hizb/44`, `/en/app/rub/174` | `/juz/1`, `/hizb/44`, `/rub/174` |
| Translated ranges | `/en/app/t/{lang}/{translator}/page/13` | `/t/{lang}/{translator}/page/13` |
| Indexes | `/en/app/surah`, `/en/app/juz`, `/en/app/pages` | `/surah`, `/juz`, `/pages` |
| Product pages | `/app/search`, `/app/settings`, `/app/bookmarks`, `/app/yours` | `/search`, `/settings`, `/bookmarks`, `/yours` |
| Arabic UI | `/ar/app/al-baqarah`, `/ar/app/page/13` | `/ar/al-baqarah`, `/ar/page/13` |
| Reader home hub | `/en/app` (noindex hub) | **removed** — `308 /app → /surah`; resume card already lives at `/yours` (see §4) |
| Scroll position | `?v={surah}:{ayah}` (D2, shipped) | unchanged — stays in `?v=`, never the path |
| Numeric alias | `/en/app/2 → 308 /en/app/al-baqarah` (D14, `hooks.server.ts:201-214`) | `/2 → 308 /al-baqarah`, `/ar/2 → 308 /ar/al-baqarah` |

Content addressing (surah/range/translation segments) is untouched — only the UI-locale
prefix and the `/app` marker segment change. The `surah*For(ctx, …)` helper family
remains the only sanctioned way to build reader hrefs (AGENTS.md invariant).

---

## 2. Route tree move

`web/src/routes/(application)/app/**` → `web/src/routes/(application)/**` (groups add no
path — `(marketing)` already proves the pattern: `/(marketing)/about` serves `/about`).
The group's three layout files move with it and become the group root:

| Today (verified by `find web/src/routes -type d`) | After |
| --- | --- |
| `(application)/app/+layout.{ts,svelte,server.ts}` | `(application)/+layout.{ts,svelte,server.ts}` |
| `(application)/app/+page.{server.ts,svelte}` (the noindex hub) | **deleted** (§4) |
| `(application)/app/[surah]/` | `(application)/[surah]/` |
| `(application)/app/[surah].md/` | `(application)/[surah=surahSlug].md/` (§3) |
| `(application)/app/[surah]/t/[lang]/[translator]/` (+ `.md`) | `(application)/[surah]/t/[lang]/[translator]/` (+ `.md`) |
| `(application)/app/page/[n]/` (+ `.md`) | `(application)/page/[n]/` (+ `.md`) |
| `(application)/app/pages/` | `(application)/pages/` |
| `(application)/app/juz/`, `juz/[n]/` (+ `.md`) | `(application)/juz/`, `juz/[n]/` (+ `.md`) |
| `(application)/app/hizb/`, `hizb/[n]/` (+ `.md`) | `(application)/hizb/`, `hizb/[n]/` (+ `.md`) |
| `(application)/app/rub/`, `rub/[n]/` (+ `.md`) | `(application)/rub/`, `rub/[n]/` (+ `.md`) |
| `(application)/app/t/[lang]/[translator]/{page,juz,hizb,rub}/[n]/` | `(application)/t/[lang]/[translator]/{page,juz,hizb,rub}/[n]/` |
| `(application)/app/surah/` (index) | `(application)/surah/` |
| `(application)/app/search/` (+ `_components`, `__tests__`) | `(application)/search/` |
| `(application)/app/settings/` (+ `_components`, `__tests__`) | `(application)/settings/` |
| `(application)/app/bookmarks/` (+ `_components`) | `(application)/bookmarks/` |
| `(application)/app/yours/` | `(application)/yours/` |
| `(application)/app/_reader/` (+ `__tests__`, `translation-nav.ts`) | `(application)/_reader/` — stays non-routable for the concrete reason that it contains **no `+`-prefixed files** (verified: 0). SvelteKit has no underscore rule: kit 2.70.2's `create_manifest_data` only makes route nodes from `+`-prefixed files, and `parse_route_id` never filters `_` — proof by precedent, `web/src/routes/_quran/[...artifact]/+server.ts` is a live endpoint served at `/_quran/...`. The move keeps the `_` convention; the §2.1 collision proof must not (and does not) lean on privacy-by-underscore. |
| `(application)/app/__tests__/` | `(application)/__tests__/` |

`git mv` preserves history; route ids change shape (`/(application)/app/settings` →
`/settings`), which is load-bearing for `(application)/app/+layout.svelte:36-39` (the
`endsWith("/app/…")` route-id checks) and for `parseReaderRoute`
(`web/src/lib/server/reader-route.ts:212-268`, marker `"/app"` at `:217`).

### 2.1 Reserved top-level names — collision proof

Reader segments introduce these reserved single-segment names at the site root:
`page`, `pages`, `juz`, `hizb`, `rub`, `t`, `surah`, `search`, `settings`, `bookmarks`,
`yours` — plus the dynamic `[surah]` slug route (letter-initial
`SURAH_SEGMENT` = `[a-z][a-z0-9]*(?:-[a-z0-9]+)*`, `reader-route.ts:33`; digits are
caught by the numeric-alias redirect first, §6).

Existing top-level static routes (from the directory listing run in this session):
marketing `about`, `contact`, `faq`, `privacy`, `terms`; auth-group `login`,
`register`, `forgot-password`, `verify-email`; `account` (`(account)/account`);
oauth `auth`; `health`, `design`, `sitemap.xml`, `llms.txt`, `llms-full.txt`,
`firebase-config.js`, `_quran` (plus `index.md`, `about.md`, … if §3's fallback split
is taken).

**No reader name collides with any marketing/auth/account/health/design name**: the
reader set is {page, pages, juz, hizb, rub, t, surah, search, settings, bookmarks,
yours}; the existing set is {about, contact, faq, privacy, terms, login, register,
forgot-password, verify-email, account, auth, health, design}. Disjoint by direct
comparison. Static segments outrank the dynamic `[surah]` in SvelteKit's route sort, so
even a hypothetical overlap would resolve statically. `[surah]` swallowing unknown
top-level segments 404s through `requireSurah` — same UX as today's `/app/typo`.

**Future rule (goes in docs):** top-level product segments must never be numeric, and
`/ar/**` is reserved for the Arabic UI (`en` likewise stays reserved by the 308 map).
The existing `RESERVED_SURAH_SEGMENTS` = `{juz, page, hizb, rub, t, surah, pages,
yours}` (`web/src/lib/i18n/reader.ts:15`) keeps its members but their meaning scales
up: they are now **site-root** reserved words, not `/app`-scoped — which is exactly
what keeps `/t`, `/page`, … from ever being swallowed as `[surah]` inputs.
`isSurahSegment` (`reader.ts:72-74`) unchanged.

---

## 3. `[slug].md` / `[slug].txt` vs reader text twins

**Problem.** Today the marketing text variants live at the root:
`web/src/routes/[slug].md/+server.ts` and `[slug].txt/+server.ts`, both
`prerender = true`, `entries = textVariantEntries` (`web/src/lib/seo/render.ts:125-128`
→ slugs `index, about, faq, contact, privacy, terms`). The reader text twins live under
`app/`: `[surah].md`, `page/[n].md`, `juz/[n].md`, `hizb/[n].md`, `rub/[n].md`,
`[surah]/t/[lang]/[translator].md`, `t/…/{page,juz,hizb,rub}/[n].md`. The `/app`
segment is the only thing keeping the two dynamic families from overlapping. After the
move, `(application)/[surah].md` and root `[slug].md` are **two dynamic single-segment
`.md` routes at the same depth** — `/al-baqarah.md` matches both patterns, and which
one wins is SvelteKit's lexicographic tie-break on the param name (`slug` < `surah`).
Relying on that is exactly the kind of silent breakage this plan must not ship.

**Multi-segment reader twins never conflict** (`page/13.md` is two segments; marketing
variants are all single-segment) — only the surah twin needs resolution. Reader has no
`.txt` twins, so `[slug].txt` is untouched.

**Decision: explicit split via param matchers.** Both routes keep their current
handler and `entries` verbatim; only the directory/param names and two new matcher
files change:

- `web/src/routes/[slug=marketingText].md/+server.ts` — the current marketing handler,
  unchanged (`fetch(pagePath(params.slug))` → turndown). Matcher
  `web/src/params/marketingText.ts`: `param` ∈ `{index, about, faq, contact, privacy,
  terms}` (derived from `MARKETING_PAGES`, `lib/config/site-structure.ts:20-27`).
- `web/src/routes/(application)/[surah=surahSlug].md/+server.ts` — the current reader
  handler, unchanged (`requireSurah` + `readSurahLocalPageData` +
  `renderSurahPageMarkdown`, entries over the 114 baked slugs). Matcher
  `web/src/params/surahSlug.ts`: `QURAN_DATA.surahBySlug(param) !== undefined`.

SvelteKit matchers are runtime filters: a route whose matcher fails is excluded and
resolution continues, so the two families are disjoint **by construction** — no
tie-break dependence. Verified-good prerequisites (review pass): the installed kit
has no `svelte.config.js` — SvelteKit options live in the `sveltekit()` plugin in
`web/vite.config.ts:91-116` — and param matchers need no config key
(`web/src/params/*.ts` just works); same-depth dynamic routes are legal; static
dotted dirs (`sitemap.xml` precedent) outrank dynamic ones. Each route's prerender
`entries` stays exactly the set its
matcher admits, so the built output is identical to today's (6 marketing `.md` + 114
surah `.md`, plus the multi-segment reader twins).

*Fallback if SvelteKit's route sorter rejects the same-depth dynamic pair at compile
time:* convert the six marketing variants to static dirs (`index.md/`, `about.md/`,
`faq.md/`, `contact.md/`, `privacy.md/`, `terms.md/`, each a 10-line `+server.ts`
delegating to a shared `marketingTextVariant(kind, id)` helper extracted from the
current handler). Static > dynamic is guaranteed specificity; `[surah].md` then has no
competitor. Dotted dirs are already proven in this repo (`sitemap.xml/+server.ts`).

`.txt`: `[slug].txt` stays marketing-only. Reader slugs get **404**, not an implicit
marketing fetch — add an explicit guard in its GET (`params.slug` must be a marketing
id) so `/al-baqarah.txt` never internally fetches the surah HTML and mint-plain-texts
it by accident.

The md sibling Link header machinery is path-shaped and needs no per-route knowledge:
`mdSiblingPathFor` (`web/src/lib/accept-parse.ts:129-131`) serves both families from
the same map/regex tables (updated in §5.4), and hooks' internal `event.fetch(mdPath)`
follows whatever route now owns the `.md` path.

---

## 4. The `/app` hub (noindex reader home)

Verified today: `(application)/app/+page.svelte` renders `Seo … noindex`
(`+page.svelte:107`), the resume card (`copy.index.yoursContinue`, `:118-152`), and the
metric strip linking the four indexes (`:78-83`). **The same resume card already lives
at `/app/yours`** (`yours/+page.svelte:161` renders `copy.index.yoursContinue`, `:187`
the `resumeToLastRead` button) — "resume card lives where it does today" is satisfied
by `/yours`, which keeps its own copy.

Decision (owner default):

- Delete `(application)/app/+page.{server.ts,svelte}` (and with it the
  `readerPrerenderHrefs` server load — its surviving consumer moves, §7.3).
- Hooks 308: `/app` → `/surah` and `/ar/app` → `/ar/surah` (rule table §6).
- `readerEntryPath(_, "home")` / `readerHomeHrefFor` (`seo.ts:72-74`, `reader.ts:159-163`)
  die; the `home` member drops from `ReaderEntryHrefFor`
  (`reader-prerender.server.ts:42-45`) and from the prerender fan-out
  (`reader-prerender.server.ts:100-106`). Prerender count: −2 pages (en+ar home); every
  other count unchanged. The sitemap already excludes home (`sitemap.xml/+server.ts:65`
  comment) — no loc change.
- `ReaderPrerenderLinks` (crawler discovery anchors) rendered **only** from the hub
  today (grep: single consumer `app/+page.svelte:108`). Its render + data load move to
  the prerendered `/surah` index page (`(application)/surah/+page.{server,svelte}`) so
  build-time route discovery keeps a seeding page. Discovery href set = same
  `readerPrerenderHrefs()` output minus the two home hrefs.
- Callers of the hub href re-point to `/surah`. **Complete inventory** (cross-checked
  with `git grep -l '/app' -- src '*.config.*' scripts`, run this session — 100 files;
  code hits that are NOT nav-guard-visible are called out):
  - `marketingReaderHomeHref` (`lib/i18n/marketing-copy.ts:207-209`, typed
    `"/en/app" | "/ar/app"`) → returns `"/surah"` / `"/ar/surah"` via
    `readerHrefFor`; `chrome-copy.ts:75`; `(marketing)/+page.svelte:412`.
  - Post-auth flow: `lib/auth/post-auth-path.ts:7-8` (`"/app"` → `"/surah"`);
    `lib/auth/route-guard.ts:11`; **`lib/components/auth/AuthModal.svelte:35`**
    (`goto(... : "/app")`); **`(auth)/login/+page.svelte:17`,
    `(auth)/register/+page.svelte:12`** (same ternary);
    **`(auth)/verify-email/+page.svelte:35,52,61`** (three `goto("/app")`);
    **`auth/[provider]/failure/+page.svelte:28`** (`href="/app"`);
    **`auth/[provider]/success/success-destination.ts:10`** (fallback `"/app"` —
    outside every nav-guard glob, needs this explicit entry).
  - **`routes/design/_variants/copy.ts:7`** (`href: "/app"` — a `.ts` file under
    `routes/`, outside all three nav-guard `import.meta.glob` roots,
    `nav-guard.test.ts:26-42` — explicit entry required).
  - **`routes/(application)/app/search/_components/TranslationPicker.svelte:12`**
    (`SETTINGS_PATH = "/app/settings"`; consumed at `:98` via `publicHref` — the
    constant line carries no `NAV_SIGNAL` token, so the guard cannot see it).
  - Search palette `site-routes.ts:26` (`href: "/app"` → `"/surah"`),
    `search-routes.ts:8`, `settings-routes.ts:8`; `Nav.svelte:138-140`.
  - **`(application)/app/yours/+page.svelte:32-36`** index links
    (`readerHrefFor(locale, "/app/surah" | "/app/juz" | "/app/pages")` — builder-input
    literals; these flow through `readerHrefFor` so they are semantically safe but the
    inputs must drop `/app`).
  - **Type literals re-typed**: `RangeReader.svelte:152`
    (`let quranHref: \`/app/${string}\``) and `TranslationModal.svelte:308`
    (`rowHref(...): \`/app/${string}\` | null`) → `` `/${string}` `` — matching the
    `quran.ts` return-type change; they must not keep a dead type shape.
  - `(application)/app/+layout.svelte:52` chrome fallback (`"/app"` → `"/surah"`).
  - **Scripts (plan was silent — blocker):**
    - `scripts/assert-headers.sh:48-99` hard-asserts today's scheme (`/en/app` is
      "the real prerendered page", `/app` a 92-byte stub, comment "/app/** 307s to
      /{en,ar}/app/**", probes `/app/al-kahf`, `/en/app/al-fatihah/t/en/sahih`,
      `/en/app/al-kahf/__data.json`). Rework in M2 to scheme A: probe `/surah` (real
      prerendered page), `/al-kahf`, `/ar/al-kahf`, `/ar/al-fatihah/t/en/sahih`,
      `/ar/al-kahf/__data.json`, and add 308 assertions for `/app`, `/en/app`,
      `/en/app/al-fatihah` with the rule-table Locations.
    - `scripts/gen-offline-pack.ts:62` throws when the build contains
      `/app/__data.json`-prefixed keys ("legacy unprefixed reader artifact leaked").
      Post-M2 the unprefixed shapes ARE the live shapes, so the guard inverts: the
      legacy set becomes `/en/app/**` + `/ar/app/**` (+ the dead `/app` hub key);
      unprefixed `/page/13/__data.json`-class keys are expected. Default (no owner
      input needed): flip the filter to the `/en|/ar/app` family; unprefixed keys pass.

---

## 5. Locale flip mechanics

### 5.1 paraglide

**Mechanism (verified against the generated runtime, blocking the earlier draft's
claim):** with custom `urlPatterns` configured, the generated runtime sets
`TREE_SHAKE_DEFAULT_URL_PATTERN_USED = false` (`web/src/lib/paraglide/runtime.js:149`)
and `localizeUrl`/`extractLocaleFromUrl` iterate ONLY the configured patterns — on no
match the URL is returned **unchanged** (`runtime.js:1009,1023,1038`) and
`extractLocaleFromUrl` yields `undefined` so strategies fall through to `baseLocale`
(`runtime.js:899-915`). The repo itself proves the shape: non-home marketing pages are
en-only published precisely because `localizeHref` cannot prefix unlisted paths
(`web/src/lib/i18n/marketing.ts:11-19` `ENGLISH_ONLY` for about/faq/contact/privacy/terms;
`marketingHref → localizeHref` `:39-46`), while home is ar-published because the `/`
pattern covers it. So "keep only the `/` pattern and rely on default prefixing" would
leave every `/ar/**` reader page resolving en and `localizeReaderHref`
(`web/src/lib/i18n/reader.ts:149-154`) throwing at build/render. Not viable.

**Decision: delete `urlPatterns` entirely.** With no custom patterns the generated
runtime flips to the default `/:locale/...` semantics, which is byte-for-byte the
wanted grammar (verified in the generated file):
- `localizeUrlDefaultPattern` (`runtime.js:1018-1036`): base locale (`en`) → path
  returned **prefix-less**; any other locale → `/{locale}` + path. So
  `localizeHref("/al-baqarah","en") = "/al-baqarah"`,
  `localizeHref("/al-baqarah","ar") = "/ar/al-baqarah"`.
- `deLocalizeUrlDefaultPattern` (`runtime.js:1111-1121`): strips the first path
  segment iff it is a locale — `/ar/al-baqarah → /al-baqarah`, `/al-baqarah` identity.
- The `ar`-as-first-segment ambiguity is nil: no baked surah slug, range segment, or
  reserved word equals `en`/`ar`, and `/ar` marketing home is a locale URL by design.

`web/paraglide.config.js` loses the whole `urlPatterns` array; `strategy:
["url", "baseLocale"]` stays. `getReaderUiCopy()` (`lib/i18n/reader-copy.ts:743`)
resolves off the URL via `getLocale()` and follows automatically.
*Alternative kept in pocket:* a single catch-all
`{ pattern: "/:path(.*)", localized: [["en", "/:path(.*)"], ["ar", "/ar/:path(.*)"]]
}` mirrors today's `/app/:path(.*)` pattern if the default-pattern path ever misbehaves
— same semantics, explicit config. **Risk pin (test):** round-trip fixtures asserting
`localizeHref`/`deLocalizeUrl` over `{/, /ar, /al-baqarah, /t/en/sahih/page/7,
/search, /ar/page/13}` so the regen (`TREE_SHAKE_DEFAULT_URL_PATTERN_USED → true`) is
machine-verified, not assumed.

### 5.2 `src/hooks.ts` (reroute)

`READER_ROUTE_PATTERNS` (`hooks.ts:13-27`) drop the `/app` prefix from all ten
patterns; `localizedReaderTuple` (`hooks.ts:39-42`) becomes: raw pathname `/ar${p}`
matches candidate `p` iff `isReaderRoute(p)`; for `en` the raw path **is** the candidate
(`deLocalizeUrl` on an unprefixed path is identity once patterns are deleted). The
`/ar` marketing-home remap (`hooks.ts:43-45`) stays. Product pages join the same
mechanism: the pattern list gains `/search`, `/settings`, `/bookmarks`, `/yours`
(static) so `/ar/settings` reroutes onto the same tree — see Open question Q1
(default: yes, uniform rule, which also fixes today's seam where an Arabic-UI user on
`/ar/app/surah` clicking Bookmarks lands on unprefixed `/app/bookmarks` with English
chrome — `lib/i18n/app-locale.ts` header comment).

### 5.3 `src/hooks.server.ts`

- `localizedReaderLocale` (`lib/server/reader-route.ts:41-44`): regex
  `^/(en|ar)/app(?:/|$)` → ar-detection only: `^/ar/…` for an application path (reuse
  the reroute pattern list or a single `^/ar/` + application-prefix check). `en` is the
  absence of a prefix. Return type stays `UiLocale | null`.
- `legacyReaderRedirect` (`hooks.server.ts:170-183`) → generalized
  `legacyPrefixRedirect` with **ordered first-match rules**, each computing the final
  Location in **one hop** (normalize, then numeric-alias + surah-local applied to the
  normalized path inside the same pass — no 308 chains):

  | Inbound | Location | Notes |
  | --- | --- | --- |
  | `/en/app/…rest` | `/{rest}` after §6 rewrites | covers every `en` URL ever published |
  | `/en` | `/` | bare prefix |
  | `/en/…rest` | `/{rest}` | per owner decision, whole-`/en` strip; only reader paths ever existed under `/en` |
  | `/ar/app/…rest` | `/ar/{rest}` after §6 rewrites | |
  | `/ar/app` | `/ar/surah` | hub |
  | `/app` | `/surah` | hub |
  | `/app/…rest` | `/{rest}` | |

  All 308 + `cache-control: public, max-age=86400` (existing policy,
  `hooks.server.ts:176-182`; audit D15/Q4). Query + hash preserved
  (`${event.url.search}${event.url.hash}` pattern, `hooks.server.ts:173`).
  Ordering precondition: legacy redirects run **before** paraglide
  (`hooks.server.ts:362` vs the `:376-388` `useI18n` branch), and the outputs of the
  table match no rule input — no redirect cycle (loop-trace in §9; holds only under
  the §5.1 default-pattern fix).
- `readerRequestBase` (`hooks.server.ts:189-194`): prefix union becomes `{"" , "/ar"}`
  (`{prefix: "/en"}` branch dies). `numericChapterRedirect` (`:201-214`) regex
  `/^\/app\/([1-9][0-9]*)$/` → `^\/([1-9][0-9]*)$` against the prefix-less `rel`, so
  `/2` and `/ar/2` both alias; the legacy one-hop composition above handles
  `/en/app/2 → /al-baqarah` directly.
- `surahLocalPageRedirect` (`:221-233`) — prefix mechanics already flow through
  `readerRequestBase`; `surahLocalRedirectTarget` (`reader-route.ts:175-200`) regexes
  lose `/app` (`:166-167`) and build `/…` paths (`:188,191`).
- `useI18n` gate (`hooks.server.ts:371-372`): `readerLocale !== null ||
  isLocalizedMarketingPath(pathname)` becomes
  `useI18n = pathname === "/" || pathname === "/ar" || pathname.startsWith("/ar/")`.
  The bare `/ar` must stay inside the gate: `isLocalizedMarketingPath`
  (`hooks.server.ts:328-330`) admits it today, and `paraglideMiddleware` is what 307s
  document requests (`Sec-Fetch-Dest: document`) to the locale-canonical URL
  (`web/src/lib/paraglide/server.js:125-137`) — that is how `/ar` reaches `/ar/` now.
  Post-flip, `/ar` outside the gate would render the English home AT `/ar`. Unprefixed
  application paths keep skipping paraglide exactly like `/app/bookmarks` does today
  (baseLocale fallback, `app-locale.ts` contract). The
  noncanonical `parseReaderRoute` 404 branch (`:379`) keeps working off the rerouted
  route id.
- md fetch de-localization (`hooks.server.ts:352`):
  `.replace(/^\/(?:en|ar)(?=\/app\/)/u, "")` → `.replace(/^\/ar(?=\/)/u, "")` (only
  `ar` md paths are prefixed now).
- `translationRouteCacheKey` (`hooks.server.ts:38-46`): **unchanged** — key is
  `sourceId + cacheKind + index + __ui-{locale}` (§7.4).

### 5.4 Accept-negotiation + production server path shapes

- `web/src/lib/accept-parse.ts`: `READER_MD_SIBLING_PATTERNS` (`:30-38`) drop `/app`
  and the optional locale prefix narrows to `(?:/ar)?`; the `readerMdSibling` guard
  (`:41-47`) loses the `/en` spellings and the index exclusion becomes
  `/juz` + `/ar/juz`. Export a shared `isReaderMdPath(pathname)` predicate from here.
- `web/server.ts`: `servePrerenderedMd` prefix regex
  `/^\/(?:en|ar)(?=\/app\/)/u` (`server.ts:326`) → `/^\/ar(?=\/)/u`; the
  `routeNegotiated` gate `/^\/(?:en|ar)\/app\/.*\.md$/u` (`:350`) → the shared
  `isReaderMdPath` predicate (kills the third copy of the pattern list).
- `agentNotFoundMarkdown` (`lib/server/markdown-negotiation.ts:34-39`): re-path
  `/app/al-fatihah` → `/al-fatihah`, `/app/juz` → `/juz`.

### 5.5 Locale-context invariant

`docs/quran-system.md` Part 5 #2 ("no locale context silently dropped when switching
texts"): the translation ctx lives in the content path (`/t/{lang}/{translator}`) and
flows exclusively through `surahRouteContext`/`routeContextFromParams` +
`surah*For(ctx, …)` (`web/src/lib/data/quran.ts:90-142`) — **none of that code changes
shape**, so translation context preservation is structurally untouched. UI locale
switcher: `(application)/app/+layout.svelte:47-57` builds both locale hrefs from the
same `chromeReaderHref` — post-flip `readerHrefFor("en", h)` = `h` and
`readerHrefFor("ar", h)` = `/ar${h}`, so the pair stays reciprocal. The
`isNonReaderAppRoute` fallback (`:36-52`) simplifies: product pages get real
`/ar/...` twins (Q1 default), so the switcher links the sibling URL directly instead of
falling back to the reader home.

---

## 6. Canonical / SEO / sitemap / prerender

- `READER_CANONICAL_UI_LOCALE` (`web/src/lib/i18n/seo.ts:7`) **deleted**.
  `readerCanonicalPath` (`:56-58`) becomes validation + identity: return the (already
  canonical-shaped) `quranHref` after `isCanonicalReaderHref`. Callers:
  `readerCanonicalUrl` (`:60-62`), sitemap `readerUrl` (`sitemap.xml/+server.ts:27-33`),
  Seo consumers. Canonical paths are the unprefixed en forms.
- **hreflang pair (default per ask: add it, two lines):** in `readerUrl`, emit
  `<xhtml:link rel="alternate" hreflang="en" href={loc}/>` +
  `<xhtml:link rel="alternate" hreflang="x-default" href={loc}/>` + one
  `hreflang="ar"` line pointing at `/ar` + canonical path. Three lines total (en is
  free — it is the `<loc>`), still one canonical per content location; translations
  stay out (D12/D13 unchanged). M3 ships it alone so the sitemap diff is attributable.
- **Sitemap**: locs unprefixed automatically once `readerCanonicalPath` is identity —
  no structural change (`sitemap.xml/+server.ts:61-71` iterates
  `readerPrerenderEntries` unchanged; single-source-of-truth constraint honored).
- **Prerender**: `readerPrerenderHrefs` (`reader-prerender.server.ts:93-112`) —
  en hrefs come out unprefixed (builder change), ar hrefs `/ar/…`; the `home`
  entry dies (§4); uniqueness assert stays; Arabic-only invariant stays (no `/t/` in
  any discovered href — asserted by `reader-prerender-locales.test.ts:71`).
  Counts: entries 1048 (114 surah + 604 page + 30 juz + 60 hizb + 240 rub, unchanged)
  × 2 locales + 3 indexes × 2 = **2102 prerendered reader pages** (was 2104 — only the
  two home pages die). Arabic reader routes stay prerendered; translated routes stay
  SSR + 7-day disk cache; **no rendering/typography change**.
- **Disk cache (`__ui-` partition)**: `translationRouteCacheKey`
  (`hooks.server.ts:38-46`) keys on `build-{version}__{sourceId}__{kind}__{index}__ui-{en|ar}`
  (`quran-disk-cache.ts:51-57`) — **no URL in the key**, so the scheme change needs no
  key migration: new build id namespaces every entry anyway (`quran-disk-cache.ts:53-56`
  comment), and any surviving old-shape entries simply expire at the 7-day TTL
  (`DEFAULT_TTL_MS`, `quran-disk-cache.ts:12`). Reviewed: no change required.

---

## 7. Builders — every href producer learns the new shapes

| File | Change |
| --- | --- |
| `web/src/lib/data/quran.ts:49-142` | All path literals drop `/app`: `surahPath` → `` `/${slug}` ``, `translationSurahPath` → `` `/${slug}/t/${lang}/${translator}` ``, `translationGlobalPagePath` → `` `/t/…/page/${n}` ``, `globalPagePathFor` → `` `/page/${n}` `` / `` `/t/…/page/${n}` ``, same for `juzPathFor`/`hizbPathFor`/`rubPathFor` and their `translation*Path` twins. Return types `` `/app/${string}` `` → `` `/${string}` ``. Arabic-only helpers (`surahPath`, `surahAyahPath`, `translation*Path`) keep their component-import ban (nav-guard). |
| `web/src/lib/i18n/reader.ts` | `QuranReaderHref` = `` `/${string}` `` (validated); `LocalizedReaderHref<en>` = `QuranReaderHref`, `<ar>` = `` `/ar${string}` ``; `readerHrefFor` stops calling paraglide for `en` (identity after validation) and prefix-strips-in for `ar` via `localizeHref` with the same byte-equality assert (`:142-157`); `readerHomeHrefFor` deleted (§4); `bookmarksPageHref`/`yoursPageHref` → `"/bookmarks"`/`"/yours"` (`:172-183`); `RESERVED_SURAH_SEGMENTS` unchanged, `isReaderPathname` (`:92-122`) re-based onto prefix-less shapes. |
| `web/src/lib/i18n/seo.ts` | `READER_CANONICAL_UI_LOCALE` removed; `readerCanonicalPath` identity; `ENTRY_PATHS` (`:64-69`) → `/juz`, `/surah`, `/pages`; `home` removed from `ReaderEntryPage`. |
| `web/src/lib/i18n/marketing-copy.ts` | `marketingReaderHomeHref` (`:207-209`) → `/surah`-`/ar/surah` (or replaced by `readerHrefFor` at its 3 call sites). |
| `web/src/routes/(application)/app/_reader/translation-nav.ts` | `positionOf` strip regex `/^\/app\/?/` (`:35`) → de-localize then optional `/ar` strip then `/` ; `arabicHrefFor`/`hrefFor` inherit helper output (`:86-111` no literal change). `liveReaderPosition` (`:77-83`) already `deLocalizeUrl`s. |
| `web/src/lib/server/reader-route.ts` | `parseReaderPath` (`:132-157`): all shapes prefix-less; `parseReaderRoute` (`:212-268`): route-id switch re-based to new ids; `surahLocalRedirectTarget` regexes/targets (`:166-200`); `localizedReaderLocale` (`:41-44`). |
| `src/hooks.ts` / `src/hooks.server.ts` | §5.2, §5.3, §6 rule table. |
| `web/src/lib/accept-parse.ts`, `web/server.ts` | §5.4. |
| `web/src/lib/components/nav/Nav.svelte:138-140` | `/app/settings|search|bookmarks` → `/settings|search|bookmarks`. |
| `web/src/lib/search/palette/sources/{site,search,settings}-routes.ts` | same re-path (`:26`, `:8`, `:8`). |
| `web/src/routes/(marketing)/+page.svelte:33-35,93` | index links + search form action → `/search` etc. |
| `web/src/routes/(marketing)/_components/MarketingHeader.svelte:68-70` | index links re-pathed via unchanged `readerHrefFor`. |
| `web/src/lib/seo/render.ts:77-83` (llms.txt) | re-path all route examples; **also delete the stale "Surah page N (N ≥ 2): `/app/<slug>/page/N`" line (`:78`) — dead since D1; llms.txt currently advertises removed URLs.** |
| `web/src/service-worker.ts` | `:55` precache `"/en/app"` → `"/surah"` (or drop); `:400-411` `isLegacyReaderPath` purge family extended to `/en/**`, `/app/**`, `/ar/app/**` so PAGES/DATA caches self-clean old-shape entries on activate. |
| `web/src/lib/seo/render.ts:123` `pagePath` | unchanged (marketing only). |
| `web/src/lib/auth/{post-auth-path,route-guard}.ts` | `"/app"` → `"/surah"` (§4). |
| `web/src/lib/components/auth/AuthModal.svelte:35`, `(auth)/{login,register,verify-email}/+page.svelte`, `auth/[provider]/failure/+page.svelte:28`, `auth/[provider]/success/success-destination.ts:10` | full goto/href re-point list in §4. |
| `web/src/routes/design/_variants/copy.ts:7`, `web/src/routes/(application)/app/search/_components/TranslationPicker.svelte:12` | re-path (§4 — both outside nav-guard visibility). |
| `web/src/routes/(application)/app/yours/+page.svelte:32-36` | `readerHrefFor` inputs drop `/app`. |
| `web/src/routes/(application)/app/_reader/{RangeReader.svelte:152,TranslationModal.svelte:308}` | type literals `` `/app/${string}` `` → `` `/${string}` ``. |
| `scripts/assert-headers.sh:48-99`, `scripts/gen-offline-pack.ts:62` | scheme-A rework / legacy-set inversion (§4, Scripts). |
| `web/src/routes/+layout.svelte:34` | canonicalPath `/app` guard → dropped with the hub (or `/surah`). |
| `web/src/lib/i18n/base-english-copy.ts:58` | `case "app"` + `path === "/app"` branch deleted with the hub. |

**Filesystem path strings (blocker class, distinct from URL literals):** nine files
read source files by the physical `src/routes/(application)/app` path and break at
import/load time after `git mv` — verified by
`git grep -ln 'application)/app' -- src scripts` (exactly these 9):
`settings/__tests__/usage-bar-guard.test.ts:6-8` (a `readFileSync` of
`UsageBar.svelte` — crashes at module load),
`settings/__tests__/route-isolation.test.ts`,
`lib/i18n/__tests__/app-locale.test.ts`,
`lib/i18n/__tests__/marketing-surface-guard.test.ts`,
`lib/server/__tests__/dotted-translator-route.test.ts`,
`lib/server/__tests__/localized-reader-hook.test.ts`,
`lib/server/__tests__/reader-route.test.ts`,
`(application)/app/__tests__/service-worker-pending.test.ts`,
`_reader/__tests__/page-heights.test.ts`. All nine re-base their relative path
strings to `(application)/…` in the same commit as the move — a grep for
`application)/app` must be zero-hit before the M2 merge gate.

**nav-guard updates** (`(application)/app/__tests__/nav-guard.test.ts`, moves to
`(application)/__tests__/`): `HAND_BUILT_APP_NAV = /\/app\/(?!\$\{string\})/`
(`:15`) was able to lean on the distinctive `/app/` marker; post-move the guard runs
two regexes over the same `NAV_SIGNAL`-scoped lines after `stripComments`
(`:16`, `:45-58` — the harness already shields comment-only hits:
`footer-links.ts:5-6`, `m/search.ts:3`, `reader-copy.ts:305`, `aliases.ts:3`,
`match.ts:118`, `engine.svelte.ts:87`, `quran-page-shape.ts:35`, `app-locale.ts:6`,
bookmarks `:26`, yours `:23,28`, `capture.ts:47`): (a) any literal `/app` on a
nav-signal line (catches stale code in *globbed* files — must be zero-hit there), and
(b) a reserved-segment ban — a range/translation literal immediately after a string
boundary followed by a path continuation, i.e. `href={"/page/" + n}` or
`` goto(`/t/en/sahih/juz/${n}`) `` is flagged, while the type-placeholder escape keeps
working exactly as today's `(?!${string})` exemption does (`:105-108`). Exact regex
wording is settled against the false-positive fixture cases (`:110-116`) at
implementation; the fixtures re-pin to the new shapes. **Guard blind spots are
explicit inventory, not guard coverage:** `success-destination.ts:10`,
`design/_variants/copy.ts:7`, and `TranslationPicker.svelte:12` carry no `NAV_SIGNAL`
token on the line, and `copy.ts` (a `.ts` under `routes/`) is outside all three
`import.meta.glob` roots (`:26-42`) — all three are re-pointed via the §4 list, and
their tests (`success-routing.test.ts`, landing-guard/design-variant fixtures,
translation-picker test) carry the regression duty. `ARABIC_ONLY_HELPERS` (`:14`)
unchanged; the `toBeInstanceOf(Function)` export list (`:61-69`) unchanged (helper
names don't move). The glob roots (`:26-42`) re-base one level up.

---

## 8. Milestones

Gates per milestone: `pnpm check`, `pnpm lint` (deny-warnings), `pnpm test` green +
`just web-build prod` (production build) before merge — the build is the only place
prerender/sitemap deltas surface. Ask's M1→M2→M3 order kept, with one justified
re-distribution: **sitemap/prerender re-path cannot wait for M3** — they are generated
from the same builders and the physical tree (single source of truth), so each
milestone carries its own re-path; M3 is hreflang + docs + exhaustive test sweep.

### M1 — locale flip inside the existing `/app` tree
en loses only the `/en` prefix; `/app` segment stays. Ships:
- paraglide: delete the whole `urlPatterns` array (`paraglide.config.js`) — the
  default `/:locale/...` pattern (§5.1) then yields exactly M1's grammar:
  `readerHrefFor("en", "/app/x") = "/app/x"`, `readerHrefFor("ar", …) = /ar/app/x`,
  `deLocalizeUrl("/ar/app/x") = /app/x`; plus the round-trip fixture tests.
- `readerHrefFor`/`LocalizedReaderHref` types reworked; **builders keep `/app` in M1**
  (`quran.ts` literals unchanged) — only the localized wrapper changes:
  `readerHrefFor("en", "/app/x") = "/app/x"`, `readerHrefFor("ar", …) = /ar/app/x`.
- hooks: `legacyReaderRedirect` `/en/app/*` → 308 `/app/*`; `localizedReaderLocale`
  regex `^/(ar)/app`; `readerRequestBase` prefix `{""|"/ar"}`; md regexes `ar`-only.
- reroute: `/ar/app/**` tuple mapping kept, `/en/app/**` no longer rerouted (hooks 308
  catches it first).
- Canonical flips here: `READER_CANONICAL_UI_LOCALE` deleted → canonical = `/app/…`
  en-form; sitemap locs re-generated unprefixed-of-`/en`; prerender en fan-out
  unprefixed (counts unchanged in M1 — home still prerendered as `/app` + `/ar/app`).
- SW precache `"/en/app"` → `"/app"`.
- Tests: reader-href, localized-seo, localized-sitemap, hook tests, reroute tests,
  prerender-locales re-pinned; new paraglide round-trip fixtures.

### M2 — drop the `/app` segment (tree move + redirect map + hub removal)
- `git mv` `(application)/app/**` → `(application)/**`; layout route-id checks,
  `parseReaderRoute` ids, nav-guard globs re-based.
- All builders drop `/app` (§7 table); `parseReaderPath` shapes prefix-less;
  `positionOf` strip regex; accept-parse + `web/server.ts` predicates.
- Redirect map live (§6 table): `/en/*` → `/*`, `/app` → `/surah`, `/app/*` → `/*`,
  `/ar/app` → `/ar/surah`, `/ar/app/*` → `/ar/*`; numeric alias at root
  (`/2`, `/ar/2`); single-hop composition.
- Hub deleted; resume card survives at `/yours`; `ReaderPrerenderLinks` + load move to
  `/surah`; `home` prerender entry dies (−2 pages, 2104 → 2102); post-auth/SW/palette
  `/app` hrefs re-point (§4 complete inventory, incl. auth flow, design copy.ts,
  TranslationPicker, RangeReader/TranslationModal type literals).
- **Scripts**: `scripts/assert-headers.sh` reworked to scheme A probes + 308
  assertions; `scripts/gen-offline-pack.ts:62` legacy filter inverted to
  `/en|/ar/app` (unprefixed keys become legal) — §4 Scripts.
- **Filesystem path strings**: the nine `application)/app` path-string files (§7)
  re-based; `grep -rn 'application)/app' src scripts` must be zero-hit at the gate.
- `[surah=surahSlug].md` + `[slug=marketingText].md` matcher split (§3) — could land
  earlier but the conflict only exists once the tree moves.
- `/ar/search|settings|bookmarks|yours` go live (Q1 default) — reroute + hooks gate
  only, no new route files.

### M3 — hreflang + docs + test sweep
- hreflang lines in sitemap `readerUrl` (§6).
- Docs: `docs/quran-system.md` Part 1 ("Public reader paths begin with `/{ui}/app`",
  ~line 172) and Part 2 ("Canonical reader paths use `/en/app/**` and `/ar/app/**`",
  ~line 195) rewritten; new Part 5 divergence #10 recording scheme A + the redirect
  policy (my-plan-raw.md readers find it); `web/README.md` layout comment
  (`(application)/app/` line) re-pathed; this file's status → implemented.
- `docs-divergence-guard.test.ts` markers (`TRANSLATED_MARKER /\/app\/t\//` `:21`,
  `BARE_APP_GLOB /\/app\/\*\*/` `:43`) re-based to the new shapes — doc
  wording keeps the negation guards passing.
- llms.txt copy fix (§7, incl. the stale `/page/N` line).
- Full literal sweep (§9) + production build eyeball: prerender dir = new shapes,
  sitemap 2102-loc-class inventory, `pnpm check && pnpm lint && pnpm test`.

---

## 9. Risks

- **Prerender path changes.** Every Arabic reader page changes path in M2. The build's
  uniqueness assert (`reader-prerender.server.ts:108-110`) catches duplicate discovery;
  `just web-build prod` + inspect `build/prerendered/` is the eyeball gate. sirv serves
  the new files by path — no stale-file risk (old files simply aren't emitted).
- **Cache-key invalidation.** Non-issue by construction: keys carry
  `build-{version}` (`quran-disk-cache.ts:53`), so a new deploy never reads old HTML;
  old keys expire at 7d TTL. The `__ui-` partition survives unchanged.
- **Paraglide middleware ordering / semantics.** The urlPatterns deletion rests on the
  generated default-pattern behavior — verified in the generated runtime source
  (`runtime.js:149,1009-1038,1018-1036,1111-1121`, §5.1) and pinned by round-trip
  fixture tests (§5.1); the existing `localizeReaderHref` byte-equality throw
  (`reader.ts:149-154`) fails the build/render loudly if the regenerated runtime
  behaves otherwise. Hooks order (redirects → paraglide gate → cache) is unchanged;
  `deLocalizeUrl` consumers (`hooks.ts:45`, `translation-nav.ts:79`,
  `+layout.svelte:45`) all re-verified in their sections.
- **Crawl-window overlap.** Every published URL (en + ar, with and without `/app`,
  with and without `/en`) is covered by the 308 table — one hop to a live canonical.
  308 + `max-age=86400` per the audit's D15 policy. Translated URL trees keep their
  shapes (only prefix+marker change) so deep translation links also redirect cleanly.
- **Tests asserting literal `/app` or `/en`** — the full grep inventory (run this
  session, `grep -rln 'en/app|/app/' --include="*.test.ts"`), amended by the review
  pass (`git grep '"/app"' -- src/lib/auth/__tests__`): adds
  `lib/auth/__tests__/return-target.test.ts:65` (`setReturnTarget("/app")`) and
  `lib/auth/__tests__/route-guard.test.ts:19,24` (`toBe("/app")`).
  Known false positives, excluded: `oauth-flow.test.ts:103` (`/api/auth/apple`),
  `reader-fonts.test.ts:21,29` (`src/app.html`), `app-locale-harness.svelte:2`
  (`$lib/i18n/app-locale`). The remaining list:
  `accept-parse`, `reader-prerender-locales`, `quran.test`, `app-locale`,
  `localized-seo`, `marketing-copy`, `marketing-surface-guard`, `public-href`,
  `reader-href`, `reader.test`, `offline keys/offline-store/pack`, `mode-param`,
  `more-param`, `query-url`, `palette sources`, `render`, `dotted-translator-route`,
  `localized-reader-hook`, `markdown-negotiation`, `preview-headers`,
  `reader-markdown`, `reader-route`, `server-headers`, `surah-local-redirect`,
  `auth-cache-purge`, `docs-divergence-guard`, `nav-guard`,
  `reader-localization-integration`, `reader-md-routes`, `routes.test`,
  `service-worker-*` (5 files), `TranslationModal`, `page-heights`, `surah-reader`,
  `translation-nav`, search/settings `route-isolation`, `landing-guard`,
  `success-routing`, `localized-sitemap`, plus the §7
  filesystem-path-string class (9 files) and `usage-bar-guard.test.ts`.
  Each is a mechanical re-pin; §10 lists the semantic ones.
- **Redirect/paraglide loop-trace (check run during review — passes under the §5.1
  default-pattern fix, only under it):** `handle` order is md-sibling negotiation
  (`hooks.server.ts:338-356`) → `legacyReaderRedirect` (`:362`) →
  numeric/surah-local (`:373-375`) → `paraglideMiddleware` only inside the `useI18n`
  branch (`:376-388`) → 404-md fallback → `applyHeaders`. The §6 rule table's outputs
  (unprefixed paths, `/surah`, `/ar/surah`, `/ar/<rest>`) match no rule input
  (`/en/**`, `/app*`, `/ar/app*`), and unprefixed en paths skip the paraglide gate —
  no 308/307 cycle. Paraglide itself only 307s document requests whose URL differs
  from the determined locale's canonical (`paraglide/server.js:125-137`); today's
  `/ar/app/**` already passes through it without redirect — same shape post-flip.
- **`/en` as a reserved word regresses.** After M2, `/en/...` 308s to `/*`; nothing may
  later create real `/en/**` routes. Note added to the docs divergence entry.
- **hub removal breaks muscle memory / saved bookmarks** to `/app` — covered by the
  308; SW precache updates; post-auth deep link re-points to `/surah`.
- **`(application)/+layout.ts` `prerender = true`** now sits at group scope: every
  translated `+page.server.ts` must keep its explicit prerender-off (verified pattern
  already in place for the translated twins — same files, moved).

---

## 10. Test plan

**Existing, updated (old-shape literals → new):**

- `(application)/app/__tests__/routes.test.ts` → moves to `(application)/__tests__/`;
  every `/app/...` expectation re-pinned (`:34-35,51-53,120,137,144,176,190-216,224,231,255,267,290-301,313,338-339`).
- `nav-guard.test.ts` — §7 guard-regex rework + fixture re-pins; glob roots.
- `lib/server/__tests__/reader-route.test.ts` — `parseReaderPath` table (`:13-91`),
  reroute cases (`:126-135`), `surahLocalRedirectTarget` targets (`:141-166`).
- `lib/server/__tests__/dotted-translator-route.test.ts` — reroute URL cases
  (`:85-95`).
- `lib/server/__tests__/localized-reader-hook.test.ts` — legacy 308 expectations
  (`:75-93`: location `/en/app/al-fatihah?view=reading` → `/al-fatihah?view=reading`),
  numeric alias (`:95-120`), ar-prefixed cases, plus the whole i18n/HTML-cache suite
  downstream (`:120+`).
- `lib/server/__tests__/server-headers.test.ts` — header/negotiation path inputs.
- `lib/server/__tests__/surah-local-redirect.test.ts` — target paths lose `/app`.
- `lib/server/__tests__/markdown-negotiation.test.ts` + `lib/__tests__/accept-parse.test.ts`
  — sibling patterns (`accept-parse.ts:30-47`).
- `routes/(application)/app/__tests__/reader-md-routes.test.ts` — handler imports move
  with the dirs (`:6-9`); canonical-path assertions inside re-pin.
- `routes/(application)/app/__tests__/reader-localization-integration.test.ts`
  (`:27-36`), `lib/i18n/__tests__/reader-href.test.ts` (`:24-57`),
  `lib/i18n/__tests__/localized-seo.test.ts` (`:36-57`),
  `lib/i18n/__tests__/app-locale.test.ts` (also a §7 path-string file),
  `lib/i18n/__tests__/marketing-copy.test.ts`,
  `lib/i18n/__tests__/marketing-surface-guard.test.ts` (path-string),
  `lib/auth/__tests__/return-target.test.ts:65` and
  `lib/auth/__tests__/route-guard.test.ts:19,24` (`"/app"` → `"/surah"`),
  `lib/components/i18n/__tests__/reader-prerender-locales.test.ts` (`:51-77` —
  `/en/app`-prefix filters become unprefixed, `INDEX_COUNT` 4→3, entry-href set
  re-keyed), `routes/sitemap.xml/__tests__/localized-sitemap.test.ts`
  (`:37-72`), `lib/seo/__tests__/render.test.ts` (llms.txt body),
  `routes/(application)/app/_reader/__tests__/translation-nav.test.ts` (`:7-49`),
  `_reader/__tests__/page-heights.test.ts` (path-string),
  `routes/(application)/app/__tests__/docs-divergence-guard.test.ts` — markers
  re-pinned to their real lines (`TRANSLATED_MARKER :21`, `SSG_MARKER :22`,
  `NEGATION :23`, `BARE_APP_GLOB :43`; used at `:37,:47,:104`); doc wording keeps the
  negation guards passing,
  `service-worker-*.test.ts` (precache/legacy-path units incl. `service-worker-pending`
  path-string), `settings/__tests__/usage-bar-guard.test.ts` (`readFileSync` path
  re-base, `:6-8`) and `settings/__tests__/route-isolation.test.ts` (path-string),
  other search/settings `route-isolation` tests,
  `routes/auth/[provider]/success/__tests__/success-routing.test.ts`
  (post-auth path + `success-destination.ts` fallback),
  `routes/(marketing)/__tests__/landing-guard.test.ts`,
  `lib/data/__tests__/quran.test.ts`, offline `keys`/`pack`/`offline-store` tests,
  `lib/reader/__tests__/{mode-param,more-param}.test.ts`,
  `lib/search/.../{query-url,sources}.test.ts`.
- New tests:
  - **308 map tables** (hook tests, table-driven): `/en/app/x → /x`,
    `/en/x → /x`, `/en → /`, `/app → /surah`, `/app/x → /x`, `/ar/app → /ar/surah`,
    `/ar/app/x → /ar/x`, `/en/app/2 → /al-baqarah` (single hop), `/ar/2 → /ar/al-baqarah`,
    query+hash preserved on every row, cache-control `public, max-age=86400` on every row.
  - **Numeric alias at root**: `/2`, `/ar/2`, `/114`, `/115` → 404 fall-through, `/0`,
    `/01` → 404 (POSITIVE_INTEGER gate).
  - **`[slug=marketingText].md` / `[surah=surahSlug].md` dispatcher cases**: `about.md`
    → marketing markdown; `al-baqarah.md` → reader markdown; `not-a-surah.md` → both
    matchers reject → 404; `al-baqarah.txt` → 404 (explicit guard); prerender entries
    = 6 marketing + 114 surah, disjoint.
  - **paraglide round-trip fixtures**: `localizeHref`/`deLocalizeUrl` identity (en) and
    `/ar` prefix (ar) over `{/, /al-baqarah, /t/en/sahih/page/7, /search, /ar/page/13}`.
  - **reroute table**: new root shapes + `/ar/**` product pages map onto physical ids;
    `/en/**` never reroutes (hooks owns it).
  - **prerender/sitemap inventory**: en hrefs unprefixed, ar hrefs `/ar/…`, zero
    `/app` substrings anywhere in either artifact (guard assertion).

---

## 11. Docs that must move with the code

- `docs/quran-system.md` Part 1 Web delivery bullet (~`:172-175`) and Part 2 bullets
  (~`:195-199`) — path grammar rewritten; Part 5 gains divergence #10 (scheme A:
  unprefixed en, `/ar` ar, `/app`+`/en` 308 maps, `/en` reserved-word note).
- `web/README.md` layout comment (routes tree) — `(application)/app/` →
  `(application)/`, `[slug].md` note gains the matcher split.
- `docs/research/navigation-audit.md` — no edit required (historical record; its
  shapes are superseded by this doc, cross-referenced).
- `AGENTS.md` nav-invariant text mentions the `surah*For` family and nav-guard —
  names unchanged, no edit.

## 12. Non-goals

- No rendering/typography/virtualization changes; no data or DB changes; no new UI
  locales; no translation-model change (D9 stands); sitemap and prerender stay fed
  from `readerPrerenderEntries` (single source of truth — untouched); scroll position
  stays in `?v=`; no content edits beyond the llms.txt route examples.

## 13. Open questions

1. **Q1 — `/ar` twins for product pages** (`/ar/search`, `/ar/settings`,
   `/ar/bookmarks`, `/ar/yours`). Default in this plan: **yes** — one uniform
   reroute/redirect rule, no exception table, and it fixes today's Arabic-chrome drop
   on those pages. Alternative: keep them en-only unprefixed (ask's literal list).
   One-line delta either way.
2. **Q2 — hub 308 target**: default `/surah` per owner. `/yours` is the alternative
   (it hosts the resume card) but is noindex-personal; `/surah` is the indexable index.
3. **Q3 — hreflang on the page `<head>`**: default sitemap-only two-liner (ask's
   "two lines"). Adding `link[rel=alternate][hreflang]` to `Seo.svelte` for reader
   pages is the follow-up if wanted.
4. **Q4 — matcher split rejected by the route compiler**: fall back to six static
   marketing `.md` dirs (§3 fallback). Decision point is the first M2 build.
5. **Q5 — SW precache replacement for the dead `/en/app` entry**: default `"/surah"`;
   dropping it entirely is fine (SW is offline shell, `/surah` is discoverable
   in-network first).
