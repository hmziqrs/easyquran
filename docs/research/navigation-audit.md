# Navigation audit — parity with the Quran's own model

Status: implemented (all milestones M1–M6; decisions per §8 defaults — Q1 full routes,
Q2 anchor kept, Q3 query param, Q4 max-age=86400, Q5 alternates dropped). Scope: reader
navigation only — addressing, URL
schemes, redirects, sidebar/wayfinding, SEO head. Companion to
[`ux-audit/01-navigation-and-wayfinding.md`](./ux-audit/01-navigation-and-wayfinding.md)
(whose NAV-* findings are UI-level; this document owns the URL/numbering model).

Every finding below was re-verified against the tree at the time of writing: code
cited by path and line, numbers reproduced from the built
`web/build/prerendered/sitemap.xml` (1307 `<loc>`, 548 surah-local, 604 global-page)
and from `web/static/quran-meta/quran-data.json` (page 601 starts at 103:1, page 602
at 106:1; al-baqarah has 48 local pages → 48 self-canonical locs in the sitemap).

---

## 1. Goal

Navigation parity with the Quran's own print model, as quran.com implements it:

1. **One global page numbering.** The Madani mushaf's pages 1–604 are the only page
   numbers anywhere a page number appears — labels, pills, sidebar, sticky bar,
   URLs, SEO titles. A printed page that spills across a surah boundary is never
   renumbered per surah.
2. **quran.com's sidebar sections.** Four tabs — Surah, Verse, Juz, Page — each
   usable from any reader route, not only from a surah route.
3. **quran.com's next/prev semantics per view.** Surah view = continuous scroll over
   **one canonical URL**, closed by an end-of-surah card linking the next/prev
   *surah*; page view (`/app/page/N`) and juz view keep explicit next/prev links
   *within their own family*. No path rewrite while scrolling; no per-spread
   canonicals.

What we deliberately do **not** adopt (see D9/D10, §4): quran.com's "translation is
a viewer setting, not a URL" model. EasyQuran's translated URL trees
(`/app/t/{lang}/{translator}/…`, `/app/{slug}/t/{lang}/{translator}`) stay — they
are a documented, bounded divergence (SSR + 7-day disk cache, translations excluded
from sitemap `<loc>`).

---

## 2. Confirmed discrepancies and decisions

| ID | Where (verified) | Gap | Severity | Decision |
| --- | --- | --- | --- | --- |
| D1 | `web/src/lib/data/quran.ts:55-62`, `app/[surah]/page/[localPage]/+page.server.ts:11-18`, `_reader/ReaderHeader.svelte:61`, `_reader/ReaderPageNav.svelte:52-74`, `lib/data/quran-data.ts:293-320`, `_reader/Sidebar.svelte:84-87` | Surah reader paginates by surah-LOCAL page numbers; one printed page splits across multiple surah URLs (page 601 = local page 1 of surahs 103, 104 **and** 105) | high | **Remove the surah-local page scheme.** Surah = one canonical URL; page addressing = global `/app/page/N` only; every label shows the global page |
| D2 | `_reader/SurahReader.svelte:859-867,636-657,190-195`, `_reader/translation-nav.ts:63-66` | Scroll rewrites the history entry's **path** to `/app/{slug}/page/N`; canonical churns with scroll | medium | Scroll never changes path. Position rides in `history.state` (already snapshot-based) + `?v={surah}:{ayah}` query only — path and canonical stay the bare surah URL |
| D3 | `_reader/SurahPageRoute.svelte:32-42`, `messages/reader/en.json:248` | A long surah = up to 48 self-canonical page URLs with 48 titles (verified: 48 al-baqarah locs) | medium | One canonical (`/app/{slug}`), one title, one description per surah; delete `reader_seo_surah_page_title` usage |
| D4 | `lib/server/quran-page-shape.ts:46-66`, `_reader/SurahPageRoute.svelte:178-185`, `app/page/[n]/+page.svelte:31-35` | `<link rel="prev/next">` pagination chain incl. cross-surah targets | medium | Remove all rel=prev/next (ignored by Google since 2019). Visible next/prev controls stay: surah cards in surah view, page links in page view |
| D5 | `_reader/ReaderShell.svelte:40-80`, `_reader/ReaderHeader.svelte:46-61` | No live mushaf position in the sticky bar; only label shown is surah-local | medium | Sticky bar gains "Page {global} · Juz {juz}" (quran.com ContextMenu pattern), live from scroll, SSR'd with the route's initial position |
| D6 | `lib/data/quran-data.ts:83-89`, `_reader/Sidebar.svelte:35`, routes tree (no `hizb/`, `rub/`) | Hizb (60) and rub' (240) baked in data, no route/tab/display | medium | Add `/app/hizb/{1..60}` + `/app/rub/{1..240}` routes mirroring the juz family (derived from baked HizbQuarter starts — no DB change); sticky bar shows hizb; juz-index quarter grid links to rub routes |
| D7 | `_reader/Sidebar.svelte:235,278-279,54-63` | Ayah tab empty on any non-surah route | medium | Ayah tab falls back to the reader's current/last-read surah (`reader.setCurrent` store field, default 1) instead of rendering nothing |
| D8 | `app/surah/+page.svelte:13,19-21`, `app/juz/+page.svelte:12,18-24`, `app/pages/+page.svelte:14,20-22` | All three index pages hardcode `arabicCtx`; rows drop translation context the sidebar preserves | medium | Index rows derive ctx from the reader's active source after hydration (`resumeCtxFor` pattern); prerendered HTML stays Arabic (deterministic, matches sitemap) |
| D9 | `lib/data/quran.ts:79-96`, `lib/data/translations.json` (378 entries) | One content location addressable at hundreds of translation URLs | low | **Keep.** Documented product divergence (docs/quran-system.md Part 5 #1-2); sitemap policy already bounds the SEO surface |
| D10 | `lib/data/quran.ts:79-87` vs `:89-96`, `_reader/translation-nav.ts:30-57` | `/t/` sits after the slug for surahs, before it for ranges; parser needs the `tIdx===1` branch | low | Keep both top-level shapes (follows D9). The surah-local **page tail** dies with D1, which simplifies `positionOf`; keep the asymmetry, document it |
| D11 | `sitemap.xml/+server.ts:96`, `app/+page.svelte:107`, `components/seo/Seo.svelte:154-157` | Sitemap lists `/en/app`; the page renders noindex with no canonical | low | Drop `plainReaderEntryUrl("home")` from the sitemap — sitemap matches robots |
| D12 | `sitemap.xml/+server.ts:90-95,40-54` | Translated routes appear only as hreflang alternates, never `<loc>` | low | **Keep** (follows D9). One canonical per content location; translation routes stay discovery-only |
| D13 | `sitemap.xml/+server.ts:40-54`, `lib/i18n/seo.ts:56-58` | 60,928 hreflang alternates marking content languages on en-UI URLs — not UI-language variants; Google ignores the cluster | low | Remove the per-translation alternates block from reader URLs; keep `ar` + `x-default` pointing at the Arabic canonical `<loc>` |
| D14 | `lib/server/reader-route-guards.ts:8-12`, `lib/i18n/reader.ts:7,15`, `lib/server/reader-route.ts:137` | `/app/2` hard-404s; no numeric chapter alias (quran.com 308s `/2` → `/al-baqarah`) | low | Add a 308 numeric alias `/app/{1..114}` → `/app/{slug}` (localized prefix preserved) in `hooks.server.ts`, before the parse-404 |
| D15 | `hooks.server.ts:167-178` | Legacy `/app/*` → `/en/app/*` is 307 + no-store; target is deterministic `/en` | low | Upgrade to 308 permanent with a bounded public cache (`max-age=86400`) |
| D16 | `_reader/ReaderPageNav.svelte:76-103` (rendered at `SurahReader.svelte:1178`), `quran-page-shape.ts:30-34` | Surah end is a bare prev/next pill pair — no end-of-chapter card grammar | low | Upgrade to the quran.com end card: heading, "Read again" (→ surah root), next/prev surah cards with meaning subtitle; boundary nulls (Fatihah/Nas) already correct |

---

## 3. Per-discrepancy fix design

### 3.1 The one-URL-per-surah migration (D1 + D2 + D3 + D10-tail)

The core move: **the surah-local page scheme is removed, not relabeled.** Relabeling
local numbers to global would keep 548 duplicate self-canonical URLs and the
"Page 3 means a different spread per surah" bug; removing the scheme collapses
surah, scroll, canonical, and sitemap into the quran.com model in one stroke.

**Routes removed** (Arabic + their `.md`/`.txt` text-variant siblings + the
translated mirrors):

- `web/src/routes/(application)/app/[surah]/page/[localPage]/` (+ `.md`)
- `web/src/routes/(application)/app/[surah]/t/[lang]/[translator]/page/[localPage]/` (+ `.md`)

**Redirects (308, permanent).** A pure builder in
`web/src/lib/server/reader-route.ts` next to `parseReaderPath`:

```ts
// surahLocalRedirectTarget(pathname) → { location, tail } | null
// /app/{slug}/page/N        → /app/{slug}#ayah-{surah}-{startAyah}
// /app/{slug}/t/l/tr/page/N → /app/{slug}/t/{lang}/{translator}#ayah-{surah}-{startAyah}
```

- Data source: `QURAN_DATA.surahLocalPage(surah, localPage)` — the baked
  `surahLocalPages` map (`web/static/quran-meta/quran-data.json`, decoded by
  `web/src/lib/data/quran-data.ts:293-320`) already carries `globalPage`,
  `startAyah` per local page. No new data, no hashing.
- Wired in `web/src/hooks.server.ts` beside `noncanonicalLocalizedReaderRedirect`
  (`hooks.server.ts:180-199`), preserving the `/en`|`/ar` prefix and query; runs
  **before** the `parseReaderPath` 404, because the local-page regexes are deleted
  from `parseReaderPath` (`reader-route.ts:137`, the
  `/app/{slug}/page/N` and `/app/{slug}/t/…/page/N` matchers).
- Why the `#ayah-` fragment: the page's first ayah anchor is already a supported
  deep link (`SurahPageRoute.svelte:89-94` `requestedAyah`), so an inbound
  `/page/N` link lands on the same spread instead of the surah top. Bare-root
  fallback if the anchor proves noisy (open question Q2).
- Keeping the deleted routes' loaders as thin 308 loaders was rejected: the hooks
  layer must handle them anyway (prerendered files for removed routes no longer
  exist, so adapter-node falls through to SSR), and one builder + one hook branch
  covers Arabic, translated, and text-variant shapes uniformly.

**Prerender impact (Arabic routes):** `readerPrerenderEntries`
(`web/src/lib/components/i18n/reader-prerender.server.ts:43-63`) loses the
`surah-local-page` kind: −548 entries × 2 UI locales = −1096 prerendered pages.
Surah roots stay prerendered; nothing Arabic becomes SSR.

**SSR + 7-day disk cache (translated routes):** the removed translated shapes exit
`parseReaderRoute`/`parseReaderPath`, so their disk-cache keys simply stop being
produced. Old cached entries expire untouched (7-day TTL). Translated surah roots
stay SSR + disk cache — no policy change.

**Client scroll/URL (D2).** In `SurahReader.svelte`:

- `setVisiblePage` (`:859-867`) stops calling `writeHistoryState` with a new
  `pagePathFor` URL. `pagePathFor` (`:190-195`) is deleted.
- `writeHistoryState` (`:636-657`) keeps `replaceState` for the restore snapshot +
  `persistReaderPosition`, but targets the **current path** with an optional
  `?v={surah}:{ayah}` query (quran.com's `?startingVerse` analogue). Path never
  moves; canonical can never churn because it stops depending on scroll (below).
  `withModeParam` continues to own `?mode=`.
- `translation-nav.ts` `positionOf` (`:30-57`) loses the surah-local branches; the
  `tIdx===1` disambiguation (`:46-52`) survives in reduced form (surah-with-t vs
  range-with-t) and `liveReaderPosition` no longer fights the scroll rewrite —
  its comment (`:63-66`) is deleted with the behavior.
- `revealRequestedAyah` (`SurahPageRoute.svelte:112-155`) drops its `goto()`
  cross-page branch (`:124-127`): the target page is streamed in-place by
  `SurahReader` (same anchor-preserving queue it already uses for
  `restoreHistoryFrom`), then `scrollIntoView`. No navigation, no URL change.

**Canonical/title/description (D3).** In `SurahPageRoute.svelte`:

- `canonicalPath` (`:35`) becomes `surahPathFor(routeContext, surah)` — scroll-
  independent; `scrolledPage` stops feeding SEO.
- `seoTitle` (`:38-42`) becomes `copy.seo.surahTitle(...)` unconditionally.
- Descriptions (`:47-65`): new surah-level copy keys (see §7 i18n), page-range
  parameters dropped.
- The `sr-only` scroll announcer (`SurahReader.svelte:1162-1164`) switches to the
  global page label; title stays stable (also resolves ux-audit KEY-01).

**Helpers (`web/src/lib/data/quran.ts`).**

- `surahAyahPathFor` (`:140-145`) is **repurposed**: drops the `localPage`
  argument, returns `` `/app/${slug}#ayah-${num}-${ayah} `` (Arabic) /
  `` /app/${slug}/t/${lang}/${translator}#ayah-… `` (translated). Call sites to
  update: `Sidebar.svelte:307-313`, `SurahPageRoute.svelte:118-122`, search
  results sources under `web/src/lib/search/` (nav-guard-globbed).
- `surahLocalPagePathFor` (`:128-138`) and `surahLocalPagePath` (`:55-62`) are
  **deleted** once their call sites are gone: `quran-page-shape.ts`,
  `reader-prerender.server.ts`, `[surah].md`/`[surah]/page/[localPage].md`
  servers, `ReaderPageNav.svelte`, `translation-nav.ts`, `SurahReader.svelte`,
  `SurahPageRoute.svelte`, `[surah]/page/[localPage]/+page.server.ts` (itself
  removed). Server-only consumers (`quran-page-shape.ts`) keep reading
  `QURAN_DATA.surahLocalPages` for data, never for hrefs.

**Degraded page pills (part of D1).** `ReaderPageNav.svelte:52-74` relabels to
global pages: `pageHref` becomes `globalPagePathFor(ctx, page.globalPage ± 1)`
—the `SurahLocalPage.globalPage` field (`quran-data.ts:308`) is already in the
page payload. Labels use `copy.range.item("page", globalPage)`.

**next/prev semantics (D4 + D16).**

- `SurahRouteNav` (`quran-page-shape.ts:20-27`): drop `readingPreviousHref` /
  `readingNextHref` and the `pageLink` chain; keep `previousSurah`/`nextSurah`
  (boundary nulls verified correct at `quran-page-shape.ts:30-34`). Extend
  `SurahLink` with `meaning` for the end-card subtitle (source:
  `CatalogEntry.meaning`, `toSurahLink` in `web/src/lib/server/quran-data.ts`).
- `SurahPageRoute.svelte:178-185`: rel prev/next block deleted.
- `app/page/[n]/+page.svelte:31-35` and `juz/[n]`: rel prev/next deleted;
  visible prev/next controls stay (page view: `globalPagePathFor ± 1`, clamped
  1..604; juz view: `juzPathFor ± 1`, clamped 1..30) — matching quran.com's
  "Next page"/"Next Juz" end links.
- `ReaderPageNav.svelte` becomes the end-of-surah card (D16): `copy.shell`
  heading ("End of Surah {name}"), "Read again" link (surah root via
  `surahPathFor`), prev/next surah cards carrying `meaning`, prev/next nulls
  render nothing (existing behavior). New copy keys per §7; hotkeys untouched
  (any new keyboard affordance goes through `$lib/hotkeys.svelte.ts` per
  AGENTS.md).

### 3.2 Sticky position indicator (D5)

- New tiny store field in `web/src/lib/stores/reader.svelte.ts`:
  `position: { globalPage: number; juz: number } | null`, written by:
  - `SurahReader.setVisiblePage` — `pageData.page.globalPage` is already in the
    payload; juz derived client-side from `loadQuranData().ranges(RangeKind.Juz)`
    (`Sidebar.svelte:130-132` `juzOf` is the existing pattern — extract it to a
    shared helper instead of duplicating).
  - Range views (`app/page/[n]`, `app/juz/[n]`, and the M4 hizb/rub routes) —
    index known at load; juz of a global page from the same helper.
- New `_reader/PositionIndicator.svelte`, rendered inside the sticky
  `ReaderShell.svelte` header row (`:46-79`) between the header snippet and the
  A−/A+ group: `Page {globalPage} · Juz {juz}` (plain spans, quran.com's
  `page-info` pattern — not links). SSR renders the route's initial position
  (Arabic routes: server-known; translated: first cacheable paint), hydration
  takes over from the store. RTL: inherits shell direction.
- `ReaderHeader.svelte:61` relabels to the global page
  (`initial.page.globalPage`) or the label moves entirely into the sticky bar —
  implementation detail settled at M1 review; the local numbering dies either way.

### 3.3 Hizb and rub' addressing (D6)

- New helpers in `web/src/lib/data/quran.ts`, ctx-aware like `juzPathFor`
  (`:152-153`): `hizbPathFor(ctx, n)` → `/app/hizb/{n}` /
  `/app/t/{lang}/{translator}/hizb/{n}`; `rubPathFor(ctx, n)` → `/app/rub/{n}` /
  `/app/t/{lang}/{translator}/rub/{n}`. Never hand-built strings (nav-guard).
- Route dirs mirror `juz/[n]` exactly:
  - `app/hizb/[n]/+page.server.ts` (+ `.md`), `app/rub/[n]/+page.server.ts` (+ `.md`):
    `prerender = true`, `entries()` over the new counts.
  - `app/t/[lang]/[translator]/hizb/[n]/+page.server.ts`, `.../rub/[n]/+page.server.ts`:
    `prerender = false`, `loadTranslationRangeData` + `markTranslationPending`
    (identical to the juz twin at
    `app/t/[lang]/[translator]/juz/[n]/+page.server.ts`).
- Counts: `RUB_COUNT = RANGE_COUNTS[RangeKind.HizbQuarter]` (=240, baked,
  `quran-data.ts:87`); `HIZB_COUNT = 60`, asserted as
  `RANGE_COUNTS[RangeKind.HizbQuarter] % 4 === 0` at module load. Hizb *i* is the
  union of quarters `4i-3..4i`: `startGlobal = quarter(4i-3).startGlobal`,
  `endGlobal = quarter(4i).endGlobal`. This is metadata math on the baked JSON —
  the Quran DBs are untouched (hard rule).
- Range data: rub *n* = `QURAN_DATA.rangeByIndex(RangeKind.HizbQuarter, n)`
  directly; hizb needs a small derivation helper next to `requireRangeIndex`
  (`reader-route-guards.ts`), returning the same `RangeEntry` shape so
  `loadRangeData`/`loadTranslationRangeData` work unmodified (both fetch by
  global index range; a hizb ≈ 104 ayahs is far under the 300-ayah range cap).
- Guard: `requireRangeIndex` (`reader-route-guards.ts:14-21`) extended with the
  `"hizb" | "rub"` kinds.
- `parseReaderPath` (`reader-route.ts:137`) + `parseReaderRoute` + the reader.ts
  canonical-shape validator (`lib/i18n/reader.ts:92-139`, add `hizb`/`rub` beside
  `page`/`juz` in the 2- and 5-segment cases) learn the new segments — RESERVED
  sets and `RESERVED_SURAH_SEGMENTS` (`reader.ts:15`) gain `hizb`, `rub`.
- UI wiring: sticky indicator shows hizb too (`Page 2 · Juz 1 · Hizb 1`, hizb =
  `ceil(quarterOf(global)/4)`); juz-index quarter grid
  (`app/juz/+page.svelte:89-102`) repoints from `pageHref(quarter.page)` to
  `rubPathFor(ctx, (juz.index-1)*4 + qi + 1)`; sidebar Juz tab stays juz-only
  (quran.com's sidebar has exactly 4 tabs — verified — so no fifth tab).
- Prerender: +300 shapes × 2 UI locales = +600 prerendered pages (Arabic),
  all bounded ints; translated hizb/rub join the SSR disk-cache family under the
  existing key shape (`cacheKind` extends to `"hizb" | "rub"` in
  `reader-route.ts` route types).

### 3.4 Sidebar Ayah tab everywhere (D7)

- Extract the selection logic into a pure helper (testable without DOM):
  `ayahTabSurah(quranData, params, readerCurrent)` → `CatalogEntry`:
  `params.surah` slug wins; else `quranData.surahByNum(readerCurrent)`; else
  surah 1. `Sidebar.svelte:235` uses it instead of the bare slug lookup;
  the empty-panel branch (`:278-279` `{#if current}` with no else) disappears.
- `selectBrowse` (`:54-63`) keeps its worker refresh for the params-surah case
  and falls back to `readerCurrent` (the field `reader.setCurrent` writes —
  `SurahPageRoute.svelte:160` already maintains it on every surah view).
- No new tab, no verse-input column yet (quran.com's two-column verse tab is a
  follow-up, out of this plan's scope); the existing sidebar search filters
  verses of the fallback surah as today.

### 3.5 Index pages preserve context (D8)

- In all three index pages, `arabicCtx` (`surah/+page.svelte:13`,
  `juz/+page.svelte:12`, `pages/+page.svelte:14`) becomes
  `const ctx = $derived(resumeCtxFor(reader.lastRead, { kind: "arabic" }))` —
  the existing `resumeCtxFor` helper (`quran.ts:112-115`); row hrefs go through
  the same `surahPathFor/juzPathFor/globalPagePathFor(ctx, …)` calls.
- SSR/prerendered HTML renders Arabic (store empty before hydration) → output is
  deterministic and stays byte-identical to the sitemap `<loc>` set; after
  hydration, rows for the reader's active source light up as translated links.
  This is the same device-state-after-hydration pattern the Continue card uses.
- No `/t/…/surah` index mirrors are added (D9 decision — the indexes stay
  Arabic-route surfaces; their ROWS carry ctx).

### 3.6 Sitemap + robots hygiene (D11, D12, D13)

`web/src/routes/sitemap.xml/+server.ts`:

- Delete `yield plainReaderEntryUrl("home")` (`:96`) — `/en/app` is noindex
  (`app/+page.svelte:107`, `Seo.svelte:154-157` emits no canonical on that
  branch), so the sitemap stops submitting it. The three browse indexes stay.
- Replace `alternatesBlock` (`:40-54`) with a static `ar` + `x-default` pair
  pointing at the (already-emitted) Arabic `<loc>`, or drop the block entirely —
  decide at implementation: keeping the two-line pair preserves a minimal
  "content language = Arabic" signal at zero cluster cost; dropping it is
  simplest. Default: **drop the whole block** (the `<loc>` already is the Arabic
  canonical; D12 keeps translations out either way).
- `readerPrerenderEntries`/`quranHrefForPrerenderEntry` remain the single source
  of truth feeding both the sitemap and SSG discovery — sitemap and prerender set
  cannot drift (hard constraint honored).
- Resulting inventory: 114 surah + 604 page + 30 juz + 60 hizb + 240 rub + 3
  browse indexes = 1051 reader/index `<loc>` (plus the unchanged marketing set,
  from 1307 total today), and 60,928 → 0 reader alternates.

### 3.7 Redirect hygiene (D14, D15)

`web/src/hooks.server.ts`:

- Numeric alias (D14): new branch before the `parseReaderPath`-gated 404. Segment
  test `/^[1-9]\d*$/` and `1 ≤ n ≤ 114` against the localized-de-Localized
  pathname; target = same prefix + `/app/{QURAN_DATA.surahByNum(n).slug}` +
  query/hash. `QURAN_DATA` is already in the hooks bundle graph via
  `$lib/server/reader-route.ts` (its `rangeRoute` uses it). `/app/115`+ still 404;
  digits never collide with `RESERVED_SURAH_SEGMENTS`.
- Legacy locale redirect (D15): `legacyReaderRedirect` (`:167-178`) →
  `status: 308`, `cache-control: public, max-age=86400` (target is
  deterministic `/en` today; if publication ever makes the target
  request-dependent, revert to 307 — noted as a comment at the branch).
- Both branches preserve query+hash like the existing 308
  (`noncanonicalLocalizedReaderRedirect`), minus the removed `/page/1`
  canonicalization branch (whole shapes are gone; its test cases move to the new
  redirect tests).

### 3.8 Docs that must move with the code

- `docs/quran-system.md` Part 1 "Web delivery": "Route families cover surah,
  surah-local page, global page, and juz" → surah, global page, juz, hizb, rub.
  Part 5 divergence list unchanged; add one line recording the surah-local
  scheme's removal (with redirect policy) so `my-plan-raw.md` readers find it.
- `web/README.md` routes comment: Arabic prerendered family list.
- This document's status header flips to "implemented" per milestone.

---

## 4. Milestones

Each milestone ships alone with `pnpm -C web check`, `pnpm -C web lint`,
`pnpm -C web test` green (message-key changes also run `pnpm i18n:check` first —
it gates the other three). No milestone mixes data, rendering, and navigation.

| # | Ships | IDs | Rough shape |
| --- | --- | --- | --- |
| M1 | Global page labels + sticky position indicator | D5, D1-labels | Store field, `PositionIndicator.svelte`, `ReaderHeader`/sr-only relabel to `globalPage`, shared `juzOf` helper, degraded-pill relabel prep. No URL, route, or sitemap change |
| M2 | Sitemap/robots hygiene | D11, D13 | Sitemap drops `/app` home + reader alternates block; sitemap tests updated. Zero reader-facing change |
| M3 | Redirect hygiene | D14, D15 | Numeric alias 308, legacy 307→308; hooks + `reader-route.test.ts`/hook tests. Zero canonical change |
| M4 | Sidebar + index context | D7, D8 | `ayahTabSurah` helper + Sidebar fallback; index ctx derivation. No URL change |
| M5 | **One URL per surah** | D1, D2, D3, D4, D10-tail, D16 | Route dirs removed, 308 builder + hook branch, `parseReaderPath` shapes dropped, helpers repurposed/deleted, canonical/title stable, scroll stops path rewrite, end-of-surah card, prerender entries −1096 pages, sitemap −548 locs, docs updated |
| M6 | Hizb + rub addressing | D6 | Helpers, 8 route dirs (4 shapes × arabic/translated × .md), guards/validator/parser, sticky hizb, juz-index grid repoint, prerender +600 pages |

Ordering rationale: M1–M4 are independent, low-risk, and each shrinks the M5 diff
(labels already global before routes move; sitemap correct before its inventory
changes; redirects land before shapes they must catch disappear). M5 is the risky
core and lands with its tests in one reviewable unit. M6 is additive and isolated.

---

## 5. Risks

- **SEO migration (M5).** 548 + 548-translated self-canonical URLs 308 into
  surah roots. Mitigations: 308 (permanent, method-preserving — same class the
  repo already uses for `/page/1`); fragment preserves spread-level deep links;
  sitemap regenerates from the same `readerPrerenderEntries` source so the
  declared index set and the served set cannot disagree; the al-baqarah
  relevance-dilution (48 canonicals) is the *point* of the migration — expect
  transient ranking churn on surah queries while consolidation propagates.
- **Prerender set changes.** M5 removes 1096 prerendered pages; M6 adds 600.
  Both flow through `readerPrerenderHrefs` uniqueness assert (duplicates fail the
  build). Arabic-only prerender policy untouched; translated routes never enter
  the prerender set (asserted by `reader-prerender-locales.test.ts`).
- **nav-guard test updates.** Deleting `surahLocalPagePathFor` must not weaken
  the guard: `ARABIC_ONLY_HELPERS` regexes stay (they then guard against
  reintroduction from a stale import), `expect(...).toBeInstanceOf(Function)`
  list gains `hizbPathFor`/`rubPathFor`. New `surahAyahPathFor` signature is
  exercised in the guard's fixture block (`nav-guard.test.ts:151-157` currently
  pins the `/page/7#ayah-30-12` shape — that expectation changes).
- **i18n copy.** New keys (en + ar, structurally identical): sticky position
  ("Page {page} · Juz {juz}"), end-card ("End of Surah {name}", "Read again"),
  surah-level SEO title/description. Removed keys: `reader_seo_surah_page_title`
  (`en.json:248`, `ar.json:248`), `reader_surah_page_title` (`:7`) once
  `copy.shell.pageOf` call sites are gone. Namespace map (`web/i18n-namespaces.json`)
  re-run via `gen-message-namespaces.ts`; unclaimed keys fail generation —
  deliberate.
- **History/restore regressions (M5).** `writeHistoryState` is load-bearing for
  scroll restore (`restoreHistory`, `:676-715`) and the translation switcher
  (`liveReaderPosition`). Changing its URL argument while keeping the snapshot
  contract needs the existing `__tests__` under `_reader/` run against scrolled
  sessions; `parseHistoryState` version bumps if the stored shape changes.
- **Cache TTL overlap (M5, translated).** Disk-cached translated `/page/N` HTML
  may be served up to 7 days after deploy while hooks already 308 the shape —
  stale-HTML-then-redirect on next navigation is self-healing but visible;
  acceptable, noted.
- **Sitemap shrink (M2).** Removing 60k alternates changes crawler-visible
  output massively in one deploy; ship M2 alone so any indexing delta is
  attributable.

---

## 6. Test plan

**Existing, updated:**

- `web/src/routes/(application)/app/__tests__/nav-guard.test.ts`
  - Keep `ARABIC_ONLY_HELPERS` + hand-built-`/app` guards unchanged in scope;
    they must keep passing against every touched component.
  - Export assertions gain `hizbPathFor`, `rubPathFor` (M6).
  - `surahAyahPathFor` fixture (`:151-157`) re-pinned to
    `/app/ar-rum/t/ms/basmeih#ayah-30-12` (M5).
  - New fixture: a component using `surahLocalPagePathFor` would fail the
    `ARABIC_ONLY_HELPERS` regex (deleted symbol cannot be imported) — add the
    literal-string false-positive checks mirroring `:78-82`.
- `web/src/lib/server/__tests__/quran-page-shape.test.ts` — drop
  `readingPreviousHref/readingNextHref` cases (`:28-67`); assert end-card links:
  prev null at Fatihah, next null at Nas, translated ctx preserved via
  `surahPathFor` (`:62-67` shapes become bare surah roots).
- `web/src/lib/server/__tests__/reader-route.test.ts` — `parseReaderPath` loses
  the two surah-local matchers; gains hizb/rub/numeric expectations; new pure
  `surahLocalRedirectTarget` cases.
- `web/src/lib/components/i18n/__tests__/reader-prerender-locales.test.ts` —
  entry-kind inventory: no `surah-local-page`; `hizb`/`rub` counts 60/240;
  uniqueness assert still holds; href count math updated (2600 → 1504 after M5,
  → 2104 after M6).
- `web/src/routes/sitemap.xml/__tests__/` — no `/en/app` loc; zero
  surah-local locs; zero `xhtml:link` inside reader `<url>` blocks; hizb/rub loc
  presence; counts asserted against `RANGE_COUNTS`-derived numbers, not literals.
- Hook tests (`localized-reader-hook.test.ts`, `server-headers.test.ts`) —
  numeric alias (`/app/2` and `/ar/app/2` → 308 slug, prefix preserved), legacy
  308 + cacheable headers, removed local-page shapes 308 with anchor.

**New unit tests (page-number mapping, the ask's explicit requirement):**

- `web/src/lib/server/__tests__/surah-local-redirect.test.ts` — exhaustive over
  the baked map: for every surah × local page 2..count, target anchor ayah equals
  `surahLocalPage(surah, localPage).startAyah`; local page 1 → bare root; count+1
  → null/404; translated shape round-trips lang/translator. Runs against
  `QURAN_DATA`, i.e. against the real 604-page map, no fixtures.
- `web/src/lib/data/__tests__/mushaf-divisions.test.ts` — hizb/rub derivation
  from the baked JSON: counts 60/240; hizb *i* spans quarters `4i-3..4i`; first
  verse of hizb 1 = `1:1`, last of hizb 60 = `114:6` (end of corpus); hizb/rub
  ranges tile 1..6236 with no gap/overlap (same invariant style as the boot-time
  range asserts); `ceil(quarterOf(global)/4)` sticky-helper mapping spot-checked
  at every quarter boundary.
- `web/src/routes/(application)/app/_reader/__tests__/` — extracted
  `ayahTabSurah` fallback (params present / absent / unknown slug / default 1)
  and the sticky-position derivation (globalPage+juz from a page payload).

**Gates per milestone:** `pnpm -C web check`, `pnpm -C web lint` (deny-warnings),
`pnpm -C web test`, plus `pnpm i18n:check` in M1/M5. A full
`just web-build prod` before M5 and M6 merge to eyeball the prerender/sitemap
delta (expected: −1096 pages/−548 locs, then +600 pages/+300 locs).

---

## 7. Non-goals

- **No reader rendering changes:** virtualization, typography, script variants,
  mushaf layout, bismillah/openers — untouched. The end-of-surah card is
  navigation chrome, not content rendering.
- **No DB changes:** Quran DBs immutable; hizb/rub derive from the baked
  metadata JSON (`web/static/quran-meta/quran-data.json`) only. No SHA-256 over
  Quran data anywhere (identity stays id-based; the existing baked
  `source` digest field is untouched metadata, validated as today).
- **No translation-model change (D9):** translated URL trees, SSR + 7-day disk
  cache, and the translation catalogue are fixed inputs to this plan.
- **No new UI locale, no marketing-site changes, no search behavior changes**
  (search result hrefs update mechanically with `surahAyahPathFor` only).
- **No quran.com features beyond navigation:** tafsir CTA, reading plans,
  audio, reciters, per-verse sub-resource routes — all out of scope.
- **No word-level navigation** (already deferred in docs/quran-system.md Part 6).

---

## 8. Open questions (decisions not made here)

1. **Q1 — hizb/rub prerender budget (D6/M6).** +600 prerendered pages is within
   the existing bounded pattern but is the only milestone that *grows* the build.
   If the owner prefers the minimal move: ship sticky-bar hizb + juz-index grid
   repoint to global pages first, defer the routes. Default in this plan: full
   routes.
2. **Q2 — redirect anchor.** 308 Location carrying `#ayah-…` preserves spread
   position but puts fragments in redirect chains; the conservative alternative
   is a bare surah-root target. Default: anchor included, drop if any crawler
   weirdness shows in logs.
3. **Q3 — scroll position in the URL (D2).** This plan writes `?v={s}:{a}` on
   settled scroll (quran.com's `?startingVerse` parity). The zero-URL-noise
   alternative keeps position in `history.state` only, losing share-current-
   position. Default: query param, path never changes.
4. **Q4 — D15 cache window.** `max-age=86400` on the legacy 308 is a guess for a
   deterministic redirect; any value ≥ 0 is correct, larger trades flexibility
   for edge-hit rate.
5. **Q5 — reader alternates (D13).** "Drop the whole block" vs "keep `ar` +
   `x-default`". Default: drop; both are one-line deltas if the owner wants the
   signal.
