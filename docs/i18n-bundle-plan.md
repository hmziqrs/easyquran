# i18n bundle plan — per-page message chunking

Follow-on to `docs/i18n.md`. That document owns *what* gets localized and *which URLs exist*. This
document owns *how many localized bytes each page downloads*, and the mechanism that keeps that
number flat as pages and locales are added.

Nothing here changes Quran data, source selection, translation identities, route grammar, or
publication gating. It is a delivery-shape change only.

---

## 1. Measured baseline

Fresh production build (`pnpm build`, `@inlang/paraglide-js` 2.23.2, Vite+ 0.2.5 / Rolldown),
measured per prerendered page by walking each page's `_app/immutable/**` asset list and counting
compiled paraglide message bodies inside them.

Catalog totals: `messages/{locale}.json` = 111 keys, `messages/reader/{locale}.json` = 171 keys.
**282 total.**

| page group | pages | messages shipped | i18n JS (gz) | i18n JS (raw) | Arabic strings |
|---|---|---|---|---|---|
| `/`, `/ar/` | 2 | 109 | 7.7 KB | 21.5 KB | 353 |
| `about`, `faq`, `contact`, `privacy`, `terms` | 5 | **109** | **7.7 KB** | 21.5 KB | 353 |
| `/ar/app/**` (reader) | 2594 | **195** | **10.8 KB** | 34.5 KB | 579 |
| `login`, `register`, `account`, … | 6 | 0 | 0 | 0 | 0 |

Two facts to internalize:

1. **`/about` ships 109 messages while rendering zero localized strings.** It pays for nav, footer,
   the appearance panel, *and the entire landing page*, because `(marketing)/+layout.svelte` imports
   `resolveMarketingCopy()`.
2. **A reader page ships 195 of 282 messages** — reader copy *plus* all marketing chrome *plus*
   landing. Per-page cost is the union of what the app uses, not what the page uses.

Consequence: every message added for `about`/`faq`/`auth`/`account` raises the byte cost of *every
other page*. At the doc's planned five locales (`en, ar, ur, id, fr`) this multiplies.

### Root causes

**(a) Barrel import.** `src/lib/paraglide/messages.js` is
`export * as m from './messages/_index.js'`. Upstream [issue #668] documents exactly this: the barrel
is a tree-shaking pinch point, so Vite/Rolldown emits one shared chunk holding the union of all
app-wide-used messages and every entry preloads it. Closed as `blocked` — bundler behaviour by
design, no upstream fix pending. Their reporter measured 321 KB shared chunk → 4.9–10.5 KB per route
after switching to individual message imports.

**(b) God resolvers.** `lib/i18n/marketing-copy.ts:212` `resolveMarketingCopy(locale)` returns one
object containing `nav + footer + tweaks + landing + seo + brand + surfaces + accents`. Any importer
pulls all 111 messages. It is imported by `(marketing)/+layout.svelte`, so every marketing page
pays. `reader-copy.ts` has the same shape for 171 reader keys. **This defeats splitting even if the
barrel is removed** — it must be fixed in the same pass.

**(c) Redundant resolution.** Six components each run `$derived(resolveMarketingCopy(locale))` —
layout, `+page`, `MarketingNav`, `MarketingFooter`, `MarketingSeo`, `MarketingTweaks` — rebuilding a
111-field object six times per render.

**(d) Every locale ships to every visitor.** Verified in
`node_modules/@inlang/paraglide-js/dist/compiler/output-structure/message-modules.js`: 2.23.2 emits
one file per message with **all locales inlined**:

```js
const en_accent_gold_label = () => `Gold`;
const ar_accent_gold_label = () => `ذهبي`;
export const accent_gold_label = ((inputs, options) => {
  const locale = experimentalStaticLocale ?? options.locale ?? getLocale()
  if (locale === "ar") return ar_accent_gold_label(inputs)
  return en_accent_gold_label(inputs)
});
```

There is no per-locale output file in this version. See §5 for what is and isn't possible here.

---

## 2. Validated fix

The mechanism was tested, not assumed. One namespace (`landing`, 28 keys) was extracted from
`resolveMarketingCopy` into a separate module that imports individual message modules
(`$lib/paraglide/messages/<key>.js`), and `(marketing)/+page.svelte` was repointed at it. Full
rebuild, re-audit:

| page | before | after | delta |
|---|---|---|---|
| `about.html` | 109 msgs / 7.7 KB gz | **81 msgs / 5.7 KB gz** | −28 msgs, −26 % |
| `index.html` (landing) | 109 msgs / 7.7 KB gz | 81 msgs shared + landing copy in its own route chunk (`nodes/23.js`) | landing copy no longer shared |
| all other marketing pages | 109 msgs | 81 msgs | −26 % |

Landing strings ended up **inlined into the landing route's node chunk** — the ideal outcome: no
shared chunk, no extra request, zero cost to other pages. Prototype reverted; the worktree is clean.

Extrapolated: with every namespace split, marketing pages fall to a chrome-only floor and reader
pages stop carrying landing/marketing-page copy. §4 phases the remaining wins.

---

## 3. The design: namespace = surface, generated barrels

The upstream fix (import each message module individually) is correct but has real friction: no
auto-import in editors (upstream [issue #345]), and hand-maintaining hundreds of import lines across
~200 files. Upstream [issue #531] requests native namespace support; it is **open, unimplemented**,
and the issue itself points at codegen as the workaround. So we generate the ergonomic layer.

### 3.1 Authoring layer — catalogs stay coarse

Keep **at most two catalog files per locale**. Do **not** add a JSON file per page.

Reason: `@inlang/plugin-message-format` 4.4.1 treats an array `pathPattern` as a **one-way merge** —
*"When exporting, all messages are written to the last path pattern in the array. Messages are not
split back across multiple files."* We already run an array of two
(`messages/{locale}.json`, `messages/reader/{locale}.json`), so a translator round-trip through
Fink/Sherlock can collapse both files into `messages/reader/{locale}.json`. That is a pre-existing
latent hazard; more files multiply it.

Organization comes from **key prefixes**, not files. Convention:

```
chrome_*      nav, footer, brand, skip link, theme + locale switcher labels
appearance_*  the Tweaks panel (currently tweaks_*, surface_*, accent_*)
landing_*     marketing home
about_*  faq_*  contact_*  legal_*      one namespace per marketing page
auth_*   account_*
reader_*      reader shell + verse tools (further split, see 4.4)
```

Each namespace's SEO strings live *in that namespace* (`landing_seo_title`, `about_seo_title`), not
in a global `seo_*` bucket. Today's `seo_home_*` becomes `landing_seo_*`.

### 3.2 Code layer — generated per-namespace barrels

`web/scripts/gen-message-namespaces.ts` reads the catalogs, groups keys by prefix through an
explicit prefix→namespace table, and emits one file per namespace:

```ts
// web/src/lib/i18n/m/about.ts — AUTO-GENERATED by scripts/gen-message-namespaces.ts. Do not edit.
export { about_seo_title } from "$lib/paraglide/messages/about_seo_title.js";
export { about_heading }   from "$lib/paraglide/messages/about_heading.js";
// …
```

Consumers get one import line, full autocomplete, and correct chunking:

```svelte
<script lang="ts">
  import * as m from "$lib/i18n/m/about";
</script>
<h1>{m.about_heading()}</h1>
```

Why this works where the global barrel fails: a namespace barrel is imported by exactly **one** route
subtree, so Rolldown has no reason to hoist it into a shared chunk — proved by the §2 prototype.
`sideEffects: false` is already present in the generated `messages/package.json`, so the re-exports
cost nothing.

Codegen runs in the same slot as paraglide compile — a `predev`/`prebuild` step — so adding a key is
edit-JSON → key appears with types. No manual import bookkeeping, ever. Generated files are
committed so type-check works on a cold clone, and the script is idempotent (`--check` mode fails CI
if output is stale).

### 3.3 Resolver policy

- **Page copy: call message functions in the component that renders them.** No page-level
  `resolveXCopy()` object. This is what lets Rolldown attribute the string to the route chunk.
- **Shared structural components** (`Nav`, `Footer`, `Tweaks`, generic `Seo`) keep receiving resolved
  label props — `docs/i18n.md` requires it so those components never import Paraglide directly.
  Their resolver is scoped to the `chrome` / `appearance` namespace only, and is computed **once in
  the layout** and passed down, not re-derived in six components.
- Module-level `m.*()` stays banned (`docs/i18n.md` rule); resolution happens at render/request time.

### 3.4 Guards

Two, both cheap, both mirroring conventions already in the repo
(`nav-guard.test.ts`, `catalogue-sha-guard.test.ts`):

1. **Source guard** (`pnpm test`): no file outside `src/lib/i18n/m/**` may import
   `$lib/paraglide/messages.js`, `$lib/paraglide/messages/_index.js`, or a namespace barrel outside
   its own surface. Also enforced as an eslint `no-restricted-imports` rule for editor feedback.
2. **Budget guard** (post-build CI step, alongside `gen-offline-pack.ts`): walk each prerendered
   page's asset list, count message bodies, compare against a committed
   `web/i18n-budgets.json` (`{ "about": { "maxMessages": 45, "maxGzip": 3072 }, … }`). Fail on
   regression. A working implementation of the measurement already exists from this audit and only
   needs promotion into `web/scripts/`.

Without guard #1 a single `import { m }` silently re-merges everything and nobody notices.

---

## 4. Delivery — what shipped

All four buildable phases are done and on `codex/localization`. Phase 5 is deliberately a no-op; see
§5. Numbers below are measured, not estimated: worst page per surface, from
`node scripts/audit-i18n-chunks.ts` after a full `pnpm build`.

| surface | before | after | change |
|---|---|---|---|
| `about` | 109 msgs / 7.7 KB | 56 msgs / 6.8 KB | now carries its own 18 messages instead of the landing page's |
| `faq` | 109 / 7.7 KB | 58 / 11.2 KB | +20 own messages (long prose, two locales) |
| `contact` | 109 / 7.7 KB | 49 / 5.8 KB | +11 own messages |
| `privacy` | 109 / 7.7 KB | 64 / 7.8 KB | +26 own (legal chrome + privacy body) |
| `terms` | 109 / 7.7 KB | 63 / 7.3 KB | +25 own (legal chrome + terms body) |
| `/`, `/ar/` | 109 / 7.7 KB | 69 / 8.4 KB | −40 messages; see the gzip note below |
| reader (1298 pages) | 195 / 10.8 KB | 155 / 7.8 KB | −40 messages, −3.0 KB |

The chrome floor is **38 messages**. Before the split it was the union of everything the app used.

The decisive check is the arithmetic in the Phase-4 commit: each page's rise equals its own
namespace size exactly, so no page pays for another's copy. 96 new messages were added and no
existing page's budget moved.

Landing is the only surface whose gzip rose (~650 B) while its message count fell 109 → 69: its copy
now sits in its own route chunk and compresses against a smaller dictionary. Recorded as a
`justification` in `i18n-budgets.json`.

### Phase 0 — instrumentation (done, `be30bec`)

`web/scripts/audit-i18n-chunks.ts` + `web/i18n-budgets.json`, wired into `postbuild` as
`pnpm i18n:budget`. Seeded from the pre-refactor build so the direction of travel is in git history.
`--update` re-seeds, `--report` never fails. Message counts are budgeted exactly; gzip gets 2 %
headroom because chunk hashes and minified identifiers move sizes by a byte or two per build.

### Phase 1 + 2 + 3 — namespaces, split resolvers, lazy panels (done, `c891e1c`)

Shipped as one commit because they are one mechanism: splitting the resolvers without the generated
barrels just moves the pinch point, and the barrels are pointless while a god resolver aggregates
every namespace.

- `i18n-namespaces.json` + `scripts/gen-message-namespaces.ts`, run inside `pnpm i18n:check` so
  `dev`, `build`, `lint` and `test` all regenerate. 13 namespaces over 378 messages.
- `marketing-copy.ts` is now types + route helpers with **no message import**, so `reader-copy.ts`
  and the reader layout — which only ever wanted types — stopped pulling marketing copy.
- `chrome-copy.ts` / `appearance-copy.ts` / `landing-copy.ts` replaced `resolveMarketingCopy`.
  Chrome resolves once in `(marketing)/+layout.svelte` and is passed down, instead of six components
  each re-deriving a 111-field object.
- `reader-settings-copy.ts` carved the reader appearance panel out of `reader-copy.ts`.
- `Tweaks.svelte` takes `triggerLabel` + `loadCopy: () => Promise<TweaksResolvedCopy>` and
  dynamically imports panel copy on first open. `DEFAULT_COPY` — 27 English strings duplicated inside
  the component — is gone, and the copy prop is required.

Two deviations from the plan as written:

1. **No mass key rename.** `tweaks_*`/`surface_*`/`accent_*` keep their names and are mapped to the
   `appearance` namespace by an explicit rule in `i18n-namespaces.json`. The generator makes
   membership explicit either way, so renaming 41 keys across two catalogs bought nothing but churn
   and merge risk. Only the namespace *convention* is enforced going forward: new surfaces use a
   `<surface>_` prefix. `seo_home_*` also kept its name and belongs to `landing`.
2. **No eslint rule.** `vp lint` is oxlint-based and the vitest guard is both deterministic and
   idiomatic here (`nav-guard.test.ts`, `catalogue-sha-guard.test.ts` set the precedent). Adding a
   second, weaker enforcement path was not worth it.

### Phase 4 — the remaining marketing pages (done, `80bc483`)

`about`, `faq`, `contact`, `privacy`, `terms` — the exact list in `docs/i18n.md` §10. 96 keys per
locale, one resolver module per page, `src/lib/data/content.ts` deleted, every list item carrying a
locale-independent `id`, and each page passing its own `title`/`description`/`inLanguage` to `Seo`.
The FAQ page now feeds localized entries to its `FAQPage` structured data, which was English-only on
`/ar` before.

English is verbatim from the old templates and `content.ts`. **Arabic is new and unreviewed**: per
`docs/i18n.md`, an Arabic catalog PR needs a named fluent reviewer, and machine-translated
publishing is out of scope. `MARKETING_PUBLICATIONS` is untouched, so `/ar/about` and friends still
404 exactly as before. Flipping those entries is gated on that review, not on this work.

### Not done: auth and account

`login`, `register`, `forgot-password`, `verify-email` and `account` remain English. This is a
deliberate stop, not an omission: `docs/i18n.md` line 111 places auth, account and health routes
**outside** the i18n middleware, and `/ar/account` is specified to 404. Localizing them means
reversing a routing decision, which is a product call with security-adjacent surface (session
handling, OAuth callbacks), not a delivery refactor. When that decision is made, the substrate is
ready: add an `auth`/`account` namespace, one resolver module per page, one budget entry.

### Phase 5 — locale scaling

Nothing to build. See §5.

---


## 5. Locale scaling — researched, currently blocked

Every visitor downloads every locale's strings. Options, all investigated:

| approach | verdict |
|---|---|
| `experimentalPerLocaleBuild` (paraglide 2.23.0, Vite 8 environments, one Rolldown graph per locale) | **Blocked for SvelteKit.** 2.23.0 removed the private SvelteKit integration. Source throws: *"experimentalPerLocaleBuild Vite environments cannot compose with an existing builder.buildApp orchestrator. A framework must expose a public client-variant API before using this backend."* SvelteKit owns `builder.buildApp`. |
| `experimentalStaticLocale` + one full build per locale | Technically real — compiles the locale branch away. Needs N builds and N deployed asset trees behind a prefix router, against a single SvelteKit manifest. Cost is disproportionate at two locales; reconsider at four or five. |
| `experimentalMiddlewareLocaleSplitting` | Docs: *"unstable and unsuitable for production"*, SSR/SSG only, **no client-side routing**. We have client routing. No. |
| `outputStructure: "locale-modules"` for production | Actively worse — docs: *"can lead to more inefficient tree-shaking and larger bundle sizes."* It is the documented **dev** default only. Leave `message-modules`. |
| Dynamic `import()` / `fetch` per locale | Upstream maintainers, [issue #88]: *"Any solution using `fetch` or `await import` is bound to introduce a render-fetch waterfall which drastically increases Time-To-Interactive."* No. |
| Rolldown `advancedChunks` / `rolldownOptions.output.codeSplitting` | Wrong lever. Chunk grouping cannot unmerge what the module graph merged. |

Upstream [issue #88] ("per locale splitting builds") is **open, unassigned, no date**. Maintainer
position: eager all-locale delivery is acceptable **below ~10 locales**.

**Decision: accept locale duplication for now.** Phases 1–3 are what actually contain the cost —
they shrink the base that locale count multiplies. Adding `ur`/`id`/`fr` on top of an 81-message
chrome floor plus per-page namespaces is affordable; on today's 195-message union it is not.

Re-evaluate when either upstream [issue #88] lands, or SvelteKit exposes the client-variant API that
unblocks `experimentalPerLocaleBuild`. The generated-barrel design in §3 is deliberately compatible
with both — they become a config change, not a refactor.

---

## 6. Definition of done

- [x] `web/i18n-budgets.json` committed and enforced in `postbuild`. Every surface is below its
      Phase-0 seed on message count; `faq` and `privacy` are above it on gzip only because they gained
      their own copy, which is the whole point.
- [x] No source file outside `src/lib/i18n/m/**` imports `$lib/paraglide/messages.js` or reaches a
      compiled message module — `message-barrel-guard.test.ts`, guarded rather than reviewed.
- [x] Adding a page's messages provably does not change any other page's budget. The Phase-4 build
      is the proof: 96 new messages, five budgets up by exactly their own namespace size, landing and
      reader unchanged.
- [x] Marketing pages sit on a 38-message chrome floor; reader pages carry chrome plus reader copy
      only, with the appearance panel lazy on both surfaces.
- [x] `docs/i18n.md` §10 points here for the delivery mechanism.

### How to add a page from here

1. Add `<page>_*` keys to `messages/en.json` and `messages/ar.json`.
2. Add a namespace to `i18n-namespaces.json` with `"prefixes": ["<page>"]`. An unclaimed key fails
   generation, so this step cannot be forgotten.
3. `pnpm i18n:check` regenerates `src/lib/i18n/m/<page>.ts` with types.
4. Write `<page>-copy.ts` importing only that barrel; render it from the route.
5. Add a budget entry and run `pnpm build`. The audit prints exactly what the page costs.

Anything that would make another page's budget move fails the audit, which is the property that makes
this scale to hundreds of pages.

## 7. Sources

- [issue #668] — barrel produces app-wide shared chunk: https://github.com/opral/paraglide-js/issues/668
- [issue #88] — per-locale splitting builds: https://github.com/opral/paraglide-js/issues/88
- [issue #531] — namespace support (unimplemented): https://github.com/opral/paraglide-js/issues/531
- [issue #345] — `m` import syntax / auto-import ergonomics: https://github.com/opral/paraglide-js/issues/345
- Compiler options: https://paraglidejs.com/compiler-options
- Message-format plugin `pathPattern` semantics: https://inlang.com/m/reootnfj/plugin-inlang-messageFormat
- Vite 8 / Rolldown environments: https://vite.dev/blog/announcing-vite8
