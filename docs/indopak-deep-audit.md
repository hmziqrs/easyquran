# IndoPak deep audit — 2026-10-06

This document preserves the v3 audit and source-provenance findings. Current v4 uses Lateef
and fresh four-engine/full-corpus evidence; see [v4 results](indopak-v4-results.md).
The full current Quran.com comparison classifies served encoding and word-boundary differences
for all 6,236 verses. Source/editorial discrepancies below remain open; the v3 Noto metrics
are historical and do not describe v4.

Version 3 repairs additional placement and wrapping defects found by auditing every private
occurrence. Measured Chromium/WebKit rendering passes. **Source-text discrepancies remain
unresolved; neither perfect parity with Quran.com nor editorial/recitation approval is claimed.**

## Scope and evidence

- Immutable local corpus: 6,236 verses, 727,385 characters, 87 distinct code points.
- Nine private codes, 1,383 occurrences across 1,218 verses; 1,257 distinct neighboring-word
  excerpts. Every occurrence is retained, including repeated contexts.
- Complete corpus shaping: zero `.notdef`, TTF/WOFF2 traces identical, 5,015 unaffected runs
  identical to original Noto. All original standard cmap mappings remain intact.
- Full browser specimen: 1,243 original verses, including repertoire controls. Reading and
  verse modes; 22/24/33/48/56px, 320/640/960px runs, and a 390×844 phone viewport.
- Continuous-reading specimen: all 116 verses containing the 121 optional-ayah occurrences,
  each with its original preceding/following verse from the same surah. Same size/width matrix.
- Live [Quran.com](https://quran.com/17/7): 201 verse pages covering all 223 recorded context
  classes. All 281 sampled private codes agree in **ordered sequence**, not merely totals,
  with the pages' legacy `textIndopak` fields. 191 legacy strings agree after comparison-only
  whitespace/format-control removal. Reader/DB strings are never normalized.
- Live font confirmed through an actual word's computed `IndoPak` family and fetched resource:
  [AlQuran IndoPak v4.2.1-WL](https://quran.com/fonts/quran/hafs/nastaleeq/indopak/indopak-nastaleeq-waqf-lazim-v4.2.1.woff2),
  83,560 bytes. Its nine private glyphs, cmap aliases, advances and bounds were re-inspected.
  Reference assets remain outside repository/build. Identity evidence also remains in the
  [original mapping audit](indopak-font-compatibility.md#encoding-provenance-and-semantic-limits).

Machine-readable findings: [deep report](../scripts/fonts/indopak/deep-audit-report.json),
[live reference comparison](../scripts/fonts/indopak/deep-reference-report.json).

## Defects found and repaired

1. **Optional-ayah 5:** version 2's horizontal lane cleared 1:7 but still touched pause signs
   in spaced/bidi contexts, including 3:4, 5:23, 97:3, 106:4 and 114:4. Version 3 retains the
   genuine Noto U+08E2 outline at half scale, zero advance and GDEF mark class; its translation
   changes from `(-250,820)` to `(-250,1120)`. The additional vertical lane clears all measured
   occurrences, including neighboring lines. Original U+08E2 and other ordinary glyphs stay intact.
2. **Terminal pause chains:** the renderer recognized only zain/ruku endings. It now handles
   all corpus terminal private pause signs, adjoining ordinary pauses, controls and whitespace.
   All 705 terminal private clusters retain exact source order above the ornament. Examples:
   16:6 (sad), 73:17 (qaf + ordinary pause), 51:54 (qaf + zain), 79:27 (waqfa).
3. **Orphaned ending:** 133 specimen verses grouped an annotation token with the number while
   leaving the actual final Arabic word breakable. The final group now includes that word and
   intervening annotation tokens. This also covers endings containing only ordinary pause signs.
4. **Inline pause pair:** 6:165's private zain touched the adjoining ordinary pause. The 28
   inline private/ordinary or private/private pause groups across 26 verses now use separate
   boxes, with zero-advance marks given their genuine Noto ink widths. Arabic letters are not
   split into individual spans. All text slices and DOM order remain exact.

Versioned `indopak-reader-compat-v3` assets and font metadata prevent reuse of the old cached
font by updated reader code. Existing v2 assets remain available for older cached clients.
Font software remains a renamed Noto derivative under OFL 1.1; no restricted outlines or
reference layout tables are imported.

## Source discrepancies requiring editorial review

These are stored/served text differences, not missing glyphs. DB immutability forbids silently
repairing them. Quran.com is a comparison source, not proof of this DB's original printed edition.

| Verse                                   | Local versus Quran.com legacy field                                                   |
| --------------------------------------- | ------------------------------------------------------------------------------------- |
| [4:142](https://quran.com/4/142)        | Local `خَادِعُوْهُمۡ`; reference `خَادِعُهُمۡ`: extra local waw + sukun.              |
| [12:1](https://quran.com/12/1)          | Local final `الۡمُبِيۡن`; reference includes final kasra, `الۡمُبِيۡنِ`.              |
| [2:10](https://quran.com/2/10)          | Different order/word association of optional-ayah, no-stop and small high meem marks. |
| [7:206](https://quran.com/7/206)        | Ruku and sajda markers occur in different source order.                               |
| 2:219, 2:233, 2:243, 3:171, 9:111, 13:5 | Reference additionally contains U+06DE rub-el-hizb marker.                            |

Current served `word.text` also differs from legacy `textIndopak`: many annotations move into
the end word and use different encoding. In [79:27](https://quran.com/79/27), legacy E01F waqfa
becomes U+06D9 no-stop in the served end word. In [51:54](https://quran.com/51/54), legacy
qaf/zain are absent from the served end word. [16:6](https://quran.com/16/6) moves sad to that
word; [73:17](https://quran.com/73/17) moves qaf and the adjoining pause. These distinctions
prevent claiming identical editorial text from matching legacy private codes.

Qualified review should reconcile these findings against the original publisher/printed
edition before the text is represented as fully verified. Original download revision and
printed-edition provenance remain unrecovered.

## What the browser measurements establish

Diagnostic fonts derive solely from the local OFL font. Transparent COLR layers isolate one
private glyph's ink while retaining original outlines, advances, cmap, GDEF, GSUB and GPOS.
Negative controls reject visible non-private ink in the ordinary-text control. Original/except/mask
reconstruction rejects geometry changes before overlap results are interpreted.

All 1,383 original neighboring-word excerpts paint visible, unclipped private ink in both
engines, with no unrelated geometry changes. Raw canvas runs still flag some pause pairs
whose real reader layout separates them; these are checked in the actual DOM, including
ornament ink. DOM captures include surrounding padding so locator cropping cannot masquerade
as glyph clipping; fixed/sticky UI is hidden only in the test page to avoid overlay artefacts.

Full matrices verify exact original DOM strings, all private counts, final-word/number
wrapping, ornament enclosure and width. Ink comparisons verify targeted marks, all optional
occurrences, candidate pause collisions, all changed inline groups, and continuous reading.
They do not certify linguistic correctness or identical Nastaleeq artwork.

An additional 46 comparisons check diagnostic rendering against the actual production font,
covering all nine codes and the critical repairs. Chromium agrees; WebKit requires a two-pixel
raster-edge tolerance because color and ordinary text produce slight ink-edge differences.
This is measured layout/outline agreement, not exact pixel parity. Masks are glyph-based:
E003 also selects standard U+0656 when present (35:11); all four E003 raw excerpts contain no
such alias. E022's standard U+08D6 alias is absent from the corpus. Same-glyph copies are
aggregated in masks, so automated overlap results supplement visual/editorial review.

## Platform and coverage limits

Chromium 153.0.8010.12 and Playwright WebKit 26.6 pass **96 layout matrices and 665 DOM ink
comparisons**, with no unresolved overlap/boundary candidates. Check, lint (deny warnings),
2,182 web tests, eight font tests and production build pass. All specimen variants return
HTTP 404 in production; served v3 WOFF2 bytes exactly match the package. Automated Firefox cannot launch on
this host; both Playwright and official geckodriver/native Firefox attempts failed. Native
Safari WebDriver is unavailable because remote automation is disabled. Prior focused native
Safari/Firefox observations concern v2, not a new v3 full-platform pass. No iOS-device pass.

Native macOS Safari 27.0 specimen pass and open layout review items: [Safari results](indopak-safari-results.md).
Remaining real-device/production coverage: [Safari testing plan](indopak-safari-testing-plan.md).

The 201-page Quran.com comparison is not a full 6,236-verse editorial/visual comparison.
The public legacy full-corpus endpoint returned HTTP 403; current official
[IndoPak API documentation](https://api-docs.quran.com/docs/content_apis_versioned/4.0.0/quran-verses-indopak/)
requires authenticated developer access. No access controls were bypassed.

All DB access in this audit is read-only. No DB writes, migrations, Quran SHA-256, identity
changes, DB staging or DB versioning. Reference fonts/screenshots remain outside repository;
local diagnostic artefacts remain under ignored `.cache/`. Reproduction commands:
[font tooling](../scripts/fonts/indopak/README.md).
