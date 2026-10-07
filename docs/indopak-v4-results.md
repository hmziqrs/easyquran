# IndoPak v4 verification progress

Updated 2026-10-07. **Verification incomplete; no editorial or recitation approval.**

Evidence lives on this Mac under `.cache/indopak-v4/2026-10-06-run2/` (called `run2`
below). Screenshots, reference fonts and full corpus reports stay ignored. The
[verification plan](indopak-v4-verification-plan.md) remains the completion checklist.
The owner authorized simulators/emulators and Quran.com as the reference. No physical-device
result or human score is inferred from those decisions.

Font/renderer candidate: `b4b58f7`; reader measurement fix: `7ccce6a`; page/juz source fix:
`0fede6b`; exact selection-copy fix: `da69f33`; ring hit-testing fix: `d570f9b`;
search URL hydration fix: `4ff7e48`; result action fix: `23d905e`; anchor URL/reveal fixes:
`cf6f3ec` and `cabdcca`; translation boot fix: `a2ab265`.
Later changes preserve the font and glyph geometry. The production full-surah sweeps used
`7ccce6a`; route/copy checks used the later builds. Evidence scopes remain distinct.

## Completed technical scopes

| Work                      | Evidence under run2                                                                                                                                     | Result                                                                                                                                                                    |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Reproducible fonts        | `phaseA/reproducibility.json`; two rebuilds equal packaged TTF/WOFF2                                                                                    | Pass                                                                                                                                                                      |
| Font tests/shaping        | 12 tests; 6,236 verses; no missing glyph or `.notdef`; TTF/WOFF2 traces equal; 5,015 unaffected base runs                                               | Pass                                                                                                                                                                      |
| Latest web gates          | `web-check-translation-boot-v2.log`, `web-lint-translation-boot-v2.log`, `web-test-translation-boot-v2.log`: 0 errors/warnings, clean lint, 2,209 tests | Pass; test workers bounded to four, assertions/timeouts unchanged                                                                                                         |
| Segmentation              | `phaseB/segmentation-full.json`: 6,236 verses, 78,297 boxes, 223 contexts, zero failures                                                                | Pass; 20 justified exceptions                                                                                                                                             |
| Four desktop engines      | `phaseC-final/*-browser-report.json`, `*-dom-report.json`, `*-ring-report.json`                                                                         | 32 states per engine; all 1,383 occurrences; 1,430 digit/ring cases; zero ink failures                                                                                    |
| Whole corpus              | `phaseC-final/*-corpus-report.json`: Chromium, WebKit, Firefox, native Safari                                                                           | All 6,236 verses × 32 states per engine pass                                                                                                                              |
| Neighbor flow             | `phaseC-final/*-flow-*.json`: same four engines                                                                                                         | 16 states; 149 private occurrences per engine pass                                                                                                                        |
| Production whole reader   | `production-final/chromium-reader-report.json`, `safari-reader-report.json`                                                                             | 114 surahs × both modes; all 6,236 verses per mode at 33px/390×844; no text, clipping or recorded runtime errors                                                          |
| Production WebKit         | `production-telemetry-blocked/webkit-reader-report.json`                                                                                                | Same full sweep passes with Firebase Installations explicitly blocked                                                                                                     |
| Production route matrix   | `interactions-copy-production/*-interaction-report.json`                                                                                                | All 48 Arabic/translated route states pass per engine; later interaction portion retained as failed before the tooltip repair                                             |
| Native route matrix       | `interactions-production/safari-range-report.json`                                                                                                      | 24 Arabic surah/page/juz states pass; errors observed after font readiness                                                                                                |
| Selection copying         | `corpus-copy-scoped/chromium-corpus-report.json`, `webkit-corpus-report.json`                                                                           | All 6,236 verses × 32 states copy exact immutable text                                                                                                                    |
| Firefox selection copying | `corpus-copy-firefox-final/firefox-corpus-report.json`                                                                                                  | All 6,236 verses × 32 states, 800 batches, zero errors                                                                                                                    |
| Ring hover                | `interactions-tooltip-dev/pointer-regression.json`, `pointer-pixel-comparison.json`                                                                     | Chromium/WebKit tooltip opens; before/after screenshots pixel-identical                                                                                                   |
| Chromium interactions     | `interactions-tooltip-scoped/chromium-interaction-report.json`                                                                                          | Script settings, history, mode/reload, bookmark persistence, selected OS clipboard and copy button pass                                                                   |
| Chromium search/bookmarks | `search-bookmark-reveal-pinned/chromium-search-bookmark-report.json`                                                                                    | Dev: twenty exact Arabic/English snippets each, actual translation picker download, requested translated verse 2:64, bookmark-list verse 1:7; zero runtime/console errors |
| Font delivery             | `phaseC-final/delivery/report.json`, `offline-report.json`                                                                                              | Exact packaged bytes, immutable headers, slow load, TTF fallback, explicit failure/reload, offline reload pass                                                            |
| Android emulator          | `phaseC-final/android/android-report.json`                                                                                                              | 12 geometry states, full 1,280-specimen scroll, 1,383 occurrences, rotation, pinch zoom, history and font equality pass                                                   |
| Android ring ink          | `phaseC-android-batched-smoke/android-ring-report.json`                                                                                                 | All 1–286 numbers × five sizes; 1,430 cases, zero failures at DPR 2.625                                                                                                   |
| iPhone/iPad simulators    | `simulators/{iphone,ipad}-{portrait,landscape}-{reading,verse}.json` (iPhone landscape reading uses `-v2`)                                              | 15 geometry states per mode/orientation/device; exact specimen text, font adjustment and canvas rings pass; DOM masks not performed                                       |
| Quran.com corpus          | `reference-current/full-reference-report.json`                                                                                                          | All 114 chapters/6,236 verses compared; 201 sampled website pages agree with proxy fields                                                                                 |
| Reference layout/sheets   | `review/*-layout-report.json`, `line-parity-summary.json`, `index.html`                                                                                 | Capture and comparison complete within recorded viewport limits; scores still blank                                                                                       |

Desktop versions: Chromium 153.0.8010.12; Playwright WebKit 26.6 (Playwright 1.63.0);
Firefox 155.0 in the Linux Playwright container; native Safari 27.0, macOS 27.0/26A428,
safaridriver 22625.1.29.11.27. Firefox on this Mac could not launch.
Android: API 37/Android 17 emulator, Chrome 145.0.7632.218, DPR 2.625, 411×782 portrait.
iPhone: iPhone 17/iOS 27 simulator, Safari, DPR 3, 402×714 portrait and 750×338 landscape web viewports.
iPad: A16/iPadOS 27 simulator, Safari, DPR 2, 820×1094 portrait and 1180×734 landscape.

## Requirements by platform

`P`: passed in the recorded scope; `E`: passed with the named overflow exceptions;
`M`: geometry/font measurement only, insufficient for the complete ink requirement;
`U`: untested. Desktop R3/R4 use production/isolated DOM ink, rather than cmap presence.

| Requirement                   | Chromium | PW WebKit | Native Safari | Firefox Linux | Android emulator | iPhone simulator | iPad simulator |
| ----------------------------- | -------- | --------- | ------------- | ------------- | ---------------- | ---------------- | -------------- |
| R1 exact text                 | P        | P         | P             | P             | P                | P specimens      | P specimens    |
| R2 no missing/fallback glyphs | P        | P         | P             | P             | M                | M                | M              |
| R3 placement                  | P        | P         | P             | P             | M                | M                | M              |
| R4 no collisions              | P        | P         | P             | P             | M                | M                | M              |
| R5 word/final-cluster breaks  | P        | P         | P             | P             | P                | P specimens      | P specimens    |
| R6 overflow                   | E        | E         | E             | E             | E                | E specimens      | E specimens    |
| R7 ring/stack ink             | P        | P         | P             | P             | Rings P; stack M | M                | M              |

R1–R7 entries apply only to the scopes above. Shared requirements:

| Requirement                      | Current result                                                                        |
| -------------------------------- | ------------------------------------------------------------------------------------- |
| R8 reference identity/boundaries | Full corpus compared with classified differences; source/editorial review open        |
| R9 owner visual scores           | Pending; no scores supplied                                                           |
| R10 platform reach               | Incomplete: mobile DOM ink, narrow iPad/device interactions, oldest iOS versions      |
| R11 delivery/support floor       | Production delivery passes; oldest-version validation pending                         |
| R12 licensing                    | Pass: OFL/FONTLOG packaged; primary names clean; original author attribution retained |
| R13 hard rules                   | Pass: immutable DB reads, no automated Quran hashes, no tracked DB, all gates green   |

## Candidate changes

TTF: 242,160 bytes; WOFF2: 92,416 bytes. Marker-specific medium/small Urdu digits fit
numbers 1–286 inside the ring. Ordinary text digits and advances remain unchanged.
OFL and FONTLOG ship with assets. Reserved names stay absent from primary names;
original SIL manufacturer/designer attribution remains. Font-only checksums live in
`scripts/fonts/indopak/outputs.json`.

The renderer keeps pause signs with preceding word boxes, handles ordinary/private sign
pairs in either order, gives zero-advance annotations measured ink boxes, shapes RTL Urdu
markers, reserves space above stacked endings, and inherits reader last-line alignment.
Text stays hidden and `aria-hidden` until the font loads. WOFF2 failure uses packaged TTF;
failure of both shows a translated error and reload action.

Two integration defects were repaired during production checks. Page/juz routes previously
kept populated Uthmani SSR data despite a saved IndoPak preference; `0fede6b` requests the
preferred source, including retry. Browser selection serialization inserted linefeeds around
inline/stacked signs; `da69f33` handles IndoPak copy events using exact DOM ranges, excluding
UI ornaments/toolbars. Partial and multi-verse selections have DB-free regression coverage.
The copy button preserves its expected separate reference line. `d570f9b` removes annotation
pointer interception over the ring; this changes hit testing, with pixel equality verified.
`4ff7e48` prevents the initial search debounce from deleting the query from a shared URL;
two Arabic/translated URL regression cases fail before the repair and pass afterward.
`23d905e` executes the verse-open callback when a search result is clicked. `cf6f3ec`
prevents intermediate reader measurements from replacing a requested anchor URL. `cabdcca`
keeps the requested virtual row mounted until reveal ends, including temporarily blocked
scrolling. `a2ab265` queues translation download until worker initialization finishes, while preserving immediate dispatch once ready. Before the fix, an early WebKit picker click returned "engine not ready" and silently left Download idle. Two boot regressions, existing cold-reader regressions and actual WebKit search/download/navigation pass after the fix.

No Quran DB/source text was modified or versioned. Audits use `mode=ro&immutable=1`.
No automated Quran-data checksum was introduced.

## Corpus exceptions and overflow

[Segmentation report](../scripts/fonts/indopak/segmentation-report.json) lists 20 exceptions:
17 verse-leading U+200B characters; one ink-free initial box in 76:17; two combining-mark
annotations, 2:10 (U+06D9 U+06E2) and 7:137 (U+06D9 U+064E). Their Chromium 56px/640px
captures preserve attachment without collision or detached line start; justification is
committed in `4bd9922`. This is rendering review, not recitation approval.

At 56px, unbreakable source tokens exceed the 286px specimen column in 12:21 and 56:23;
18:110 exceeds the reader's 272px column. The 12:21 box advances 277.48px plus a 15.12px
stop margin. 56:23 and 18:110 contain joined source words. The source is preserved.
Former merged boxes in 19:17, 91:14 and 97:5 now fit. Captures are retained under
`phaseC-final/` and `interactions-copy-production/`; matrices reject any other overflow key.

Native Safari's raw `safari-report.json` remains `review_required`: predicted matrix overflow
and six additional 56px sweep records for 2:101, 16:6, 26:51 in both modes. These warnings
occur at 200–230px widths, below the plan's 320px minimum. Final word and marker remain
intact. This does not promise support at 200px.

## Quran.com comparison and visual review

Website build `cfO91iJO_kGMxzuatsf-J`; IndoPak v4.2.1 font, 83,560 bytes; `text_indopak`,
15-line mushaf. Public content proxy returned all 114 chapters without failed requests,
with at most two concurrent requests. Reference assets stay ignored, outside the font build.

All 1,383 legacy private occurrences match. Full legacy verse strings match after
comparison-only removal of whitespace/format characters; no reader normalization occurs.
Proxy fields match 201/201 sampled actual website pages. Served encoding differs:
5,778 identical sequences, 191 order-only differences, 267 sign-pipeline differences.
The encoding table describes reference font assignments, not Unicode equivalence.

Word boundaries: 5,417 identical; 788 reference-joined clusters; 26 source-joined clusters;
two letter-pipeline differences (4:142, 79:28); three mixed cases (4:171, 5:7, 6:77).
These findings need source/editorial review, never silent data repair.

Layout capture uses a seeded 300-verse sample (two per chapter plus 72), all 116 flow verses,
critical verses and context representatives. Same measured CSS column, 26px, light theme;
reference adaptive font size and transition override are recorded. Chromium/WebKit cover
678 keys × three viewports; native Safari covers the 428-key random/flow/targeted union.

| Engine        | Comparable states | Identical line breaks | Mean local minus reference lines |
| ------------- | ----------------- | --------------------- | -------------------------------- |
| Chromium      | 2,031             | 66.864%               | −0.0532                          |
| PW WebKit     | 2,031             | 56.721%               | −0.1521                          |
| Native Safari | 1,281             | 59.094%               | −0.1460                          |

Three letter-pipeline cases per engine are excluded from line-anchor comparison. Native Safari
could not produce a 320px inner window: requested 320, measured 336, recorded as a viewport
gap; 390 and 1280 were exact. The failed 320 attempt is retained. Line-count deltas of at
least two and extra final-cluster lines are listed in `line-parity-summary.json`, with marker
orphaning and source boundaries recorded. Font/spacing assessment remains separate from
editorial identity and owner approval.

The seven large line-count shifts have a separate technical review in
`line-spacing-review-clean/technical-assessment.json`: 2:102, 2:213, 2:233, 2:282, 3:7,
5:110 and 18:22, at all three widths in WebKit. At 320px, v4/reference line counts are
15/17, 10/12, 14/16, 27/30, 9/11, 15/16 and 7/9 respectively. Exact source and the final
word/marker enclosure hold throughout. Independent Lateef advances and original source
boundaries produce the compact flow. For example, 2:233 contains an original whitespace
boundary inside the conventional dhal/lam cluster; preserving it can separate those boxes.
No source or font-spacing change was justified by this technical review.

These follow-up crops observe Quran.com build `C8ogpWPjm5fic3yLrNxLc`, distinct from the
original full comparison. Fixed/sticky reference chrome is hidden without layout changes;
earlier obscured crops remain under `line-spacing-review/`. This assessment supplies no
owner score, editorial approval or native Safari visual-crop claim.

331 three-way crops (Quran.com | v4 | historical v3 at `104f049`) form 28 sheets. The local
`review/index.html` provides seven editable 1–5 categories for v4/v3 and score export.
**Owner scores remain blank. Qualified reviewer and named printed edition remain unknown.**

## Delivery, runtime and platform limits

Production font bytes equal the package and carry immutable cache headers. A font held over
3.5 seconds stays hidden; blocked WOFF2 falls back to TTF; both formats blocked show the
explicit error, and reload recovers. An activated service worker serves byte-equal v4 and
reloads a visited Fatihah offline with all seven exact strings. Recorded cache: 245 entries,
4,820,150 bytes, 17 font assets. Removing old font/TTF precache entries remains an optional
performance follow-up; no cache policy was changed.

`7ccce6a` schedules both reader ResizeObservers through animation frames. Earlier failed
reports are retained. Native Safari and Chromium fresh full sweeps have zero recorded errors.
WebKit's unmodified sweep recorded a Firebase Installations CORS/SDK console failure; the
controlled sweep blocks that endpoint explicitly and passes all reader assertions. No runtime
error filter converts the raw failure into a pass.

Chromium's nonstandard `window.find` finds individual words but not the tested phrase across
word boxes; WebKit finds the phrase. This is not evidence of native Cmd+F behaviour. Native
find UI, assistive-technology pronunciation/private-code behaviour, and selection highlighting
still need direct verification. Native route error listeners start after font readiness and do
not establish startup-error coverage.

Working support floor: Safari/iOS/iPadOS 17+, Chrome 111+, Firefox 128+. This combines
[Tailwind v4 requirements](https://tailwindcss.com/docs/compatibility) with
[Safari 17 size-adjust support](https://webkit.org/blog/14445/webkit-features-in-safari-17-0/).
It is a chosen target, not an oldest-version pass. Chromium 111.0.5563.19 and Firefox 128.0
are running in the official older Playwright Linux images under x64 emulation. Chromium 111's
integral advance rounding invalidated the original 132px normalized width probe. `990c617`
compares at equal effective size, keeping the 1/32 CSS-pixel tolerance; seven positive and
seven deliberately unadjusted negative samples behave correctly. Firefox 128 passes 32
matrix states, all 1,338 DOM cases/1,383 occurrences, 1,430 ring cases and 149 neighbor
occurrences in 16 states. Chromium 111 rings and combined 1,338-case/1,383-occurrence ink analysis pass;
final corrected-driver matrix remains in progress. Its old Playwright 1.31 driver injected service-worker registration into
an insecure-origin page with no service-worker API; raw errors remain retained. The corrected
run uses the current driver with the same official Chromium 111 binary. Eight affected ink
cases were recaptured with real viewport tiles and character ranges plus independent font
bounds for negative side bearings; all pass with unchanged tolerances. Replacement metadata
preserves key/code/index/DPR and original captures in a separate combined evidence folder.
No fallback for older unsupported engines was added. Only iOS 27 is installed; the official iOS 17.5 runtime download was unavailable.

Simulator WebDriver repeatedly found no matching session host. The dev-only in-page probe
produced actual iPhone and iPad portrait/landscape geometry reports in both modes, 15 states
each; it explicitly records DOM mask analysis as not performed. Native checks resumed after unlock. Mac then relocked before native
find/control checks finished; manual unlock is requested. iPad narrow/split view, Safari text size, zoom/history/copy and full iOS ink
coverage remain open. Android full DOM ink capture completed, but oversized element screenshots
repeat viewport content: 280/1,383 occurrence scopes are blank and 15 cases have geometry
mismatches. Neighbor flow similarly has 85/149 blank scopes and six mismatches. These are
invalid diagnostics, not accepted collision results. Actual viewport tiled recapture is in
progress, with full geometry and row-coverage guards. Native ring batches and the earlier
six-case DOM smoke pass. Oversized early ring screenshots and a desktop floating-button obstruction
are retained as invalid capture attempts; clean recaptures pass without changing tolerances.
Android performance comparison completed 12 trials with equal `cabdcca` app infrastructure,
local environment and dependencies, replacing only historical v3 renderer/parser/ornament
files from `104f049`. Each trial visits all 286 verses of surah 2 or all 227 of surah 26,
without runtime errors. Fourfold CPU-throttled emulator medians:

| Surah | Variant | Ready ms | Full scroll ms | Long tasks | Max mounted spans | CLS   |
| ----- | ------- | -------- | -------------- | ---------- | ----------------- | ----- |
| 2     | v3      | 2236     | 23518          | 54         | 98                | 0.011 |
| 2     | v4      | 1752     | 20530          | 52         | 492               | 0.083 |
| 26    | v3      | 2289     | 7817           | 18         | 216               | 0.477 |
| 26    | v4      | 1851     | 7328           | 17         | 650               | 0.550 |

V4 has more mounted spans and higher recorded CLS, despite faster median readiness/scroll
and similar long-task counts in these trials. Both variants have substantial scroll-window
CLS in surah 26; no visual-stability improvement is claimed. Counters start after
DOMContentLoaded and stay within the same document. Evidence:
`performance-api-fixed/android-performance-report.json`, `summary.json`, and
`performance-current-provenance.json`. Earlier unconfigured/counter-reset attempts are
invalid. These timings do not establish physical mid-range-device performance.

## Reproduction and remaining work

Commands/options: [tooling README](../scripts/fonts/indopak/README.md). Captures wait for
fonts and the audit server disables HMR. Frozen tolerances remain 2 CSS pixels × DPR and
3 raw collision pixels. Ring recapture adds 4px grid padding outside digit cells; no threshold
was widened. Original invalid/failed captures remain distinct from their replacements.

**Open:** complete native selection-copy sweep;
Android/iOS DOM ink; narrow iPad/device interactions and oldest iOS versions;
native find and VoiceOver/TalkBack;
owner scores and qualified editorial/printed-edition review. No 100% completion claim is made.

Production reader actions (Chromium and WebKit, `cabdcca`) now pass five actions each with
zero runtime/console errors in `interactions-production-api-fixed/`. SDK fixtures use the
packaged public config/local installation and empty analytics; service workers are blocked.
Production search/download/bookmarks at `a2ab265` pass in both engines: three actions,
20 exact Arabic and 20 exact English snippets each, actual picker download, translated
2:64 navigation and bookmark 1:7; zero runtime/console errors. Evidence:
`search-bookmark-production-translation-boot/`.
