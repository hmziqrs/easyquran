# EasyQuran — Quran content delivery to the web (design plan)

> Status: **plan** (not yet implemented). Scope: deliver the **Arabic (Uthmani) text** of all 114
> surahs to the web reader — **server-rendered for SEO, hydrated to an SPA, cached for instant
> repeat visits** — with **no runtime backend in the read path**. Translations, tafsir, and
> full-corpus search are designed-but-deferred extensions that slot in later without rework.
>
> Aligns with the product goals in `ROADMAP.md` and with the verified current state of the repo.

---

## 0. Relationship to `docs/quran-api.md`

`quran-api.md` specifies a backend (Axum `quran_v1`) content API. **This plan supersedes it for
content *reads*.** The reasoning is in §1, but in one line: the Qur'an is immutable and the web
deploy is already fully prerendered (`@sveltejs/adapter-static`), so reads are delivered as
build-time static artifacts — faster, cheaper, cacheable forever, and requiring no server at
runtime.

`quran-api.md` is **retained** as the design for the two things that genuinely need a server
**later**: **full-corpus FTS search** (its §9) and **accounts / bookmark sync**. Those slot in
behind `PUBLIC_API_BASE_URL` (Phase 5) and never enter the content-read path.

**Reused verbatim from `quran-api.md`** (they are source-of-truth facts, not API-specific):

- Source-data inventory (its §2) and the **global-index invariant** (`quran_text."index"` is the
  contiguous 1..6236 global ayah id).
- The **bismillah invariant** (its §6): Tanzil embeds the basmala as a byte-identical prefix on
  ayah-1 of 112 surahs; surah 1 ayah 1 *is* the basmala; surah 9 has none.
- The **slug table** decision (its §4): ship a committed 114-entry `num→slug` map; the XML `tname`
  (`Al-Baqara`) cannot be normalized to the already-shipped slugs (`al-baqarah`).
- The **Tanzil license note** (non-commercial; tanzil.net attribution). Revisit the moment billing
  / accounts go live.

---

## 1. Goals & the architectural decision

### Goals (what the reader must deliver)

1. **SEO / server-rendered.** Every `/app/<slug>` page ships as real HTML with the Uthmani verse
   text in the DOM — indexable by Google/Bing/social/AI crawlers without relying on their JS
   rendering queue, and best-possible LCP / Core Web Vitals.
2. **SPA-fast after hydration.** Navigating between surahs is client-side, no full page reload,
   prefetched on hover.
3. **Instant repeat visits.** Second load and onward serve from a local cache — works offline.
4. **No third-party WASM.** Native web tech only (no `sql.js`, no SQLite-in-browser).
5. **Contract preserved.** The `Surah` / `VerseKey` interface and the pure helpers in
   `web/src/lib/data/quran.ts` stay; the `_reader` components (`SurahReader`, `VerseRow`,
   `Sidebar`, `Results`) are untouched.

### The decision, and why (verified against the repo)

The conversation weighed three options — backend-API-first, browser SQLite via WASM, and
build-time static JSON — and landed on a **hybrid static-delivery** model. The verified facts that
drive it:

| Verified fact | Consequence |
|---|---|
| `quran-data.xml` is **metadata only** (114 sura rows + juz/page/ruku/sajda; **zero** verse text, no `text=` attribute). The Uthmani text lives in `db/quran/tanzil/arabic/quran-uthmani.sqlite` (1.6 MB, 6236 rows). | The build reads **both**: XML for the catalog, the `.sqlite` for verse text. |
| The web **hardcodes 11 surahs** in `quran.ts` and **never fetches the backend** (`grep fetch(` → no content calls; `PUBLIC_API_BASE_URL` is documented "device-registration only"). | There is no live content path to disrupt; the cutover is a clean swap behind the seam. |
| `rust/data/easyquran.db` is a **ruxlog CMS clone** (users/posts/media/billing…); **no surah/verse tables**. The Axum router exposes **zero** Qur'an routes. | The backend is built for stateful *writes*, not content reads. Immutable content doesn't need a runtime API in front of it. |
| Deploy is `adapter-static` with `prerender = true` on `[surah]/+page.ts`. | SSG already happens at build time — extending it from 11 to 114 surahs needs no infra change. |
| `db/quran/tanzil/translations/` is a workspace package whose `scripts/lib.ts` already parses Tanzil SQL and writes into `web/` (`DEFAULT_WEB_TARGET`). Node is **v24** → built-in `node:sqlite`. | The build pipeline reuses existing tooling and reads the `.sqlite` with **zero new dependencies**. |

**Why not serve reads from the Axum backend?** A runtime API in front of a 1.6 MB canonical text
that never changes is pure overhead — a migration, SeaORM entities, a seed binary, a build-time
backend dependency, CORS/rate-limit wiring — to mediate immutable content. The clean split is:
**static reads on the CDN; dynamic writes on Axum.**

**Why not browser SQLite via WASM (`sql.js`)?** It is a database engine for a workload that is
really just key/index lookups; it adds a ~1 MB WASM tax on every visitor and a blocking first
download before first paint, and it cannot produce the server-rendered HTML that SEO requires (so
the build-time read exists regardless). There is also **no native SQL database in browsers anymore**
— `Web SQL` was deprecated in 2010 and removed by ~2023; `IndexedDB` (NoSQL) is the native option,
and it covers everything a reader needs without WASM.

---

## 2. Source data (verified on disk)

| Asset | Path | Shape |
|---|---|---|
| Arabic — Uthmani | `db/quran/tanzil/arabic/quran-uthmani.sqlite` (1.6 MB) | `quran_text("index" PK, sura, aya, text)`, idx on `(sura,aya)`, 6236 rows |
| Metadata | `db/quran/tanzil/quran-data.xml` (77 KB) | `<suras>`(114) `<juzs>`(30) `<pages>`(604) `<rukus>`(556) `<sajdas>`(15) |
| Translations (future) | `db/quran/tanzil/translations/sql/*.sql` (115 dumps) | `index, sura, aya, text` each; catalog in `translations/index.json` |

Surah text query is a single indexed seek: `SELECT text FROM quran_text WHERE sura=? ORDER BY aya`.

> **`lib.ts` caveat.** `db/quran/tanzil/translations/scripts/lib.ts::parseSql` only **counts** rows
> (`ayaCount`) and discards verse text — confirmed by reading the source. It is fine for catalog
> building but **must not be used to extract verse text**. The Arabic build reads the `.sqlite`
> directly via `node:sqlite` (Phase 4 translations will need a dedicated text-extracting parser).

---

## 3. Architecture — four native layers, zero WASM

```
BUILD TIME
  quran-uthmani.sqlite ─┐
  quran-data.xml ───────┴──► build-quran.mjs (node:sqlite, zero deps)
        │                       │  strips bismillah prefix (§6 of quran-api.md)
        │                       │  maps num→slug from committed slug table
        │                       ▼
        │            ┌──────────────────────────────────────────┐
        │            │ web/src/lib/data/surahs.generated.ts      │  114-entry catalog (client)
        │            │ web/src/lib/server/quran-verses.generated │  full verse map (server-only)
        │            │ web/static/quran/ar/{1..114}.json         │  per-surah chunks (CDN)
        │            │ web/static/quran/search.json              │  search corpus (Phase 4)
        │            └──────────────────────────────────────────┘
        ▼
  SvelteKit prerender (adapter-static)
        │  [surah]/+page.server.ts load() reads the server-only verse map
        ▼
  114 × static HTML (verses baked into DOM → SEO)  +  __data.json per page

DEPLOY   →  static HTML + JSON on Cloudflare CDN

RUNTIME
  first visit   = CDN serves prerendered HTML → hydrate → SPA router takes over
  client nav    = SvelteKit fetches the target page's prerendered __data.json
  repeat visit  = Service Worker serves cached HTML + __data.json (instant, offline)
  user data     = IndexedDB (bookmarks / notes / last-read) — Phase 4
  offline search= IndexedDB over the search corpus — Phase 4
```

| Layer | Native technology | Job |
|---|---|---|
| Delivery | Static JSON + prerendered HTML on the CDN | get bytes to the browser |
| SSR / SEO | SvelteKit **prerender** (build-time server render) | verses in the DOM for crawlers + first paint |
| Offline cache | **Service Worker** (cache-first) | persisted, offline, never expires |
| Query + user data | **IndexedDB** (native, NoSQL) | bookmarks/notes + structured offline search |

> **No `sql.js`, no WASM, no runtime backend for reads.** The Service Worker already delivers
> "cached + offline + no management" for content; IndexedDB layers on top for *structured* access
> (user data and search), not as a substitute for the SW.

---

## 4. The build pipeline — `web/scripts/build-quran.mjs`

A Node script, no native deps (uses the built-in `node:sqlite`). Runs before `vite build` via a
`prebuild` hook. Lives under `web/scripts/` (workspace sibling of the translations package; it may
import shared helpers from `db/quran/tanzil/translations/scripts/lib.ts` such as `decodeEntities`,
but **not** `parseSql`).

Inputs: `quran-uthmani.sqlite` + `quran-data.xml` + the committed slug table (`slugs.mjs`).
Outputs:

- `web/src/lib/data/surahs.generated.ts` — `SURAHS_META: {num, slug, name, arabic, place,
  ayahCount}[]` (~3–8 KB gzipped; **ships to client**).
- `web/src/lib/server/quran-verses.generated.ts` — `VERSES: Record<number, string[]>` (the full
  6236 verses; **server-only** via `$lib/server`, tree-shaken from the client bundle).
- `web/static/quran/ar/{1..114}.json` — `{num, verses:[...]}` per surah (clean, stable,
  cross-platform asset; used by cross-surah bookmark text in §6 and native apps later).
- `web/static/quran/search.json` — 6236 `{k, s, a, t}` rows (emitted now, **consumed in Phase 4**).

Skeleton:

```js
// web/scripts/build-quran.mjs
import { DatabaseSync } from "node:sqlite";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { SLUGS } from "./slugs.mjs";                       // 114-entry num→slug map

const BISMILLAH_PREFIX = "بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ ";   // see §6 of quran-api.md
const db = new DatabaseSync("db/quran/tanzil/arabic/quran-uthmani.sqlite", { readOnly: true });
const suras = parseSuras(readFileSync("db/quran/tanzil/quran-data.xml", "utf8")); // 114 <sura> rows

const catalog = [], versesMap = {}, search = [];
for (const s of suras) {
  const rows = db.prepare("SELECT aya, text FROM quran_text WHERE sura=? ORDER BY aya").all(s.index);
  const verses = rows.map(({ aya, text }, i) => {
    // strip the embedded basmala prefix on ayah-1 for the 112 surahs that carry it
    if (aya === 1 && s.index !== 1 && s.index !== 9 && text.startsWith(BISMILLAH_PREFIX))
      return text.slice(BISMILLAH_PREFIX.length);
    return text;
  });
  versesMap[s.index] = verses;
  writeJson(`web/static/quran/ar/${s.index}.json`, { num: s.index, verses });
  verses.forEach((t, i) => search.push({ k: `${s.index}:${i + 1}`, s: s.index, a: i + 1, t }));
  catalog.push({ num: s.index, slug: SLUGS[s.index], name: s.tname, arabic: s.name,
                 place: s.type === "Meccan" ? "Meccan" : "Medinan", ayahCount: s.ayas });
}
writeFileSync("web/src/lib/data/surahs.generated.ts", emitCatalog(catalog));
writeFileSync("web/src/lib/server/quran-verses.generated.ts", emitServerMap(versesMap));
writeJson("web/static/quran/search.json", search);
// one-time assertion: every surah except 1 and 9 had the prefix on ayah 1 (see §6)
```

**Bismillah handling** is done **at build time** (in the generator), reusing the verified invariant
from `quran-api.md` §6 — so the web's existing `showsBismillah` / header logic is unchanged and the
basmala is never doubled.

**Slugs.** Ship `web/scripts/slugs.mjs` — a committed 114-entry `num→slug` map. The 11 already-live
slugs (`al-fatihah`, `al-baqarah`, `al-asr`, `al-fil`, `quraysh`, `al-kawthar`, `al-kafirun`,
`an-nasr`, `al-ikhlas`, `al-falaq`, `an-nas`) are a verbatim subset; the other 103 are authored
once and frozen. This is load-bearing for routing + SEO (Open question 1).

---

## 5. The seam — `web/src/lib/data/quran.ts` (contract preserved)

`quran.ts` stays the contract + pure helpers. Internal changes only:

- `SURAHS` becomes `SURAHS_META.map(m => ({ ...m, verses: [] }))` — synchronous metadata, no verse
  text. `Sidebar`, `adjacentSurahs`, `surahMeta`, and the reader-store name lookups keep working
  off metadata alone.
- Add `ayahCount: number` to the `Surah` interface (additive); `surahMeta` reads `s.ayahCount`
  instead of `s.verses.length`.
- **Untouched:** `verseKey`, `parseKey`, `toArabicDigits`, `BISMILLAH`, `showsBismillah`,
  `surahPath`, `slugFor`, `verseOfTheDay`.
- `searchVerses` keeps its instant metadata-only (name/number) behavior now; the full-corpus async
  search arrives in Phase 4.

The route supplies verse text through SvelteKit's data flow — see §6. The `_reader` components
read `surah` as a prop of the same `Surah` shape, so **`SurahReader.svelte`, `VerseRow.svelte`,
`Sidebar.svelte`, `Results.svelte` are not edited.**

---

## 6. SSR / SPA / SEO — the route cutover

**`web/src/routes/(application)/app/[surah]/+page.server.ts`** (replaces the current `+page.ts`
shell — `prerender` + `entries` move here, and a `load` is added):

```ts
import { SURAHS_META } from "$lib/data/surahs.generated";
import { VERSES } from "$lib/server/quran-verses.generated";   // server-only; not in client bundle
import { adjacentSurahs } from "$lib/data/quran";

export const prerender = true;
export const entries = () => SURAHS_META.map((s) => ({ surah: s.slug }));

export function load({ params }) {
  const meta = SURAHS_META.find((s) => s.slug === params.surah);
  if (!meta) throw error(404, "Surah not found");
  return { surah: { ...meta, verses: VERSES[meta.num] }, ...adjacentSurahs(meta.num) };
}
```

- **At build (prerender):** `load` runs on the server, reads the server-only verse map, and the
  verses are baked into the static HTML (SEO) **and** the page's `__data.json`.
- **Client navigation:** SvelteKit fetches the prerendered `__data.json` for the target surah — no
  `load` re-run, no separate fetch to write. The Service Worker caches it (§7) → instant + offline.
- **`[surah]/+page.svelte`:** one line changes — `const surah = $derived(data.surah)` instead of
  `surahBySlug(slug)`. Everything below it (the `SurahReader` prop, the `?verse=N` scroll effect,
  the `reader.setCurrent` sync) is unchanged.

**Cross-surah verse text (bookmarks/copy in other surahs).** The reader store's `verseText(key)`
currently reads `surahByNum(num).verses[n-1]`, which is now empty for non-current surahs. Add a
tiny module-level verse cache (`web/src/lib/data/verse-cache.ts`) that the page warms with the
current surah's verses; for a bookmark in *another* surah, lazily `fetch(`/quran/ar/${num}.json`)`
(the clean chunks from §4) and cache it. This is the only place the seam needs a cache; the current
surah always works, others fetch on demand.

**SEO metadata.** Per-page `<title>`/description, JSON-LD (`QuranChapter`/`Quotation`), and
`<link rel="canonical">` via the existing `<Seo>` component; drop any `noindex` on surah pages; add
the 114 `/app/<slug>` entries to the sitemap; enable `precompress` in `svelte.config.js`.

**SPA snappiness.** Add `data-sveltekit-preload-data="hover"` on surah links so the next page's
`__data.json` is prefetched before click.

---

## 7. Offline — Service Worker

`web/src/service-worker.ts` (the SvelteKit-compiled ESM worker that `lib/firebase/messaging.ts`
already references but does not yet exist). Runtime caching:

- **Cache-first:** `/quran/**`, the prerendered `/app/<slug>` HTML, and their `__data.json`.
- **Stale-while-revalidate:** the app shell (HTML/JS/CSS/fonts).
- Precache the build manifest on install.

This is what delivers the FAQ promise ("surahs you've opened stay available offline") and the
"super-fast second load" — the SW serves the cached HTML + data from disk.

**Two workers at root scope.** The new content SW coexists with the classic
`web/static/firebase-messaging-sw.js`. Firebase owns its own registration, so the content SW
registration (in `+layout.svelte` `onMount`, browser-only, idempotent) must use a **separately
versioned cache name** so clearing one never nukes the other's state. Verify
`navigator.serviceWorker.getRegistrations()` shows two after a hard reload (Open question 4).

---

## 8. IndexedDB (Phase 4) — native, no WASM

`IndexedDB` is the native browser database (NoSQL object store). There is **no native SQL database
in browsers** — `Web SQL` was deprecated (2010) and removed (~2023); every in-browser SQL option
today is WASM. IndexedDB covers everything a reader needs (key/index lookups) without it. Two uses:

1. **User data** — migrate the reader store's persistence off `localStorage`
   (`easyquran.reader`: current, fontSize, mode, bookmarks, notes, lastRead). `localStorage` is
   synchronous, ~5 MB capped, strings-only; IndexedDB is async, far larger, stores objects
   directly. `hydrate()` already runs after mount, so the async read is a natural fit.
2. **Offline search** — load `search.json` (§4) into an IndexedDB object store indexed by `s`
   (surah); scan/fold for Arabic substring matching client-side, no server. (No built-in FTS, so it
   is exact-token/diacritic-folded matching — the same limitation `quran-api.md` §9 notes; backend
   FTS5 remains the Phase-5 upgrade for ranked search.)

> IndexedDB is **not** the content cache — the Service Worker already does that. It is the
> *structured-query* layer on top, justified by user data + search.

---

## 9. Translations, tafsir, native apps (future, Phase 4+)

- **Translations** — same chunk pattern: `web/static/quran/trans/{id}/{1..114}.json`. The catalog
  (`web/src/lib/data/translations.json`) is already generated by the translations package. A
  dedicated text-extracting SQL parser is required (`lib.ts::parseSql` discards text — §2).
  Default set ships in-repo; the long tail goes to R2 via the existing `upload.ts`.
- **Tafsir** — `web/static/quran/tafsir/{id}/{n}.json`, same loader; the `tafsirFor` stub becomes a
  lookup.
- **Native apps** — consume the same `/quran/ar/{n}.json` chunks (or a thin backend
  `/quran/v1/surah/{n}` from `quran-api.md`); the chunk filenames/shapes become the stable
  cross-platform contract.

---

## 10. Phased plan

Each phase ships independently; nothing breaks the current 11-surah site until Phase 2 flips the
source.

**Phase 0 — Slug table (no code behavior change).**
Author `web/scripts/slugs.mjs` (114 entries; the 11 live slugs verbatim, 103 derived + frozen).
*Exit:* the canonical slug scheme is decided and committed.

**Phase 1 — Build pipeline + catalog (no behavior change).**
`web/scripts/build-quran.mjs` (+ `slugs.mjs`); emits `surahs.generated.ts`, the server-only verse
map, `/quran/ar/{1..114}.json`, and `search.json`. Add `"quran:build": "node scripts/build-quran.mjs"`
and a `prebuild` hook to `web/package.json`. `quran.ts` still exports the hardcoded `SURAHS`, so the
site is unchanged.
*Exit:* `pnpm quran:build` regenerates all artifacts; spot-check a few surahs' verse counts + the
bismillah-strip assertion.

**Phase 2 — Cutover (the keystone).**
Edit `quran.ts` internals (§5); add `verse-cache.ts`; replace `[surah]/+page.ts` with
`+page.server.ts` (§6); change one line in `+page.svelte` to `data.surah`.
*Exit:* `vite build` emits 114 prerendered `/app/<slug>` pages each with full Uthmani text in the
DOM; `grep` the client bundle confirms no verse strings leaked; reader navigation works.

**Phase 3 — SEO + Service Worker + SPA prefetch.**
Per-page SEO metadata + JSON-LD + sitemap + `precompress`; `service-worker.ts` + registration in
`+layout.svelte`; `data-sveltekit-preload-data="hover"`.
*Exit:* DevTools → Offline, navigate between two opened surahs — all work; Lighthouse SEO pass.

**Phase 4 — IndexedDB (user data + offline search).**
Migrate reader persistence to IndexedDB; load `search.json` into a store; `Results.svelte` goes
async with loading/empty states (the one `_reader` component change). Translations/tafsir chunks.
*Exit:* bookmarks survive a cold session; offline search returns hits across all 114 surahs.

**Phase 5 (later, behind `PUBLIC_API_BASE_URL`) — backend search + accounts.**
Implement the `quran_v1` module from `quran-api.md` for **FTS5 search only** + bookmark sync.
Content chunks stay static; the backend never enters the read path.

---

## 11. Open questions / decisions

1. **Slugs** — confirm the transliteration rule for the 103 new slugs (the 11 live ones are frozen).
   Load-bearing for routing + SEO.
2. **Commit or gitignore generated artifacts?** Lean: **commit** (`web/static/quran/**`,
   `surahs.generated.ts`) — the data is canonical/immutable (changes ~once a decade) and committed
   artifacts let Cloudflare Pages build without the `prebuild` step. Trade-off: ~1.5 MB of JSON in
   the repo.
3. **Server-only verse map vs. per-surah chunks for prerender** — recommended: server-only map
   (`$lib/server`) feeds `+page.server.ts` at build; chunks feed cross-surah bookmark text + native
   apps. (Confirm `adapter-static` ships no server bundle so the map never reaches the client.)
4. **Two service workers** — idempotent content-SW registration + separately-versioned caches so it
   never disturbs `firebase-messaging-sw.js`.
5. **IndexedDB timing** — now (Phase 4) vs. keep `localStorage` until accounts/sync ship. The
   content path (Phases 1–3) does not depend on it.
6. **Verse-of-the-day** — deterministic by UTC date (globally consistent, cacheable), chosen at
   build or client-side after mount.
7. **License** — Tanzil is non-commercial; revisit the moment billing/accounts go live
   (`quran-api.md` license note). Render a tanzil.net attribution in the reader footer.
