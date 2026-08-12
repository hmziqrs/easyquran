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

## 4. Phases

Each phase is independently shippable and independently measurable. Re-run the budget audit after
every phase and record the numbers in the PR.

### Phase 0 — instrumentation (½ day, no behaviour change)

- `web/scripts/audit-i18n-chunks.ts` — per-page message count + gzip, from `build/prerendered/**`.
- `web/i18n-budgets.json` seeded with today's numbers (109 / 195) so the direction of travel is
  visible.
- Wire into CI after `pnpm build`.

Ship this first; everything after it is judged by its output.

### Phase 1 — split the god resolvers, no new copy (1–2 days)

Highest value, zero translation work, zero visible change.

- `lib/i18n/marketing-copy.ts` → `chrome-copy.ts` (nav, footer, brand, skip, theme, locale links) +
  `appearance-copy.ts` (tweaks, surfaces, accents) + `landing-copy.ts` (landing + landing SEO).
  Keep pure-routing helpers (`marketingHref`, `marketingLocaleLinks`, `marketingLocaleFromPath`) in a
  message-free module so importing a URL helper never pulls copy.
- `(marketing)/+layout.svelte` resolves chrome once, passes props down. `MarketingNav`,
  `MarketingFooter`, `MarketingSeo`, `MarketingTweaks` stop calling a resolver themselves.
- `(marketing)/+page.svelte` imports `landing-copy` only.
- `reader-copy.ts` → per-surface modules mirroring the component tree: `reader-shell`,
  `reader-sidebar`, `reader-verse-tools`, `reader-search`, `reader-translation-picker`,
  `reader-page-nav`.
- Add guard #1.

Expected: marketing pages 109 → ~81 messages (measured), reader pages 195 → chrome + only the reader
surfaces that page mounts.

### Phase 2 — generated namespace barrels (1 day)

- `scripts/gen-message-namespaces.ts` + prefix table + `--check` mode + `predev`/`prebuild` wiring.
- Rename keys to the §3.1 convention (`tweaks_*`/`surface_*`/`accent_*` → `appearance_*`,
  `seo_home_*` → `landing_seo_*`) in one mechanical commit, both locales together.
- Convert Phase-1 modules to consume namespace barrels instead of the `m` barrel.
- eslint rule.

No byte change expected on its own; this is the ergonomics substrate that makes Phase 4 cheap.

### Phase 3 — lazy the appearance panel (½ day)

`appearance_*` is ~41 keys (`tweaks` 27 + `surface` 10 + `accent` 4) — the largest single block on
the chrome floor — and it renders inside a panel the user must open. `Tweaks.svelte` additionally
carries a `DEFAULT_COPY` object duplicating all 27 strings as English literals in the component.

- Render the panel body behind `{#if open}` with a dynamic `import()`.
- Delete `DEFAULT_COPY`; make the copy prop required.

Expected: another ~40 messages / ~2.5 KB gz off **every** page in the app. Trade-off: one small
lazy chunk fetched on first panel open, after user intent — no render-fetch waterfall on initial
paint, which is the failure mode upstream [issue #88] warns about.

### Phase 4 — localize the remaining pages (per page, ~½ day each)

Only now, on a substrate where a new page costs *only its own* bytes. Order by SEO value:

1. `faq` + `legal` (`privacy`, `terms`) — these also fix `<Seo faq={FAQS} />` emitting English-only
   structured data on `/ar`.
2. `about`, `contact`.
3. `auth` (`login`, `register`, `forgot-password`, `verify-email`).
4. `account`.

Per page: add `<ns>_*` keys to both catalogs, run codegen, convert the `.svelte` to
`import * as m from "$lib/i18n/m/<ns>"`, convert its `lib/data/content.ts` arrays (`FAQS`,
`PRIVACY_SECTIONS`, `TERMS_SECTIONS`, `ABOUT_STATS`, `LEGAL_UPDATED`) to message-keyed data, add the
`{ pageId, locale }` entry to `MARKETING_PUBLICATIONS`, add a budget entry.

Note `docs/i18n.md` line 111: auth/account routes are deliberately outside i18n middleware. Phases
4.3/4.4 need that decision revisited explicitly, not silently.

### Phase 5 — locale scaling (revisit at locale #3)

Do nothing yet. Details in §5.

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

- `web/i18n-budgets.json` committed, enforced in CI, and every entry lower than its Phase-0 seed.
- No source file outside `src/lib/i18n/m/**` imports `$lib/paraglide/messages.js` — guarded, not
  merely reviewed.
- Adding a page's messages provably does not change any other page's budget (the budget guard's diff
  is the proof).
- Marketing pages at the chrome floor; reader pages carry chrome + only the reader surfaces they
  mount.
- `docs/i18n.md` §10 updated: the follow-on list now points here for the delivery mechanism.

## 7. Sources

- [issue #668] — barrel produces app-wide shared chunk: https://github.com/opral/paraglide-js/issues/668
- [issue #88] — per-locale splitting builds: https://github.com/opral/paraglide-js/issues/88
- [issue #531] — namespace support (unimplemented): https://github.com/opral/paraglide-js/issues/531
- [issue #345] — `m` import syntax / auto-import ergonomics: https://github.com/opral/paraglide-js/issues/345
- Compiler options: https://paraglidejs.com/compiler-options
- Message-format plugin `pathPattern` semantics: https://inlang.com/m/reootnfj/plugin-inlang-messageFormat
- Vite 8 / Rolldown environments: https://vite.dev/blog/announcing-vite8
