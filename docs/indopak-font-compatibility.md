# IndoPak font compatibility — preview, production blocked

Audit/build date: 2026-10-05. Concrete derivative and development integration are delivered.
Production acceptance is **not met**: the original E004 inverted damma in 17:7 loses correct
placement in browser shaping. The font is intentionally absent from production script/font
selection. Removing missing-glyph boxes is not approval.

## Repository and immutable baseline

Reader is SvelteKit/browser, not a native app. Script selection lives in reader settings;
font definitions are `web/src/lib/config/reader-fonts.ts`, lazy FontFace loading is
`web/src/lib/fonts/arabic-fonts.ts`, and `ReadingAyah.svelte` renders verse strings. Existing
default KFGQPC Uthmanic Hafs path, preference schema, source profiles and reader components
remain unchanged. No changes to search, copying, bookmarks, translations or audio alignment.

IndoPak DB is provisioned at `db/quran/arabic/quran-indopak.sqlite`, table `quran_text`.
Existing identity is `quran-indopak`; existing renderer profile is
`indopak-naveed-7d3c21e0`. Neither was changed or replaced by a checksum. DBs and importer
resources under ignored `db/` stay out of Git. New tooling/context lives under tracked
`scripts/fonts/indopak/`.

Before work, five Arabic DB byte sizes and manual SHA-512 snapshots were recorded outside
repository. Final manual audit confirms identical bytes/digests. Final read-only counts are
6,236 rows each; identical bytes also establish identical baseline counts. Recorded evidence:
[`preservation-report.json`](../scripts/fonts/indopak/preservation-report.json). These are
manual preservation observations, never DB identities or automated build/runtime hashes.
No Quran SHA-256 was computed, no DB opened for writing, no data migration performed.

| DB id                   | Bytes, before = after | Verse count, before = after |
| ----------------------- | --------------------: | --------------------------: |
| quran-indopak           |             1,634,304 |                       6,236 |
| quran-simple-clean      |               929,792 |                       6,236 |
| quran-tajweed           |             2,015,232 |                       6,236 |
| quran-uthmani-annotated |             1,605,632 |                       6,236 |
| quran-uthmani           |             1,593,344 |                       6,236 |

Corpus scan: **727,385 characters, 87 distinct code points, nine PUA codes, 1,383 PUA
occurrences**. Expected nine/1,383 baseline confirmed. All three Unicode PUA ranges scanned;
no supplementary PUA found. Inventory records counts, representative verse keys, category,
combining class and bidi class. Context signatures distinguish immediate boundary categories,
complete contiguous diacritic stacks before/after each PUA and adjacent PUA combinations.
223 such context classes are represented across 226 specimen verses, including additional
verses covering every corpus
character. Verse ornaments use the actual reader component, exposing end-of-verse adjacency.

## Encoding provenance and semantic limits

Ignored importer metadata identifies Naveed Ahmad's `Quran-text` `ayah-by-ayah/<sura>.json`
source and documents trimming leading/trailing whitespace. All 6,236 DB strings match local
mirror through that documented importer operation. Original remote path is no longer
available at observed upstream revision. Exact original download revision and printed mushaf
edition were **not recovered**. Current upstream revision is recorded only as an observation,
not falsely attributed to these DB bytes.

Original source README recommends PDMS Saleem for its PUA encoding. Legitimately available
PDMS and QuranWBW reference fonts were inspected outside repository: cmap aliases, shapes,
metrics and GDEF classes. No outlines, traces, bitmaps, layout tables or other restricted
assets enter derivative/build/repository. PDMS maps E003/U+0656 and E004/U+0657 to the same
glyphs; QuranWBW independently corroborates these equivalences. Distinct pause/section signs
were checked against both reference faces and Unicode descriptions where applicable.

[Unicode proposal L2/14-095](https://www.unicode.org/L2/L2014/14095-quranic-marks.pdf),
[L2/14-105](https://unicode.org/L2/L2014/14105-pakistani-koranic-marks.pdf) and
[Unicode annotation names](https://www.unicode.org/Public/18.0.0/charts/nameslist/08a0/)
establish dedicated meanings/forms; they do not themselves assign vendor PUA codes. Evidence
IDs, inspected reference revisions/checksums and limitations are machine-readable in
[`mapping.json`](../scripts/fonts/indopak/mapping.json) and
[`upstream.json`](../scripts/fonts/indopak/upstream.json).

Manifest `verified` status means **symbol identity/source encoding**, not approval of browser
placement or recitation rulings. Exact edition provenance and qualified recitation/editorial
review remain separate open items. Generated composite metrics also need that review before
production. The text's existing license/attribution is unchanged; OFL covers font software.

## Preferred candidate and base choice

DigitalKhatt source pinned to `23a0f34ed24e85fcf45e070616433d005ffca962`; conventional platform
TTF pinned to `78372d7a1e211bbab677cc0304384f278f508657`. Binary checksum is recorded in
`upstream.json`. This candidate has real TrueType `glyf` and conventional GDEF/GSUB/GPOS,
Arabic script features and standard extension lookups (GSUB type 7, GPOS type 9). This
inspection concerns that conventional TTF, not arbitrary DigitalKhatt web/custom builds.

Equivalent Quranic glyphs exist under other Unicode mappings. Missing PUA cmap entries did
not establish missing symbols. However, complete corpus audit also found **13 ordinary
visible/spacing characters missing**, plus U+200B zero-width-space control:

| Code   | Meaning                         | Occurrences |
| ------ | ------------------------------- | ----------: |
| U+066E | Dotless beh                     |       1,029 |
| U+0674 | High hamza                      |          13 |
| U+06A9 | Keheh                           |         149 |
| U+06AA | Swash kaf                       |          16 |
| U+06BE | Heh doachashmee                 |           3 |
| U+06C1 | Heh goal                        |           7 |
| U+06CC | Farsi yeh                       |          35 |
| U+06D2 | Yeh barree                      |           1 |
| U+06E1 | Small high dotless head of khah |      62,026 |
| U+06E4 | Small high madda                |       2,723 |
| U+06EB | Empty-centre high stop          |           3 |
| U+2003 | Em space                        |          26 |
| U+FE8E | Alef final presentation form    |           1 |

U+200B occurs 4,747 times and is legitimately invisible; it is reported separately from
missing visible glyphs. No ordinary letters/vowels were aliased to similar-looking glyphs.
Adding the missing contextual letter repertoire to DigitalKhatt would require substantial
shaping work beyond nine aliases.

Noto Naskh Arabic is already an offered open font family. Its **complete upstream** 2.021
variable TTF covers all ordinary corpus glyphs except em space and supplies all nine genuine
annotation equivalents. Existing Fontsource Arabic subset omits relevant glyphs, so it was
not patched. Chosen input: `google/fonts` revision
`ad2e6647e3d97ad5a8b6b5f95042f99161af5915`, instantiated at weight 400. Complete repertoire
and layout retained; no app/runtime font editor or shaper added.

## Nine mappings and layout

All derivative components are from the checksum-pinned Noto OFL input. Source behavior is
four zero-advance marks and five spacing pause signs, not a blanket mark classification.

| Private code | Count | Symbol / equivalent                   | Derivative implementation                                                                          |
| ------------ | ----: | ------------------------------------- | -------------------------------------------------------------------------------------------------- |
| U+E003       |     4 | Subscript alef, U+0656                | Reuse genuine mark, GDEF class 3 and existing anchors                                              |
| U+E004       |     1 | Inverted damma, U+0657                | Reuse genuine mark/anchors; browser failure remains                                                |
| U+E01A       |   242 | Small high zain, U+0617               | Composite of existing annotation; nonzero advance, class 1                                         |
| U+E01B       |   166 | Small high sad, U+08D5                | Dedicated sad composite; spacing/class 1                                                           |
| U+E01C       |   145 | Small high qaf, U+08D7                | Dedicated qaf composite; spacing/class 1                                                           |
| U+E01E       |   131 | Small high word qif, U+08DE           | Dedicated qif composite; spacing/class 1                                                           |
| U+E01F       |    20 | Small high word waqfa, U+08DF         | Dedicated waqfa composite; spacing/class 1                                                         |
| U+E021       |   121 | Disputed/optional end of ayah, U+08E2 | Dedicated glyph scaled/raised as high mark; zero advance, class 3; Noto high-mark attachment added |
| U+E022       |   553 | Small high ain, end of ruku, U+08D6   | Reuse genuine high mark/anchors, class 3                                                           |

Five spacing composites retain their source vertical placement; outline bounds plus side
bearings determine advances. E021 uses dedicated disputed-ayah glyph, not a generic numeral
or a ruku sign. Its transform and attachment are explicit in build script. No two distinct
private meanings share a target. Original cmap mappings/layout remain intact. U+2003 is a
new empty glyph with one-em advance, preserving stored spacing.

Mappings update applicable format 4/12 Unicode cmap tables. Supplementary additions use
format 12; format 14 variation sequences remain untouched. Tests exercise those boundaries.
GDEF classes, mark attachment/filter coverage, GSUB/GPOS compilation and reopened final
TTF/WOFF2 are checked. These are structural checks, not proof of correct browser positioning.

## Exact font-only blocker and additional display requirement

Reproduction: development `/design/indopak`, original verse 17:7, sequence
`لِيَسُـوْۤء\uE004ا`. E004 aliases the correct inverted-damma glyph, but retains Unicode
category `Co`, combining class 0, unknown script and bidi class `L`. U+0657 is a combining
mark with bidi `NSM`. Cmap, GDEF and GPOS cannot change a character's Unicode properties.

Forced whole-run Arabic/RTL HarfBuzz can attach the glyph; browser bidi/script itemization
can split/reorder the original PUA separately from hamza. Original and diagnostic U+0657
comparison render differently: original loses correct inverted-damma placement. Real Safari
27 reproduces loss; comparison attaches the sign to hamza. Diagnostic substitution exists
only in the explicitly labelled development view. A tested word-scoped RTL isolate/override
also failed; no bidi override was landed.

An OFL-only intrinsic-positioning experiment additionally composed E004 from Noto's existing
U+0657 glyph with x/y translation `(122, -427)`, derived from forced-run hamza attachment,
and shifted copied mark anchors to preserve full-run attachment. This made the mark visible
but did not reproduce browser Unicode-mark placement: 9/9 size/width pairs differed in both
Chromium and WebKit. Integer line heights removed screenshot rounding differences; remaining
pixel difference was confined to the sign, showing collision with hamza. Experiment is not
in packaged font. This strengthens the failed-placement finding without proving every
possible intrinsic/contextual font construction impossible.

The minimum unresolved display requirement is to treat this vendor code as the verified
Arabic mark **during itemization and shaping**, while preserving original logical string and
cluster/source indices. Ordinary FontFace/cmap API offers no per-character Unicode-property
override. A property-aware shaping/display adapter would need to own the complete affected
Arabic word/run, render shaped glyph positions and preserve original DOM/accessibility/copy
text and reference alignment. Font-only aliasing does not supply that behavior. No custom
engine, hidden production substitution, per-character wrappers or DB normalization introduced.
This requirement is established; such an adapter remains unimplemented and would require
separate selection/copy/accessibility validation. No claim that every conceivable font/layout
approach has been disproved.

## Reproducible package and development integration

[`build.py`](../scripts/fonts/indopak/build.py) verifies upstream font/license SHA-256 on
download and cached reuse; these are **font input** checksums, never Quran checksums. Pinned
FontTools 4.60.2, Brotli 1.2.0, uharfbuzz 0.51.7 are enforced. Reviewed Python 3.9.6/HarfBuzz
12.1.0. Build rejects unresolved/conflicting mappings, missing targets and inventory/context
mismatches. Default command refuses unapproved production; explicit `--preview` permits
research build, retaining all semantic checks. Instructions:
[`scripts/fonts/indopak/README.md`](../scripts/fonts/indopak/README.md).

Distinct family `IndoPak Reader Compat Preview`, PostScript
`IndoPakReaderCompatPreview-Regular`; family, full, unique, typographic, WWS and version names
updated consistently. Original copyright/OFL name records preserved. Complete OFL and FONTLOG
accompany font. No reserved upstream names reused as derivative family.

| Package |   Bytes | SHA-256, font only                                                 |
| ------- | ------: | ------------------------------------------------------------------ |
| TTF     | 201,032 | `3a54f6421c2b017b553b8db7b13c5b7fd1f747f16ab5848f4c9148af1191b66d` |
| WOFF2   |  71,260 | `0d7b1194c16cfb9fda82ac53ad43b9fd6ce0bc5089b8551d7187873c492ec385` |

Files under `web/static/fonts/indopak-reader-compat-preview-v1.*`; 1,726 glyphs. TTF is genuine
TrueType, not renamed OTF. Separate rebuilds produced byte-identical TTF/WOFF2/OFL. Static
versioned `-v1` URL and unique family with no `local()` source prevent accidental installed
font use. Font bytes fetched by browser are compared with packaged WOFF2. Future edits must
bump asset version, following existing static font naming/cache approach.

Development route is not prerendered; production server load returns 404 before DB access.
It uses actual `ReadingAyah`, complete original strings, multiple font sizes, wrap widths,
verse digits, paired punctuation and unaffected IndoPak/Uthmani controls. No new runtime
dependency or persisted settings field. Static font assets follow existing SvelteKit
service-worker precache/version policy; they are separate from Quran offline data pack.
Production default selection remains unchanged until rendering acceptance is met.

## Validation and open acceptance items

- Complete read-only corpus inventory compared with recorded inventory; packaged TTF/WOFF2
  have no missing visible/spacing characters or unmapped controls.
- HarfBuzz 12.1.0 shaped all 6,236 original strings with forced Arabic/RTL; zero `.notdef`;
  packaged TTF/decoded WOFF2 glyph IDs, advances, offsets and clusters agree. 5,015 unaffected
  runs exactly match original Noto glyph names/positions/clusters. This does not simulate
  browser bidi itemization. Recorded [`shaping-report.json`](../scripts/fonts/indopak/shaping-report.json).
- Seven focused Python tests cover manifest rejection, checksum tampering, full packaged
  coverage/layout/classes/advances, original cmap conflict protection, supplementary/variation
  handling, differing PUA mark clusters/properties and production refusal.
- Browser engine results, exact versions, specimen counts and known rendering failure are
  recorded in [`browser-report.json`](../scripts/fonts/indopak/browser-report.json). Harness
  exercises 24/33/48 px, 320/640/960 px widths, 390×844 viewport, original strings and all 87
  characters. Screenshot inspection is separate from nominal DOM/coverage assertions.
  Playwright 1.63.0: Chromium 153.0.8010.12 and WebKit 26.6 completed harness; both reproduce
  E004 failure. Playwright Firefox failed launch before navigation (`Could not find profile
folder`); older build also failed compositor initialization and headed retry. Harness exits
  nonzero and records failure; no automated Firefox pass claimed.
- Native Safari **27.0 on macOS 27.0**, GUI inspection: original/diagnostic 17:7 mismatch,
  punctuation/digits, unaffected IndoPak controls, KFGQPC Uthmani 1:7 control. This focused
  real-Safari check is separate from Playwright WebKit. No iOS device validation performed.
- Installed Firefox **147.0.1 on macOS 27.0**, focused GUI inspection: original/diagnostic
  17:7 mismatch, punctuation/digits, KFGQPC Uthmani 1:7, E003 in wrapped 5:7. This provides
  actual Firefox evidence, but does not replace unavailable full automated Firefox checks.
- Representative screenshots for each of nine private codes were inspected in Chromium and
  WebKit. Pause forms remain distinct; no obvious clipping in those sampled runs. E004 failure
  remains visible. This is not approval of all 223 contexts or exact editorial placement.
- Production preview server: specimen HTTP 404; TTF, WOFF2, OFL and FONTLOG HTTP 200 with
  exact packaged bytes; WOFF2 appears in existing generated versioned service-worker cache.
- `pnpm check`: zero errors/warnings. `pnpm lint --deny-warnings`: pass. `pnpm test`: 172 files,
  2,177 tests pass. Production build: pass. Quran DB bytes and counts unchanged.

Remaining: original printed-edition/download provenance; qualified semantic/recitation review;
full visual placement/collision review of generated composites; correct E004 browser
itemization/shaping; complete platform acceptance and production script integration.
Nominal font coverage and forced-run shaping pass; overall rendering acceptance fails.
