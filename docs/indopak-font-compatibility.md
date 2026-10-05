# IndoPak font compatibility

Implemented 2026-10-05; re-audited 2026-10-06. Version 3 is active for IndoPak in both
reading modes. The [deep audit](indopak-deep-audit.md) found and repaired further optional-ayah,
inline-pause and final-word grouping defects beyond the version 2 examples. It also records
unresolved source-text discrepancies with Quran.com. Original database strings remain exact.

This is a Noto-based open font, not a reproduction of Quran.com's Nastaleeq artwork. Technical
integration approval does not establish the original printed edition or approve recitation
rulings. Platform validation and limits are recorded below.

## Repository and immutable baseline

Reader is SvelteKit/browser, not a native app. Script selection lives in reader settings;
font definitions are `web/src/lib/config/reader-fonts.ts`, lazy FontFace loading is
`web/src/lib/fonts/arabic-fonts.ts`, and `ReadingAyah.svelte` renders verse strings. Existing
default KFGQPC Uthmanic Hafs path, preference schema and source profiles remain unchanged.
Both reader components now select an explicit IndoPak font/render branch. No changes to search, copying, bookmarks, translations or audio alignment.

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
review remain separate open items. Manifest production approval covers technical font and
reader integration, not those editorial questions. The text's existing license/attribution is unchanged; OFL covers font software.

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

| Private code | Count | Symbol / equivalent                   | Derivative implementation                                                     |
| ------------ | ----: | ------------------------------------- | ----------------------------------------------------------------------------- |
| U+E003       |     4 | Subscript alef, U+0656                | Reuse genuine mark, GDEF class 3 and existing anchors                         |
| U+E004       |     1 | Inverted damma, U+0657                | Composite of genuine mark, translated for the standalone bidi run             |
| U+E01A       |   242 | Small high zain, U+0617               | Composite of existing annotation; nonzero advance, class 1                    |
| U+E01B       |   166 | Small high sad, U+08D5                | Dedicated sad composite; spacing/class 1                                      |
| U+E01C       |   145 | Small high qaf, U+08D7                | Dedicated qaf composite; spacing/class 1                                      |
| U+E01E       |   131 | Small high word qif, U+08DE           | Dedicated qif composite; spacing/class 1                                      |
| U+E01F       |    20 | Small high word waqfa, U+08DF         | Dedicated waqfa composite; spacing/class 1                                    |
| U+E021       |   121 | Disputed/optional end of ayah, U+08E2 | Dedicated glyph scaled/raised with a separate ink lane; zero advance, class 3 |
| U+E022       |   553 | Small high ain, end of ruku, U+08D6   | Reuse genuine high mark/anchors, class 3                                      |

Five spacing composites retain their source vertical placement; outline bounds plus side
bearings determine advances. E021 uses dedicated disputed-ayah glyph, not a generic numeral
or a ruku sign. Its transform is explicit in the build script; v1's incorrect borrowed ruku anchor is removed. No two distinct
private meanings share a target. Original cmap mappings/layout remain intact. U+2003 is a
new empty glyph with one-em advance, preserving stored spacing.

Mappings update applicable format 4/12 Unicode cmap tables. Supplementary additions use
format 12; format 14 variation sequences remain untouched. Tests exercise those boundaries.
GDEF classes, preserved standard attachment/filter coverage, GSUB/GPOS compilation and reopened final
TTF/WOFF2 are checked. These are structural checks, not proof of correct browser positioning.

## Unicode properties and the version 2 repair

The original 17:7 word is `لِيَسُـوْۤء\uE004ا`. Private E004 is category Co, combining
class 0, script Unknown and bidi L; U+0657 is NSM. A cmap alias never changes those properties.
Version 1 reused the Unicode glyph and depended on its Arabic attachment. Actual browser
itemization split the private glyph into a standalone run, putting it near alif.

Version 2 composes the same licensed Noto glyph with translation `(122,-377)` and zero
advance. This places it above hamza in the sole E004 corpus context. The additional 50-unit
clearance fixes the hamza collision seen in the earlier `(122,-427)` experiment. No copied
GPOS anchor is added: it is an intrinsic glyph for the original standalone PUA run. Standard
U+0657 and its attachment tables remain intact. This is deliberately scoped to this immutable
corpus and the browser renderer, not a general Unicode-property repair or approval for a
native renderer that forces an entire verse into one Arabic run.

E021 is a genuine optional-ayah 5 from Noto U+08E2, half scale, translated `(-250,1120)`.
Its ink lies left of the following annotation, with a gap, rather than overprinting it.
The original-source reference uses adjacent signs; Quran.com's modern served text instead
uses a vertical stack and adds a small six. We preserve our source's two original signs.
The borrowed v1 ruku anchor was incorrect and is removed. Original U+08E2 is unchanged.

Trailing zain/ruku needs a small display-layer change because the ornament uses another font
and is a separate shaping run. `IndoPakAyah.svelte` groups the last word and ornament,
positions the original trailing signs above the ornament, and isolates only that generated
end cluster. It splits strings into exact slices, preserving every codepoint and logical DOM
order; it adds no text replacement, bidi override, custom shaper or per-character wrapping.
Two ruku-plus-pause endings (26:51, 43:15) retain both distinct signs. Font selection is
confined to IndoPak; Uthmani and Tajweed remain on their existing paths.

## Reproducible package and integration

[`build.py`](../scripts/fonts/indopak/build.py) verifies pinned **font/license** input checksums
on every use and enforces FontTools 4.60.2, Brotli 1.2.0 and uharfbuzz 0.51.7. It rejects
unresolved mappings, conflicts, absent targets and inventory/context mismatches. It never
reads or hashes Quran data. A manifest with unapproved rendering is rejected before download
or output. Rebuild instructions are in the [tool README](../scripts/fonts/indopak/README.md).

Family: `IndoPak Reader Compat`; PostScript: `IndoPakReaderCompat-Regular`; version 3.000.
Genuine TTF and WOFF2 retain the full repertoire and layout. Complete OFL and FONTLOG are
packaged alongside `web/static/fonts/indopak-reader-compat-v3.*`. The distinct name, explicit
versioned URL and absence of `local()` ensure the packaged derivative is used. Separate
rebuilds are byte-identical. Exact bytes, font-only checksums and name records are in
[`outputs.json`](../scripts/fonts/indopak/outputs.json).

`ReadingAyah` and `VerseRow` select the same font and end renderer for IndoPak. Copy tools
continue receiving the original text prop. The font is registered through CSS and fetched
only when used. Existing SvelteKit service-worker static precaching includes the versioned
font and notices. No runtime dependency or persisted font preference is added.

The `/design/indopak` development route covers all 223 inventoried context classes in 226
verses, all 87 corpus characters, sizes 24/33/48, widths 320/640/960 and Uthmani controls.
Its server returns 404 in production before DB access. Only a labelled diagnostic row uses
an E004/U+0657 comparison string; no production reader text is substituted.

The earlier shared LTR ornament isolation repair remains in place. Digits 1 through 286
compose inside their frame, including 2:101 and the Scheherazade fallback.

## Validation and limits

- Read-only corpus audit: 6,236 verses, 727,385 characters, 87 codepoints; nine PUA codes,
  1,383 occurrences. Inventory unchanged. Packaged TTF/WOFF2: zero missing visible characters.
- HarfBuzz 12.1.0: all 6,236 original strings, zero notdef, equal TTF/WOFF2 traces. 5,015
  unaffected runs exactly match upstream. Forced whole-run shaping verifies coverage and
  regressions; actual browser paint checks verify the private mark positioning.
- Project gates: `pnpm check` (zero errors/warnings), `pnpm lint` (deny warnings),
  `pnpm test` (173 files, 2,182 tests) and production build pass. Packaged WOFF2 is served
  byte-identically in production; the development specimen returns 404.
- Eight Python regressions include cmap/property limits, manifests, coverage, layout,
  checksums, supplementary/variation tables, mark collision and hamza clearance.
- Reader regressions exercise both modes, exact DOM text after excluding the generated
  ornament, unchanged Uthmani branch and trailing control/annotation order.
- Automated Chromium 153.0.8010.12 and Playwright WebKit 26.6 exercise the full specimen,
  nine size/width combinations and 390×844 viewport. Paint regressions reject the old E004
  drift and E021 overlap. All ornaments remain enclosed; end signs clear the ornament and
  stay with the final word. Browser font response equals packaged bytes.
- Playwright Firefox cannot launch on this host (`Could not find profile folder`), including
  an explicit persistent-profile retry. Harness reports failure and exits nonzero. Native
  Firefox 147.0.1 supplies focused GUI evidence separately, not a full automated pass.
- Native Safari and Firefox checks are documented in
  [`browser-report.json`](../scripts/fonts/indopak/browser-report.json). Playwright WebKit
  is not real Safari or iOS. No iOS device validation is claimed.

Reference fonts remain outside the repository and are never build inputs. Noto pause forms
retain Noto's higher annotation lane; the glyph meanings stay distinct and genuine. This
sampled comparison does not prove pixel parity with Quran.com's different font or certify
recitation/editorial accuracy. Original printed-edition/download provenance and qualified
recitation review remain unresolved.

## Version 3 full-occurrence audit

The [2026-10-06 deep audit](indopak-deep-audit.md) supersedes the narrower version 2 placement
checks recorded above. It covers all private occurrences, terminal pause chains, changed inline pairs
and optional-ayah marks with neighboring verses. Source-text differences and platform gaps
remain open; technical font integration is not editorial certification.
