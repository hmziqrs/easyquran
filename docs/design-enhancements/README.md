# EasyQuran: refine the current design

Standalone visual studies using the existing white/black neutrals and four
accent colors. Page order, reader navigation, Quran text and Arabic font remain
unchanged. Production components and tokens are untouched.

Serve from the repository root:

```sh
python3 -m http.server 5392 --bind 127.0.0.1 --directory docs/design-enhancements
```

Open [the comparison](http://127.0.0.1:5392/index.html?concept=color&type=nunito).
Switch card treatment, font, light/dark mode, desktop/mobile width and
homepage/reader view. Current design remains available for comparison.

| Treatment        | Homepage                                                               | Reader                                                                 |
| ---------------- | ---------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| Color cards      | Existing four solid accents in separate 20px cards; 18px chapter cards | Flat outlined verse groups, matching 18px corners, blue selected state |
| Sharper outlines | White cards with colored edges and numbers; 12px corners               | Stronger chapter outline, open verse rows, blue selected state         |

Color cards is the recommended starting point: retain the current identity,
make the metric strip feel like four deliberate shortcuts, soften card corners,
and group each verse with its translation and actions. Sharper outlines explores
a lighter alternative while keeping the same accent palette.

Typography is independent of card treatment: keep current Nunito or compare
Manrope. Both treatments keep the Quran's existing KFGQPC Uthmanic Hafs font and
captured 33px size. All backgrounds and surfaces come from the existing neutral
tokens. Accents use the existing cobalt, emerald, violet and rose tokens.
There is no elevation or replacement palette.

The gallery controls, page theme toggle and Arabic text size controls work.
Other page actions display a visual-preview notice. Snapshots are visual studies,
not a replacement for validating an adopted treatment in the live app,
translated routes and Arabic UI.

`home.html` and `reader.html` contain rendered snapshots from the local app.
`*-base.css` contains its captured CSS; `variants.css` contains the proposals.
Fonts are local. No database files are copied or modified.

Optional [Manrope](https://github.com/google/fonts/tree/main/ofl/manrope) is
self-hosted with its OFL license in `fonts/`. Baseline and Arabic fonts are copied
from the app with their available licenses; the Quran face is the app's existing
King Fahd Complex font.

Web gates pass: `pnpm check`, `pnpm lint`, `pnpm test` (2,209 tests).

Preview validation: 32 desktop/mobile combinations pass without horizontal
overflow. Gallery font, theme, width and view controls work; all seven captured
Arabic verses match the baseline. Comparison images live in `previews/`.
