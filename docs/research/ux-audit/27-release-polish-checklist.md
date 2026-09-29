# 27 · Release polish checklist

[← Back to the index](README.md)

One checklist for the pre-release UI polish, generated from every finding in this folder. Tick items as they land; each links to the full write-up (screenshot, cause, fix, file). Cross-reference entries (findings that point at another finding) are folded into the item they point to.

**Totals (distinct items):** 7 P0 · 33 P1 · 45 P2 · 8 P3.

**Release gate suggestion:** all P0 and P1 ticked, P2 ≥ 80 % ticked, and the [guard tests](16-fix-roadmap.md#guardrails-to-add-so-it-stays-fixed) green in CI.

Order inside each group follows the [roadmap](16-fix-roadmap.md): shared components first, then call sites.

## P0 — Must fix before release (7)

Trust or core-task blockers.

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

## P1 — Should fix before release (33)

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

## P2 — Polish pass (45)

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

## P3 — After release (8)

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

## Before you tag the release

- [ ] Re-run the capture harness ([tools/](tools/README.md)) on the key routes and compare with the screenshots in this folder.
- [ ] axe: 0 serious/critical on the 10 key routes × every palette × light/dark.
- [ ] Manual pass on a real iPhone (Safari) and a real Android phone (Chrome): read a surah, add a translation, bookmark, search, switch to Arabic, go offline.
- [ ] 20-minute VoiceOver or TalkBack pass on home → reader → bookmark → settings.
- [ ] 5-person task test from [16 · Fix roadmap](16-fix-roadmap.md#validate-with-real-people).
- [ ] Every English string has an Arabic twin; Arabic UI walked end-to-end after a hard refresh on every page.
- [ ] `pnpm check`, `pnpm lint`, `pnpm test` green.
