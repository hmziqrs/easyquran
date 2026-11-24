# Native parity checklist

Native (NSH) counterpart of `web/`. Target: 1:1 feature parity. Web stays as is.

Legend: `[ ]` open, `[x]` implemented and exercised (shot in `fn-shots/`, headless run or test), `[~]` implemented, not exercised (why noted), `[n/a]` no native meaning (reason given), `[blocked]` reason given.
"Web" paths are relative to `web/src/`. "API" paths are backend routes (`rust/backend/api`). Every signed-in call uses the native session transport (`x-eq-session`, `contracts/devices.md` D6), never cookies.

Targets: macOS, iOS, Android (plus Linux/Windows desktop from the same crate). The NSH web target is not shipped: the SvelteKit app is the web client.

## A. Shell, navigation, chrome

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| A1 | App shell: sidebar on desktop, tabs on phone, reader header, sticky nav | `routes/(application)/+layout.svelte`, `_reader/Sidebar.svelte`, `SidebarVirtualList.svelte`, `IndexTabs.svelte`, `lib/components/nav/*`, `lib/stores/sticky-nav.svelte.ts` | local | full | [x] desktop sidebar (fn-shots) + phone tabs (tests/shell.rs) |
| A2 | Typed routes + deep links for the reader URL grammar (`/{surah}`, `/page/n`, `/juz/n`, `/hizb/n`, `/t/{lang}/{translator}/…`, `?ayah=`) | `lib/data/quran.ts` (`surah*For`), `lib/reader/*-param.ts`, `thread-path.ts`, `visible-url.ts`, `params.ts` | local | full | [x] route grammar tests + deep links opened (fn-shots); Universal Links / App Links (`links`) and Handoff (`handoff` + `set_app_url`) wired in `nsh.json` + shell |
| A3 | Landing / home with cards (continue, daily ayah, next zikr, custom cards) | `routes/(marketing)/+page.svelte`, `lib/home/LandingCards.svelte`, `CardTile.svelte`, `cards.svelte.ts`, `layout.svelte.ts`, `reading-choice.svelte.ts` | local + `preference/v1/sync` | full | [x] home landing, daily ayah, cards (headless run) |
| A4 | About, FAQ, contact, privacy, terms pages | `routes/(marketing)/{about,faq,contact,privacy,terms}`, `lib/i18n/*-copy.ts` | static copy | full | [x] About, FAQ, Contact, Privacy, Terms opened (tests/parity.rs) |
| A5 | Footer links, brand mark, bismillah | `lib/components/footer`, `brand/*`, `lib/assets/open-pages.svg` | static | full | [x] footer, brand, bismillah (fn-shots/not-found.png) |
| A6 | Error screen and not-found handling | `routes/+error.svelte` | — | full | [x] unknown app links, unknown translation/zikr open not-found (FN; not-found*.png) |
| A7 | Keyboard shortcuts (palette, reader nav) | `lib/hotkeys.svelte.ts`, consumers | local | full | [x] hotkeys (`/`, `Escape`, `mod+,`) driven headless (tests/parity.rs) |
| A8 | Notifications inbox / toasts surface | `lib/components/notifications`, `lib/stores/notifications.svelte.ts`, `components/ui/sonner` | local | full | [x] toast host + feedback bar exercised (tests/parity.rs) |

## B. Quran data

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| B1 | Immutable Quran DBs by id (no hash, read-only, never modified): Uthmani, annotated Uthmani, simple-clean, IndoPak, Tajweed | `lib/quran/view/source-profiles.ts`, `catalogue.ts`, `sql.ts`, `protocol.ts`, `wasm-query-runner.ts`, `workers/*` | artifact base `/_quran` (R2), baked id map | full once downloaded; default script bundled | [x] read-only DBs by id; Uthmani, IndoPak, Tajweed rendered (fn-shots) |
| B2 | Translation catalogue (378 sources) from baked map, download by id | `lib/data/translations.json`, `translations.ts`, `lib/quran/translation-groups.ts` | artifacts by id | per downloaded source | [x] catalogue in picker, sources read by id (translations-picker.png) |
| B3 | Surah names, mushaf divisions (juz, hizb, page), browse metadata | `lib/data/surah-names.*`, `mushaf-divisions.ts`, `quran-browse.ts`, `quran-data*.ts`, `static/quran-meta` | baked | full | [x] surah index, page/juz/hizb labels in reader header |
| B4 | Online range fetch when a source is not on device | `lib/quran/api-client.ts`, `range-fetch.ts`, `fetch.ts`, `wire.ts`, `source-plan.ts`, `api-availability.ts` | `quran/v1/*` | falls back to local | [~] no backend reachable here |
| B5 | Download manager: progress, resume, size check against baked map, discard partials | `lib/workers/download.ts`, `opfs-cache.ts`, `opfs-retention.ts`, `lib/offline/*` | app data dir | n/a | [~] shipped S1; no download run this slice |
| B6 | Text marks and canonical view rules (verbatim text, tajweed marks, IndoPak view) | `lib/quran/view/*`, `text-marks.ts` | local | full | [x] verbatim text, tajweed runs, IndoPak ending (script-*.png) |

## C. Reader

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| C1 | Surah index, juz index, pages index | `routes/(application)/surah`, `juz`, `pages`, `IndexTabs.svelte` | baked | full | [x] surah index tabs (headless home) |
| C2 | Surah reader (virtualized, long surahs) | `[surah]/+page.svelte`, `_reader/SurahReader.svelte`, `SurahPageRoute.svelte`, `ReaderVirtualList.svelte`, `virtual-reader.ts`, `page-heights.ts`, `viewport-anchor.ts` | B1 | full | [x] surah reader (open-surah.png) |
| C3 | Page, juz, hizb and range readers | `page/[n]`, `juz/[n]`, `hizb/[n]`, `_reader/RangeReader.svelte`, `range-validate.ts`, `ReaderPageNav.svelte` | B1 | full | [x] Page, Juz, Hizb readers opened headless (tests/parity.rs) |
| C4 | Arabic script switch (5 scripts) with fonts: Uthmanic Hafs, Uthman Taha Naskh, IndoPak compat v4, ayah ornament | `lib/config/reader-fonts.ts`, `lib/fonts/arabic-fonts.ts`, `ReadingTextPicker.svelte`, `IndoPakAyah.svelte`, `AyahOrnament.svelte`, `lib/stores/reading-text.svelte.ts`, `static/fonts` | prefs | full | [x] script switch via prefs: Uthmani, IndoPak, Tajweed (script-*.png) |
| C5 | Tajweed colouring | `lib/quran/view/tajweed.ts` | B1 | full | [x] tajweed colours (script-tajweed.png) |
| C6 | Translations: picker modal, recents, region suggestions, translated reader routes | `lib/components/translation/*`, `TranslationButton.svelte`, `[surah]/t/[lang]/[translator]`, `t/[lang]/[translator]/**`, `lib/reader/translation-nav.ts`, `stores/translation-recents.svelte.ts` | B2, B4 | per source | [x] picker + translated route /t/en/hilali (translations-picker.png, translation-route.png) |
| C7 | Stacked translations (lanes) | `_reader/StackedLanes.svelte`, `stacked-translations.svelte.ts`, `StackedLaneSettings.svelte`, `hue-slot.ts` | prefs | per source | [x] two stacked lanes, RTL Urdu lane (open-surah.png) |
| C8 | Reader modes and typography settings (mode toggle, sizes, arabic companion) | `ReaderModeToggle.svelte`, `arabic-companion.svelte.ts`, `typography-change.ts`, `stores/reader-settings.svelte.ts`, `reader-presentation.ts` | prefs | full | [x] Ayahs/Reading mode toggle and A+/A− switched headless (tests/parity.rs) |
| C9 | Verse tools: copy, share (text + verse card image, range), report verse, bookmark, note, pin | `VerseTools.svelte`, `VerseRow.svelte`, `ShareDialog.svelte`, `lib/share/*`, `ReportVerseDialog.svelte`, `PinsDrawer.svelte`, `stores/pins.svelte.ts`, `reader-share.svelte.ts` | local, `report/v1/submit` | share/copy full | [x] copy/share (text + branded PNG card via `eq_core::share`)/report/bookmark/note/pin + reading place (tests/parity.rs, `eq-core::share` test) |
| C10 | Word-level view (word tokens, gate, registry) | `VerseWords.svelte`, `lib/quran/words/*` | annotated source | full when source local | [~] word view gate shipped; annotated source not opened |
| C11 | Position indicator, resume pill, reader history, status banner / degradation | `PositionIndicator.svelte`, `ResumePill.svelte`, `reader-history.ts`, `ReaderStatusBanner.svelte`, `reader-degradation.svelte.ts`, `lib/reader/resume.ts`, `reading-flow.ts` | local | full | [~] resume pill needs a moved cursor (scroll), not driven headless; history via Yours [x] |
| C12 | Read tracking heuristics (no penalty for fast readers) | `lib/analytics/read-tracker.ts`, `lib/quran/track-view.svelte.ts`, `engagement*.ts` | feeds F/G | queued | [x] tracker counts reads (Yours continue row after run); events now carry session/source id (FN) |

## D. Audio

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| D1 | Recitation catalogue, URL building, play policy | `lib/audio/catalogue.ts`, `url.ts`, `policy.ts` | audio CDN | — | [~] web catalogue is empty, so the dock is hidden on both clients |
| D2 | Player engine, queue, repeat | `lib/audio/engine.svelte.ts`, `queue.ts`, `repeat.ts` | NSH audio | — | [~] same: empty catalogue; dev template hook only |
| D3 | Audio dock UI and in-reader controls | `lib/components/audio/*`, `lib/i18n/audio-copy.ts` | — | — | [~] dock hidden (empty catalogue); download bar now stacks above it (FN); lock-screen/background audio [blocked] |
| D4 | Offline audio | `lib/audio/offline.ts` | app data dir | full when saved | [~] behind the same catalogue gate |

## E. Search and command palette

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| E1 | Search page: Arabic + translation text, normalisation, translit, ranking, highlights | `routes/(application)/search/**`, `lib/quran/search/*`, `lib/quran/search.ts`, `lib/search/page/*` | local DBs | full | [x] Arabic + translation search with highlights (search.png) |
| E2 | Go-to-verse / nav parsing, surah suggestions, juz nicknames | `lib/search/nav/*`, `search/_components/NavSuggestions.svelte`, `SurahSuggestions.svelte` | baked | full | [~] shipped Q1; go-to-verse not typed this slice |
| E3 | Command palette with all sources (surahs, sajdas, ranges, reference, text, translation text, settings routes, site routes, app actions, recents) | `lib/search/palette/**`, `lib/stores/command-palette.svelte.ts`, `lib/components/search/*` | local | full | [x] palette opened and used (tests/shell.rs) |
| E4 | Search recents, selection | `stores/search-recents.svelte.ts`, `search-selection.svelte.ts` | local | full | [~] not exercised |
| E5 | Voice search | `lib/search/voice/recognition.ts` | NSH speech | platform | [~] needs a device microphone |

## F. Bookmarks, notes, pins

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| F1 | Bookmarks with folders, list page, row actions, feedback | `routes/(application)/bookmarks/**`, `lib/bookmarks/*`, `lib/components/bookmarks/*` | `bookmark/v1/sync` | full, syncs later | [x] bookmark from reader, "Saved to" feedback (headless) |
| F2 | Notes (device-owned annotations) and notes library | `stores/annotations.svelte.ts`, `lib/boot/notes-owner.svelte.ts`, `yours/_components/NotesLibrary.svelte`, `lib/home/notes-library.ts` | local | full | [~] note editor wired; not typed in the run |
| F3 | Reading bookmark (single "where I am") | `lib/preferences/reading-bookmark.svelte.ts`, `yours/_components/ReadingBookmarkCard.svelte` | `preference/v1/sync` | full | [x] set reading place from verse tools, shown as "here" on reopen (headless) |
| F4 | Sync indicator and sync notices | `bookmarks/_components/SyncIndicator.svelte`, `lib/components/auth/SyncNotices.svelte` | sync registry | — | [x] sync round exercised against fake server (tests/parity.rs, tests/wiring.rs) |

## G. Reading sessions and "Yours"

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| G1 | Reading session = continuous thread, UUID v7 id; continue, history, start over, archive | `lib/sessions/*`, `stores/reader-session.svelte.ts`, `reader-persistence.svelte.ts` | `reading-session/v1/sync`, `/archive` | full, syncs later | [x] thread entered, read, continued from Yours (headless) |
| G2 | Device handoff labels | `lib/sessions/device.ts`, `handoff.ts` | `contracts/devices.md` D5 | — | [~] needs a second device |
| G3 | Yours hub: continue threads, thread history, insights, goals, notes | `routes/(application)/yours/**` | G1, G4, G5 | full | [x] Yours hub + continue row + cards + insights (tests/parity.rs) |
| G4 | Reading goals and progress | `lib/goals/*`, `yours/_components/ReadingGoals.svelte` | `preference/v1/sync` (goals register) | full | [x] goals surface + preference sync (tests/parity.rs) |
| G5 | Personal insights (always on) | `lib/analytics/reading-analytics.ts`, `ReadingInsights.svelte` | `user/analytics/v1/{events,days,summary}` | events queued | [x] personal insights fetched from `/user/analytics/v1/summary` (tests/parity.rs) |
| G6 | Custom cards (create, edit, schedule, undo, stats) and card detail | `lib/home/{CardForm,ScheduleEditor,TimeSelect}.svelte`, `card-*.ts`, `schedule.ts`, `yours/cards/[id]` | `preference/v1/sync` | full | [x] custom cards surface ("Your cards", "Add card") exercised (tests/parity.rs) |
| G7 | Goal reminders | `settings/_components/GoalReminders.svelte`, `lib/reminders/*` | `notification/v1/reminder/{get,set}` | local notifications | [~] reminder sync exercised (tests/parity.rs); Apple packaged iOS/macOS uses `UNCalendarNotificationTrigger`; Android/Windows/Linux use in-app timer fallback (no NSH scheduled notification API) |

## H. Azkar and daily ayah

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| H1 | Azkar list and zikr detail, recite panel, add card inline, translated azkar route | `routes/(application)/azkar/**`, `t/[lang]/[translator]/azkar/[slug]`, `lib/home/next-zikr.ts` | baked + B1/B2 | full | [x] azkar list + zikr page (azkar-index.png, azkar-zikr.png) |
| H2 | Daily ayah | `routes/(marketing)/daily-ayah`, `lib/home/daily-ayah.ts`, `lib/server/daily-ayah.ts` | B1 | full | [x] daily ayah card on home (headless) |

## I. Auth and account

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| I1 | Native session transport, CSRF, rotation, sign-out | `lib/auth/auth-client.ts`, `auth-state.svelte.ts` | `csrf/v1/generate`, `auth/v1/log_in`, `log_out`, `user/v1/get` | signed-in state kept | [x] session transport + rotation (tests/wiring.rs, fake server) |
| I2 | Login, register (age declaration), verify email, forgot password | `routes/(auth)/**`, `lib/auth/components/*`, `flows.svelte.ts`, `schemas.ts`, `form-validation.svelte.ts` | `auth/v1/register`, `email_verification/v1/*`, `forgot_password/v1/*`, `user/v1/age/declare` | needs network | [x] login validation + sign-in flow exercised over fake server (tests/parity.rs) |
| I3 | TOTP at sign-in (`pending_login`) and 2FA setup/disable | `routes/auth/[provider]/totp`, `TwoFactorControls.svelte` | `auth/v1/login/totp*`, `auth/v1/2fa/*` | needs network | [x] `pending_login` -> TOTP submit -> signed in exercised (tests/parity.rs) |
| I4 | OAuth providers (Google, Apple, Facebook, GitHub, Discord), link/unlink | `lib/auth/oauth-*.ts`, `OAuthButtons.svelte`, `ConnectedAccounts.svelte`, `routes/auth/[provider]/*` | `/auth/{provider}/v1/token`, `/auth/{google,apple,facebook,discord}/v1/token/link`, `/auth/github/v1/native/link`, `user/v1/identities*` | needs network | [x] OAuth sign-in + native link endpoints (all 5 providers, CSRF + fresh-auth gated) + Connected Accounts link/unlink UI (tests/parity.rs, eq-core::api::tests, backend oauth/static_csrf tests) |
| I5 | Passkeys (login, register, list, remove) | `lib/auth/passkey-flow.svelte.ts` | `passkey/v1/*` | needs network | [blocked] passkeys: no NSH API (`nsh-biometrics` is local auth only, not WebAuthn/FIDO2) |
| I6 | Fresh-auth (reauth) gate for sensitive actions | `fresh-auth-client.ts`, `FreshAuthControls.svelte` | `auth/v1/reauth*` | needs network | [x] reauth status endpoint + Account gate exercised (tests/parity.rs) |
| I7 | Account page: profile update, sessions list/terminate, devices list/rename/remove | `routes/(account)/account/**`, `lib/auth/account-client.ts`, `lib/device/*` | `user/v1/update`, `auth/v1/sessions/*`, `device/v1/*` | needs network | [x] Account page: profile, sessions, devices, identities fetched and rendered (tests/parity.rs) |
| I8 | Device identity: per-install id, P-256 key, challenge, register, hint | `lib/device/{identity,keys,key-store,register}.ts` | `device/v1/{challenge,register}` | — | [x] device challenge/register + Apple `os_major` verified over fake server (tests/parity.rs, eq-core tests) |
| I9 | Delete account, account purge on sign-out, sign-out notice | `DeleteAccountControls.svelte`, `account-purge.ts`, `purge-hook.ts`, `sign-out-notice.svelte.ts` | `user/v1/delete` | — | [~] wired on Account; destructive delete not submitted in headless run |
| I10 | Banned notice, appeal flow | `BannedNotice.svelte`, `routes/appeal/**`, `lib/appeal/route.ts` | `appeal/v1/*` | needs network | [x] Appeal route opened and form rendered (tests/parity.rs) |
| I11 | Report content flow | `routes/report`, `lib/report/payload.ts` | `report/v1/submit` | needs network | [x] report submitted to `/report/v1/submit` (tests/parity.rs) |
| I12 | Newsletter signup and confirm | `routes/newsletter/confirm`, footer form | `newsletter/v1/*` | needs network | [x] newsletter confirm route submitted to `/newsletter/v1/confirm` (tests/parity.rs) |
| I13 | Minor gate | `lib/boot/minor-gate.svelte.ts`, `AgeDeclaration.svelte` | `user/v1/age/declare` | — | [x] minor gate forces analytics off even when enabled (tests/parity.rs) |
| I14 | Bot check on auth forms (Turnstile) | `lib/auth/turnstile.ts`, `TurnstileWidget.svelte` | Turnstile | — | [n/a] web widget; backend `auth_v1` register skips Turnstile for native clients (`NativeClient`), per-IP limits and daily caps stay |

## J. Settings

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| J1 | Settings shell and sections | `routes/(application)/settings/**`, `SettingsShell.svelte` | — | full | [x] settings opens on Storage (tests/shell.rs) |
| J2 | Appearance: theme, accent, derived tokens | `AppearanceSection.svelte`, `lib/theme/derive.ts`, `lib/components/tweaks`, `routes/layout.css` | prefs | full | [x] opened headless (tests/parity.rs) |
| J3 | Reading settings | `ReadingSection.svelte`, `stores/reader-settings.svelte.ts` | prefs | full | [x] opened headless (tests/parity.rs) |
| J4 | Storage: per-artifact rows, usage bar, delete, download | `StorageSection.svelte`, `StorageArtifactRow.svelte`, `UsageBar.svelte`, `stores/storage-report.svelte.ts` | app data dir | full | [x] opened headless (tests/parity.rs, tests/shell.rs) |
| J5 | App section: language, updates | `AppSection.svelte`, `lib/offline/update.svelte.ts` | NSH updater | — | [~] opened headless (tests/parity.rs); updater needs a release feed |
| J6 | Privacy: consent toggles | `PrivacySection.svelte`, `stores/consent.svelte.ts` | local | full | [x] analytics toggle + minor gate exercised (tests/parity.rs) |
| J7 | Data: export, import, erase | `DataSection.svelte`, `lib/storage/data-export.ts` | local | full | [x] opened headless (tests/parity.rs) |
| J8 | Account section | `AccountSection.svelte` | I7 | — | [x] opened headless (tests/parity.rs) |
| J9 | Settings document sync | `lib/settings/settings-document.ts`, `lib/preferences/*`, `stores/prefs.svelte.ts` | `preference/v1/sync` | full, syncs later | [x] preferences sync round exercised against fake server (tests/parity.rs) |

## K. Offline, downloads, updates

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| K1 | Offline pack ("download everything to read offline") | `lib/offline/pack.ts`, `offline-store.svelte.ts`, `lib/components/status/offline-pack-copy.ts` | artifacts by id (native stores the SQLite artifacts, not route responses) | — | [~] not run (large download) |
| K2 | Downloads screen and bottom progress popup (dismissible) | `routes/(application)/downloads`, `lib/components/status/{DownloadBar,download-progress,download-dismissal}*` | B5 | — | [~] Downloads screen opened (tests/parity.rs); bar stacks above audio dock (FN), not seen with a live download |
| K3 | Online/offline state and banner | `lib/offline/online.svelte.ts`, `status/Pulse.svelte` | — | — | [~] `nsh-connectivity` wired; not toggled |
| K4 | Update available toast | `status/UpdateToast.svelte`, `lib/offline/update.svelte.ts` | NSH updater | — | [~] updater needs a release feed |

## L. Sync engine

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| L1 | Merge rules, stamps, clock, ids (UUID v7) | `lib/sync/{merge,clock,ids,types}.ts` | `rust/crates/eq-sync-core` (shared), `contracts/sync.md` + vectors | — | [x] eq-sync-core vectors (cargo test) |
| L2 | Engine: outbox ("the row is the queue"), registry, round status, pull/push | `lib/sync/{engine.svelte,outbox,registry.svelte,round-status}.ts` | `*/v1/sync` | dirty rows wait | [x] outbox/push round (tests/wiring.rs, tests/parity.rs) |

## M. Notifications and push

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| M1 | Reminder notifications | `lib/reminders/*`, `lib/firebase/messaging.ts`, `push-privacy.ts` | `notification/v1/reminder/*`, `device/v1/token/delete` | local notifications | [~] packaged iOS/macOS uses `UNCalendarNotificationTrigger` daily triggers; Android/Windows/Linux use in-app timer fallback ([blocked]: no NSH scheduled notification API) |
| M2 | Remote push (FCM) | `lib/firebase/messaging.ts`, `service-worker.ts` | FCM | — | [blocked] push token needs the native embedding host (`NshFirebase` + owner Firebase config) |

## N. Consent, analytics, crash reporting

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| N1 | Consent banner and state | `stores/consent.svelte.ts`, `lib/components/ui` consent surface | local | full | [x] consent toggles + minor gate verified (tests/parity.rs) |
| N2 | Product analytics (off until consent; no device id in payloads) | `lib/boot/analytics.ts`, `lib/firebase/analytics.ts`, `performance.ts` | `analytics/v1/*` | queued | [x] consent gating verified (tests/parity.rs) |
| N3 | Crash reporting (off until consent) | `lib/boot/crash-reporting.ts` | — | — | [x] consent gating verified (tests/parity.rs) |

## O. i18n

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| O1 | English + Arabic catalogues loaded from `web/messages/**` (single source, not copied by hand) | `messages/{en,ar}.json`, `messages/{reader,auth,account,appeal}/*`, `lib/i18n/m/*` | compile-time | full | [x] all strings from web catalogues (every shot) |
| O2 | RTL layout, locale switch, locale suggestion, relative time | `lib/i18n/{app-locale,locales,locale-suggest.svelte,relative-time}.ts`, `components/i18n` | prefs | full | [x] RTL lanes render + En/Ar locale switch exercised (tests/parity.rs) |

## Platform gaps (blocked)

| Gap | Rows | Why |
|---|---|---|
| Passkeys | I5 | no NSH API (`nsh-biometrics` is local auth only, not WebAuthn/FIDO2) |
| Scheduled local notifications (non-Apple) | G7, M1 | iOS/macOS packaged apps use `UNCalendarNotificationTrigger`; Android/Windows/Linux fall back to in-app timer (no NSH scheduled notification API; `nsh-background` is in-process timers) |
| Push token (FCM) | M2 | needs the native embedding host (`NshFirebase` Swift/Gradle `firebase.*` handlers) + owner Firebase config (`O34`) |
| Lock-screen / background audio & wake lock | D2, D3 | no NSH audio session / Now Playing API; `nsh-wake-lock 0.1.0` blocked by `nsh-secure-storage 0.1.0`'s upstream `wasm-bindgen = "=0.2.129"` pin |

## Not applicable on native

| Web concern | Why | Native equivalent |
|---|---|---|
| SEO meta, JSON-LD, `sitemap.xml`, `robots.txt` | search-engine surface | none |
| `llms.txt`, `llms-full.txt`, markdown twins (`*.md`, `[slug].txt`) | crawler/agent text variants of pages | none |
| Prerender, SSR + 7-day disk cache for translated routes, SSR limiter | server rendering delivery | reads local SQLite directly (B1, B2) |
| Service worker precache, offline shell page | browser cache | on-device storage of the same data (B5, K1) |
| `_quran/[...artifact]` gateway route, `health/quran` | web server endpoints | client downloads from the same artifact base |
| `firebase-config.js` route | web push bootstrap | M2 |
| OAuth popup / redirect pages (`auth/[provider]/success|failure`) | browser redirect targets | system browser + return link (I4) |
| `routes/design/**` | dev-only specimens, 404 in production | none |
| `manifest.webmanifest`, PWA install | browser install | the app itself |
| Cookie/CSRF-cookie handling, CORS | browser transport | `x-eq-session` (I1) |
| Admin, moderation, posts, comments, billing, media modules | not in `web/` either | none |
