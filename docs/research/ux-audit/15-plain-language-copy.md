# 15 · Plain language: words, names and a glossary

[← Back to the index](README.md)

> **Question this page answers:** *Would a non-technical reader — say, a grandparent reading after Fajr — understand every word on screen?*

The app is built for non-technical people. Many labels were written by and for developers. This page collects them in one place with suggested replacements. English is shown; the Arabic equivalents need the same treatment (see [10 · Arabic and RTL](10-arabic-and-rtl.md)).

## 1 · Rewrites

| Where | Current | Problem | Suggested |
| --- | --- | --- | --- |
| Settings → Storage | Browser storage limit for this site: 10.0 GB — computed by your browser from your disk, not a fixed allowance. | Browser internals | *(remove)* — show "Using 3.9 MB on this device" |
| Settings → Storage | Item sizes come from easyquran's own records and may differ from the browser's estimate. | Implementation detail | *(remove)* |
| Settings → Storage | Persistent storage · Not granted — the browser may clear downloads under pressure. · Request again | Jargon | **Keep downloads safe** — "Your phone may delete downloaded text when it runs low on space." · **Keep them** |
| Settings → Storage | Uthmani · On disk | Two jargon words | **Qur'an text (Uthmani script) · Downloaded** |
| Settings → Storage | Offline pack · Off — download every route for offline reading. | "route" | **Read without internet** — "Download everything so the whole app works offline." |
| Settings → Storage | Unused translations are removed automatically after 30 days or when downloads exceed 256 MB. | Dense | "Translations you haven't opened in 30 days are removed to save space." |
| Settings → Appearance | Custom colours · Background / Accent / **Pop** · preset | Designer vocabulary | Hide under "Developer options" |
| Settings → Appearance | Surfaces, hairlines and text steps are derived from the background; the soft washes from the accent. | Design-system vocabulary | *(remove)* |
| Settings → Appearance | Copy CSS | Developer action | *(hide)* |
| Settings → Appearance | Magenta — accent over a warm reading page. | Outdated | "Magenta" |
| Settings → Privacy | Performance — Reloads the page to apply — Firebase Performance can only be switched at startup | Vendor name | **Share speed reports** — "Takes effect next time the app opens." |
| Settings → Privacy | Notifications unavailable (not configured). | Developer message | *(hide row when unavailable)* |
| Settings → Reading | 33px · 17px | Units | Small · Medium · Large · Extra large (with preview) |
| Settings → Reading | KFGQPC Uthman Taha Naskh v1 Bold | File name | "Madinah Mushaf style (bold)" |
| Floating panel | SETTINGS (code font) | Second "Settings" | *(remove panel)* |
| Reader | Ayah-by-Ayah / Ayahs / Reading | Inconsistent | **Verse by verse** / **Continuous** |
| Reader | 2:1 (11 px mono) | Notation | "Verse 1" |
| Reader | SURAH 2 · PAGE 1 OF 48 | Local page count confuses | "Surah 2 · Medinan · 286 verses" |
| Reader → verse | Note & tafsir (pencil) | Two things, one pencil | **Add note** · **Read tafsir** |
| Reader → verse | Sample commentary for Surah 2, 2:1 — in the full app this slot carries… | Placeholder | *(hide until real)* |
| Translations modal | Up to 5 translations alongside the primary. | "primary" | "Show up to 5 more translations under the main one." |
| Juz list | 30 ajzā' | Transliterated plural | "30 parts (juz)" |
| Juz list | 1:1 - 2:141 (mono) | Notation | "Al-Fātiḥah 1 → Al-Baqarah 141" |
| Juz list | Quarter 1 … 4 | Unexplained | "¼ · ½ · ¾ · end" + one-line explainer |
| Pages list | 604 pages (mono) | Code font | "604 pages" in UI font + "Go to page" field |
| Search | Search the Quran and translations — pick translations below. | Points nowhere | "Type a word, a surah name, or a verse like 2:255." |
| Search | No results. | No help | "No verses contain '{q}' in the selected texts. Try another word or add a translation." |
| Palette | Try 2:255, juz 5, page 100 (mono) | Code | "Try a surah name, a verse like 2:255, or 'juz 5'." |
| Home | Resume instantly | Unclear | (Settings) "Open where I left off when I start the app" |
| Yours | Jump | Unclear | **Continue** |
| Bookmarks | Al-Baqarah Ayah 1 | Mixed format | "Al-Baqarah · verse 1" |
| Bookmarks | Stored in this browser | Vague | "Saved on this device" |
| Error page | ← Back to EasyQuran (mono) | Style + brand | "Back to reading" button |
| Account | Loading your account… Checking your session. | Tech | "Just a moment…" (+ timeout message, see STATE-03) |
| Login | OR CONTINUE WITH | Mono caps | "or" |
| Footer | @@hmziqrs | Typo | "@hmziqrs" |
| About | Arabic text only, for now | Outdated | Mention translations |

## 2 · One word for each thing

| Concept | Use | Avoid mixing with |
| --- | --- | --- |
| The book | **Qur'an** (with apostrophe, everywhere including placeholders) | Quran |
| A chapter | **Surah** (plural **Surahs**) | chapter (fine in explanations only) |
| A verse | **verse** in UI labels; *ayah* may appear in explanations | Ayah, ayahs (landing cards), "Ayah 1" |
| 1/30 of the Qur'an | **Juz** (plural "juz" or "parts") | ajzā' |
| Printed page | **Page** (of 604) | local "Page 1 of 48" |
| Saved verse | **Bookmark** | — |
| Personal hub | **My Qur'an** (Arabic **قرآني**) — or keep "Yours" but use it everywhere | Yours / Bookmarks / لك mix |
| Resume action | **Continue** | Jump |
| Explanation text | **Tafsir** (commentary) | translation |
| Brand | **easyquran** (lowercase, as in the logo) | EasyQuran |

## 3 · One spelling for each surah

Use one transliteration per surah everywhere (see [LIST-01](06-browse-lists.md#list-01--every-surah-has-two-or-three-english-spellings)). Keep alternates ("Al-Faatiha", "Yaseen", "Al-Fātiḥah") as **search aliases only**. Put the canonical list in the catalogue module and add a test that no component renders any other.

## 4 · Tone checklist for new copy

- Could a 70-year-old who has never used an app understand it? If not, rewrite.
- No product, vendor, browser or file names (Firebase, OPFS, route, KFGQPC, CSS).
- Say what happens and what to do next ("You're offline. The Arabic Qur'an still works.").
- One sentence per message. Sentence case. No code font.
- Every English string has an Arabic twin in `messages/*/ar.json` before merge.
