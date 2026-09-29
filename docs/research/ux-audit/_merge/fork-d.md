# Merge notes — Fork D (loading & perceived performance, PWA & page metadata, cross-browser)

New docs: [24-loading-and-perceived-performance.md](../24-loading-and-perceived-performance.md), [25-pwa-and-page-metadata.md](../25-pwa-and-page-metadata.md), [26-cross-browser.md](../26-cross-browser.md).
New screenshot folders: `screenshots/loading/` (9), `screenshots/pwa/` (1), `screenshots/browsers/` (3) — ≈ 0.65 MB total, all referenced. Links and anchors in 24–26 validated (0 broken).

## 1 · New issues (21: 1 P0 · 7 P1 · 8 P2 · 5 P3)

| ID | Title | Sev | File | Anchor |
| --- | --- | --- | --- | --- |
| LOAD-01 | The reader shows text for ~25 s before its controls exist, then the page jumps | P1 | 24-loading-and-perceived-performance.md | `#load-01--the-reader-shows-text-for-25-s-before-its-controls-exist-then-the-page-jumps` |
| LOAD-02 | "Preparing offline Quran" sits on top of the header for the whole download | P1 | 24-loading-and-perceived-performance.md | `#load-02--preparing-offline-quran-sits-on-top-of-the-header-for-the-whole-download` |
| LOAD-03 | A shared search link ignores its query until the app wakes up | P1 | 24-loading-and-perceived-performance.md | `#load-03--a-shared-search-link-ignores-its-query-until-the-app-wakes-up` |
| LOAD-04 | The UI font arrives late and moves the page | P2 | 24-loading-and-perceived-performance.md | `#load-04--the-ui-font-arrives-late-and-moves-the-page` |
| LOAD-05 | 6–7 seconds of white screen on a slow first visit | P2 | 24-loading-and-perceived-performance.md | `#load-05--67-seconds-of-white-screen-on-a-slow-first-visit` |
| LOAD-06 | Ayah markers change shape during loading | P3 | 24-loading-and-perceived-performance.md | `#load-06--ayah-markers-change-shape-during-loading` |
| LOAD-07 | Without JavaScript the app is always dark | P3 | 24-loading-and-perceived-performance.md | `#load-07--without-javascript-the-app-is-always-dark` |
| PWA-01 | The home-screen icon and favicons are broken, off-brand artwork | P0 | 25-pwa-and-page-metadata.md | `#pwa-01--the-home-screen-icon-and-favicons-are-broken-off-brand-artwork` |
| PWA-02 | Manifest: installed app always opens in English, old colours, brand spelling | P1 | 25-pwa-and-page-metadata.md | `#pwa-02--manifest-installed-app-always-opens-in-english-old-colours-brand-spelling` |
| PWA-03 | Tab titles follow five patterns; several pages have no title | P1 | 25-pwa-and-page-metadata.md | `#pwa-03--tab-titles-follow-five-patterns-several-pages-have-no-title` |
| PWA-04 | Browser/status-bar colour never matches the chosen theme | P2 | 25-pwa-and-page-metadata.md | `#pwa-04--browserstatus-bar-colour-never-matches-the-chosen-theme` |
| PWA-05 | No help to install the app | P2 | 25-pwa-and-page-metadata.md | `#pwa-05--no-help-to-install-the-app` |
| PWA-06 | In the installed app, some screens have no way back | P1 | 25-pwa-and-page-metadata.md | `#pwa-06--in-the-installed-app-some-screens-have-no-way-back` |
| PWA-07 | Safe-area padding is dead code; no `viewport-fit` | P3 | 25-pwa-and-page-metadata.md | `#pwa-07--safe-area-padding-is-dead-code-no-viewport-fit` |
| PWA-08 | The update notice speaks in "tabs" and has a tiny button | P2 | 25-pwa-and-page-metadata.md | `#pwa-08--the-update-notice-speaks-in-tabs-and-has-a-tiny-button` |
| PWA-09 | Link previews: one old image for every page | P2 | 25-pwa-and-page-metadata.md | `#pwa-09--link-previews-one-old-image-for-every-page` |
| PWA-10 | Personal pages are indexable; search/settings lack robots rules | P3 | 25-pwa-and-page-metadata.md | `#pwa-10--personal-pages-are-indexable-searchsettings-lack-robots-rules` |
| BRW-01 | Every text field is under 16 px — iPhones zoom in on tap | P1 | 26-cross-browser.md | `#brw-01--every-text-field-is-under-16-px--iphones-zoom-in-on-tap` |
| BRW-02 | Continuous reading mode breaks Arabic lines differently per engine | P2 | 26-cross-browser.md | `#brw-02--continuous-reading-mode-breaks-arabic-lines-differently-per-engine` |
| BRW-03 | Viewport-height and tap details for iOS | P3 | 26-cross-browser.md | `#brw-03--viewport-height-and-tap-details-for-ios` |
| BRW-04 | Firefox is untested — code-level risk list | P2 (risk, not a confirmed bug) | 26-cross-browser.md | `#brw-04--firefox-is-untested--code-level-risk-list` |

## 2 · Extensions / corrections to existing findings

| Existing | Note |
| --- | --- |
| [RDR-01](../03-reader.md#rdr-01--the-floating-appearance-button-covers-the-quran-text-on-phones) / [SET-08](../08-settings-and-appearance.md#set-08--a-second-settings-floats-over-every-page) | **Dependency:** `OfflinePackBar` (offline-pack progress) and `Notifications` are mounted *inside* `Tweaks.svelte` (lines 331, 376). Removing the floating panel also removes the offline-pack progress UI — move `OfflinePackBar` to the root layout first. |
| [VIS-09](../11-visual-consistency.md#vis-09--browser-theme-colour-is-from-the-old-design) | Extended by PWA-04 (no runtime `theme-color` update) and PWA-02 (manifest `theme_color`/`background_color` `#0c0d0c`). |
| [HOME-01](../02-home.md#home-01--once-you-have-read-anything-the-browse-shortcuts-disappear) | Manifest shortcut "Continue reading" → `/app` → home; it does not resume (PWA-02). |
| [NAV-02](../01-navigation-and-wayfinding.md#nav-02--two-url-schemes-some-links-lose-the-language-some-return-a-bare-not-found) / [RTL-04](../10-arabic-and-rtl.md#rtl-04--settings-search-bookmarks-yours-english-body-and-english-after-refresh) | The manifest `start_url: /app` is a prerendered meta-refresh that always goes to `/en/app`, so the **installed** app is always English (PWA-02). |
| [AUTH-01](../13-sign-in-and-account.md#auth-01--no-way-back-from-sign-in--create-account), [STATE-01](../12-states-errors-offline.md#state-01--wrong-reader-urls-return-a-plain-text-not-found), [STATE-03](../12-states-errors-offline.md#state-03--account-page-spins-forever-when-the-api-is-unreachable) | Worse in the installed app (no browser toolbar) — PWA-06. |
| [A11Y-05](../09-accessibility.md#a11y-05--duplicate-and-nested-landmarks-missing-page-titles) | Login/Register missing `<title>` also applies to **/forgot-password** and **/verify-email**; the app home's title is just "Home"/"الرئيسية" (PWA-03). |
| [RDR-06](../03-reader.md#rdr-06--reading-mode-on-phones-spreads-words-far-apart) | Worst on Chromium/Android: Chromium fits fewer words per line than WebKit (BRW-02). |
| [TR-06](../04-translations.md#tr-06--when-the-api-is-unreachable-translated-pages-fail-without-falling-back-to-arabic) / [12 · States](../12-states-errors-offline.md) | Positive counter-evidence for the "What's good" section: after one online visit, a never-opened surah, juz, mushaf page, search for a new word, the Arabic-UI reader, and even two never-opened translations (Sahih, Pickthall — apparently prefetched) all opened **offline** on the production build. |
| [MKT-04](../14-marketing-site-parity.md#mkt-04--footer-typo-and-brand-spelling) / [15 · Plain language](../15-plain-language-copy.md) | Brand spelled 4 ways in metadata too: manifest "EasyQuran", titles "EasyQuran"/"easyquran", Arabic landing "إيزي قرآن"; manifest/OG say "Quran" (PWA-02, PWA-03, PWA-09). |
| Fork A SCRIPT-04 (script/font flash) | Related: LOAD-06 (ayah-marker glyphs re-draw at ~29 s on Slow 3G) and LOAD-04 (Nunito not preloaded). Fix together as "font loading" work. |
| README "115 translations" | Coordinator note: the catalogue is **378 translations across 105 languages** — README/04 wording needs correcting at merge (not edited by this fork). |

Out-of-scope observations for other areas (not written up here): innerText on every app page contains **two** "Skip to content" links (root + app layout; both sr-only) — check with keyboard pass; the sidebar trigger's accessible name "Toggle Sidebar" is untranslated in the Arabic UI.

## 3 · Roadmap additions

| Wave | ID | Fix | Effort |
| --- | --- | --- | --- |
| 1 (P0) | PWA-01 | Re-export all icons/favicons/logo from the current ق mark; separate maskable icons | S |
| 2 (P1) | LOAD-01 | Render reader controls in SSR (behaviour after hydration); reserve space; CSS-driven theme icon | M |
| 2 (P1) | LOAD-02 | Move download progress under the header / to a bottom toast; localize; silent background downloads | S |
| 2 (P1) | LOAD-03 | SSR the `?q=` value; "Searching…" status with progress; results `min-height` | S |
| 2 (P1) | PWA-02 | Locale-aware `start_url`, resume shortcut, v2 colours, Arabic manifest | S |
| 2 (P1) | PWA-03 | One title pattern for all routes + test | S |
| 2 (P1) | PWA-06 | Visible exits on auth/account/404 screens; standalone back affordance | S |
| 2 (P1) | BRW-01 | ≥ 16 px inputs on touch devices | S |
| 3 (P2) | LOAD-04, LOAD-05 | Preload Nunito + metric-matched fallback; trim eager JS chunks; compress SSR | M |
| 3 (P2) | PWA-04, PWA-05, PWA-08, PWA-09 | Runtime theme-color; install card; plain update toast; new/per-surah OG images | M |
| 3 (P2) | BRW-02, BRW-04 | No justification on narrow screens; real Firefox pass + Share label | S |
| 4 (P3) | LOAD-06, LOAD-07, PWA-07, PWA-10, BRW-03 | Marker font preload; no-JS OS theme; `viewport-fit=cover` + safe areas; robots; `dvh` + tap highlight | S |

Guardrails to add: a Lighthouse/CDP CI run on the production build (Slow 4G phone profile) failing on CLS > 0.1 for `/`, `/en/app`, `/en/app/al-baqarah`, `/app/search?q=…`; a route test that every page has a non-empty title ending in the brand; a CSS test that `input`/`textarea` compute ≥ 16 px at 390 px width.

## 4 · Coverage

| Checked | Could not / not done |
| --- | --- |
| Production build (`PUBLIC_ENV=local pnpm build`, `bun server.ts` :5392) — build succeeded, **no tracked files changed** (only `docs/remaining/*` already modified by someone else, untouched) | CDN/proxy compression and caching of the real production host |
| 10 throttled runs (Slow 3G + 4× CPU phone; Fast 3G desktop), cold and warm, with CLS/LCP/FCP/hydration marks and filmstrips; no-JS render; dark-mode first paint | Real devices; Lighthouse scores |
| Offline after one visit: 11 never-opened routes | Brand-new visitor fully offline (browser error page — nothing the app can do) |
| Manifest, icons, favicons, apple-touch, OG image, titles/descriptions/robots/canonical on 33 routes, SW update toast (code + copy), install prompt, standalone, safe areas | Real install on Android/iOS; the update toast was reviewed in code, not triggered |
| WebKit 26.6 (iPhone 15 profile + desktop) vs Chromium 152: 60 screenshots, input sizes, sticky/fixed, fonts | **Firefox** — Playwright Firefox 155 fails to launch on this macOS ("Could not find profile folder"); code-level risk list instead. Headless WebKit doesn't paint `backdrop-filter`, so blur couldn't be compared |
