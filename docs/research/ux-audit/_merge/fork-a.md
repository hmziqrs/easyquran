# Merge notes — Fork A (themes, palettes, Arabic scripts/fonts, display conditions)

New docs: [17-themes-palettes-and-scripts.md](../17-themes-palettes-and-scripts.md), [18-display-conditions.md](../18-display-conditions.md).
New screenshot folders: `screenshots/themes/` (11), `screenshots/scripts/` (9), `screenshots/display/` (5) — ≈ 0.8 MB total, all referenced.

## 1 · New issues (18: 0 P0 · 8 P1 · 7 P2 · 3 P3)

| ID | Title | Sev | File | Anchor |
| --- | --- | --- | --- | --- |
| THEME-01 | Custom colours can make the Qur'an text invisible | P1 (P0 while custom colours stay reader-visible) | 17-themes-palettes-and-scripts.md | `#theme-01--custom-colours-can-make-the-quran-text-invisible` |
| THEME-02 | Custom colours pick the wrong text colour for middle tones | P1 | 17-themes-palettes-and-scripts.md | `#theme-02--custom-colours-pick-the-wrong-text-colour-for-middle-tones` |
| THEME-03 | The focus ring is too faint on dark surfaces (Cobalt, Magenta, Emerald) | P1 | 17-themes-palettes-and-scripts.md | `#theme-03--the-focus-ring-is-too-faint-on-dark-surfaces-cobalt-magenta-emerald` |
| THEME-04 | The chosen palette only reaches the buttons | P2 | 17-themes-palettes-and-scripts.md | `#theme-04--the-chosen-palette-only-reaches-the-buttons` |
| THEME-05 | Ink dark mode turns the home hero into a bright white slab | P2 | 17-themes-palettes-and-scripts.md | `#theme-05--ink-dark-mode-turns-the-home-hero-into-a-bright-white-slab` |
| THEME-06 | See-through text on colour fills fails in Magenta and Emerald | P2 | 17-themes-palettes-and-scripts.md | `#theme-06--see-through-text-on-colour-fills-fails-in-magenta-and-emerald` |
| THEME-07 | The selected palette card's description is 3.7–3.9:1 in dark mode | P2 | 17-themes-palettes-and-scripts.md | `#theme-07--the-selected-palette-cards-description-is-37391-in-dark-mode` |
| SCRIPT-01 | Tajweed colours don't change for dark mode; several letters nearly vanish | P1 | 17-themes-palettes-and-scripts.md | `#script-01--tajweed-colours-dont-change-for-dark-mode-several-letters-nearly-vanish` |
| SCRIPT-02 | The KFGQPC fonts draw the verse-end marker wrongly | P1 | 17-themes-palettes-and-scripts.md | `#script-02--the-kfgqpc-fonts-draw-the-verse-end-marker-wrongly` |
| SCRIPT-03 | IndoPak text with KFGQPC Hafs shows a dotted circle in place of a letter | P1 | 17-themes-palettes-and-scripts.md | `#script-03--indopak-text-with-kfgqpc-hafs-shows-a-dotted-circle-in-place-of-a-letter` |
| SCRIPT-04 | The chosen script and font appear late: Uthmani and Amiri flash first | P2 | 17-themes-palettes-and-scripts.md | `#script-04--the-chosen-script-and-font-appear-late-uthmani-and-amiri-flash-first` |
| SCRIPT-05 | Search results ignore the chosen script and font | P2 | 17-themes-palettes-and-scripts.md | `#script-05--search-results-ignore-the-chosen-script-and-font` |
| SCRIPT-06 | The Bismillah doesn't scale with the Arabic text size | P3 | 17-themes-palettes-and-scripts.md | `#script-06--the-bismillah-doesnt-scale-with-the-arabic-text-size` |
| DISP-01 | The app ignores the browser's text-size setting | P1 | 18-display-conditions.md | `#disp-01--the-app-ignores-the-browsers-text-size-setting` |
| DISP-02 | Windows High Contrast: nothing shows what is selected | P1 | 18-display-conditions.md | `#disp-02--windows-high-contrast-nothing-shows-what-is-selected` |
| DISP-03 | Translation lines are ~100 characters long on wide screens | P2 | 18-display-conditions.md | `#disp-03--translation-lines-are-100-characters-long-on-wide-screens` |
| DISP-04 | The menu panel still slides when "reduce motion" is on | P3 | 18-display-conditions.md | `#disp-04--the-menu-panel-still-slides-when-reduce-motion-is-on` |
| DISP-05 | No response to "increase contrast" | P3 | 18-display-conditions.md | `#disp-05--no-response-to-increase-contrast` |


## 2 · Extensions / corrections to existing findings

| Existing | Add |
| --- | --- |
| A11Y-01 | Magenta dark primary-as-text 2.5–3.0:1 (same failure as Cobalt); Emerald light eyebrow 4.45:1; **Ink passes in both modes** (0 contrast issues in dark on 9/10 routes). Suggest mentioning Ink as the accessible palette today. |
| A11Y-02 | The 4.17:1 green chip and 3.94:1 Juz caption appear identically in all four palettes (hue slots don't change per palette). |
| A11Y-04 | Tiny px sizes can't be rescued by the browser text-size setting (DISP-01). |
| RDR-06 | At 56 px on a 360 px phone, continuous mode justifies two words per line into two columns (`screenshots/scripts/phone-56px-continuous.webp`). |
| RDR-08 | In forced colours the bookmarked state disappears completely (DISP-02). |
| NAV-05 | On a 280 px fold the header forces the page to 351 px, so the whole page is zoomed out (`screenshots/display/fold-280.webp`). |
| VIS-01 | The hard-coded Google-blue translation badge also ignores the palette (THEME-04). |
| VIS-04 | None of the four "selected" styles survive forced colours (DISP-02). |
| VIS-08 | Decorative hue slots contradict the palette choice, esp. Ink "no accent hue" (THEME-04). |
| SET-03 | THEME-01/02 are concrete harms from exposing custom colours; strengthens "hide them". Magenta's "warm reading page" description is shown in all palettes/modes. |
| SET-07 | Font list offers fonts that can't render some scripts (SCRIPT-03) or break markers (SCRIPT-02). |
| HOME-02 | At 2560 px the home content is 1024 px in the middle of a mostly empty page. |
| Correction (docs/design-system.md §5) | Doc says Magenta dark `--primary` is `oklch(0.50 0.23 352)`; code (`layout.css:526`) is `oklch(0.51 0.22 352)`. Cosmetic doc drift. |

## 3 · Roadmap additions

| Wave | ID | Fix | Effort |
| --- | --- | --- | --- |
| 1 (with SET-03) | THEME-01 | Hide custom colours from readers; if kept, derive reader tokens / switch mode | S (hide) · M (derive) |
| 2 | SCRIPT-03 | Restrict fonts per script + build-time glyph-coverage test | M |
| 2 | SCRIPT-02 | Render ayah ornament in a fixed font or SVG, independent of the text font | S–M |
| 2 | SCRIPT-01 | Per-mode tajweed palette as CSS vars + contrast gate | S |
| 2 | THEME-03 | Dark `--focus-ring` → `--primary-legible`; gate ring vs surface ≥ 3:1 | S |
| 2 | THEME-02 | Max-contrast on-colour selection; legible accent derivation; picker warning | M |
| 2 | DISP-01 | Remove `html{font-size:16px}`; rem ramp; relative reader sizes | M–L |
| 2 | DISP-02 | `@media (forced-colors: active)` layer for selected states + swatches | S |
| 3 | THEME-04/05/06/07 | Palette-following decoration; soft fills for large Ink surfaces; no opacity text on fills; gate muted on primary-soft | M |
| 3 | SCRIPT-04/05 | Hide text until variant loads + preload font; search uses preferred source + `--font-quran` | M |
| 3 | DISP-03 | `max-width: 68ch` for translation lines (or scale size) | S |
| 4 | SCRIPT-06, DISP-04, DISP-05 | Bismillah in em; `prefersReducedMotion` in Nav; `prefers-contrast: more` layer | S each |

New guard suggestions for 16 · "Guardrails": contrast pairs for tajweed × mode, focus ring vs surface, muted on primary-soft, derived tokens for random custom seeds; font × script glyph-coverage test; a forced-colors visual snapshot of Settings → Appearance.

## 4 · Coverage

**Checked**
- 80 axe scans: 4 palettes × light/dark × 10 routes (landing, app home, surahs, juz, reader, translated reader, Arabic search results, settings appearance, bookmarks with data, yours with data). Visual review of home/reader/appearance/search per palette; focus ring measured per palette.
- Custom seeds: background `#aaaaaa/#999999/#777777/#202020/#f5f5f5`, accent `#22aaff/#ffe066/#ffd400/#111111/#ff4fa0`, in light and dark (derive.ts evaluated directly + screenshots).
- Scripts uthmani / simple-clean / indopak / tajweed × light/dark; 6 fonts with Uthmani and with IndoPak; Arabic 22/56 px × verse/continuous × 360/390/1440 px; tajweed contrast for all 16 rule colours; font/script swap timeline with CLS (PerformanceObserver); parity of search/sidebar/header/palette with chosen script/font.
- Display: Chrome default font size 16 vs 32 (CDP `Page.setFontSizes`, control page verified), forced-colors (CDP `Emulation.setEmulatedMedia`), prefers-contrast: more, reduced motion on 4 overlays (Web Animations sampled), viewports 2560×1440 … 280×653 incl. phone landscape; overflow scan and characters-per-line measurement.

**Not checked / limits**
- Real Windows High Contrast themes (only Chrome's forced-colors emulation, default system colours).
- Safari/iOS text-size (Dynamic Type) and Android font scale — only Chrome desktop's font-size preference.
- Tajweed/IndoPak/Simple with every font in dark mode and every surah (sampled Al-Baqarah 1–5 and Al-Fātiḥah); glyph coverage was judged visually, not by an automated codepoint scan.
- Timing figures come from the local dev server; production (prerendered + CDN) will be faster, but the order (Uthmani first) is structural.
- Custom-seed scenarios on landing/marketing pages and in Arabic UI.
