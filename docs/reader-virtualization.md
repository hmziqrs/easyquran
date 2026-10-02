# Surah reader virtualization

Verse mode renders measured ayahs as sibling `li` elements in one `ol`. Mushaf page
numbers remain metadata on each row; the header, URL, history, and worker reads still
use that metadata. Reading mode renders measured page sections containing inline text
spans. It does not instantiate verse actions, translation lanes, or note editors.

Both use the existing TanStack Svelte Virtual dependency and window scrolling. The
rendered range covers the viewport plus 80% of its height on each side, bounded to
500–1200px. Item count has no fixed cap, so short ayahs and tall screens remain covered.
One focused row can stay mounted outside the range; a margin represents the omitted
items between it and the viewport. List padding represents everything else omitted.

ResizeObserver measures mounted items. Layout changes invalidate measurements;
ordinary height changes above the viewport receive scroll compensation. Explicit
font, mode, and note changes preserve the existing ayah text anchor. Deep links first
load and mount their target. A jump selects the contiguous loaded page span around
that target, so scrolling backward requests its preceding page instead of skipping
across a gap to previously cached content.

Primary text still loads by page with 900px of lookahead. Arabic companion and stacked
translation reads follow mounted page coverage. Data batching and DOM virtualization
remain separate. Reading fetches only its selected translation, and loads the Naskh
translation font only when visible content needs it. History retains a bounded page
snapshot for restoration.

## Browser comparison

Measured in isolated Chromium sessions on October 2, 2026, with identical source,
fonts, and all seven As-Saffat pages loaded. Counts cover `.reader-pages` descendants;
these are DOM measurements, not production frame-rate or memory benchmarks.

| Mode | Viewport | Previous mounted ayah rows | Current mounted ayah rows | Previous DOM elements | Current DOM elements |
| --- | --- | ---: | ---: | ---: | ---: |
| Verse | 1280 × 720 | 182 | 13 | 3301 | 239 |
| Verse | 390 × 844 | 182 | 13 | 3301 | 239 |
| Reading | 1280 × 720 | 182 | 0 | 3301 | 235 |

Reading rendered three text-only page sections at the measured position, with zero
verse-action buttons; the previous renderer mounted seven pages and 728 hidden action
buttons. Desktop mode switching preserved the tracked text point within 2px. Mobile
font enlargement on long ayah 2:282 preserved it within 1px.

Regression tests cover short-item viewport coverage, tall screens, focus retention
without mounting intervening rows, distant targets, flat and inline text anchors,
and cold-worker recovery followed by backward loading after a jump.

Quran.com's public implementation uses the same useful division: ayah items in
[TranslationView](https://github.com/quran/quran.com-frontend-next/blob/production/src/components/QuranReader/TranslationView/index.tsx)
and page items in
[ReadingView](https://github.com/quran/quran.com-frontend-next/blob/production/src/components/QuranReader/ReadingView/index.tsx).
Its Arabic reading pages use explicit mushaf lines. EasyQuran keeps its existing
natural text wrapping.
