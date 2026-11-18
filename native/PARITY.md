# Native parity checklist

Native (NSH) counterpart of `web/`. Target: 1:1 feature parity. Web stays as is.

Legend: `[ ]` open, `[x]` implemented and checked, `[~]` implemented, not verified by a run, `[n/a]` no native meaning (reason given), `[blocked]` reason given.
"Web" paths are relative to `web/src/`. "API" paths are backend routes (`rust/backend/api`). Every signed-in call uses the native session transport (`x-eq-session`, `contracts/devices.md` D6), never cookies.

Targets: macOS, iOS, Android (plus Linux/Windows desktop from the same crate). The NSH web target is not shipped: the SvelteKit app is the web client.

## A. Shell, navigation, chrome

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| A1 | App shell: sidebar on desktop, tabs on phone, reader header, sticky nav | `routes/(application)/+layout.svelte`, `_reader/Sidebar.svelte`, `SidebarVirtualList.svelte`, `IndexTabs.svelte`, `lib/components/nav/*`, `lib/stores/sticky-nav.svelte.ts` | local | full | [ ] |
| A2 | Typed routes + deep links for the reader URL grammar (`/{surah}`, `/page/n`, `/juz/n`, `/hizb/n`, `/t/{lang}/{translator}/…`, `?ayah=`) | `lib/data/quran.ts` (`surah*For`), `lib/reader/*-param.ts`, `thread-path.ts`, `visible-url.ts`, `params.ts` | local | full | [ ] |
| A3 | Landing / home with cards (continue, daily ayah, next zikr, custom cards) | `routes/(marketing)/+page.svelte`, `lib/home/LandingCards.svelte`, `CardTile.svelte`, `cards.svelte.ts`, `layout.svelte.ts`, `reading-choice.svelte.ts` | local + `preference/v1/sync` | full | [ ] |
| A4 | About, FAQ, contact, privacy, terms pages | `routes/(marketing)/{about,faq,contact,privacy,terms}`, `lib/i18n/*-copy.ts` | static copy | full | [ ] |
| A5 | Footer links, brand mark, bismillah | `lib/components/footer`, `brand/*`, `lib/assets/open-pages.svg` | static | full | [ ] |
| A6 | Error screen and not-found handling | `routes/+error.svelte` | — | full | [ ] |
| A7 | Keyboard shortcuts (palette, reader nav) | `lib/hotkeys.svelte.ts`, consumers | local | full | [ ] |
| A8 | Notifications inbox / toasts surface | `lib/components/notifications`, `lib/stores/notifications.svelte.ts`, `components/ui/sonner` | local | full | [ ] |

## B. Quran data

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| B1 | Immutable Quran DBs by id (no hash, read-only, never modified): Uthmani, annotated Uthmani, simple-clean, IndoPak, Tajweed | `lib/quran/view/source-profiles.ts`, `catalogue.ts`, `sql.ts`, `protocol.ts`, `wasm-query-runner.ts`, `workers/*` | artifact base `/_quran` (R2), baked id map | full once downloaded; default script bundled | [ ] |
| B2 | Translation catalogue (378 sources) from baked map, download by id | `lib/data/translations.json`, `translations.ts`, `lib/quran/translation-groups.ts` | artifacts by id | per downloaded source | [ ] |
| B3 | Surah names, mushaf divisions (juz, hizb, page), browse metadata | `lib/data/surah-names.*`, `mushaf-divisions.ts`, `quran-browse.ts`, `quran-data*.ts`, `static/quran-meta` | baked | full | [ ] |
| B4 | Online range fetch when a source is not on device | `lib/quran/api-client.ts`, `range-fetch.ts`, `fetch.ts`, `wire.ts`, `source-plan.ts`, `api-availability.ts` | `quran/v1/*` | falls back to local | [ ] |
| B5 | Download manager: progress, resume, size check against baked map, discard partials | `lib/workers/download.ts`, `opfs-cache.ts`, `opfs-retention.ts`, `lib/offline/*` | app data dir | n/a | [ ] |
| B6 | Text marks and canonical view rules (verbatim text, tajweed marks, IndoPak view) | `lib/quran/view/*`, `text-marks.ts` | local | full | [ ] |

## C. Reader

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| C1 | Surah index, juz index, pages index | `routes/(application)/surah`, `juz`, `pages`, `IndexTabs.svelte` | baked | full | [ ] |
| C2 | Surah reader (virtualized, long surahs) | `[surah]/+page.svelte`, `_reader/SurahReader.svelte`, `SurahPageRoute.svelte`, `ReaderVirtualList.svelte`, `virtual-reader.ts`, `page-heights.ts`, `viewport-anchor.ts` | B1 | full | [ ] |
| C3 | Page, juz, hizb and range readers | `page/[n]`, `juz/[n]`, `hizb/[n]`, `_reader/RangeReader.svelte`, `range-validate.ts`, `ReaderPageNav.svelte` | B1 | full | [ ] |
| C4 | Arabic script switch (5 scripts) with fonts: Uthmanic Hafs, Uthman Taha Naskh, IndoPak compat v4, ayah ornament | `lib/config/reader-fonts.ts`, `lib/fonts/arabic-fonts.ts`, `ReadingTextPicker.svelte`, `IndoPakAyah.svelte`, `AyahOrnament.svelte`, `lib/stores/reading-text.svelte.ts`, `static/fonts` | prefs | full | [ ] |
| C5 | Tajweed colouring | `lib/quran/view/tajweed.ts` | B1 | full | [ ] |
| C6 | Translations: picker modal, recents, region suggestions, translated reader routes | `lib/components/translation/*`, `TranslationButton.svelte`, `[surah]/t/[lang]/[translator]`, `t/[lang]/[translator]/**`, `lib/reader/translation-nav.ts`, `stores/translation-recents.svelte.ts` | B2, B4 | per source | [ ] |
| C7 | Stacked translations (lanes) | `_reader/StackedLanes.svelte`, `stacked-translations.svelte.ts`, `StackedLaneSettings.svelte`, `hue-slot.ts` | prefs | per source | [ ] |
| C8 | Reader modes and typography settings (mode toggle, sizes, arabic companion) | `ReaderModeToggle.svelte`, `arabic-companion.svelte.ts`, `typography-change.ts`, `stores/reader-settings.svelte.ts`, `reader-presentation.ts` | prefs | full | [ ] |
| C9 | Verse tools: copy, share (text + verse card image, range), report verse, bookmark, note, pin | `VerseTools.svelte`, `VerseRow.svelte`, `ShareDialog.svelte`, `lib/share/*`, `ReportVerseDialog.svelte`, `PinsDrawer.svelte`, `stores/pins.svelte.ts`, `reader-share.svelte.ts` | local, `report/v1/submit` | share/copy full | [ ] |
| C10 | Word-level view (word tokens, gate, registry) | `VerseWords.svelte`, `lib/quran/words/*` | annotated source | full when source local | [ ] |
| C11 | Position indicator, resume pill, reader history, status banner / degradation | `PositionIndicator.svelte`, `ResumePill.svelte`, `reader-history.ts`, `ReaderStatusBanner.svelte`, `reader-degradation.svelte.ts`, `lib/reader/resume.ts`, `reading-flow.ts` | local | full | [ ] |
| C12 | Read tracking heuristics (no penalty for fast readers) | `lib/analytics/read-tracker.ts`, `lib/quran/track-view.svelte.ts`, `engagement*.ts` | feeds F/G | queued | [ ] |

## D. Audio

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| D1 | Recitation catalogue, URL building, play policy | `lib/audio/catalogue.ts`, `url.ts`, `policy.ts` | audio CDN | — | [ ] |
| D2 | Player engine, queue, repeat | `lib/audio/engine.svelte.ts`, `queue.ts`, `repeat.ts` | NSH audio | — | [ ] |
| D3 | Audio dock UI and in-reader controls | `lib/components/audio/*`, `lib/i18n/audio-copy.ts` | — | — | [ ] |
| D4 | Offline audio | `lib/audio/offline.ts` | app data dir | full when saved | [ ] |

## E. Search and command palette

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| E1 | Search page: Arabic + translation text, normalisation, translit, ranking, highlights | `routes/(application)/search/**`, `lib/quran/search/*`, `lib/quran/search.ts`, `lib/search/page/*` | local DBs | full | [ ] |
| E2 | Go-to-verse / nav parsing, surah suggestions, juz nicknames | `lib/search/nav/*`, `search/_components/NavSuggestions.svelte`, `SurahSuggestions.svelte` | baked | full | [ ] |
| E3 | Command palette with all sources (surahs, sajdas, ranges, reference, text, translation text, settings routes, site routes, app actions, recents) | `lib/search/palette/**`, `lib/stores/command-palette.svelte.ts`, `lib/components/search/*` | local | full | [ ] |
| E4 | Search recents, selection | `stores/search-recents.svelte.ts`, `search-selection.svelte.ts` | local | full | [ ] |
| E5 | Voice search | `lib/search/voice/recognition.ts` | NSH speech | platform | [ ] |

## F. Bookmarks, notes, pins

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| F1 | Bookmarks with folders, list page, row actions, feedback | `routes/(application)/bookmarks/**`, `lib/bookmarks/*`, `lib/components/bookmarks/*` | `bookmark/v1/sync` | full, syncs later | [ ] |
| F2 | Notes (device-owned annotations) and notes library | `stores/annotations.svelte.ts`, `lib/boot/notes-owner.svelte.ts`, `yours/_components/NotesLibrary.svelte`, `lib/home/notes-library.ts` | local | full | [ ] |
| F3 | Reading bookmark (single "where I am") | `lib/preferences/reading-bookmark.svelte.ts`, `yours/_components/ReadingBookmarkCard.svelte` | `preference/v1/sync` | full | [ ] |
| F4 | Sync indicator and sync notices | `bookmarks/_components/SyncIndicator.svelte`, `lib/components/auth/SyncNotices.svelte` | sync registry | — | [ ] |

## G. Reading sessions and "Yours"

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| G1 | Reading session = continuous thread, UUID v7 id; continue, history, start over, archive | `lib/sessions/*`, `stores/reader-session.svelte.ts`, `reader-persistence.svelte.ts` | `reading-session/v1/sync`, `/archive` | full, syncs later | [ ] |
| G2 | Device handoff labels | `lib/sessions/device.ts`, `handoff.ts` | `contracts/devices.md` D5 | — | [ ] |
| G3 | Yours hub: continue threads, thread history, insights, goals, notes | `routes/(application)/yours/**` | G1, G4, G5 | full | [ ] |
| G4 | Reading goals and progress | `lib/goals/*`, `yours/_components/ReadingGoals.svelte` | `preference/v1/sync` (goals register) | full | [ ] |
| G5 | Personal insights (always on) | `lib/analytics/reading-analytics.ts`, `ReadingInsights.svelte` | `user/analytics/v1/{events,days,summary}` | events queued | [ ] |
| G6 | Custom cards (create, edit, schedule, undo, stats) and card detail | `lib/home/{CardForm,ScheduleEditor,TimeSelect}.svelte`, `card-*.ts`, `schedule.ts`, `yours/cards/[id]` | `preference/v1/sync` | full | [ ] |
| G7 | Goal reminders | `settings/_components/GoalReminders.svelte`, `lib/reminders/*` | `notification/v1/reminder/{get,set}` | local notifications | [ ] |

## H. Azkar and daily ayah

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| H1 | Azkar list and zikr detail, recite panel, add card inline, translated azkar route | `routes/(application)/azkar/**`, `t/[lang]/[translator]/azkar/[slug]`, `lib/home/next-zikr.ts` | baked + B1/B2 | full | [ ] |
| H2 | Daily ayah | `routes/(marketing)/daily-ayah`, `lib/home/daily-ayah.ts`, `lib/server/daily-ayah.ts` | B1 | full | [ ] |

## I. Auth and account

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| I1 | Native session transport, CSRF, rotation, sign-out | `lib/auth/auth-client.ts`, `auth-state.svelte.ts` | `csrf/v1/generate`, `auth/v1/log_in`, `log_out`, `user/v1/get` | signed-in state kept | [ ] |
| I2 | Login, register (age declaration), verify email, forgot password | `routes/(auth)/**`, `lib/auth/components/*`, `flows.svelte.ts`, `schemas.ts`, `form-validation.svelte.ts` | `auth/v1/register`, `email_verification/v1/*`, `forgot_password/v1/*`, `user/v1/age/declare` | needs network | [ ] |
| I3 | TOTP at sign-in (`pending_login`) and 2FA setup/disable | `routes/auth/[provider]/totp`, `TwoFactorControls.svelte` | `auth/v1/login/totp*`, `auth/v1/2fa/*` | needs network | [ ] |
| I4 | OAuth providers (Google, Apple, Facebook, GitHub, Discord), link/unlink | `lib/auth/oauth-*.ts`, `OAuthButtons.svelte`, `ConnectedAccounts.svelte`, `routes/auth/[provider]/*` | `/auth/{provider}/v1/token`, `user/v1/identities*` | needs network | [ ] |
| I5 | Passkeys (login, register, list, remove) | `lib/auth/passkey-flow.svelte.ts` | `passkey/v1/*` | needs network | [ ] |
| I6 | Fresh-auth (reauth) gate for sensitive actions | `fresh-auth-client.ts`, `FreshAuthControls.svelte` | `auth/v1/reauth*` | needs network | [ ] |
| I7 | Account page: profile update, sessions list/terminate, devices list/rename/remove | `routes/(account)/account/**`, `lib/auth/account-client.ts`, `lib/device/*` | `user/v1/update`, `auth/v1/sessions/*`, `device/v1/*` | needs network | [ ] |
| I8 | Device identity: per-install id, P-256 key, challenge, register, hint | `lib/device/{identity,keys,key-store,register}.ts` | `device/v1/{challenge,register}` | — | [ ] |
| I9 | Delete account, account purge on sign-out, sign-out notice | `DeleteAccountControls.svelte`, `account-purge.ts`, `purge-hook.ts`, `sign-out-notice.svelte.ts` | `user/v1/delete` | — | [ ] |
| I10 | Banned notice, appeal flow | `BannedNotice.svelte`, `routes/appeal/**`, `lib/appeal/route.ts` | `appeal/v1/*` | needs network | [ ] |
| I11 | Report content flow | `routes/report`, `lib/report/payload.ts` | `report/v1/submit` | needs network | [ ] |
| I12 | Newsletter signup and confirm | `routes/newsletter/confirm`, footer form | `newsletter/v1/*` | needs network | [ ] |
| I13 | Minor gate | `lib/boot/minor-gate.svelte.ts`, `AgeDeclaration.svelte` | `user/v1/age/declare` | — | [ ] |
| I14 | Bot check on auth forms (Turnstile) | `lib/auth/turnstile.ts`, `TurnstileWidget.svelte` | Turnstile | — | [ ] |

## J. Settings

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| J1 | Settings shell and sections | `routes/(application)/settings/**`, `SettingsShell.svelte` | — | full | [ ] |
| J2 | Appearance: theme, accent, derived tokens | `AppearanceSection.svelte`, `lib/theme/derive.ts`, `lib/components/tweaks`, `routes/layout.css` | prefs | full | [ ] |
| J3 | Reading settings | `ReadingSection.svelte`, `stores/reader-settings.svelte.ts` | prefs | full | [ ] |
| J4 | Storage: per-artifact rows, usage bar, delete, download | `StorageSection.svelte`, `StorageArtifactRow.svelte`, `UsageBar.svelte`, `stores/storage-report.svelte.ts` | app data dir | full | [ ] |
| J5 | App section: language, updates | `AppSection.svelte`, `lib/offline/update.svelte.ts` | NSH updater | — | [ ] |
| J6 | Privacy: consent toggles | `PrivacySection.svelte`, `stores/consent.svelte.ts` | local | full | [ ] |
| J7 | Data: export, import, erase | `DataSection.svelte`, `lib/storage/data-export.ts` | local | full | [ ] |
| J8 | Account section | `AccountSection.svelte` | I7 | — | [ ] |
| J9 | Settings document sync | `lib/settings/settings-document.ts`, `lib/preferences/*`, `stores/prefs.svelte.ts` | `preference/v1/sync` | full, syncs later | [ ] |

## K. Offline, downloads, updates

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| K1 | Offline pack ("download everything to read offline") | `lib/offline/pack.ts`, `offline-store.svelte.ts`, `lib/components/status/offline-pack-copy.ts` | artifacts by id (native stores the SQLite artifacts, not route responses) | — | [ ] |
| K2 | Downloads screen and bottom progress popup (dismissible) | `routes/(application)/downloads`, `lib/components/status/{DownloadBar,download-progress,download-dismissal}*` | B5 | — | [ ] |
| K3 | Online/offline state and banner | `lib/offline/online.svelte.ts`, `status/Pulse.svelte` | — | — | [ ] |
| K4 | Update available toast | `status/UpdateToast.svelte`, `lib/offline/update.svelte.ts` | NSH updater | — | [ ] |

## L. Sync engine

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| L1 | Merge rules, stamps, clock, ids (UUID v7) | `lib/sync/{merge,clock,ids,types}.ts` | `rust/crates/eq-sync-core` (shared), `contracts/sync.md` + vectors | — | [ ] |
| L2 | Engine: outbox ("the row is the queue"), registry, round status, pull/push | `lib/sync/{engine.svelte,outbox,registry.svelte,round-status}.ts` | `*/v1/sync` | dirty rows wait | [ ] |

## M. Notifications and push

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| M1 | Reminder notifications | `lib/reminders/*`, `lib/firebase/messaging.ts`, `push-privacy.ts` | `notification/v1/reminder/*`, `device/v1/token/delete` | local notifications | [ ] |
| M2 | Remote push (FCM) | `lib/firebase/messaging.ts`, `service-worker.ts` | FCM | — | [ ] |

## N. Consent, analytics, crash reporting

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| N1 | Consent banner and state | `stores/consent.svelte.ts`, `lib/components/ui` consent surface | local | full | [ ] |
| N2 | Product analytics (off until consent; no device id in payloads) | `lib/boot/analytics.ts`, `lib/firebase/analytics.ts`, `performance.ts` | `analytics/v1/*` | queued | [ ] |
| N3 | Crash reporting (off until consent) | `lib/boot/crash-reporting.ts` | — | — | [ ] |

## O. i18n

| # | Feature | Web source | API / local store | Offline | State |
|---|---|---|---|---|---|
| O1 | English + Arabic catalogues loaded from `web/messages/**` (single source, not copied by hand) | `messages/{en,ar}.json`, `messages/{reader,auth,account,appeal}/*`, `lib/i18n/m/*` | compile-time | full | [ ] |
| O2 | RTL layout, locale switch, locale suggestion, relative time | `lib/i18n/{app-locale,locales,locale-suggest.svelte,relative-time}.ts`, `components/i18n` | prefs | full | [ ] |

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
