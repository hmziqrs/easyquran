# Fork B merge notes — remaining screens, flows, signed-in

Owner files: [19-remaining-screens-and-flows.md](../19-remaining-screens-and-flows.md), [20-signed-in-experience.md](../20-signed-in-experience.md), `screenshots/screens/` (13), `screenshots/flows/` (9), `screenshots/signed-in/` (9).

Totals: **26 new findings** — 1 P0, 6 P1, 16 P2, 3 P3.

## 1 · New issues

| ID | Title | Sev | File | Anchor |
| --- | --- | --- | --- | --- |
| SCR-01 | Legal pages say "placeholder text" and promise things the app doesn't do | P0 | 19-remaining-screens-and-flows.md | `#scr-01--legal-pages-say-placeholder-text-and-promise-things-the-app-doesnt-do` |
| SCR-02 | Legal pages are hard to read | P2 | 19-remaining-screens-and-flows.md | `#scr-02--legal-pages-are-hard-to-read` |
| SCR-03 | Auth side pages look like a different product | P2 | 19-remaining-screens-and-flows.md | `#scr-03--auth-side-pages-look-like-a-different-product` |
| SCR-04 | Landing lower sections: wrong text colour, a self-link, a very long list | P2 | 19-remaining-screens-and-flows.md | `#scr-04--landing-lower-sections-wrong-text-colour-a-self-link-a-very-long-list` |
| SCR-05 | "Nothing tracking what you read" while analytics is on by default | P1 | 19-remaining-screens-and-flows.md | `#scr-05--nothing-tracking-what-you-read-while-analytics-is-on-by-default` |
| SCR-06 | Every reader type ends differently — and the end of the Qur'an is a small link | P2 | 19-remaining-screens-and-flows.md | `#scr-06--every-reader-type-ends-differently--and-the-end-of-the-quran-is-a-small-link` |
| SCR-07 | Special Qur'an moments aren't explained | P3 | 19-remaining-screens-and-flows.md | `#scr-07--special-quran-moments-arent-explained` |
| SCR-08 | The reading-mode dialog uses its own button and list styles | P3 | 19-remaining-screens-and-flows.md | `#scr-08--the-reading-mode-dialog-uses-its-own-button-and-list-styles` |
| FLOW-01 | "Clear cached pages & data" — no confirmation, no visible result | P1 | 19-remaining-screens-and-flows.md | `#flow-01--clear-cached-pages--data--no-confirmation-no-visible-result` |
| FLOW-02 | Offline pack: no progress, no "ready" moment, developer wording | P2 | 19-remaining-screens-and-flows.md | `#flow-02--offline-pack-no-progress-no-ready-moment-developer-wording` |
| FLOW-03 | Search has a second, different translation picker — and it needs two steps | P1 | 19-remaining-screens-and-flows.md | `#flow-03--search-has-a-second-different-translation-picker--and-it-needs-two-steps` |
| FLOW-04 | Storage download rows: jargon chips and an unexplained disabled button | P2 | 19-remaining-screens-and-flows.md | `#flow-04--storage-download-rows-jargon-chips-and-an-unexplained-disabled-button` |
| FLOW-05 | Shared verses carry no link back | P2 | 19-remaining-screens-and-flows.md | `#flow-05--shared-verses-carry-no-link-back` |
| FLOW-06 | ⌘K "go to verse" lands on the page, not the verse; deep links don't highlight | P2 | 19-remaining-screens-and-flows.md | `#flow-06--k-go-to-verse-lands-on-the-page-not-the-verse-deep-links-dont-highlight` |
| FLOW-07 | Switching language throws away your reading position | P2 | 19-remaining-screens-and-flows.md | `#flow-07--switching-language-throws-away-your-reading-position` |
| FLOW-08 | Many bookmarks become an unsorted wall | P2 | 19-remaining-screens-and-flows.md | `#flow-08--many-bookmarks-become-an-unsorted-wall` |
| FLOW-09 | Update and notification toasts cover the header and have tiny controls | P2 | 19-remaining-screens-and-flows.md | `#flow-09--update-and-notification-toasts-cover-the-header-and-have-tiny-controls` |
| ACCT-01 | Signing out silently removes your account bookmarks from the device | P1 | 20-signed-in-experience.md | `#acct-01--signing-out-silently-removes-your-account-bookmarks-from-the-device` |
| ACCT-02 | The Account page is a bare developer screen | P1 | 20-signed-in-experience.md | `#acct-02--the-account-page-is-a-bare-developer-screen` |
| ACCT-03 | Nothing shows that you're signed in | P2 | 20-signed-in-experience.md | `#acct-03--nothing-shows-that-youre-signed-in` |
| ACCT-04 | Bookmark folders: squashed add button, disguised dropdowns, repeated labels | P2 | 20-signed-in-experience.md | `#acct-04--bookmark-folders-squashed-add-button-disguised-dropdowns-repeated-labels` |
| ACCT-05 | Sync status is good but small | P3 | 20-signed-in-experience.md | `#acct-05--sync-status-is-good-but-small` |
| ACCT-06 | Auth forms: tiny errors, mixed icons, hidden password rule | P2 | 20-signed-in-experience.md | `#acct-06--auth-forms-tiny-errors-mixed-icons-hidden-password-rule` |
| ACCT-07 | Verification and reset codes don't match, and the copy talks to strangers | P2 | 20-signed-in-experience.md | `#acct-07--verification-and-reset-codes-dont-match-and-the-copy-talks-to-strangers` |
| ACCT-08 | Server and configuration errors reach readers raw or mislabelled | P1 | 20-signed-in-experience.md | `#acct-08--server-and-configuration-errors-reach-readers-raw-or-mislabelled` |
| ACCT-09 | Signing in merges local bookmarks — say so, and say what doesn't sync | P2 | 20-signed-in-experience.md | `#acct-09--signing-in-merges-local-bookmarks--say-so-and-say-what-doesnt-sync` |

Doc 19 also ends with a "What works well" list (offline after download, deep links, phone copy feedback, no theme flash, bookmarks survive cache clearing); doc 20 has the same for signed-in (merge on sign-in, offline queue, folder-delete confirmation, clear auth messages).

## 2 · Extensions and corrections to existing findings

| Existing | Add |
| --- | --- |
| STATE-01 | More raw `text/plain` "Not found" URLs: a **trailing slash** (`/en/app/al-fatihah/`), **capital letters** (`/en/app/AL-FATIHAH`), `/en/app/juz/0`, `/en/app/page/0`, `/en/app/al-baqarah/page/0`, unknown translators (`/en/app/al-baqarah/t/en/nope`, `…/t/xx/yy`), `/en/app/t/en/sahih/page/605`. The trailing slash is what some chat apps add to shared links; normalise it (and lowercase slugs) with a 308. |
| STATE-02 / RTL | `/ar/nope` renders the **English, left-to-right** error page (`lang="en"`), and "← Back to EasyQuran" goes to the English home. |
| AUTH-01 | Also applies to `/forgot-password`, `/verify-email`, `/auth/[provider]/success|failure`. After sign-in/sign-up the app always goes to `/en/app` (or `/verify-email`) — e.g. signing in from Bookmarks lands on Home. After Sign out it lands on `/login`. |
| AUTH-02 | Forgot password and Verify email also lack the brand block; the forgot-password email field has no icon/placeholder. |
| SET-02 | Offline pack status also says "On — 2613 routes stored." and "2613 routes · 14.3 MB· saved 9/29/2026" (count twice, missing space, US date). Download rows show chips "Arabic · On disk · In use". |
| SET-04 | Evidence for analytics-on-by-default: production build sends Google Analytics `g/collect` (`tid=G-ZYL6HY24W6`) and Firebase Installations requests on `/en/app/al-kahf` — contradicts landing copy (SCR-05). |
| SRCH-04 | ⌘K palette Arabic results start at the left edge in LTR rows (`screenshots/flows/palette-arabic-alignment.webp`). |
| TR-02 | The reading-mode dialog is the only UI that names the main translation ("Saheeh International"). |
| TR-03 / VIS-05 | Reading-mode dialog: `rounded-lg` buttons, flags, `shadow-lg` (SCR-08). The search page's picker is a different component (FLOW-03). |
| BM-02 | Anonymous bookmarks are stored as `{ "2:255": true }` — no timestamp, so "sort by date" needs a schema change (FLOW-08). |
| BM-04 | Signed-in "Remove" also has no undo; removal while offline disappears instantly (queued). |
| VIS-01 | Register form mixes a line icon (Name) with filled icons (Email/Password) in the same form. |
| VIS-07 | Bookmarks group headings ("UNFILED", "DAILY READING") and offline-pack status lines use code font / wide tracking. |
| MKT-02 / copy-corrections C01–C07 | Legal "placeholder text" label and the three contact identities (salam@easyquran.app, easyquran.fyi, hmziqrs@gmail.com) are new (SCR-01). Notes saved before sign-in are not moved into the account (ACCT-09). |
| RDR-01 | The floating panel also has a "Data & privacy" section below Copy CSS/Reset (visible when scrolled); clicking any `aria-expanded=false` button on FAQ opens it — the floating trigger is in the tab/expander flow of every page. |

## 3 · Roadmap additions

| Wave | ID | Effort |
| --- | --- | --- |
| 1 (P0) | SCR-01 legal copy review + single contact | S (copy) + legal review |
| 2 (P1) | SCR-05 analytics copy or opt-in + path scrubbing | S–M |
| 2 | FLOW-01 "Free up space" rename, confirm, refreshed numbers | S |
| 2 | FLOW-03 one translation picker (reuse reader modal), one-step add | M |
| 2 | ACCT-01 sign-out keep-a-copy / explanation | S–M |
| 2 | ACCT-02 account page in app layout, human sessions, change password, delete account | M–L |
| 2 | ACCT-08 enabled-providers list, OAuth error redirect, honest error mapping | S–M |
| 3 (P2) | SCR-02, SCR-03, SCR-04, SCR-06, FLOW-02, FLOW-04, FLOW-05, FLOW-06, FLOW-07, FLOW-08, FLOW-09, ACCT-03, ACCT-04, ACCT-06, ACCT-07, ACCT-09 | S–M each |
| 4 (P3) | SCR-07, SCR-08, ACCT-05 | S |

Suggested guard: a route test that every browser 404 under `/en|ar/app` returns HTML with the app header (covers the STATE-01 extensions), and a unit test that OAuth start routes never return JSON to a navigation request.

## 4 · Coverage

**Checked:** `/forgot-password` (+ code step), `/verify-email` (direct, wrong code, resend), `/auth/google/success`, `/auth/google/failure` (±`error`), `/privacy`, `/terms`, `/faq` (accordion), `/contact` (phone), full landing (desktop, phone, dark, Arabic `/ar/`), juz readers 2/15/30 (top, end, prev/next), page readers 2/300/604 (top, end), `/en/app/al-baqarah/page/2` and `/page/48`, end nav for every reader type, reading-mode dialog (desktop + phone), At-Tawbah, sajdah verse 7:206, `/en/app/juz` and `/en/app/pages` on phone, 14 404/redirect URL variants, legal line length. Flows: offline pack download (dev fails; prod build succeeds in ~2 s) then offline reading of page 400, a translated page and Arabic search; "Clear cached pages & data" (no confirm; bookmarks/notes kept; pack still reported on); translation download from the reader modal → Storage row; search page picker (tick vs Download, results); update toast (rendered from component markup — SW update not triggerable); copy and share on phone emulation; ⌘K verse/juz/Arabic queries and Enter; deep links `#ayah-2-255`; language switch position; 50 seeded bookmarks; long note; theme pre-paint (no flash observed). Signed-in: register (empty, weak, mismatch, existing email with same password → signs in), sign-in errors (wrong password, unknown email, invalid email/short password), verify email, resend, forgot-password request, `/account` (desktop, phone), Settings → Account, header state, Bookmarks sync states (synced, offline, pending), folders (create, move, delete confirm), anonymous→account merge, sign out.

**Could not / not done:**
- Receiving real emails (no SMTP locally) — verification was completed by marking only the test user verified in `rust/data/easyquran.db` (noted in doc 20). The test account `ux-audit.test+1@example.com` (user id 1) remains in the local dev DB with ~10 sessions from the audit; delete it when convenient.
- Real OAuth providers (not configured locally) — only the unconfigured-provider failure was observable.
- A real service-worker update prompt (UpdateToast was rendered from its markup) and push notifications.
- Password change / account deletion / 2FA / passkey enrolment flows were not exercised (2FA/passkey buttons exist; change password and delete account don't).
- Registration from :5391 fails because the API CORS allowlist only includes :5173 (`CONSUMER_PORT`); the audit used a local proxy on :5173 (`scratchpad/fork-b/proxy5173.mjs`, stopped after the audit).
- Offline-pack and analytics checks used the production build another fork was serving on :5392.
