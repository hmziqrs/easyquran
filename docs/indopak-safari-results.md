# IndoPak Safari test results — 2026-10-06

**Partial coverage. Not a Safari rendering approval.** Native macOS Safari 27.0 and Playwright
WebKit pass every mechanical and ink check in the full specimen matrices. Layout review items
A–E stay open. Older Safari, real iPhone/iPad, production reader and Quran.com rows remain
untested. Plan: [Safari testing plan](indopak-safari-testing-plan.md). Proposed fix
direction, copying Quran.com's word-box layout and font: [Quran.com study](indopak-qurancom-rendering-study.md). Earlier results and
editorial findings: [deep audit](indopak-deep-audit.md).

## Run record

| Item          | Value                                                                                                  |
| ------------- | ------------------------------------------------------------------------------------------------------ |
| Candidate     | `6486488` renderer/font (unchanged); harness changes in this commit                                    |
| Database      | `quran-indopak`, opened read-only; 6,236 verses, 1,218 private verses, 1,383 private occurrences       |
| Font          | `/fonts/indopak-reader-compat-v3.woff2`, 71,036 bytes; served bytes identical to package (both runs)   |
| Native Safari | Safari 27.0, safaridriver "Included with Safari 27.0 (22625.1.29.11.27)", macOS 27.0 (26A428), Mac15,8 |
| WebKit        | Playwright 1.63.0, WebKit 26.6, headless, DPR 1                                                        |
| Safari layout | `devicePixelRatio` 2, visual viewport scale 1, page zoom 100%; system dark mode                        |
| Viewports     | Requested 1100×900 and 390×844; measured inner viewport exact for both                                 |
| Origin        | `http://localhost:5391` dev server, `/design/indopak?audit=all` and `?audit=flow`                      |
| Time          | 2026-10-05T21:20Z–22:30Z UTC (2026-10-06 local, UTC+5)                                                 |

Font face status `loaded`, and `document.fonts.check` covers all nine private codes. Computed
family alone is not used as glyph evidence: DOM ink captures paint the production face.

## Results

| Environment                         | Result       | Coverage                                                                                     |
| ----------------------------------- | ------------ | -------------------------------------------------------------------------------------------- |
| Installed macOS Safari 27.0         | Pass\*       | 48 matrices, 1,383 raster occurrences, 307 DOM ink cases, 16 critical states, 32 wrap sweeps |
| Playwright WebKit 26.6              | Pass\*       | 48 matrices, 1,383 raster occurrences, 307 DOM ink cases; reproduced prior v3 result         |
| Previous Safari/macOS               | **Untested** | Single host; only Safari 27.0 available                                                      |
| Real iPhone, current iOS            | **Untested** | No device connected; simulators not run                                                      |
| Real iPhone, oldest supported iOS   | **Untested** | Supported version not yet chosen                                                             |
| Real iPad, current iPadOS           | **Untested** | No device connected                                                                          |
| Production reader in Safari (§5)    | **Untested** | Slow/failed font, cache, 114-surah sweep, navigation and 404 checks open                     |
| Quran.com comparison in Safari (§6) | **Untested** | Earlier 201-page comparison was Chromium only                                                |

\* Every mechanical/ink assertion passes; layout review items A–E below stay open. Matrix =
reading and verse modes × 22/24/33/48/56 px × 320/640/960 px plus 390×844 at 48 px (32), and
flow reading × same sizes/widths plus phone (16). Each matrix asserts exact DOM strings against
the DB, all private counts (1,383 per `audit=all` run), unsplit final word, ornament ≤ 1.05 em
and final-word fit.

Ink: all 1,383 neighboring-word excerpts paint visible, unclipped private ink in both engines.
The transparent negative control passes and outside-mask geometry is unchanged. Raw canvas
flags the same 10 pause pairs in both engines (15:79, 26:51, 43:15, 44:47, 56:40, 70:35, 73:17,
75:30, 76:30, 79:26). DOM ink resolves all of them: 190 `audit=all` + 117 flow cases, each
with production-font capture. Zero invisible, zero geometry mismatch, zero overlap, zero
boundary candidates.

Critical checklist (Safari): both modes × light/dark × 33/56 px × 320/640 px, 25 verses per
state, including every mapping-entry representative. 2:9, 2:99 and 2:100 contain no private
code, so they are absent from the specimen. One-/two-/three-digit ornament controls therefore
move to the production-reader pass. Wrap sweeps narrow 2:101, 6:165, 16:6, 73:17, 51:54,
79:27, 26:51 and 43:15 from 960 to 200 px in 2 px steps, at 33/56 px, both modes. Final word
and ornament never separate, and the final word never splits. Captures before/at/after the
first wrap boundary are retained.

Agent visual review (not a human editorial review) covered:

- 2:101 at its wrap boundary (33/56 px): `١٠١` enclosed, zain clear
- 17:7 E004 at native resolution: visible, attached, clear of letters/madda
- the full light 33/640 state
- ten dark 56/320 verses (1:7, 5:7, 6:165, 16:6, 17:7, 35:11, 51:54, 73:17, 79:27, 97:3)

The remaining 365 of 400 critical captures were not individually inspected.

## Review items

**A. WebKit/Safari final-group overflow (renderer defect candidate).** The nowrap
`.indopak-final-word` (last word, annotations, ornament) sits 4–9 px past the run's line end
in 18 specimen verses / 38 matrix cells. Examples: 71:23 at 22/320, 88:21 at 33/320, 4:142
at 56/320, 33:7 at 48/960. Chromium wraps the same groups. WebKit underestimates the shaped
width of a nowrap run containing the inline-block ornament. Kerning-off has no effect,
ligatures-off halves the overflow, and hiding the ornament or allowing a break removes it.
Ink is not clipped in the specimen (16 px padding), but crosses the text column edge.

Prototype `.indopak-final-word { display: inline-block }`, injected in Playwright only, not
applied:

- WebKit overflow drops 46 → 8 cells (4 verses), leaving only items B/C, the same set as Chromium.
- Chromium is pixel-identical in all 24 sampled captures (12 critical verses × 33/640, 56/320).
- WebKit shows subpixel shifts, and 4:142 wraps like Chromium.

Applying it needs a new candidate, the three web gates, and a full WebKit/Safari rerun.

**B. End-sign stacks containing EM SPACE.** Eleven verses carry U+2003 inside the terminal
annotation run: 69:46, 83:29–31, 84:22–23, 87:16, 89:27, 90:11, 91:1, 104:4. Private
pause-sign spans keep natural width, so the 1 em gap pushes zain/qaf/sad well off the
ornament. At line end, ink crosses the run edge by up to 12 px (83:31 22/320; 89:27, 91:1
56/320), in both engines. DOM text must keep U+2003. A display fix (for example fixed
private-sign widths like ordinary marks) also touches the 28 audited inline groups and needs
design review.

**C. 18:110 source cluster.** The DB joins `صَالِحًاوَّلَايُشۡرِكۡ` without spaces. The
unbreakable run is about 298 px at 56 px, overflowing a 286 px run by 12.7 px in both engines.
This is an immutable-source property; breaking inside words is not acceptable.

**D. Ordinary standalone pause marks wrap alone.** A space-separated ordinary sign, for
example U+0615 in 79:27 (`السَّمَآءُ` + ZWSP + space + U+0615 + space) or 35:11, can begin the
next line detached from its word. Both engines. Not a private code. Grouping candidate
analogous to v3's final-word grouping.

**E. Narrow-run limit.** At 56 px, runs narrower than about 224 px (article ≤ 258 px) cannot
hold the final group of 2:101, 16:6 or 26:51, which then overflows. It stays whole and
ornament-attached. This is below the 320 px matrix minimum and is recorded only.

The new run/page horizontal-overflow check (both harnesses) finds A–C. Safari's overflow sets
equal Playwright WebKit's in all 48 matrices.

## Harness notes

New: [`safari-native-check.mjs`](../scripts/fonts/indopak/safari-native-check.mjs) (W3C
WebDriver over `/usr/bin/safaridriver`) and
[`deep-browser-shared.mjs`](../scripts/fonts/indopak/deep-browser-shared.mjs) (assertions
shared with Playwright). After the refactor, the Playwright WebKit paint report is
byte-identical and the DOM ink/matrix results are identical to the pre-refactor baseline.

Safari capture problems were each caught by validity checks, never counted as passes:

- **Occluded window:** WebKit throttles `requestAnimationFrame` there, which stalled one run.
  Frame waits are now timer-bounded. Occluded captures are pixel-identical to earlier ones.
- **Viewport clipping:** element screenshots clip to the viewport. The element is now
  scrolled to the top and the viewport grows in height only (width and line breaking fixed,
  up to 4,373 px). Each PNG must equal element box × DPR.
- **Edge fill:** for a fractional element top, Safari adds one pure opaque black edge row
  (36 + 41 captures). The analyzer crops it only when identical across all four images of a
  case and full-width pure black (audit background is forced white). The count is recorded,
  and any other difference stays invalid.
- **Edge tolerance:** stays 2 CSS px, scaled by DPR, frozen before review. Overlap threshold
  stays 3 raw device pixels, stricter at DPR 2.

## Evidence

Ignored, under `.cache/indopak-safari/`:

- `2026-10-06-webkit/`: plan §2 baseline, unmodified harness at `6486488`
- `2026-10-06-webkit-shared/`: refactor parity
- `2026-10-06-safari/`: `safari-report.json`, `safari-flow-report.json`,
  `safari-*dom-report.json`, `safari-dom-*`, `safari-critical-*`, `safari-wrap-*`, logs and
  `run.txt`
- `2026-10-06-safari-smoke*/`: occlusion and tall-capture checks
- `probe-*`, `fixA/`: item A–D investigations

No DB writes, normalization, versioning or Quran-data SHA-256. Remote automation was off
before testing and was enabled by the host owner for this run.
