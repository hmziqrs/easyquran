# Development copy placeholder ledger

EasyQuran is under active development. Entries below are expected placeholders or stale development
copy—not defects in a released product. Keep this standalone ledger for the pre-release content pass,
when wording should be aligned with the features selected for launch.

## C01 — Auth sync placeholder

Current sign-in copy says:

> Sign in to sync your bookmarks and reading progress.

Current registration copy says:

> A free account syncs your bookmarks, notes and reading place across devices.

Sources:

- [`web/messages/auth/en.json`](../../web/messages/auth/en.json) (`auth_sign_in_subheading`,
  `auth_register_subheading`)

Actual behavior: bookmark sync is real since 2026-08-27 (`POST /bookmark/v1/sync`), so the bookmarks
claims are accurate. Notes and reading place/progress remain device-local only.

**Before release:** scope sign-in/registration copy to bookmarks until notes/progress sync ships.

## C02 — Privacy sync and deletion placeholders

[`content.ts`](../../web/src/lib/data/content.ts) says:

- bookmarks, notes, and deeds link to accounts and sync across devices/native apps;
- users can delete account and data at any time;
- local data leaves device if sync is enabled.

Actual behavior: bookmarks sync for signed-in users (2026-08-27); notes, deeds, and native apps do
not exist; account page has no self-service deletion flow.

**Before release:** trim the synced list to bookmarks and document which data is server-side.

## C03 — Recitation and tafsir placeholders in Terms

[`content.ts`](../../web/src/lib/data/content.ts) describes optional recitation audio and implies
translations/tafsir are credited beside each source.

Actual behavior: no production audio player exists. Translation catalogue includes source details,
but tafsir UI renders an explicit sample string from
[`quran.ts`](../../web/src/lib/data/quran.ts), not a credited tafsir source.

**Before release:** retain these statements only if production recitation and credited tafsir ship.
Otherwise scope the service description to available sources and features.

## C04 — Offline-audio placeholder in FAQ

[`content.ts`](../../web/src/lib/data/content.ts) answers the offline question with:

> Recitation audio needs a connection the first time you play it.

That describes an offline audio cache that does not exist.

**Before release:** make this match the final audio/offline policy. If audio remains deferred, limit
the answer to shipped offline reading behavior.

## C05 — Recitation preview placeholder on landing

[`(marketing)/+page.svelte`](../../web/src/routes/(marketing)/+page.svelte) says search, bookmarks,
and recitation “are there when you need them,” then says the full player is “only a preview.” No
working preview was found in production reader code; design variants are not product features.

**Before release:** connect this statement to a working preview/player, or describe recitation as
planned.

## C06 — Full-Quran and translation rollout placeholders

Stale text says:

- the reader holds a curated selection and all 114 chapters are coming;
- the product is Arabic-only;
- selected translations are on the way;
- “Hadith, tafsir & translations” are one roadmap item.

Sources:

- [`(marketing)/+page.svelte`](../../web/src/routes/(marketing)/+page.svelte)
- [`about/+page.svelte`](../../web/src/routes/(marketing)/about/+page.svelte)
- [`content.ts`](../../web/src/lib/data/content.ts)

Actual behavior: all 114 surahs and 378 translation sources across 105 languages are shipped. Real
tafsir and Hadith remain absent.

**Before release:** advertise current breadth accurately; keep Hadith/real tafsir as future work.

## C07 — Bookmark backup placeholder

FAQ correctly says bookmarks live in browser and cloud sync/export are coming. Use it as the current
development-state description when reviewing the other placeholders.

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

## Suggested corrected capability statement

Until new work ships, a defensible short description is:

> Read the complete Quran in Uthmani Arabic or one of three other mushaf scripts, and choose from
> 378 translation sources in 105 languages. Search Arabic and translated text, save bookmarks
> (synced with an account) and notes in this browser, resume where you left off, and keep selected
> reading data available offline. Other reading data does not yet sync across devices.

Recheck counts against the baked catalogue when editing user-facing copy; do not derive or hash
Quran bytes at runtime.
