# flutter-quran-sample

Flutter 3.47 macOS sample rendering the EasyQuran surah reader (Arabic +
Urdu translation). **This is the chosen desktop approach** — see
[`../render-compare/COMPARE.md`](../render-compare/COMPARE.md) for the
GPUI/Flutter/browser comparison that decided it.

Same font ids as the web reader, same data extract as the gpui-kit sample
(`assets/quran-sample.json`). Dropdowns switch Arabic font (all six web
faces), Urdu face (5 open-license Nastaliq/Urdu faces), and Arabic dataset
(uthmani / simple / indopak); **F9** cycles Arabic font, **F10** dataset,
**F11** Urdu face.

```sh
flutter build macos --debug
open build/macos/Build/Products/Debug/flutter_quran_sample.app
```

Notes:

- Urdu default is **Noto Nastaliq Urdu** (SIL OFL 1.1) — Mehr Nastaliq Web
  (kept as a dropdown option) depends on OpenType chaining contexts that
  CoreText only partially applies, which flattened it in the GPUI sample.
  Under Flutter's HarfBuzz all five Urdu faces render correctly:
  Noto Nastaliq Urdu, Gulzar, Awami Nastaliq, Mehr Nastaliq Web, Lateef —
  all open-licensed (OFL except Mehr).
- Urdu needs a per-font `height` (Noto/Gulzar 2.5, Awami 2.6, Lateef 2.2,
  Mehr 2.9 at 20px) — Flutter does not derive leading from the font's
  vertical metrics the way a browser does.
- Fonts need **no patching** here (GPUI's latin-`m` guard does not exist in
  Flutter's text stack); the TTFs are plain copies.
- Taha v1 / v1 Bold share the internal family `KFGQPC Uthman Taha Naskh` and
  are selected by declared weight in `pubspec.yaml`.
- Urdu text is NFKC-normalized at extraction (`../gpui-kit-sample/data/generate.py`)
  because the junagarhi source stores lam-alef as the isolated presentation
  form U+FEFB, which breaks mid-word in every stack.
