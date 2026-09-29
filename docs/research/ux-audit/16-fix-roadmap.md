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

## Validate with real people

After Wave 1–2, run 5 short sessions (20 min, remote is fine) with the real audience — ideally two older readers, one Arabic-first reader, one who can't read Arabic, one phone-only user. Tasks:

1. "Open Surah Al-Kahf and read verse 10 with an English translation."
2. "Save this verse so you can find it tomorrow, then find it."
3. "Make the Arabic text bigger."
4. "Find verses about patience."
5. "Switch the app to Arabic, close it, open it again."

Success = each task in under a minute without help. Note every hesitation; that list is the next audit.
