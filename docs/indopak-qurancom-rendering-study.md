# How Quran.com renders IndoPak — study and recommendation (2026-10-06)

Goal: copy the approach of an established reader instead of inventing one. Quran.com was
observed live in Chromium 153 (Playwright), Playwright WebKit 26.6 and native Safari 27.0
(macOS 27.0). **Firefox was not observed:** automated Firefox/Gecko cannot launch on this host
(Playwright Firefox, geckodriver and headless Firefox all exit at startup), and screen access
to the user's Firefox was declined. Quran.com's DOM/CSS is identical across engines and Gecko
also shapes with HarfBuzz, but no Firefox result is claimed.

Evidence (ignored, local only): `.cache/qurancom/` — per-engine captures and `results.json`
for 13 verses × desktop/390/320 px, reading-mode capture, the prototype page and its renders.
Frontend source inspected at
[quran.com-frontend-next `74eb4e2`](https://github.com/quran/quran.com-frontend-next/tree/74eb4e20f4e78cc11c055a3e130136e8dbb7743e).
Reference fonts and screenshots stay outside the repository.

## What Quran.com does

| Area                   | Quran.com behaviour                                                                                                                                                                            |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Font                   | `IndoPak` = AlQuran IndoPak by QuranWBW 4.2.1-WL (Nastaleeq style), same file in every browser                                                                                                 |
| Text                   | Per-word `text` from its word database (current IndoPak encoding), not our DB's legacy `textIndopak` string                                                                                    |
| Verse/translation view | Every word is its own box: `display:inline-block; white-space:nowrap; unicode-bidi:isolate`, items of an RTL `flex-wrap` row. Lines break **only between words**                               |
| Word gaps              | 6 px after each word, 13 px after a word containing a stop sign (`INDO_PAK_STOP_SIGN_CHARS` in `QuranWord.tsx`)                                                                                |
| Pause signs            | Stored inside the preceding word (`السَّمَآءُ ؕ` is one box), so a sign never starts a line                                                                                                    |
| Ayah end               | Separate "end" word: zero-ink U+06DF base + pause/ruku marks + precomposed circular number glyph (U+F4FF + ayah). Signs sit stacked above the circle                                           |
| Reading view (desktop) | Printed 15-line IndoPak mushaf: words carry page/line numbers from the API, each line is a fixed-width (750 px) flex row justified with `space-between`. The browser never chooses line breaks |
| Reading view (mobile)  | Above font scale 3 (default is 4) lines become inline word flow, same atomic word boxes                                                                                                        |
| Font size              | Scales 1–10 mapped to `vh`/`vw`; single-ayah page default 26 px                                                                                                                                |

Measured on 13 verses (all review items from the [Safari results](indopak-safari-results.md)):
**zero horizontal overflow in every engine and width**, because atomic word boxes cannot be
mis-measured the way our long nowrap final group is (item A). Quran.com does **orphan the ayah
marker** onto its own line: Chromium 390 px 83:31, 17:7; WebKit and Safari 390 px 79:27,
71:23, 83:31, 2:282; Safari 336 px (its minimum inner width) 2:101; WebKit 320 px 6:165.

## Can we copy it while keeping our DB immutable?

Prototype (`.cache/qurancom/proto.html`): our exact DB strings, the QuranWBW font, Quran.com's
word-box layout, standalone pause tokens fused into the preceding word box, whitespace kept as
DOM text nodes (flex drops them visually), circular marker glyph kept inside the final word's
box. Results:

- The QuranWBW font maps every code point in our corpus except U+200B (ignorable) and U+2003
  (falls back to a space). Shaping all 6,236 verses gives **zero `.notdef`**. All nine private
  codes are in its cmap: the font was built for exactly this legacy encoding. All 286 marker
  glyphs exist.
- DOM text equals the DB string for every prototype verse, in Chromium and WebKit.
- Body text is visually the same as Quran.com: same letterforms, pause signs and E004.
- No overflow at 26 px/318 px or 33 px/286 px. At 56 px/286 px only 89:27 (EM SPACE ending)
  overflows, in Chromium.
- Open implementation details:
  - Word boundaries must also split at U+200B. 17:7 joins qif to the next word with ZWSP, so a
    sign prefix must move back to the previous word.
  - Inline and end sign clusters containing private codes need forced RTL ordering (PUA
    characters are bidi class L), as v3 already does.
  - End-sign stacks must be centered on glyph ink: a per-glyph table from the new font, like
    v3's Noto ink widths.
  - WebKit drops a lone zero-advance ruku mark unless it gets an explicit box width.

Remaining visible differences are source-text differences already recorded in the
[deep audit](indopak-deep-audit.md): for example 79:27 waqfa versus Quran.com's served `ۙ`,
4:142's extra waw, and Quran.com's added ruku counters. We do not import served text.

## Font licensing

The QuranWBW font's own licence field says: "NOT FOR SALE, NOT FOR MODIFICATION, NOT FOR
DISTRIBUTION OR NOT FOR DEVELOPMENT WITHOUT WRITTEN NOTICE BY QURANWBW.COM". Quran Foundation's
[font documentation](https://api-docs.quran.com/docs/tutorials/fonts/font-rendering/) permits
caching or bundling its served font files (including this one, from
`verses.quran.foundation/fonts/quran/hafs/nastaleeq/indopak/`) when the app keeps an active
Quran Foundation Developer Console account and credits Quran Foundation, for example "Quran fonts
provided by Quran Foundation". Files may ship only inside the app, never as a separate download.
The font is unmodified, so no OFL derivative is needed. Written confirmation from QuranWBW
(quranwbw@gmail.com) would remove any doubt between the two notices.

Open alternative: [DigitalKhatt IndoPak](https://github.com/DigitalKhatt/indopakfont) (OFL 1.1,
based on the 13-line IndoPak mushaf, beta 2024-12). It is not what Quran.com uses and has not
been evaluated against our encoding.

## Recommendation

1. **Copy Quran.com's verse layout now.** Use atomic word boxes, break only between words, put
   pause signs inside the preceding word, and use Quran.com's two gap sizes. Keep our one
   deliberate deviation: the ayah marker stays in the final word's box, so it is never
   orphaned. This removes review items A (WebKit/Safari overflow) and D (lone pause signs) by
   construction and keeps exact DB text.
2. **Use Quran.com's font**, AlQuran IndoPak by QuranWBW 4.2.1, with its circular ayah markers.
   Load it from Quran Foundation's documented URL or bundle it under their terms. This needs the
   owner's Developer Console account and an attribution line. It renders our immutable text
   with zero missing glyphs and retires the Noto-derived compatibility font for IndoPak. Keep
   v3 as fallback until licensing is settled.
3. **Later, a true mushaf page mode** like Quran.com's desktop reading view: 15-line pages from
   a licensed word-to-page/line layout dataset (Quran Foundation API or QUL). The dataset must
   live outside `db/`, and its word segmentation must be reconciled with our DB tokens (for
   example 18:110 and 17:7 join words that the layout data splits).

Each step needs the three web gates and a full WebKit/native Safari rerun of the
[Safari plan](indopak-safari-testing-plan.md). Visual parity with Quran.com is a design target,
not a claim of editorial or recitation verification.
