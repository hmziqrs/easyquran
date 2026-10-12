# Account page UX — slim chrome, primitives, localization

Status: **plan, not implemented.** Baseline `32bb4f07` (2026-10-13).

Scope: UI/UX improvements for the chrome-less surfaces (no app header/footer), of which
`/account` is the only true outlier. No behaviour, data, or route changes — presentation and
copy only.

## Findings (measured at baseline)

- **Chrome inventory.** `(marketing)` and `(application)` render full `Nav` + `Footer`. The
  `(auth)` group (login, register, forgot-password, verify-email) uses the deliberate split
  screen: brand aside + `AuthTopBar` on mobile + purpose-scoped footer links. The `auth/`
  OAuth callbacks (success/failure/totp) all render `AuthTopBar` — `auth/+layout.svelte`
  documents why: "Callback pages used to be chromeless: a user landing on a failure had no route
  back into the product." `design/*` is dev-only. **`/account` is the outlier**
  (`web/src/routes/(account)/+layout.svelte`): a bare `Container width="narrow"` + session gate,
  nothing else.
- **/account gaps** (`web/src/routes/(account)/account/+page.svelte`, 327 lines):
  1. **Dead end.** No brand, no exit — the only way back to the product is browser back. Every
     other non-reader surface has the one-slim-row rule ("Auth chrome is one slim row… never a
     stacked identity block" — `AuthTopBar.svelte` doc comment).
  2. **Hand-rolled blocks.** Session rows and the Passkeys card use inline
     `rounded-xl border border-line-2 bg-bg-2` instead of the sanctioned `Card` primitive
     (`rounded-lg border-border bg-surface p-6`; design-system §61).
  3. **Inconsistent anonymous CTAs.** "Sign in" opens the auth modal
     (`authModal.show("login")`), "No account? Create one" routes to `/register` — two different
     interaction models side by side.
  4. **No localization.** The page is 100% hardcoded English in an en/ar app; every other auth
     surface resolves copy via `messages/auth/{en,ar}.json` + `auth-copy.ts`.
- Root layout treats `account` as a non-reader first segment (non-reader surfaces set in
  `web/src/routes/+layout.svelte`) — a slim bar, not full Nav+Footer, is the established
  treatment for this class of page.

## Build

1. **Slim top bar on `(account)`.** Reuse `AuthTopBar` in
   `web/src/routes/(account)/+layout.svelte` exactly as `auth/+layout.svelte` does: `Brand` →
   `marketingHomeHref(locale)` left, one "Read Quran" exit → `marketingReaderHomeHref(locale)`
   right, `min-h-11` touch targets, logical properties only. The loading gate stays inside the
   shell so even "Checking your session…" renders the bar — the exit exists in prerendered HTML
   before JS runs (same rationale as the callback fix).
2. **Card primitive.** Replace the hand-rolled session-row and Passkey-card styling with `Card`;
   keep inner markup and the "This session" pill (`rounded-pill bg-accent-soft`). No new
   primitives, no shadow/elevation (§14).
3. **One CTA model for the anonymous view.** Both actions go through the auth modal
   (`authModal.show("login")` / register tab) so the user stays on `/account` and the page
   re-renders on auth; if the modal has no register tab, fall back to `returnTo=/account` links
   for both, matching the `(auth)` pages. Never one of each.
4. **Localization.** New `account` namespace: `web/messages/account/{en,ar}.json` (Arabic copy
   consistent with `messages/auth/ar.json` vocabulary) + `web/src/lib/i18n/account-copy.ts`
   resolver following the `auth-copy.ts` pattern; the layout sets `lang`/`dir` on its root like
   `(auth)` does. Claim the `account_` prefix in `web/i18n-namespaces.json` — every catalog key
   must be claimed exactly once or generation fails — then regenerate the barrel.
5. **Copy/layout polish.** Heading scale matches the `(auth)` pages; `FreshAuthControls` and
   `ConnectedAccounts` untouched; loading view unchanged apart from sitting under the bar.

## Acceptance

- `/account` (authenticated, anonymous, and loading) shows the slim bar with brand + working
  exit; prerendered HTML contains the exit without JS.
- Session rows and the Passkeys card render via `Card`; no inline `border-line-2 bg-bg-2`
  block styles remain in the page.
- Anonymous view uses a single CTA model.
- `en` and `ar` both fully localized (no hardcoded strings left in the page); `lang`/`dir`
  correct for both locales.
- All three gates green (`bun run check`, `bun run lint`, `bun run test` from `web/`), including
  the logical-properties and i18n namespace guards.

## Touch

`web/src/routes/(account)/+layout.svelte`,
`web/src/routes/(account)/account/+page.svelte`,
`web/messages/account/{en,ar}.json` (new),
`web/src/lib/i18n/account-copy.ts` (new),
`web/i18n-namespaces.json`,
regenerated `web/src/lib/i18n/m/account.ts`.

## Not doing

- No `Nav` + `Footer` on `/account` — the one-slim-row chrome is the deliberate direction for
  non-reader surfaces (`fbf5edcd`, `10220dd7`).
- No changes to `(auth)`, `auth/` callbacks, or `design/` — already patterned.
- No behaviour/data changes: session management, 2FA, passkeys, connected accounts stay as is.
