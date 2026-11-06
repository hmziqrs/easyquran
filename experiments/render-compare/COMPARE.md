# Quran text rendering comparison: GPUI vs Flutter vs browser

> **Correction (2026-11-06).** Stock GPUI output below is broken, and the earlier
> "Mehr flattens under CoreText" diagnosis was wrong. Root cause is a GPUI bug:
> `gpui_apple` stores CoreText's y-up glyph `position.y` as is, then `line.rs` adds
> it to a y-down baseline. Every glyph with a vertical offset lands mirrored
> around the baseline: harakat and Urdu dots fall below the line. Negating that
> one value fixes CoreText for KFGQPC Hafs and Mehr
> ([stock](gpui/coretext-stock-yflip-bug.jpg),
> [patched](gpui/coretext-yflip-patched.jpg)). HarfRust shaping painted through
> `paint_glyph` gives the same result with no GPUI patch
> ([HarfRust](gpui/harfrust-paint-glyph.jpg),
> [Amiri, wrapped](gpui/harfrust-amiri-wrapped.jpg)). The numbered `gpui/` shots
> predate this and show the bug. Checked on macOS only.

## NSH (Native Shell 0.4.0, CoreText): Arabic and Urdu work on macOS and iOS Simulator

App: [`../nsh-quran-sample`](../nsh-quran-sample). Shots: `nsh/`, vision-checked,
no pixel diff. Checked in the selector reader: hafs, taha-v1-bold, amiri (Arabic);
noto, awami, mehr (Urdu); uthmani, indopak. Checked on the earlier test page, iOS
only: noto-naskh, scheherazade, gulzar, bidi. Not looked at: taha-v1, katibeh,
mirza, lateef, aref-ruqaa, lalezar, noto-sans-arabic, simple in the reader.

Joining, harakat, RTL, ayah markers and Nastaliq match the Flutter shots. No y-flip
bug. Awami Nastaliq does not join on either platform, same as Flutter and Safari
(Graphite only, no `GSUB`), so this is not NSH-specific.

## Rune vs NSH on macOS

App: [`../rune-quran-sample`](../rune-quran-sample) (Rune `dev` rev `781e2b6d`, Parley and
HarfRust, own GPU renderer). Shots: `rune/`, vision-checked, no pixel diff. All 6 Arabic
fonts, all 12 Urdu fonts and all 3 databases were looked at in headless snapshots. Selector
and scrolling were tried in a real window. **Rune has no iOS or Android support yet**, so
it was not run on mobile.

Arabic and Urdu shape correctly in all but Awami Nastaliq, which fails as it does in NSH,
Flutter and Safari. Default view matches NSH by eye. Rune needed U+200F on the ayah marker
because it has no paragraph direction setting.

Median of 3 runs, first run discarded, same 1200x830 window size, same text, Apple silicon
on battery. "Long" repeats the sample 17 times (289 ayahs, about Al-Baqarah's count, shorter
text). Scripts and raw numbers: [`bench/`](bench).

| | NSH 0.4.0 | Rune |
| --- | --- | --- |
| App size | 44 MB | 28 MB |
| Launch to window on screen, long | 225 ms | 280 ms |
| CPU used by 2.5 s after launch, long | 0.36 s | 0.20 s |
| Idle CPU, long | 0% | 0% |
| Memory at idle, long | 66 MB | 70 MB |
| Memory after 20 font switches, long | 75 MB | 342 MB |
| Memory after scrolling, long | 146 MB | 343 MB |
| CPU per font switch, long (range) | 38 ms (37-46) | 48 ms (35-58) |
| CPU while scrolling, long (range) | 49 ms/s (48-104) | 55 ms/s (43-104) |
| Memory at idle, short sample | 34 MB | 66-318 MB across runs |

Read these as rough:

- CPU per switch and while scrolling overlap, so treat them as a tie.
- Rune memory jumps between about 65 MB and 340 MB, at short and long content alike. It
  does not track content size. Cause not investigated.
- Rune's short-sample font switch numbers are left out: two of four runs show almost no
  CPU, so I can't tell the switches ran. The long runs were checked on screen.
- Not measured: frame rate, input latency, GPU time, WindowServer CPU. "Window on screen"
  is not "text painted".
- The same wheel events moved Rune further than NSH, so scroll CPU is not per point of
  travel.

Same faces, same data extract (`gpui-kit-sample/data/quran-sample.json`),
same type settings (Arabic 30px / line-height 1.9, Urdu 20px with per-font
Nastaliq leading), macOS + Retina, captured per combo.

- **flutter/** — Flutter 3.47 macOS sample (`../flutter-quran-sample`), Impeller + Skia text stack with bundled HarfBuzz shaping. Not the chosen approach; kept as a rendering reference.
- **web/** — minimal HTML page loading the identical TTFs via `@font-face` (`reference/index.html`, regenerate with `reference/generate.py`), Safari/WebKit rendering — the same stack the real web reader uses.
- **gpui/** — gpui-kit 0.8.1 sample (`../gpui-kit-sample`), CoreText shaping + GPUI's own Metal rasterizer. **The chosen stack**, with HarfRust shaping in place of CoreText (see correction above); numbered screenshots show the y-flip bug.

## Urdu faces (12, all selectable; Noto Nastaliq Urdu is the default)

Coverage gate: every face was cmap-tested for the full Urdu letter set
(ے U+06D2, ہ U+06C1, ٹ U+0679, ڈ U+0688, ڑ U+0691, ں U+06BA, ھ U+06BE)
before inclusion.

| Face               | License        | Source             | Style                | Leading (20px) | Flutter                                              | Web                                          |
| ------------------ | -------------- | ------------------ | -------------------- | -------------- | ---------------------------------------------------- | -------------------------------------------- |
| Noto Nastaliq Urdu | OFL 1.1        | Google Fonts       | Nastaliq             | 2.5            | [shot](flutter/urdu-noto-in-flutter.png)             | [shot](web/urdu-noto-in-web.png)             |
| Gulzar             | OFL 1.1        | Google Fonts       | Nastaliq typographic | 2.5            | [shot](flutter/urdu-gulzar-in-flutter.png)           | [shot](web/urdu-gulzar-in-web.png)           |
| Awami Nastaliq     | OFL 1.1        | SIL                | Nastaliq wide        | 2.6            | [shot](flutter/urdu-awami-in-flutter.png)            | [shot](web/urdu-awami-in-web.png)            |
| Mehr Nastaliq Web  | free (non-OFL) | bundled            | Nastaliq             | 2.9            | [shot](flutter/urdu-mehr-in-flutter.png)             | [shot](web/urdu-mehr-in-web.png)             |
| Katibeh            | OFL 1.1        | Google Fonts       | Nastaliq display     | 2.2            | [shot](flutter/urdu-katibeh-in-flutter.png)          | [shot](web/urdu-katibeh-in-web.png)          |
| Mirza              | OFL 1.1        | Google Fonts       | Nastaliq-influenced  | 2.3            | [shot](flutter/urdu-mirza-in-flutter.png)            | [shot](web/urdu-mirza-in-web.png)            |
| Lateef             | OFL 1.1        | SIL / Google Fonts | Naskh Urdu           | 2.2            | [shot](flutter/urdu-lateef-in-flutter.png)           | [shot](web/urdu-lateef-in-web.png)           |
| Scheherazade New   | OFL 1.1        | SIL / Google Fonts | Naskh Urdu           | 2.2            | [shot](flutter/urdu-scheherazade-in-flutter.png)     | [shot](web/urdu-scheherazade-in-web.png)     |
| Amiri              | OFL 1.1        | Google Fonts       | Naskh Arabic-first   | 2.2            | [shot](flutter/urdu-amiri-in-flutter.png)            | [shot](web/urdu-amiri-in-web.png)            |
| Aref Ruqaa         | OFL 1.1        | Google Fonts       | Ruqaa                | 2.0            | [shot](flutter/urdu-aref-ruqaa-in-flutter.png)       | [shot](web/urdu-aref-ruqaa-in-web.png)       |
| Lalezar            | OFL 1.1        | Google Fonts       | Display              | 2.0            | [shot](flutter/urdu-lalezar-in-flutter.png)          | [shot](web/urdu-lalezar-in-web.png)          |
| Noto Sans Arabic   | OFL 1.1        | Google Fonts       | Sans                 | 2.4            | [shot](flutter/urdu-noto-sans-arabic-in-flutter.png) | [shot](web/urdu-noto-sans-arabic-in-web.png) |

Eleven shape correctly in Flutter (HarfBuzz) and the browser reference. Awami
Nastaliq does not join in either ([Flutter](flutter/urdu-awami-in-flutter.png),
[web](web/urdu-awami-in-web.png)): the font has Graphite tables (`Silf`) and no `GSUB`.
Mehr's broken look in stock GPUI is the y-flip bug, not CoreText. Flutter sample: `Urdu`
dropdown + **F11** cycles faces. Web reference: `&urdu=<id>` param.

## Verified matrix (all vision-checked)

Urdu = Noto Nastaliq Urdu unless noted.

| Combo                          | Flutter                                          | Web                                          | GPUI (frozen)                                 |
| ------------------------------ | ------------------------------------------------ | -------------------------------------------- | --------------------------------------------- |
| kfgqpc-hafs × uthmani          | [01](flutter/01-kfgqpc-hafs-uthmani.png)         | [01](web/01-kfgqpc-hafs-uthmani.png)         | [01](gpui/01-kfgqpc-hafs-uthmani.png)         |
| kfgqpc-taha-v1 × uthmani       | [02](flutter/02-kfgqpc-taha-v1-uthmani.png)      | [02](web/02-kfgqpc-taha-v1-uthmani.png)      | [02](gpui/02-kfgqpc-taha-v1-uthmani.png)      |
| kfgqpc-taha-v1-bold × uthmani  | [03](flutter/03-kfgqpc-taha-v1-bold-uthmani.png) | [03](web/03-kfgqpc-taha-v1-bold-uthmani.png) | [03](gpui/03-kfgqpc-taha-v1-bold-uthmani.png) |
| amiri × uthmani                | [04](flutter/04-amiri-uthmani.png)               | [04](web/04-amiri-uthmani.png)               | [04](gpui/04-amiri-uthmani.png)               |
| scheherazade-new × uthmani     | [05](flutter/05-scheherazade-new-uthmani.png)    | [05](web/05-scheherazade-new-uthmani.png)    | [05](gpui/05-scheherazade-new-uthmani.png)    |
| noto-naskh-arabic × uthmani    | [06](flutter/06-noto-naskh-arabic-uthmani.png)   | [06](web/06-noto-naskh-arabic-uthmani.png)   | [06](gpui/06-noto-naskh-arabic-uthmani.png)   |
| noto-naskh-arabic × simple     | [07](flutter/07-noto-naskh-arabic-simple.png)    | —                                            | [07](gpui/07-noto-naskh-arabic-simple.png)    |
| noto-naskh-arabic × indopak    | [08](flutter/08-noto-naskh-arabic-indopak.png)   | —                                            | [08](gpui/08-noto-naskh-arabic-indopak.png)   |
| kfgqpc-hafs × indopak          | [09](flutter/09-kfgqpc-hafs-indopak.png)         | [08](web/08-kfgqpc-hafs-indopak.png)         | [09](gpui/09-kfgqpc-hafs-indopak.png)         |
| kfgqpc-hafs × simple           | —                                                | [07](web/07-kfgqpc-hafs-simple.png)          | —                                             |
| Mehr Urdu in Flutter (vs Noto) | [mehr](flutter/urdu-mehr-in-flutter.png)         | [mehr](web/urdu-mehr-in-web.png)             | —                                             |

Every cell: correct joining, correct harakat stacking, correct RTL alignment,
correct uthmani hamzat-wasl (ٱ), correct IndoPak small-kashida marks, ayah
markers ﴿١﴾ composed, Urdu Nastaliq legible, **except the GPUI column**: its
harakat and dots are mirrored below the line (see correction above).

## Findings

0. **Urdu face choice.** Noto Nastaliq Urdu (OFL) is the default in the
   Flutter sample and the web reference. License
   note: prefer OFL faces (Noto, Amiri, Scheherazade, Noto Naskh); Mehr's
   redistribution terms are less clear than OFL.
1. **Urdu data bug (found by this comparison, now fixed).** The junagarhi
   translation source encodes lam-alef as the _isolated_ presentation form
   `U+FEFB` (6× in the sample), which every stack renders as a broken-looking
   mid-word ﻻ. The production web pipeline does not normalize it either.
   `gpui-kit-sample/data/generate.py` now NFKC-normalizes Urdu (Arabic
   datasets contain no presentation forms and are untouched); all screenshots
   show the normalized text with the properly joined والا.
2. **Weight / crispness.** Browser glyphs read heaviest and most "solid"
   (subpixel AA + platform gamma). Flutter (Impeller grayscale AA) sits
   close behind — near-indistinguishable from web at a glance.
3. **Leading.** Browser derives the line box from the font's own metrics;
   Nastaliq needs no manual tuning. Flutter needs a manual multiplier
   (Noto 2.5×, Mehr 2.9× at 20px) or diacritics collide.
4. **Font pipeline friction.**
   - Flutter: fonts work as shipped (pubspec family mapping), Taha bold via
     `weight: 700` mapping. Zero patches needed.
   - Web: `@font-face`, zero patches.
   - GPUI (historical): refuses fonts without a Latin `m` glyph — needed
     patched TTFs; manual metrics; negative scroll offsets; focus quirks.
5. **Cross-platform.** Flutter ships its own HarfBuzz on every OS. GPUI uses a
   different shaper per OS unless the app pre-shapes with HarfRust.

## Verdict

- **Decision (2026-11-06): GPUI + HarfRust + `paint_glyph` is the desktop stack.**
  Flutter is dropped. It was chosen only while GPUI Arabic looked broken, and
  that was the y-flip bug, which HarfRust shaping avoids.
- Open work: the custom element has no selection, caret, bidi or accessibility
  text. All text with Arabic must go through it, because stock GPUI text is
  still wrong on Windows (reversed surah names, flipped ayah brackets).
- Platform status: macOS verified. Windows launches and shows correct ayah text
  in an ARM VM, but sharpness is unmeasured and Zed blur issue #65664 is open.
  Linux: owner tested the ARM build in a VM on 2026-11-06 and it works the same
  as Windows.
- If pixel-fidelity to the web reader ever becomes the bar again, a webview
  shell (Tauri) is the zero-divergence option.

Reproduce the web reference: `python3 -m http.server 8777` from `experiments/`,
then `http://localhost:8777/render-compare/reference/index.html?font=<id>&urdu=<urdu id>&db=<dataset>`.
