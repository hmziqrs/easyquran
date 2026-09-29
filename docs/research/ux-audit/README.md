# easyquran UX audit — parity, consistency, accessibility, readability

**Dates:** 2026-09-29 → 2026-09-30 · **Build:** `master` @ `49e33b04` (app code unchanged during the audit) · **Scope:** every reader-facing screen and flow, English + Arabic, 4 palettes × light/dark, phone → 2560 px, signed-out and signed-in, online/offline/API-down, dev and production builds, Chromium + WebKit · **Method and coverage:** [00 · How this audit was done](00-how-this-audit-was-done.md)

easyquran is built for non-technical people who want to read the Qur'an simply. This audit asks one question of every screen: *would a first-time, non-technical reader — on a phone, maybe older, maybe Arabic-first, maybe unable to read Arabic — understand it, trust it, and get where they want to go?* It was run in two passes: a first pass over the core screens (docs 01–16) and a second, release-readiness pass over everything else (docs 17–26). [27 · Release polish checklist](27-release-polish-checklist.md) turns every finding into a tickable list.

![Reader overview with the main problems numbered](screenshots/reader/overview-desktop.webp)

## The short version

**What's genuinely good.** The Arabic text is set beautifully and is the highest-contrast thing on the page. Offline is excellent: after one online visit, surahs, juz, mushaf pages, search and even prefetched translations you never opened work with the network off, and once the service worker is installed every page opens in about 0.2 s. Bookmark sync merges anonymous bookmarks into the account and queues offline changes. Search handles Arabic words, `2:255` and `juz 5`. Focus rings, the menu panel's focus trap and route announcements are solid foundations. WebKit and Chromium render Arabic identically. 378 translations in 105 languages are available.

**What holds it back.** Around that strong core, the app speaks developer, repeats itself, and drifts from its own design system:

1. **Trust.** Placeholder text is shown as "TAFSIR"; Arabic tafsir can be picked as the *main* text and looks like Qur'an verses; the Privacy and Terms pages say "placeholder text"; the landing page says nothing tracks reading while analytics is on by default; a floating button sits on Qur'an words on phones; the home-screen icon is broken v1 artwork.
2. **Language.** Arabic readers get English on the home page, reader header, surah list, settings, loading pill and accessible names — and several pages (and the installed app) flip to English.
3. **Translations.** Choosing a translation *removes* the Arabic and the Bismillah; the translator is never named; searching an English word returns zero results; the picker lists duplicates; transliteration shows raw HTML tags.
4. **Readability and access.** Dark-mode blue text 2.6–3.1:1; Tajweed colours nearly vanish in dark mode; controls 26–40 px; lots of 11–13 px text; the browser's text-size setting is ignored; Windows High Contrast hides every selected state; iPhones zoom on every text field; keyboard users face 4 Tab stops per verse.
5. **Consistency.** 7 search-box styles, 7 button styles, 4 selected styles, 4 backdrops, 4 spinners, 4 icon families, 5 page widths, 5 tab-title patterns, 3 spellings of surah names, 4 places to switch theme, 2 translation pickers.

**By the numbers:** **204 findings — 10 P0 · 61 P1 · 97 P2 · 36 P3** (a few are cross-references). Evidence: 155 annotated screenshots, 116 axe-core scans (36 in the first pass + 80 across palettes), accessibility-tree dumps of 10 routes, 10 throttled load runs on a production build, 60 WebKit captures.

## Fix these first (P0)

| # | Finding | Why it's first |
| --- | --- | --- |
| 1 | [RDR-03 · Placeholder text shown as "TAFSIR"](03-reader.md#rdr-03--the-tafsir-panel-shows-placeholder-text-to-real-readers) | Religious-content trust |
| 2 | [TRX-01 · Arabic commentary as the main translation looks like Qur'an text](22-translations-deep-dive.md#trx-01--arabic-commentary-chosen-as-the-main-translation-looks-like-quran-text) | Commentary presented as scripture |
| 3 | [SCR-01 · Legal pages say "placeholder text" and promise things the app doesn't do](19-remaining-screens-and-flows.md#scr-01--legal-pages-say-placeholder-text-and-promise-things-the-app-doesnt-do) | Legal and trust exposure |
| 4 | [RDR-01 · Floating button covers Qur'an text on phones](03-reader.md#rdr-01--the-floating-appearance-button-covers-the-quran-text-on-phones) | Hides the text itself (move the offline-pack bar out of it first) |
| 5 | [TR-01 · Picking a translation removes the Arabic and Bismillah](04-translations.md#tr-01--picking-a-translation-removes-the-arabic-text-and-the-bismillah) | Breaks reader expectations |
| 6 | [SRCH-01 · English word search returns 0 results](05-search.md#srch-01--searching-an-english-word-returns-nothing-with-no-hint-why) | Most common query fails silently |
| 7 | [NAV-02 · Two URL schemes lose the language / 404](01-navigation-and-wayfinding.md#nav-02--two-url-schemes-some-links-lose-the-language-some-return-a-bare-not-found) | Arabic users lose Arabic |
| 8 | [RTL-04 · Settings/Search/Bookmarks/Yours English after refresh](10-arabic-and-rtl.md#rtl-04--settings-search-bookmarks-yours-english-body-and-english-after-refresh) | Same root cause as #7 |
| 9 | [STATE-01 · Raw text "Not found" for bad reader URLs](12-states-errors-offline.md#state-01--wrong-reader-urls-return-a-plain-text-not-found) | Looks broken; trailing slashes from chat apps hit it |
| 10 | [PWA-01 · Broken, off-brand home-screen icon and favicons](25-pwa-and-page-metadata.md#pwa-01--the-home-screen-icon-and-favicons-are-broken-off-brand-artwork) | First thing people see after installing |

Treat [THEME-01 · Custom colours can make the Qur'an text invisible](17-themes-palettes-and-scripts.md#theme-01--custom-colours-can-make-the-quran-text-invisible) as P0 for as long as custom colours stay visible to readers (hiding them — [SET-03](08-settings-and-appearance.md#set-03--designer-and-developer-tools-are-exposed-to-readers) — fixes both).

**Top P1s for the same release:** [SCR-05 analytics vs "nothing tracking"](19-remaining-screens-and-flows.md#scr-05--nothing-tracking-what-you-read-while-analytics-is-on-by-default) · [ACCT-01 sign-out removes bookmarks](20-signed-in-experience.md#acct-01--signing-out-silently-removes-your-account-bookmarks-from-the-device) · [SCRIPT-03 dotted circle in IndoPak](17-themes-palettes-and-scripts.md#script-03--indopak-text-with-kfgqpc-hafs-shows-a-dotted-circle-in-place-of-a-letter) · [A11Y-01 dark contrast](09-accessibility.md#a11y-01--dark-mode-uses-the-fill-blue-as-a-text-colour) · [A11Y-03 targets](09-accessibility.md#a11y-03--tap-targets-are-well-below-the-promised-44-px) · [DISP-01 text size ignored](18-display-conditions.md#disp-01--the-app-ignores-the-browsers-text-size-setting) · [BRW-01 iPhone input zoom](26-cross-browser.md#brw-01--every-text-field-is-under-16-px--iphones-zoom-in-on-tap) · [KEY-03 Tab stops per verse](21-keyboard-focus-and-screen-reader.md#key-03--every-verse-adds-four-tab-stops-with-identical-names) · [LOAD-01 controls appear late](24-loading-and-perceived-performance.md#load-01--the-reader-shows-text-for-25-s-before-its-controls-exist-then-the-page-jumps) · [HOME-04 translation on first run](02-home.md#home-04--new-readers-are-never-asked-do-you-read-arabic). Full plan: [16 · Fix roadmap](16-fix-roadmap.md) · tick-list: [27 · Release polish checklist](27-release-polish-checklist.md).

## Documents

| Doc | Covers | P0 | P1 | P2 | P3 |
| --- | --- | :-: | :-: | :-: | :-: |
| [00 · How this audit was done](00-how-this-audit-was-done.md) | Scope, environment, methods, severity scale, **coverage and gaps** |  |  |  |  |
| [01 · Navigation and wayfinding](01-navigation-and-wayfinding.md) | Header, current page, URL/locale schemes, duplicate doors, logo, small-screen overflow | 1 | 3 | 3 |  |
| [02 · App home](02-home.md) | First vs returning visit, layout, hero chips, first-run translation choice |  | 4 | 3 |  |
| [03 · Reader](03-reader.md) | Floating button, verse tools, placeholder tafsir, header noise, controls, reading mode, range readers, sidebar | 2 | 4 | 5 | 1 |
| [04 · Translations](04-translations.md) | Arabic disappears, credits, picker, phone picker, script fonts, API-down fallback | 1 | 3 | 2 |  |
| [05 · Search](05-search.md) | English queries, ⌘K dead end, empty state, results, 7 search boxes | 1 | 1 | 3 |  |
| [06 · Browse lists](06-browse-lists.md) | Surah spellings, rainbow numbers, juz notation, 604 pages, phone truncation |  | 2 | 3 | 1 |
| [07 · Bookmarks, notes, Yours](07-bookmarks-notes-yours.md) | Two overlapping pages, missing notes, no undo |  | 1 | 3 | 1 |
| [08 · Settings and appearance](08-settings-and-appearance.md) | Tab order, jargon, developer tools, switches, second Settings panel |  | 5 | 3 |  |
| [09 · Accessibility and readability](09-accessibility.md) | axe results, dark contrast, targets, text size, landmarks, focus, reflow |  | 4 | 4 |  |
| [10 · Arabic UI and RTL](10-arabic-and-rtl.md) | Untranslated screens, bidi breaks, locale loss, footer, digits | 1 | 3 | 2 | 1 |
| [11 · Visual consistency](11-visual-consistency.md) | Icons, search boxes, selected states, buttons, widths, fonts, colour meaning, tokens |  |  | 9 | 2 |
| [12 · States, errors, offline](12-states-errors-offline.md) | 404s, error page, API-down hangs, offline indicator, empty states | 1 | 3 | 2 |  |
| [13 · Sign in and account](13-sign-in-and-account.md) | Exit, parity, social buttons, links, value message |  | 1 | 3 | 1 |
| [14 · Marketing ↔ app parity](14-marketing-site-parity.md) | Two headers, outdated copy, phone landing, footer typo |  | 1 | 2 | 1 |
| [15 · Plain language](15-plain-language-copy.md) | Copy rewrites, one-word-per-concept glossary, tone checklist |  |  |  |  |
| [16 · Fix roadmap](16-fix-roadmap.md) | 4 waves (first + second pass), effort, shared components, guard tests, user validation |  |  |  |  |
| [17 · Themes, palettes, Arabic scripts & fonts](17-themes-palettes-and-scripts.md) | 4 palettes × 2 modes (80 axe scans), custom colours, Tajweed contrast, KFGQPC markers, IndoPak glyphs, font swaps |  | 6 | 6 | 1 |
| [18 · Display conditions](18-display-conditions.md) | Browser text size, Windows High Contrast, increase contrast, reduced motion, 280–2560 px, landscape |  | 2 | 1 | 2 |
| [19 · Remaining screens and flows](19-remaining-screens-and-flows.md) | Legal pages, landing sections, juz/page edges, reading-mode dialog, offline pack, clear data, search picker, deep links | 1 | 3 | 11 | 2 |
| [20 · Signed-in experience](20-signed-in-experience.md) | Register/sign-in errors, account page, sessions, sync states, folders, sign-out, OAuth failure |  | 3 | 5 | 1 |
| [21 · Keyboard, focus, screen reader](21-keyboard-focus-and-screen-reader.md) | Tab order, overlays/focus traps, announcer, shortcuts, accessible names, headings, titles |  | 2 | 8 | 5 |
| [22 · Translations deep dive](22-translations-deep-dive.md) | Tafsir as main text, transliteration tags, duplicates, picker, RTL translations, empty verses, persistence | 1 | 2 | 8 | 5 |
| [23 · Interaction details](23-interaction-details.md) | Deep links, highlight, Back, reading-mode actions, copy text, pressed states, scrims, loading, ending |  | 1 | 3 | 7 |
| [24 · Loading and perceived performance](24-loading-and-perceived-performance.md) | Slow-3G filmstrips, layout shift, font swap, download pill, shared search links, offline-after-one-visit |  | 3 | 2 | 2 |
| [25 · PWA and page metadata](25-pwa-and-page-metadata.md) | Icons, manifest, installed-app language, titles, OG images, theme-color, update toast, safe areas | 1 | 3 | 4 | 2 |
| [26 · Cross-browser (WebKit vs Chromium)](26-cross-browser.md) | iPhone input zoom, justification, dvh/tap highlight, Firefox risk list |  | 1 | 2 | 1 |
| [27 · Release polish checklist](27-release-polish-checklist.md) | **Every finding as a tickable list, ordered by release priority, with 'done when' checks** |  |  |  |  |
| [tools/](tools/README.md) | The capture/axe harness to re-run everything | | | | |
| [_merge/](_merge/) | Raw notes from the four second-pass audits (coverage detail, extensions) | | | | |

## Area health at a glance

| Area | Health | One-line verdict |
| --- | --- | --- |
| Qur'an text rendering (default font) | 🟢 Strong | Clear, high-contrast, respectful typography; WebKit = Chromium |
| Offline and speed after first visit | 🟢 Strong | ~0.2 s page opens; never-opened pages work offline |
| Bookmark sync | 🟢 Solid core | Merge, folders and offline queue work; sign-out handling doesn't |
| Reader chrome | 🟠 Needs work | Noisy header, tiny unlabeled tools, floating button over text, 4 Tab stops/verse |
| Alternative scripts and fonts | 🔴 Weak | Tajweed dark contrast, broken KFGQPC markers, IndoPak dotted circle |
| Translations | 🔴 Weak | Replace the Arabic; tafsir as scripture; duplicates; credits missing |
| Search | 🟠 Needs work | Great for Arabic/refs, fails English words; two pickers |
| Browse lists | 🟠 Needs work | Double spellings, code notation |
| Your stuff (bookmarks/notes/account) | 🟠 Needs work | Two pages, notes not listed, thin account page |
| Settings | 🔴 Weak | Developer vocabulary and tools up front; destructive clear without confirm |
| Arabic UI | 🔴 Weak | Many English screens and names; locale lost on refresh and in the installed app |
| Accessibility | 🟠 Needs work | Dark contrast, targets, small text, text-size and high-contrast support |
| Themes and palettes | 🟠 Needs work | Ink is best; palette only reaches buttons; custom colours unsafe |
| Visual consistency | 🟠 Needs work | Many one-off styles despite a good design system |
| Errors, legal, trust copy | 🔴 Weak | Raw 404s, placeholder legal text, analytics claim |
| Loading on slow phones | 🟠 Needs work | Controls ~25 s late, layout jumps, download pill blocks header |
| PWA and metadata | 🔴 Weak | Broken icon, English-only install, inconsistent titles, old colours |

## Screenshots

All evidence lives in [`screenshots/`](screenshots/) (155 WebP files, ≈ 5.8 MB), one folder per area. **Red boxes and numbered labels mark the problem**; comparison images put two or more screens side by side. Every screenshot is embedded in the sub-document that discusses it.

| Folders | Pass | Used in |
| --- | --- | --- |
| `navigation/` `home/` `reader/` `translations/` `search/` `lists/` `bookmarks/` `settings/` `accessibility/` `rtl/` `visual/` `states/` `auth/` `marketing/` | First | 01–14 |
| `themes/` `scripts/` `display/` | Second | 17–18 |
| `screens/` `flows/` `signed-in/` | Second | 19–20 |
| `keyboard/` `translations-deep/` `interactions/` | Second | 21–23 |
| `loading/` `pwa/` `browsers/` | Second | 24–26 |

## Full issue register

<!-- register:start -->
| ID | Finding | Sev | Doc |
| --- | --- | --- | --- |
| [NAV-01](01-navigation-and-wayfinding.md#nav-01--the-header-never-shows-which-section-you-are-in) | The header never shows which section you are in | P1 | [01](01-navigation-and-wayfinding.md) |
| [NAV-02](01-navigation-and-wayfinding.md#nav-02--two-url-schemes-some-links-lose-the-language-some-return-a-bare-not-found) | Two URL schemes: some links lose the language, some return a bare "Not found" | P0 | [01](01-navigation-and-wayfinding.md) |
| [NAV-03](01-navigation-and-wayfinding.md#nav-03--same-action-many-doors-theme--4-search--3-settings--2) | Same action, many doors (theme × 4, search × 3, settings × 2) | P2 | [01](01-navigation-and-wayfinding.md) |
| [NAV-04](01-navigation-and-wayfinding.md#nav-04--the-logo-leaves-the-app) | The logo leaves the app | P2 | [01](01-navigation-and-wayfinding.md) |
| [NAV-05](01-navigation-and-wayfinding.md#nav-05--header-overflows-on-small-phones) | Header overflows on small phones | P1 | [01](01-navigation-and-wayfinding.md) |
| [NAV-06](01-navigation-and-wayfinding.md#nav-06--the-reader-sub-bar-hides-two-key-tools-behind-unexplained-icons) | The reader sub-bar hides two key tools behind unexplained icons | P1 | [01](01-navigation-and-wayfinding.md) |
| [NAV-07](01-navigation-and-wayfinding.md#nav-07--yours-bookmarks-and-the-home-yours-card-overlap) | "Yours", "Bookmarks", and the home "Yours" card overlap | P2 | [01](01-navigation-and-wayfinding.md) |
| [HOME-01](02-home.md#home-01--once-you-have-read-anything-the-browse-shortcuts-disappear) | Once you have read anything, the browse shortcuts disappear | P1 | [02](02-home.md) |
| [HOME-02](02-home.md#home-02--layout-is-lopsided-on-desktop-and-the-juz-card-is-shorter-than-its-neighbours) | Layout is lopsided on desktop and the Juz card is shorter than its neighbours | P2 | [02](02-home.md) |
| [HOME-03](02-home.md#home-03--hero-chips-only-the-first-looks-like-a-button) | Hero chips: only the first looks like a button | P2 | [02](02-home.md) |
| [HOME-04](02-home.md#home-04--new-readers-are-never-asked-do-you-read-arabic) | New readers are never asked "Do you read Arabic?" | P1 | [02](02-home.md) |
| [HOME-05](02-home.md#home-05--home-copy-is-hard-coded-english) | Home copy is hard-coded English | P1 | [02](02-home.md) |
| [HOME-06](02-home.md#home-06--no-page-title-h1) | No page title (`<h1>`) | P2 | [02](02-home.md) |
| [HOME-07](02-home.md#home-07--the-floating-appearance-button-covers-content-on-phones) | The floating appearance button covers content on phones | P1 | [02](02-home.md) |
| [RDR-01](03-reader.md#rdr-01--the-floating-appearance-button-covers-the-quran-text-on-phones) | The floating appearance button covers the Qur'an text on phones | P0 | [03](03-reader.md) |
| [RDR-02](03-reader.md#rdr-02--verse-actions-are-tiny-unlabeled-and-ambiguous) | Verse actions are tiny, unlabeled and ambiguous | P1 | [03](03-reader.md) |
| [RDR-03](03-reader.md#rdr-03--the-tafsir-panel-shows-placeholder-text-to-real-readers) | The "tafsir" panel shows placeholder text to real readers | P0 | [03](03-reader.md) |
| [RDR-04](03-reader.md#rdr-04--the-surah-number-is-shown-four-times-the-header-colour-changes-per-surah) | The surah number is shown four times; the header colour changes per surah | P2 | [03](03-reader.md) |
| [RDR-05](03-reader.md#rdr-05--text-size-and-mode-controls-are-small-and-unclear) | Text-size and mode controls are small and unclear | P1 | [03](03-reader.md) |
| [RDR-06](03-reader.md#rdr-06--reading-mode-on-phones-spreads-words-far-apart) | Reading mode on phones spreads words far apart | P2 | [03](03-reader.md) |
| [RDR-07](03-reader.md#rdr-07--page-and-juz-readers-lack-the-surah-readers-header-and-controls) | Page and Juz readers lack the surah reader's header and controls | P2 | [03](03-reader.md) |
| [RDR-08](03-reader.md#rdr-08--bookmarked-state-is-a-thin-colour-change-only) | Bookmarked state is a thin colour change only | P2 | [03](03-reader.md) |
| [RDR-09](03-reader.md#rdr-09--the-sidebar-spells-some-arabic-surah-names-differently-from-the-rest-of-the-app) | The sidebar spells some Arabic surah names differently from the rest of the app | P1 | [03](03-reader.md) |
| [RDR-10](03-reader.md#rdr-10--dark-mode-ayah-markers-and-the-wordmark-are-too-faint) | Dark mode: ayah markers and the wordmark are too faint | P1 | [03](03-reader.md) |
| [RDR-11](03-reader.md#rdr-11--verse-numbers-are-code-style-and-tiny) | Verse numbers are code-style and tiny | P2 | [03](03-reader.md) |
| [RDR-12](03-reader.md#rdr-12--continuous-scroll-changes-page-n-of-48-silently) | Continuous scroll changes "Page N of 48" silently | P3 | [03](03-reader.md) |
| [TR-01](04-translations.md#tr-01--picking-a-translation-removes-the-arabic-text-and-the-bismillah) | Picking a translation removes the Arabic text (and the Bismillah) | P0 | [04](04-translations.md) |
| [TR-02](04-translations.md#tr-02--the-main-translation-is-never-credited) | The main translation is never credited | P1 | [04](04-translations.md) |
| [TR-03](04-translations.md#tr-03--the-translation-picker-opens-on-arabic-tafsir-uses-flags-and-says-primary) | The translation picker opens on Arabic *tafsir*, uses flags, and says "primary" | P1 | [04](04-translations.md) |
| [TR-04](04-translations.md#tr-04--on-phones-the-picker-hides-translators-one-level-deep) | On phones the picker hides translators one level deep | P2 | [04](04-translations.md) |
| [TR-05](04-translations.md#tr-05--extra-language-lines-are-small-and-use-a-generic-font) | Extra-language lines are small and use a generic font | P2 | [04](04-translations.md) |
| [TR-06](04-translations.md#tr-06--when-the-api-is-unreachable-translated-pages-fail-without-falling-back-to-arabic) | When the API is unreachable, translated pages fail without falling back to Arabic | P1 | [04](04-translations.md) |
| [SRCH-01](05-search.md#srch-01--searching-an-english-word-returns-nothing-with-no-hint-why) | Searching an English word returns nothing, with no hint why | P0 | [05](05-search.md) |
| [SRCH-02](05-search.md#srch-02--the-k-palette-dead-ends-on-words) | The ⌘K palette dead-ends on words | P1 | [05](05-search.md) |
| [SRCH-03](05-search.md#srch-03--the-empty-state-points-at-something-that-isnt-there) | The empty state points at something that isn't there | P2 | [05](05-search.md) |
| [SRCH-04](05-search.md#srch-04--results-show-arabic-only-floating-mid-row) | Results show Arabic only, floating mid-row | P2 | [05](05-search.md) |
| [SRCH-05](05-search.md#srch-05--seven-search-boxes-seven-looks-seven-wordings) | Seven search boxes, seven looks, seven wordings | P2 | [05](05-search.md) |
| [LIST-01](06-browse-lists.md#list-01--every-surah-has-two-or-three-english-spellings) | Every surah has two (or three) English spellings | P1 | [06](06-browse-lists.md) |
| [LIST-02](06-browse-lists.md#list-02--rainbow-numbers-carry-no-meaning-and-the-green-fails-contrast) | Rainbow numbers carry no meaning (and the green fails contrast) | P2 | [06](06-browse-lists.md) |
| [LIST-03](06-browse-lists.md#list-03--juz-list-is-written-in-reference-code) | Juz list is written in reference code | P1 | [06](06-browse-lists.md) |
| [LIST-04](06-browse-lists.md#list-04--604-page-cards-and-no-go-to-page-box) | 604 page cards and no "go to page" box | P2 | [06](06-browse-lists.md) |
| [LIST-05](06-browse-lists.md#list-05--phone-rows-cut-off-the-verse-count) | Phone rows cut off the verse count | P2 | [06](06-browse-lists.md) |
| [LIST-06](06-browse-lists.md#list-06--list-pages-have-no-title-and-no-filter-of-their-own) | List pages have no title and no filter of their own | P3 | [06](06-browse-lists.md) |
| [BM-01](07-bookmarks-notes-yours.md#bm-01--two-pages-two-layouts-two-formats-for-the-same-bookmark) | Two pages, two layouts, two formats for the same bookmark | P2 | [07](07-bookmarks-notes-yours.md) |
| [BM-02](07-bookmarks-notes-yours.md#bm-02--bookmark-rows-have-no-context) | Bookmark rows have no context | P2 | [07](07-bookmarks-notes-yours.md) |
| [BM-03](07-bookmarks-notes-yours.md#bm-03--notes-are-saved-but-never-shown-anywhere) | Notes are saved but never shown anywhere | P1 | [07](07-bookmarks-notes-yours.md) |
| [BM-04](07-bookmarks-notes-yours.md#bm-04--remove-deletes-instantly-no-undo) | "Remove" deletes instantly, no undo | P2 | [07](07-bookmarks-notes-yours.md) |
| [BM-05](07-bookmarks-notes-yours.md#bm-05--stored-in-this-browser--sign-in-to-sync-is-vague) | "Stored in this browser" / "Sign in to sync" is vague | P3 | [07](07-bookmarks-notes-yours.md) |
| [SET-01](08-settings-and-appearance.md#set-01--settings-opens-on-storage-the-most-technical-tab) | Settings opens on "Storage", the most technical tab | P1 | [08](08-settings-and-appearance.md) |
| [SET-02](08-settings-and-appearance.md#set-02--storage-copy-is-browserdeveloper-jargon) | Storage copy is browser/developer jargon | P1 | [08](08-settings-and-appearance.md) |
| [SET-03](08-settings-and-appearance.md#set-03--designer-and-developer-tools-are-exposed-to-readers) | Designer and developer tools are exposed to readers | P1 | [08](08-settings-and-appearance.md) |
| [SET-04](08-settings-and-appearance.md#set-04--toggles-are-on-text-pills-analytics-is-on-by-default) | Toggles are "on" text pills; analytics is on by default | P1 | [08](08-settings-and-appearance.md) |
| [SET-05](08-settings-and-appearance.md#set-05--a-focus-rectangle-is-drawn-around-the-whole-panel-after-clicking-a-tab) | A focus rectangle is drawn around the whole panel after clicking a tab | P2 | [08](08-settings-and-appearance.md) |
| [SET-06](08-settings-and-appearance.md#set-06--phone-tabs-run-off-screen-with-no-hint) | Phone: tabs run off-screen with no hint | P2 | [08](08-settings-and-appearance.md) |
| [SET-07](08-settings-and-appearance.md#set-07--reading-settings-speak-in-pixels-and-font-file-names) | Reading settings speak in pixels and font file names | P2 | [08](08-settings-and-appearance.md) |
| [SET-08](08-settings-and-appearance.md#set-08--a-second-settings-floats-over-every-page) | A second "Settings" floats over every page | P1 | [08](08-settings-and-appearance.md) |
| [A11Y-01](09-accessibility.md#a11y-01--dark-mode-uses-the-fill-blue-as-a-text-colour) | Dark mode uses the fill blue as a text colour | P1 | [09](09-accessibility.md) |
| [A11Y-02](09-accessibility.md#a11y-02--light-mode-green-on-green-chips-and-the-juz-card-caption) | Light mode: green-on-green chips and the Juz card caption | P2 | [09](09-accessibility.md) |
| [A11Y-03](09-accessibility.md#a11y-03--tap-targets-are-well-below-the-promised-44-px) | Tap targets are well below the promised 44 px | P1 | [09](09-accessibility.md) |
| [A11Y-04](09-accessibility.md#a11y-04--too-much-text-is-1113-px) | Too much text is 11–13 px | P1 | [09](09-accessibility.md) |
| [A11Y-05](09-accessibility.md#a11y-05--duplicate-and-nested-landmarks-missing-page-titles) | Duplicate and nested landmarks; missing page titles | P2 | [09](09-accessibility.md) |
| [A11Y-06](09-accessibility.md#a11y-06--links-identified-by-colour-only-focus-ring-style-inconsistent) | Links identified by colour only; focus ring style inconsistent | P2 | [09](09-accessibility.md) |
| [A11Y-07](09-accessibility.md#a11y-07--state-shown-by-colour-or-hover-alone) | State shown by colour or hover alone | P2 | [09](09-accessibility.md) |
| [A11Y-08](09-accessibility.md#a11y-08--reflow-and-obscured-content) | Reflow and obscured content | P1 | [09](09-accessibility.md) |
| [RTL-01](10-arabic-and-rtl.md#rtl-01--app-home-hero-and-continue-card-are-english) | App home hero and continue card are English | P1 | [10](10-arabic-and-rtl.md) |
| [RTL-02](10-arabic-and-rtl.md#rtl-02--reader-header-english-name-first-broken-number-order-english-metadata) | Reader header: English name first, broken number order, English metadata | P1 | [10](10-arabic-and-rtl.md) |
| [RTL-03](10-arabic-and-rtl.md#rtl-03--surah-list-metadata-is-english-in-arabic-ui) | Surah list metadata is English in Arabic UI | P1 | [10](10-arabic-and-rtl.md) |
| [RTL-04](10-arabic-and-rtl.md#rtl-04--settings-search-bookmarks-yours-english-body-and-english-after-refresh) | Settings, Search, Bookmarks, Yours: English body, and English after refresh | P0 | [10](10-arabic-and-rtl.md) |
| [RTL-05](10-arabic-and-rtl.md#rtl-05--arabic-footer-drops-company-and-legal) | Arabic footer drops "Company" and "Legal" | P2 | [10](10-arabic-and-rtl.md) |
| [RTL-06](10-arabic-and-rtl.md#rtl-06--translation-picker-lists-language-names-in-english) | Translation picker lists language names in English | P2 | [10](10-arabic-and-rtl.md) |
| [RTL-07](10-arabic-and-rtl.md#rtl-07--small-bidi-and-wording-issues) | Small bidi and wording issues | P3 | [10](10-arabic-and-rtl.md) |
| [VIS-01](11-visual-consistency.md#vis-01--four-icon-families-and-two-icon-weights) | Four icon families and two icon weights | P2 | [11](11-visual-consistency.md) |
| [VIS-02](11-visual-consistency.md#vis-02--icons-mean-different-things-in-different-places) | Icons mean different things in different places | P2 | [11](11-visual-consistency.md) |
| [VIS-03](11-visual-consistency.md#vis-03--seven-different-search-boxes) | Seven different search boxes | P2 | [11](11-visual-consistency.md) |
| [VIS-04](11-visual-consistency.md#vis-04--four-different-selected-looks) | Four different "selected" looks | P2 | [11](11-visual-consistency.md) |
| [VIS-05](11-visual-consistency.md#vis-05--buttons-come-in-seven-shapes) | Buttons come in seven shapes | P2 | [11](11-visual-consistency.md) |
| [VIS-06](11-visual-consistency.md#vis-06--every-page-uses-a-different-width-and-header-pattern) | Every page uses a different width and header pattern | P2 | [11](11-visual-consistency.md) |
| [VIS-07](11-visual-consistency.md#vis-07--monospace-and-letter-spacing-where-people-read-words) | Monospace and letter-spacing where people read words | P2 | [11](11-visual-consistency.md) |
| [VIS-08](11-visual-consistency.md#vis-08--colour-used-as-decoration-not-meaning) | Colour used as decoration, not meaning | P2 | [11](11-visual-consistency.md) |
| [VIS-09](11-visual-consistency.md#vis-09--browser-theme-colour-is-from-the-old-design) | Browser theme colour is from the old design | P3 | [11](11-visual-consistency.md) |
| [VIS-10](11-visual-consistency.md#vis-10--legacy-tokens-and-ad-hoc-sizes-on-un-migrated-pages) | Legacy tokens and ad-hoc sizes on un-migrated pages | P3 | [11](11-visual-consistency.md) |
| [VIS-11](11-visual-consistency.md#vis-11--terminology-drifts-between-screens) | Terminology drifts between screens | P2 | [11](11-visual-consistency.md) |
| [STATE-01](12-states-errors-offline.md#state-01--wrong-reader-urls-return-a-plain-text-not-found) | Wrong reader URLs return a plain-text "Not found" | P0 | [12](12-states-errors-offline.md) |
| [STATE-02](12-states-errors-offline.md#state-02--the-error-page-is-a-dead-end) | The error page is a dead end | P1 | [12](12-states-errors-offline.md) |
| [STATE-03](12-states-errors-offline.md#state-03--account-page-spins-forever-when-the-api-is-unreachable) | Account page spins forever when the API is unreachable | P1 | [12](12-states-errors-offline.md) |
| [STATE-04](12-states-errors-offline.md#state-04--translation-failures-hide-verses-instead-of-falling-back) | Translation failures hide verses instead of falling back | P1 | [12](12-states-errors-offline.md) |
| [STATE-05](12-states-errors-offline.md#state-05--offline-indicator-a-dot-on-phones-and-nothing-about-what-still-works) | Offline indicator: a dot on phones, and nothing about what still works | P2 | [12](12-states-errors-offline.md) |
| [STATE-06](12-states-errors-offline.md#state-06--empty-states-are-inconsistent-and-sometimes-wrong) | Empty states are inconsistent and sometimes wrong | P2 | [12](12-states-errors-offline.md) |
| [AUTH-01](13-sign-in-and-account.md#auth-01--no-way-back-from-sign-in--create-account) | No way back from Sign in / Create account | P1 | [13](13-sign-in-and-account.md) |
| [AUTH-02](13-sign-in-and-account.md#auth-02--sign-in-and-create-account-dont-match) | Sign in and Create account don't match | P2 | [13](13-sign-in-and-account.md) |
| [AUTH-03](13-sign-in-and-account.md#auth-03--social-sign-in-rows-dont-look-like-buttons-github-for-this-audience) | Social sign-in rows don't look like buttons; GitHub for this audience | P2 | [13](13-sign-in-and-account.md) |
| [AUTH-04](13-sign-in-and-account.md#auth-04--small-colour-only-links-and-missing-page-titles) | Small, colour-only links and missing page titles | P2 | [13](13-sign-in-and-account.md) |
| [AUTH-05](13-sign-in-and-account.md#auth-05--why-sign-in-the-value-isnt-stated-where-it-matters) | Why sign in? The value isn't stated where it matters | P3 | [13](13-sign-in-and-account.md) |
| [MKT-01](14-marketing-site-parity.md#mkt-01--two-different-headers) | Two different headers | P2 | [14](14-marketing-site-parity.md) |
| [MKT-02](14-marketing-site-parity.md#mkt-02--website-copy-is-out-of-date-with-the-app) | Website copy is out of date with the app | P1 | [14](14-marketing-site-parity.md) |
| [MKT-03](14-marketing-site-parity.md#mkt-03--landing-on-phones-headline-fills-the-screen-header-search-is-s) | Landing on phones: headline fills the screen; header search is "S…" | P2 | [14](14-marketing-site-parity.md) |
| [MKT-04](14-marketing-site-parity.md#mkt-04--footer-typo-and-brand-spelling) | Footer typo and brand spelling | P3 | [14](14-marketing-site-parity.md) |
| [THEME-01](17-themes-palettes-and-scripts.md#theme-01--custom-colours-can-make-the-quran-text-invisible) | Custom colours can make the Qur'an text invisible | P1 | [17](17-themes-palettes-and-scripts.md) |
| [THEME-02](17-themes-palettes-and-scripts.md#theme-02--custom-colours-pick-the-wrong-text-colour-for-middle-tones) | Custom colours pick the wrong text colour for middle tones | P1 | [17](17-themes-palettes-and-scripts.md) |
| [THEME-03](17-themes-palettes-and-scripts.md#theme-03--the-focus-ring-is-too-faint-on-dark-surfaces-cobalt-magenta-emerald) | The focus ring is too faint on dark surfaces (Cobalt, Magenta, Emerald) | P1 | [17](17-themes-palettes-and-scripts.md) |
| [THEME-04](17-themes-palettes-and-scripts.md#theme-04--the-chosen-palette-only-reaches-the-buttons) | The chosen palette only reaches the buttons | P2 | [17](17-themes-palettes-and-scripts.md) |
| [THEME-05](17-themes-palettes-and-scripts.md#theme-05--ink-dark-mode-turns-the-home-hero-into-a-bright-white-slab) | Ink dark mode turns the home hero into a bright white slab | P2 | [17](17-themes-palettes-and-scripts.md) |
| [THEME-06](17-themes-palettes-and-scripts.md#theme-06--see-through-text-on-colour-fills-fails-in-magenta-and-emerald) | See-through text on colour fills fails in Magenta and Emerald | P2 | [17](17-themes-palettes-and-scripts.md) |
| [THEME-07](17-themes-palettes-and-scripts.md#theme-07--the-selected-palette-cards-description-is-37391-in-dark-mode) | The selected palette card's description is 3.7–3.9:1 in dark mode | P2 | [17](17-themes-palettes-and-scripts.md) |
| [SCRIPT-01](17-themes-palettes-and-scripts.md#script-01--tajweed-colours-dont-change-for-dark-mode-several-letters-nearly-vanish) | Tajweed colours don't change for dark mode; several letters nearly vanish | P1 | [17](17-themes-palettes-and-scripts.md) |
| [SCRIPT-02](17-themes-palettes-and-scripts.md#script-02--the-kfgqpc-fonts-draw-the-verse-end-marker-wrongly) | The KFGQPC fonts draw the verse-end marker wrongly | P1 | [17](17-themes-palettes-and-scripts.md) |
| [SCRIPT-03](17-themes-palettes-and-scripts.md#script-03--indopak-text-with-kfgqpc-hafs-shows-a-dotted-circle-in-place-of-a-letter) | IndoPak text with KFGQPC Hafs shows a dotted circle in place of a letter | P1 | [17](17-themes-palettes-and-scripts.md) |
| [SCRIPT-04](17-themes-palettes-and-scripts.md#script-04--the-chosen-script-and-font-appear-late-uthmani-and-amiri-flash-first) | The chosen script and font appear late: Uthmani and Amiri flash first | P2 | [17](17-themes-palettes-and-scripts.md) |
| [SCRIPT-05](17-themes-palettes-and-scripts.md#script-05--search-results-ignore-the-chosen-script-and-font) | Search results ignore the chosen script and font | P2 | [17](17-themes-palettes-and-scripts.md) |
| [SCRIPT-06](17-themes-palettes-and-scripts.md#script-06--the-bismillah-doesnt-scale-with-the-arabic-text-size) | The Bismillah doesn't scale with the Arabic text size | P3 | [17](17-themes-palettes-and-scripts.md) |
| [DISP-01](18-display-conditions.md#disp-01--the-app-ignores-the-browsers-text-size-setting) | The app ignores the browser's text-size setting | P1 | [18](18-display-conditions.md) |
| [DISP-02](18-display-conditions.md#disp-02--windows-high-contrast-nothing-shows-what-is-selected) | Windows High Contrast: nothing shows what is selected | P1 | [18](18-display-conditions.md) |
| [DISP-03](18-display-conditions.md#disp-03--translation-lines-are-100-characters-long-on-wide-screens) | Translation lines are ~100 characters long on wide screens | P2 | [18](18-display-conditions.md) |
| [DISP-04](18-display-conditions.md#disp-04--the-menu-panel-still-slides-when-reduce-motion-is-on) | The menu panel still slides when "reduce motion" is on | P3 | [18](18-display-conditions.md) |
| [DISP-05](18-display-conditions.md#disp-05--no-response-to-increase-contrast) | No response to "increase contrast" | P3 | [18](18-display-conditions.md) |
| [SCR-01](19-remaining-screens-and-flows.md#scr-01--legal-pages-say-placeholder-text-and-promise-things-the-app-doesnt-do) | Legal pages say "placeholder text" and promise things the app doesn't do | P0 | [19](19-remaining-screens-and-flows.md) |
| [SCR-02](19-remaining-screens-and-flows.md#scr-02--legal-pages-are-hard-to-read) | Legal pages are hard to read | P2 | [19](19-remaining-screens-and-flows.md) |
| [SCR-03](19-remaining-screens-and-flows.md#scr-03--auth-side-pages-look-like-a-different-product) | Auth side pages look like a different product | P2 | [19](19-remaining-screens-and-flows.md) |
| [SCR-04](19-remaining-screens-and-flows.md#scr-04--landing-lower-sections-wrong-text-colour-a-self-link-a-very-long-list) | Landing lower sections: wrong text colour, a self-link, a very long list | P2 | [19](19-remaining-screens-and-flows.md) |
| [SCR-05](19-remaining-screens-and-flows.md#scr-05--nothing-tracking-what-you-read-while-analytics-is-on-by-default) | "Nothing tracking what you read" while analytics is on by default | P1 | [19](19-remaining-screens-and-flows.md) |
| [SCR-06](19-remaining-screens-and-flows.md#scr-06--every-reader-type-ends-differently--and-the-end-of-the-quran-is-a-small-link) | Every reader type ends differently — and the end of the Qur'an is a small link | P2 | [19](19-remaining-screens-and-flows.md) |
| [SCR-07](19-remaining-screens-and-flows.md#scr-07--special-quran-moments-arent-explained) | Special Qur'an moments aren't explained | P3 | [19](19-remaining-screens-and-flows.md) |
| [SCR-08](19-remaining-screens-and-flows.md#scr-08--the-reading-mode-dialog-uses-its-own-button-and-list-styles) | The reading-mode dialog uses its own button and list styles | P3 | [19](19-remaining-screens-and-flows.md) |
| [FLOW-01](19-remaining-screens-and-flows.md#flow-01--clear-cached-pages--data--no-confirmation-no-visible-result) | "Clear cached pages & data" — no confirmation, no visible result | P1 | [19](19-remaining-screens-and-flows.md) |
| [FLOW-02](19-remaining-screens-and-flows.md#flow-02--offline-pack-no-progress-no-ready-moment-developer-wording) | Offline pack: no progress, no "ready" moment, developer wording | P2 | [19](19-remaining-screens-and-flows.md) |
| [FLOW-03](19-remaining-screens-and-flows.md#flow-03--search-has-a-second-different-translation-picker--and-it-needs-two-steps) | Search has a second, different translation picker — and it needs two steps | P1 | [19](19-remaining-screens-and-flows.md) |
| [FLOW-04](19-remaining-screens-and-flows.md#flow-04--storage-download-rows-jargon-chips-and-an-unexplained-disabled-button) | Storage download rows: jargon chips and an unexplained disabled button | P2 | [19](19-remaining-screens-and-flows.md) |
| [FLOW-05](19-remaining-screens-and-flows.md#flow-05--shared-verses-carry-no-link-back) | Shared verses carry no link back | P2 | [19](19-remaining-screens-and-flows.md) |
| [FLOW-06](19-remaining-screens-and-flows.md#flow-06--k-go-to-verse-lands-on-the-page-not-the-verse-deep-links-dont-highlight) | ⌘K "go to verse" lands on the page, not the verse; deep links don't highlight | P2 | [19](19-remaining-screens-and-flows.md) |
| [FLOW-07](19-remaining-screens-and-flows.md#flow-07--switching-language-throws-away-your-reading-position) | Switching language throws away your reading position | P2 | [19](19-remaining-screens-and-flows.md) |
| [FLOW-08](19-remaining-screens-and-flows.md#flow-08--many-bookmarks-become-an-unsorted-wall) | Many bookmarks become an unsorted wall | P2 | [19](19-remaining-screens-and-flows.md) |
| [FLOW-09](19-remaining-screens-and-flows.md#flow-09--update-and-notification-toasts-cover-the-header-and-have-tiny-controls) | Update and notification toasts cover the header and have tiny controls | P2 | [19](19-remaining-screens-and-flows.md) |
| [ACCT-01](20-signed-in-experience.md#acct-01--signing-out-silently-removes-your-account-bookmarks-from-the-device) | Signing out silently removes your account bookmarks from the device | P1 | [20](20-signed-in-experience.md) |
| [ACCT-02](20-signed-in-experience.md#acct-02--the-account-page-is-a-bare-developer-screen) | The Account page is a bare developer screen | P1 | [20](20-signed-in-experience.md) |
| [ACCT-03](20-signed-in-experience.md#acct-03--nothing-shows-that-youre-signed-in) | Nothing shows that you're signed in | P2 | [20](20-signed-in-experience.md) |
| [ACCT-04](20-signed-in-experience.md#acct-04--bookmark-folders-squashed-add-button-disguised-dropdowns-repeated-labels) | Bookmark folders: squashed add button, disguised dropdowns, repeated labels | P2 | [20](20-signed-in-experience.md) |
| [ACCT-05](20-signed-in-experience.md#acct-05--sync-status-is-good-but-small) | Sync status is good but small | P3 | [20](20-signed-in-experience.md) |
| [ACCT-06](20-signed-in-experience.md#acct-06--auth-forms-tiny-errors-mixed-icons-hidden-password-rule) | Auth forms: tiny errors, mixed icons, hidden password rule | P2 | [20](20-signed-in-experience.md) |
| [ACCT-07](20-signed-in-experience.md#acct-07--verification-and-reset-codes-dont-match-and-the-copy-talks-to-strangers) | Verification and reset codes don't match, and the copy talks to strangers | P2 | [20](20-signed-in-experience.md) |
| [ACCT-08](20-signed-in-experience.md#acct-08--server-and-configuration-errors-reach-readers-raw-or-mislabelled) | Server and configuration errors reach readers raw or mislabelled | P1 | [20](20-signed-in-experience.md) |
| [ACCT-09](20-signed-in-experience.md#acct-09--signing-in-merges-local-bookmarks--say-so-and-say-what-doesnt-sync) | Signing in merges local bookmarks — say so, and say what doesn't sync | P2 | [20](20-signed-in-experience.md) |
| [KEY-01](21-keyboard-focus-and-screen-reader.md#key-01--screen-readers-are-interrupted-with-page-n-of-48-while-you-scroll) | Screen readers are interrupted with "Page N of 48" while you scroll | P1 | [21](21-keyboard-focus-and-screen-reader.md) |
| [KEY-02](21-keyboard-focus-and-screen-reader.md#key-02--two-skip-to-content-links--and-in-arabic-the-first-one-is-english) | Two "Skip to content" links — and in Arabic the first one is English | P2 | [21](21-keyboard-focus-and-screen-reader.md) |
| [KEY-03](21-keyboard-focus-and-screen-reader.md#key-03--every-verse-adds-four-tab-stops-with-identical-names) | Every verse adds four Tab stops with identical names | P1 | [21](21-keyboard-focus-and-screen-reader.md) |
| [KEY-04](21-keyboard-focus-and-screen-reader.md#key-04--the-focus-ring-vanishes-on-the-blue-surahs-card) | The focus ring vanishes on the blue "Surahs" card | P2 | [21](21-keyboard-focus-and-screen-reader.md) |
| [KEY-05](21-keyboard-focus-and-screen-reader.md#key-05--the-search-palette-drops-focus-when-it-closes) | The search palette drops focus when it closes | P2 | [21](21-keyboard-focus-and-screen-reader.md) |
| [KEY-06](21-keyboard-focus-and-screen-reader.md#key-06--three-dialogs-leave-the-page-behind-them-readable) | Three dialogs leave the page behind them readable | P2 | [21](21-keyboard-focus-and-screen-reader.md) |
| [KEY-07](21-keyboard-focus-and-screen-reader.md#key-07--focus-wanders-behind-the-floating-appearance-panel) | Focus wanders behind the floating appearance panel | P3 | [21](21-keyboard-focus-and-screen-reader.md) |
| [KEY-08](21-keyboard-focus-and-screen-reader.md#key-08--overlays-open-on-an-unhelpful-first-control) | Overlays open on an unhelpful first control | P3 | [21](21-keyboard-focus-and-screen-reader.md) |
| [KEY-09](21-keyboard-focus-and-screen-reader.md#key-09--keyboard-shortcuts-are-hidden-and-some-are-single-keys-that-cant-be-turned-off) | Keyboard shortcuts are hidden, and some are single keys that can't be turned off | P2 | [21](21-keyboard-focus-and-screen-reader.md) |
| [KEY-10](21-keyboard-focus-and-screen-reader.md#key-10--vague-or-duplicated-control-names) | Vague or duplicated control names | P2 | [21](21-keyboard-focus-and-screen-reader.md) |
| [KEY-11](21-keyboard-focus-and-screen-reader.md#key-11--english-accessible-names-in-the-arabic-ui) | English accessible names in the Arabic UI | P2 | [21](21-keyboard-focus-and-screen-reader.md) |
| [KEY-12](21-keyboard-focus-and-screen-reader.md#key-12--heading-outline-gaps) | Heading outline gaps | P2 | [21](21-keyboard-focus-and-screen-reader.md) |
| [KEY-13](21-keyboard-focus-and-screen-reader.md#key-13--browser-tab-titles-follow-five-different-patterns) | Browser tab titles follow five different patterns | P3 | [21](21-keyboard-focus-and-screen-reader.md) |
| [KEY-14](21-keyboard-focus-and-screen-reader.md#key-14--the-verse-note-button-doesnt-say-it-opens-a-panel) | The verse note button doesn't say it opens a panel | P3 | [21](21-keyboard-focus-and-screen-reader.md) |
| [KEY-15](21-keyboard-focus-and-screen-reader.md#key-15--sign-in-errors-well-built-two-gaps) | Sign-in errors: well built, two gaps | P3 | [21](21-keyboard-focus-and-screen-reader.md) |
| [TRX-01](22-translations-deep-dive.md#trx-01--arabic-commentary-chosen-as-the-main-translation-looks-like-quran-text) | Arabic commentary chosen as the main translation looks like Qur'an text | P0 | [22](22-translations-deep-dive.md) |
| [TRX-02](22-translations-deep-dive.md#trx-02--the-transliteration-shows-raw-html-tags) | The transliteration shows raw HTML tags | P1 | [22](22-translations-deep-dive.md) |
| [TRX-03](22-translations-deep-dive.md#trx-03--the-same-translator-is-listed-twice-and-some-languages-twice) | The same translator is listed twice, and some languages twice | P1 | [22](22-translations-deep-dive.md) |
| [TRX-04](22-translations-deep-dive.md#trx-04--picker-search-misses-common-spellings) | Picker search misses common spellings | P2 | [22](22-translations-deep-dive.md) |
| [TRX-05](22-translations-deep-dive.md#trx-05--the-chosen-translation-chips-hide-most-of-your-choices) | The chosen-translation chips hide most of your choices | P2 | [22](22-translations-deep-dive.md) |
| [TRX-06](22-translations-deep-dive.md#trx-06--the--on-the-main-translation-does-nothing-primary-is-a-mystery-button) | The × on the main translation does nothing; "Primary" is a mystery button | P2 | [22](22-translations-deep-dive.md) |
| [TRX-07](22-translations-deep-dive.md#trx-07--at-the-5-translation-cap-disabled-boxes-look-enabled-and-the-message-is-jargon) | At the 5-translation cap, disabled boxes look enabled and the message is jargon | P3 | [22](22-translations-deep-dive.md) |
| [TRX-08](22-translations-deep-dive.md#trx-08--changes-apply-instantly-done-and--are-the-same-thing) | Changes apply instantly; "Done" and ✕ are the same thing | P3 | [22](22-translations-deep-dive.md) |
| [TRX-09](22-translations-deep-dive.md#trx-09--switching-the-main-translation-mid-surah-loses-your-place) | Switching the main translation mid-surah loses your place | P2 | [22](22-translations-deep-dive.md) |
| [TRX-10](22-translations-deep-dive.md#trx-10--the-serif-translation-font-is-forgotten-after-a-reload) | The serif translation font is forgotten after a reload | P2 | [22](22-translations-deep-dive.md) |
| [TRX-11](22-translations-deep-dive.md#trx-11--verses-with-no-translation-text-look-broken) | Verses with no translation text look broken | P2 | [22](22-translations-deep-dive.md) |
| [TRX-12](22-translations-deep-dive.md#trx-12--right-to-left-translations-are-set-like-english) | Right-to-left translations are set like English | P2 | [22](22-translations-deep-dive.md) |
| [TRX-13](22-translations-deep-dive.md#trx-13--stacked-credits-break-right-to-left-lines) | Stacked credits break right-to-left lines | P2 | [22](22-translations-deep-dive.md) |
| [TRX-14](22-translations-deep-dive.md#trx-14--five-stacked-translations-on-a-phone-one-verse-per-screen) | Five stacked translations on a phone: one verse per screen | P3 | [22](22-translations-deep-dive.md) |
| [TRX-15](22-translations-deep-dive.md#trx-15--reading-mode-choice-dialog-clear-with-small-inconsistencies) | Reading-mode choice dialog: clear, with small inconsistencies | P3 | [22](22-translations-deep-dive.md) |
| [TRX-16](22-translations-deep-dive.md#trx-16--the-row-hover-card-shows-technical-metadata) | The row hover card shows technical metadata | P3 | [22](22-translations-deep-dive.md) |
| [INT-01](23-interaction-details.md#int-01--a-shared-link-to-a-long-verse-opens-in-the-wrong-place-on-phones) | A shared link to a long verse opens in the wrong place on phones | P1 | [23](23-interaction-details.md) |
| [INT-02](23-interaction-details.md#int-02--the-highlight-on-a-linked-verse-is-nearly-invisible) | The highlight on a linked verse is nearly invisible | P2 | [23](23-interaction-details.md) |
| [INT-03](23-interaction-details.md#int-03--back-after-next-surah-returns-to-the-top-not-where-you-were) | Back after "Next surah" returns to the top, not where you were | P2 | [23](23-interaction-details.md) |
| [INT-04](23-interaction-details.md#int-04--reading-mode-removes-every-verse-action) | Reading mode removes every verse action | P2 | [23](23-interaction-details.md) |
| [INT-05](23-interaction-details.md#int-05--what-copy-puts-on-the-clipboard) | What "Copy" puts on the clipboard | P3 | [23](23-interaction-details.md) |
| [INT-06](23-interaction-details.md#int-06--pressed-feedback-exists-on-only-a-few-buttons-buttons-show-an-arrow-cursor) | Pressed feedback exists on only a few buttons; buttons show an arrow cursor | P3 | [23](23-interaction-details.md) |
| [INT-07](23-interaction-details.md#int-07--four-different-backdrops-behind-overlays) | Four different backdrops behind overlays | P3 | [23](23-interaction-details.md) |
| [INT-08](23-interaction-details.md#int-08--the-floating-button-stays-on-top-of-open-dialogs) | The floating button stays on top of open dialogs | P3 | [23](23-interaction-details.md) |
| [INT-09](23-interaction-details.md#int-09--new-verses-appear-before-their-tools-no-loading-more-cue) | New verses appear before their tools; no "loading more" cue | P3 | [23](23-interaction-details.md) |
| [INT-10](23-interaction-details.md#int-10--loading-indicators-come-in-four-styles) | Loading indicators come in four styles | P3 | [23](23-interaction-details.md) |
| [INT-11](23-interaction-details.md#int-11--finishing-the-quran-has-no-ending) | Finishing the Qur'an has no ending | P3 | [23](23-interaction-details.md) |
| [LOAD-01](24-loading-and-perceived-performance.md#load-01--the-reader-shows-text-for-25-s-before-its-controls-exist-then-the-page-jumps) | The reader shows text for ~25 s before its controls exist, then the page jumps | P1 | [24](24-loading-and-perceived-performance.md) |
| [LOAD-02](24-loading-and-perceived-performance.md#load-02--preparing-offline-quran-sits-on-top-of-the-header-for-the-whole-download) | "Preparing offline Quran" sits on top of the header for the whole download | P1 | [24](24-loading-and-perceived-performance.md) |
| [LOAD-03](24-loading-and-perceived-performance.md#load-03--a-shared-search-link-ignores-its-query-until-the-app-wakes-up) | A shared search link ignores its query until the app wakes up | P1 | [24](24-loading-and-perceived-performance.md) |
| [LOAD-04](24-loading-and-perceived-performance.md#load-04--the-ui-font-arrives-late-and-moves-the-page) | The UI font arrives late and moves the page | P2 | [24](24-loading-and-perceived-performance.md) |
| [LOAD-05](24-loading-and-perceived-performance.md#load-05--67-seconds-of-white-screen-on-a-slow-first-visit) | 6–7 seconds of white screen on a slow first visit | P2 | [24](24-loading-and-perceived-performance.md) |
| [LOAD-06](24-loading-and-perceived-performance.md#load-06--ayah-markers-change-shape-during-loading) | Ayah markers change shape during loading | P3 | [24](24-loading-and-perceived-performance.md) |
| [LOAD-07](24-loading-and-perceived-performance.md#load-07--without-javascript-the-app-is-always-dark) | Without JavaScript the app is always dark | P3 | [24](24-loading-and-perceived-performance.md) |
| [PWA-01](25-pwa-and-page-metadata.md#pwa-01--the-home-screen-icon-and-favicons-are-broken-off-brand-artwork) | The home-screen icon and favicons are broken, off-brand artwork | P0 | [25](25-pwa-and-page-metadata.md) |
| [PWA-02](25-pwa-and-page-metadata.md#pwa-02--manifest-installed-app-always-opens-in-english-old-colours-brand-spelling) | Manifest: installed app always opens in English, old colours, brand spelling | P1 | [25](25-pwa-and-page-metadata.md) |
| [PWA-03](25-pwa-and-page-metadata.md#pwa-03--tab-titles-follow-five-patterns-several-pages-have-no-title) | Tab titles follow five patterns; several pages have no title | P1 | [25](25-pwa-and-page-metadata.md) |
| [PWA-04](25-pwa-and-page-metadata.md#pwa-04--browserstatus-bar-colour-never-matches-the-chosen-theme) | Browser/status-bar colour never matches the chosen theme | P2 | [25](25-pwa-and-page-metadata.md) |
| [PWA-05](25-pwa-and-page-metadata.md#pwa-05--no-help-to-install-the-app) | No help to install the app | P2 | [25](25-pwa-and-page-metadata.md) |
| [PWA-06](25-pwa-and-page-metadata.md#pwa-06--in-the-installed-app-some-screens-have-no-way-back) | In the installed app, some screens have no way back | P1 | [25](25-pwa-and-page-metadata.md) |
| [PWA-07](25-pwa-and-page-metadata.md#pwa-07--safe-area-padding-is-dead-code-no-viewport-fit) | Safe-area padding is dead code; no `viewport-fit` | P3 | [25](25-pwa-and-page-metadata.md) |
| [PWA-08](25-pwa-and-page-metadata.md#pwa-08--the-update-notice-speaks-in-tabs-and-has-a-tiny-button) | The update notice speaks in "tabs" and has a tiny button | P2 | [25](25-pwa-and-page-metadata.md) |
| [PWA-09](25-pwa-and-page-metadata.md#pwa-09--link-previews-one-old-image-for-every-page) | Link previews: one old image for every page | P2 | [25](25-pwa-and-page-metadata.md) |
| [PWA-10](25-pwa-and-page-metadata.md#pwa-10--personal-pages-are-indexable-searchsettings-lack-robots-rules) | Personal pages are indexable; search/settings lack robots rules | P3 | [25](25-pwa-and-page-metadata.md) |
| [BRW-01](26-cross-browser.md#brw-01--every-text-field-is-under-16-px--iphones-zoom-in-on-tap) | Every text field is under 16 px — iPhones zoom in on tap | P1 | [26](26-cross-browser.md) |
| [BRW-02](26-cross-browser.md#brw-02--continuous-reading-mode-breaks-arabic-lines-differently-per-engine) | Continuous reading mode breaks Arabic lines differently per engine | P2 | [26](26-cross-browser.md) |
| [BRW-03](26-cross-browser.md#brw-03--viewport-height-and-tap-details-for-ios) | Viewport-height and tap details for iOS | P3 | [26](26-cross-browser.md) |
| [BRW-04](26-cross-browser.md#brw-04--firefox-is-untested--code-level-risk-list) | Firefox is untested — code-level risk list | P2 | [26](26-cross-browser.md) |
<!-- register:end -->

## Related docs

- `docs/design-system.md` — the contract most VIS/A11Y/THEME findings measure against (one small value drift, from [_merge/fork-a.md](_merge/fork-a.md): Magenta dark `--primary` is `oklch(0.50 0.23 352)` in §5 but `oklch(0.51 0.22 352)` in `layout.css:526`).
- `docs/remaining/feature-gap-catalogue.md` — missing *features* (this audit covers the UX of what already ships; overlaps noted inline: R04, R05, D01, L01).
- `docs/remaining/copy-corrections.md` — placeholder copy ledger ([MKT-02](14-marketing-site-parity.md#mkt-02--website-copy-is-out-of-date-with-the-app) and [SCR-01](19-remaining-screens-and-flows.md#scr-01--legal-pages-say-placeholder-text-and-promise-things-the-app-doesnt-do) extend it).
