# 09 — Locale-suggest popup

Status: **draft plan, not implemented.** Owner asked for a one-time, client-side
"this app is also in your language — switch?" popup. Nothing in this plan may
auto-redirect, set cookies, or move locale truth out of the URL.

## 0. Problem and the guarantee

Users whose browser language differs from the page they landed on (an Arabic-primary
browser user landing on `/`, an English-primary user opening a shared `/ar/app/...`
link) get no hint that a UI locale they read better exists. The classic fix —
IP/geo → forced locale — is the exact anti-pattern to avoid: a user in Pakistan with
an English browser must never be pushed Urdu (or Arabic) UI.

**The guarantee this plan is graded on:** `navigator.languages = ["en-PK", "en", "ur"]`
on `/` or `/en/app/**` → no popup, ever. Detection reads the *browser's own ordered
preference list*, takes the first entry that matches a supported UI locale, and only
speaks up when that first-match differs from the current URL locale. No IP, no geo,
no header negotiation.

Signal research summary (2026-08): `Accept-Language` and `navigator.languages` are the
same browser setting viewed server/client-side; Chrome's Accept-Language Reduction is
truncating the header (and eventually the JS list) to one entry for fingerprinting
protection, and Brave farbles both. Consequences baked into this plan: detect
**client-side only** via `navigator.languages` (works identically with one entry),
never build server logic on the header, and treat detection as a first-visit hint
that an explicit dismissal permanently overrides.

## 1. Non-negotiables (why each)

1. **URL stays the sole locale source of truth.** Paraglide strategy is
   `["url", "baseLocale"]`; `docs/quran-system.md` Part 2 owns this. The popup only
   *points* at a localized URL.
2. **No cookies. Ever.** `web/src/hooks.server.ts` bypasses the translation SSR disk
   cache for any request carrying cookies (`requestHasCookie`) and flips responses to
   `private, no-store`. A locale cookie would nuke cache hit rate for every visitor
   who saw the popup. Persistence is `localStorage` only, read/written exclusively on
   the client.
3. **No server changes.** Zero diffs in `hooks.server.ts`, `hooks.ts` (reroute),
   prerender config, or disk-cache keys. The popup is invisible to SSR, prerendered
   HTML, and `Vary` semantics.
4. **Suggest, never redirect.** Industry consensus (NN/g, Smashing, MediaWiki ULS):
   detection → dismissible suggestion → persisted choice. Auto-redirect breaks shared
   links, SEO clarity, and is the "app decided for me" feel we are avoiding.
5. **"Don't ask again" is permanent per locale.** Any interaction — accept, dismiss,
   Escape, overlay click — resolves that target locale forever on that browser
   (§4). The only reset paths are clearing site data or a future explicit reset
   control (§9, open question 3).
6. **UI locale stays independent from Quran translation source** (feature-gap
   catalogue L01 constraint). Phase 2's translation offer must not touch the UI
   locale, and vice versa.

## 2. Detection spec

### 2.1 First-match algorithm

Reuse the generated paraglide runtime helper — it already implements exactly this and
is imported app-code style elsewhere (`getLocale`, `localizeHref` come from the same
module):

```ts
import { extractLocaleFromNavigator, localizeHref, deLocalizeHref } from "$lib/paraglide/runtime";
import { isUiLocale, type UiLocale } from "$lib/i18n/locales";

function suggestedUiLocale(): UiLocale | null {
  const hit = extractLocaleFromNavigator(); // full tag then base tag, in navigator.languages order
  return isUiLocale(hit) ? hit : null;
}
```

Semantics (from `web/src/lib/paraglide/runtime.js`): iterate `navigator.languages` in
order; for each entry try the full tag (`ar-SA` → `ar`) then the primary subtag
(`en-PK` → `en`) against `locales = ["en","ar"]`; first match wins. So
`["en-PK","en","ur"]` → `en` (Urdu never surfaces — correct), `["ar","en"]` → `ar`,
`["ur-PK","en"]` → `en` (Urdu unsupported as UI locale, falls through to `en`),
`["fr"]` → `undefined` → no popup. Works unchanged after Chrome's reduction to a
single entry. Brave's farbled `en-US` just yields `en` — worst case a pointless-but-
dismissible English suggestion on `/ar`, which the current-locale check (§2.2) already
suppresses.

### 2.2 Current-locale resolution

Do **not** use `getLocale()` — `$lib/i18n/app-locale.ts` documents that prefix-less
URLs (`/app/settings`, `/about`) fall back to `en` without the middleware running.
Derive from the pathname instead, mirroring `marketingLocaleFromPath`:

```ts
function currentUiLocale(pathname: string): UiLocale {
  return pathname === "/ar" || pathname.startsWith("/ar/") ? "ar" : "en";
}
```

### 2.3 Target-URL construction

Locale switches in this codebase are full-reload links (`data-sveltekit-reload`) —
`documentElement.dir` is only ever set server-side, so client-side `goto()` would
leave `dir` stale. The accept action renders an `<a>` built from paraglide's own
patterns, which handle all three `urlPatterns` (`/` ↔ `/ar/`, `/app/**` ↔
`/{en,ar}/app/**`) and preserve query + hash:

```ts
const targetHref = localizeHref(deLocalizeHref(location.pathname + location.search + location.hash), { locale: target });
```

wrapped with `publicHref(...)` like every other locale link (`Nav.svelte`,
`MarketingHeader.svelte`). This deliberately goes through paraglide rather than the
`surah*For(ctx,…)` family: those build *canonical* reader hrefs from route context
the root-layout shell doesn't own, and `localizeHref(deLocalizeHref(...))` round-trips
any localized app path — including translated-reader routes, so
`/en/app/al-fatihah/t/en/sahih` → `/ar/app/al-fatihah/t/en/sahih` keeps the
translation context intact (the nav-guard's spirit; no hand-built `/app/` strings).

## 3. Trigger rules and the edge-case matrix

Evaluate once, client-side, from the root layout after paint (§5.3). Show the popup
only when **every** row below passes:

| # | Condition | Rationale / edge case covered |
| --- | --- | --- |
| 1 | `navigator.languages` is a non-empty array | Empty/undefined (some embeds, privacy hardening) → silently no popup. |
| 2 | First-match locale (§2.1) exists | Unsupported primary (`ur`, `fr`, …) → nothing to suggest. The Pakistan guarantee. |
| 3 | First-match ≠ current URL locale (§2.2) | Already-visited-in-right-locale users are never nagged. |
| 4 | Target locale not in the resolved map (§4) | "Don't ask again" — the user's explicit requirement. Permanent per locale. |
| 5 | Current surface has a localized variant: pathname is `/`, `/ar/`, or starts with `/{en,ar}/app/` | Marketing subpages (`/about`, `/faq`, …) have **no** `ar` variant (`urlPatterns` covers only `/` and `/app/**`). On those pages: don't show *and don't record anything* — first landing on `/about` must not burn the one chance; the popup still fires on a later `/` or `/app/**` visit. |
| 6 | No other modal open at fire time (`authModal.open`, `commandPalette.open`) | Cross-guard precedent: `AuthModalShell.svelte` closes the palette when auth opens. If busy, retry once after the modal closes; never stack. |
| 7 | Storage write works | If `localStorage` throws (private mode edge), still show — worst case it re-asks next session. Never crash. |

Surfaces where condition 5 fails also include auth/account/design routes — same rule:
skip silently, record nothing.

### The dismissal contract (user's headline edge case)

- Pressing ✕, "Not now", Escape, or clicking the overlay → write
  `resolved[target] = "dismissed"` → that locale is never suggested again on this
  browser, even after browser-language changes, reinstalls of the app shell, or
  future visits from other entry pages.
- Accepting → `resolved[target] = "accepted"` → navigate; also never suggested again
  (a user who accepted `ar` and later deliberately reads `/en/app` must not be nagged
  back).
- Showing the popup records **nothing**. Only interaction persists. A crash between
  show and click leaves the state clean for next visit.
- Re-check the resolved map at fire time (after the §5.3 delay), not just at mount —
  covers the two-tab race where the other tab already dismissed.

## 4. Storage schema

One new key, existing conventions (`web/src/lib/storage`: `readJSON`/`writeJSON` are
browser-guarded + try/catch; version-checked via `isFutureSchema`; decoders
`asObject`/`asStringRecord`; cross-tab `onStorageKey`):

```
easyquran.localeSuggest = { "v": 1, "resolved": { "ar": "accepted" | "dismissed" } }
```

- Only locales the user actually interacted with appear as keys. Empty map = never
  asked anything.
- Unknown locales / malformed values decode to absence (decoder tolerance pattern
  from `safe-storage.ts` + `decoders.ts`); a future `v: 2` written by a newer build
  is ignored via `isFutureSchema(raw, 1)` — the popup then behaves as first-visit,
  which is safe.
- Name follows the `easyquran.<area>[.<sub>]` convention (`easyquran.prefs`,
  `easyquran.reader.source`, …). Window-event style (`easyquran:localeSuggest`) if
  cross-tab reactivity is wanted; `onStorageKey` gives it for free.
- Not part of `SettingsDoc` in v1 (`settings-document.ts` is a local aggregate, not
  synced); cross-device sync is open question 4.

## 5. UI spec

### 5.1 Shape

Follow the two established patterns exactly:

- **Mount:** root layout `web/src/routes/+layout.svelte`, sibling of
  `<NotificationToast /> <UpdateToast /> <DownloadBar /> <GlobalSearch />
  <AuthModalShell />`. Component: `web/src/lib/components/i18n/LocaleSuggest.svelte`
  (the `components/i18n/` dir already exists). It is tiny and has no heavy imports —
  mount it directly; no lazy shell needed (unlike `AuthModal`/`GlobalSearch`, there
  is no second chunk to defer).
- **Dialog markup:** bits-ui `Dialog` primitives, copied from
  `web/src/lib/components/auth/AuthModal.svelte` — `Root`/`Portal`/`Overlay`/
  `Content`/`Title`/`Description`/`Close`. There is no shadcn `dialog` wrapper in
  `ui/`; do not add one for this. Sizing: `w-[calc(100vw-2rem)] max-w-[400px]`,
  centered, `z-[92]` (above AuthModal's `z-[90]/[91]`, below the toasts'
  `z-[1002]`).
- **Direction/lang:** `Content` gets the *current* locale + its `uiDirection()` —
  the pitch is written in the language the user is currently reading. The target
  locale appears as its **endonym** (`UI_LOCALES[target].endonym` → "العربية" /
  "English"), matching every existing switcher (`MarketingHeader`, `Nav` locale
  grid). All layout utilities logical (`ms-`/`me-`/`start-`/`end-`) so the ar→en
  direction is free.

### 5.2 Copy

New paraglide message ids (added to `web/messages/en.json` + `ar.json`; namespaces
regenerated with `pnpm i18n:namespaces`; `pnpm i18n:check` gates all three commands
via `pre*`):

| id | en | ar |
| --- | --- | --- |
| `locale_suggest_title` | Switch to {locale}? | التبديل إلى {locale}؟ |
| `locale_suggest_body` | EasyQuran is also available in {locale}. | القرآن السهل متاح أيضًا بـ{locale}. |
| `locale_suggest_accept` | Switch to {locale} | التبديل إلى {locale} |
| `locale_suggest_dismiss` | Not now | ليس الآن |
| `locale_suggest_close` | Dismiss | تجاهل |

Final ar wording to be reviewed in the L01 fluent-Arabic pass; placeholders ship with
the endonym. Rendered through the generated `$lib/i18n/m/*` barrel, per-locale via
the `m.locale_suggest_title(undefined, { locale: current })` options pattern.

### 5.3 Timing

Root layout already computes `firstPaintComplete` (double rAF). The popup arms then
waits **1500 ms** — past LCP, past the offline-engine kick — re-checks conditions 4
and 6 at fire, then opens. One retry if a modal was open (condition 6); never queues
behind more than one retry.

### 5.4 State module

`web/src/lib/i18n/locale-suggest.svelte.ts` — class store in the `auth-modal.svelte.ts`
mold: `open = $state(false)`, `target: UiLocale | null`, `evaluate(pathname)` running
the §3 matrix, `accept()` → persist + let the link navigate, `dismiss()` → persist +
close. No `$effect` writing state; all mutations are event-handler-driven per the
Svelte 5 rules (effects are an escape hatch).

### 5.5 Analytics

Consent-gated `track()` from `web/src/lib/firebase/analytics.ts`, snake_case names
matching `notification_*` style: `locale_suggest_shown`, `locale_suggest_accepted`,
`locale_suggest_dismissed`, each with `suggested_locale` param. No new PII (the
browser language list itself is never sent).

## 6. Phase 2 (separate build, optional) — translation suggest

The Pakistan case has a second, better answer: UI stays English, but offer an Urdu
*Quran translation*. Plumbing already exists client-side:

- `TRANSLATION_CATALOGUE` (`web/src/lib/quran/catalogue.ts`) carries `languageCode`
  (BCP-47 primary subtag) per source — `ur` has 8 entries, 44 languages total.
- Selection persists via `readerSource.setSourceId(id)` + `noteTranslationChosen(id)`
  (pattern: `TranslationPicker.svelte`).

Rules: scan the *full* `navigator.languages` list (not just first-match) for a
`languageCode` hit whose language ≠ current translation and ≠ `ar` (Arabic readers
use the Arabic text, not `ar.*` tafsir entries); surface once per browser per language
under a sibling key `easyquran.translationSuggest` with the same dismissal contract;
**never** change the UI locale in the same breath (non-negotiable 6). Open question 2
(curated pick) blocks this phase — there is no "recommended" field today.

## 7. Docs to amend when this lands

- `docs/quran-system.md` Part 2 currently states: "Locale switches are links and
  preserve query/fragment; there is no locale cookie, storage preference, or
  Accept-Language override." Amend to keep it truthful: locale switches remain links;
  still no locale cookie and no Accept-Language override; a one-time client-side
  suggestion popup records only its own dismissal/acceptance in localStorage and
  never overrides the URL.
- `docs/remaining/feature-gap-catalogue.md` L01: append the shipped popup (and
  phase 2 if built) to the "shipped" evidence list.

## 8. Tests (co-located `__tests__`, `vite-plus/test` imports, happy-dom)

1. **Pure logic** — `web/src/lib/i18n/__tests__/locale-suggest.test.ts`: table-drive
   the §3 matrix. Navigator stubbing per repo pattern
   (`vi.stubGlobal("navigator", { languages: [...] })` + `vi.unstubAllGlobals()`,
   with the anti-slop disable comment — see `offline/__tests__/messages.test.ts`):
   `["en-PK","en","ur"]` on `/` → null (the guarantee, asserted by name);
   `["ar","en"]` on `/` → `ar`; `["ur-PK","en"]` → null; `["fr"]` → null; empty
   array → null; target == current → null; `/about` surface → null **and** nothing
   persisted; dismissed `ar` in storage → null; accepted `ar` → null;
   `["ar"]` (Chrome-reduction shape) on `/en/app/al-fatihah/t/en/sahih` → `ar` with
   target href `/ar/app/al-fatihah/t/en/sahih` (translation context preserved).
   Happy-dom's real `localStorage` — `localStorage.clear()` in `beforeEach`.
2. **Storage codec** — decode tolerance: malformed JSON, wrong shape, future version
   (`isFutureSchema`), unknown locale keys survive/ignore.
3. **Component** — mount style from
   `app/search/__tests__/translation-picker.test.ts` (`mount`/`unmount` from
   `"svelte"`, raw `querySelector`, `MouseEvent` clicks, hand-rolled `settle()`):
   dialog renders endonym + both actions; ✕ and "Not now" write `dismissed` and
   close; accept link carries `data-sveltekit-reload`, correct `href`, query+hash;
   Escape closes; re-`evaluate()` after dismissal is a no-op.
4. **Guard sweep** — extend the source-scan idea of `nav-guard.test.ts` only if a
   hand-built `/app/` string sneaks in; otherwise nothing (the popup builds hrefs
   exclusively via `localizeHref`).

Gate checklist before commit: `pnpm check` (`--fail-on-warnings`), `pnpm lint`
(`--deny-warnings`, no nested ternaries — use early-return helpers), `pnpm test`.
All three pre-run `pnpm i18n:check`, so the new message ids must compile in both
locales first.

## 9. Open questions for the owner

1. **Popup vs toast styling** — plan says bits-ui Dialog (focus-trapped, Escape,
   described). `UpdateToast`-style non-modal card is lighter-touch but has no focus
   semantics. Default: Dialog.
2. **Phase-2 curated pick** — which Urdu (or per-language) source is the default
   offer? Needs a `recommended` flag in the bake pipeline for
   `web/src/lib/data/translations.json` (never the immutable DBs), or a heuristic
   (none exists today).
3. **Reset path** — is "clear site data" acceptable as the only un-dismiss, or should
   a future Settings → Language section expose a reset? Default: acceptable for v1.
4. **Cross-device sync** — authed users could carry `resolved` in `SettingsDoc`
   (which already reserves unknown keys). Deferred; v1 is per-browser.
5. **Delay** — 1500 ms after first paint is a guess; confirm it doesn't fight the
   landing hero animations.

## 10. Non-goals

No server-side Accept-Language negotiation (Chrome is truncating it; `Vary` hell;
breaks the disk cache). No IP/geo anything. No auto-redirect, ever. No new UI locales
(en/ar only). No locale cookie. No changes to prerender/SSR/cache keys. Phase 2 does
not ship in the same change as phase 1.
