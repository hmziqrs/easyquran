# Quran text rendering comparison: GPUI vs Flutter vs browser

Same faces, same data extract (`gpui-kit-sample/data/quran-sample.json`),
same type settings (Arabic 30px / line-height 1.9, Urdu 20px with per-font
Nastaliq leading), macOS + Retina, captured per combo.

- **flutter/** — Flutter 3.47 macOS sample (`../flutter-quran-sample`), Impeller + Skia text stack with bundled HarfBuzz shaping. **The chosen approach.**
- **web/** — minimal HTML page loading the identical TTFs via `@font-face` (`reference/index.html`, regenerate with `reference/generate.py`), Safari/WebKit rendering — the same stack the real web reader uses.
- **gpui/** — gpui-kit 0.8.1 sample (`../gpui-kit-sample`), CoreText shaping + GPUI's own Metal rasterizer. **Frozen and deprecated as an approach** (see Urdu finding below); screenshots kept for the record and predate the Urdu font switch.

## Urdu faces

- **Noto Nastaliq Urdu** (default everywhere) — SIL OFL 1.1, shapes correctly
  under HarfBuzz (Flutter + browser) and acceptably under CoreText.
- **Mehr Nastaliq Web** — selectable option. Its heavy OpenType chaining
  contexts (cursive attachment + `dist`) render correctly under HarfBuzz but
  flatten into stretched, disconnected forms under CoreText — that is the
  "Urdu looks horrible in the desktop app" symptom that killed the GPUI
  path. HarfBuzz also documents CoreText mismatches for Noto Nastaliq
  chaining rules (harfbuzz#2516), so any CoreText-based Nastaliq needs
  visual review, not assumptions.
- Flutter sample: `Urdu` dropdown + **F11** cycles faces. Web reference:
  `&urdu=<id>` query param.

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
| Mehr Urdu in Flutter (vs Noto) | [10](flutter/10-mehr-urdu-in-flutter.png)        | [09](web/09-mehr-urdu-in-web.png)            | —                                             |

Every cell: correct joining, correct harakat stacking, correct RTL alignment,
correct uthmani hamzat-wasl (ٱ), correct IndoPak small-kashida marks, ayah
markers ﴿١﴾ composed, Urdu Nastaliq legible. **No framework produced broken
Arabic.**

## Findings

0. **Urdu face choice is the real desktop risk.** Mehr Nastaliq Web renders
   fine under HarfBuzz (browser, Flutter) but flattens under CoreText —
   the GPUI "horrible Urdu". Noto Nastaliq Urdu (OFL) survives all stacks;
   it is the default in the Flutter sample and the web reference. License
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
5. **Cross-platform.** Flutter and web are engine-identical on every OS.
   CoreText-based stacks diverge on Nastaliq chaining rules by design.

## Verdict

- Flutter + Noto Nastaliq Urdu is the desktop approach: correct Quran text in
  all six web Arabic faces and all three datasets, Urdu Nastaliq that reads
  like the browser, open licenses throughout.
- If pixel-fidelity to the web reader ever becomes the bar again, a webview
  shell (Tauri) is the zero-divergence option.

Reproduce the web reference: `python3 -m http.server 8777` from `experiments/`,
then `http://localhost:8777/render-compare/reference/index.html?font=<id>&urdu=<urdu id>&db=<dataset>`.
