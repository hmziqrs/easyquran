# rune-quran-sample

Experiment: the Flutter/GPUI/NSH surah reader in [Rune](https://github.com/anomalyco/rune),
pinned to `dev` rev `781e2b6d`. macOS only: Rune targets macOS, Windows, Linux and the
web. iOS and Android are later milestones in its `PLAN.md`, with no shell yet. Fonts and
text are read from [`../flutter-quran-sample`](../flutter-quran-sample). Results are in
[`../render-compare/COMPARE.md`](../render-compare/COMPARE.md).

```sh
./bundle.sh
open target/RuneQuran.app
./target/release/rune-quran-sample --list-scenes
./target/release/rune-quran-sample --snap out.png --scene ur-gulzar
```

Notes:

- Text goes through Parley and HarfRust, drawn by Rune's own GPU renderer, not CoreText.
- Rune has no paragraph direction setting that I could find in its source. It detects
  direction from the first strong letter. The ayah marker starts with Arabic digits,
  which are not strong, so it laid out left to right with its brackets swapped until I
  added U+200F. RTL text that starts with Latin or digits needs the same.
- `awami-nastaliq` does not join here, same as NSH, Flutter and Safari.
- `Select::width(220)` did not widen the popup, so long font ids are cut off.
- Selector choices must close the popup in `on_choose`. It stays open otherwise.
- `QURAN_BENCH_REPEAT` and `QURAN_BENCH_CYCLES` drive `../render-compare/bench`.
