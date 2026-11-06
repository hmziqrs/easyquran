# nsh-quran-sample

Experiment: the Flutter/GPUI surah reader in [Native Shell](https://nsh.dev) 0.4.0
(TSX), to check native Arabic and Urdu rendering on macOS and iOS Simulator.
Text goes through CoreText on both. Same font ids, data and selectors as
[`../flutter-quran-sample`](../flutter-quran-sample). Results are in
[`../render-compare/COMPARE.md`](../render-compare/COMPARE.md).

```sh
brew install longbridge/tap/nsh
nsh run . --target ios --device "iPhone 18 Pro"
nsh package . --target macos --run
nsh check . && nsh test .
```

Notes:

- `noto-nastaliq-urdu` is named, not bundled. iOS and macOS ship a font with
  that PostScript name; on iOS, importing a copy fails registration
  (`GSFont: "NotoNastaliqUrdu" already exists`) and the dev host shows an
  uncaught error.
- RTL text needs `textAlign: "right"`. With `writingDirection: "rtl"`,
  `textAlign: "start"` sits flush left.
- `awami-nastaliq` does not join here, and breaks the same way in the Flutter
  sample and Safari. The font has Graphite tables (`Silf`) and no `GSUB`, the
  likely cause. Not specific to NSH or CoreText.
- Latin `m` is blank in these fonts (`../gpui-kit-sample` patch), so mixed
  Latin text loses it. This comes from the font files, not NSH.
- `QURAN_BENCH_REPEAT` and `QURAN_BENCH_CYCLES` drive `../render-compare/bench`.
