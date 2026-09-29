# Merge notes — Fork C (keyboard/focus/screen reader · translations deep dive · interaction details)

Docs added: [21 · Keyboard, focus and screen reader](../21-keyboard-focus-and-screen-reader.md), [22 · Translations deep dive](../22-translations-deep-dive.md), [23 · Interaction details](../23-interaction-details.md).
Screenshots added: `screenshots/keyboard/` (4), `screenshots/translations-deep/` (12), `screenshots/interactions/` (7).

**Totals: 42 new findings — 1 P0 · 5 P1 · 19 P2 · 17 P3** (KEY 15 · TRX 16 · INT 11).

## 1 · New issues (for the README register)

| ID | Title | Severity | File | Anchor |
| --- | --- | :-: | --- | --- |
| [KEY-01](../21-keyboard-focus-and-screen-reader.md#key-01--screen-readers-are-interrupted-with-page-n-of-48-while-you-scroll) | Screen readers are interrupted with "Page N of 48" while you scroll | P1 | `21-keyboard-focus-and-screen-reader.md` | `#key-01--screen-readers-are-interrupted-with-page-n-of-48-while-you-scroll` |
| [KEY-02](../21-keyboard-focus-and-screen-reader.md#key-02--two-skip-to-content-links--and-in-arabic-the-first-one-is-english) | Two "Skip to content" links — and in Arabic the first one is English | P2 | `21-keyboard-focus-and-screen-reader.md` | `#key-02--two-skip-to-content-links--and-in-arabic-the-first-one-is-english` |
| [KEY-03](../21-keyboard-focus-and-screen-reader.md#key-03--every-verse-adds-four-tab-stops-with-identical-names) | Every verse adds four Tab stops with identical names | P1 | `21-keyboard-focus-and-screen-reader.md` | `#key-03--every-verse-adds-four-tab-stops-with-identical-names` |
| [KEY-04](../21-keyboard-focus-and-screen-reader.md#key-04--the-focus-ring-vanishes-on-the-blue-surahs-card) | The focus ring vanishes on the blue "Surahs" card | P2 | `21-keyboard-focus-and-screen-reader.md` | `#key-04--the-focus-ring-vanishes-on-the-blue-surahs-card` |
| [KEY-05](../21-keyboard-focus-and-screen-reader.md#key-05--the-search-palette-drops-focus-when-it-closes) | The search palette drops focus when it closes | P2 | `21-keyboard-focus-and-screen-reader.md` | `#key-05--the-search-palette-drops-focus-when-it-closes` |
| [KEY-06](../21-keyboard-focus-and-screen-reader.md#key-06--three-dialogs-leave-the-page-behind-them-readable) | Three dialogs leave the page behind them readable | P2 | `21-keyboard-focus-and-screen-reader.md` | `#key-06--three-dialogs-leave-the-page-behind-them-readable` |
| [KEY-07](../21-keyboard-focus-and-screen-reader.md#key-07--focus-wanders-behind-the-floating-appearance-panel) | Focus wanders behind the floating appearance panel | P3 | `21-keyboard-focus-and-screen-reader.md` | `#key-07--focus-wanders-behind-the-floating-appearance-panel` |
| [KEY-08](../21-keyboard-focus-and-screen-reader.md#key-08--overlays-open-on-an-unhelpful-first-control) | Overlays open on an unhelpful first control | P3 | `21-keyboard-focus-and-screen-reader.md` | `#key-08--overlays-open-on-an-unhelpful-first-control` |
| [KEY-09](../21-keyboard-focus-and-screen-reader.md#key-09--keyboard-shortcuts-are-hidden-and-some-are-single-keys-that-cant-be-turned-off) | Keyboard shortcuts are hidden, and some are single keys that can't be turned off | P2 | `21-keyboard-focus-and-screen-reader.md` | `#key-09--keyboard-shortcuts-are-hidden-and-some-are-single-keys-that-cant-be-turned-off` |
| [KEY-10](../21-keyboard-focus-and-screen-reader.md#key-10--vague-or-duplicated-control-names) | Vague or duplicated control names | P2 | `21-keyboard-focus-and-screen-reader.md` | `#key-10--vague-or-duplicated-control-names` |
| [KEY-11](../21-keyboard-focus-and-screen-reader.md#key-11--english-accessible-names-in-the-arabic-ui) | English accessible names in the Arabic UI | P2 | `21-keyboard-focus-and-screen-reader.md` | `#key-11--english-accessible-names-in-the-arabic-ui` |
| [KEY-12](../21-keyboard-focus-and-screen-reader.md#key-12--heading-outline-gaps) | Heading outline gaps | P2 | `21-keyboard-focus-and-screen-reader.md` | `#key-12--heading-outline-gaps` |
| [KEY-13](../21-keyboard-focus-and-screen-reader.md#key-13--browser-tab-titles-follow-five-different-patterns) | Browser tab titles follow five different patterns | P3 | `21-keyboard-focus-and-screen-reader.md` | `#key-13--browser-tab-titles-follow-five-different-patterns` |
| [KEY-14](../21-keyboard-focus-and-screen-reader.md#key-14--the-verse-note-button-doesnt-say-it-opens-a-panel) | The verse note button doesn't say it opens a panel | P3 | `21-keyboard-focus-and-screen-reader.md` | `#key-14--the-verse-note-button-doesnt-say-it-opens-a-panel` |
| [KEY-15](../21-keyboard-focus-and-screen-reader.md#key-15--sign-in-errors-well-built-two-gaps) | Sign-in errors: well built, two gaps | P3 | `21-keyboard-focus-and-screen-reader.md` | `#key-15--sign-in-errors-well-built-two-gaps` |
| [TRX-01](../22-translations-deep-dive.md#trx-01--arabic-commentary-chosen-as-the-main-translation-looks-like-quran-text) | Arabic commentary chosen as the main translation looks like Qur'an text | P0 | `22-translations-deep-dive.md` | `#trx-01--arabic-commentary-chosen-as-the-main-translation-looks-like-quran-text` |
| [TRX-02](../22-translations-deep-dive.md#trx-02--the-transliteration-shows-raw-html-tags) | The transliteration shows raw HTML tags | P1 | `22-translations-deep-dive.md` | `#trx-02--the-transliteration-shows-raw-html-tags` |
| [TRX-03](../22-translations-deep-dive.md#trx-03--the-same-translator-is-listed-twice-and-some-languages-twice) | The same translator is listed twice, and some languages twice | P1 | `22-translations-deep-dive.md` | `#trx-03--the-same-translator-is-listed-twice-and-some-languages-twice` |
| [TRX-04](../22-translations-deep-dive.md#trx-04--picker-search-misses-common-spellings) | Picker search misses common spellings | P2 | `22-translations-deep-dive.md` | `#trx-04--picker-search-misses-common-spellings` |
| [TRX-05](../22-translations-deep-dive.md#trx-05--the-chosen-translation-chips-hide-most-of-your-choices) | The chosen-translation chips hide most of your choices | P2 | `22-translations-deep-dive.md` | `#trx-05--the-chosen-translation-chips-hide-most-of-your-choices` |
| [TRX-06](../22-translations-deep-dive.md#trx-06--the--on-the-main-translation-does-nothing-primary-is-a-mystery-button) | The × on the main translation does nothing; "Primary" is a mystery button | P2 | `22-translations-deep-dive.md` | `#trx-06--the--on-the-main-translation-does-nothing-primary-is-a-mystery-button` |
| [TRX-07](../22-translations-deep-dive.md#trx-07--at-the-5-translation-cap-disabled-boxes-look-enabled-and-the-message-is-jargon) | At the 5-translation cap, disabled boxes look enabled and the message is jargon | P3 | `22-translations-deep-dive.md` | `#trx-07--at-the-5-translation-cap-disabled-boxes-look-enabled-and-the-message-is-jargon` |
| [TRX-08](../22-translations-deep-dive.md#trx-08--changes-apply-instantly-done-and--are-the-same-thing) | Changes apply instantly; "Done" and ✕ are the same thing | P3 | `22-translations-deep-dive.md` | `#trx-08--changes-apply-instantly-done-and--are-the-same-thing` |
| [TRX-09](../22-translations-deep-dive.md#trx-09--switching-the-main-translation-mid-surah-loses-your-place) | Switching the main translation mid-surah loses your place | P2 | `22-translations-deep-dive.md` | `#trx-09--switching-the-main-translation-mid-surah-loses-your-place` |
| [TRX-10](../22-translations-deep-dive.md#trx-10--the-serif-translation-font-is-forgotten-after-a-reload) | The serif translation font is forgotten after a reload | P2 | `22-translations-deep-dive.md` | `#trx-10--the-serif-translation-font-is-forgotten-after-a-reload` |
| [TRX-11](../22-translations-deep-dive.md#trx-11--verses-with-no-translation-text-look-broken) | Verses with no translation text look broken | P2 | `22-translations-deep-dive.md` | `#trx-11--verses-with-no-translation-text-look-broken` |
| [TRX-12](../22-translations-deep-dive.md#trx-12--right-to-left-translations-are-set-like-english) | Right-to-left translations are set like English | P2 | `22-translations-deep-dive.md` | `#trx-12--right-to-left-translations-are-set-like-english` |
| [TRX-13](../22-translations-deep-dive.md#trx-13--stacked-credits-break-right-to-left-lines) | Stacked credits break right-to-left lines | P2 | `22-translations-deep-dive.md` | `#trx-13--stacked-credits-break-right-to-left-lines` |
| [TRX-14](../22-translations-deep-dive.md#trx-14--five-stacked-translations-on-a-phone-one-verse-per-screen) | Five stacked translations on a phone: one verse per screen | P3 | `22-translations-deep-dive.md` | `#trx-14--five-stacked-translations-on-a-phone-one-verse-per-screen` |
| [TRX-15](../22-translations-deep-dive.md#trx-15--reading-mode-choice-dialog-clear-with-small-inconsistencies) | Reading-mode choice dialog: clear, with small inconsistencies | P3 | `22-translations-deep-dive.md` | `#trx-15--reading-mode-choice-dialog-clear-with-small-inconsistencies` |
| [TRX-16](../22-translations-deep-dive.md#trx-16--the-row-hover-card-shows-technical-metadata) | The row hover card shows technical metadata | P3 | `22-translations-deep-dive.md` | `#trx-16--the-row-hover-card-shows-technical-metadata` |
| [INT-01](../23-interaction-details.md#int-01--a-shared-link-to-a-long-verse-opens-in-the-wrong-place-on-phones) | A shared link to a long verse opens in the wrong place on phones | P1 | `23-interaction-details.md` | `#int-01--a-shared-link-to-a-long-verse-opens-in-the-wrong-place-on-phones` |
| [INT-02](../23-interaction-details.md#int-02--the-highlight-on-a-linked-verse-is-nearly-invisible) | The highlight on a linked verse is nearly invisible | P2 | `23-interaction-details.md` | `#int-02--the-highlight-on-a-linked-verse-is-nearly-invisible` |
| [INT-03](../23-interaction-details.md#int-03--back-after-next-surah-returns-to-the-top-not-where-you-were) | Back after "Next surah" returns to the top, not where you were | P2 | `23-interaction-details.md` | `#int-03--back-after-next-surah-returns-to-the-top-not-where-you-were` |
| [INT-04](../23-interaction-details.md#int-04--reading-mode-removes-every-verse-action) | Reading mode removes every verse action | P2 | `23-interaction-details.md` | `#int-04--reading-mode-removes-every-verse-action` |
| [INT-05](../23-interaction-details.md#int-05--what-copy-puts-on-the-clipboard) | What "Copy" puts on the clipboard | P3 | `23-interaction-details.md` | `#int-05--what-copy-puts-on-the-clipboard` |
| [INT-06](../23-interaction-details.md#int-06--pressed-feedback-exists-on-only-a-few-buttons-buttons-show-an-arrow-cursor) | Pressed feedback exists on only a few buttons; buttons show an arrow cursor | P3 | `23-interaction-details.md` | `#int-06--pressed-feedback-exists-on-only-a-few-buttons-buttons-show-an-arrow-cursor` |
| [INT-07](../23-interaction-details.md#int-07--four-different-backdrops-behind-overlays) | Four different backdrops behind overlays | P3 | `23-interaction-details.md` | `#int-07--four-different-backdrops-behind-overlays` |
| [INT-08](../23-interaction-details.md#int-08--the-floating-button-stays-on-top-of-open-dialogs) | The floating button stays on top of open dialogs | P3 | `23-interaction-details.md` | `#int-08--the-floating-button-stays-on-top-of-open-dialogs` |
| [INT-09](../23-interaction-details.md#int-09--new-verses-appear-before-their-tools-no-loading-more-cue) | New verses appear before their tools; no "loading more" cue | P3 | `23-interaction-details.md` | `#int-09--new-verses-appear-before-their-tools-no-loading-more-cue` |
| [INT-10](../23-interaction-details.md#int-10--loading-indicators-come-in-four-styles) | Loading indicators come in four styles | P3 | `23-interaction-details.md` | `#int-10--loading-indicators-come-in-four-styles` |
| [INT-11](../23-interaction-details.md#int-11--finishing-the-quran-has-no-ending) | Finishing the Qur'an has no ending | P3 | `23-interaction-details.md` | `#int-11--finishing-the-quran-has-no-ending` |

Suggested README document-table rows (paths here are relative to `_merge/`; drop the `../` when pasting into README):

| Doc | Covers | P0 | P1 | P2 | P3 |
| --- | --- | :-: | :-: | :-: | :-: |
| [21 · Keyboard, focus, screen reader](../21-keyboard-focus-and-screen-reader.md) | Tab order, overlays, announcer, shortcuts, names, headings, titles | | 2 | 8 | 5 |
| [22 · Translations deep dive](../22-translations-deep-dive.md) | Tafsir as primary, transliteration tags, duplicates, picker, RTL, empty verses, persistence | 1 | 2 | 8 | 5 |
| [23 · Interaction details](../23-interaction-details.md) | Deep links, highlight, Back, reading-mode actions, copy text, pressed states, scrims, loading, ending | | 1 | 3 | 7 |

P0 table candidate: **TRX-01 · Arabic commentary chosen as the main translation looks like Qur'an text** (religious-content trust; pairs with RDR-03 / TR-01).

## 2 · Extensions and corrections to existing findings

| Existing | Extension / correction |
| --- | --- |
| [TR-01](../04-translations.md#tr-01--picking-a-translation-removes-the-arabic-text-and-the-bismillah) | Worse with tafsir as the main text — TRX-01. Copy on translated routes also drops the Arabic (INT-05). |
| [TR-02](../04-translations.md#tr-02--the-main-translation-is-never-credited) | The stacked credit is inline Latin inside RTL text and splits lines at large sizes (TRX-13). |
| [TR-03](../04-translations.md#tr-03--the-translation-picker-opens-on-arabic-tafsir-uses-flags-and-says-primary) | Picker also lists duplicates (TRX-03), search misses "sahih" (TRX-04), chips hide selections (TRX-05), × on the main chip is dead (TRX-06); flags also in the reading-mode dialog (TRX-15). |
| [TR-05](../04-translations.md#tr-05--extra-language-lines-are-small-and-use-a-generic-font) | Also applies to RTL translations as the **main** text (TRX-12). |
| [RDR-02](../03-reader.md#rdr-02--verse-actions-are-tiny-unlabeled-and-ambiguous) | Keyboard cost: 4 Tab stops per verse, 1,144 in Al-Baqarah; names lack the verse number (KEY-03). Reading mode has no verse actions at all (INT-04). |
| [RDR-01](../03-reader.md#rdr-01--the-floating-appearance-button-covers-the-quran-text-on-phones) | The floating button also sits above modal backdrops (INT-08) and its panel lets focus wander behind it (KEY-07). |
| [RDR-04](../03-reader.md#rdr-04--the-surah-number-is-shown-four-times-the-header-colour-changes-per-surah) / [RDR-12](../03-reader.md#rdr-12--continuous-scroll-changes-page-n-of-48-silently) | The changing "Page N of 48" is also in `<title>` and is announced assertively on every page boundary (KEY-01). |
| [HOME-06](../02-home.md#home-06--no-page-title-h1) | Full per-page heading outline in KEY-12; range-reader surah groups are not headings. |
| [A11Y-05](../09-accessibility.md#a11y-05--duplicate-and-nested-landmarks-missing-page-titles) | Add the duplicate skip link (KEY-02) and inconsistent tab titles (KEY-13). |
| [A11Y-06](../09-accessibility.md#a11y-06--links-identified-by-colour-only-focus-ring-style-inconsistent) | Focus ring invisible on the blue home card (KEY-04). |
| [RTL-01](../10-arabic-and-rtl.md#rtl-01--app-home-hero-and-continue-card-are-english)–[RTL-03](../10-arabic-and-rtl.md#rtl-03--surah-list-metadata-is-english-in-arabic-ui) | Also true for accessible names, region labels and tab titles (KEY-11); copied text appends an English reference in Arabic UI (INT-05). |
| [VIS-04](../11-visual-consistency.md#vis-04--four-different-selected-looks) | Same pattern for backdrops (INT-07), pressed states (INT-06) and loading indicators (INT-10). |
| [STATE-06](../12-states-errors-offline.md#state-06--empty-states-are-inconsistent-and-sometimes-wrong) | Account / auth-success spinners still use v1 tokens `border-line-2 border-t-accent` (INT-10). |
| [SET-07](../08-settings-and-appearance.md#set-07--reading-settings-speak-in-pixels-and-font-file-names) | The translation-font choice doesn't survive a reload (TRX-10); root cause `applyPersisted` in `reader-persistence.svelte.ts`. |
| [AUTH-04](../13-sign-in-and-account.md#auth-04--small-colour-only-links-and-missing-page-titles) | Sign-in validates the 12-character *registration* rule and drops focus after a server error (KEY-15); otherwise error semantics are good. |
| Catalogue size (README, 00, 04) | The catalogue is **378 translations across 105 languages** — replace any "115 translations" wording when merging. |

## 3 · Roadmap additions

| Wave | IDs | Fix | Effort |
| --- | --- | --- | --- |
| 1 (P0) | TRX-01 | Tafsir never as the main text; label commentary; redirect `/t/ar/muyassar`, `/t/ar/jalalayn` | S |
| 2 (P1) | KEY-01 | Stable `<title>` while scrolling; no assertive announcements on scroll | S |
| 2 (P1) | KEY-03 | One Tab stop per verse (roving tabindex) + verse number in action names + skip-to-page-nav | M |
| 2 (P1) | TRX-02 | View-layer parser for transliteration `<u>`/`<b>` markup (data stays immutable) | S |
| 2 (P1) | TRX-03 | De-duplicate catalogue by (language, translator); merge split languages; keep aliases | M |
| 2 (P1) | INT-01 | Deep-link scroll after layout settles; `scroll-margin-top` from the real bar height | S |
| 3 (P2) | KEY-02, 04, 05, 06, 09, 10, 11, 12 | One localized skip link; card focus ring; palette focus return; `inert` background for all modals; shortcuts help + single-key toggle + Ctrl/⌘ B via `registerHotkey`; names/headings sweep | M |
| 3 (P2) | TRX-04, 05, 06, 09, 10, 11, 12, 13 | Picker aliases/fuzzy search; wrapping chip list; remove dead ×; keep verse on switch + readable slugs; persist `translationFamily`; empty-verse notice + fixed `dir`; RTL size/Nastaliq; credit on its own line | M |
| 3 (P2) | INT-02, 03, 04, 08 | Stronger linked-verse highlight; Back after Next surah restores; verse actions in reading mode; floating button under scrims (or removed) | S–M |
| 4 (P3) | KEY-07, 08, 13, 14, 15 · TRX-07, 08, 14, 15, 16 · INT-05, 06, 07, 09, 10, 11 | Polish sweep: first-focus targets, title pattern, `aria-expanded`, sign-in rule, cap UI, compare mode, copy format, pressable recipe + `cursor:pointer`, one scrim, one spinner/skeleton, end-of-Qur'an block | M |

Guard suggestions: announcer-mutation test while scrolling the reader (KEY-01); AX-tree test "no duplicate button names on a reader page" (KEY-03); catalogue test "no duplicate (language, translator)" (TRX-03); render test "no `<` in transliteration output" (TRX-02); persistence round-trip test for every reader setting (TRX-10).

## 4 · Coverage

**Checked.** Tab-order walks (home, surah/page/juz readers, surah/juz/pages lists, settings all tabs, search, yours, bookmarks, sign-in; English and Arabic). Every overlay (menu panel, floating panel, sidebar sheet, translations modal, ⌘K palette, reading-mode dialog, verse note panel) opened by click, keyboard and hotkey, with trap / Escape / focus-return checks at 1440 and 390 px. Full accessibility-tree dumps (names, duplicates, landmarks, headings, lang/dir) on 10 routes. Route-announcer behaviour during navigation and scrolling. All registered hotkeys incl. Arabic keyboard layouts (physical-key fallback). Sign-in error semantics. Translations: RTL main text (ur, fa) in English and Arabic UI; tafsir as main text; transliteration; stacking on surah, page and juz readers; five stacked on a phone; reading mode with extras (dialog); three of the four known empty verses (80:39, 108:3, 21:56); sizes 13 and 28 px; serif; the longest verse; switching the main translation mid-surah; picker search, cap, reorder, remove, clear, Done vs ✕. Interactions: hover/press on ~25 control types; overlay animation specs; the reduced-motion rule; header collapse/reveal (reader and lists, both widths); scroll restoration (list↔reader, Next surah, sidebar); deep links (short and long verses, both widths); clipboard contents in four contexts; throttled infinite scroll; the end of the Qur'an.

**Could not / not done.** Real screen readers (VoiceOver, TalkBack, NVDA) — findings come from the accessibility tree and live-region observation. The native share sheet's contents (headless Chrome exposes `navigator.share` but the sheet can't be observed). True long-press and text-selection gestures on a touch device (emulated only). Safari/Firefox focus and hotkey behaviour. `sq.mehdiu` 77:14 (same code path as 21:56). Frame-level jank of transitions (only durations read from source).
