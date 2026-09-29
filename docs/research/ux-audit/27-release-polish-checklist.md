# 27 · Release polish checklist

[← Back to the index](README.md)

One checklist for the pre-release UI polish, generated from every finding in this folder. Tick items as they land; each links to the full write-up (screenshot, cause, fix, file). Cross-reference entries (findings that point at another finding) are folded into the item they point to.

**Totals (distinct items):** 10 P0 · 59 P1 · 95 P2 · 36 P3.

**Release gate suggestion:** all P0 and P1 ticked, P2 ≥ 80 % ticked, and the [guard tests](16-fix-roadmap.md#guardrails-to-add-so-it-stays-fixed) green in CI.

Order inside each group follows the [roadmap](16-fix-roadmap.md): shared components first, then call sites.

## P0 — Must fix before release (10)

Trust or core-task blockers. Also treat [THEME-01](17-themes-palettes-and-scripts.md#theme-01--custom-colours-can-make-the-quran-text-invisible) as P0 until custom colours are hidden from readers.

### 01 · Navigation and wayfinding

- [ ] **[NAV-02](01-navigation-and-wayfinding.md#nav-02--two-url-schemes-some-links-lose-the-language-some-return-a-bare-not-found)** Two URL schemes: some links lose the language, some return a bare "Not found"  
  _Done when:_ Any `/app` URL can be refreshed in Arabic and stays Arabic; `/en/app/search` and `/ar/app/settings` both load a real page.

### 03 · Reader (surah, page and juz readers)

- [ ] **[RDR-01](03-reader.md#rdr-01--the-floating-appearance-button-covers-the-quran-text-on-phones)** The floating appearance button covers the Qur'an text on phones  
  _Done when:_ At 390 × 844, no fixed element overlaps Qur'an text, list cards or footer links at any scroll position.
- [ ] **[RDR-03](03-reader.md#rdr-03--the-tafsir-panel-shows-placeholder-text-to-real-readers)** The "tafsir" panel shows placeholder text to real readers  
  _Done when:_ No user-visible string contains "Sample" or "in the full app".

### 04 · Translations

- [ ] **[TR-01](04-translations.md#tr-01--picking-a-translation-removes-the-arabic-text-and-the-bismillah)** Picking a translation removes the Arabic text (and the Bismillah)  
  _Done when:_ The translated reader shows the same Arabic + Bismillah as the Arabic reader, with the translation added.

### 05 · Search

- [ ] **[SRCH-01](05-search.md#srch-01--searching-an-english-word-returns-nothing-with-no-hint-why)** Searching an English word returns nothing, with no hint why  
  _Done when:_ A fresh visitor searching "mercy" sees English verse results without touching any setting.

### 10 · Arabic UI and right-to-left

- [ ] **[RTL-04](10-arabic-and-rtl.md#rtl-04--settings-search-bookmarks-yours-english-body-and-english-after-refresh)** Settings, Search, Bookmarks, Yours: English body, and English after refresh

### 12 · Loading, empty, error and offline states

- [ ] **[STATE-01](12-states-errors-offline.md#state-01--wrong-reader-urls-return-a-plain-text-not-found)** Wrong reader URLs return a plain-text "Not found"  
  _Done when:_ Every 404 a browser can hit shows the app header and a way back.

### 19 · Remaining screens and flows

- [ ] **[SCR-01](19-remaining-screens-and-flows.md#scr-01--legal-pages-say-placeholder-text-and-promise-things-the-app-doesnt-do)** Legal pages say "placeholder text" and promise things the app doesn't do  
  _Done when:_ No legal page contains "placeholder"; every capability named in Terms/Privacy can be found in the app; Terms, Privacy, Contact and footer show the same contact.

### 22 · Translations deep dive

- [ ] **[TRX-01](22-translations-deep-dive.md#trx-01--arabic-commentary-chosen-as-the-main-translation-looks-like-quran-text)** Arabic commentary chosen as the main translation looks like Qur'an text  
  _Done when:_ No route shows commentary in the verse position, and every tafsir line carries the word "commentary"/"تفسير".

### 25 · Installed app (PWA) and page metadata

- [ ] **[PWA-01](25-pwa-and-page-metadata.md#pwa-01--the-home-screen-icon-and-favicons-are-broken-off-brand-artwork)** The home-screen icon and favicons are broken, off-brand artwork  
  _Done when:_ Tab, launcher (circle and squircle masks) and iOS home screen all show the blue ق mark.

## P1 — Should fix before release (59)

Real friction for many readers or WCAG 2.2 AA failures.

### 01 · Navigation and wayfinding

- [ ] **[NAV-01](01-navigation-and-wayfinding.md#nav-01--the-header-never-shows-which-section-you-are-in)** The header never shows which section you are in  
  _Done when:_ On every index page, exactly one header link is visibly different and has `aria-current="page"`.
- [ ] **[NAV-05](01-navigation-and-wayfinding.md#nav-05--header-overflows-on-small-phones)** Header overflows on small phones  
  _Done when:_ No header control is clipped at 320 px, online or offline.
- [ ] **[NAV-06](01-navigation-and-wayfinding.md#nav-06--the-reader-sub-bar-hides-two-key-tools-behind-unexplained-icons)** The reader sub-bar hides two key tools behind unexplained icons  
  _Done when:_ A new user can find "read with English translation" within 10 seconds without hovering.

### 02 · App home (`/en/app`, `/ar/app`)

- [ ] **[HOME-01](02-home.md#home-01--once-you-have-read-anything-the-browse-shortcuts-disappear)** Once you have read anything, the browse shortcuts disappear  
  _Done when:_ A returning reader sees Continue + all four browse shortcuts on one screen.
- [ ] **[HOME-04](02-home.md#home-04--new-readers-are-never-asked-do-you-read-arabic)** New readers are never asked "Do you read Arabic?"  
  _Done when:_ A first-time English-speaking visitor reaches a verse with an English translation in one tap from home.
- [ ] **[HOME-05](02-home.md#home-05--home-copy-is-hard-coded-english)** Home copy is hard-coded English

### 03 · Reader (surah, page and juz readers)

- [ ] **[RDR-02](03-reader.md#rdr-02--verse-actions-are-tiny-unlabeled-and-ambiguous)** Verse actions are tiny, unlabeled and ambiguous  
  _Done when:_ Every verse action has a visible text label somewhere in the flow (sheet or inline), and each target is ≥ 44 × 44 px on phones.
- [ ] **[RDR-05](03-reader.md#rdr-05--text-size-and-mode-controls-are-small-and-unclear)** Text-size and mode controls are small and unclear  
  _Done when:_ Controls are ≥ 44 px, show their current value, and read the same on phone and desktop.
- [ ] **[RDR-09](03-reader.md#rdr-09--the-sidebar-spells-some-arabic-surah-names-differently-from-the-rest-of-the-app)** The sidebar spells some Arabic surah names differently from the rest of the app  
  _Done when:_ The same Arabic name string is used for a surah everywhere.
- [ ] **[RDR-10](03-reader.md#rdr-10--dark-mode-ayah-markers-and-the-wordmark-are-too-faint)** Dark mode: ayah markers and the wordmark are too faint  
  _Done when:_ axe reports no colour-contrast issues on the dark reader.

### 04 · Translations

- [ ] **[TR-02](04-translations.md#tr-02--the-main-translation-is-never-credited)** The main translation is never credited  
  _Done when:_ A reader can name the translation on screen without looking at the URL.
- [ ] **[TR-03](04-translations.md#tr-03--the-translation-picker-opens-on-arabic-tafsir-uses-flags-and-says-primary)** The translation picker opens on Arabic *tafsir*, uses flags, and says "primary"  
  _Done when:_ An English-UI user sees English translations first; no flags are shown; no internal words remain.
- [ ] **[TR-06](04-translations.md#tr-06--when-the-api-is-unreachable-translated-pages-fail-without-falling-back-to-arabic)** When the API is unreachable, translated pages fail without falling back to Arabic  
  _Done when:_ With the API stopped, `/en/app/al-baqarah/t/en/sahih` shows 2:1 onward with Arabic and a clear notice.

### 05 · Search

- [ ] **[SRCH-02](05-search.md#srch-02--the-k-palette-dead-ends-on-words)** The ⌘K palette dead-ends on words  
  _Done when:_ Enter on any palette query that has no quick match opens full search.

### 06 · Browse lists (Surahs, Juz, Pages)

- [ ] **[LIST-01](06-browse-lists.md#list-01--every-surah-has-two-or-three-english-spellings)** Every surah has two (or three) English spellings  
  _Done when:_ A surah's English name is spelled identically on every screen.
- [ ] **[LIST-03](06-browse-lists.md#list-03--juz-list-is-written-in-reference-code)** Juz list is written in reference code

### 07 · Bookmarks, notes and "Yours"

- [ ] **[BM-03](07-bookmarks-notes-yours.md#bm-03--notes-are-saved-but-never-shown-anywhere)** Notes are saved but never shown anywhere  
  _Done when:_ A note written on 2:1 is visible from Yours within one tap.

### 08 · Settings and appearance controls

- [ ] **[SET-01](08-settings-and-appearance.md#set-01--settings-opens-on-storage-the-most-technical-tab)** Settings opens on "Storage", the most technical tab
- [ ] **[SET-02](08-settings-and-appearance.md#set-02--storage-copy-is-browserdeveloper-jargon)** Storage copy is browser/developer jargon
- [ ] **[SET-03](08-settings-and-appearance.md#set-03--designer-and-developer-tools-are-exposed-to-readers)** Designer and developer tools are exposed to readers
- [ ] **[SET-04](08-settings-and-appearance.md#set-04--toggles-are-on-text-pills-analytics-is-on-by-default)** Toggles are "on" text pills; analytics is on by default
- [ ] **[SET-08](08-settings-and-appearance.md#set-08--a-second-settings-floats-over-every-page)** A second "Settings" floats over every page  
  _Done when:_ There is exactly one surface titled "Settings".

### 09 · Accessibility and readability

- [ ] **[A11Y-01](09-accessibility.md#a11y-01--dark-mode-uses-the-fill-blue-as-a-text-colour)** Dark mode uses the fill blue as a text colour  
  _Done when:_ axe shows 0 color-contrast issues on the dark reader, landing, and contact pages.
- [ ] **[A11Y-03](09-accessibility.md#a11y-03--tap-targets-are-well-below-the-promised-44-px)** Tap targets are well below the promised 44 px
- [ ] **[A11Y-04](09-accessibility.md#a11y-04--too-much-text-is-1113-px)** Too much text is 11–13 px
- [ ] **[A11Y-08](09-accessibility.md#a11y-08--reflow-and-obscured-content)** Reflow and obscured content

### 10 · Arabic UI and right-to-left

- [ ] **[RTL-01](10-arabic-and-rtl.md#rtl-01--app-home-hero-and-continue-card-are-english)** App home hero and continue card are English
- [ ] **[RTL-02](10-arabic-and-rtl.md#rtl-02--reader-header-english-name-first-broken-number-order-english-metadata)** Reader header: English name first, broken number order, English metadata
- [ ] **[RTL-03](10-arabic-and-rtl.md#rtl-03--surah-list-metadata-is-english-in-arabic-ui)** Surah list metadata is English in Arabic UI

### 12 · Loading, empty, error and offline states

- [ ] **[STATE-02](12-states-errors-offline.md#state-02--the-error-page-is-a-dead-end)** The error page is a dead end
- [ ] **[STATE-03](12-states-errors-offline.md#state-03--account-page-spins-forever-when-the-api-is-unreachable)** Account page spins forever when the API is unreachable

### 13 · Sign in, register and account

- [ ] **[AUTH-01](13-sign-in-and-account.md#auth-01--no-way-back-from-sign-in--create-account)** No way back from Sign in / Create account

### 14 · Marketing site ↔ app parity

- [ ] **[MKT-02](14-marketing-site-parity.md#mkt-02--website-copy-is-out-of-date-with-the-app)** Website copy is out of date with the app

### 17 · Themes, palettes and Arabic scripts

- [ ] **[THEME-01](17-themes-palettes-and-scripts.md#theme-01--custom-colours-can-make-the-quran-text-invisible)** Custom colours can make the Qur'an text invisible  
  _Done when:_ For any background seed in either mode, Qur'an text on the reader ground measures ≥ 7:1 (add a property test over seeds `#000…#fff` to `token-contrast.test.ts`).
- [ ] **[THEME-02](17-themes-palettes-and-scripts.md#theme-02--custom-colours-pick-the-wrong-text-colour-for-middle-tones)** Custom colours pick the wrong text colour for middle tones  
  _Done when:_ Every derived text/fill pair passes the same floors as the built-in palettes (§9) for any seed.
- [ ] **[THEME-03](17-themes-palettes-and-scripts.md#theme-03--the-focus-ring-is-too-faint-on-dark-surfaces-cobalt-magenta-emerald)** The focus ring is too faint on dark surfaces (Cobalt, Magenta, Emerald)
- [ ] **[SCRIPT-01](17-themes-palettes-and-scripts.md#script-01--tajweed-colours-dont-change-for-dark-mode-several-letters-nearly-vanish)** Tajweed colours don't change for dark mode; several letters nearly vanish  
  _Done when:_ Every tajweed colour is ≥ 3:1 on the reader ground in both modes.
- [ ] **[SCRIPT-02](17-themes-palettes-and-scripts.md#script-02--the-kfgqpc-fonts-draw-the-verse-end-marker-wrongly)** The KFGQPC fonts draw the verse-end marker wrongly  
  _Done when:_ The marker looks the same (number inside one ornament) for all six fonts and four scripts.
- [ ] **[SCRIPT-03](17-themes-palettes-and-scripts.md#script-03--indopak-text-with-kfgqpc-hafs-shows-a-dotted-circle-in-place-of-a-letter)** IndoPak text with KFGQPC Hafs shows a dotted circle in place of a letter  
  _Done when:_ No script × font combination offered in Settings renders a missing glyph for any verse.

### 18 · Display conditions: text size, high contrast, motion, screen sizes

- [ ] **[DISP-01](18-display-conditions.md#disp-01--the-app-ignores-the-browsers-text-size-setting)** The app ignores the browser's text-size setting  
  _Done when:_ With the browser default set to 20 px, UI body text becomes ≈ 19 px and translation ≈ 21 px.
- [ ] **[DISP-02](18-display-conditions.md#disp-02--windows-high-contrast-nothing-shows-what-is-selected)** Windows High Contrast: nothing shows what is selected  
  _Done when:_ With forced colours on, every selected control is distinguishable from its siblings.

### 19 · Remaining screens and flows

- [ ] **[SCR-05](19-remaining-screens-and-flows.md#scr-05--nothing-tracking-what-you-read-while-analytics-is-on-by-default)** "Nothing tracking what you read" while analytics is on by default
- [ ] **[FLOW-01](19-remaining-screens-and-flows.md#flow-01--clear-cached-pages--data--no-confirmation-no-visible-result)** "Clear cached pages & data" — no confirmation, no visible result  
  _Done when:_ The label says what is removed, a confirmation appears, and the numbers visibly change.
- [ ] **[FLOW-03](19-remaining-screens-and-flows.md#flow-03--search-has-a-second-different-translation-picker--and-it-needs-two-steps)** Search has a second, different translation picker — and it needs two steps

### 20 · Signed-in experience

- [ ] **[ACCT-01](20-signed-in-experience.md#acct-01--signing-out-silently-removes-your-account-bookmarks-from-the-device)** Signing out silently removes your account bookmarks from the device  
  _Done when:_ After signing out, the reader either still sees their bookmarks or sees a clear message saying where they are.
- [ ] **[ACCT-02](20-signed-in-experience.md#acct-02--the-account-page-is-a-bare-developer-screen)** The Account page is a bare developer screen  
  _Done when:_ Every session is distinguishable, times are human-readable, and a reader can get back to the Qur'an in one tap.
- [ ] **[ACCT-08](20-signed-in-experience.md#acct-08--server-and-configuration-errors-reach-readers-raw-or-mislabelled)** Server and configuration errors reach readers raw or mislabelled  
  _Done when:_ No auth path can show raw JSON, and each error message matches its real cause.

### 21 · Keyboard, focus and screen reader

- [ ] **[KEY-01](21-keyboard-focus-and-screen-reader.md#key-01--screen-readers-are-interrupted-with-page-n-of-48-while-you-scroll)** Screen readers are interrupted with "Page N of 48" while you scroll  
  _Done when:_ A mutation observer on `#svelte-announcer` records 0 changes while scrolling Al-Baqarah from top to page 10.
- [ ] **[KEY-03](21-keyboard-focus-and-screen-reader.md#key-03--every-verse-adds-four-tab-stops-with-identical-names)** Every verse adds four Tab stops with identical names  
  _Done when:_ From the top of Al-Baqarah, the page navigation is reachable in under 20 key presses, and no two buttons on a reader page share a name.

### 22 · Translations deep dive

- [ ] **[TRX-02](22-translations-deep-dive.md#trx-02--the-transliteration-shows-raw-html-tags)** The transliteration shows raw HTML tags  
  _Done when:_ No `<` or `>` character appears in any rendered or copied transliteration verse.
- [ ] **[TRX-03](22-translations-deep-dive.md#trx-03--the-same-translator-is-listed-twice-and-some-languages-twice)** The same translator is listed twice, and some languages twice  
  _Done when:_ No translator appears twice within a language, and each language appears once in the rail.

### 23 · Interaction details (the last 10% of polish)

- [ ] **[INT-01](23-interaction-details.md#int-01--a-shared-link-to-a-long-verse-opens-in-the-wrong-place-on-phones)** A shared link to a long verse opens in the wrong place on phones  
  _Done when:_ `#ayah-2-282` opens with the start of 2:282 fully visible below the bar at 390 and 1440 px.

### 24 · Loading and perceived performance

- [ ] **[LOAD-01](24-loading-and-perceived-performance.md#load-01--the-reader-shows-text-for-25-s-before-its-controls-exist-then-the-page-jumps)** The reader shows text for ~25 s before its controls exist, then the page jumps  
  _Done when:_ Reader and Surahs CLS < 0.02 on the throttled run; controls are visible in the first frame that shows text.
- [ ] **[LOAD-02](24-loading-and-perceived-performance.md#load-02--preparing-offline-quran-sits-on-top-of-the-header-for-the-whole-download)** "Preparing offline Quran" sits on top of the header for the whole download  
  _Done when:_ No status element overlaps or blocks header controls at any time; Arabic UI shows Arabic.
- [ ] **[LOAD-03](24-loading-and-perceived-performance.md#load-03--a-shared-search-link-ignores-its-query-until-the-app-wakes-up)** A shared search link ignores its query until the app wakes up  
  _Done when:_ From the first paint the box contains the query and a status line; CLS < 0.1.

### 25 · Installed app (PWA) and page metadata

- [ ] **[PWA-02](25-pwa-and-page-metadata.md#pwa-02--manifest-installed-app-always-opens-in-english-old-colours-brand-spelling)** Manifest: installed app always opens in English, old colours, brand spelling  
  _Done when:_ Installing from an Arabic page opens the Arabic app; the splash colour matches the reader's theme; "Continue reading" resumes.
- [ ] **[PWA-03](25-pwa-and-page-metadata.md#pwa-03--tab-titles-follow-five-patterns-several-pages-have-no-title)** Tab titles follow five patterns; several pages have no title  
  _Done when:_ All 33 routes above have a title in the one pattern, in the page's language.
- [ ] **[PWA-06](25-pwa-and-page-metadata.md#pwa-06--in-the-installed-app-some-screens-have-no-way-back)** In the installed app, some screens have no way back

### 26 · Cross-browser (Safari engine vs Chrome)

- [ ] **[BRW-01](26-cross-browser.md#brw-01--every-text-field-is-under-16-px--iphones-zoom-in-on-tap)** Every text field is under 16 px — iPhones zoom in on tap  
  _Done when:_ Every `input`/`textarea` computes to ≥ 16 px at 393 px width.

## P2 — Polish pass (95)

Consistency and finish — what makes it feel release-quality.

### 01 · Navigation and wayfinding

- [ ] **[NAV-03](01-navigation-and-wayfinding.md#nav-03--same-action-many-doors-theme--4-search--3-settings--2)** Same action, many doors (theme × 4, search × 3, settings × 2)  
  _Done when:_ Each action has one obvious home per screen size, and duplicate entry points (if kept) look and behave identically.
- [ ] **[NAV-04](01-navigation-and-wayfinding.md#nav-04--the-logo-leaves-the-app)** The logo leaves the app  
  _Done when:_ From any reader page, the logo returns to the app home in the same language.
- [ ] **[NAV-07](01-navigation-and-wayfinding.md#nav-07--yours-bookmarks-and-the-home-yours-card-overlap)** "Yours", "Bookmarks", and the home "Yours" card overlap  
  _Done when:_ The same word is used in the header, the home card, the footer and the page title, in both languages.

### 02 · App home (`/en/app`, `/ar/app`)

- [ ] **[HOME-02](02-home.md#home-02--layout-is-lopsided-on-desktop-and-the-juz-card-is-shorter-than-its-neighbours)** Layout is lopsided on desktop and the Juz card is shorter than its neighbours  
  _Done when:_ At 1440 px the home content is centred; all four cards are the same height in both themes; no card uses a word where others use a number.
- [ ] **[HOME-03](02-home.md#home-03--hero-chips-only-the-first-looks-like-a-button)** Hero chips: only the first looks like a button  
  _Done when:_ All hero chips look equally tappable on a phone.
- [ ] **[HOME-06](02-home.md#home-06--no-page-title-h1)** No page title (`<h1>`)

### 03 · Reader (surah, page and juz readers)

- [ ] **[RDR-04](03-reader.md#rdr-04--the-surah-number-is-shown-four-times-the-header-colour-changes-per-surah)** The surah number is shown four times; the header colour changes per surah  
  _Done when:_ The surah number appears once per screen and header colour is identical (or meaningfully explained) across surahs.
- [ ] **[RDR-06](03-reader.md#rdr-06--reading-mode-on-phones-spreads-words-far-apart)** Reading mode on phones spreads words far apart  
  _Done when:_ No gap between words exceeds roughly one word-width on a 390 px screen.
- [ ] **[RDR-07](03-reader.md#rdr-07--page-and-juz-readers-lack-the-surah-readers-header-and-controls)** Page and Juz readers lack the surah reader's header and controls  
  _Done when:_ Switching between surah, page and juz readers keeps the same header layout and controls.
- [ ] **[RDR-08](03-reader.md#rdr-08--bookmarked-state-is-a-thin-colour-change-only)** Bookmarked state is a thin colour change only  
  _Done when:_ A bookmarked verse is recognisable in greyscale.
- [ ] **[RDR-11](03-reader.md#rdr-11--verse-numbers-are-code-style-and-tiny)** Verse numbers are code-style and tiny  
  _Done when:_ No reader-facing label uses `font-mono`.

### 04 · Translations

- [ ] **[TR-04](04-translations.md#tr-04--on-phones-the-picker-hides-translators-one-level-deep)** On phones the picker hides translators one level deep
- [ ] **[TR-05](04-translations.md#tr-05--extra-language-lines-are-small-and-use-a-generic-font)** Extra-language lines are small and use a generic font

### 05 · Search

- [ ] **[SRCH-03](05-search.md#srch-03--the-empty-state-points-at-something-that-isnt-there)** The empty state points at something that isn't there
- [ ] **[SRCH-04](05-search.md#srch-04--results-show-arabic-only-floating-mid-row)** Results show Arabic only, floating mid-row

### 06 · Browse lists (Surahs, Juz, Pages)

- [ ] **[LIST-02](06-browse-lists.md#list-02--rainbow-numbers-carry-no-meaning-and-the-green-fails-contrast)** Rainbow numbers carry no meaning (and the green fails contrast)
- [ ] **[LIST-04](06-browse-lists.md#list-04--604-page-cards-and-no-go-to-page-box)** 604 page cards and no "go to page" box
- [ ] **[LIST-05](06-browse-lists.md#list-05--phone-rows-cut-off-the-verse-count)** Phone rows cut off the verse count

### 07 · Bookmarks, notes and "Yours"

- [ ] **[BM-01](07-bookmarks-notes-yours.md#bm-01--two-pages-two-layouts-two-formats-for-the-same-bookmark)** Two pages, two layouts, two formats for the same bookmark  
  _Done when:_ A bookmark row is the same component on Yours, Bookmarks and Home.
- [ ] **[BM-02](07-bookmarks-notes-yours.md#bm-02--bookmark-rows-have-no-context)** Bookmark rows have no context
- [ ] **[BM-04](07-bookmarks-notes-yours.md#bm-04--remove-deletes-instantly-no-undo)** "Remove" deletes instantly, no undo

### 08 · Settings and appearance controls

- [ ] **[SET-05](08-settings-and-appearance.md#set-05--a-focus-rectangle-is-drawn-around-the-whole-panel-after-clicking-a-tab)** A focus rectangle is drawn around the whole panel after clicking a tab
- [ ] **[SET-06](08-settings-and-appearance.md#set-06--phone-tabs-run-off-screen-with-no-hint)** Phone: tabs run off-screen with no hint
- [ ] **[SET-07](08-settings-and-appearance.md#set-07--reading-settings-speak-in-pixels-and-font-file-names)** Reading settings speak in pixels and font file names

### 09 · Accessibility and readability

- [ ] **[A11Y-02](09-accessibility.md#a11y-02--light-mode-green-on-green-chips-and-the-juz-card-caption)** Light mode: green-on-green chips and the Juz card caption
- [ ] **[A11Y-05](09-accessibility.md#a11y-05--duplicate-and-nested-landmarks-missing-page-titles)** Duplicate and nested landmarks; missing page titles
- [ ] **[A11Y-06](09-accessibility.md#a11y-06--links-identified-by-colour-only-focus-ring-style-inconsistent)** Links identified by colour only; focus ring style inconsistent
- [ ] **[A11Y-07](09-accessibility.md#a11y-07--state-shown-by-colour-or-hover-alone)** State shown by colour or hover alone

### 10 · Arabic UI and right-to-left

- [ ] **[RTL-05](10-arabic-and-rtl.md#rtl-05--arabic-footer-drops-company-and-legal)** Arabic footer drops "Company" and "Legal"
- [ ] **[RTL-06](10-arabic-and-rtl.md#rtl-06--translation-picker-lists-language-names-in-english)** Translation picker lists language names in English

### 11 · Visual consistency (design-system parity)

- [ ] **[VIS-01](11-visual-consistency.md#vis-01--four-icon-families-and-two-icon-weights)** Four icon families and two icon weights
- [ ] **[VIS-02](11-visual-consistency.md#vis-02--icons-mean-different-things-in-different-places)** Icons mean different things in different places
- [ ] **[VIS-03](11-visual-consistency.md#vis-03--seven-different-search-boxes)** Seven different search boxes
- [ ] **[VIS-04](11-visual-consistency.md#vis-04--four-different-selected-looks)** Four different "selected" looks
- [ ] **[VIS-05](11-visual-consistency.md#vis-05--buttons-come-in-seven-shapes)** Buttons come in seven shapes
- [ ] **[VIS-06](11-visual-consistency.md#vis-06--every-page-uses-a-different-width-and-header-pattern)** Every page uses a different width and header pattern
- [ ] **[VIS-07](11-visual-consistency.md#vis-07--monospace-and-letter-spacing-where-people-read-words)** Monospace and letter-spacing where people read words
- [ ] **[VIS-08](11-visual-consistency.md#vis-08--colour-used-as-decoration-not-meaning)** Colour used as decoration, not meaning

### 12 · Loading, empty, error and offline states

- [ ] **[STATE-05](12-states-errors-offline.md#state-05--offline-indicator-a-dot-on-phones-and-nothing-about-what-still-works)** Offline indicator: a dot on phones, and nothing about what still works
- [ ] **[STATE-06](12-states-errors-offline.md#state-06--empty-states-are-inconsistent-and-sometimes-wrong)** Empty states are inconsistent and sometimes wrong

### 13 · Sign in, register and account

- [ ] **[AUTH-02](13-sign-in-and-account.md#auth-02--sign-in-and-create-account-dont-match)** Sign in and Create account don't match
- [ ] **[AUTH-03](13-sign-in-and-account.md#auth-03--social-sign-in-rows-dont-look-like-buttons-github-for-this-audience)** Social sign-in rows don't look like buttons; GitHub for this audience
- [ ] **[AUTH-04](13-sign-in-and-account.md#auth-04--small-colour-only-links-and-missing-page-titles)** Small, colour-only links and missing page titles

### 14 · Marketing site ↔ app parity

- [ ] **[MKT-01](14-marketing-site-parity.md#mkt-01--two-different-headers)** Two different headers
- [ ] **[MKT-03](14-marketing-site-parity.md#mkt-03--landing-on-phones-headline-fills-the-screen-header-search-is-s)** Landing on phones: headline fills the screen; header search is "S…"

### 17 · Themes, palettes and Arabic scripts

- [ ] **[THEME-04](17-themes-palettes-and-scripts.md#theme-04--the-chosen-palette-only-reaches-the-buttons)** The chosen palette only reaches the buttons  
  _Done when:_ A screenshot of home and reader in each palette shows only that palette's accent plus neutrals.
- [ ] **[THEME-05](17-themes-palettes-and-scripts.md#theme-05--ink-dark-mode-turns-the-home-hero-into-a-bright-white-slab)** Ink dark mode turns the home hero into a bright white slab
- [ ] **[THEME-06](17-themes-palettes-and-scripts.md#theme-06--see-through-text-on-colour-fills-fails-in-magenta-and-emerald)** See-through text on colour fills fails in Magenta and Emerald
- [ ] **[THEME-07](17-themes-palettes-and-scripts.md#theme-07--the-selected-palette-cards-description-is-37391-in-dark-mode)** The selected palette card's description is 3.7–3.9:1 in dark mode
- [ ] **[SCRIPT-04](17-themes-palettes-and-scripts.md#script-04--the-chosen-script-and-font-appear-late-uthmani-and-amiri-flash-first)** The chosen script and font appear late: Uthmani and Amiri flash first
- [ ] **[SCRIPT-05](17-themes-palettes-and-scripts.md#script-05--search-results-ignore-the-chosen-script-and-font)** Search results ignore the chosen script and font

### 18 · Display conditions: text size, high contrast, motion, screen sizes

- [ ] **[DISP-03](18-display-conditions.md#disp-03--translation-lines-are-100-characters-long-on-wide-screens)** Translation lines are ~100 characters long on wide screens

### 19 · Remaining screens and flows

- [ ] **[SCR-02](19-remaining-screens-and-flows.md#scr-02--legal-pages-are-hard-to-read)** Legal pages are hard to read
- [ ] **[SCR-03](19-remaining-screens-and-flows.md#scr-03--auth-side-pages-look-like-a-different-product)** Auth side pages look like a different product
- [ ] **[SCR-04](19-remaining-screens-and-flows.md#scr-04--landing-lower-sections-wrong-text-colour-a-self-link-a-very-long-list)** Landing lower sections: wrong text colour, a self-link, a very long list
- [ ] **[SCR-06](19-remaining-screens-and-flows.md#scr-06--every-reader-type-ends-differently--and-the-end-of-the-quran-is-a-small-link)** Every reader type ends differently — and the end of the Qur'an is a small link
- [ ] **[FLOW-02](19-remaining-screens-and-flows.md#flow-02--offline-pack-no-progress-no-ready-moment-developer-wording)** Offline pack: no progress, no "ready" moment, developer wording
- [ ] **[FLOW-04](19-remaining-screens-and-flows.md#flow-04--storage-download-rows-jargon-chips-and-an-unexplained-disabled-button)** Storage download rows: jargon chips and an unexplained disabled button
- [ ] **[FLOW-05](19-remaining-screens-and-flows.md#flow-05--shared-verses-carry-no-link-back)** Shared verses carry no link back
- [ ] **[FLOW-06](19-remaining-screens-and-flows.md#flow-06--k-go-to-verse-lands-on-the-page-not-the-verse-deep-links-dont-highlight)** ⌘K "go to verse" lands on the page, not the verse; deep links don't highlight
- [ ] **[FLOW-07](19-remaining-screens-and-flows.md#flow-07--switching-language-throws-away-your-reading-position)** Switching language throws away your reading position
- [ ] **[FLOW-08](19-remaining-screens-and-flows.md#flow-08--many-bookmarks-become-an-unsorted-wall)** Many bookmarks become an unsorted wall
- [ ] **[FLOW-09](19-remaining-screens-and-flows.md#flow-09--update-and-notification-toasts-cover-the-header-and-have-tiny-controls)** Update and notification toasts cover the header and have tiny controls

### 20 · Signed-in experience

- [ ] **[ACCT-03](20-signed-in-experience.md#acct-03--nothing-shows-that-youre-signed-in)** Nothing shows that you're signed in
- [ ] **[ACCT-04](20-signed-in-experience.md#acct-04--bookmark-folders-squashed-add-button-disguised-dropdowns-repeated-labels)** Bookmark folders: squashed add button, disguised dropdowns, repeated labels
- [ ] **[ACCT-06](20-signed-in-experience.md#acct-06--auth-forms-tiny-errors-mixed-icons-hidden-password-rule)** Auth forms: tiny errors, mixed icons, hidden password rule
- [ ] **[ACCT-07](20-signed-in-experience.md#acct-07--verification-and-reset-codes-dont-match-and-the-copy-talks-to-strangers)** Verification and reset codes don't match, and the copy talks to strangers
- [ ] **[ACCT-09](20-signed-in-experience.md#acct-09--signing-in-merges-local-bookmarks--say-so-and-say-what-doesnt-sync)** Signing in merges local bookmarks — say so, and say what doesn't sync

### 21 · Keyboard, focus and screen reader

- [ ] **[KEY-02](21-keyboard-focus-and-screen-reader.md#key-02--two-skip-to-content-links--and-in-arabic-the-first-one-is-english)** Two "Skip to content" links — and in Arabic the first one is English  
  _Done when:_ The first Tab on `/en/app` and `/ar/app` shows one skip link in the page's language; the second Tab is the logo.
- [ ] **[KEY-04](21-keyboard-focus-and-screen-reader.md#key-04--the-focus-ring-vanishes-on-the-blue-surahs-card)** The focus ring vanishes on the blue "Surahs" card  
  _Done when:_ Each metric card shows a ring with ≥ 3:1 contrast against both the card and the page ground.
- [ ] **[KEY-05](21-keyboard-focus-and-screen-reader.md#key-05--the-search-palette-drops-focus-when-it-closes)** The search palette drops focus when it closes  
  _Done when:_ After Escape, focus is on the header search button (or the element that had focus before the shortcut).
- [ ] **[KEY-06](21-keyboard-focus-and-screen-reader.md#key-06--three-dialogs-leave-the-page-behind-them-readable)** Three dialogs leave the page behind them readable  
  _Done when:_ With each modal open, the accessibility tree contains only the dialog's contents.
- [ ] **[KEY-09](21-keyboard-focus-and-screen-reader.md#key-09--keyboard-shortcuts-are-hidden-and-some-are-single-keys-that-cant-be-turned-off)** Keyboard shortcuts are hidden, and some are single keys that can't be turned off  
  _Done when:_ Every shortcut is listed in one place, and single-key shortcuts can be switched off.
- [ ] **[KEY-10](21-keyboard-focus-and-screen-reader.md#key-10--vague-or-duplicated-control-names)** Vague or duplicated control names
- [ ] **[KEY-11](21-keyboard-focus-and-screen-reader.md#key-11--english-accessible-names-in-the-arabic-ui)** English accessible names in the Arabic UI
- [ ] **[KEY-12](21-keyboard-focus-and-screen-reader.md#key-12--heading-outline-gaps)** Heading outline gaps

### 22 · Translations deep dive

- [ ] **[TRX-04](22-translations-deep-dive.md#trx-04--picker-search-misses-common-spellings)** Picker search misses common spellings
- [ ] **[TRX-05](22-translations-deep-dive.md#trx-05--the-chosen-translation-chips-hide-most-of-your-choices)** The chosen-translation chips hide most of your choices  
  _Done when:_ All selected translations are visible (or clearly counted and one tap away) at 390 px.
- [ ] **[TRX-06](22-translations-deep-dive.md#trx-06--the--on-the-main-translation-does-nothing-primary-is-a-mystery-button)** The × on the main translation does nothing; "Primary" is a mystery button
- [ ] **[TRX-09](22-translations-deep-dive.md#trx-09--switching-the-main-translation-mid-surah-loses-your-place)** Switching the main translation mid-surah loses your place  
  _Done when:_ Switching translation keeps the same verse at the top of the screen.
- [ ] **[TRX-10](22-translations-deep-dive.md#trx-10--the-serif-translation-font-is-forgotten-after-a-reload)** The serif translation font is forgotten after a reload  
  _Done when:_ Serif survives a reload and a new tab.
- [ ] **[TRX-11](22-translations-deep-dive.md#trx-11--verses-with-no-translation-text-look-broken)** Verses with no translation text look broken
- [ ] **[TRX-12](22-translations-deep-dive.md#trx-12--right-to-left-translations-are-set-like-english)** Right-to-left translations are set like English
- [ ] **[TRX-13](22-translations-deep-dive.md#trx-13--stacked-credits-break-right-to-left-lines)** Stacked credits break right-to-left lines

### 23 · Interaction details (the last 10% of polish)

- [ ] **[INT-02](23-interaction-details.md#int-02--the-highlight-on-a-linked-verse-is-nearly-invisible)** The highlight on a linked verse is nearly invisible
- [ ] **[INT-03](23-interaction-details.md#int-03--back-after-next-surah-returns-to-the-top-not-where-you-were)** Back after "Next surah" returns to the top, not where you were  
  _Done when:_ Next surah → Back returns to the same scroll position in all three test surahs.
- [ ] **[INT-04](23-interaction-details.md#int-04--reading-mode-removes-every-verse-action)** Reading mode removes every verse action

### 24 · Loading and perceived performance

- [ ] **[LOAD-04](24-loading-and-perceived-performance.md#load-04--the-ui-font-arrives-late-and-moves-the-page)** The UI font arrives late and moves the page  
  _Done when:_ Landing CLS < 0.02 on the throttled phone run.
- [ ] **[LOAD-05](24-loading-and-perceived-performance.md#load-05--67-seconds-of-white-screen-on-a-slow-first-visit)** 6–7 seconds of white screen on a slow first visit  
  _Done when:_ Reader first paint < 3 s and first-visit transfer < 250 KB on the throttled run.

### 25 · Installed app (PWA) and page metadata

- [ ] **[PWA-04](25-pwa-and-page-metadata.md#pwa-04--browserstatus-bar-colour-never-matches-the-chosen-theme)** Browser/status-bar colour never matches the chosen theme
- [ ] **[PWA-05](25-pwa-and-page-metadata.md#pwa-05--no-help-to-install-the-app)** No help to install the app
- [ ] **[PWA-08](25-pwa-and-page-metadata.md#pwa-08--the-update-notice-speaks-in-tabs-and-has-a-tiny-button)** The update notice speaks in "tabs" and has a tiny button
- [ ] **[PWA-09](25-pwa-and-page-metadata.md#pwa-09--link-previews-one-old-image-for-every-page)** Link previews: one old image for every page

### 26 · Cross-browser (Safari engine vs Chrome)

- [ ] **[BRW-02](26-cross-browser.md#brw-02--continuous-reading-mode-breaks-arabic-lines-differently-per-engine)** Continuous reading mode breaks Arabic lines differently per engine
- [ ] **[BRW-04](26-cross-browser.md#brw-04--firefox-is-untested--code-level-risk-list)** Firefox is untested — code-level risk list

## P3 — After release (36)

Nice to have.

### 03 · Reader (surah, page and juz readers)

- [ ] **[RDR-12](03-reader.md#rdr-12--continuous-scroll-changes-page-n-of-48-silently)** Continuous scroll changes "Page N of 48" silently

### 06 · Browse lists (Surahs, Juz, Pages)

- [ ] **[LIST-06](06-browse-lists.md#list-06--list-pages-have-no-title-and-no-filter-of-their-own)** List pages have no title and no filter of their own

### 07 · Bookmarks, notes and "Yours"

- [ ] **[BM-05](07-bookmarks-notes-yours.md#bm-05--stored-in-this-browser--sign-in-to-sync-is-vague)** "Stored in this browser" / "Sign in to sync" is vague

### 10 · Arabic UI and right-to-left

- [ ] **[RTL-07](10-arabic-and-rtl.md#rtl-07--small-bidi-and-wording-issues)** Small bidi and wording issues

### 11 · Visual consistency (design-system parity)

- [ ] **[VIS-09](11-visual-consistency.md#vis-09--browser-theme-colour-is-from-the-old-design)** Browser theme colour is from the old design
- [ ] **[VIS-10](11-visual-consistency.md#vis-10--legacy-tokens-and-ad-hoc-sizes-on-un-migrated-pages)** Legacy tokens and ad-hoc sizes on un-migrated pages

### 13 · Sign in, register and account

- [ ] **[AUTH-05](13-sign-in-and-account.md#auth-05--why-sign-in-the-value-isnt-stated-where-it-matters)** Why sign in? The value isn't stated where it matters

### 14 · Marketing site ↔ app parity

- [ ] **[MKT-04](14-marketing-site-parity.md#mkt-04--footer-typo-and-brand-spelling)** Footer typo and brand spelling

### 17 · Themes, palettes and Arabic scripts

- [ ] **[SCRIPT-06](17-themes-palettes-and-scripts.md#script-06--the-bismillah-doesnt-scale-with-the-arabic-text-size)** The Bismillah doesn't scale with the Arabic text size

### 18 · Display conditions: text size, high contrast, motion, screen sizes

- [ ] **[DISP-04](18-display-conditions.md#disp-04--the-menu-panel-still-slides-when-reduce-motion-is-on)** The menu panel still slides when "reduce motion" is on
- [ ] **[DISP-05](18-display-conditions.md#disp-05--no-response-to-increase-contrast)** No response to "increase contrast"

### 19 · Remaining screens and flows

- [ ] **[SCR-07](19-remaining-screens-and-flows.md#scr-07--special-quran-moments-arent-explained)** Special Qur'an moments aren't explained
- [ ] **[SCR-08](19-remaining-screens-and-flows.md#scr-08--the-reading-mode-dialog-uses-its-own-button-and-list-styles)** The reading-mode dialog uses its own button and list styles

### 20 · Signed-in experience

- [ ] **[ACCT-05](20-signed-in-experience.md#acct-05--sync-status-is-good-but-small)** Sync status is good but small

### 21 · Keyboard, focus and screen reader

- [ ] **[KEY-07](21-keyboard-focus-and-screen-reader.md#key-07--focus-wanders-behind-the-floating-appearance-panel)** Focus wanders behind the floating appearance panel
- [ ] **[KEY-08](21-keyboard-focus-and-screen-reader.md#key-08--overlays-open-on-an-unhelpful-first-control)** Overlays open on an unhelpful first control
- [ ] **[KEY-13](21-keyboard-focus-and-screen-reader.md#key-13--browser-tab-titles-follow-five-different-patterns)** Browser tab titles follow five different patterns
- [ ] **[KEY-14](21-keyboard-focus-and-screen-reader.md#key-14--the-verse-note-button-doesnt-say-it-opens-a-panel)** The verse note button doesn't say it opens a panel
- [ ] **[KEY-15](21-keyboard-focus-and-screen-reader.md#key-15--sign-in-errors-well-built-two-gaps)** Sign-in errors: well built, two gaps

### 22 · Translations deep dive

- [ ] **[TRX-07](22-translations-deep-dive.md#trx-07--at-the-5-translation-cap-disabled-boxes-look-enabled-and-the-message-is-jargon)** At the 5-translation cap, disabled boxes look enabled and the message is jargon
- [ ] **[TRX-08](22-translations-deep-dive.md#trx-08--changes-apply-instantly-done-and--are-the-same-thing)** Changes apply instantly; "Done" and ✕ are the same thing
- [ ] **[TRX-14](22-translations-deep-dive.md#trx-14--five-stacked-translations-on-a-phone-one-verse-per-screen)** Five stacked translations on a phone: one verse per screen
- [ ] **[TRX-15](22-translations-deep-dive.md#trx-15--reading-mode-choice-dialog-clear-with-small-inconsistencies)** Reading-mode choice dialog: clear, with small inconsistencies
- [ ] **[TRX-16](22-translations-deep-dive.md#trx-16--the-row-hover-card-shows-technical-metadata)** The row hover card shows technical metadata

### 23 · Interaction details (the last 10% of polish)

- [ ] **[INT-05](23-interaction-details.md#int-05--what-copy-puts-on-the-clipboard)** What "Copy" puts on the clipboard
- [ ] **[INT-06](23-interaction-details.md#int-06--pressed-feedback-exists-on-only-a-few-buttons-buttons-show-an-arrow-cursor)** Pressed feedback exists on only a few buttons; buttons show an arrow cursor
- [ ] **[INT-07](23-interaction-details.md#int-07--four-different-backdrops-behind-overlays)** Four different backdrops behind overlays
- [ ] **[INT-08](23-interaction-details.md#int-08--the-floating-button-stays-on-top-of-open-dialogs)** The floating button stays on top of open dialogs
- [ ] **[INT-09](23-interaction-details.md#int-09--new-verses-appear-before-their-tools-no-loading-more-cue)** New verses appear before their tools; no "loading more" cue
- [ ] **[INT-10](23-interaction-details.md#int-10--loading-indicators-come-in-four-styles)** Loading indicators come in four styles
- [ ] **[INT-11](23-interaction-details.md#int-11--finishing-the-quran-has-no-ending)** Finishing the Qur'an has no ending

### 24 · Loading and perceived performance

- [ ] **[LOAD-06](24-loading-and-perceived-performance.md#load-06--ayah-markers-change-shape-during-loading)** Ayah markers change shape during loading
- [ ] **[LOAD-07](24-loading-and-perceived-performance.md#load-07--without-javascript-the-app-is-always-dark)** Without JavaScript the app is always dark

### 25 · Installed app (PWA) and page metadata

- [ ] **[PWA-07](25-pwa-and-page-metadata.md#pwa-07--safe-area-padding-is-dead-code-no-viewport-fit)** Safe-area padding is dead code; no `viewport-fit`
- [ ] **[PWA-10](25-pwa-and-page-metadata.md#pwa-10--personal-pages-are-indexable-searchsettings-lack-robots-rules)** Personal pages are indexable; search/settings lack robots rules

### 26 · Cross-browser (Safari engine vs Chrome)

- [ ] **[BRW-03](26-cross-browser.md#brw-03--viewport-height-and-tap-details-for-ios)** Viewport-height and tap details for iOS

## Before you tag the release

- [ ] Re-run the capture harness ([tools/](tools/README.md)) on the key routes and compare with the screenshots in this folder.
- [ ] axe: 0 serious/critical on the 10 key routes × every palette × light/dark.
- [ ] Manual pass on a real iPhone (Safari) and a real Android phone (Chrome): read a surah, add a translation, bookmark, search, switch to Arabic, go offline.
- [ ] 20-minute VoiceOver or TalkBack pass on home → reader → bookmark → settings.
- [ ] 5-person task test from [16 · Fix roadmap](16-fix-roadmap.md#validate-with-real-people).
- [ ] Every English string has an Arabic twin; Arabic UI walked end-to-end after a hard refresh on every page.
- [ ] `pnpm check`, `pnpm lint`, `pnpm test` green.
