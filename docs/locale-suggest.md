# Locale-suggest popup

Status: **feature shipped; pending retirement in V.** Phase 1 is [`next-features.md`](next-features.md)
Feature 10 (that doc wins on conflict). Written for URL scheme A (unprefixed `en`, `/ar/` prefix,
no `/app`).

Sequenced by: phase **V** in [`master-plan.md`](master-plan.md) — verify this spec against the
shipped implementation, then retire the doc with the V batch.

## 1. Guarantee and non-negotiables

A one-time, client-side "this app is also in your language — switch?" popup.

**The guarantee:** `navigator.languages = ["en-PK", "en", "ur"]` on `/` → no popup, ever.
Detection takes the first browser-preferred language that is a supported UI locale and speaks up
only when it differs from the URL's locale. No IP, no geo, no `Accept-Language` (Chrome is
truncating it; client `navigator.languages` works the same with one entry).

1. **The URL is the only locale truth** (paraglide `["url", "baseLocale"]`, `quran-system.md`
   Part 2). The popup only points at a localized URL.
2. **No cookies, ever** — any cookie makes `hooks.server.ts` bypass the translated SSR disk cache
   and go `private, no-store`. Persistence is `localStorage`, client-side only.
3. **No server changes**: nothing in `hooks.server.ts`, `hooks.ts`, prerender or cache keys.
4. **Suggest, never redirect.**
5. **Every answer is permanent per target locale** (§3).
6. **UI locale stays independent from the Quran translation** (gap catalogue L01).

## 2. Detection

- `suggestedUiLocale()`: paraglide's `extractLocaleFromNavigator()` — walks `navigator.languages`
  in order, full tag then primary subtag, first match against `["en", "ar"]` wins. `["ur-PK","en"]`
  → `en`; `["fr"]` → none.
- `currentUiLocale(pathname)`: `/ar` or `/ar/…` → `ar`, else `en` (mirrors
  `marketingLocaleFromPath`; do not use `getLocale()` — it misreads de-localized pages).
- Target href: `publicHref(localizeHref(deLocalizeHref(path + search + hash), { locale }))`,
  rendered as an `<a data-sveltekit-reload>` (locale switches are full reloads so `dir` is set
  server-side). Normalize `/ar` to `/ar/` first and assert the target ≠ the current path — a
  same-path accept link must never ship. Round-tripping keeps translation context
  (`/al-fatihah/t/en/sahih` ↔ `/ar/al-fatihah/t/en/sahih`), query and hash.

## 3. Trigger matrix and dismissal contract

Evaluate after first paint and again on every `afterNavigate`. Show only when **all** pass:

| #   | Condition                                        | Covers                                                                                                                                                                                                                                                                                |
| --- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `navigator.languages` is a non-empty array       | Privacy-hardened / embedded browsers → silence                                                                                                                                                                                                                                        |
| 2   | A first-match locale exists                      | Unsupported primary language → nothing to suggest (the guarantee)                                                                                                                                                                                                                     |
| 3   | First match ≠ current URL locale                 | Readers already in their locale are never nagged                                                                                                                                                                                                                                      |
| 4   | Target locale not yet in the resolved map (§4)   | "Don't ask again"                                                                                                                                                                                                                                                                     |
| 5   | The current surface has an `ar`/`en` twin        | `/`, `/ar`, `/ar/`, and every path matching `READER_ROUTE_PATTERNS` in `web/src/hooks.ts` after stripping `/ar` (reader routes, indexes, `/search`, `/settings`, `/bookmarks`, `/yours`). Elsewhere (unpublished marketing pages, auth, account, design): skip **and record nothing** |
| 6   | No other modal open at fire time and while shown | (a) busy at fire → retry once when it closes, re-running the whole matrix; (b) auth modal or command palette opens while shown → close, record nothing, no re-show this load                                                                                                          |

**Dismissal contract** (normative):

- ✕, "Not now", Escape or overlay click → `resolved[target] = "dismissed"`; accept →
  `"accepted"` then navigate. Either way that locale is never suggested again on this browser.
- Showing records nothing; only the user's own answer persists. A forced close (6b, or another
  tab resolving it) records nothing.
- Re-check the resolved map at fire time (after the §5.3 delay), not only at mount.
- Cross-tab: `onStorageKey(KEY)` — if another tab resolves the open target, close without writing.
  Same-millisecond double show in two tabs is accepted (per-locale keys, last answer wins).

## 4. Storage

`easyquran.localeSuggest` = `{ "v": 1, "resolved": { "ar": "accepted" | "dismissed" } }`

- Values decode as a literal union (`{"ar":"bogus"}` drops the key); unknown locales ignored;
  future versions ignored via `isFutureSchema(raw, 1)`.
- In-memory write-through: the state module's own map is the read source, `writeJSON` is
  best-effort, so blocked storage nags at most once per page load and never throws.
- Not in `SettingsDoc` for v1 (cross-device sync is §9 open question 3).

## 5. UI

### 5.1 Shape

`web/src/lib/components/i18n/LocaleSuggest.svelte`, mounted in the root `+layout.svelte` next to
the toasts and `AuthModalShell`. bits-ui `Dialog` primitives copied from `AuthModal.svelte`
(no shadcn dialog wrapper); `w-[calc(100vw-2rem)] max-w-[400px]`, `z-[92]` (above AuthModal,
below toasts and Tweaks). SSR-safe: `open = $state(false)`, dialog inside `{#if open}`, all
browser APIs in client lifecycle only. `Content` carries the **current** locale's `lang`/`dir`;
the target appears as its endonym (`UI_LOCALES[target].endonym`); logical utilities only. ✕ renders
first so initial focus lands on the safe action.

### 5.2 Copy

| id                       | en                                       | ar                                  |
| ------------------------ | ---------------------------------------- | ----------------------------------- |
| `locale_suggest_title`   | Switch to {locale}?                      | التبديل إلى {locale}؟               |
| `locale_suggest_body`    | EasyQuran is also available in {locale}. | القرآن السهل متاح أيضًا بـ{locale}. |
| `locale_suggest_accept`  | Switch to {locale}                       | التبديل إلى {locale}                |
| `locale_suggest_dismiss` | Not now                                  | ليس الآن                            |
| `locale_suggest_close`   | Dismiss                                  | تجاهل                               |

Shipped strings wrap the placeholder in isolation characters (`⁦{locale}⁩`). Call shape:
`locale_suggest_title({ locale: endonym }, { locale: currentUiLocale })` — the param is the target
endonym, the option is the rendering locale. Arabic wording gets the L01 fluent-Arabic review.

### 5.3 Timing

Own double-rAF after mount, then **1500 ms**, then re-check conditions 4 and 6 and open. Only the
initial evaluation is delayed; `afterNavigate` re-evaluations are immediate (a first landing on an
ineligible page must not burn the chance). Shown-latch: at most one show (or forced close) per
page load. At most one retry for 6a.

### 5.4 State module

`web/src/lib/i18n/locale-suggest.svelte.ts`, class store in the `auth-modal.svelte.ts` mold:
`open`, `target`, `evaluate(pathname)`, `accept()`, `dismiss()`. Every close path (buttons,
Escape, overlay, `onOpenChange`) goes through one `resolve(answer)` so a dismissal is written
exactly once. The only `$effect` that writes state is the 6b forced-close guard.

### 5.5 Analytics

Consent-gated GA4 `track()` from `web/src/lib/firebase/analytics.ts`: `locale_suggest_shown`,
`locale_suggest_accepted`, `locale_suggest_dismissed` with `suggested_locale`; dismissals add
`dismiss_reason: escape | overlay | button | close_icon` (via `onEscapeKeyDown` /
`onInteractOutside`). The browser language list is never sent.

## 6. Phase 2 — superseded

Translation suggest (offer an Urdu Quran translation while the UI stays English) is superseded:
no catalogue `recommended` flag. If revisited, derive the offer client-side from
`navigator.languages` + the catalogue (`next-features.md` Feature 8 grouping), never in the same
change as Phase 1, and never touching the UI locale.

## 7. Docs to amend when it lands

- `quran-system.md` Part 2: still no locale cookie or `Accept-Language` override; add that a
  one-time client-side suggestion records only its own answer in `localStorage`.
- `remaining/feature-gap-catalogue.md` L01: add the shipped popup to the evidence list.

## 8. Tests

Co-located `__tests__`, `vite-plus/test`, happy-dom; `localStorage.clear()` in `beforeEach`.

1. **Logic** (`web/src/lib/i18n/__tests__/locale-suggest.test.ts`), table-driven over §3:
   `["en-PK","en","ur"]` on `/` → none (asserted by name); `["ar","en"]` on `/` → `ar`;
   `["ur-PK","en"]`, `["fr"]`, `[]` → none; target = current → none; ineligible surface → none
   **and nothing persisted**; dismissed/accepted `ar` → none; `["ar"]` on
   `/al-fatihah/t/en/sahih` → href `/ar/al-fatihah/t/en/sahih`; `/ar` + target `en` → `/`;
   target href never equals the current path.
2. **Codec**: malformed JSON, wrong shape, future version, literal-union rejection, unknown keys.
3. **Component**: closed → zero dialog DOM; endonym + both actions render; ✕ / "Not now" /
   Escape each write `dismissed` exactly once; accept link has `data-sveltekit-reload`, correct
   href, query + hash; forced closes write nothing; throwing `setItem` → in-memory only, no throw;
   analytics spy checks events and `dismiss_reason`, and that no payload carries the language
   list.
4. **Lifecycle** through the store: ineligible then eligible `evaluate` shows; shown-latch;
   6a retry-once.

Manual matrix (fresh profile per row): `en` browser on `/` → never; `ar`-primary on `/` → popup
≈1.5 s, accept → `/ar/`; dismiss → never again; deep translated link keeps its translation;
ineligible page then client-side nav to an eligible one → popup; ⌘K/auth while open → silent close;
two tabs → other tab closes without writing; `/ar` without slash on an `en` browser → accept lands
on `/`; private window → no error.

## 9. Open questions

1. Dialog (default) vs a lighter non-modal toast.
2. Reset path: "clear site data" only, or a future Settings → Language reset (default: permanence
   plus a future explicit reset).
3. Cross-device sync of `resolved` via `SettingsDoc` (deferred; v1 is per-browser).
4. Is 1500 ms right next to the landing hero animations?
