# Audit — `docs/multi-translation-plan.md`

> Status: audit findings, verified against source at commit `a16c74c` (2026-08-13).
> Only claims re-proved from code/data are recorded here. Every item carries `file:line` evidence.
> Retracted/unverified claims are deliberately absent.

Architecture verdict: sound. Query-param mirror (≡ `mode` precedent) keeps nav helpers, disk-cache
keys and SSG guarantees untouched — confirmed. The findings below are defects *inside* that design,
not objections to it.

---

## Defects — fix before implementing

### 1. `lang` attribute uses the display name, not the language code

Plan §1 declares `language: string; // .language (display)` and §5 renders `lang={row.t.language}`.

Evidence:

- `src/lib/data/translations.json` row 0 — `['sq.nahi','Albanian','sq','ltr','Efendi Nahi','Hasan Efendi Nahi','sqlite/sq.nahi.sqlite',1175552]`: index 1 = display name, index 2 = BCP-47 code.
- `src/lib/quran/catalogue.ts:97-98` maps both `language` and `languageCode` onto `TranslationCatalogueEntry`.
- `src/lib/data/quran-types.ts:151-160` — both fields exist on the entry.

Emitted markup would be `lang="Albanian"` — invalid, degrades screen readers and hyphenation.

**Fix:** add `languageCode` to `StackedTranslation`; use it for `lang=`, keep `language` for the label.

### 2. `translator` is nullable in the catalogue type

Plan §1 types `translator: string`.

Evidence:

- `src/lib/data/quran-types.ts:157` — `translator: string | null`.
- `src/lib/quran/catalogue.ts:69` — decoder explicitly accepts `null`.

`tsconfig` is `strict`, so the assignment fails `pnpm check`.

Scope note: **0 of 115** baked rows currently have a null translator, so this is a type-correctness
fix, not a runtime-fallback need.

**Fix:** type as `string | null`; label fallback `translator ?? name`.

### 3. Mirror `$effect` writes the stale value before the adopted one

Plan §4 snippet reads `ids` via `untrack` *before* `setIds(parsed)`, then reuses that stale `ids` in
the `replaceState` line. Trace on a `?more=ur.jalandhry` deep link with an empty store:

1. adopt → store `= ["ur.jalandhry"]`;
2. `replaceState(withMoreParam(url, []))` → **`?more` deleted from the URL**;
3. url change re-runs the effect → `?more=ur.jalandhry` written back.

It converges (store keeps the deep-linked value) but costs two extra `replaceState` calls and a
visible query-string flicker.

Why `mode` does not show this despite the identical shape (`+layout.svelte:53-61`):

- `onMount` compiles to `user_effect` (svelte 5.56.8 `src/index-client.js:99`), queued in declaration
  order, and is declared at `+layout.svelte:39` — *before* the mirror effect at `:53`.
- `reader.hydrate(parseModeParam(page.url))` adopts the URL mode first
  (`src/lib/stores/reader-persistence.svelte.ts:175-199`, adoption at `:181-182`).

Plan §4 explicitly has **no** hydrate call (constructor reads localStorage only) → `more` has no such
guard.

**Fix:** `return` immediately after the adopt branch.

### 4. The reader is Arabic XOR translation — extras cannot sit "under each Arabic ayah" on `/t/**`

Evidence:

- `src/routes/(application)/app/_reader/VerseRow.svelte:70-88` — `{#if translationActive}` renders the
  translation *instead of* the Arabic, never both.
- `src/lib/server/quran-translation-page.ts` contains **zero** Arabic references — translated-route
  page data carries no Arabic text at all.
- `src/lib/data/quran.ts:68` — `translationIdFromSegments(lang: string, translator: string)` takes
  non-optional strings, but on the Arabic route `page.params.lang` / `page.params.translator` are
  `undefined`, so §7's primary-dedupe lookup needs a guard.

Consequences to settle in the plan text:

- Arabic route (`/app/al-baqarah`): no primary translation exists → primary-dedupe is a no-op; this is
  the route where "stacked under the Arabic" is literally true, and it matches the plan's own manual
  verification steps.
- `/t/**`: the stack is translations-only, with no Arabic anywhere on the page.

### 5. Extras are never gated to verse mode

Plan §5 states extras render only in verse mode, but the snippet has no gate.

Evidence:

- `VerseRow.svelte:111-119` — under `[data-reader-mode="reading"] [data-source-kind="arabic"]`,
  `.verse-row` and `.verse-text` become `display: inline`.
- `data-source-kind` is set at `SurahReader.svelte:742` and `RangeReader.svelte:162`.

A `display: block` `.verse-extra` inside an inlined `<li>` breaks the reading-mode flow.

**Fix:** wrap `extraRows` in `{#if reader.isVerseMode}` (`reader` is already imported in `VerseRow`).

### 6. "First use = full OPFS download → skeleton" is wrong

`quranWorker.readRange` routes through `withSourceFallback`
(`src/lib/quran/worker-client.ts:348-375`), whose miss path is
(`src/lib/quran/worker-client.ts:194-217`):

1. `tryLocal()` fails;
2. `onMiss()` → background `ensureTranslation` (fire-and-forget);
3. `if (QURAN.apiBase)` → `await apiFetch()` — served over HTTP **now**;
4. otherwise `fail()` → `ReadChainError` thrown immediately.

It never waits for the OPFS download. `.env:68` sets `PUBLIC_QURAN_API_BASE`, so the API branch is the
live one here (`src/lib/config/site.ts:21,37`).

**Consequences for the plan:** the "server untouched" claim needs qualifying — N extras issue N API
range calls per range on first use. Skeletons will rarely be visible; use `onStatus.servedBy` to
surface a network tier instead. Offline with a cold extra throws rather than degrading.

### 7. Selected extras are not pin-protected from OPFS pruning, and pruning runs at boot

Evidence:

- `src/lib/workers/quran.worker.ts:328` — prune after a fresh download.
- `src/lib/workers/quran.worker.ts:487` — prune on **every `init`** message.
- Both pass only `pinnedArabicIds: PINNED_ARABIC`; `computeEvictions`
  (`src/lib/workers/opfs-retention.ts`) pins nothing else.

A persisted-but-cold extra is an ordinary artifact and can be TTL/cap-evicted at boot, then silently
re-downloaded. The plan never feeds the stacked ids into `PruneOptions`.

**Fix:** pin `stackedTranslations.ids` + the primary translation id alongside the Arabic ids.

---

## Sizing — optional, not MVP-blocking

### 8. `TRANSLATION_DB_CAP = 6` leaves zero headroom on `/t/**`

`evictTranslationDbs` is `while (translationDbs.size >= TRANSLATION_DB_CAP)`
(`quran.worker.ts:288`) and runs *before* the insert (`:330-332`) → resident maximum equals the cap
exactly.

Scope: Arabic scripts live in a separate `sources` map (`quran.worker.ts:67`), so the "+1 for the
primary" applies **only on `/t/**` routes**. There, 5 extras + primary fills all 6 slots and any other
translation read evicts one. On the Arabic route, cap 6 leaves one spare slot.

**Recommendation:** 8, or cap extras at 4.

### 9. Retention caps overshoot the corpus by ~4x

Measured from `src/lib/data/translations.json`: **115 translations, 194,895,872 B = 185.9 MiB total,
largest single DB 12.43 MiB.**

Plan's `CAP_COUNT = 512` / `CAP_BYTES = 768 MiB` can never bind and invites browser-side OPFS quota
eviction, which the app does not control.

**Recommendation:** `CAP_COUNT = 128`, `CAP_BYTES = 256 * 1024 * 1024` — covers the whole corpus with
margin while keeping a real backstop. Combine with the pinning fix in §7, which is the substantive
reason to touch this file at all.

---

## Test-plan corrections

### 10. The lockstep note is right, but the coverage risk is inverted

`src/lib/workers/__tests__/opfs-retention.test.ts` mirrors the constants at `:6-8`, loops
`CAP_COUNT + 1` at `:41`, and asserts `<= CAP_COUNT` at `:158`.

Updating the mirrors keeps every test **passing**. The real hazard is the 100-candidate property test
at `:150-160`: at any cap ≥ 100 nothing is evicted, so `expect(evicted).not.toContain("uthmani")`
passes vacuously and the test stops proving anything.

**Fix:** rewrite that test to generate `CAP_COUNT + N` candidates rather than a hard-coded 100.

### 11. Missing cases in plan §Tests

- Mirror-`$effect` adopt path: a `?more=` deep link must not be blanked before being re-written (§3).
- Pin-protection: `computeEvictions` must not evict a currently-selected extra id (§7).

---

## Out of scope — state explicitly in the plan

- `VerseTools` copy/share use only the `text` prop (`VerseTools.svelte:17,32,40`) → extras are never
  included in copied or shared verse text.
- Extras work offline only if their DB already sits in OPFS; otherwise the chain in §6 falls to the
  API and, offline, throws.

---

## Re-verified as correct in the plan (no action)

- `readRange(from, to, validator, source, onStatus)` argument order — `worker-client.ts:348-353`.
- Both `mode` strip sites identified: `src/service-worker.ts:45` and `src/lib/offline/keys.ts` — each
  currently `if (key === "mode") continue;` and each needs `more`.
- `Ayah.key` exists as the join key — `src/lib/data/quran-types.ts:179`.
- Disk-cache key excludes the query string; `surah*For(ctx,…)` helpers unchanged; nav-guard unaffected.
- `docs-divergence-guard.test.ts` exists at `src/routes/(application)/app/__tests__/`.
- Locales are `en` + `ar` only (`src/lib/i18n/locales.ts:1`) — the 13 new message ids need both.
- Query params do **not** reach `<link rel="canonical">`: `Seo.svelte:56` builds `SITE.url + path`,
  and every app caller passes a path-helper result (`SurahPageRoute.svelte:34-36`,
  `page/[n]/+page.svelte:12`, `t/[lang]/[translator]/page/[n]/+page.svelte:20`).
