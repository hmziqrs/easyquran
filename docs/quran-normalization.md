# EasyQuran — Quran source normalization

> Status: **implemented for the web** (SSG, browser Worker, UI, and web search).
> The Rust/API work remains outside this pass. Scope: a source-independent canonical
> view over the Arabic Quran databases accepted by EasyQuran — currently corpora
> using the same 6,236-ayah Hafs/Medina numbering — so that display, search, and
> range queries behave identically no matter which registered source backs them.
>
> This document owns the canonical view, the source-profile registry, and the
> adapter contract. `quran-api.md` owns the Rust runtime and the HTTP contract;
> `quran-web-delivery.md` owns browser storage and SSG. Where they disagree
> about basmala handling, **this document wins** and the other two get amended.
>
> Planned web-only metadata migration:
> [`quran-ssg-optimization-plan.md`](./quran-ssg-optimization-plan.md) replaces
> the web build's XML parsing and checked-in names/coordinates projections after
> implementation. The canonical text/opener rules here and the backend's
> retained `quran-data.xml` are unchanged.

---

## 0. Recommendation

**Do not modify any database.** The Tanzil corpus is correct as it stands — its
ayah numbering is right (§1.1), its 95/97 orthography is right (§6), and its
conversion to SQLite is byte-clean. There is no data-repair task here.

The problem is that the basmala is packaged _inside_ ayah 1's text for 112
surahs, and every consumer has to cope with that on its own. The recommendation
is to stop coping ad hoc and add one read-time view:

1. **Adopt source-qualified `raw` / `body` / `opener` accessors as the only way
   layers read a corpus** (§4). `raw` stays byte-identical; `body` and `opener`
   are derived.
2. **Represent an embedded prefix with a small scalar cut record per surah**
   (§4.1), computed by a skeleton walk (§4.2) and converted to each runtime's
   native string indices at load. Never a string rewrite, never a hardcoded
   character count, never a newline split (§5.2).
3. **Describe each source with a profile instead of assuming Tanzil** (§5), so an
   additional upstream corpus is a registry entry rather than a boot failure.
4. **Point display and search at `body` + `opener`** (§7, §8), which removes the
   duplicated basmala header and fixes the misattribution of 112 search hits.
5. **Share one fixture set between the Rust and TypeScript implementations**
   (§8) — divergence between them is the most likely way this breaks.

Everything below is the justification and the measured evidence for those five
points.

### 0.1 Web implementation map

The web implementation deliberately separates these concerns:

| Concern                                         | Shared module                                      | Used by                                  |
| ----------------------------------------------- | -------------------------------------------------- | ---------------------------------------- |
| Serialized Quran domain values                  | `web/src/lib/data/quran-types.ts`                  | SSG, Worker, UI, wire decoders           |
| Canonical search hit contract                   | `web/src/lib/quran/search/types.ts`                | Worker, API decoder, UI, tooling         |
| Artifact, script, packaging, and schema profile | `web/src/lib/quran/view/source-profiles.ts`        | manifest, SSG, Worker, tooling           |
| Typed SQL queries and row decoding              | `web/src/lib/quran/sql.ts`                         | Node and sqlite-wasm query runners       |
| Canonical coordinates from `quran-data.xml`     | `web/src/lib/data/quran-coordinates.json`         | SSG and Worker source validation         |
| Canonical `raw` / `body` / `opener` view        | `web/src/lib/quran/view/`                          | surah/range rendering and search         |
| Product source roles                            | `web/src/lib/quran/source-plan.ts`                 | SSG defaults, Worker, fixture generation |

`runQuery()` and `runOne()` are platform-neutral. Node supplies a cached
`node:sqlite` prepared-statement runner; the browser supplies a sqlite-wasm
object-row runner. Query text and decoding therefore exist once, including for
the fixture generator.

To register a future IndoPak or Tajweed source:

1. add its stable source ID (script style is a separate value);
2. add a database adapter only if its schema differs from Tanzil's;
3. add one registered source profile with artifact and opener packaging;
4. point a role in `source-plan.ts` at it when it should back reading or search.

The delivery manifest is derived from the profile registry. Platform adapters
do not need source-specific query code. A source with different numbering still
requires an explicit alignment design and cannot be registered by labelling it
IndoPak or Tajweed alone.

---

## 1. The problem

The corpora in scope agree on the same 6,236 numbered ayahs. They disagree on
where the **basmala** lives. Tanzil glues it onto the front of ayah 1 for 112
surahs; quran.com-shaped sources can carry its presence as chapter metadata,
and other exports may use a separate row. The numbered verse identity is the
same; only the opener's packaging differs.

This is deliberately narrower than “every Arabic Quran corpus.” Other numbering
traditions exist, as Tanzil's own note explains. A source with different ayah
numbering is outside this adapter: it needs an explicit numbering/alignment
design before it can be registered, not a more permissive basmala detector.

That packaging difference leaks into every layer that touches the corpus:

- **Display** — a reader that renders a basmala header _and_ the Tanzil ayah 1
  shows it twice.
- **Search** — searching `بسم الله الرحمن الرحيم` against simple-clean returns
  **114 rows**: 112 embedded prefixes, the genuine 1:1, and 27:30 (the basmala
  inside Sulayman's letter). All 114 are real Quran text, but 112 of them are
  reported as _numbered ayah_ hits when they are unnumbered surah openers — the
  attribution is an artifact of storage shape (§7).
- **Row identity** — a source that adds one row for each of the 112 unnumbered
  openers carries 6,348 rows rather than 6,236, and its global indices and
  juz/page/ruku boundaries drift against `quran-data.xml`. Separate-row exports
  need not all use that exact row convention (§5).

Adding a third source today is not supported by the fixed `Script` enum, paths,
or source assumptions. More importantly, the current classifier is not a safe
detector: after recognizing 1:1 by exact equality and hardcoding surah 9 as
`none`, it labels every other surah `embedded-prefix` without checking that a
prefix exists. A quran.com-shaped 6,236-row source would therefore be
misclassified. Prefix detection in Rust is new work, not behavior already
supplied by the `1 / 112 / 1` assertion.

### 1.1 What is _not_ wrong: the ayah numbering

Reading the raw rows invites a specific and very natural misdiagnosis — that the
basmala occupies ayah 1 and has pushed the real first ayah down to ayah 2. It
has not. The numbering in this database is correct, and no data needs repair.

Verified against `quran-uthmani.sqlite` at the boundary between surahs 1 and 2:

```text
index  sura  aya  text
   7     1    7   صِرَٰطَ ٱلَّذِينَ أَنْعَمْتَ … وَلَا ٱلضَّآلِّينَ
   8     2    1   بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ الٓمٓ
   9     2    2   ذَٰلِكَ ٱلْكِتَـٰبُ لَا رَيْبَ ۛ فِيهِ ۛ هُدًى لِّلْمُتَّقِينَ
```

Row `index=8` is a **single row holding both** the unnumbered basmala and
`الٓمٓ`, space-separated — one string, 45 characters, 5 words:

```text
scalarSlice(raw(uthmani,2,1), 0, 39)    بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ
scalarSlice(raw(uthmani,2,1), 39, 40)   U+0020 SPACE
scalarSlice(raw(uthmani,2,1), 40, end)  الٓمٓ
```

So `الٓمٓ` **is** ayah 1. Ayah 2 is `ذَٰلِكَ ٱلْكِتَـٰبُ…`, as it should be. Confirmed
across the whole corpus: **the only row whose entire text is the basmala is
`index=1`** — genuinely 1:1, Al-Fatiha's numbered first verse.

The layout that _would_ be broken, and which this database does not have:

```text
   8     2    1   بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ     ← does not exist
   9     2    2   الٓمٓ
```

That shifted form is exactly what Tanzil avoided by putting the basmala on the
first ayah's line — it keeps 6,236 rows without inventing `ayah 0` rows or
renumbering (§2.2), and it is consistent with the quran.com coordinate model
(§5.1).

The distinction that resolves the confusion: **the basmala is inside ayah 1's
`text`, but it is not inside ayah 1's `aya` number.** Row identity is correct;
only text packaging is not. That is why the fix is a read-time view (§4) and
never an edit to the data.

## 2. Fixed decisions

1. **Source files are never modified.** Not rewritten, not re-encoded, not
   NFC-normalized, not stripped. This already holds (`quran-api.md` §1.2) and
   this document does not relax it. Tanzil's licence permits verbatim copies
   only, which makes it a legal constraint as well as a correctness one.
2. **Normalization is a read-time view, never a stored artifact.** No third
   Arabic SQLite database is built (`quran-web-delivery.md` §1.2, `quran-api.md`
   §1.10 both depend on this). This is also what the upstream vendor instructs:

   > Applications that are using Tanzil Quran text are recommended to add a
   > newline after each Bismillah **on the fly (not in the Quran text file)** to
   > display Bismillahs in separate lines, just like in the Medina Mushaf.
   >
   > — [A Note on Bismillah](https://tanzil.net/docs/a_note_on_bismillah), Tanzil

   The embedded encoding is a deliberate file-format decision, not sloppiness:
   Tanzil keeps one ayah per line so the file has exactly 6,236 lines, and
   explicitly rejected both extra lines and aya-0 numbering to get there. The
   split is expected to happen at read time — which is exactly §4.

3. **Embedded-prefix views are expressed as scalar cut records, not rewritten
   strings.** See §4. This makes decision 1 mechanically enforceable rather than
   aspirational.
4. **Verbatim text stays reachable from every layer.** Normalization adds a
   view; it never replaces the raw accessor.
5. **Canonical opener semantics and source packaging are separate.** The
   canonical model says whether a surah has a numbered opener, an unnumbered
   header, or none. A source profile says how that opener is packaged in one
   corpus. Detection/adaptation is asserted against a registry entry; an
   unrecognized or incomplete profile is a load error, not a silent best guess.

## 3. Pre-implementation baseline

This was the baseline audited before the web implementation pass. The Rust rows
remain current; the web prototype and raw-search gaps are resolved by the shared
modules in §0.1 and retained here to explain the design decisions:

| Piece                                                  | Location                                    | State                                                                            |
| ------------------------------------------------------ | ------------------------------------------- | -------------------------------------------------------------------------------- |
| `Bismillah` enum (`FirstAyah`/`None`/`EmbeddedPrefix`) | `rust/.../quran/store.rs:57`                | ships, in the API and OpenAPI schema                                             |
| Partial classification + `1/112/1` assertion           | `rust/.../quran/loader.rs:325-352`          | ships, **assumes** every non-1/non-9 surah is embedded; does not test the prefix |
| `bismillah` on the web catalog entry                   | `web/src/lib/data/quran-types.ts:22`        | ships                                                                            |
| `showsBismillah()`                                     | `web/src/lib/data/quran.ts:72`              | ships                                                                            |
| `withoutBasmalaPrefix()` — skeleton-based stripper     | `web/src/routes/design/_variants/verses.ts` | **prototype only**, two defects — see §3.1                                       |
| Catalog `bismillah` derivation                         | `web/vite-plugin-quran.ts:109`              | ships, **hardcoded by surah number**, never reads the text                       |
| Search corpus construction                             | `web/src/lib/workers/quran.worker.ts:152`   | ships, indexes raw text                                                          |

The `withoutBasmalaPrefix` prototype got the hard part — the skeleton walk —
right, and §4 records how it was promoted after correcting its defects. The
shipping Rust side still has no equivalent walk; its classifier still assumes
Tanzil's shape.

The two classifiers make the same unsafe assumption by different routes.
`loader.rs` recognizes only the special cases and assigns
`EmbeddedPrefix` in its fallback branch; `vite-plugin-quran.ts:109` derives the
same value from the surah number alone. Neither verifies that ayah 1 actually
starts with the corpus's own basmala. Both must use the profile-aware detection
defined below, and the detected result must be checked against the registered
source rather than treated as the registry itself.

### 3.1 Audit findings on the existing stripper

Executed against both corpora, all 114 first ayahs. Two defects, both blocking
promotion to `$lib`:

**It returns an empty string for surah 1.** `withoutBasmalaPrefix(raw(1,1))`
strips the whole verse, because 1:1 _is_ the basmala — 113 of 114 ayahs are
stripped, and surah 1's body comes back as `""`. Today this is masked: the only
caller, `displayVerses()`, early-returns on
`surah.bismillah !== "embedded-prefix"` so surah 1 never reaches it. The guard
lives in the caller, not the function. Moving the function to `$lib` without
carrying the guard blanks Al-Fatiha's first verse.

**It is completely inert on simple-clean** — 0 of 114 ayahs stripped. The
`BISMILLAH` constant spells the definite articles with `ٱ` (U+0671 ALEF WASLA);
simple-clean uses `ا` (U+0627). Alef wasla is a _letter_, not a combining mark,
so the skeleton does not remove it and the prefix never matches. Search runs on
simple-clean, so a stripper wired up as-is would silently do nothing there while
appearing to work on the display corpus.

Both are fixed by the same rule (§4.2): take the reference basmala from the
corpus's own `index = 1` row rather than a shared constant, and treat surah 1 as
`PrefixCut = {0,0}` by definition.

## 4. The canonical view

Three source-qualified accessors over any registered corpus `c`:

```text
raw(c, s, a)   → verbatim numbered-ayah text from c                 [copy, SEO, fidelity]
body(c, s, a)  → numbered ayah minus an embedded opener             [display, search, highlighting]
opener(c, s)   → canonical kind + profile-declared text for c       [the header line]
```

```text
opener(c, s) = { kind: "verse",  text }   // surah 1 — the basmala IS ayah 1, numbered
             | { kind: "header", text }   // 112 surahs — set above ayah 1, unnumbered
             | { kind: "none"          }   // surah 9
```

The `kind` is canonical metadata for the supported numbering model and therefore
does not vary by source. How the text is packaged _does_ vary and belongs to the
source profile (§5). For an `embedded-prefix` corpus, `body` and `raw` differ at
ayah 1 of the 112 header surahs. For `chapter-flag` sources the numbered ayah is
already the body, so `body ≡ raw`; the profile must provide the header text by a
separate trusted mechanism. `opener(c, s).text` always uses corpus `c`'s
orthography. Display asks the Uthmani corpus; simple-clean search asks the
simple-clean corpus.

### 4.1 PrefixCut — scalar contract, native runtime slices

For each surah whose source packaging is `embedded-prefix`, the skeleton walk
produces this conceptual record:

```text
PrefixCut {
  openerEndScalar,   // first scalar after the exact opener text
  bodyStartScalar,   // first scalar of the numbered ayah, after separator whitespace
}
```

Both fields count **Unicode scalar values from the start of `raw(c,s,1)`**.
They are not UTF-8 byte offsets and not JavaScript UTF-16 indices. This is the
portable fixture and review unit. The relation is:

```text
opener text = scalarSlice(raw, 0, openerEndScalar)
separator   = scalarSlice(raw, openerEndScalar, bodyStartScalar)
body        = scalarSlice(raw, bodyStartScalar, end)
```

Non-embedded packages use `{0, 0}`. Every ayah after ayah 1 is a straight
passthrough. The table is per corpus, not global: Uthmani and simple-clean are
different orthographies, so one corpus's cuts cannot be reused for another.

Each runtime converts the scalar record once at load and stores native indices:

- Rust records UTF-8 byte boundaries from `char_indices()` and slices `&str` only
  at those boundaries.
- TypeScript records UTF-16 code-unit boundaries while iterating code points and
  uses those values with `slice()`.

The current Arabic data happens to be entirely in the BMP, so scalar and UTF-16
counts coincide today; that is an observed property, not the contract. Shared
fixtures assert the scalar cuts **and** the resulting opener/body strings. They
must never feed the scalar number directly to a Rust byte slice.

For `embedded-prefix`, this cut record is the whole text mechanism. Other
packages additionally need the opener provider declared in §5. Consequences:

- **Non-destructive by construction.** A slice cannot rewrite a character. There
  is no code path where a mark is reordered, a tatweel dropped, or NFC applied —
  the failure mode `quran-api.md` §3.3 spends a section warning about is not
  merely tested against, it is unrepresentable.
- **Small.** At most two native integers per surah, computed once at load. There
  is no second in-memory copy of either corpus. Rust returns borrowed slices;
  JavaScript substring allocation remains engine-dependent and is not part of
  the correctness contract.
Measured against both current Tanzil sources:

|                                          | Uthmani                                           | simple-clean             |
| ---------------------------------------- | ------------------------------------------------- | ------------------------ |
| surahs with `bodyStartScalar > 0`        | 112                                               | 112                      |
| surahs with `bodyStartScalar = 0`        | 1, 9                                              | 1, 9                     |
| distinct opener ends (scalars)           | **39**, and **40** for surahs 95 and 97           | **22** for all 112       |
| distinct body starts                     | **40**, and **41** for surahs 95 and 97           | **23** for all 112       |
| UTF-8 byte cuts `(openerEnd, bodyStart)` | **(75,76)**, and **(77,78)** for surahs 95 and 97 | **(41,42)** for all 112  |
| reference basmala                        | `بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ`         | `بسم الله الرحمن الرحيم` |

The 41 is not an anomaly to be smoothed out — see §6. Note it has **no analogue
in simple-clean**, which has a single body start: that corpus carries no
diacritics, so the 95/97 shadda does not exist there and cannot be recovered from
it. Any `opener(c,s)` text intended for display must come from the Uthmani
corpus/profile.

### 4.2 Computing PrefixCut: the skeleton walk

The stored basmala and any header constant are **not byte-identical**, in two
independent ways. Comparing them directly fails, and "fixing" either side means
editing Quranic text. Verified on the current source:

```text
web BISMILLAH constant   0628 0650 0633 0652 0645 0650 0020 0671 0644 0644 064e 0651 …
database ayah 1:1        0628 0650 0633 0652 0645 0650 0020 0671 0644 0644 0651 064e …
                                                                        ^^^^^^^^^
                              fatha-then-shadda  vs  shadda-then-fatha
```

plus the verse text writes `ٱلرَّحْمَـٰنِ` with a tatweel (U+0640) carrying the
superscript alef where the constant has none. Skeletons match; bytes do not.

So the comparison runs over a **skeleton** — the string with every combining
mark, Quranic annotation sign and tatweel removed — `ً-ٟ` (harakat and
tanwin), `ـ` (tatweel), `ٰ` (superscript alef), `ۖ-ۭ`
(annotation signs) — leaving base letters and spaces. Match on the skeleton,
then walk the original counting only base characters to map the cut point back.
After taking any trailing marks on the final base letter, record
`openerEndScalar`; skip separating whitespace and record `bodyStartScalar`.
The separator is preserved as a raw subslice for reconstruction tests even
though neither display unit includes it.

**If the skeleton does not match where the registered profile expects an
embedded prefix, loading fails.** During untrusted detection the candidate cut
is `{0,0}`; the registry comparison then turns the mismatch into a named load
error. Never silently guess at Quranic text. For a profile that does not expect
an embedded prefix, no skeleton cut is attempted.

The character class is written with `\u` escapes deliberately: spelled as literal
ranges it silently swallows the Arabic-Indic digits at U+0660–0669, which sit
between two of its ranges.

Two rules make this work across corpora, both from the §3.1 audit:

**For the supported numbering model, take the reference basmala from the
corpus's own canonical `1:1` row. Never from a shared cross-corpus constant.** A
registered embedded-prefix corpus must have a numbered 1:1 basmala or an
explicit equivalent reference in its profile; otherwise it is rejected. A
shared constant is what makes the current prototype inert on simple-clean: the
constant uses `ٱ` U+0671 ALEF WASLA, simple-clean uses `ا` U+0627, and alef wasla
is a letter that the skeleton does not remove. Reading the reference in-band
also removes the constant as a thing that can drift from the data.

**`PrefixCut[1] = {0,0}` by definition, before any matching runs.** Surah 1's
ayah 1 _is_ the basmala, so the walk would legitimately match the entire verse
and return an empty body. That is a property of the algorithm, not a bug to
patch downstream — so the guard belongs inside the source adapter, not in each
caller. Equivalently: `bodyStartScalar[s] > 0` only where
`packaging(c,s) == embedded-prefix` (invariant 7, §9).

## 5. Source profiles

A source profile describes how one corpus packages the canonical opener and how
exact opener text can be obtained. It is not the canonical opener model itself.

| packaging         | meaning                                                                     | seen in                                                                        |
| ----------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `embedded-prefix` | basmala prepended to ayah 1's text                                          | Tanzil, suras 2–114                                                            |
| `numbered-ayah`   | opener text is the numbered ayah itself                                     | Tanzil 1:1                                                                     |
| `chapter-flag`    | corpus holds 6,236 numbered ayahs; the opener is a boolean on the _chapter_ | quran.com / Quran Foundation API (`bismillah_pre`)                             |
| `separate-row`    | basmala is its own unnumbered row (often aya 0)                             | named by Tanzil as an encoding it declined; not yet observed in a file we hold |
| `absent`          | source contains no opener for this surah                                    | surah 9                                                                        |

`chapter-flag` is presence metadata, not a text provider. A profile using it must
also identify an exact, trusted opener-text source: a chapter text field, a
companion opener artifact, or an explicitly registered opener set in the same
orthography. A boolean plus `raw(c,1,1)` is insufficient because it cannot
recover source-specific variants such as the shadda in the Uthmani openers for
95 and 97. A source without a trusted provider is incomplete and will not load.

```text
profile := {
  id,
  sourceId,                       // artifact/cache identity
  script,                         // orthography, independent of sourceId
  artifact: { repositoryPath, r2Path, sizeBytes },
  database,                       // typed schema/query adapter
  canonicalRowCount: 6236,
  packaging: { [sura]: packaging },
  expectedPackagingCounts,        // per source; Tanzil is 1 / 112 / 1
  openerProvider,                 // embedded slice, separate row, or trusted companion
}
```

Load order is explicit: select the registered source profile, run its schema
adapter to derive the observed row/package facts, then compare every observed
fact with the entry. Unknown sources, missing opener providers, or
observed/expected mismatches all fail closed with distinct errors.

Separately, canonical metadata for this numbering model is fixed at one
`verse`, 112 `header`, and one `none`. `SuraMeta`/the web catalog expose that
canonical kind. Per-source packaging never lives in the single shared
`Bismillah` field; it lives under the source profile. This permits Tanzil and a
chapter-flag source to back the same canonical surah without asking one enum
value to describe both storage layouts.

Two changes from today's behaviour:

- The `1/112/1` _packaging_ assertion becomes **one registry entry**, not a
  global invariant. Tanzil keeps asserting exactly one numbered-ayah, 112
  embedded-prefix, and one absent package and still fails closed if its own
  source drifts. Another source asserts its own expected packaging.
- Row-shape adapters run **before** canonicalization, for shapes that need one.
  A `separate-row` source would be remapped to canonical `(sura, aya)` over
  1..6236 — opener rows retained by the profile's provider rather than dropped,
  global indices recomputed. A source with 112 such extra rows would contain
  6,348 rows, but `separate-row` does not itself imply one universal source row
  count. `chapter-flag` and `embedded-prefix` sources need no numbered-row
  remapping (§5.1). `quran-data.xml` is the arbiter of canonical ayah counts for
  every registered source; it already validates against the Tanzil DB (114/114
  `start` offsets, 15 sajdas, 604 pages, 30 juz, 240 hizb quarters, 556 rukus).

`separate-row` is the shape Tanzil names and declines to use — "giving number 0
to Bismillahs". It is retained here as a defensive case, not a known source.

### 5.1 quran.com API — verified coordinate alignment

The Quran Foundation API exposes the _same_ canonical coordinates by a different
route: 6,236 numbered ayahs with opener presence as a chapter-level boolean, so
`2:1` is `الٓمٓ` with no basmala in the verse text. Its `bismillah_pre` is
`false` for surahs 1 and 9 and `true` for the other 112.

Critically, **the API's global index is the same index we already use.**
Spot-checked `verse_index` values against the Tanzil DB's
`"index"` column:

| verse | quran.com `verse_index` | Tanzil global |     |
| ----- | ----------------------: | ------------: | --- |
| 1:1   |                       1 |             1 | ✓   |
| 1:7   |                       7 |             7 | ✓   |
| 2:1   |                       8 |             8 | ✓   |
| 2:255 |                     262 |           262 | ✓   |
| 114:6 |                    6236 |          6236 | ✓   |

So `verse_index ≡ our global index ≡ quran-data.xml start+1`, and
`(chapter_id, verse_number) ≡ (sura, aya)`. **No numbered-ayah re-indexing
adapter is needed for the API shape.** An opener adapter is still required: the
chapter boolean establishes presence but does not supply exact opener text.
Until a concrete export and its opener provider are profiled, quran.com remains
an alignment example rather than a loadable registry entry.

Two notes on adopting their vocabulary wholesale:

- `bismillah_pre` is **less expressive than our existing `Bismillah` enum**. It
  is `false` for both surah 1 and surah 9, collapsing "the basmala is ayah 1"
  and "there is no basmala" into one value. Our canonical three-value opener
  kind already distinguishes them. Map `bismillah_pre` onto the source's
  packaging profile and combine it with canonical metadata; do not adopt the
  boolean as the internal model or treat it as opener text.
- A quran.com-shaped **schema** (a derived `verses` table holding pre-stripped
  Tanzil text) is not adoptable here regardless of its merits: it materializes
  modified ayah text, which fixed decisions 1 and 2 forbid, and exceeds Tanzil's
  verbatim-copy licence. The canonical read-time view yields the same
  presentation without materializing rewritten Tanzil ayahs.

### 5.2 Do not split on a newline

Guidance circulating for Tanzil imports says to separate the basmala on an
embedded newline (`row.text.split('\n')`). **That does not work on this data.**
Verified on both artifacts:

|                                                                            | newlines found |
| -------------------------------------------------------------------------- | -------------: |
| `quran-uthmani.sqlite`, all 6,236 rows                                     |              0 |
| `sql/quran-uthmani.sql`, all 6,236 quoted values (literal or `\n`-escaped) |              0 |

The separator in `2:1` is a single U+0020 space. Tanzil's own wording is that
applications should "add a newline … on the fly" — the newline is an output the
consumer _produces_, not a delimiter the file provides. A splitter written that
way silently returns the whole row unchanged, or throws for all 112 surahs if it
asserts on the split result.

Hardcoded character offsets (`text.substring(39)`) are equally wrong, for the
reasons in §4.2 — and measurably so: the real cut lands at **40** for 110 surahs
and **41** for surahs 95 and 97. The skeleton walk is the only approach here that
survives both the orthographic variation and the absence of a delimiter.

Detection runs at load and is compared to the registry. Mismatch is a load
error naming both sides, in the style of the existing `Invariant` messages.

## 6. Surahs 95 and 97 are not a bug

Their ayah 1 begins `بِّسْمِ` — U+0628 U+0651 U+0650 — carrying a shadda the
other 110 do not have. This is **correct** and must survive normalization.

Word-initial shadda appears 3,724 times in this corpus (`مِّن`, `لَّا`,
`رَّبِّهِمْ`…). It is how Uthmani script marks _idgham_, assimilation into the
preceding word's final consonant. The basmala case is that rule applied across a
surah boundary:

| preceding surah ends  | final ب          | next surah's ayah 1 |
| --------------------- | ---------------- | ------------------- |
| 13 → `ٱلْكِتَـٰبِ`    | kasra            | `بِسْمِ`            |
| 14 → `ٱلْأَلْبَـٰبِ`  | kasra            | `بِسْمِ`            |
| 34 → `مُّرِيبٍۭ`      | tanwin           | `بِسْمِ`            |
| **94 → `فَٱرْغَب`**   | **no diacritic** | **`بِّسْمِ`**       |
| **96 → `وَٱقْتَرِب`** | **no diacritic** | **`بِّسْمِ`**       |

Surahs 94 and 96 are the only two in the Quran ending in a _majzoom_ (jussive) ب.
Their final ب carries no diacritic at all — verified, U+0628 with nothing
following it — precisely because it merges into the ب of the basmala under the
Arabic idgham rule. The three surahs ending in a _voweled_ ب correctly take no
shadda. Five for five against the phonological rule.

Tanzil documents this directly, and treats it as evidence that the basmala is
Quran text rather than decoration:

> A strong indication for this fact is that Bismillahs at the beginning of two
> suras, i.e. suras 95 and 97, are written slightly differently, with a Shadda
> above Bah. The reason for adding these Shaddas is that the preceding suras end
> with a Majzoom letter Beh, causing the two Behs […] to be merged by the Arabic
> Idgham rule […] Note that the letter Beh at the end of the above two suras have
> no diacritic in Medina Mushaf because of being merged to the succeeding Beh of
> Bismillah.
>
> — [A Note on Bismillah](https://tanzil.net/docs/a_note_on_bismillah), Tanzil

Verified not to be a conversion artifact either: the shadda is present in the
upstream dump at `db/quran/tanzil/arabic/sql/quran-uthmani.sql`, and all 6,236
rows of that dump are byte-identical to `quran-uthmani.sqlite`.

One incidental trap: surah 96's last ayah ends with the sajda mark ۩ (U+06E9)
_after_ the ب. Any "what letter does this surah end with" heuristic has to skip
annotation signs, or it reads 96 as ending in ۩ and misses the idgham.

Requirements that follow:

1. `opener(uthmani,95)` and `opener(uthmani,97)` return the **shadda'd**
   spelling, not a shared constant. A single hardcoded `BISMILLAH` string renders
   the wrong orthography for these two.
2. Prefix matching must accept both spellings. The skeleton walk (§4.2) already
   does — shadda is a combining mark and is skeletonized away — which is
   precisely why the scalar body starts there are 41 rather than 40.
3. A constant-string prefix match would silently skip exactly these two surahs.
   Do not introduce one.

## 7. Effects on search

Search indexes `body` **plus the openers as their own units** — not `body`
alone. The distinction matters, and Tanzil's documentation is what settles it:
the basmala of suras 2–114 is _Quran text_, merely unnumbered.

> While Bismillahs are not individually numbered in suras 2 to 114, they are
> considered as part of the Quran text in Medina Mushaf. […] Tanzil Quran text
> closely follows the same standards […] In particular, it considers Bismillahs
> as part of the Quran text.

So dropping them from the index would be under-reporting real content. The fix
is attribution, not exclusion. Measured on simple-clean, `بسم الله الرحمن الرحيم`
returns **114** rows today:

|                                        |       now (`raw`) | `body` only |        `body` + openers |
| -------------------------------------- | ----------------: | ----------: | ----------------------: |
| surahs 2–114 ayah 1                    | 112, as ayah hits |           0 | 112, as **opener** hits |
| genuine 1:1                            |                 1 |           1 |                       1 |
| 27:30 (mid-ayah, in Sulayman's letter) |                 1 |           1 |                       1 |

`raw` and `body + openers` return the same 114 content units for this exact
phrase; `body` alone returns only the two numbered ayahs. The difference between
the two complete indexes is what a hit _claims_: today the API says "surah 2
ayah 1 matches", which misattributes an unnumbered opener to a numbered ayah and
makes ayah 1 of 112 surahs look like it begins with words it does not begin with.
As opener hits they carry no ayah number, the way the Mushaf sets them.

27:30 is byte-identical to 1:1 in the Uthmani text and is the only non-first
ayah containing the basmala in simple-clean. It is numbered verse content and
must keep matching as an ordinary ayah hit.

An `ayah` hit carries `raw(c,s,a)` in the response even though matching runs over
`body(c,s,a)`. Its highlight is computed against that exact body occurrence and
rebased by the runtime-native body start before being applied to `raw`; rerunning
the query against all of `raw` is wrong when the same phrase occurs in both the
opener and body (for example the body of 55:1). An `opener` hit carries the exact
`opener(c,s).text`, and its highlights are relative to that string.

The wire contract becomes an explicit tagged union, not merely a discriminator
added to the existing ayah-only DTO:

```text
SearchHit = {
  kind: "ayah",
  ayah: Ayah,                         // existing raw numbered-ayah DTO
  highlights: [{ start, end }],       // UTF-16 offsets into ayah.text
} | {
  kind: "opener",
  key: "opener:<surah>",              // stable identity; never a VerseKey
  surah,
  anchorAyah: 1,                      // navigation target, not hit attribution
  text,                               // exact opener text in requested script
  highlights: [{ start, end }],       // UTF-16 offsets into text
}
```

The web adapter exposes the same union instead of requiring `ayah`, `key`, and
`globalIndex` on every result. The shipped `/quran/search` response already
requires `kind` on every hit (today only `kind: "ayah"` is emitted; the
`opener` arm is the deferred forward shape above), so the union is the contract,
not a relaxation. The immutable anchor for cache identity is the pinned sha256
digest surfaced on `/health/ready` — the Quran corpus is immutable, so there
is no corpus or search label to bump. Any change to this contract lands
as a coordinated update to `quran-api.md`, OpenAPI, decoder, component, and
fixtures.

Ordering is stable by `(anchorGlobal, rank)`: an opener uses the surah's
`startGlobal` with rank 0; a numbered ayah uses its `globalIndex` with rank 1.
Thus a header sorts immediately before ayah 1. `total`, `limit`, and `offset`
count both hit kinds in that order. Clicking an opener navigates to the surah's
first ayah but the UI labels it “Surah opener,” never “ayah 1.”

The opener and numbered body are distinct search units. A phrase spanning the
storage-only boundary between them (for example the end of the basmala followed
by the first word of 2:1) intentionally stops matching. Such a phrase is not
contiguous within either canonical unit, and a chapter-flag source could never
match it. Pin this behavior in both implementations.

Changing the indexed units from 6,236 raw rows to canonical ayahs plus openers
changes both match attribution and some boundary-spanning matches. The rule
set is frozen and carries no label; correctness is pinned by the shared
fixtures and the corpus sha256, not by bumping a tag, even before the
query-normalization correction in §7.1.

Both search implementations — `quran.worker.ts:152` and the Rust in-memory
corpus — build their index from the same view, or the "same search behavior
online and offline" requirement (`quran-web-delivery.md` §1.4) breaks.

### 7.1 Separate defect found during audit: copied Uthmani queries diverge

This is not caused by the basmala and is not fixed by anything above, but it
lives in the same code path and should be fixed in the same pass.

The web `normalizeArabic` (`web/src/lib/quran/search/normalize.ts:39`) removes
`\p{Mn}\p{Me}\p{Cf}` and folds `آأإ→ا`, `ى→ي`, `ة→ه`. It does **not** handle two
characters that are pervasive in the Uthmani corpus:

| character             | category               | why it survives normalization                  | count in Uthmani | count in simple-clean |
| --------------------- | ---------------------- | ---------------------------------------------- | ---------------: | --------------------: |
| `ٱ` U+0671 ALEF WASLA | `Lo` (letter)          | not a combining mark; no fold rule             |           13,819 |                     0 |
| `ـ` U+0640 TATWEEL    | `Lm` (modifier letter) | **not** `Mn`/`Me`/`Cf`, so the class misses it |            6,848 |                     0 |

The search corpus is simple-clean, which contains neither, so corpus-side
indexing is unaffected. The **query** side is not:

```text
normalizeArabic(uthmani 1:1)       →  "بسم ٱلله ٱلرحمـن ٱلرحيم"
normalizeArabic(simple-clean 1:1)  →  "بسم الله الرحمن الرحيم"
equal? false
```

The reader displays Uthmani. A user who copies the full displayed basmala and
pastes it into the offline web search therefore gets zero results for text that
is demonstrably present. It fails silently and looks like missing content rather
than a normalization gap.

The Rust implementation is **already different**, not a byte-for-byte mirror:
it folds U+0671 and drops U+0640, but maps U+0670 SUPERSCRIPT ALEF to a real alef.
The web property class drops U+0670 because it is `Mn`. Consequently, adding the
two missing web rules alone would still leave online and offline normalization
different: Rust produces an extra alef in `ٱلرَّحْمَـٰن`, while web/simple-clean
does not.

The minimum coordinated correction for this pass is therefore:

1. fold `ٱ → ا` and strip `ـ` in TypeScript;
2. settle U+0670 identically in both runtimes — for the current simple-clean
   matching contract, retain the existing web behavior and drop it in Rust;
3. the normalization rule set is frozen — pin the change through the regenerated
   fixtures and the corpus sha256 instead of bumping a label;
4. regenerate `__fixtures__/queries.json` and consume the same fixture file from
   Rust tests, not only from web tooling;
5. add the full Uthmani 1:1 string as a query fixture and assert it normalizes to
   the simple-clean 1:1 value and returns the canonical 114-unit result set.

U+0670 is genuinely ambiguous across the whole corpus: in some Uthmani words it
corresponds to a written simple-clean alef, while in `الرحمن` it does not. The
five rules above fix the demonstrated basmala regression and runtime parity; they
do **not** claim complete Uthmani-to-simple orthographic transliteration. If the
product requires every arbitrary Uthmani paste to match its simple-clean form,
that needs a separately measured source-aware search design rather than another
unreviewed one-character fold.

## 8. Where the layer lives

Three runtimes consume the corpus and all three need the same answers:

| Runtime        | Entry point                           | Change                                                                                                              |
| -------------- | ------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Rust backend   | `rust/.../quran/loader.rs`            | **not part of the web implementation pass**; consume the shared fixtures when implemented                           |
| SSG            | `web/src/lib/server/quran-sqlite.ts`  | adapt `node:sqlite` to the shared runner; validate the selected reader profile; prerender raw text plus descriptors |
| Browser worker | `web/src/lib/workers/quran.worker.ts` | adapt sqlite-wasm to the shared runner; load source roles; build the canonical search corpus                        |
| Tooling        | `web/scripts/gen-*.ts`                 | generate coordinates, search, and scalar-cut fixtures through the same shared loaders, queries, views, and corpus  |

The shared TypeScript stripper is promoted out of `routes/design/_variants/` into
`$lib/quran/view/`, where the SSG reader adapter and worker can both reach it.
The design variants then import the real thing instead of keeping a private
copy.

### 8.1 HTTP and reader delivery

`quran-api.md` continues to own the wire contract and keeps `Ayah.text` raw. To
make the canonical view available to live clients, its surah metadata gains one
normalization descriptor per requested/available script:

```text
normalization: {
  openerKind: "verse" | "header" | "none",   // canonical, source-independent
  scripts: {
    uthmani: {
      bodyStartScalar,
      openerText,                              // exact Uthmani text; null for none
    },
    simpleClean: {
      bodyStartScalar,
      openerText,                              // exact simple-clean text; null for none
    },
  },
}
```

`bodyStartScalar` is 0 when the numbered ayah is already the body. Source
packaging remains internal unless a diagnostic endpoint needs it. The existing
public `bismillah` field is not widened with storage variants; it is deprecated
in favor of `openerKind`. The descriptor is anchored by the corpus sha256 on
`/health/ready`, not by any API or content label. The shipped `readSurah` and
`range` endpoints already carry this descriptor on their `normalization` field
(see `quran-api.md`).

The three display paths apply the same rules:

- A full-surah reader renders a `header` opener once and passes `body` for ayah
  1; `verse` and `none` render no separate header.
- A juz/page/range reader does the same whenever its group contains ayah 1. If a
  range starts later in a surah, it does not synthesize an opener.
- Search renders the hit unit from §7. Copy, share, SEO fidelity checks, and API
  `Ayah.text` continue to use `raw`.

The shared range grouper replaces the former `RangeReader` behavior that treated
the basmala as always inline. It also closes the live-fallback gap: the browser does
not need to assume Uthmani scalar cuts for a simple-clean or future registered
source. SSG, Worker, and live API derive or receive the descriptor for the exact
corpus they are using.

Rust and TypeScript necessarily hold two implementations of the skeleton walk.
They are kept honest by a **shared fixture file** — surah number, expected
`openerEndScalar`, `bodyStartScalar`, exact opener, separator, and expected first
20 scalars of `body` — checked by both test suites. Runtime tests additionally
assert that scalar-to-native conversion lands on UTF-8/UTF-16 boundaries and
produces those same strings. Native byte/code-unit offsets are intentionally not
compared across languages. Divergence between the two walks is the most likely
way this layer breaks, and the fixture is what catches it.

The web pass commits that fixture at
`web/src/lib/quran/view/__fixtures__/prefix-cuts.json` and checks all 228 source/
surah records. The Rust pass must consume this same file when implemented; until
then cross-runtime invariant 12 remains intentionally pending with the rest of
the Rust/API work. Regenerate the Worker-safe coordinate projection and both
committed web fixtures from `web/` with:

```sh
pnpm dlx vite-node --config vite.config.ts scripts/gen-quran-coordinates.ts
pnpm dlx vite-node --config vite.config.ts scripts/gen-search-fixtures.ts
```

## 9. Invariants to assert

At load, per registered source:

1. Canonical row count is 6,236 after any row-shape adaptation.
2. `(sura, aya)` keys and per-surah ayah counts match `quran-data.xml`.
3. The detected packaging split matches the registry entry for that source.
4. `body(c,s,a) == raw(c,s,a)` for every ayah where `a > 1`, and for every ayah
   whose package is not `embedded-prefix`.
5. For every embedded prefix,
   `openerText + separator + body(c,s,1) == raw(c,s,1)` byte-for-byte, where all
   three values are subslices of the original. This proves that the view only
   partitions text; it never rewrites it.
6. `bodyStartScalar[c,s] > 0` exactly when
   `packaging(c,s) == embedded-prefix`; `openerEndScalar <= bodyStartScalar`.
7. Canonical `openerKind(1) == "verse"`, `openerKind(9) == "none"`, and all
   other surahs are `"header"`, independent of source packaging.
8. Every `verse`/`header` opener has exact text in every corpus exposed for that
   script. A `chapter-flag` boolean without a trusted text provider fails load.
9. `body(c,s,1)` is non-empty and has no leading or trailing whitespace for all
   114 surahs. This catches the surah-1-blanked defect (§3.1) inside the adapter.
10. Prefix cuts are computed **per corpus**. An embedded-prefix corpus whose
    detected cut count is 0 is a hard error — the exact signature of the current
    simple-clean prototype's inertness.
11. Rust and TypeScript agree on every scalar cut and derived opener/separator/
    body fixture. Each runtime separately proves its native indices are valid.
12. Surah, range, and search presentation tests exercise the same descriptor;
    a range containing ayah 1 renders one header and a range starting at ayah 2
    renders none.

Invariants 4 and 5 make “we never alter text” checkable rather than a claim.
Invariants 9 and 10 exist because both prototype defects were silent — one
masked by a caller-side guard, the other a no-op that looked like success.

### 9.1 What the corpus audit cleared

A full sweep of both Arabic corpora, the metadata XML and all 115 translation
packs found **no structural or data defects**. Recorded so this is not re-run:

| check                                                                                          | result                                                                                                      |
| ---------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| row count, contiguous `index` 1..6236, unique `(sura,aya)`, sequential `aya` within each surah | pass, both corpora                                                                                          |
| ayah counts and `start` offsets vs `quran-data.xml`                                            | pass, 114/114                                                                                               |
| juz 30 · page 604 · ruku 556 · hizb-quarter 240 · manzil 7 · sajda 15                          | all present, monotonic, unique, every marker resolves to a real verse                                       |
| empty / untrimmed / double-space / control char / newline / tab / BOM / NBSP                   | none, both corpora                                                                                          |
| codepoint inventory                                                                            | 70 distinct (Uthmani), 37 (simple-clean); every one in the Arabic blocks — no stray Latin or punctuation    |
| 115 translation packs: 6,236 rows, contiguous index                                            | pass, 115/115                                                                                               |
| translation empty verses                                                                       | 4, in 3 files — exactly as `docs/translation-empty-verses.md` records                                       |

Uthmani is NFC-unstable in 5,782 of 6,236 rows, matching the documented figure.
That is a property of the source, so read-time views must never normalize or
rewrite the stored text.

All defects found by the audit are in _our code_, not the data: §3.1 and §7.1.

### 9.2 Validation: the proposed rule executed against all 228 first ayahs

The scalar partition at the core of §4.1/§4.2 was prototyped — reference basmala
from the corpus's own `raw(c,1,1)`, surah 1 forced to `{0,0}`, skeleton walk
mapped back onto the original — and run over ayah 1 of all 114 surahs in
**both** corpora. **0 failures.** This validates the scalar cuts and lossless
partition; native Rust byte-boundary and JavaScript UTF-16 conversion remain
implementation work covered by the fixtures in §8.

|                                                   | Uthmani                                | simple-clean       |
| ------------------------------------------------- | -------------------------------------- | ------------------ |
| stripped                                          | 112                                    | 112                |
| zero-cut                                          | 1, 9                                   | 1, 9               |
| distinct opener ends (scalars)                    | 39, 40                                 | 22                 |
| distinct body starts (scalars)                    | 40, 41                                 | 23                 |
| UTF-8 byte cuts `(openerEnd, bodyStart)`          | (75,76), (77,78)                       | (41,42)            |
| distinct opener texts                             | **2** — 110 plain, 2 shadda'd (95, 97) | **1**              |
| `opener + separator + body == raw`, byte-for-byte | 112/112                                | 112/112            |
| separator partitioned                             | exactly one U+0020                     | exactly one U+0020 |
| body non-empty and trimmed                        | 114/114                                | 114/114            |
| removed prefix skeleton-equals the basmala        | 112/112                                | 112/112            |
| corpora disagree on basmala presence              | none                                   | none               |

Reconstruction is lossless in both corpora: the separator between opener and
body is exactly one U+0020, retained by the partition for reconstruction and
rendered by neither canonical text unit. That is invariant 6 demonstrated
rather than asserted.

**Stress cases** — ayah 1 texts that would fool a looser matcher, all correct:

| surah  | body after cut                            | why it is a trap                                                                               |
| ------ | ----------------------------------------- | ---------------------------------------------------------------------------------------------- |
| 55     | `ٱلرَّحْمَـٰنُ`                           | ayah 1 _is_ a word that occurs inside the basmala; a greedy or last-occurrence matcher eats it |
| 96     | `ٱقْرَأْ بِٱسْمِ رَبِّكَ ٱلَّذِى خَلَقَ`  | contains `بِٱسْمِ` — the basmala's own opening letters — _after_ the cut                       |
| 87     | `سَبِّحِ ٱسْمَ رَبِّكَ ٱلْأَعْلَى`        | contains `ٱسْمَ`                                                                               |
| 95, 97 | `وَٱلتِّينِ…` / `إِنَّآ أَنزَلْنَـٰهُ…`   | shadda'd basmala, scalar body start 41 not 40                                                  |
| 9      | `بَرَآءَةٌ مِّنَ ٱللَّهِ…`                | opens with `بَ` and names Allah, but has no basmala — correctly untouched                      |
| 1      | `بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ` | ayah 1 is the basmala; cut forced to `{0,0}`, verse preserved whole                            |

The 29 muqatta'at surahs are the most sensitive to an off-by-one, since their
entire body is one to five base letters plus marks (`الٓمٓ`, `الٓمٓصٓ`, `كٓهيعٓصٓ`,
`حمٓ`, `قٓ`, `نٓ`, `طه`, `يسٓ`, `صٓ`, `الٓر`, `الٓمٓر`, `طسٓ`, `طسٓمٓ`). All came
through intact — a cut one position early or late would corrupt them visibly.

## 10. Open items

- **The quran.com source is not in the repo and has not been profiled.** §5.1
  describes the Quran Foundation _API_ model — 6,236 numbered ayahs, opener as a
  chapter flag, no basmala in `2:1` — which is inferred from the public API and
  schema, not measured from a corpus. A downloaded export may not match it; QUL
  and mushaf-layout exports are packaged differently from the API model. Profile
  the actual file before writing its registry entry: row count, ayah-number base,
  where the basmala sits per surah, whether `verse_index` matches ours (§5.1),
  and which trusted field or companion artifact supplies exact opener
  text, including the 95/97 spelling. Until then it has no profile and will not
  load — the intended behaviour under fixed decision 5.

  Note that an earlier concern that this project's Tanzil database itself carried
  the basmala "as ayah 1 in some places" was checked and did not hold; see §1.1.
  Do not carry that assumption into the quran.com profiling — measure it.

- Whether the second source becomes a published artifact or stays a
  build/verification input. This decides whether `/quran/scripts` and OPFS
  storage grow a third file. The corpus is immutable, so identity is the
  artifact sha256 only: the OPFS cache is keyed by that digest, and `/scripts`
  lists each artifact by its content hash with no per-build path segment in
  the download URL.
- **Complete arbitrary Uthmani-paste search.** §7.1 deliberately fixes the
  demonstrated basmala failure and online/offline parity without pretending
  that U+0670 has one context-free mapping to simple-clean. Supporting every
  copied Uthmani phrase needs a measured source-aware search design, not
  another unreviewed one-character fold.

---

## 11. References

- [A Note on Bismillah](https://tanzil.net/docs/a_note_on_bismillah) — Tanzil.
  Normative for §2, §5, §6 and §7: the numbering convention, why suras 2–114 are
  encoded with the basmala on the first ayah's line, the 95/97 shadda and its
  idgham rationale, and the instruction to split at read time rather than in the
  file.
- [Quran Foundation content API](https://api-docs.quran.foundation/docs/content_apis_versioned/4.0.0/get-chapter/)
  — `bismillah_pre` on the chapter resource; the
  [verse-by-key resource](https://api-docs.quran.foundation/docs/content_apis_versioned/4.0.0/verses-by-verse-key/)
  supplies `verse_index` / `verse_key`. Sources for §5.1. The API shape is not a
  profiled export, so packaging claims there remain limited to the public API
  surface and schema.
- `docs/quran-web-delivery.md` §1.2, §1.4 — files unaltered; identical search
  behaviour online and offline.
- `db/quran/tanzil/quran-data.xml` — ayah counts and navigation markers; the
  arbiter of canonical `(sura, aya)` for every registered source in scope.
