# EasyQuran — SSG reader optimization plan

> Status: **planned, not implemented**.
>
> Priority: minimize the HTML and critical data needed for first paint. Quran
> text in the requested Surah-local page is useful content; reader controls,
> duplicate render trees, and corpus-wide metadata are not.
>
> This plan supersedes the full-Surah SSG and web-build XML decisions in
> [`quran-web-delivery.md`](./quran-web-delivery.md) only after its acceptance
> checks pass. It does not change the Rust API or the two offline Arabic
> databases.

---

## 1. Outcomes and source rule

The migration has two independent outcomes:

1. **A bounded, content-first SSG document.** A Surah URL renders exactly one
   Mushaf-aligned Surah-local page with one copy of each ayah and no server-
   rendered reader controls or corpus-wide navigation data.
2. **A compact, immutable metadata boundary.** The web build and SPA stop
   parsing `quran-data.xml`. Two checked-in positional JSON snapshots and
   TypeScript accessors replace the current XML virtual module,
   `surah-names.json`, and `quran-coordinates.json`.

Changing XML to compact JSON does not by itself reduce HTML, and reducing HTML
does not require deleting the XML.

`db/quran/tanzil/quran-data.xml` remains tracked, unchanged, and available to
the Rust backend and as the provenance source. **Do not delete it.** Neither
web SSG nor browser code may read, parse, watch, copy, or bundle it after this
migration.

The compact JSON conversion is a one-time migration. A disposable converter
may produce and verify the snapshots, but no converter or data-maintenance
command remains in the repository afterward.

---

## 2. Page coordinate model

The word “page” has two meanings. Code, types, routes, and docs must always
name which coordinate system is being used.

### 2.1 Global Mushaf page

A global page is one of the 604 canonical Mushaf pages. Its identity is
`globalPage`, in `1..604`.

- Global page 1 contains Al-Fatihah.
- Global page 2 starts Al-Baqarah.
- A global page may contain the end of one Surah and the start of one or more
  later Surahs.

The global route `/app/page/2` renders global page 2 as a whole. It does not
SSG-render global pages 1 or 3.

### 2.2 Surah-local page

A Surah-local page is the non-empty intersection of one Surah's ayah range and
one global Mushaf page's ayah range. Those intersections are renumbered from
`1` inside each Surah:

```ts
interface SurahLocalPageId {
  surah: number;
  localPage: number;
  globalPage: number;
}
```

Al-Baqarah's first Surah-local page is local page 1 even though it lies on
global page 2. If a global page contains multiple Surahs, it legitimately maps
to multiple clipped Surah-local pages. A Surah-local page never includes ayahs
from another Surah.

The current corpus produces 662 Surah-local pages. Fifty-one global pages cross
a Surah boundary, and one global page can contain as many as three Surahs.
Those cases are required fixtures, not edge cases to flatten away.

### 2.3 Surah SSG page

A Surah becomes one continuous scroll after the client starts, but its initial
SSG document contains exactly the requested Surah-local page. It does not
contain the previous or next page.

For Surah `s` and requested local page `p`:

```text
SSG(s, p) = { p }
```

When the URL has no local page number, `p` is always 1. This rule is identical
for a one-page Surah, the first page, an interior page, and the last page.

After first paint, the client may add another page when the reader actually
approaches a scroll boundary. It reads the offline database first. If the
offline data is unavailable, a future range API will be the fallback. Merely
opening or hydrating a Surah page must not preload either neighbor.

There is no wraparound and no client-loaded page may cross into an adjacent
Surah.

---

## 3. Canonical URLs and crawlability

Static generation cannot emit different HTML for query-string variants. The
local page belongs in the path:

| Intent | Canonical URL | SSG content |
|---|---|---|
| Surah local page 1 | `/app/<surah-slug>` | local page 1 only |
| Surah local page `p > 1` | `/app/<surah-slug>/page/<p>` | local page `p` only |
| global Mushaf page `g` | `/app/page/<g>` | global page `g` only |

`/app/<surah-slug>/page/1` is not emitted or added to the sitemap. If hosting
accepts it, it permanently redirects to `/app/<surah-slug>`.

Ayah links resolve to the URL for the ayah's Surah-local page plus a stable
fragment, for example:

```text
/app/al-baqarah/page/4#ayah-2-24
```

The fragment identifies the ayah, not the paginated document. The path already
selects the one SSG page that contains it.

Every emitted local page has:

- a self-referencing canonical URL;
- a title and description identifying the Surah and current local page;
- real `<a href>` links making the local sequence crawlable;
- an entry in the generated sitemap.

Adjacent Surah-local URLs do not overlap: each owns one unique clipped page.
Global-page and Juz views still present some of the same Quran text for their
different reading intents.

This follows Google's current guidance that paginated documents have distinct
URLs, self-canonicals, and sequential links:
[Pagination best practices](https://developers.google.com/search/docs/specialty/ecommerce/pagination-and-incremental-page-loading).

Adding the 548 non-first local paths increases generated file count and repeats
the page shell, but it does not repeat ayahs between Surah-local documents.
Across the Surah route family, each ayah is emitted exactly once. Measure both
the per-document result and aggregate deploy size because the repeated shell
and extra files still have a cost.

### Juz routes

`/app/juz/<n>` currently renders a complete Juz and is in the sitemap. The
Surah-local and global-page rules do not implicitly define Juz behavior.
Before this plan is complete, choose and document one of:

1. keep a full-Juz SSG document while applying the single-text DOM and client-
   only control rules;
2. introduce explicit Juz-local page paths; or
3. make a Juz URL a navigation alias to its first global page and starting
   ayah.

The implementation must not silently treat a Juz number as either a global or
Surah-local page number. Existing Juz behavior stays unchanged and is measured
separately until that decision is made.

---

## 4. Current production baseline

The August 2026 production-build audit found 748 generated reader documents:
114 Surah routes, 604 global-page routes, and 30 Juz routes.

| Current cost | Finding | Target |
|---|---|---|
| Verse actions | Four tooltip buttons and four inline SVGs are SSR-rendered per ayah; 74,832 buttons occupy about 68.8 MB of raw generated HTML across the route families. | No action button, tooltip, textarea, or action SVG in SSG HTML. |
| Duplicate modes | Both tab panels render, so Arabic text appears in Ayah-by-Ayah and Reading trees. For Al-Baqarah the hidden copy is about 126 KB raw. | One ayah node/text; CSS changes presentation. |
| Hydration data | SvelteKit serializes loaded Quran data after rendering it as HTML. Al-Baqarah repeats about 108.6 KB raw in page data. | First bound it to the requested local page; if the measured budget still fails, isolate static content from hydration. |
| Whole-Surah load | `/app/[surah]` calls `readSurahText` and returns the complete `LoadedSurah.verses`. | Read and return only the requested local page. |
| Corpus metadata | The virtual module compiles XML, names, coordinates, range families, and sajdas. The emitted metadata chunk is about 121.7 KB raw / 13.4 KB Brotli. | Embed only route metadata; fetch compact snapshots after paint or on demand. |
| Reader JavaScript | A reader page preloads 26 JavaScript files, about 525.7 KB raw / 134.7 KB Brotli. | Dynamically import client controls and sidebar. |

The closed sidebar's 114 menu items are **not** currently in SSG HTML. Its cost
is the eager component/code and metadata dependency. Keep the sidebar absent
from HTML and remove it from the critical JavaScript graph.

Closed drawers, settings panels, notifications, search results, notes, tafsir,
and the continue-reading banner are already absent from static HTML. Do not
spend this migration “removing” markup that the build does not contain.

Small shell cleanup—repeated inline font-size styles, redundant in-app CTA,
theme-script comments, and favicon markup—comes after structural work.

---

## 5. Two immutable compact JSON snapshots

The plan uses these names:

```text
web/static/quran-meta/v1/quran-catalog.json
web/static/quran-meta/v1/quran-navigation.json
```

There are exactly two tracked web metadata JSON files after migration. They
absorb and replace:

```text
web/src/lib/data/surah-names.json
web/src/lib/data/quran-coordinates.json
```

The `/quran-meta/` path avoids the existing `/_quran/` service-worker and local
SQLite artifact behavior.

### 5.1 Catalog snapshot

`quran-catalog.json` has a schema/provenance header and 114 positional Surah
rows. Surah number is `row index + 1`. A row retains every non-derived XML
Surah value and carries forward the app's stable slug/display name before
`surah-names.json` is removed.

Conceptual shape:

```json
[
  1,
  ["sha256-of-quran-data.xml", "source-license"],
  [
    ["al-fatihah", "Al-Fatihah", "الفاتحة", "Al-Faatiha", "The Opening", 0, 7, 5, 1]
  ]
]
```

The example is explanatory, not hand-authored source. Exact XML spellings and
route slugs are copied by the disposable converter and proven against the
current catalog before removal.

```ts
export const CatalogRoot = {
  Version: 0,
  Source: 1,
  Surahs: 2,
} as const;

export const SurahField = {
  Slug: 0,
  DisplayName: 1,
  ArabicName: 2,
  TanzilTransliteration: 3,
  EnglishMeaning: 4,
  PlaceCode: 5,
  AyahCount: 6,
  RevelationOrder: 7,
  RukuCount: 8,
} as const;
```

Global starts/ends come from the prefix sum of `AyahCount`. Surah number,
opener kind, and bismillah packaging are also derived rather than repeated.

### 5.2 Navigation snapshot

`quran-navigation.json` contains compact numeric series for page, Juz, ruku,
hizb-quarter, and manzil starts, plus sajda markers. Range ends,
`first`/`last` VerseKeys, and Surah-local intersections are derived.

```ts
export const RangeKind = {
  Page: 0,
  Juz: 1,
  Ruku: 2,
  HizbQuarter: 3,
  Manzil: 4,
} as const;

export const SajdaField = {
  Surah: 0,
  Ayah: 1,
  KindCode: 2,
} as const;
```

Each range series stores its first 1-based global start followed by positive
deltas. Range index is `array index + 1`. Sajda rows need only Surah, ayah, and
a kind code. In a measured candidate encoding, the two snapshots together are
about 12.5 KB raw / 4.1 KB Brotli while retaining more Surah metadata than the
current compiled catalog.

### 5.3 Accessors are the public boundary

No route, component, store, or worker indexes raw arrays directly. Accessors
return named objects:

```ts
surahByNum(number)
surahBySlug(slug)
verseKeyAtGlobal(globalIndex)
globalIndexOf(surah, ayah)
rangeByIndex(kind, index)
globalPageRange(globalPage)
surahLocalPageCount(surah)
surahLocalPage(surah, localPage)
previousSurahLocalPage(surah, localPage)
nextSurahLocalPage(surah, localPage)
sajdaAt(surah, ayah)
```

The previous/next accessors identify a possible client scroll request. SSG does
not call them to load Quran text.

Invalid coordinates return `undefined` or throw explicitly; they do not
silently fall back to Al-Fatihah.

Provide two adapters over the same contracts:

- a synchronous server-only loader for prerendering; and
- an asynchronous browser loader fetching snapshots after first paint or when
  sidebar/navigation is opened.

Compact arrays never enter Svelte page data. The SSG loader selects only values
needed by the route. A bundle assertion verifies that rendering a page does not
pull all range families into critical client JavaScript; two JSON files alone
do not guarantee tree shaking.

### 5.4 One-time conversion and provenance

The migration performs this sequence once:

1. read the unchanged XML, current names, and current coordinates;
2. emit the two compact snapshots;
3. compare every expanded catalog/range/sajda entry with current output;
4. record XML SHA-256, source metadata, and snapshot schema version;
5. commit the snapshots; and
6. delete the disposable converter and superseded JSON inputs.

The web build does not compare against or hash XML afterward. JSON-only tests
validate structure and immutable corpus totals. Replacing XML would be a new,
explicit data migration with new snapshots/provenance—not a recurring command.

The Rust backend may continue hashing/parsing XML for its own integrity
anchor, the pinned sha256 digest surfaced on `/health/ready` as
`sourceDigests` (there is no corpus `contentVersion`). The client metadata
cache has its own schema and does not assume a backend digest invalidates a
web snapshot.

---

## 6. Target SSG data flow

### 6.1 Surah routes

Generate 662 canonical Surah-local documents:

- 114 local-page-1 documents use `/app/<slug>`;
- 548 later documents use `/app/<slug>/page/<localPage>`.

For each entry:

1. resolve `{ surah, localPage, globalPage }` through compact accessors;
2. query Uthmani SQLite only for that one Surah/global-page intersection;
3. normalize each returned ayah exactly once;
4. emit a minimal route header and that page's content; and
5. serialize no neighbor page, catalog, range family, sajda list, or full-Surah
   array.

The route DTO is coordinate-aware. It carries VerseKeys and page bounds; it
must not pass a clipped `string[]` to an API that assumes element zero is ayah
1.

### 6.2 Global page routes

`/app/page/<globalPage>` queries one complete global page. It may contain
multiple Surahs, but never loads a neighboring global page. It uses the same
one-text DOM and client-only control boundary as a Surah route.

### 6.3 Normalization and openers

Canonical opener handling occurs once per rendered text unit:

- Surah local page 1 contains ayah 1 and may render its opener according to the
  existing normalization rules;
- later Surah-local pages never synthesize another opener;
- a global page renders an opener only when it contains that Surah's ayah 1;
  and
- prepending/restoring a page does not duplicate an opener or ayah.

---

## 7. One mode-neutral Quran DOM

SSG emits one semantic sequence. Each ayah has one text node and marker,
identified by the complete VerseKey:

```html
<ol class="quran-text" data-reader-content>
  <li id="ayah-2-24" data-verse-key="2:24" class="ayah">
    <span class="ayah-text" dir="rtl">…</span>
    <span class="ayah-marker">٢٤</span>
    <span data-verse-actions></span>
  </li>
</ol>
```

An ancestor `data-reader-mode="ayah" | "reading"` changes layout with CSS:

- Ayah-by-Ayah presents each `li` as a row/card;
- Reading presents the same `li` elements inline as continuous text.

There is no second tab-panel copy, alternate text tree, or duplicate ID.

Static generation cannot vary HTML by visitor cookie. A small pre-paint script
may read a non-HttpOnly `reader-mode` cookie and set the mode attribute before
paint. Missing/invalid state falls back to Ayah-by-Ayah. The regular app later
owns changes and persists them back to the cookie.

The script may not initialize the Worker, open OPFS, fetch metadata, or download
a database in the render-blocking path.

### Client-only scope

The following are absent from SSG HTML and dynamically imported after content
paints:

- the 114-item sidebar and browse/search data;
- mode and font-size controls;
- bookmark, copy, share, note, and tafsir controls;
- tooltips, textareas, and their icon SVGs; and
- closed reader drawers/panels.

Minimal headings, ayah anchors, canonical page links, and accessible fallback
navigation remain server-rendered because they are content structure, not app
controls.

An `onMount` condition alone is insufficient: an eager component import still
adds its module and dependencies to the initial preload graph.

---

## 8. Hydration and continuous scrolling

The one-page SSG DOM is immediately readable and remains the fallback while
client data starts.

1. Hydration adopts the requested SSG page by VerseKey. It does not fetch the
   previous or next page merely because the route mounted.
2. If the immutable offline database is cached, it becomes the first source for
   later scroll requests.
3. If the database is not cached, preserve the painted SSG page and initialize
   the offline data after first paint without blocking interaction.
4. When the reader actually approaches the bottom or explicitly tries to move
   above the top, request only that adjacent Surah-local page. Try the offline
   database first; use the future range API when offline data is unavailable.
5. Until the API exists, a cold reader keeps the SSG page usable while the
   offline database initializes, then fulfills the pending scroll request.
6. A loaded page never crosses the current Surah. Prepending preserves visual
   position, and insertion is idempotent by `{ surah, localPage, globalPage }`
   and VerseKey.
7. As the visible page changes, `history.replaceState` updates the canonical
   local-page path without reload.
8. Keep a bounded number of pages in the DOM. Before removing a distant page,
   remember its rendered height and leave a same-height spacer so the scrollbar
   and visible text do not jump. A default estimated height is only a fallback
   until that page has been measured.

The future API itself is not implemented by this migration and is not an
acceptance blocker. This migration only gives page loading a source boundary so
the API can be added later without changing the reader.

The current cache is unsafe for this model: `seedSurah(num, string[])` assumes
array element zero is ayah 1, and `refreshFromWorker(num)` fetches a whole
Surah. Replace it with coordinate-aware storage such as `Map<VerseKey, text>`
plus a page-source boundary that can use Worker ranges now and the future API
later. Copy/share/note operations must work for a non-first local page and never
read the wrong `n - 1` element.

If an in-app deep ayah action names a page outside the currently mounted set,
resolve and load that local page before scrolling to its full VerseKey anchor.

---

## 9. Implementation phases

### Phase 0 — freeze baselines and contracts

- Save raw, gzip, and Brotli HTML/page-data sizes for representative one-page,
  boundary, interior, Al-Baqarah, cross-Surah global, and Juz routes.
- Save initial modulepreload and JavaScript byte totals.
- Add fixtures for all 662 Surah-local pages and 51 cross-Surah global pages.
- Decide the Juz strategy from §3 before final acceptance.

### Phase 1 — create the immutable metadata boundary

- Produce and verify two compact snapshots with a disposable converter.
- Add TypeScript maps, accessors, and server/client loaders.
- Move entries, sitemap, normalization, sidebar search, and range lookups to
  accessors.
- Delete names/coordinates JSON, coordinate generator, XML scanner, virtual
  metadata module/declaration, and Vite/Vitest metadata registration.
- Preserve the Vite plugin's unrelated local SQLite serving/emission in a
  renamed focused plugin.
- Remove the web Docker build's XML copy/watch dependency; retain backend XML.

### Phase 2 — add canonical local-page routes

- Add `/app/[surah]/page/[localPage]` prerender entries; base path is local 1.
- Add strict bounds/404 behavior and `/page/1` normalization.
- Replace whole-Surah SQL/DTOs with one clipped, coordinate-aware local page.
- Prove that SSG HTML and page data contain neither adjacent local page.
- Update deep links, canonicals, sitemap, and sequential crawl links.
- Keep `/app/page/[globalPage]` single-page with cross-Surah fixtures.

### Phase 3 — collapse the DOM and move controls client-side

- Replace two reader-mode trees with one mode-neutral ayah sequence.
- Split semantic Quran markup from dynamically imported client controls.
- Add the pre-paint mode boot and CSS presentation modes.
- Use complete VerseKey anchors everywhere.
- Measure the one-page hydration data. If duplicated Quran text still misses the
  agreed budget, make semantic content non-hydrated and mount small interactive
  islands that read coordinates/text from the DOM.

### Phase 4 — make offline state page-aware

- Replace full-Surah array indexing with VerseKey/range caching.
- Add Worker range requests, cached/cold initialization, and a page-source
  boundary for the future API fallback.
- Load adjacent pages only after scroll intent—not during route initiation or
  hydration.
- Implement prepend/append, URL updates, measured-height spacers, bounded
  eviction, and deep-link load.
- Verify no boundary crossing or duplicate opener/ayah insertion.

### Phase 5 — trim critical metadata and verify output

- Lazy-load sidebar/catalog/navigation after paint or explicit intent.
- Remove unused DTO fields and repeated inline presentation values.
- Compare per-document, page-data, critical JS, aggregate output, and build
  time with Phase 0.
- Inspect generated HTML—not only source components—for every acceptance rule.

---

## 10. Acceptance criteria

### Data and coordinates

- Exactly two declared tracked web metadata JSON snapshots exist.
- Web build/runtime has no XML read, parser, watcher, virtual metadata module,
  Docker copy, or maintained converter/generator.
- Snapshot provenance records unchanged XML digest and schema version.
- Counts are exactly 114 Surahs, 6,236 ayahs, 604 pages, 30 Juz, 556 rukus, 240
  hizb quarters, 7 manzils, and 15 sajdas.
- The 604 global pages tile ayahs `1..6236` without gaps/overlap.
- The 662 local intersections tile each Surah once, use contiguous local
  indices, and each reference exactly one global page.

### SSG content

- Every Surah route contains exactly its requested Surah-local page, regardless
  of whether it is the first, an interior page, or the last.
- No Surah route contains or serializes ayahs from its previous or next local
  page.
- No Surah route performs a whole-Surah read or serializes a full-Surah array.
- A global route contains exactly its page, all intersecting Surahs, and no
  neighboring page ayahs.
- Each VerseKey appears once, with one globally unique anchor and one
  text/marker representation shared by both modes.
- No sidebar item, action button, tooltip, action SVG, note field, or hidden
  alternate-mode Quran tree appears in built SSG HTML.
- No full catalog/navigation/sajda collection appears in Svelte page data.

### First paint and client behavior

- Useful Quran HTML paints without waiting for Worker, OPFS, compact JSON, or
  Quran database download.
- Initial modulepreloads exclude sidebar, tooltip, textarea, verse-action, and
  corpus-navigation feature modules.
- A valid mode cookie selects Reading or Ayah-by-Ayah without replacing Quran
  nodes or causing hydration mismatch; invalid/missing state selects Ayah mode.
- Cached initialization adopts SSG; cold initialization leaves it usable and
  begins after paint.
- Route initiation and hydration make no previous/next Surah-page read from
  either the offline database or an API.
- Approaching a scroll boundary loads only the needed adjacent page, preferring
  offline data and allowing the future API as the cold-cache fallback.
- Scrolling and eviction preserve position, never duplicate ayahs/openers, and
  never cross a Surah boundary.
- Copy, share, bookmark, and note behavior passes on non-first local pages.

### URL and SEO

- Base Surah URL represents local page 1; every later local page has a
  prerendered path URL and self-canonical.
- Sequential pages use real anchors and are in the sitemap; `/page/1` is not an
  indexed duplicate.
- Reload restores the same single requested local page.
- Deep ayah URLs select their containing local page before scrolling.

No fixed byte budget is invented here. Phase 0 establishes a checked-in
baseline, and the implementation PR sets budgets from the measured bounded
design. Structural acceptance remains mandatory even when Brotli makes
redundant markup look inexpensive.

---

## 11. Explicit non-goals

This plan does not:

- delete or rewrite `quran-data.xml`;
- change Rust API parsing or Arabic SQLite source files;
- introduce a recurring Quran data-generation command;
- include previous or next Surah-local pages in SSG HTML or page data;
- make global routes include previous/next global pages in SSG;
- allow client-loaded Surah pages to cross a Surah boundary;
- render separate Quran trees for reader modes;
- redesign translations, service-worker precaching, Firebase, or R2 delivery;
  or
- claim query parameters/fragments create separately prerendered HTML.

---

## 12. Independent release blocker: persistence ordering

`setCurrent()` currently writes persistence before `hydrate()` restores
localStorage on a Surah load. That can overwrite bookmarks and notes with the
default empty state. It is not an SSG-size optimization, but paginated routing
will exercise the same path more often.

Before shipping the route refactor, suppress writes until hydration completes
(or guarantee hydration first), with a regression test proving that an
existing bookmark and note survive direct Surah and Surah-local-page loads.
