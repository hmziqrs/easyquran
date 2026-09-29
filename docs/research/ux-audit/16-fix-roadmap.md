# 16 · Fix roadmap

[← Back to the index](README.md)

Grouped so each wave is shippable on its own. Effort: **S** ≤ ½ day · **M** 1–3 days · **L** ≥ 1 week. All changes must keep `pnpm check`, `pnpm lint`, `pnpm test` green (repo rule), and respect the reader-nav, no-nested-ternary and hotkey rules in `AGENTS.MD`.

## Wave 1 — Trust and blockers (P0)

| ID | Fix | Effort | Main files |
| --- | --- | --- | --- |
| [RDR-03](03-reader.md#rdr-03--the-tafsir-panel-shows-placeholder-text-to-real-readers) | Hide placeholder tafsir; rename action to "Add note" | S | `lib/data/quran.ts`, `VerseTools.svelte` |
| [RDR-01](03-reader.md#rdr-01--the-floating-appearance-button-covers-the-quran-text-on-phones) | Remove floating appearance button (or move to reader "Aa") | S | `app/+layout.svelte`, `tweaks/Tweaks.svelte` |
| [STATE-01](12-states-errors-offline.md#state-01--wrong-reader-urls-return-a-plain-text-not-found) | Styled 404 for browsers; redirects for `/app/1`, bad juz/page | S | `hooks.server.ts`, `+error.svelte` |
| [NAV-02](01-navigation-and-wayfinding.md#nav-02--two-url-schemes-some-links-lose-the-language-some-return-a-bare-not-found) / [RTL-04](10-arabic-and-rtl.md#rtl-04--settings-search-bookmarks-yours-english-body-and-english-after-refresh) | Localized routes for search/bookmarks/yours/settings; stop locale loss | M | `paraglide.config.js`, `app/+layout.svelte`, those pages |
| [SRCH-01](05-search.md#srch-01--searching-an-english-word-returns-nothing-with-no-hint-why) | Search default translation for Latin queries; helpful empty state | M | `app/search/*`, search worker |
| [TR-01](04-translations.md#tr-01--picking-a-translation-removes-the-arabic-text-and-the-bismillah) | Arabic + translation in translated reader; keep Bismillah | M | `SurahReader.svelte`, `VerseRow.svelte` |

## Wave 2 — Readability and access (P1)

| ID | Fix | Effort |
| --- | --- | --- |
| [A11Y-01](09-accessibility.md#a11y-01--dark-mode-uses-the-fill-blue-as-a-text-colour) | `--primary-legible` token + contrast test pair | S |
| [A11Y-03](09-accessibility.md#a11y-03--tap-targets-are-well-below-the-promised-44-px) | 44 px targets via primitives; size guard test | M |
| [A11Y-04](09-accessibility.md#a11y-04--too-much-text-is-1113-px) | Text floor 13.5 / 15 px; ramp roles | M |
| [RDR-02](03-reader.md#rdr-02--verse-actions-are-tiny-unlabeled-and-ambiguous) | Verse "More" sheet with labelled actions | M |
| [RDR-05](03-reader.md#rdr-05--text-size-and-mode-controls-are-small-and-unclear) | Reader "Aa" options panel | M |
| [RDR-09](03-reader.md#rdr-09--the-sidebar-spells-some-arabic-surah-names-differently-from-the-rest-of-the-app) | One surah-name source + test | S |
| [RDR-10](03-reader.md#rdr-10--dark-mode-ayah-markers-and-the-wordmark-are-too-faint) | (covered by A11Y-01) | — |
| [NAV-01](01-navigation-and-wayfinding.md#nav-01--the-header-never-shows-which-section-you-are-in) | Current-page state + `aria-current` | S |
| [NAV-05](01-navigation-and-wayfinding.md#nav-05--header-overflows-on-small-phones) | Header fits 320 px | S |
| [NAV-06](01-navigation-and-wayfinding.md#nav-06--the-reader-sub-bar-hides-two-key-tools-behind-unexplained-icons) | Labelled Surahs + Translation buttons | S |
| [HOME-01](02-home.md#home-01--once-you-have-read-anything-the-browse-shortcuts-disappear) | Continue card above, not instead of, shortcuts | S |
| [HOME-04](02-home.md#home-04--new-readers-are-never-asked-do-you-read-arabic) | "Read in: Arabic / Arabic + English…" first-run choice | M |
| [TR-02](04-translations.md#tr-02--the-main-translation-is-never-credited) / [TR-03](04-translations.md#tr-03--the-translation-picker-opens-on-arabic-tafsir-uses-flags-and-says-primary) | Credit translator; picker opens on UI language, no flags | M |
| [TR-06](04-translations.md#tr-06--when-the-api-is-unreachable-translated-pages-fail-without-falling-back-to-arabic) / [STATE-03](12-states-errors-offline.md#state-03--account-page-spins-forever-when-the-api-is-unreachable) | Graceful API-down fallbacks | M |
| [SRCH-02](05-search.md#srch-02--the-k-palette-dead-ends-on-words) | Palette → "Search all verses for …" | S |
| [LIST-01](06-browse-lists.md#list-01--every-surah-has-two-or-three-english-spellings) / [LIST-03](06-browse-lists.md#list-03--juz-list-is-written-in-reference-code) | One transliteration; human juz ranges | M |
| [BM-03](07-bookmarks-notes-yours.md#bm-03--notes-are-saved-but-never-shown-anywhere) | Notes section on Yours | M |
| [SET-01](08-settings-and-appearance.md#set-01--settings-opens-on-storage-the-most-technical-tab) … [SET-04](08-settings-and-appearance.md#set-04--toggles-are-on-text-pills-analytics-is-on-by-default), [SET-08](08-settings-and-appearance.md#set-08--a-second-settings-floats-over-every-page) | Reorder tabs, plain copy, hide dev tools, real switches | M |
| [RTL-01](10-arabic-and-rtl.md#rtl-01--app-home-hero-and-continue-card-are-english) … [RTL-03](10-arabic-and-rtl.md#rtl-03--surah-list-metadata-is-english-in-arabic-ui) | Translate hard-coded strings; Arabic-first names in Arabic UI | M |
| [STATE-02](12-states-errors-offline.md#state-02--the-error-page-is-a-dead-end) | Error page inside layout with ways out | S |
| [AUTH-01](13-sign-in-and-account.md#auth-01--no-way-back-from-sign-in--create-account) | Back/close on auth pages | S |
| [MKT-02](14-marketing-site-parity.md#mkt-02--website-copy-is-out-of-date-with-the-app) | Update About/FAQ copy | S |

## Wave 3 — Consistency (P2)

One **"parity sprint"** that builds/uses shared components, then sweeps call sites:

1. `PageHeader` (title, subtitle, actions) + two centred widths → VIS-06, LIST-06, BM-01.
2. `SearchField` → VIS-03, SRCH-03, SRCH-05.
3. `SegmentedControl` / `OptionCard` with one selected style → VIS-04, RDR-05.
4. `Button` variants only (no ad-hoc) → VIS-05, AUTH-03, TR-03 (Done).
5. `EmptyState` → STATE-06.
6. `VerseRef` (one format: "Al-Baqarah · verse 1") → BM-01, RDR-11, search results.
7. Icon family cleanup → VIS-01, VIS-02.
8. Neutral numbering, no decorative hue → LIST-02, RDR-04, VIS-08.
9. Remaining P2 items: HOME-02/03/06, RDR-06/07/08, TR-04/05, SRCH-04, LIST-04/05, BM-02/04, SET-05/06/07, A11Y-02/05/06/07, RTL-05/06, STATE-05, AUTH-02/04, MKT-01/03, NAV-03/04/07.

## Wave 4 — Polish (P3)

RDR-12, LIST-06, BM-05, RTL-07, VIS-09, VIS-10, AUTH-05, MKT-04.

## Guardrails to add (so it stays fixed)

| Guard | Catches |
| --- | --- |
| `token-contrast.test.ts`: add `--primary-legible` on ground, `--hue-N-legible` on `--hue-N-soft` (light) | A11Y-01, A11Y-02 |
| Playwright/Puppeteer target-size check at 390 px on `/app/*` | A11Y-03 |
| Lint/test: ban `text-[<13.5px]` and `font-mono` in `routes/(application)` except allow-list | A11Y-04, VIS-07 |
| i18n test: no string literals in `.svelte` under `routes/(application)` (allow-list punctuation) | HOME-05, RTL-01..03 |
| Catalogue test: sidebar, list, palette render identical surah names | RDR-09, LIST-01 |
| axe run in CI on 10 key routes × 2 themes (fail on serious/critical) | regressions |
| Route test: every `/en/app/*` and `/ar/app/*` 404 returns HTML with the app header | STATE-01, NAV-02 |

## Second pass additions (docs 17–26)

The second pass added 107 findings. They slot into the same four waves. Two cross-cutting notes first:

- **Before removing the floating button ([RDR-01](03-reader.md#rdr-01--the-floating-appearance-button-covers-the-quran-text-on-phones) / [SET-08](08-settings-and-appearance.md#set-08--a-second-settings-floats-over-every-page)):** `OfflinePackBar` and `Notifications` are mounted inside `web/src/lib/components/tweaks/Tweaks.svelte` (≈ lines 331, 376). Move them to the root layout first, or the offline-pack progress UI disappears with the button.
- **Hide custom colours ([SET-03](08-settings-and-appearance.md#set-03--designer-and-developer-tools-are-exposed-to-readers)) in Wave 1** — it also closes [THEME-01](17-themes-palettes-and-scripts.md#theme-01--custom-colours-can-make-the-quran-text-invisible) and [THEME-02](17-themes-palettes-and-scripts.md#theme-02--custom-colours-pick-the-wrong-text-colour-for-middle-tones).

### Wave 1 — Trust and blockers (P0) — additions

| IDs | Fix | Effort |
| --- | --- | --- |
| [THEME-01](17-themes-palettes-and-scripts.md#theme-01--custom-colours-can-make-the-quran-text-invisible) | Hide custom colours from readers; if kept, derive reader tokens / switch mode | S (hide) · M (derive) |
| [SCR-01](19-remaining-screens-and-flows.md#scr-01--legal-pages-say-placeholder-text-and-promise-things-the-app-doesnt-do) | legal copy review + single contact | S (copy) + legal review |
| [TRX-01](22-translations-deep-dive.md#trx-01--arabic-commentary-chosen-as-the-main-translation-looks-like-quran-text) | Tafsir never as the main text; label commentary; redirect `/t/ar/muyassar`, `/t/ar/jalalayn` | S |
| [PWA-01](25-pwa-and-page-metadata.md#pwa-01--the-home-screen-icon-and-favicons-are-broken-off-brand-artwork) | Re-export all icons/favicons/logo from the current ق mark; separate maskable icons | S |

### Wave 2 — Readability and access (P1) — additions

| IDs | Fix | Effort |
| --- | --- | --- |
| [SCRIPT-03](17-themes-palettes-and-scripts.md#script-03--indopak-text-with-kfgqpc-hafs-shows-a-dotted-circle-in-place-of-a-letter) | Restrict fonts per script + build-time glyph-coverage test | M |
| [SCRIPT-02](17-themes-palettes-and-scripts.md#script-02--the-kfgqpc-fonts-draw-the-verse-end-marker-wrongly) | Render ayah ornament in a fixed font or SVG, independent of the text font | S–M |
| [SCRIPT-01](17-themes-palettes-and-scripts.md#script-01--tajweed-colours-dont-change-for-dark-mode-several-letters-nearly-vanish) | Per-mode tajweed palette as CSS vars + contrast gate | S |
| [THEME-03](17-themes-palettes-and-scripts.md#theme-03--the-focus-ring-is-too-faint-on-dark-surfaces-cobalt-magenta-emerald) | Dark `--focus-ring` → `--primary-legible`; gate ring vs surface ≥ 3:1 | S |
| [THEME-02](17-themes-palettes-and-scripts.md#theme-02--custom-colours-pick-the-wrong-text-colour-for-middle-tones) | Max-contrast on-colour selection; legible accent derivation; picker warning | M |
| [DISP-01](18-display-conditions.md#disp-01--the-app-ignores-the-browsers-text-size-setting) | Remove `html{font-size:16px}`; rem ramp; relative reader sizes | M–L |
| [DISP-02](18-display-conditions.md#disp-02--windows-high-contrast-nothing-shows-what-is-selected) | `@media (forced-colors: active)` layer for selected states + swatches | S |
| [SCR-05](19-remaining-screens-and-flows.md#scr-05--nothing-tracking-what-you-read-while-analytics-is-on-by-default) | analytics copy or opt-in + path scrubbing | S–M |
| [FLOW-01](19-remaining-screens-and-flows.md#flow-01--clear-cached-pages--data--no-confirmation-no-visible-result) | "Free up space" rename, confirm, refreshed numbers | S |
| [FLOW-03](19-remaining-screens-and-flows.md#flow-03--search-has-a-second-different-translation-picker--and-it-needs-two-steps) | one translation picker (reuse reader modal), one-step add | M |
| [ACCT-01](20-signed-in-experience.md#acct-01--signing-out-silently-removes-your-account-bookmarks-from-the-device) | sign-out keep-a-copy / explanation | S–M |
| [ACCT-02](20-signed-in-experience.md#acct-02--the-account-page-is-a-bare-developer-screen) | account page in app layout, human sessions, change password, delete account | M–L |
| [ACCT-08](20-signed-in-experience.md#acct-08--server-and-configuration-errors-reach-readers-raw-or-mislabelled) | enabled-providers list, OAuth error redirect, honest error mapping | S–M |
| [KEY-01](21-keyboard-focus-and-screen-reader.md#key-01--screen-readers-are-interrupted-with-page-n-of-48-while-you-scroll) | Stable `<title>` while scrolling; no assertive announcements on scroll | S |
| [KEY-03](21-keyboard-focus-and-screen-reader.md#key-03--every-verse-adds-four-tab-stops-with-identical-names) | One Tab stop per verse (roving tabindex) + verse number in action names + skip-to-page-nav | M |
| [TRX-02](22-translations-deep-dive.md#trx-02--the-transliteration-shows-raw-html-tags) | View-layer parser for transliteration `<u>`/`<b>` markup (data stays immutable) | S |
| [TRX-03](22-translations-deep-dive.md#trx-03--the-same-translator-is-listed-twice-and-some-languages-twice) | De-duplicate catalogue by (language, translator); merge split languages; keep aliases | M |
| [INT-01](23-interaction-details.md#int-01--a-shared-link-to-a-long-verse-opens-in-the-wrong-place-on-phones) | Deep-link scroll after layout settles; `scroll-margin-top` from the real bar height | S |
| [LOAD-01](24-loading-and-perceived-performance.md#load-01--the-reader-shows-text-for-25-s-before-its-controls-exist-then-the-page-jumps) | Render reader controls in SSR (behaviour after hydration); reserve space; CSS-driven theme icon | M |
| [LOAD-02](24-loading-and-perceived-performance.md#load-02--preparing-offline-quran-sits-on-top-of-the-header-for-the-whole-download) | Move download progress under the header / to a bottom toast; localize; silent background downloads | S |
| [LOAD-03](24-loading-and-perceived-performance.md#load-03--a-shared-search-link-ignores-its-query-until-the-app-wakes-up) | SSR the `?q=` value; "Searching…" status with progress; results `min-height` | S |
| [PWA-02](25-pwa-and-page-metadata.md#pwa-02--manifest-installed-app-always-opens-in-english-old-colours-brand-spelling) | Locale-aware `start_url`, resume shortcut, v2 colours, Arabic manifest | S |
| [PWA-03](25-pwa-and-page-metadata.md#pwa-03--tab-titles-follow-five-patterns-several-pages-have-no-title) | One title pattern for all routes + test | S |
| [PWA-06](25-pwa-and-page-metadata.md#pwa-06--in-the-installed-app-some-screens-have-no-way-back) | Visible exits on auth/account/404 screens; standalone back affordance | S |
| [BRW-01](26-cross-browser.md#brw-01--every-text-field-is-under-16-px--iphones-zoom-in-on-tap) | ≥ 16 px inputs on touch devices | S |

### Wave 3 — Consistency (P2) — additions

| IDs | Fix | Effort |
| --- | --- | --- |
| [THEME-04](17-themes-palettes-and-scripts.md#theme-04--the-chosen-palette-only-reaches-the-buttons)/05/06/07 | Palette-following decoration; soft fills for large Ink surfaces; no opacity text on fills; gate muted on primary-soft | M |
| [SCRIPT-04](17-themes-palettes-and-scripts.md#script-04--the-chosen-script-and-font-appear-late-uthmani-and-amiri-flash-first)/05 | Hide text until variant loads + preload font; search uses preferred source + `--font-quran` | M |
| [DISP-03](18-display-conditions.md#disp-03--translation-lines-are-100-characters-long-on-wide-screens) | `max-width: 68ch` for translation lines (or scale size) | S |
| [SCR-02](19-remaining-screens-and-flows.md#scr-02--legal-pages-are-hard-to-read), [SCR-03](19-remaining-screens-and-flows.md#scr-03--auth-side-pages-look-like-a-different-product), [SCR-04](19-remaining-screens-and-flows.md#scr-04--landing-lower-sections-wrong-text-colour-a-self-link-a-very-long-list), [SCR-06](19-remaining-screens-and-flows.md#scr-06--every-reader-type-ends-differently--and-the-end-of-the-quran-is-a-small-link), [FLOW-02](19-remaining-screens-and-flows.md#flow-02--offline-pack-no-progress-no-ready-moment-developer-wording), [FLOW-04](19-remaining-screens-and-flows.md#flow-04--storage-download-rows-jargon-chips-and-an-unexplained-disabled-button), [FLOW-05](19-remaining-screens-and-flows.md#flow-05--shared-verses-carry-no-link-back), [FLOW-06](19-remaining-screens-and-flows.md#flow-06--k-go-to-verse-lands-on-the-page-not-the-verse-deep-links-dont-highlight), [FLOW-07](19-remaining-screens-and-flows.md#flow-07--switching-language-throws-away-your-reading-position), [FLOW-08](19-remaining-screens-and-flows.md#flow-08--many-bookmarks-become-an-unsorted-wall), [FLOW-09](19-remaining-screens-and-flows.md#flow-09--update-and-notification-toasts-cover-the-header-and-have-tiny-controls), [ACCT-03](20-signed-in-experience.md#acct-03--nothing-shows-that-youre-signed-in), [ACCT-04](20-signed-in-experience.md#acct-04--bookmark-folders-squashed-add-button-disguised-dropdowns-repeated-labels), [ACCT-06](20-signed-in-experience.md#acct-06--auth-forms-tiny-errors-mixed-icons-hidden-password-rule), [ACCT-07](20-signed-in-experience.md#acct-07--verification-and-reset-codes-dont-match-and-the-copy-talks-to-strangers), [ACCT-09](20-signed-in-experience.md#acct-09--signing-in-merges-local-bookmarks--say-so-and-say-what-doesnt-sync) | See each finding's **Fix** | S–M each |
| [KEY-02](21-keyboard-focus-and-screen-reader.md#key-02--two-skip-to-content-links--and-in-arabic-the-first-one-is-english), 04, 05, 06, 09, 10, 11, 12 | One localized skip link; card focus ring; palette focus return; `inert` background for all modals; shortcuts help + single-key toggle + Ctrl/⌘ B via `registerHotkey`; names/headings sweep | M |
| [TRX-04](22-translations-deep-dive.md#trx-04--picker-search-misses-common-spellings), 05, 06, 09, 10, 11, 12, 13 | Picker aliases/fuzzy search; wrapping chip list; remove dead ×; keep verse on switch + readable slugs; persist `translationFamily`; empty-verse notice + fixed `dir`; RTL size/Nastaliq; credit on its own line | M |
| [INT-02](23-interaction-details.md#int-02--the-highlight-on-a-linked-verse-is-nearly-invisible), 03, 04, 08 | Stronger linked-verse highlight; Back after Next surah restores; verse actions in reading mode; floating button under scrims (or removed) | S–M |
| [LOAD-04](24-loading-and-perceived-performance.md#load-04--the-ui-font-arrives-late-and-moves-the-page), [LOAD-05](24-loading-and-perceived-performance.md#load-05--67-seconds-of-white-screen-on-a-slow-first-visit) | Preload Nunito + metric-matched fallback; trim eager JS chunks; compress SSR | M |
| [PWA-04](25-pwa-and-page-metadata.md#pwa-04--browserstatus-bar-colour-never-matches-the-chosen-theme), [PWA-05](25-pwa-and-page-metadata.md#pwa-05--no-help-to-install-the-app), [PWA-08](25-pwa-and-page-metadata.md#pwa-08--the-update-notice-speaks-in-tabs-and-has-a-tiny-button), [PWA-09](25-pwa-and-page-metadata.md#pwa-09--link-previews-one-old-image-for-every-page) | Runtime theme-color; install card; plain update toast; new/per-surah OG images | M |
| [BRW-02](26-cross-browser.md#brw-02--continuous-reading-mode-breaks-arabic-lines-differently-per-engine), [BRW-04](26-cross-browser.md#brw-04--firefox-is-untested--code-level-risk-list) | No justification on narrow screens; real Firefox pass + Share label | S |

### Wave 4 — Polish (P3) — additions

| IDs | Fix | Effort |
| --- | --- | --- |
| [SCRIPT-06](17-themes-palettes-and-scripts.md#script-06--the-bismillah-doesnt-scale-with-the-arabic-text-size), [DISP-04](18-display-conditions.md#disp-04--the-menu-panel-still-slides-when-reduce-motion-is-on), [DISP-05](18-display-conditions.md#disp-05--no-response-to-increase-contrast) | Bismillah in em; `prefersReducedMotion` in Nav; `prefers-contrast: more` layer | S each |
| [SCR-07](19-remaining-screens-and-flows.md#scr-07--special-quran-moments-arent-explained), [SCR-08](19-remaining-screens-and-flows.md#scr-08--the-reading-mode-dialog-uses-its-own-button-and-list-styles), [ACCT-05](20-signed-in-experience.md#acct-05--sync-status-is-good-but-small) | See each finding's **Fix** | S |
| [KEY-07](21-keyboard-focus-and-screen-reader.md#key-07--focus-wanders-behind-the-floating-appearance-panel), 08, 13, 14, 15 · [TRX-07](22-translations-deep-dive.md#trx-07--at-the-5-translation-cap-disabled-boxes-look-enabled-and-the-message-is-jargon), 08, 14, 15, 16 · [INT-05](23-interaction-details.md#int-05--what-copy-puts-on-the-clipboard), 06, 07, 09, 10, 11 | Polish sweep: first-focus targets, title pattern, `aria-expanded`, sign-in rule, cap UI, compare mode, copy format, pressable recipe + `cursor:pointer`, one scrim, one spinner/skeleton, end-of-Qur'an block | M |
| [LOAD-06](24-loading-and-perceived-performance.md#load-06--ayah-markers-change-shape-during-loading), [LOAD-07](24-loading-and-perceived-performance.md#load-07--without-javascript-the-app-is-always-dark), [PWA-07](25-pwa-and-page-metadata.md#pwa-07--safe-area-padding-is-dead-code-no-viewport-fit), [PWA-10](25-pwa-and-page-metadata.md#pwa-10--personal-pages-are-indexable-searchsettings-lack-robots-rules), [BRW-03](26-cross-browser.md#brw-03--viewport-height-and-tap-details-for-ios) | Marker font preload; no-JS OS theme; `viewport-fit=cover` + safe areas; robots; `dvh` + tap highlight | S |

### More guardrails

| Guard | Catches |
| --- | --- |
| Contrast pairs: Tajweed colours × mode; focus ring vs every surface; muted text on `--primary-soft`; tokens derived from random custom seeds | [SCRIPT-01](17-themes-palettes-and-scripts.md#script-01--tajweed-colours-dont-change-for-dark-mode-several-letters-nearly-vanish), [THEME-02](17-themes-palettes-and-scripts.md#theme-02--custom-colours-pick-the-wrong-text-colour-for-middle-tones), [THEME-03](17-themes-palettes-and-scripts.md#theme-03--the-focus-ring-is-too-faint-on-dark-surfaces-cobalt-magenta-emerald), [THEME-07](17-themes-palettes-and-scripts.md#theme-07--the-selected-palette-cards-description-is-37391-in-dark-mode) |
| Font × script glyph-coverage test (no `.notdef` / U+25CC dotted circle) | [SCRIPT-02](17-themes-palettes-and-scripts.md#script-02--the-kfgqpc-fonts-draw-the-verse-end-marker-wrongly), [SCRIPT-03](17-themes-palettes-and-scripts.md#script-03--indopak-text-with-kfgqpc-hafs-shows-a-dotted-circle-in-place-of-a-letter) |
| Forced-colors visual snapshot of Settings → Appearance and the reader mode toggle | [DISP-02](18-display-conditions.md#disp-02--windows-high-contrast-nothing-shows-what-is-selected) |
| No `html { font-size: <px> }`; reader sizes in rem/em | [DISP-01](18-display-conditions.md#disp-01--the-app-ignores-the-browsers-text-size-setting) |
| Announcer-mutation test while scrolling the reader (no assertive announcements) | [KEY-01](21-keyboard-focus-and-screen-reader.md#key-01--screen-readers-are-interrupted-with-page-n-of-48-while-you-scroll) |
| Accessibility-tree test: no duplicate button names on a reader page | [KEY-03](21-keyboard-focus-and-screen-reader.md#key-03--every-verse-adds-four-tab-stops-with-identical-names), [KEY-10](21-keyboard-focus-and-screen-reader.md#key-10--vague-or-duplicated-control-names) |
| Catalogue test: no duplicate (language, translator); no `<` in rendered transliteration | [TRX-02](22-translations-deep-dive.md#trx-02--the-transliteration-shows-raw-html-tags), [TRX-03](22-translations-deep-dive.md#trx-03--the-same-translator-is-listed-twice-and-some-languages-twice) |
| Round-trip persistence test for every reader setting | [TRX-10](22-translations-deep-dive.md#trx-10--the-serif-translation-font-is-forgotten-after-a-reload) |
| Every browser 404 under `/en|ar/app` (incl. trailing slash, capitals) returns HTML with the app header; OAuth start routes never return JSON to a navigation | [STATE-01](12-states-errors-offline.md#state-01--wrong-reader-urls-return-a-plain-text-not-found), [ACCT-08](20-signed-in-experience.md#acct-08--server-and-configuration-errors-reach-readers-raw-or-mislabelled) |
| Lighthouse/CDP CI on the production build (Slow 4G phone): CLS ≤ 0.1 on `/`, `/en/app`, `/en/app/al-baqarah`, `/app/search?q=…` | [LOAD-01](24-loading-and-perceived-performance.md#load-01--the-reader-shows-text-for-25-s-before-its-controls-exist-then-the-page-jumps), [LOAD-03](24-loading-and-perceived-performance.md#load-03--a-shared-search-link-ignores-its-query-until-the-app-wakes-up) |
| Every route has a non-empty `<title>` in one pattern ending in the brand | [PWA-03](25-pwa-and-page-metadata.md#pwa-03--tab-titles-follow-five-patterns-several-pages-have-no-title), [KEY-13](21-keyboard-focus-and-screen-reader.md#key-13--browser-tab-titles-follow-five-different-patterns) |
| `input`/`textarea` compute ≥ 16 px at 390 px width | [BRW-01](26-cross-browser.md#brw-01--every-text-field-is-under-16-px--iphones-zoom-in-on-tap) |

## Validate with real people

After Wave 1–2, run 5 short sessions (20 min, remote is fine) with the real audience — ideally two older readers, one Arabic-first reader, one who can't read Arabic, one phone-only user. Tasks:

1. "Open Surah Al-Kahf and read verse 10 with an English translation."
2. "Save this verse so you can find it tomorrow, then find it."
3. "Make the Arabic text bigger."
4. "Find verses about patience."
5. "Switch the app to Arabic, close it, open it again."

Success = each task in under a minute without help. Note every hesitation; that list is the next audit.
