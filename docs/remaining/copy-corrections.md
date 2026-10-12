# Development copy placeholder ledger

EasyQuran is under active development. Entries below are expected placeholders or stale development
copy—not defects in a released product. Keep this standalone ledger for the pre-release content pass,
when wording should be aligned with the features selected for launch.

> **Updated 2026-10-12 (§9.8 copy pass):** copy now lives in `web/messages/*.json` (the old
> `content.ts` links below are dead). Resolved this pass: **C01** (auth copy scoped to bookmarks +
> reading sessions; notes stay local), **C02** (privacy/terms sync + deletion claims trimmed to the
> real scope — notes never sync, deletion is by request, no native apps), **C06** (FAQ/about rollout
> copy now states 378 sources / 105 languages + two Arabic tafsir sources as shipped), **C07** (FAQ
> backup answer now describes the shipped account sync + offline queue), and the merge-review dialog
> strings moved from a hardcoded English block into `web/messages/auth/*.json` (`auth_merge_*`).
> C05 is resolved (landing bands retired). The §9.8 sync-UI copy landed with it: the sign-in
> note on `/bookmarks` now carries the full BM-05 promise ("Your bookmarks are saved on this device.
> Sign in (free) to keep them safe and see them on your other devices."), and the two §9.5 one-liners
> shipped as `auth_session_expired_banner` ("Session expired — sign in to resume sync") and
> `auth_sign_out_unsynced_note` ("Your unsynced changes are kept on this device for your next
> sign-in"), rendered by `SyncNotices.svelte` behind `AuthModalShell`. ACCT-05 (caption-size sync
> status + offline note above the list) and the ACCT-09 merge toast wording are in via the
> `SyncIndicator`/`auth_merge_result` work. **Still open:** C03 (Terms recitation/tafsir wording),
> C04 (FAQ offline-audio answer), and the C08 sweep of the marketing landing page and reader
> empty/loading labels.

## C01 — Auth sync placeholder — RESOLVED 2026-10-12

The sign-in, registration and auth-dialog copy in `web/messages/auth/{en,ar}.json`
(`auth_sign_in_subheading`, `auth_register_subheading`, `auth_dialog_description`) and
`settings_account_signed_out_note` now promise exactly the shipped scope: bookmarks + reading
sessions sync with a free account; notes stay on the device.

## C02 — Privacy sync and deletion placeholders — RESOLVED 2026-10-12

The privacy (`legal_privacy_summary`, `legal_privacy_accounts_body`, `legal_privacy_device_body`,
`legal_privacy_choices_body`) and terms (`legal_terms_content_body`) strings in
`web/messages/{en,ar}.json` now match reality: bookmarks, folders and reading sessions sync when
signed in; notes never leave the device; no deeds, no native apps. The deletion claim is now "ask us
to delete your account and its data" — there is still no self-service deletion flow in the web app
(`user_v1` exposes only an admin delete), so the stronger wording must wait for that flow.

## C03 — Recitation and tafsir placeholders in Terms

The former `content.ts` copy (now `web/messages/*.json`: `legal_terms_service_body`,
`legal_terms_third_party_body`) describes optional recitation audio and implies
translations/tafsir are credited beside each source.

Actual behavior (2026-10-12): the tafsir half is resolved — two credited Arabic tafsir sources
ship (`ar.jalalayn`, `ar.muyassar` in `web/src/lib/data/translations.json`, offered as
"Commentary (tafsir)"; the FAQ `faq_translations_answer` already advertises them), and the old
sample string in [`quran.ts`](../../web/src/lib/data/quran.ts) (`tafsirFor`) is unreferenced dead
code. The recitation half stands: no production audio player exists.

**Before release:** drop or clearly defer the recitation-audio statements; the tafsir/credit
statements are accurate as shipped.

## C04 — Offline-audio placeholder in FAQ

The former `content.ts` copy (now `web/messages/*.json`, key `faq_offline_answer`) answers the
offline question with:

> Recitation audio needs a connection the first time you play it.

That describes an offline audio cache that does not exist.

**Before release:** make this match the final audio/offline policy. If audio remains deferred, limit
the answer to shipped offline reading behavior.

## C05 — Recitation preview placeholder on landing

[`(marketing)/+page.svelte`](<../../web/src/routes/(marketing)/+page.svelte>) says search, bookmarks,
and recitation “are there when you need them,” then says the full player is “only a preview.” No
working preview was found in production reader code; design variants are not product features.

**Before release:** connect this statement to a working preview/player, or describe recitation as
planned.

## C06 — Full-Quran and translation rollout placeholders — RESOLVED 2026-10-12

The stale "curated selection / coming / on the way" strings in `web/messages/{en,ar}.json`
(`faq_next_answer`, `faq_translations_question`/`_answer`, `faq_account_answer`,
`about_feature_tools`) now state what shipped: all 114 surahs, 378 translation sources in 105
languages (counts rechecked against the baked catalogue on 2026-10-12), and two Arabic tafsir
sources offered as commentary. Hadith, the audio player and the deeds feature stay listed as future
work only.

## C07 — Bookmark backup placeholder — RESOLVED 2026-10-12

`faq_backup_answer` in `web/messages/{en,ar}.json` now describes the shipped behaviour: bookmarks
live in the browser while signed out, sync to the account and other devices once signed in, and
offline changes are kept on the device until the next online sync (§9.5 quarantine).

## C08 — Pre-release content review

Before changing copy, run one feature-truth review across:

- home, About, FAQ, Terms, Privacy;
- login/register/account;
- reader empty/loading/settings labels;
- site metadata/footer blurb;
- app store/PWA descriptions if added.

For every capability, use exactly one of: **available now**, **beta/preview with accessible entry
point**, or **planned**. Design mocks, backend-only primitives, enums, and legal aspirations do not
count as available product features.

Progress 2026-10-12: About, FAQ, Privacy, Terms, the auth modal/register strings, the
merge-review dialog, and the sync surfaces (bookmark sign-in note, session-expired banner,
sign-out unsynced notice — §9.8) now pass the truth test (C01/C02/C06/C07 + `auth_merge_*`).
Not yet swept: the marketing landing page (C03/C04/C05 territory), reader empty/loading/settings
labels, and any future store/PWA descriptions.

## Suggested corrected capability statement

A defensible short description, matching what shipped as of 2026-10-12:

> Read the complete Quran in Uthmani Arabic or one of three other mushaf scripts, and choose from
> 378 translation sources in 105 languages. Search Arabic and translated text, save bookmarks
> (synced with a free account, together with your reading sessions) and notes in this browser,
> resume where you left off, and keep selected reading data available offline. Notes stay on this
> device.

Recheck counts against the baked catalogue when editing user-facing copy; do not derive or hash
Quran bytes at runtime.
