# Account UX

[Back to index](../README.md)

Account page chrome and localization.

#### U1. Account page chrome, primitives, localization

Build: (1) Add `AuthTopBar` to the `(account)` layout exactly as `auth/+layout.svelte` does: `Brand` → `marketingHomeHref(locale)` left, one "Read Quran" exit → `marketingReaderHomeHref(locale)` right, `min-h-11` targets, logical properties only, loading gate inside the shell. (2) Replace hand-rolled session rows and the Passkeys card with `Card`; keep inner markup and the "This session" pill; no new primitives or elevation. (3) Anonymous view uses one CTA model: both actions through the auth modal; if it has no register tab, `returnTo=/account` links for both. (4) New `account` namespace: `web/messages/account/{en,ar}.json`, `account-copy.ts` following `auth-copy.ts`, layout sets `lang`/`dir`, claim `account_` in `web/i18n-namespaces.json`, regenerate the barrel. (5) Heading scale matches `(auth)` pages. No `Nav`/`Footer`, no behaviour or data changes.
Files: `web/src/routes/(account)/+layout.svelte`, `web/src/routes/(account)/account/+page.svelte`, `web/i18n-namespaces.json`, new: `web/messages/account/{en,ar}.json`, new: `web/src/lib/i18n/account-copy.ts`
Done when: bar + exit show on authenticated, anonymous and loading views, and the exit is in prerendered HTML without JS; no inline `border-line-2 bg-bg-2` left; `en` and `ar` fully localized with correct `lang`/`dir`.
