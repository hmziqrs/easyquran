# Product and copy

[Back to index](../README.md)

Product features, then the copy truth pass.

## Product

#### F2. Settings and widget-layout sync

Build: sync the settings document and home widget layout across devices on the `contracts/sync.md` protocol: one stamp per setting key, one for the layout document, per-row results, delta pull, `expectedUserId`. New migration (next free number), new module, wire `settings-document.ts`. Contract detail: `documentation/settings-system.md` §7. Notes stay device-local by decision; never add them to sync.
Files: `web/src/lib/home/layout.svelte.ts`, `web/src/lib/sync/`, `rust/backend/api/src/modules/`, `rust/backend/api/migration/src/`
Done when: changing a setting or widget layout on one signed-in device shows on another after sync; two different settings changed offline on two devices both survive; signed-out behaviour unchanged; vectors and `cargo test` cover merge and owner mismatch.
Blocked by: S7

#### F5. Explicit reading bookmark

Build: user-set page/verse reading bookmark, separate from automatic resume. Set, replace with Undo toast, timestamp, source, card on `/yours`, synced with the account as a new `contracts/sync.md` domain, with its rule and vectors added first.
Files: `web/src/lib/stores/reader.svelte.ts`, `web/src/routes/(application)/_reader/VerseTools.svelte`, `web/src/routes/(application)/yours/`
Done when: automatic resume never overwrites the user-set bookmark; set, replace and undo are tested.
Blocked by: S7

#### F6. Personal data export and import

Build: versioned JSON export of bookmarks, folders, notes, threads, cards and settings. Import validates, adds only rows whose ids are absent (with a toast), and rejects newer schema versions. Lives in Settings.
Files: `web/src/routes/(application)/settings/`, new: `web/src/lib/storage/data-export.ts`
Done when: export then import on a clean browser round-trips everything; malformed and future-schema files are rejected with a clear message.

#### F7. Self-service account deletion

Build: new self-service route (only `admin_delete` exists today), through the A38 erase path, with `FreshAuthControls` re-auth. Revokes sessions, deletes Quran-state rows, device rows and device tokens, offers export first, clears local account data. Immediate, no grace period (default until the owner says otherwise). Update the privacy and terms deletion wording to match.
Files: `rust/backend/api/src/modules/user_v1/`, `web/src/routes/(account)/account/+page.svelte`, `web/messages/en.json`, `web/messages/ar.json`
Done when: API test proves rows and tokens are gone and the session is dead; legal copy no longer says "ask us to delete".
Blocked by: F6, A38

#### F8. Reading goals and streaks

Build: goal presets (time per day, finish in 30 days, finish in a year) plus custom range, built on `reading_daily` and `localDay`. Goals sync as a new `contracts/sync.md` domain, with its rule and vectors added first. Progress comes from server totals, so reads from all devices count. Pause and completion. Streak calendar on `/yours`. Must never penalise very fast readers: time goals count verse reads too.
Files: `web/src/routes/(application)/yours/_components/ReadingInsights.svelte`, `web/src/lib/analytics/reading-analytics.ts`, `rust/backend/api/src/modules/user_analytics_v1/`
Done when: goal progress and streak derive from recorded reads, respect the user's local day, and are covered by tests including a fast-reader case.
Blocked by: S7

#### F9. Goal reminders

Build: `notification_v1` has no schedules today, so add a stored schedule per user (time zone, quiet hours, per-kind preferences, unsubscribe) and a job in the scheduler that writes `job_runs`. Push goes to the account's most recently seen device with a token. GPUI uses local notifications only when the account has no mobile push token (G6), so nobody is told twice.
Files: `rust/backend/api/src/modules/notification_v1/`, `web/src/lib/firebase/`, `web/src/routes/(application)/settings/`
Done when: a scheduled reminder fires at the user's local time, respects quiet hours, and stops on unsubscribe.
Blocked by: F8

#### F10. Pin and compare verses

Build: device-local ordered pin set (no sync at launch) with add, remove and reorder; compare drawer with composed Arabic and translation selection; mobile and overflow handling; convert pins into a bookmark folder.
Files: `web/src/routes/(application)/_reader/VerseTools.svelte`, new: `web/src/lib/stores/pins.svelte.ts`
Done when: pins survive navigation, compare renders on phone and desktop, conversion creates a folder.

#### F11. Parallel translation layout

Build: side-by-side presentation for stacked translations (extras stack vertically today, verse mode only); per-extra font controls. Keep the current extras limit if one exists; otherwise set 5. Stays client-only, no change to cache keys.
Files: `web/src/routes/(application)/_reader/StackedLanes.svelte`, `web/src/routes/(application)/_reader/stacked-translations.svelte.ts`
Done when: side-by-side works with up to 5 extras, collapses on narrow screens, and SSR/cache keys are untouched.

#### F12. Deeper navigation

Build: revelation-order sort in the sidebar (`revelationOrder` is in the data); `surah:ayah` and range jump from anywhere in the palette, extending the existing go-to-verse action in the palette's Quran source rather than adding a second one; browse by ruku, manzil and sajda from existing metadata and `/quran/v1` routes.
Files: `web/src/routes/(application)/_reader/Sidebar.svelte`, `web/src/routes/(application)/_reader/IndexTabs.svelte`, `web/src/lib/components/search/GlobalSearchPalette.svelte`
Done when: `2:255` and `2:255-2:286` jump correctly from any route with translation context preserved; revelation sort and new browse lists are tested.

#### F13. Share dialog and image card

Build: dialog for range, source and format with attribution options and the canonical link (`…/{slug}?ayah=2:255-2:286`); branded accessible image card with download. Plain copy/share stays the fast path. Video waits for audio.
Files: `web/src/lib/stores/reader-share.svelte.ts`, `web/src/routes/(application)/_reader/VerseTools.svelte`
Done when: card renders Arabic correctly and downloads; shared links open the right range.

#### F14. Bookmark bulk actions

Build: multi-select on `/bookmarks`: move and delete with an Undo toast.
Files: `web/src/routes/(application)/bookmarks/+page.svelte`, `web/src/lib/bookmarks/store.svelte.ts`
Done when: bulk move and delete work offline and sync; Undo restores them.
Blocked by: S7

#### F15. Voice search

Build: voice input in the palette and `/search` with unsupported, permission and error states. Preserve the Arabic normalization/parity contract. The microphone is blocked today by `Permissions-Policy: microphone=()` in `web/src/hooks.server.ts`, `web/server.ts` and `security_headers.rs`; allow `self` on the web tier only and update `web/scripts/assert-headers.sh`. Browser speech recognition can send audio to the browser vendor, so add a privacy sentence and the A34 ledger reference.
Files: `web/src/lib/components/search/GlobalSearchPalette.svelte`, `web/src/routes/(application)/search/+page.svelte`, `web/src/hooks.server.ts`, `web/server.ts`, `web/scripts/assert-headers.sh`
Done when: control is hidden where unsupported; denied and failed states are handled and tested.

#### F16. Public daily ayah route

Build: indexable daily route from `/quran/random`, which is already deterministic per UTC date and cached to UTC midnight. Source-aware link, share metadata.
Files: `web/src/lib/home/daily-ayah.ts`, `web/src/routes/(marketing)/`
Done when: every visitor sees the same ayah on a UTC day; link preserves translation context.

#### F17. Translation correction report

Build: "Report an issue" on a verse, prefilled with immutable source id and verse key. `/contact` is only a `mailto:` link, so use the A24 report intake with a new `translation` kind, sharing its limits. Never edits a Quran DB.
Files: `web/src/routes/(application)/_reader/VerseTools.svelte`, `rust/backend/api/src/modules/report_v1/*`
Done when: report is stored with source id and verse key; the A24 limits apply (test).
Blocked by: A24

#### F18. RTL polish and localized text variants

Build: fix remaining physical utilities and reader arrows in RTL; localized `.md`/`.txt` representations. Keep UI locale out of reader server cache keys.
Files: `web/src/routes/[slug=marketingText].md/`, `web/src/routes/[slug].txt/`, `web/src/routes/(application)/_reader/`
Done when: logical-properties guard green; Arabic text variants serve Arabic.

#### F19. Recitation audio and player

Build: update the CSP for audio hosts. Reciter/recording catalogue, verse timing, surah and ayah queue, play/pause/seek/next/previous, persistent mini and full player, playback speed, media-session controls, failure/retry, optional offline audio with explicit storage limits, attribution.
Files: `web/src/lib/stores/reader-core.svelte.ts`, `web/src/lib/stores/reader.svelte.ts`, new: `web/src/lib/audio/`
Done when: continuous surah playback survives navigation and backgrounding; failures recover.
Blocked by: O22

#### F20. Memorization repeat engine

Build: verse/range selection, repeat counters, inter-ayah delay, boundaries, cancellation, screen-lock behaviour, saved presets.
Files: new: `web/src/lib/audio/repeat.ts`
Done when: a 3-verse range repeats the set count with the delay and stops cleanly.
Blocked by: F19

#### F21. Word-by-word

Build: canonical word coordinates, Arabic token display, per-word meaning and transliteration, popovers with keyboard and touch, offline delivery, attribution. Tests must prove Quran text is never rewritten.
Files: `web/src/routes/(application)/_reader/VerseRow.svelte`, new: `web/src/lib/quran/words/`
Done when: every token maps to its source word; reader text bytes are unchanged.
Blocked by: O23

#### F22. More tafsir sources

Build: add licensed tafsir beyond `ar.jalalayn` and `ar.muyassar`, each as a new immutable database id in the baked map: verse coverage, source switching, attribution, footnotes, citation links, offline rules, missing-source states.
Files: `web/src/lib/data/translations.json`, `web/src/lib/components/translation/TranslationModal.svelte`
Done when: each new source is credited and missing verses show a clear state.
Blocked by: O24

## Copy

#### C1. Copy truth pass

Build: drop or defer recitation claims in `legal_terms_service_body`, `legal_terms_third_party_body` and `faq_offline_answer` unless F19 shipped. Then sweep landing, reader empty/loading/settings labels, and PWA manifest descriptions: every capability is available now, beta with an entry point, or planned. Remove the unused `auth_merge_*` strings. Recheck translation-source counts against the baked catalogue.
Files: `web/messages/en.json`, `web/messages/ar.json`, `web/messages/auth/en.json`, `web/messages/auth/ar.json`, `web/static/manifest.webmanifest`
Done when: no shipped copy promises a feature that doesn't exist; `en` and `ar` match.
