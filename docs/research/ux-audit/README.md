# easyquran UX audit — parity, consistency, accessibility, readability

**Date:** 2026-09-29 · **Build:** `master` @ `49e33b04` · **Scope:** every reader-facing screen, English + Arabic, light + dark, phone + tablet + desktop · **Method:** [00 · How this audit was done](00-how-this-audit-was-done.md)

easyquran is built for non-technical people who want to read the Qur'an simply. This audit asks one question of every screen: *would a first-time, non-technical reader — on a phone, maybe older, maybe Arabic-first, maybe unable to read Arabic — understand it, trust it, and get where they want to go?*

![Reader overview with the main problems numbered](screenshots/reader/overview-desktop.webp)

## The short version

**What's genuinely good.** The Arabic text is set beautifully and is the highest-contrast thing on the page. Offline reading works — with the network cut, you can still open a new surah. Search handles Arabic words, `2:255` and `juz 5`. The token system is contrast-gated, the skip link and ARIA labels are in place, and 115 translations are available.

**What holds it back.** Around that strong core, the app speaks developer, repeats itself, and drifts from its own design system:

1. **Trust.** The "tafsir" panel shows placeholder text; a floating button sits *on top of* Qur'an words on phones; mistyped links show a bare "Not found".
2. **Language.** Arabic readers get English on the home page, reader header, surah list and settings — and some pages flip to English on refresh.
3. **Translations.** Choosing a translation *removes* the Arabic and the Bismillah; the translator is never named; searching an English word returns zero results.
4. **Readability.** In dark mode, blue text is 2.6–3.1:1; many controls are 26–40 px (the design system promises 44); lots of text is 11–13 px, some in a code font.
5. **Consistency.** 7 search-box styles, 7 button styles, 4 "selected" styles, 4 icon families, 5 page widths, 3 spellings of surah names, 4 places to switch theme.

**By the numbers:** 97 findings (≈ 90 distinct; a few are cross-references) — **7 P0 · 35 P1 · 47 P2 · 8 P3**. axe-core: colour-contrast failures on 23 of 36 scans (18 of 18 in dark mode), duplicate/nested landmarks on every reader and list page, missing `<title>` on sign-in/register.

## Fix these first (P0)

| # | Finding | Why it's first |
| --- | --- | --- |
| 1 | [RDR-03 · Placeholder text shown as "TAFSIR"](03-reader.md#rdr-03--the-tafsir-panel-shows-placeholder-text-to-real-readers) | Religious-content trust |
| 2 | [RDR-01 · Floating button covers Qur'an text on phones](03-reader.md#rdr-01--the-floating-appearance-button-covers-the-quran-text-on-phones) | Hides the text itself |
| 3 | [TR-01 · Picking a translation removes the Arabic and Bismillah](04-translations.md#tr-01--picking-a-translation-removes-the-arabic-text-and-the-bismillah) | Breaks reader expectations |
| 4 | [SRCH-01 · English word search returns 0 results](05-search.md#srch-01--searching-an-english-word-returns-nothing-with-no-hint-why) | Most common query fails silently |
| 5 | [NAV-02 · Two URL schemes lose the language / 404](01-navigation-and-wayfinding.md#nav-02--two-url-schemes-some-links-lose-the-language-some-return-a-bare-not-found) | Arabic users lose Arabic |
| 6 | [RTL-04 · Settings/Search/Bookmarks/Yours English after refresh](10-arabic-and-rtl.md#rtl-04--settings-search-bookmarks-yours-english-body-and-english-after-refresh) | Same root cause as #5 |
| 7 | [STATE-01 · Raw text "Not found" for bad reader URLs](12-states-errors-offline.md#state-01--wrong-reader-urls-return-a-plain-text-not-found) | Looks broken; no way back |

Then the P1 wave: [A11Y-01 dark contrast](09-accessibility.md#a11y-01--dark-mode-uses-the-fill-blue-as-a-text-colour) · [A11Y-03 targets](09-accessibility.md#a11y-03--tap-targets-are-well-below-the-promised-44-px) · [A11Y-04 text size](09-accessibility.md#a11y-04--too-much-text-is-1113-px) · [HOME-04 translation on first run](02-home.md#home-04--new-readers-are-never-asked-do-you-read-arabic) · [RDR-02 verse tools](03-reader.md#rdr-02--verse-actions-are-tiny-unlabeled-and-ambiguous) · [SET-03 dev tools in Settings](08-settings-and-appearance.md#set-03--designer-and-developer-tools-are-exposed-to-readers). Full plan: [16 · Fix roadmap](16-fix-roadmap.md).

## Documents

| Doc | Covers | P0 | P1 | P2 | P3 |
| --- | --- | :-: | :-: | :-: | :-: |
| [00 · How this audit was done](00-how-this-audit-was-done.md) | Scope, environment, methods, severity scale, limits | | | | |
| [01 · Navigation and wayfinding](01-navigation-and-wayfinding.md) | Header, current page, URL/locale schemes, duplicate doors, logo, small-screen overflow | 1 | 3 | 3 | |
| [02 · App home](02-home.md) | First visit vs returning visit, layout, hero chips, first-run translation choice | | 4 | 3 | |
| [03 · Reader](03-reader.md) | Floating button, verse tools, placeholder tafsir, header noise, controls, reading mode, range readers, sidebar | 2 | 4 | 5 | 1 |
| [04 · Translations](04-translations.md) | Arabic disappears, credits, picker, phone picker, script fonts, API-down fallback | 1 | 3 | 2 | |
| [05 · Search](05-search.md) | English queries, ⌘K dead end, empty state, results, 7 search boxes | 1 | 1 | 3 | |
| [06 · Browse lists](06-browse-lists.md) | Surah spellings, rainbow numbers, juz notation, 604 pages, phone truncation | | 2 | 3 | 1 |
| [07 · Bookmarks, notes, Yours](07-bookmarks-notes-yours.md) | Two overlapping pages, missing notes, no undo | | 1 | 3 | 1 |
| [08 · Settings and appearance](08-settings-and-appearance.md) | Tab order, jargon, developer tools, switches, second Settings panel | | 5 | 3 | |
| [09 · Accessibility and readability](09-accessibility.md) | axe results, dark contrast, targets, text size, landmarks, focus, reflow | | 4 | 4 | |
| [10 · Arabic UI and RTL](10-arabic-and-rtl.md) | Untranslated screens, bidi breaks, locale loss, footer, digits | 1 | 3 | 2 | 1 |
| [11 · Visual consistency](11-visual-consistency.md) | Icons, search boxes, selected states, buttons, widths, fonts, colour meaning, tokens | | | 9 | 2 |
| [12 · States, errors, offline](12-states-errors-offline.md) | 404s, error page, API-down hangs, offline indicator, empty states | 1 | 3 | 2 | |
| [13 · Sign in and account](13-sign-in-and-account.md) | Exit, parity, social buttons, links, value message | | 1 | 3 | 1 |
| [14 · Marketing ↔ app parity](14-marketing-site-parity.md) | Two headers, outdated copy, phone landing, footer typo | | 1 | 2 | 1 |
| [15 · Plain language](15-plain-language-copy.md) | 37 copy rewrites, one-word-per-concept glossary, tone checklist | | | | |
| [16 · Fix roadmap](16-fix-roadmap.md) | 4 waves, effort, shared components, guard tests, user validation plan | | | | |
| [tools/](tools/README.md) | The capture/axe harness to re-run everything | | | | |

## Area health at a glance

| Area | Health | One-line verdict |
| --- | --- | --- |
| Qur'an text rendering | 🟢 Strong | Clear, high-contrast, respectful typography |
| Offline reading | 🟢 Strong | Works with the network off |
| Reader chrome | 🟠 Needs work | Noisy header, tiny unlabeled tools, floating button over text |
| Translations | 🔴 Weak | Replace the Arabic; picker and credits confusing |
| Search | 🟠 Needs work | Great for Arabic/refs, fails English words |
| Browse lists | 🟠 Needs work | Double spellings, code notation |
| Your stuff (bookmarks/notes) | 🟠 Needs work | Two pages, notes not listed |
| Settings | 🔴 Weak | Developer vocabulary and tools up front |
| Arabic UI | 🔴 Weak | Many English screens; locale lost on refresh |
| Accessibility | 🟠 Needs work | Dark contrast, target size, small text |
| Visual consistency | 🟠 Needs work | Many one-off styles despite a good design system |
| Errors / edge states | 🟠 Needs work | Raw 404s, hangs when API is unreachable |

## Screenshots

All evidence lives in [`screenshots/`](screenshots/) (64 WebP files, ≈ 2.5 MB), one folder per area. **Red boxes and numbered labels mark the problem**; comparison images put two or more screens side by side. Every screenshot is embedded in the sub-document that discusses it.

| Folder | Examples |
| --- | --- |
| `navigation/` | header without current page, menu panel, 320 px overflow, raw "Not found" |
| `home/` | first visit, returning visit, phone |
| `reader/` | overview, floating button, verse tools zoom, tafsir placeholder, header colours, sidebar spelling, dark mode |
| `translations/` | Arabic vs translated, credits, picker desktop/phone, API-down failure |
| `search/` | English zero results, palette dead end, empty state, Arabic results |
| `lists/`, `bookmarks/`, `settings/` | per-page evidence |
| `accessibility/` | dark primary text, tap-target map, focus ring |
| `rtl/` | Arabic home, reader header, list, settings, locale lost on reload |
| `visual/` | 7 search boxes, icon families, button styles, selected styles, page widths |
| `states/`, `auth/`, `marketing/` | error pages, offline, account hang, sign-in, header comparison |

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
<!-- register:end -->

## Related docs

- `docs/design-system.md` — the contract most VIS/A11Y findings measure against.
- `docs/remaining/feature-gap-catalogue.md` — missing *features* (this audit covers the UX of what already ships; overlaps noted inline: R04, R05, D01, L01).
- `docs/remaining/copy-corrections.md` — placeholder copy ledger (MKT-02 extends it).
