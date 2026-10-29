# gpui-kit Quran reader sample

Experiment: render the EasyQuran surah reader (Arabic Uthmani + Urdu
translation) in a native GPUI app via [gpui-kit](https://gpui-kit.com).

![what it does]: Al-Fatihah (all 7 ayahs) + Al-Baqarah 1–10, each ayah showing
Arabic (KFGQPC Uthmanic Hafs) above Urdu (Mehr Nastaliq Web), right-aligned
RTL, scrollable, PageDown/PageUp bindings.

## Run

```sh
cargo build
open QuranGpui.app   # wrapper bundle; `cargo run` also works
```

Rebuild the bundle binary after code changes:
`cargo build && cp target/debug/gpui-kit-quran-sample QuranGpui.app/Contents/MacOS/`

## Findings (macOS / gpui-kit 0.8.1)

1. **Arabic shaping is correct.** GPUI's Apple text system shapes through
   CoreText, so Uthmani harakat, lam-alef ligatures and the ﴿٢﴾ ayah markers
   all render properly. RTL alignment works via `.text_align(TextAlign::Right)`.
2. **GPUI refuses fonts without a Latin 'm'.** `AppleTextSystem::load_family`
   skips any font whose cmap lacks `m` (its em-measurement guard) — and both
   Quran fonts have no Latin at all, so they silently fall back to system
   fonts. Fix: `fonts/*-appui.ttf` are copies with `m` mapped to the space
   glyph (see `fonts/` and `main.rs`). Symptom before the patch: everything
   renders, but in Geeza Pro instead of the loaded face.
3. **Scroll offsets are negative.** `ScrollHandle` offsets run from `0` down to
   `-max_offset.y`; positive values clamp straight back to the top.
4. **Manual scroll changes need `cx.notify()`** after `set_offset`.
5. **Nastaliq metrics:** Mehr Nastaliq Web needs generous line height
   (~2.9× at 20px) or diacritics collide with the next line.
6. **Keyboard for unsigned binaries:** macOS accessibility input refuses
   bare executables; wrap in `QuranGpui.app` (Info.plist with a bundle id)
   so tools (and LaunchServices) can target it.

## Data

`data/quran-sample.json` is generated from the repo's immutable DBs
(`db/quran/arabic/quran-uthmani.sqlite` +
`db/quran/translations/sqlite/ur.junagarhi.sqlite`) and embedded at compile
time. Databases themselves are untouched.
