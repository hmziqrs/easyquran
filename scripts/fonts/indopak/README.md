# IndoPak compatibility font

Version 4 is integrated into both IndoPak reader modes. Default Uthmani stays unchanged.
It is built on SIL Lateef SemiBold (OFL, South Asian letterforms) to match Quran.com's IndoPak
look; see [Quran.com study](../../../docs/indopak-qurancom-rendering-study.md). Versions 2–3
(Noto Naskh base) stay packaged for cached clients.
See [font/data evidence](../../../docs/indopak-font-compatibility.md) and
[Quran.com comparison](../../../docs/indopak-qurancom-comparison.md),
[deep audit/source discrepancies](../../../docs/indopak-deep-audit.md).

Run from repository root with Python 3.9+, Node 24+, and pnpm dependencies:

```sh
python3 -m venv .cache/indopak-venv
.cache/indopak-venv/bin/pip install -r scripts/fonts/indopak/requirements.txt
.cache/indopak-venv/bin/python scripts/fonts/indopak/build.py
.cache/indopak-venv/bin/python scripts/fonts/indopak/audit.py \
  --font web/static/fonts/indopak-reader-compat-v4.ttf \
  --font web/static/fonts/indopak-reader-compat-v4.woff2
.cache/indopak-venv/bin/python -m unittest discover -s scripts/fonts/indopak -p 'test_*.py'
.cache/indopak-venv/bin/python scripts/fonts/indopak/validate.py \
  --upstream .cache/fonts/indopak/Lateef-SemiBold.ttf
```

Equivalent commands when pinned Python dependencies are installed globally:
`pnpm font:indopak:preview` (the retained command accepts `--preview`),
`pnpm font:indopak:audit`, `pnpm font:indopak:test`.

The build verifies font/license checksums and pinned dependency versions. It rejects
unresolved/conflicting mappings, absent glyphs, altered original cmaps and inventory/context
mismatches. An unapproved manifest is rejected before downloading/writing production output.
It has no Quran DB input and never hashes Quran data.

Inventory/mapping files are tracked; immutable Quran DBs are separately provisioned under
ignored `db/`. Audit/validation open SQLite `mode=ro&immutable=1`. All three PUA ranges are
inventoried. No DB, restricted font, outlines or reference layout enter this package.

For actual browser checks, start the development server and run:

```sh
pnpm --dir web dev --port 5391
pnpm font:indopak:browser
```

Override host with `INDOPAK_SPECIMEN_BASE`, output with `INDOPAK_SPECIMEN_OUTPUT`; default
screenshots/report go to ignored `.cache/indopak-browser`. The `/design/indopak` specimen
returns 404 in production. It uses the real reader component and original DB strings.
A separate labelled diagnostic substitutes only E004/U+0657, never reader data.

Harness covers every recorded context/repertoire character, original DOM text, fetched font
bytes, sizes/wrapping/mobile, ornament enclosure and composing fallback. Actual canvas paint
checks detect inverted-damma drift and optional-ayah/pause collisions. End-sign checks reject
ornament collisions and orphaned final-word markers. Unavailable engines are reported, while
remaining engines continue; any engine failure exits nonzero. Firefox cannot launch on this
Mac; full Gecko evidence is collected in the official Linux Playwright container.

`production_approved` denotes technical browser integration, not printed-edition provenance
or recitation approval. Lateef's letterforms resemble, but are not, Quran.com's restricted font.
E004 intrinsic positioning is scoped to its one immutable corpus context and actual browser
bidi runs; forced whole-verse shaping is not a substitute for browser position verification.

Files: `upstream.json` input pins/provenance, `inventory.json` complete repertoire/context
counts, `mapping.json` symbol identities/constructions, `outputs.json` packaged font metadata,
`shaping-report.json` and `browser-report.json` validation, `qurancom-comparison.json` reference
observations, `requirements.txt` pinned build/test dependencies. Font assets include complete
OFL and modification log. Existing Quran text license/attribution is unchanged.

## Full occurrence and live-reference audits

Install additional audit dependencies with `pip install -r scripts/fonts/indopak/audit-requirements.txt`.
Use the same Python environment as the pinned build. These are explicit audits, not boot/build paths.

```sh
python3 scripts/fonts/indopak/deep_audit.py
node scripts/fonts/indopak/audit-server.mjs
node scripts/fonts/indopak/deep-browser-check.mjs
python3 scripts/fonts/indopak/dom_ink_analysis.py --engine chromium
python3 scripts/fonts/indopak/dom_ink_analysis.py --engine webkit
python3 scripts/fonts/indopak/dom_ink_analysis.py --engine firefox
python3 scripts/fonts/indopak/ring_ink_analysis.py --engine chromium
node scripts/fonts/indopak/corpus-browser-check.mjs
INDOPAK_DEEP_FLOW=1 INDOPAK_DEEP_ENGINES=chromium,webkit node scripts/fonts/indopak/deep-browser-check.mjs
python3 scripts/fonts/indopak/dom_ink_analysis.py --engine chromium-flow
python3 scripts/fonts/indopak/dom_ink_analysis.py --engine webkit-flow
node scripts/fonts/indopak/live-reference-check.mjs
python3 scripts/fonts/indopak/compare_reference.py
```

Full output defaults to ignored `.cache/indopak-deep`; override with `INDOPAK_DEEP_OUTPUT` and
`deep_audit.py --output`. `audit=all` adds every private-containing verse to the real reader
specimen. `audit=flow` checks every optional-ayah verse with original same-surah neighbors.
`INDOPAK_DEEP_SKIP_DOM=1` runs the matrix without DOM captures; the report records that scope.
`INDOPAK_DEEP_VIEWPORT_TILES=1` uses real scroll tiles for desktop captures too, useful when
older screenshot backends repeat viewport content. Occurrence scopes include character
ranges and independent font bounds for negative private-glyph side bearings.

`INDOPAK_DEEP_ENGINES` selects engines; default includes Firefox, whose unavailable state
produces nonzero exit while other engines continue. Never report that as a Firefox pass.

Run `audit-server.mjs` in a separate terminal on port 5391 (override with
`INDOPAK_SPECIMEN_PORT`). It disables HMR so source regeneration cannot reload a page
during screenshots. Closing a Vite WebSocket is insufficient: Vite reconnects and reloads.
Freeze renderer/font source before starting a full run. Each capture waits for font loading.

`INDOPAK_DEEP_DIAGNOSTICS_ONLY=1` skips matrices for a targeted ink rerun;
`INDOPAK_DEEP_KEYS` optionally narrows DOM captures. Reports distinguish diagnostic-only from
full-matrix passes. Mask fonts preserve all original layout; transparent COLR layers isolate
ink. DOM reconstruction, negative controls and outside-mask checks reject invalid diagnostics.
Ink/boundary candidates stop approval until reviewed. Production-font/diagnostic raster
comparisons use a recorded two-pixel edge tolerance; masks include shared cmap aliases. No restricted reference font is used
to construct diagnostic fonts.

Font adjustment compares the packaged 33px face against pinned upstream Lateef at the
same effective size (41.25px for 125%). The older 132px/divide-by-four measurement remains
recorded as a diagnostic: integral advance rounding in Chromium 111 makes that comparison
size-dependent. The 1/32 CSS-pixel tolerance is unchanged. A packaged face deliberately set
to 100% rejects every sampled word in the Chromium 111 negative control.

Native Android ink uses the actual device viewport through Playwright `_android`:

```sh
INDOPAK_DEEP_ENGINES=android INDOPAK_DEEP_DIAGNOSTICS_ONLY=1 INDOPAK_DEEP_SKIP_RINGS=1 \
  node scripts/fonts/indopak/deep-browser-check.mjs
.cache/indopak-venv/bin/python scripts/fonts/indopak/dom_ink_analysis.py \
  --output "$INDOPAK_DEEP_OUTPUT" --engine android
INDOPAK_DEEP_ENGINES=android INDOPAK_DEEP_DIAGNOSTICS_ONLY=1 INDOPAK_DEEP_SKIP_RINGS=1 INDOPAK_DEEP_FLOW=1 \
  node scripts/fonts/indopak/deep-browser-check.mjs
.cache/indopak-venv/bin/python scripts/fonts/indopak/dom_ink_analysis.py \
  --output "$INDOPAK_DEEP_OUTPUT" --engine android-flow
```

Ring captures use twelve-number batches in three columns; analysis requires each number
1–286 at each size exactly once before checking pixels. This avoids oversized native
screenshots. Fixed overlays are hidden after scrolling and before each capture. Native
DOM results record DPR and remain distinct from the separate Android geometry matrix.
Each native DOM image is assembled from actual viewport screenshots after scrolling.
Tile crops, scroll offsets, dimensions and complete row coverage are recorded and checked;
missing/repeated rows and out-of-viewport crops fail. Analysis rejects older oversized native
element screenshots, which can repeat viewport content on Android. Failed attempts stay ignored.

## Renderer segmentation audit (Phase B)

`segmentation_audit.mjs` imports the real `indopakEnding` (`web/src/lib/quran/view/indopak.ts`,
Node 24 type stripping) and checks its word boxes, splits, final word, end/inline annotations,
`stop` flags and ink boxes on all 6,236 verses (DB via `node:sqlite`, `mode=ro&immutable=1`).
`box_widths.py` shapes every box with the v4 TTF (venv HarfBuzz) for 56px widths and overflow
prediction. It also maps all 223 inventory context classes to capture verses for Phase C.

```sh
node scripts/fonts/indopak/segmentation_audit.mjs
```

Summary (no Quran text): `segmentation-report.json`. Full per-verse detail goes to
`INDOPAK_PHASEB_OUTPUT` directory (default `.cache/indopak-v4/2026-10-06-run1/phaseB`),
in `segmentation-full.json`.
Exit 1 on any assertion failure.

The development specimen accepts `?keys=19:17,91:14,97:5,56:23` to include regression
verses without private codes. Use the summary's `context_coverage.phase_c_capture_keys`
and `phase_c_review_keys` for the browser capture set. Current verification status and
evidence paths: [v4 results](../../../docs/indopak-v4-results.md).

`?audit=corpus&offset=0&limit=256` pages through all 6,236 immutable verses.
`corpus-browser-check.mjs` checks exact text and end geometry for all verses in 32
states per engine. `reader-browser-check.mjs` visits actual reader rows through scrolling,
checks text and clipping, and writes first/last screenshots for every surah. Set
`INDOPAK_READER_BASE`, `INDOPAK_READER_ENGINE=chromium|webkit|safari`,
`INDOPAK_READER_SURAHS=1,2`, `INDOPAK_READER_MODES=reading,verse`, or
`INDOPAK_READER_SIZE=33` to choose the run. `INDOPAK_READER_OUTPUT` separates a partial
run's reports/captures from full-sweep evidence. Default reader target is local production
port 5392; the specimen stays development-only.

`ring_ink_analysis.py` validates actual DOM digit/ring masks for numbers 1–286 at five
sizes, retaining original font geometry. DOM occurrence scopes include source text ranges
as well as ink bounds: Gecko canvas bounds can underestimate a mixed-direction run.
Frozen tolerances remain 2 CSS pixels × DPR and 3 raw pixels for collision candidates.
Native Safari saves partial capture metadata every 25 cases without claiming a partial pass.
`INDOPAK_SAFARI_RINGS_ONLY=1` refreshes only ring captures and writes a separate capture
report. Grid padding keeps screenshot edge fill outside number cells; analysis tolerances
are unchanged.

`qurancom-corpus.mjs` collects public legacy API data. `qurancom-current-corpus.mjs`
collects the website's observed content proxy, preserving current `word.text`, legacy fields,
word locations, font URLs and build ID, with at most two concurrent requests. Set
`INDOPAK_REFERENCE_OUTPUT` to an ignored directory. Compare full data with:

```sh
python3 scripts/fonts/indopak/compare_reference.py \
  --reference .cache/indopak-v4/reference-current \
  --observed-reference .cache/indopak-v4/reference-live \
  --segmentation .cache/indopak-v4/2026-10-06-run1/phaseB/segmentation-full.json \
  --report .cache/indopak-v4/reference-current/full-reference-report.json
```

The optional observed-reference argument verifies proxy word fields against sampled live
website pages and rejects differences. The comparator records encoded sign sequences and exact per-verse word-boundary offsets.
Its encoding table describes the reference font, not Unicode equivalence. Current served
text differs from legacy fields; matching legacy fields alone never proves editorial parity.
Reference font assets and source text stay ignored, outside the font build.

`android-browser-check.mjs` uses a running adb device with Playwright `_android`, checks
both orientations and reader modes at three sizes, then zoom and history. An emulator
is recorded as an emulator. Use `adb reverse tcp:5391 tcp:5391` for the specimen and
corresponding mappings for production/API ports. Android Chrome must allow command-line
flags for Playwright's browser launch.

## Native macOS Safari

Playwright WebKit is not branded Safari. `safari-native-check.mjs` drives installed Safari
through bundled `/usr/bin/safaridriver` (W3C WebDriver), reusing `deep_audit.py` inputs and
the browser-side assertions in `deep-browser-shared.mjs`. Enable Safari → Settings →
Advanced → "Show features for web developers", then Developer → "Allow remote automation"
(first use may also need `safaridriver --enable`). One session at a time; an occluded
automation window is fine (frame waits are timer-bounded, screenshots paint on demand).

```sh
export INDOPAK_DEEP_OUTPUT=.cache/indopak-safari/<run>
python3 scripts/fonts/indopak/deep_audit.py --output "$INDOPAK_DEEP_OUTPUT"
node scripts/fonts/indopak/safari-native-check.mjs
python3 scripts/fonts/indopak/dom_ink_analysis.py --output "$INDOPAK_DEEP_OUTPUT" --engine safari
INDOPAK_DEEP_FLOW=1 node scripts/fonts/indopak/safari-native-check.mjs
python3 scripts/fonts/indopak/dom_ink_analysis.py --output "$INDOPAK_DEEP_OUTPUT" --engine safari-flow
```

Beyond the Playwright matrix it records environment metadata, requested versus measured
viewport, `devicePixelRatio`, served-versus-packaged font bytes, page/run horizontal overflow,
critical-verse captures (both modes, light/dark, 33/56px, 320/640px) and wrap-boundary sweeps.
Element screenshots must match element box × DPR or the run is invalid. Ink analysis keeps
the two-pixel edge tolerance in CSS pixels and scales it by capture DPR; overlap thresholds
stay in raw device pixels (stricter at higher DPR). `INDOPAK_SAFARI_SKIP_CRITICAL=1` skips
critical captures. Reports: `safari-report.json`, `safari-flow-report.json`.

Live reference tool fetches all representative context pages with two concurrent requests,
verifies deployed font family/resource, and stores unchanged reference assets outside repository
(default `/tmp/easyquran-deep-reference`, override `INDOPAK_REFERENCE_OUTPUT`). Comparison
script checks ordered legacy private-code sequences, records source differences, and does not
mutate or normalize DB/reader text. Qualified editorial review remains open.

## Full corpus copying and production interactions

The whole-corpus runners also validate the copy event payload. Specimen control samples
outside the corpus are excluded; every copied corpus string must equal the immutable DB
exactly, with its private signs and whitespace. Use a separate destination so failed/partial
runs cannot overwrite earlier full evidence:

```sh
export INDOPAK_DEEP_OUTPUT=.cache/indopak-v4/2026-10-06-run2/phaseC-final
INDOPAK_CORPUS_COPY=1 INDOPAK_CORPUS_OUTPUT=.cache/indopak-v4/2026-10-06-run2/corpus-copy \
  node scripts/fonts/indopak/corpus-browser-check.mjs
INDOPAK_CORPUS_COPY=1 INDOPAK_CORPUS_OUTPUT=.cache/indopak-v4/2026-10-06-run2/corpus-copy \
  node scripts/fonts/indopak/corpus-safari-check.mjs
INDOPAK_INTERACTION_OUTPUT=.cache/indopak-v4/2026-10-06-run2/interactions \
  node scripts/fonts/indopak/reader-interaction-check.mjs
INDOPAK_READER_ENGINE=webkit INDOPAK_INTERACTION_OUTPUT=.cache/indopak-v4/2026-10-06-run2/interactions \
  node scripts/fonts/indopak/reader-interaction-check.mjs
INDOPAK_INTERACTION_OUTPUT=.cache/indopak-v4/2026-10-06-run2/interactions \
  node scripts/fonts/indopak/reader-range-safari-check.mjs
```

`INDOPAK_CORPUS_RESUME=/absolute/path/safari-corpus-report.json` resumes Safari from complete
256-verse batches. The checkpoint validates ordered states, copy/occurrence counts, viewport,
overflow, ending clearance, source id, font and renderer code identity, and browser version.
Partial batches rerun because their runtime-error collection may not have finished. Reports
save atomically. Older reports without scope metadata require explicit
`INDOPAK_CORPUS_RESUME_LEGACY=1` after confirming their provenance; keep the original report.
Only font/code bytes are hashed. Immutable Quran data remains identified by source id.

The Playwright interaction matrix covers surah/page/juz and English translation routes,
both modes, 22/56px, light/dark, exact source text, font size and clipping. Actions cover
script settings, history, mode/reload, anonymous bookmark persistence, selection/copy and
ring tooltip. Chromium reads the OS clipboard for selected text and the copy button;
WebKit records that clipboard-permission limitation. `INDOPAK_INTERACTIONS_ONLY=1` skips
the route matrix explicitly. `window.find` results are recorded separately from native
browser find UI. The Safari range tool records its error window as after font readiness.

`INDOPAK_INTERACTION_CONFIG_FIXTURE=1` supplies local Firebase installation/public-config
responses and an empty analytics script. Reports record every fixture request and retain
runtime/console errors. This isolates reader interactions from external SDK availability.
Unmodified network failures remain separate evidence.

`search-bookmark-check.mjs` checks twenty Arabic API snippets, twenty English snippets
against the provisioned immutable translation DB, actual picker download, result navigation
with translation context, exact selected IndoPak text, and anonymous bookmark-list navigation:

```sh
INDOPAK_INTERACTION_OUTPUT=.cache/indopak-v4/2026-10-06-run2/search-bookmarks \
  node scripts/fonts/indopak/search-bookmark-check.mjs
INDOPAK_READER_ENGINE=webkit INDOPAK_INTERACTION_OUTPUT=.cache/indopak-v4/2026-10-06-run2/search-bookmarks \
  node scripts/fonts/indopak/search-bookmark-check.mjs
```

The search tool uses the named SDK fixtures and blocks service workers. Failed/aborted
requests are recorded. Optional `INDOPAK_SEARCH_WORKER_DEBUG=1` reads the real worker state
through dev-only module imports; leave it unset against production.

`INDOPAK_READER_BLOCK_TELEMETRY=1` in the full reader runner blocks Firebase Installations
explicitly; it does not suppress collected errors. Preserve unmodified failed runs too.
Do not run web gates/builds while a browser capture is using generated dev modules.

## Reference layout and review sheets

```sh
INDOPAK_REVIEW_OUTPUT=.cache/indopak-v4/2026-10-06-run2/review \
  node scripts/fonts/indopak/reference-layout-check.mjs
INDOPAK_REVIEW_ENGINE=webkit INDOPAK_REVIEW_OUTPUT=.cache/indopak-v4/2026-10-06-run2/review \
  node scripts/fonts/indopak/reference-layout-check.mjs
INDOPAK_REVIEW_OUTPUT=.cache/indopak-v4/2026-10-06-run2/review \
  node scripts/fonts/indopak/reference-safari-check.mjs
.cache/indopak-venv/bin/python scripts/fonts/indopak/reference_layout_summary.py \
  --review .cache/indopak-v4/2026-10-06-run2/review \
  --reference .cache/indopak-v4/2026-10-06-run2/reference-current/full-reference-report.json
.cache/indopak-venv/bin/python scripts/fonts/indopak/review_sheets.py \
  --output .cache/indopak-v4/2026-10-06-run2/review
```

The seeded sample includes 300 stratified verses, every optional-ayah flow verse, targeted
cases and context representatives. The reference is captured at 26px in the same measured
column; actual native viewport limits are recorded. V3 comes from historical renderer,
parser and ornament at `104f049`, with only import/family alias adaptation for the specimen.
Reference data/fonts stay ignored and never enter the font build. `INDOPAK_REVIEW_KEYS`
selects a partial run in either runner; `INDOPAK_REVIEW_RESUME=1` resumes Playwright captures.
`INDOPAK_REVIEW_CAPTURE_ALL=1` captures all three viewports in the selected engine, with
engine/width-qualified filenames. Resume rejects a different capture scope. Fixed/sticky
reference controls are hidden after scrolling without changing layout.

`review/index.html` shows Quran.com | v4 | v3 sheets, blank editable owner scores and JSON
export. Layout statistics, large shifts, final-cluster lines and source boundary classes
live in `line-parity-summary.json`. These comparisons do not constitute editorial approval.

## Offline font delivery

```sh
INDOPAK_DELIVERY_OUTPUT=.cache/indopak-v4/2026-10-06-run2/delivery \
  node scripts/fonts/indopak/delivery-browser-check.mjs
INDOPAK_DELIVERY_OUTPUT=.cache/indopak-v4/2026-10-06-run2/delivery \
  node scripts/fonts/indopak/offline-font-check.mjs
```

The offline tool waits for an activated service worker, checks the cached font against the
package, then reloads the visited Fatihah offline and compares all seven original strings.
It records cache entries and bytes. Slow/failure cases remain in the separate delivery tool.

## Native simulator probe

When simulator WebDriver is unavailable, a dev-only module executes the shared assertions
inside actual simulator Safari and POSTs reports to the audit server:

```sh
node scripts/fonts/indopak/simulator-probe.mjs
INDOPAK_SIMULATOR_OUTPUT=.cache/indopak-v4/2026-10-06-run2/simulators \
  node scripts/fonts/indopak/audit-server.mjs
```

Open `/design/indopak?audit=all&mode=reading&simulator=iphone-portrait-reading` in actual
simulator Safari; use `mode=verse` and a distinct label for verse mode. The generated module
lives in ignored `web/.svelte-kit`. The endpoint is present only when the audit-server
output option is enabled; the specimen and hook remain dev-only. Labels containing
`-landscape-` wait for landscape and reject orientation changes during the matrix.

Reports record actual user agent, viewport, DPR, source text, boundaries, end geometry,
canvas ring samples, size adjustment and font bytes. They explicitly distinguish geometry
from full DOM mask analysis. Browser validation of the module is not simulator evidence.
A locked Mac prevents native simulator UI navigation.

## Android rendering performance

`android-performance-check.mjs` measures three trials of surahs 2 and 26 in both variants
on the actual running emulator with fourfold CDP CPU throttling. It records exact source,
mounted spans, long tasks, standard CLS session windows, CDP duration deltas, heap and font
resources. This is not a physical mid-range-device benchmark.

Prepare a separate ignored baseline with the same current app infrastructure/dependencies
and local environment, replacing only historical v3 renderer/parser/ornament files from
`104f049`. Provision DB access out of band/read-only. Missing local environment can leave
an unupgraded SSR first page and invalidates the full-scroll comparison. Record baseline
provenance and confirm every source verse is actually visited.

```sh
INDOPAK_PERFORMANCE_APP_COMMIT=cabdcca \
INDOPAK_PERFORMANCE_V4_COMMIT=cabdcca \
INDOPAK_PERFORMANCE_V3_BASE=http://127.0.0.1:5397 \
INDOPAK_READER_BASE=http://127.0.0.1:5396 \
INDOPAK_PERFORMANCE_OUTPUT=.cache/indopak-v4/2026-10-06-run2/performance \
  node scripts/fonts/indopak/android-performance-check.mjs
```

Map both ports/API through adb first. Do not launch another Android browser session during
this run, or rebuild the served candidate while timings are collected. Failed baseline
attempts remain invalid evidence rather than a performance regression claim.
