# Quran text rendering comparison: GPUI vs Flutter vs browser

Same 7 TTF faces, same data extract (`gpui-kit-sample/data/quran-sample.json`),
same type settings (Arabic 30px / line-height 1.9, Urdu Mehr Nastaliq Web 20px
/ line-height 2.9), macOS + Retina, captured per combo.

- **gpui/** — gpui-kit 0.8.1 sample (`../gpui-kit-sample`), Apple text system (CoreText shaping) + GPUI's own Metal rasterizer.
- **flutter/** — Flutter 3.47 macOS sample (`../flutter-quran-sample`), Impeller + Skia text stack with bundled HarfBuzz shaping.
- **web/** — minimal HTML page loading the identical TTFs via `@font-face` (`reference/index.html`), Safari/WebKit rendering — the same stack the real web reader uses.

## Verified matrix (all vision-checked)

| Combo                         | GPUI                                          | Flutter                                          | Web                                          |
| ----------------------------- | --------------------------------------------- | ------------------------------------------------ | -------------------------------------------- |
| kfgqpc-hafs × uthmani         | [01](gpui/01-kfgqpc-hafs-uthmani.png)         | [01](flutter/01-kfgqpc-hafs-uthmani.png)         | [01](web/01-kfgqpc-hafs-uthmani.png)         |
| kfgqpc-taha-v1 × uthmani      | [02](gpui/02-kfgqpc-taha-v1-uthmani.png)      | [02](flutter/02-kfgqpc-taha-v1-uthmani.png)      | [02](web/02-kfgqpc-taha-v1-uthmani.png)      |
| kfgqpc-taha-v1-bold × uthmani | [03](gpui/03-kfgqpc-taha-v1-bold-uthmani.png) | [03](flutter/03-kfgqpc-taha-v1-bold-uthmani.png) | [03](web/03-kfgqpc-taha-v1-bold-uthmani.png) |
| amiri × uthmani               | [04](gpui/04-amiri-uthmani.png)               | [04](flutter/04-amiri-uthmani.png)               | [04](web/04-amiri-uthmani.png)               |
| scheherazade-new × uthmani    | [05](gpui/05-scheherazade-new-uthmani.png)    | [05](flutter/05-scheherazade-new-uthmani.png)    | [05](web/05-scheherazade-new-uthmani.png)    |
| noto-naskh-arabic × uthmani   | [06](gpui/06-noto-naskh-arabic-uthmani.png)   | [06](flutter/06-noto-naskh-arabic-uthmani.png)   | [06](web/06-noto-naskh-arabic-uthmani.png)   |
| noto-naskh-arabic × simple    | [07](gpui/07-noto-naskh-arabic-simple.png)    | [07](flutter/07-noto-naskh-arabic-simple.png)    | —                                            |
| noto-naskh-arabic × indopak   | [08](gpui/08-noto-naskh-arabic-indopak.png)   | [08](flutter/08-noto-naskh-arabic-indopak.png)   | —                                            |
| kfgqpc-hafs × indopak         | [09](gpui/09-kfgqpc-hafs-indopak.png)         | [09](flutter/09-kfgqpc-hafs-indopak.png)         | [08](web/08-kfgqpc-hafs-indopak.png)         |
| kfgqpc-hafs × simple          | —                                             | —                                                | [07](web/07-kfgqpc-hafs-simple.png)          |

Every cell: correct joining, correct harakat stacking, correct RTL alignment,
correct uthmani hamzat-wasl (ٱ), correct IndoPak small-kashida marks, ayah
markers ﴿١﴾ composed, Urdu Nastaliq legible. **No framework produced broken
Arabic.**

## What actually differs

1. **Weight / crispness.** Browser glyphs read heaviest and most "solid"
   (subpixel AA + platform gamma). Flutter (Impeller grayscale AA) sits close
   behind. GPUI reads thinnest/lightest at identical px sizes. This is the
   main "way too much difference" impression from the GPUI sample — it is a
   rasterization difference, not a shaping one.
2. **Leading.** Browser derives the line box from the font's own metrics;
   Nastaliq needs no manual tuning. GPUI and Flutter both need a manual
   multiplier (2.4–2.9× at 20px) or diacritics collide. Flutter's default
   (2.4) still clips slightly; web is right for free.
3. **Font pipeline friction.**
   - GPUI: refuses fonts without a Latin `m` glyph (all six Quran faces lack
     Latin) — needs patched TTFs or silent system fallback.
   - Flutter: fonts work as shipped (pubspec family mapping), Taha bold via
     `weight: 700` mapping. Zero patches needed.
   - Web: `@font-face`, zero patches.
4. **IndoPak marks (ٔ-style superscripts, small-kashida style).** All three
   render them; GPUI's mark spacing looks slightly looser at the same size.
   Cosmetic.
5. **Cross-platform shape.** Web and Flutter are engine-identical on every
   OS. GPUI macOS uses CoreText; the Linux path (harfrust) was not exercised
   here and is a known divergence risk.

## Verdict

- **Correctness**: tie — all three shape Quran text correctly on macOS.
- **Fidelity to the web reader**: web > Flutter > GPUI. Flutter's gap to web
  is small (leading defaults, slightly lighter AA). GPUI's gap is the
  rasterizer weight plus real engineering friction (font guard, manual
  metrics, negative scroll offsets, focus quirks).
- **Desktop reader recommendation**: if pixel-fidelity to the web reader is
  the bar, a webview shell (Tauri) is still the only zero-divergence option.
  If a native canvas is wanted, Flutter gets to "indistinguishable from web
  at a glance" far cheaper than GPUI; GPUI is viable but you own Arabic
  typography from here on.

Reproduce the web reference: `python3 -m http.server 8777` from `experiments/`,
then `http://localhost:8777/render-compare/reference/index.html?font=<id>&db=<dataset>`.
