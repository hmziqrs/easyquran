# IndoPak v4 verification progress

Updated 2026-10-07. **Engineering validation complete within the agreed scope; editorial and
recitation approval remain separate.**

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
| Native Safari copying     | `corpus-copy-safari-resumed/safari-corpus-report.json`                                                                                                  | All 6,236 verses × 32 states, 800 batches, zero errors; 512 complete checkpoint rows reused, nine partial rows rerun                                                      |
| Ring hover                | `interactions-tooltip-dev/pointer-regression.json`, `pointer-pixel-comparison.json`                                                                     | Chromium/WebKit tooltip opens; before/after screenshots pixel-identical                                                                                                   |
| Chromium interactions     | `interactions-tooltip-scoped/chromium-interaction-report.json`                                                                                          | Script settings, history, mode/reload, bookmark persistence, selected OS clipboard and copy button pass                                                                   |
| Chromium search/bookmarks | `search-bookmark-reveal-pinned/chromium-search-bookmark-report.json`                                                                                    | Dev: twenty exact Arabic/English snippets each, actual translation picker download, requested translated verse 2:64, bookmark-list verse 1:7; zero runtime/console errors |
| Font delivery             | `phaseC-final/delivery/report.json`, `offline-report.json`                                                                                              | Exact packaged bytes, immutable headers, slow load, TTF fallback, explicit failure/reload, offline reload pass                                                            |
| Android emulator          | `phaseC-final/android/android-report.json`                                                                                                              | 12 geometry states, full 1,280-specimen scroll, 1,383 occurrences, rotation, pinch zoom, history and font equality pass                                                   |
| Android full DOM ink      | `phaseC-android-view-full/android-dom-report.json`                                                                                                      | 1,338 cases, all 1,383 occurrences; zero invisible, geometry, boundary, overlap or ambiguous scopes; actual view pixels                                                   |
| Android ring ink          | `phaseC-android-batched-smoke/android-ring-report.json`                                                                                                 | All 1–286 numbers × five sizes; 1,430 cases, zero failures at DPR 2.625                                                                                                   |
| iPhone/iPad simulators    | `simulators/{iphone,ipad}-{portrait,landscape}-{reading,verse}.json` (iPhone landscape reading uses `-v2`)                                              | 15 geometry states per mode/orientation/device; exact specimen text, font adjustment and canvas rings pass; DOM masks not performed                                       |
| Native mobile find        | `simulators/native-controls-report.json`                                                                                                                | Exact 19:17 phrase found across two word boxes on iPhone/iPad; visible continuous highlight; specimen scope                                                               |
| Narrow iPad Safari        | `simulators/ipad-narrow-{reading,verse}.json`, `native-controls-report.json`                                                                            | 497×636/DPR 2 native window, both modes × 15 geometry states pass; further narrowing unchanged; back/forward restore specimen mode URLs                                   |
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
| R2 no missing/fallback glyphs | P        | P         | P             | P             | P                | M                | M              |
| R3 placement                  | P        | P         | P             | P             | P                | M                | M              |
| R4 no collisions              | P        | P         | P             | P             | P                | M                | M              |
| R5 word/final-cluster breaks  | P        | P         | P             | P             | P                | P specimens      | P specimens    |
| R6 overflow                   | E        | E         | E             | E             | E                | E specimens      | E specimens    |
| R7 ring/stack ink             | P        | P         | P             | P             | P                | M                | M              |

R1–R7 entries apply only to the scopes above. Exact-copy passes validate DOM copy-event
payloads; native OS paste can reorder combining marks, as documented below.

Owner decision, 2026-10-07: current mobile-browser coverage is sufficient. Exhaustive mobile
integration is deferred to native-app work. Current iOS/iPadOS geometry/native samples and
full Android ink/ring/neighbor checks supplement the passing desktop engines. Deferred checks
remain untested; this decision does not imply desktop engines prove every mobile behavior.

Shared requirements:

| Requirement                      | Current result                                                                                 |
| -------------------------------- | ---------------------------------------------------------------------------------------------- |
| R8 reference identity/boundaries | Full corpus compared with classified differences; source/editorial review open                 |
| R9 owner visual scores           | Pending; no scores supplied                                                                    |
| R10 platform reach               | Owner accepts current mobile coverage; full iOS ink/native integration and oldest iOS deferred |
| R11 delivery/support floor       | Production delivery, Chrome 111 and Firefox 128 pass; oldest Safari/iOS untested               |
| R12 licensing                    | Pass: OFL/FONTLOG packaged; primary names clean; original author attribution retained          |
| R13 hard rules                   | Pass: immutable DB reads, no automated Quran hashes, no tracked DB, all gates green            |

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
occurrences in 16 states. Chromium 111 also passes all 32 matrix states, combined
1,338-case/1,383-occurrence ink analysis, 1,430 rings and all 16 neighbor states/149
occurrences. Evidence: `floor-chromium111-reviewed/` and `floor-chromium111-flow/`.
Its old Playwright 1.31 driver injected service-worker registration into
an insecure-origin page with no service-worker API; raw errors remain retained. The corrected
run uses the current driver with the same official Chromium 111 binary. Eight affected ink
cases were recaptured with real viewport tiles and character ranges plus independent font
bounds for negative side bearings; all pass with unchanged tolerances. Replacement metadata
preserves key/code/index/DPR and original captures in a separate combined evidence folder.
No fallback for older unsupported engines was added. Only iOS 27 is installed; the official iOS 17.5 runtime download was unavailable.

Simulator WebDriver repeatedly found no matching session host. The dev-only in-page probe
produced actual iPhone and iPad portrait/landscape geometry reports in both modes, 15 states
each; it explicitly records DOM mask analysis as not performed. Native checks resumed after
unlock. iPhone and iPad Find on Page locate the exact immutable 19:17 phrase across two word
boxes, with a continuous visible highlight. iPad native Page Zoom reaches 115% and was restored
to 100%; both simulators have Capture Keyboard off. Evidence:
`simulators/native-controls-report.json`. These are specimen checks; production find, OS
selection/copy, production history and full iOS ink coverage remain open.
Native iPad window resizing reaches 497×636 at DPR 2; further narrowing produced no change.
Both modes pass all 15 geometry states in this window. Native Back restores the reading URL
and Forward restores the verse URL. Reports and full-resolution native pixels are retained
under `simulators/`; these checks remain within specimen scope.
Native iPad production Al-Fatihah also adopts the IndoPak preference, switches between
verse/reading modes, toggles light/dark and keeps reading mode after reload. Native Find
locates the exact 1:1 phrase across boxes. The copy button confirms success and native address
paste contains the exact verse prefix/reference. Multiline paste remains unverified: an
independent fixed-text clipboard control outside the app fails too. Failed control metadata,
fixture and pixels are retained; no font/renderer change was justified by that control.
Desktop Safari native Find returns one cross-box phrase match on the targeted specimen;
screenshot capture was unavailable. Details: `simulators/native-controls-report.json`.
VoiceOver was enabled, its gesture notice dismissed and individual word focus observed on
production 1:1. VoiceOver was restored off; speech and private-code behavior remain unverified.
Native UI later resumed. Desktop Safari fixed-text clipboard control preserves all 51 code
points but swaps U+0651/U+064E at two positions. Native paste is canonically equivalent,
but exact code-point equality fails outside the reader/font. iPad paste events show the same
reordering; its textarea value also differs. Evidence: `simulators/desktop-safari-clipboard-control-attempt1.json`
and `simulators/ipad-clipboard-control-codepoints.json`. No renderer change was justified;
exact DOM copy payloads and OS paste outcomes remain distinct.
Android full DOM ink capture completed, but oversized element screenshots
repeat viewport content: 280/1,383 occurrence scopes are blank and 15 cases have geometry
mismatches. Neighbor flow similarly has 85/149 blank scopes and six mismatches. These are
invalid diagnostics, not accepted collision results. The subsequent surface-based viewport
tiles eliminate geometry mismatches but still leave 40 full-run and six neighbor occurrences
invisible. Native screenshots prove that Chrome surface capture reflows text differently from
the actual view despite unchanged DOM geometry. The harness now captures CDP view pixels
(`fromSurface=false`) and validates DPR and raster origin with an external calibration square.
The analyzer rejects Android surface tiles. Eight previously failing cases now pass with zero
invisible signs, geometry mismatches, boundary or overlap candidates and unchanged tolerances.
Evidence: `android-view-smoke/`; full actual-view analysis now passes all 1,338 cases and 1,383 occurrences, with zero
invisible signs, geometry mismatches, boundary, overlap or ambiguous scopes. Evidence:
`phaseC-android-view-full/android-dom-report.json`. The neighbor capture was interrupted by
an Android guest system crash (`DeadSystemException` in System UI and other guest services).
Raw failure/crash logs remain under `android-view-flow-interrupted/`. The separate retry
passes all 142 cases/149 neighbor occurrences, with zero invisible, geometry, boundary,
overlap or ambiguous-scope failures: `phaseC-android-view-flow/android-flow-dom-report.json`. Native ring batches and the earlier
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

**Engineering scope:** desktop validation complete; current mobile coverage accepted by the
owner on 2026-10-07. Full mobile integration, iOS DOM masks, oldest iOS and complete
assistive-technology/OS clipboard checks are deferred to native-app work.

Android corpus-copy run logged 18 complete reading batches (4,608 verses × 16 run states)
before the emulator/process went offline. No final report exists; this is an interrupted run,
not a full-corpus pass. Log: `corpus-copy-android.log`; rerun deferred under the revised scope.

**Human review:** owner visual scores and qualified editorial/printed-edition approval remain
open. Engineering acceptance does not establish recitation or editorial approval.

Production reader actions (Chromium and WebKit, `cabdcca`) now pass five actions each with
zero runtime/console errors in `interactions-production-api-fixed/`. SDK fixtures use the
packaged public config/local installation and empty analytics; service workers are blocked.
Production search/download/bookmarks at `a2ab265` pass in both engines: three actions,
20 exact Arabic and 20 exact English snippets each, actual picker download, translated
2:64 navigation and bookmark 1:7; zero runtime/console errors. Evidence:
`search-bookmark-production-translation-boot/`.

## Local workspace cleanup

On 2026-10-07, removed 2,493,653,828 bytes (about 2.5 GB) of disposable audit files:
four production/performance build copies, two old Playwright runtime installations and
two generated browser probes. Source snapshots remain in `run2/scratch-sources/` as four
archives totaling 8,254,130 bytes, excluding builds, dependencies, DBs, environment files
and symlinks. Old runtime package manifests are retained there too.

Audit reports, screenshots, review sheets, font inputs, Python audit environment and
reproduction provenance remain available. Provisioned `db/` and shipped font metadata
were checked before/after and stayed unchanged; selected final reports and review files
also stayed unchanged. The local removal manifest is `run2/cleanup.json`.
Removed scratch builds need reconstruction before rerunning their server commands.
