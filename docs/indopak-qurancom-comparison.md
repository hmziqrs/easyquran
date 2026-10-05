# IndoPak comparison with live Quran.com

Observed 2026-10-05. **All nine symbol identities are corroborated; full rendering correctness
is not confirmed.** Comparison reproduces the existing E004 placement failure and identifies
an E021/no-stop stacking defect at 1:7. Production remains blocked.

## Reference and method

Live Quran.com reported `text_indopak`, 15-line preference, deployment build
`cfO91iJO_kGMxzuatsf-J`. Word and end-marker spans both computed to `font-family: IndoPak`.
The observed CSS font resource was captured from the browser, outside this repository:

- [Loaded WOFF2](https://quran.com/fonts/quran/hafs/nastaleeq/indopak/indopak-nastaleeq-waqf-lazim-v4.2.1.woff2):
  AlQuran IndoPak by QuranWBW, version 4.2.1-WL, 83,560 bytes. Font-only SHA-256:
  `ca2a573bd46af4a03f208ebfbaa0336242d0ea4199f1e20b1f06e1e2a1d87900`.
- [Official font documentation](https://api-docs.quran.com/docs/tutorials/fonts/font-rendering/)
  identifies the same v4.2.1 family. Its documented TTF has matching cmap and glyph order.
- [Observed source font registration](https://github.com/quran/quran.com-frontend-next/blob/74eb4e20f4e78cc11c055a3e130136e8dbb7743e/src/styles/fonts.scss)
  and [word rendering styles](https://github.com/quran/quran.com-frontend-next/blob/74eb4e20f4e78cc11c055a3e130136e8dbb7743e/src/components/dls/QuranWord/TextWord.module.scss)
  support the live observations. That master revision is a source inspection pin, not a proven
  Git revision of the deployed build.

Public page data contains both legacy `word.textIndopak` and served `word.text`. They are
different strings. The served display changes some letter/mark encodings, adds font-specific
codes, and moves some signs to end-marker words. We did not substitute that text into our
reader or database.

Nine live verse pages supplied ten comparison cases, covering every added private code and
the 2:101 end-marker case. All ten legacy excerpts occur in the immutable local DB; trailing
reference U+200F was excluded only from the containment comparison. SQLite opened with
`mode=ro&immutable=1`. No Quran hashes were calculated.

Three columns were rendered at equal nominal sizes in Chromium 153.0.8010.12 and WebKit 26.6:

1. Our packaged preview font with the original excerpt.
2. The actually loaded Quran.com font with that same original excerpt.
3. The Quran.com font with its current served display text.

This separates font behavior from text/pipeline differences. Live browser inspection also
checked 2:101, 17:7 and 1:7. The comparison is sampled, not a review of all 1,383 occurrences
or all 223 context classes. Reference fonts, diagnostic fonts and screenshots remain outside
the repository; none are build inputs or redistributed assets.

Machine-readable observations:
[qurancom-comparison.json](../scripts/fonts/indopak/qurancom-comparison.json).

## Results

| Private code | Symbol                 | Live representative                       | Result                                                                                                                                       |
| ------------ | ---------------------- | ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| E003         | Subscript alef         | [5:7](https://quran.com/5/7), word 8      | Identity corroborated; reference cmap aliases E003/U+0656. Subscript visible in sampled local rendering.                                     |
| E004         | Inverted damma         | [17:7](https://quran.com/17/7), word 12   | Identity corroborated by E004/U+0657 cmap alias. Original PUA placement remains incorrect.                                                   |
| E01A         | High zain              | [2:7](https://quran.com/2/7), word 9      | Zain form corroborated. Local mark sits higher; height/spacing requires review.                                                              |
| E01B         | High sad               | [2:16](https://quran.com/2/16), word 5    | Sad form corroborated. Local height/spacing differs.                                                                                         |
| E01C         | High qaf               | [2:5](https://quran.com/2/5), word 5      | Qaf form corroborated. Local height/spacing differs.                                                                                         |
| E01E         | Qif                    | [2:83](https://quran.com/2/83), word 9    | Full qif form corroborated. Local height/spacing differs.                                                                                    |
| E01F         | Waqfa                  | [2:286](https://quran.com/2/286), word 40 | Full waqfa form corroborated. Local height/spacing differs.                                                                                  |
| E021         | Optional/disputed ayah | [1:7](https://quran.com/1/7), word 4      | Identity corroborated; local glyph overlaps adjacent no-stop sign. Stacking fails.                                                           |
| E022         | End-of-ruku ain        | [1:7](https://quran.com/1/7), word 9      | Ain identity corroborated. Quran.com places its indication in the end-marker word; our legacy source retains it after the final Arabic word. |

The five spacing pause forms are recognizable and distinct. Their higher position in Noto
is an observed difference requiring layout/editorial review, not proof that Quran.com's
font metrics must be copied. Different font proportions are expected. Exact glyph-outline
or pixel equality is not an acceptance requirement established by this comparison.

Quran.com's current font uses U+06EA for its sad form, U+06D7 for qaf, U+06EB for qif and
U+06E5 for waqfa. These are observed font-specific encodings, **not Unicode semantic
equivalences** and not replacement mappings for our stored text.

## Newly reproduced E021 stacking defect

At 1:7, E021 precedes U+06D9 (no-stop sign). The preview's `compat.E021` and `uni06D9` both
use GPOS mark class 0 and anchor Y=1030. Their X anchors are 134 and 82 respectively; their
ink occupies the same attachment area. The optional-ayah form and no-stop sign crowd/overlap
in both Chromium and WebKit, also visible in the actual development reader.

The generated glyph copies the attachment record of `uni08D6` (ruku ain). That attachment
does not establish correct placement beside another high sign. A temporary diagnostic font
mapped U+08E2 to the **same** `compat.E021` glyph. The diagnostic also overlaps U+06D9 in both
engines. The defect cannot be attributed solely to E021's private-character bidi properties.
The diagnostic changes neither packaged fonts nor Quran data.

Required repair: distinct positioning for this stack, followed by review of all E021 contexts
and adjacent marks. This comparison does not deliver that repair or approve a guessed offset.

## Ayah ornament and evidence limits

At [2:101](https://quran.com/2/101), our Arabic 101 stays enclosed after the shared ornament
fix. Quran.com uses a precomposed circular marker; our existing Scheherazade ornament has a
different decorative frame. Enclosure is correct; identical artwork is not claimed.

Quran.com's served end word is U+06DF, U+0617, U+F564. Its zain sits with the marker. Our
unchanged source keeps E01A after the final Arabic word, followed by a separate ornament.
The same pipeline distinction affects end-of-ruku indicators. Matching those placements
would require an explicit display design, not copying Quran.com's altered word strings.

Quran.com uses the **same QuranWBW family** already inspected at v4.2.2 during the initial
audit. This adds live deployment, text and layout confirmation; it is not an independent
printed-edition or qualified recitation review. Exact source-edition provenance, mark
placement across all contexts, E004, and the E021 stack still prevent production approval.
