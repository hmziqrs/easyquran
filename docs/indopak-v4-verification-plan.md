# IndoPak v4 verification plan (Quran.com reference)

Created 2026-10-06 for candidate `584569c` ("Build Lateef-based IndoPak font v4 with Quran.com-style
word layout"). **Plan only. Nothing below counts as passed until its evidence exists.** It supersedes
the v3-era [Safari testing plan](indopak-safari-testing-plan.md). That plan's device, delivery and
Quran.com sections are folded in here and extended to the new font and layout.

Background: [Quran.com study](indopak-qurancom-rendering-study.md) (what Quran.com does and why v4
copies it), [font record](indopak-font-compatibility.md#version-4-2026-10-06-sil-lateef-base-qurancom-style-layout),
[v3 Safari results](indopak-safari-results.md), [deep audit and source discrepancies](indopak-deep-audit.md),
[tooling](../scripts/fonts/indopak/README.md).

## 0. What "verified to standard" means

v4 is approved only when every row below has evidence. A missing platform is **untested**, never
passed. Editorial and recitation approval stay separate from rendering approval (§9).

| #   | Requirement           | Measurable acceptance                                                                                                                                                                                                                 |
| --- | --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R1  | Text integrity        | DOM text of every verse equals the DB string exactly, in every engine, mode, size and width. Copy/paste returns the same code points. No normalization, ever                                                                          |
| R2  | Coverage              | Zero `.notdef` across 6,236 verses (TTF, WOFF2, every browser). All 9 private codes, U+FE8E, U+2003 and U+200B handled                                                                                                                |
| R3  | Sign visibility       | Every one of the 1,383 private occurrences paints visible, unclipped ink, both in the canvas excerpt and in the real DOM                                                                                                              |
| R4  | No collisions         | Zero DOM overlap/boundary candidates after review. Any accepted exception is listed by verse, code, engine and reason, with its capture                                                                                               |
| R5  | Line breaking         | Breaks occur only at source whitespace between word boxes, or after a ZWSP-joined sign. No pause sign starts a line. The final word and its ayah marker never separate. The final word is never split                                 |
| R6  | Overflow              | No horizontal overflow at the matrix sizes (22–56px) and widths (320–960px), except unbreakable source words wider than the column. Those are listed with their measured width                                                        |
| R7  | End-of-ayah           | Ring encloses its digits (1–3). Digits are Urdu-form Extended Arabic-Indic. Trailing signs are stacked, centred above the ring, clear of the ring, the word and the previous line                                                     |
| R8  | Quran.com parity      | Sign identity and order match Quran.com's legacy `textIndopak` for every compared verse; differences are only those already recorded as source discrepancies. Word boundaries match Quran.com words except documented joined clusters |
| R9  | Visual similarity     | Side-by-side review sheets (§5.4) scored by the owner and, where possible, a qualified reviewer. Scores are recorded per category; no category is "worse than v3"                                                                     |
| R10 | Engine/platform reach | Chromium, Playwright WebKit, native macOS Safari, Firefox (Gecko), iOS Safari, iPadOS Safari and Android Chrome all pass R1–R7. Each platform is recorded with exact versions                                                         |
| R11 | Delivery              | Production serves `indopak-reader-compat-v4.woff2` with byte equality to the package. Behaviour on older browsers without `size-adjust`, and on slow or failed font loads, is defined and acceptable                                  |
| R12 | Licensing             | OFL text and FONTLOG ship with the font. Reserved Font Names "Lateef" and "SIL" never appear in our font names. No restricted font asset (QuranWBW, PDMS, KFGQPC) in repo or build                                                    |
| R13 | Hard rules            | DBs untouched (read-only `mode=ro&immutable=1`), no Quran-data SHA-256, `db/` untracked, three web gates green                                                                                                                        |

## 1. Current state at `584569c`

Already done (re-run in §2 anyway, because the candidate changed after some of these runs):

- **Font:** v4 built from pinned Lateef SemiBold. 11 font unit tests pass. `audit.py` reports
  zero missing glyphs. `validate.py` shapes all 6,236 verses with 0 `.notdef`, identical
  TTF/WOFF2 traces, and 5,015 unaffected verses identical to upstream Lateef.
- **Web gates:** `pnpm check` 0/0, `pnpm lint` exit 0, `pnpm test` 2,185 passed. These ran
  before the last CSS tweak; only the 178 reader tests were re-run after it.
- **Pre-fix browser run** (`.cache/indopak-safari/2026-10-06-v4-engines/`):
  - Chromium 153 and WebKit 26.6 pass all 32 matrices each, with exact text and 1,383/1,383
    occurrences.
  - Overflow only in 2 cells (56px at a 320px run), on 12:21, 19:17 and 91:14.
  - DOM ink is valid (316 / 312 cases, 0 invisible, 0 geometry mismatch, 0 boundary).
- **Open issues from that run:**
  - DOM overlap candidates: Chromium 28, WebKit 20. The private signs (E01C qaf, E01A zain,
    E01E qif) touch either a following seli/lam-alef sign or the madda of the preceding word.
  - The candidate now carries fixes, unverified: 123-unit sign side bearings, 0.16em gap
    between inline signs, 0.36em end-sign rows.
  - Native Safari's v4 run was stopped before completion.
  - Firefox was never run.
- **Raw canvas candidates rose** (Chromium 145, WebKit 102, against 10 for v3). The canvas paints
  each excerpt as one run without word boxes, so it no longer models the renderer (§4.1).

## 2. Phase A — Freeze the candidate and rerun static gates

Owner: agent. Time: about 30 minutes.

- [ ] Record the candidate commit, `git status` (only `.claude/launch.json` may be untracked),
      Node, pnpm, Playwright, Python, fontTools, HarfBuzz and macOS versions in `.cache/indopak-v4/<run>/env.json`.
- [ ] Reproducible build: run `build.py` twice into two temp dirs. TTF and WOFF2 must be
      byte-identical to each other and to `web/static/fonts/indopak-reader-compat-v4.*`
      (`outputs.json` sizes and checksums; font checksums are not Quran data).
- [ ] Font tests, coverage and shaping:

  ```sh
  .cache/indopak-venv/bin/python -m unittest discover -s scripts/fonts/indopak -p 'test_*.py'
  .cache/indopak-venv/bin/python scripts/fonts/indopak/audit.py \
    --font web/static/fonts/indopak-reader-compat-v4.ttf \
    --font web/static/fonts/indopak-reader-compat-v4.woff2
  .cache/indopak-venv/bin/python scripts/fonts/indopak/validate.py \
    --upstream .cache/fonts/indopak/Lateef-SemiBold.ttf
  ```

- [ ] Web gates: `pnpm --dir web check`, `pnpm --dir web lint`, `pnpm --dir web test`. All three
      must be green; a warning is a failure.
- [ ] Licence/name audit: dump every `name` record. "Lateef" and "SIL" may appear only in
      copyright, trademark and licence records (IDs 0, 7, 13). OFL file is present beside the
      font. FONTLOG matches `build.py` constants (side bearing 123, ring stroke 110, E004 and E021
      offsets).

Pass: all green. Fail: fix at source and restart Phase A.

## 3. Phase B — Renderer logic over the whole corpus (new tooling)

Owner: agent. Time: about 2 hours including tooling. Unit tests cover 13 hand-picked strings;
this phase proves the segmentation rules on all 6,236 immutable strings.

- [ ] **Build `scripts/fonts/indopak/segmentation_audit.mjs`.** It reads the DB read-only through
      `node:sqlite`, imports `indopakEnding` from `web/src/lib/quran/view/indopak.ts` (via the
      Vite/tsx loader), and writes `segmentation-report.json`. It asserts per verse:
  - Words + gaps + final word + suffix concatenate to the exact DB string (R1).
  - Every word box contains at least one letter, except an explicitly listed leading-sign case.
    No box starts with a sign that had whitespace before it (R5).
  - Splits happen only at whitespace, or right after `U+200B` + signs followed by a letter.
    List every ZWSP split with verse and offset for review.
  - The final word contains a letter. Its trailing whitespace goes only to `lastWordSpace`.
    End annotations reassemble to `sign + following`.
  - The `stop` flag is true exactly when the box holds a Quran.com stop sign or a private
    pause/ruku code.
  - Every annotation that needs an ink box has one (zero-advance marks), and none is applied
    to a spacing glyph.
- [ ] **Distribution report**: word-box count per verse, longest box in em at 56px (from
      HarfBuzz advances × 1.25), ZWSP-split count, stop-box count, multi-sign end stacks (count
      and maximum height). The longest boxes predict R6 overflow cases before any browser runs.
- [ ] **Context coverage**: map the 223 context classes in `inventory.json` to concrete verses,
      so every class appears in Phase C captures.
- [ ] Add a DB-free unit test for each new edge case the audit discovers (`indopak-ayah.test.ts`).

Pass: zero assertion failures; every listed exception reviewed and justified in the report.

## 4. Phase C — Browser mechanical and ink matrix

Owner: agent; the host owner keeps Safari "Allow remote automation" on during runs. Time: about
3 hours of machine time.

### 4.1 Harness updates before running

- [ ] **Word-box-aware paint diagnostics.** Make `paintOccurrences` draw each excerpt through the
      same segmentation as the renderer, either as separate runs per word box with the CSS gaps
      or through DOM capture only. Raw canvas candidates must again mean something. Keep the
      old single-run paint as a separate "shaping-only" report.
- [ ] **New DOM assertions in `inspectSpecimens`** (both runners):
  - No `.indopak-word` box starts a line with only signs.
  - Each `.indopak-end-sign` row's ink box sits above the ring top with at least 0.02em
    clearance and is horizontally centred within ±0.05em of the ring centre.
  - Stacked end signs don't cross the previous line's ink box.
  - The ornament box is ≤ 1.15em and the digits' ink lies inside the ring's inner circle
    (sampled by canvas at the ornament rect).
- [ ] **Clipping check in the real reader** (not only the specimen): scroll rows of the virtual
      list (`ReaderVirtualList`) and the page reader; confirm stacked signs on a row's first line
      are not clipped by row boundaries or `contain` rules.
- [ ] Record `getComputedStyle` font descriptors: `size-adjust` honoured
      (`document.fonts.check` plus measured advance of a known word equals 1.25× upstream).

### 4.2 Runs

For each engine: `audit=all` (both modes × 22/24/33/48/56px × 320/640/960px + 390×844) and
`audit=flow` (reading mode, same sizes and widths + phone). Then DOM ink analysis with the frozen
tolerances: 2 CSS px edge tolerance × DPR, overlap ≥ 3 raw device px, capture-fill cropping only
for identical pure-black full-width edge lines.

- [ ] Chromium (Playwright): `INDOPAK_DEEP_ENGINES=chromium node scripts/fonts/indopak/deep-browser-check.mjs`, flow, ink analysis.
- [ ] WebKit (Playwright): same with `webkit`.
- [ ] Native macOS Safari: `node scripts/fonts/indopak/safari-native-check.mjs` plus flow, including
      the 16 critical states (light/dark × 33/56px × 320/640px × both modes) and wrap sweeps.
- [ ] Specific v4 re-checks:
  - the 28/20 previous overlap cases (3:193, 4:143, 18:18, 30:25, 77:23, 2:168, 4:140, 4:141,
    6:146, 7:83, 10:16, 11:46, 21:92, 46:26, 3:98, 3:119, 3:173, 6:138, 29:46, 29:60, 37:31,
    40:28, 43:81, 58:1, 6:141, 7:195, 46:23, 73:20, 19:1, 51:54)
  - the three overflow verses 12:21, 19:17 and 91:14
  - 17:7 E004 compared with its U+0657 diagnostic in all three engines (bidi itemization differs
    by engine)
  - E021 optional-ayah lanes across all 116 flow verses
  - E022 ruku alone in an end stack in WebKit/Safari (zero-advance mark visibility)

Pass (per engine):

- every matrix asserts R1/R2/R5
- DOM ink has 0 invisible and 0 geometry mismatches
- overlap/boundary candidates are 0 after review, or each is accepted in §10
- overflow is limited to Phase B's predicted unbreakable words

## 5. Phase D — Quran.com reference comparison

Owner: agent, with owner review. Reference conditions are recorded for every capture: Quran.com
build id (`__NEXT_DATA__.buildId`), font URL/version/bytes, `quranFont=text_indopak`,
`mushafLines`, date and engine. Reference fonts and screenshots stay in ignored `.cache/` only.
Quran.com is a comparison source, not proof of the printed edition. Respect its rate limits:
two concurrent requests at most.

### 5.1 Sign identity and order — full corpus

- [ ] **Decide the data source:**
  - Quran Foundation API (needs the owner's Developer Console account; full 6,236 verses, words
    with `text_indopak`)
  - or per-verse pages' `__NEXT_DATA__` (6,236 pages; slow but public)
- [ ] **Build the comparator** (extend `compare_reference.py`): for every verse, compare our
      ordered private-code sequence with Quran.com's legacy `textIndopak` (exact) and with its
      served `word.text` through the observed code table (E01A↔U+0617, E01B↔U+06EA,
      E01C↔U+06D7, E01E↔U+06EB, E01F↔U+06E5, …). That table describes their font's encoding,
      not Unicode equivalence.
- [ ] Output `full-reference-report.json`: per-verse match, with classified differences:
      already recorded (4:142, 12:1, 2:10, 7:206, six rub-el-hizb) / new / served-vs-legacy only.

Pass: every difference is classified. New source differences go to §9 editorial review and are
never "fixed" in the DB.

### 5.2 Word-boundary parity

- [ ] For each verse, compare our word boxes (Phase B output) with Quran.com's word list
      (positions, `char_type_name=word`; end-marker words excluded). Map by order, comparing
      letters only, with marks and signs stripped for the comparison only.
- [ ] Report verses whose box count differs, with the cause: joined source clusters (18:110
      type), ZWSP joins, signs attached to a different word, or Quran.com splitting where our
      source has no whitespace.

Pass: every mismatch has a cause. Attachment mismatches where the source supports
Quran.com's attachment become renderer bugs for §10.

### 5.3 Line-break parity (same width, same size)

- [ ] **Conditions:** Quran.com single-ayah page at 390×844 (26px, 318px column), 320×640, and
      1280×900, in Chromium, WebKit and Safari. Our reader runs at the same CSS width and size
      (26px with `size-adjust` 125%).
- [ ] **Sample:** the 116 flow verses, all critical verses, and a stratified random 300 verses
      (seeded, listed in the report).
- [ ] Record the first word of each line in both readers. Metrics: % verses with identical breaks,
      mean line-count difference, and verses where ours wraps earlier or later by more than one
      word.
- [ ] **Tracked, not gating:** font metrics legitimately differ. Investigate any verse whose
      line count differs by 2 or more (spacing too wide or narrow) and any case where Quran.com
      fits a final word plus marker on a line that we wrap.

### 5.4 Visual review sheets (human-scored)

- [ ] For each of the 9 private codes, a representative of every context class, plus ordinary
      pause signs, generate side-by-side crops: Quran.com | ours v4 | ours v3. Use the same
      text size, light theme, 2× DPR. Group them into sheets of 12 (`.cache/indopak-v4/review/*.png`)
      with an index HTML.
- [ ] **Categories scored 1–5 by the owner** (and a reviewer if available):
  - letterforms (lam, qaf, kaf, yeh, heh, lam-alef, final nun)
  - stroke weight and colour
  - inline pause-sign size and placement
  - end-sign stacking
  - ring and digits
  - word spacing
  - overall line rhythm
- [ ] **Targeted pairs:** 2:101, 1:7, 17:7, 6:165, 79:27, 89:27, 83:31, 4:142, 2:286, 18:110,
      71:23, 91:1, 104:4, 16:6, 73:17, 51:54, 26:51, 43:15, 35:11, 5:7.

Pass: no category below 3 without a recorded follow-up, and no category scored worse than v3.

### 5.5 Known intentional differences (documented, not defects)

- The final word keeps its marker; Quran.com can orphan it.
- Source-text differences listed in the deep audit.
- Glyph artwork comes from Lateef, not QuranWBW: similar tradition, never traced.
- Text colour is our theme's foreground; Quran.com uses dark grey.

## 6. Phase E — Firefox (Gecko)

Automated Firefox cannot launch on this Mac (Playwright Firefox, geckodriver and headless
Firefox all exit at startup).

- [ ] **Option 1 (preferred):** a Linux host or container (OrbStack is installed) running
      Playwright Firefox against the dev server. Run the full Phase C matrix and ink analysis
      (`--engine firefox` support must be added to `dom_ink_analysis.py`).
- [ ] **Option 2:** another Mac or Windows machine with geckodriver.
- [ ] **Option 3 (minimum):** the owner opens the specimen and the critical verses in their own
      Firefox and supplies screenshots. Recorded as manual coverage only.

Pass: R1–R7 in Gecko, or Firefox stays explicitly **untested**.

## 7. Phase F — Real devices

Owner: owner (devices) plus agent (pages, scripts, analysis).

- [ ] **Decide the support floor.** `size-adjust` needs Safari/iOS 17+, Chrome 92+ and
      Firefox 92+. On older engines the text renders 20% smaller.
  - Either accept that and document it,
  - or add a fallback: `@supports not (size-adjust: 125%)` cannot detect a descriptor, so the
    fallback would be a measured `font-size` multiplier on `.arabic-text.indopak` when
    `document.fonts` reports no adjustment.
  - Decide before testing the oldest iOS.
- [ ] **Serve the dev server on the LAN** (`pnpm --dir web dev --host 0.0.0.0 --port 5391`). Open
      the private specimen from the devices; it stays 404 in production.
- [ ] **iPhone (current iOS and the oldest supported), iPad (current, split-view narrowest),
      Android phone (current Chrome):**
  - critical verses at the minimum, default and maximum reader sizes, portrait and landscape
  - full-specimen scroll sweep
  - pinch zoom, Safari text-size, rotation, dark mode
  - back/forward
  - copy/paste of a verse, comparing code points
- [ ] **Automation where available:** iOS Safari WebDriver (`safaridriver` with a paired device,
      `safari:useSimulator=false`) and Android Chrome via `adb` + Playwright `_android`.
      Simulator and emulator results are labelled as such and never stand in for a device pass.

## 8. Phase G — Production reader and delivery

Owner: agent. Time: about 2 hours.

- [ ] `just web-build prod` (or `pnpm --dir web build`), then `pnpm --dir web start`.
- [ ] **Font delivery:**
  - The network panel shows `indopak-reader-compat-v4.woff2` with bytes equal to the package.
  - There is no request for v3 unless an old client is cached.
  - Cache headers are immutable.
- [ ] **Service worker:** the precache includes v4. Measure precache size: all v2/v3/v4 TTFs and
      WOFF2s are in `files`. Decide whether to drop TTFs and old versions from precache (perf
      ticket, not a blocker).
- [ ] **Font load:** a slow font (3G throttling) and a failed font request (blocked URL). Capture
      initial and settled rendering. `font-display: block` hides text for up to about 3s, then
      falls back. Record what fallback shows for private codes (expected: tofu or nothing).
      Decide whether that is acceptable or needs a fallback face.
- [ ] **Real reader routes**, both modes, light/dark, smallest/largest size:
  - Arabic `/{surah}` and `/page`, `/juz`, translated `/t/**` (SSR + disk cache)
  - script switching Uthmani ↔ IndoPak; reopening settings; back/forward restoration
  - search result snippets, bookmarks, verse tools; tooltip on the ring ornament
- [ ] **114-surah sweep** in production in Chromium and Safari at the default size, both modes.
      Visit every verse (6,236) through the virtual list, assert R1 per rendered verse, and
      capture a screenshot of each surah's first and last screen.
- [ ] **Accessibility:** `lang`/`dir` on verses and the ornament (`lang="ur"` digits).
      VoiceOver and TalkBack read the verse text (private-code behaviour recorded). Find-in-page
      (Cmd+F) finds words across box boundaries. Selection highlight spans whole verses.
- [ ] **Performance:** layout cost of about 15 inline-block spans per verse in long surahs (2,
      26). Long-task and CLS numbers before and after v4 on a mid-range Android.
- [ ] `/design/indopak`, `?audit=all` and `?audit=flow` return 404 in production.

## 9. Phase H — Editorial and recitation review (human)

Rendering approval is not editorial approval.

- [ ] A qualified reviewer, familiar with the South Asian (Taj/13- and 15-line) mushaf, reviews:
  - the Phase D sheets
  - the full list of source discrepancies (deep audit + §5.1 new ones)
  - the meaning and placement of each sign type: zain, sad, qaf, qif, waqfa, optional ayah,
    ruku, subscript alef, inverted damma
- [ ] Compare against a named printed edition (record publisher, line count and page images used).
- [ ] Outcomes are recorded per item: accepted / renderer change / font change / source issue.
      Source issues never change the immutable DB; they go to a separate data-provenance
      decision.

## 10. Correction loop and regression policy

| Finding type                                                | Fix location                                                | Rerun scope                                                       |
| ----------------------------------------------------------- | ----------------------------------------------------------- | ----------------------------------------------------------------- |
| Glyph geometry (side bearing, lane, E004/E021 offset, ring) | `build.py` constants → rebuild font, update FONTLOG         | Phase A + B (ink table) + full Phase C, all engines + D.4 sheets  |
| Renderer CSS (gaps, stacking, sizes)                        | `IndoPakAyah.svelte`                                        | Web gates + full Phase C + affected D.3/D.4                       |
| Segmentation rule                                           | `indopak.ts` + new unit test                                | Phase B + C + D.2                                                 |
| Harness artefact (capture fill, clipping, timing)           | harness only, with a validity check that rejects it         | Affected engine run; prove the artefact is rejected, never passed |
| Engine bug (one engine only)                                | Workaround only if it does not change other engines' pixels | That engine + regression on the others                            |
| Source-text difference                                      | none (document, §9)                                         | —                                                                 |

**Rules:**

- Never widen a tolerance or threshold to turn a failure into a pass.
- Every accepted exception names a verse, code, engine, measured value and capture.
- Tolerances are frozen per run before results are viewed.

## 11. Reporting

- [ ] `docs/indopak-v4-results.md`: a pass / fail / untested table for R1–R13 × platform, with
      exact versions, commit, evidence paths, and accepted exceptions with captures. Remaining
      gaps are stated in bold.
- [ ] Update the study doc (v4 implemented), the Safari results (v3 → superseded), the deep audit
      links, `scripts/fonts/indopak/README.md`, and machine reports
      (`browser-report.json`, `deep-audit-report.json`, `shaping-report.json`,
      `full-reference-report.json`).
- [ ] Commit evidence summaries only; screenshots and reference assets stay in ignored `.cache/`.

## 12. Order, owners and decisions needed

| Step | Phase                                  | Owner                 | Blocked on                      |
| ---- | -------------------------------------- | --------------------- | ------------------------------- |
| 1    | A — gates and font                     | agent                 | —                               |
| 2    | B — corpus segmentation                | agent                 | —                               |
| 3    | C — Chromium/WebKit/Safari             | agent                 | Safari remote automation on     |
| 4    | D.1–D.2 — reference identity and words | agent                 | API access decision             |
| 5    | D.3–D.4 — layout and visual sheets     | agent → owner scoring | —                               |
| 6    | G — production delivery                | agent                 | —                               |
| 7    | E — Firefox                            | agent                 | Linux container or another host |
| 8    | F — devices                            | owner + agent         | devices, support-floor decision |
| 9    | H — editorial                          | qualified reviewer    | reviewer, printed edition       |
| 10   | Report and sign-off                    | agent → owner         | all above                       |

**Decisions needed from the owner:**

1. Quran Foundation Developer Console account (full-corpus reference data) or page scraping.
2. Minimum supported browsers (`size-adjust` floor) and whether to add a fallback.
3. Firefox route (Linux container recommended).
4. Which devices are available.
5. Who the qualified reviewer is, and which printed edition is the reference.
6. Whether pure black text stays or matches Quran.com's dark grey.
