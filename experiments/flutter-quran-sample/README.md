# flutter-quran-sample

Flutter 3.47 macOS sample rendering the EasyQuran surah reader (Arabic +
Urdu translation) for the rendering comparison in
[`../render-compare/COMPARE.md`](../render-compare/COMPARE.md).

Same font ids as the web reader, same data extract as the gpui-kit sample
(`assets/quran-sample.json`). Dropdowns switch Arabic font (all six web
faces) and Arabic dataset (uthmani / simple / indopak); **F9** cycles font,
**F10** cycles dataset.

```sh
flutter build macos --debug
open build/macos/Build/Products/Debug/flutter_quran_sample.app
```

Notes:

- Fonts need **no patching** here (GPUI's latin-`m` guard does not exist in
  Flutter's text stack); the TTFs are plain copies of the converted faces.
- Taha v1 / v1 Bold share the internal family `KFGQPC Uthman Taha Naskh` and
  are selected by declared weight in `pubspec.yaml`.
- Mehr Nastaliq Web needs a manual `height` (~2.4–2.9 at 20px) or diacritics
  collide — Flutter does not derive leading from the font's vertical metrics
  the way a browser does.
