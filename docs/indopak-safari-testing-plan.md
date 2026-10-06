# IndoPak Safari testing plan

> **Superseded** for font v4 (`584569c`) by the [v4 verification plan](indopak-v4-verification-plan.md),
> which folds in this plan's device, delivery and Quran.com sections. Kept as the v3 record.

Created 2026-10-06. **§2–3 run 2026-10-06: [results](indopak-safari-results.md). Native macOS
Safari passes mechanical/ink checks with open layout review items; real-device, older-Safari,
production-reader and Quran.com checks remain pending.**
Font/renderer baseline: commit `104f049`, `indopak-reader-compat-v3`.
Existing results and editorial findings: [deep audit](indopak-deep-audit.md).

Goal: establish evidence for private-symbol rendering in native macOS Safari and Safari on
real iPhone/iPad hardware. Preserve exact Quran text, verify actual reader layout, document
every failure and unavailable platform. Browser approval and editorial approval have separate
completion criteria.

## 1. Test environments and evidence

| Environment                                        | Required coverage                                                         | Current status                                                  |
| -------------------------------------------------- | ------------------------------------------------------------------------- | --------------------------------------------------------------- |
| Installed stable macOS Safari                      | Full occurrence/layout matrix, ink diagnostics, real reader               | Safari 27.0 specimen pass, review items open; real reader open  |
| Previous Safari/macOS version intended for support | Same full matrix and critical reader checks on separate host              | Pending; record exact available version before claiming support |
| Real iPhone, current stable iOS                    | Full occurrence sweep, critical matrix, touch/zoom/cache tests            | Pending                                                         |
| Real iPhone, oldest iOS intended for support       | Same device checks; prefer smallest supported screen                      | Pending; supported version must be recorded                     |
| Real iPad, current stable iPadOS                   | Full occurrence sweep, critical matrix, portrait/landscape, narrow window | Pending                                                         |
| Playwright WebKit on macOS                         | Repeatable baseline and regression diagnostics                            | WebKit 26.6 rerun against `6486488`: pass, review items open    |

Use one device for multiple rows only when its recorded OS/version satisfies those rows.
Unavailable hardware/version stays **untested**. Simulator and Safari Technology Preview may
provide supplemental evidence; label their results explicitly.

Playwright uses a patched WebKit build and cannot control branded Safari. Its device presets
provide emulation. Native Safari needs its own driver pass.
([Playwright browser documentation](https://playwright.dev/docs/browsers#webkit))

For each run record: commit, database id `quran-indopak`, font asset URL/version/byte size,
device model, OS/build, Safari version, driver version, viewport in CSS pixels,
`devicePixelRatio`, page zoom, reader mode, font size, run width, theme, date/time and origin.
Store screenshots, machine reports and logs under ignored `.cache/indopak-safari/<run>/`.
Commit a concise results document linking evidence locations and identifying untested rows.

All corpus access stays read-only. Keep DBs ignored/untracked; no DB changes, migrations,
normalization, versioning or Quran-data SHA-256. Use existing inventory and exact source
strings as expectations. Reference fonts and screenshots stay outside tracked production assets.

## 2. Prepare and reproduce WebKit baseline

- [x] Record candidate commit and environment metadata; retain previous audit evidence.
- [x] Use repository-pinned Node/pnpm/Playwright and Python audit dependencies. Reuse
      `.cache/indopak-venv`; create it if absent, then install
      `scripts/fonts/indopak/audit-requirements.txt` into that environment when needed.
- [x] Start or reuse dev server at `http://localhost:5391`. Check specimen controls hydrate.
- [x] Generate fresh diagnostic inputs and run existing WebKit checks below. Inspect JSON
      reports and ink-analysis results as well as exit codes; raw overlap candidates require review.

Run from repository root, with dev server running in another terminal:

```sh
pnpm --dir web dev --port 5391
```

```sh
export INDOPAK_SPECIMEN_BASE=http://localhost:5391
export INDOPAK_DEEP_OUTPUT=.cache/indopak-safari/2026-10-06-webkit
.cache/indopak-venv/bin/python scripts/fonts/indopak/deep_audit.py --output "$INDOPAK_DEEP_OUTPUT"
INDOPAK_DEEP_ENGINES=webkit node scripts/fonts/indopak/deep-browser-check.mjs
.cache/indopak-venv/bin/python scripts/fonts/indopak/dom_ink_analysis.py --output "$INDOPAK_DEEP_OUTPUT" --engine webkit
INDOPAK_DEEP_FLOW=1 INDOPAK_DEEP_ENGINES=webkit node scripts/fonts/indopak/deep-browser-check.mjs
.cache/indopak-venv/bin/python scripts/fonts/indopak/dom_ink_analysis.py --output "$INDOPAK_DEEP_OUTPUT" --engine webkit-flow
```

Choose a new output directory for each candidate/run. `INDOPAK_DEEP_ENGINES=safari` is
**not supported**; native Safari runs through `scripts/fonts/indopak/safari-native-check.mjs`
(see [font tooling](../scripts/fonts/indopak/README.md#native-macos-safari)).

## 3. Native macOS Safari automation

- [x] Enable Safari's developer menu and Allow Remote Automation on test host. If first-time
      driver setup requires authorization, host owner completes `/usr/bin/safaridriver --enable`.
      Record original settings and restore them after testing when appropriate. Confirm setup
      against installed Safari UI and `man safaridriver`.
- [x] Create a small W3C WebDriver adapter around bundled `/usr/bin/safaridriver`, reusing
      existing audit inputs and browser-side assertions. Keep native reports separate from WebKit.
- [x] Run one Safari session at a time. Use accessible control labels and existing specimen
      identifiers. Wait for hydration, explicit font load and settled layout; avoid fixed sleeps.
- [x] Record requested and actual viewport dimensions. Browser-window dimensions and inner
      viewport dimensions differ; do not label a run 320px unless the measured run is 320px.
- [x] Extend ink-analysis input handling for native captures, including screenshot scale/DPR.
      Existing analyzer accepts Chromium/WebKit names only. Preserve original production-font
      captures alongside every diagnostic capture.

Safari includes its own WebDriver executable and requires enabling remote automation.
([Apple WebDriver documentation](https://developer.apple.com/documentation/webkit/testing-with-webdriver-in-safari),
[WebKit setup reference](https://webkit.org/blog/6900/webdriver-support-in-safari-10/))

### Full native matrix

- [x] `/design/indopak?audit=all`: all 1,243 specimen verses, including all **1,383 private
      occurrences across 1,218 verses** plus repertoire controls. Run reading and verse modes
      at every combination of 22/24/33/48/56px and 320/640/960px run width: 30 combinations.
      Add 390×844 viewport at 48px/320px in each mode: 32 combinations total.
- [x] `/design/indopak?audit=flow`: all 116 optional-ayah verses containing 121 occurrences,
      with original same-surah neighbors. Reading mode, same 15 size/width combinations,
      plus 390×844 viewport at 48px/320px: 16 combinations.
- [x] Compare exact rendered text slices and ordered private codes with original DB strings;
      preserve whitespace and format controls. Verify expected counts on every matrix run.
- [ ] Check final Arabic word, intervening annotations and ayah ornament stay together;
      Arabic digits remain enclosed; no horizontal overflow, clipped ink or line collision.
- [x] Paint all 1,383 original neighboring-word excerpts using local diagnostic fonts.
      Capture actual DOM for all E003/E004/E021 contexts, all 28 changed inline pause groups,
      candidate collisions and critical terminal chains. Include surrounding lines and padding.
- [x] Validate transparent negative controls, production/diagnostic geometry, ordinary-text
      reconstruction and private ink before interpreting overlap measurements. Calibrate any
      raster-edge tolerance for native screenshot scale and freeze it before reviewing failures.
      Existing WebKit's two-pixel tolerance does not automatically apply to every device/DPR.

Alias caveat: E003 shares its glyph with ordinary U+0656, including in 35:11; E022 shares
U+08D6, absent from this corpus. Same-glyph masks may aggregate instances. Resolve such
results using original captures and visual inspection; a mask pass alone cannot certify them.

### Critical visual checklist

Inspect at normal reading size and enlarged size, in both modes, light/dark themes and narrow
width. For 2:101 and dense pause chains, resize through the actual wrap boundary. Capture
before, at and after wrapping; confirm final word and ornament move together.

| Verses                             | Required observation                                                                   |
| ---------------------------------- | -------------------------------------------------------------------------------------- |
| **2:101**                          | Arabic `١٠١` stays inside ornament; pause remains clear; final word never orphaned     |
| 2:9, 2:10, 2:99, 2:100, 2:286      | One-, two-, three-digit ornaments remain centered and enclosed                         |
| **17:7**                           | E004 inverted damma visible, attached to intended context, clear of letters/marks      |
| 5:7, 6:46, 35:2, 35:11             | Every E003 occurrence visible below correct word; no clipping                          |
| 1:7, 3:4, 5:23, 97:3, 106:4, 114:4 | E021 optional-ayah 5 clear of pause signs and adjacent lines                           |
| **6:165**                          | Inline private zain and ordinary pause remain distinct, original order preserved       |
| 16:6, 73:17, 51:54, 79:27          | Sad/qaf/zain/waqfa ending chains visible above ornament with clear spacing             |
| 26:51, 43:15                       | Final Arabic word grouped with annotations and marker                                  |
| Every mapping entry                | All nine PUA codes covered, including qif and ruku; use recorded representative verses |

Expected counts: E003=4, E004=1, E01A=242, E01B=166, E01C=145, E01E=131,
E01F=20, E021=121, E022=553. Source: [mapping manifest](../scripts/fonts/indopak/mapping.json).

## 4. Real iPhone/iPad Safari

- [ ] Open specimen from device using a reachable Mac LAN address and dev server bound to
      that interface, or an approved private test origin. `localhost` on device refers to device.
      Keep diagnostic specimen private; public production specimen must remain 404.
- [ ] Where available, run native device WebDriver through a trusted, unlocked, connected
      device with Remote Automation enabled. Otherwise perform manual checks and record
      precise coverage; automation unavailability must not turn into a device pass.
- [ ] Sweep all 1,243 specimen verses in both modes at device's natural portrait and
      landscape width, minimum/default/maximum offered reader sizes. Review all private
      occurrences; inspect every optional-ayah context with its original neighboring verses.
      Record automated text/count checks separately from visual coverage.
- [ ] Run critical checklist above at 22/33/56px where offered. On iPad also test smallest
      available Safari window/split-view width. Record actual inner width for each state.
- [ ] In ordinary Safari browsing sessions, test pinch zoom, Safari page zoom, supported
      text-size controls, toolbar expansion/collapse, rotation, scroll, background/foreground,
      back/forward restoration, theme changes and script switching. Inspect after each change.
- [ ] Confirm selectable/copyable Arabic text retains original characters and annotation
      order. Keep copied samples in audit evidence; do not normalize them into a new baseline.
- [ ] Capture screenshots directly on device, retaining native resolution and metadata.
      Simulator/emulated viewport screenshots receive their own platform labels.

Device WebDriver requires host/device setup; its isolated automation sessions suppress some
system interactions. Ordinary-session gesture and persistence checks therefore remain part
of device testing.
([WebKit iOS WebDriver documentation](https://webkit.org/blog/9395/webdriver-is-coming-to-safari-in-ios-13/))

## 5. Font delivery and production reader

- [ ] Test fresh browser/site storage, warm cache, reload and return visit in ordinary Safari.
      If an older deployment is available, test v2-cached client loading v3 candidate.
      Confirm downloaded asset is `indopak-reader-compat-v3.woff2` (71,036 bytes at baseline),
      font face loads successfully and actual text paints private glyphs. Computed family alone
      cannot prove glyph coverage. Compare fetched font bytes with packaged font bytes.
- [ ] Test slow font delivery and failed font request. Capture initial and settled rendering.
      Record missing private symbols/fallback behavior explicitly; do not call readable Latin/UI
      fallback a Quran-symbol pass. Any fix needs a new candidate and regression rerun.
- [ ] Build/serve production candidate separately. Use actual reader links and controls to
      repeat critical verses in reading/verse modes, light/dark, small/large sizes and translation
      context. Test script toggling, navigation, settings reopen and back/forward restoration.
- [ ] Sweep all 114 surahs in actual production reader at default size in each mode on native
      desktop Safari. Walk every rendered page/section if rendering is paginated or virtualized;
      verify all 6,236 verses were visited. Record device critical-reader coverage separately.
- [ ] Check internal navigation/font resources resolve and translation context persists.
      `/design/indopak`, `?audit=all`, `?audit=flow` must each return 404 in production.

For implementation/fix candidates, complete `pnpm --dir web check`, `pnpm --dir web lint`,
`pnpm --dir web test`, font audits/tests and production build before browser approval.
Documentation-only planning does not constitute a new implementation test run.

## 6. Quran.com verification and completion

- [ ] In same native Safari/device, select Quran.com's IndoPak script explicitly. Record
      observation date, actual loaded font family/resource and whether compared strings are
      current served words or legacy `textIndopak` fields.
- [ ] Review all 201 previously sampled verse pages covering 223 context classes on desktop
      Safari. Repeat critical checklist on real devices. Compare symbol identity, context/order,
      attachment and readable separation. Font artwork/line breaks differ between Noto and
      Quran.com's Nastaleeq; screenshot pixel equality is not an acceptance criterion.
- [ ] Keep known source differences visible: 4:142 extra local waw+sukun, 12:1 missing final
      kasra, 2:10 mark association, 7:206 marker order, six rub-el-hizb additions, and served-word
      versus legacy annotation changes. See [editorial findings](indopak-deep-audit.md#source-discrepancies-requiring-editorial-review).
      New differences require review; never patch immutable DB text to match a screenshot.

Rendering pass requires: all required environments completed, exact text/counts
preserved, correct v3 font delivered, no missing/clipped private ink, no confirmed letter/mark
collision, no escaped digits or orphaned endings, and every diagnostic candidate resolved
with retained evidence. Missing required environment means partial coverage, with that row open.

For each failure retain verse/code, platform, mode/size/width/zoom, original text, expected
behavior, screenshot including neighbors, font/network state and reproduction steps. Correct
font/CSS/renderer defect, rerun failing case and affected full matrix, then critical device checks.
Never widen screenshot tolerances merely to make a failure pass.

Finish with a results table of **pass / fail / untested**, checked commit and evidence paths.
Editorial discrepancies remain open until qualified review against original printed-edition
provenance. Safari rendering approval must not be reported as perfect Quran.com parity or
recitation/text certification.
