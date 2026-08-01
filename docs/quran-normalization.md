# EasyQuran — Quran source normalization (design plan)

> Status: **plan** (not yet implemented). Scope: a source-independent canonical
> view over Arabic Quran databases, so that display, search, and range queries
> behave identically no matter which upstream corpus backs them.
>
> This document owns the canonical view, the source-profile registry, and the
> adapter contract. `quran-api.md` owns the Rust runtime and the HTTP contract;
> `quran-web-delivery.md` owns browser storage and SSG. Where they disagree
> about basmala handling, **this document wins** and the other two get amended.

---

## 0. Recommendation

**Do not modify any database.** The Tanzil corpus is correct as it stands — its
ayah numbering is right (§1.1), its 95/97 orthography is right (§6), and its
conversion to SQLite is byte-clean. There is no data-repair task here.

The problem is that the basmala is packaged *inside* ayah 1's text for 112
surahs, and every consumer has to cope with that on its own. The recommendation
is to stop coping ad hoc and add one read-time view:

1. **Adopt `raw` / `body` / `opener` as the only way layers read the corpus**
   (§4). `raw` stays byte-identical and digest-guarded; `body` and `opener` are
   derived.
2. **Implement it as `bodyOffset` — 114 integers, one slice per surah** (§4.1),
   computed by a skeleton walk (§4.2). Never a string rewrite, never a hardcoded
   character count, never a newline split (§5.2).
3. **Describe each source with a profile instead of assuming Tanzil** (§5), so a
   second corpus is a registry entry rather than a boot failure.
4. **Point display and search at `body` + `opener`** (§7, §8), which removes the
   duplicated basmala header and fixes the misattribution of 112 search hits.
5. **Share one fixture set between the Rust and TypeScript implementations**
   (§8) — divergence between them is the most likely way this breaks.

Everything below is the justification and the measured evidence for those five
points.

---

## 1. The problem

Every Arabic Quran corpus agrees on the 6,236 ayahs. They disagree on where the
**basmala** lives. Tanzil glues it onto the front of ayah 1 for 112 surahs;
quran.com-derived dumps often carry it as a separate row or a separate field.
The verse text is the same; only its packaging differs.

That packaging difference leaks into every layer that touches the corpus:

- **Display** — a reader that renders a basmala header *and* the Tanzil ayah 1
  shows it twice.
- **Search** — searching `بسم الله الرحمن الرحيم` against simple-clean returns
  **114 rows**: 112 embedded prefixes, the genuine 1:1, and 27:30 (the basmala
  inside Sulayman's letter). All 114 are real Quran text, but 112 of them are
  reported as *numbered ayah* hits when they are unnumbered surah openers — the
  attribution is an artifact of storage shape (§7).
- **Row identity** — a source that gives the basmala its own row carries 6,348
  rows rather than 6,236, and its global indices and juz/page/ruku boundaries
  drift against `quran-data.xml`. (Not every second source does this — see
  §5.1.)

Adding a second source today does not produce subtly wrong output. It produces
a **hard boot failure**: `rust/backend/api/src/quran/loader.rs:346` asserts the
basmala split is exactly `1 / 112 / 1` and returns `Invariant` otherwise.

### 1.1 What is *not* wrong: the ayah numbering

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
raw(2,1)[0:40]   بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ
raw(2,1)[40:]    الٓمٓ
```

So `الٓمٓ` **is** ayah 1. Ayah 2 is `ذَٰلِكَ ٱلْكِتَـٰبُ…`, as it should be. Confirmed
across the whole corpus: **the only row whose entire text is the basmala is
`index=1`** — genuinely 1:1, Al-Fatiha's numbered first verse.

The layout that *would* be broken, and which this database does not have:

```text
   8     2    1   بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ     ← does not exist
   9     2    2   الٓمٓ
```

That shifted form is exactly what Tanzil avoided by putting the basmala on the
first ayah's line — it keeps 6,236 rows without inventing `ayah 0` rows or
renumbering (§2.2), and it is what the quran.com model warns against (§5.1).

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
3. **The canonical view is expressed as offsets, not strings.** See §4. This is
   what makes decision 1 mechanically enforceable rather than aspirational.
4. **Verbatim text stays reachable from every layer.** Normalization adds a
   view; it never replaces the raw accessor.
5. **The golden digests keep guarding the raw corpus** and are computed over
   raw text, before any view is applied (`quran-api.md` §3.3).
6. **A source is described, not assumed.** Shape detection produces a profile
   that is asserted against a registry entry. An unrecognized shape is a load
   error, not a silent best guess.

## 3. What already exists

This is a consolidation as much as a new build. Present today:

| Piece | Location | State |
|---|---|---|
| `Bismillah` enum (`FirstAyah`/`None`/`EmbeddedPrefix`) | `rust/.../quran/store.rs:57` | ships, in the API and OpenAPI schema |
| Load-time classification + `1/112/1` assertion | `rust/.../quran/loader.rs:325-352` | ships, **Tanzil-only** |
| `bismillah` on the web catalog entry | `web/src/lib/data/quran-types.ts:22` | ships |
| `showsBismillah()` | `web/src/lib/data/quran.ts:72` | ships |
| `withoutBasmalaPrefix()` — skeleton-based stripper | `web/src/routes/design/_variants/verses.ts` | **prototype only**, two defects — see §3.1 |
| Catalog `bismillah` derivation | `web/vite-plugin-quran.ts:109` | ships, **hardcoded by surah number**, never reads the text |
| Search corpus construction | `web/src/lib/workers/quran.worker.ts:152` | ships, indexes raw text |

The `withoutBasmalaPrefix` prototype got the hard part — the skeleton walk —
right, and it is promoted in §4. But it is not promotable as written; §3.1
records what an audit of it found. The other gaps are that the Rust side has no
equivalent, the classification is hardcoded to one source, and search uses none
of it.

Note the method mismatch on classification: `loader.rs` **derives** the basmala
shape from the text and asserts it, while `vite-plugin-quran.ts:109` computes
`num === 1 ? "first-ayah" : num === 9 ? "none" : "embedded-prefix"` from the
surah number alone. The Rust side would catch a source whose shape changed; the
web side would keep asserting `embedded-prefix` and strip a prefix that is not
there. Both must derive from the same detection.

### 3.1 Audit findings on the existing stripper

Executed against both corpora, all 114 first ayahs. Two defects, both blocking
promotion to `$lib`:

**It returns an empty string for surah 1.** `withoutBasmalaPrefix(raw(1,1))`
strips the whole verse, because 1:1 *is* the basmala — 113 of 114 ayahs are
stripped, and surah 1's body comes back as `""`. Today this is masked: the only
caller, `displayVerses()`, early-returns on
`surah.bismillah !== "embedded-prefix"` so surah 1 never reaches it. The guard
lives in the caller, not the function. Moving the function to `$lib` without
carrying the guard blanks Al-Fatiha's first verse.

**It is completely inert on simple-clean** — 0 of 114 ayahs stripped. The
`BISMILLAH` constant spells the definite articles with `ٱ` (U+0671 ALEF WASLA);
simple-clean uses `ا` (U+0627). Alef wasla is a *letter*, not a combining mark,
so the skeleton does not remove it and the prefix never matches. Search runs on
simple-clean, so a stripper wired up as-is would silently do nothing there while
appearing to work on the display corpus.

Both are fixed by the same rule (§4.2): take the reference basmala from the
corpus's own `index = 1` row rather than a shared constant, and treat surah 1 as
`bodyOffset = 0` by definition.

## 4. The canonical view

Three accessors over any registered source:

```text
raw(s, a)   → verbatim ayah text, byte-identical to source   [copy, SEO, fidelity, digests]
body(s, a)  → ayah text minus any surah-opening basmala      [display, search, highlighting]
opener(s)   → how the surah's basmala should be presented    [the header line]
```

```text
opener(s) = { kind: "verse",  text }   // surah 1 — the basmala IS ayah 1, numbered
          | { kind: "header", text }   // 112 surahs — set above ayah 1, unnumbered
          | { kind: "none"          }   // surah 9
```

`body` and `raw` differ for exactly one ayah per surah — ayah 1 of the 112
embedded-prefix surahs. Everywhere else `body ≡ raw`.

### 4.1 bodyOffset — normalization as 114 integers per corpus

For each surah, `bodyOffset[s]` is the index into ayah 1 at which the verse body
begins; `0` means nothing is embedded. `body(s, 1) = raw(s, 1)[bodyOffset[s]..]`
and every other ayah is a straight passthrough.

**The table is per corpus, not global.** Uthmani and simple-clean are different
orthographies, so their offsets differ and one cannot be reused for the other.
Both are loaded, so there are 228 integers in play, not 114.

This is the whole mechanism. Consequences that matter:

- **Non-destructive by construction.** A slice cannot rewrite a character. There
  is no code path where a mark is reordered, a tatweel dropped, or NFC applied —
  the failure mode `quran-api.md` §3.3 spends a section warning about is not
  merely tested against, it is unrepresentable.
- **Free.** 114 integers computed once at load. No per-ayah allocation, no
  second copy of a 1.36 MB corpus.
- **Digest-compatible.** Digests run over `raw`, so they are unaffected and keep
  their full detective power.

Measured against both current Tanzil sources:

| | Uthmani | simple-clean |
|---|---|---|
| surahs with `bodyOffset > 0` | 112 | 112 |
| surahs with `bodyOffset = 0` | 1, 9 | 1, 9 |
| distinct offsets | **40**, and **41** for surahs 95 and 97 | **23** for all 112 |
| reference basmala | `بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ` | `بسم الله الرحمن الرحيم` |

The 41 is not an anomaly to be smoothed out — see §6. Note it has **no analogue
in simple-clean**, which has a single offset: that corpus carries no diacritics,
so the 95/97 shadda does not exist there and cannot be recovered from it. Any
`opener(s)` text intended for display must come from the Uthmani corpus.

### 4.2 Computing bodyOffset: the skeleton walk

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
then walk the original
counting only base characters to map the cut point back, taking any trailing
marks on the final base letter with it, then skip separating spaces.

**If the skeleton does not match, `bodyOffset` is 0 and the ayah is returned
verbatim.** Never guess at Quranic text. A missed prefix renders a duplicate
header — visible, reportable, harmless. A wrong cut corrupts scripture.

The character class is written with `\u` escapes deliberately: spelled as literal
ranges it silently swallows the Arabic-Indic digits at U+0660–0669, which sit
between two of its ranges.

Two rules make this work across corpora, both from the §3.1 audit:

**Take the reference basmala from the corpus's own `index = 1` row. Never from a
shared constant.** Every source that has a basmala at all has it as global row 1,
so the reference is always available in-band and is always spelled in that
corpus's own orthography. A shared constant is what makes the current prototype
inert on simple-clean: the constant uses `ٱ` U+0671 ALEF WASLA, simple-clean uses
`ا` U+0627, and alef wasla is a letter that the skeleton does not remove. Reading
the reference in-band also removes the constant as a thing that can drift from
the data.

**`bodyOffset[1] = 0` by definition, before any matching runs.** Surah 1's ayah 1
*is* the basmala, so the walk would legitimately match the entire verse and
return an empty body. That is a property of the algorithm, not a bug to patch
downstream — so the guard belongs inside the function that computes the offset,
not in each caller. Equivalently: `bodyOffset[s] > 0` only where
`shape(s) == embedded-prefix` (invariant 7, §9).

## 5. Source profiles

A profile describes how one corpus packages the basmala and is asserted at load.

| shape | meaning | seen in |
|---|---|---|
| `embedded-prefix` | basmala prepended to ayah 1's text | Tanzil, suras 2–114 |
| `first-ayah` | basmala is ayah 1 and is counted | Tanzil 1:1 |
| `chapter-flag` | corpus holds 6,236 numbered ayahs; the opener is a boolean on the *chapter* | quran.com / Quran Foundation API (`bismillah_pre`) |
| `separate-row` | basmala is its own row (aya 0), corpus has 6,348 rows | named by Tanzil as the encoding it declined; not yet observed in a file we hold |
| `none` | no basmala | surah 9 |

```text
profile := {
  id, sha256,
  rowCount,                       // 6236 canonical, or the source's own
  shapes: { [sura]: shape },      // detected, then asserted against the registry
  expected: { first-ayah: 1, embedded-prefix: 112, none: 1 },   // per source
}
```

Two changes from today's behaviour:

- The `1/112/1` assertion becomes **one registry entry**, not a global
  invariant. Tanzil keeps asserting exactly `1/112/1` and still fails closed if
  its own source drifts. A second source asserts its own expected split.
- Row-shape adapters run **before** canonicalization, for shapes that need one.
  A `separate-row` source would be remapped to canonical `(sura, aya)` over
  1..6236 — basmala rows consumed into `opener(s)` rather than dropped, global
  indices recomputed. `chapter-flag` and `embedded-prefix` sources need no
  remapping at all (§5.1). `quran-data.xml` is the arbiter of ayah counts for
  every source; it already validates against the Tanzil DB (114/114 `start`
  offsets, 15 sajdas, 604 pages, 30 juz, 240 hizb quarters, 556 rukus).

`separate-row` is the shape Tanzil names and declines to use — "giving number 0
to Bismillahs". It is retained here as a defensive case, not a known source.

### 5.1 quran.com — verified alignment

quran.com reaches the *same* canonical model by a different route: it stores
6,236 numbered ayahs with the opener as a chapter-level boolean, so `2:1` is
`الٓمٓ` with no basmala in the text. Its `bismillah_pre` is `false` for surahs 1
and 9 and `true` for the other 112.

Critically, **its global index is the same index we already use.** Spot-checked
`verse_index` values from the Quran Foundation API against the Tanzil DB's
`"index"` column:

| verse | quran.com `verse_index` | Tanzil global | |
|---|---:|---:|---|
| 1:1 | 1 | 1 | ✓ |
| 1:7 | 7 | 7 | ✓ |
| 2:1 | 8 | 8 | ✓ |
| 2:255 | 262 | 262 | ✓ |
| 114:6 | 6236 | 6236 | ✓ |

So `verse_index ≡ our global index ≡ quran-data.xml start+1`, and
`(chapter_id, verse_number) ≡ (sura, aya)`. **No re-indexing adapter is needed
for a quran.com source** — only the opener differs, which is exactly what
`opener(s)` already abstracts. That removes the largest piece of speculative
work from §5.

Two notes on adopting their vocabulary wholesale:

- `bismillah_pre` is **less expressive than our existing `Bismillah` enum**. It
  is `false` for both surah 1 and surah 9, collapsing "the basmala is ayah 1"
  and "there is no basmala" into one value — quran.com recovers the difference
  in the UI with a hardcoded `CHAPTERS_WITHOUT_BISMILLAH = ['1','9']` list. Our
  three-value enum already distinguishes them. Map `bismillah_pre` *onto*
  `opener(s)` on import; do not adopt it as the internal model.
- A quran.com-shaped **schema** (a derived `verses` table holding pre-stripped
  text) is not adoptable here regardless of its merits: it materializes modified
  ayah text, which fixed decisions 1 and 2 forbid, drops the corpus outside the
  golden digests, and exceeds Tanzil's verbatim-copy licence. `bodyOffset`
  yields the identical read-time result with nothing materialized.

### 5.2 Do not split on a newline

Guidance circulating for Tanzil imports says to separate the basmala on an
embedded newline (`row.text.split('\n')`). **That does not work on this data.**
Verified on both artifacts:

| | newlines found |
|---|---:|
| `quran-uthmani.sqlite`, all 6,236 rows | 0 |
| `sql/quran-uthmani.sql`, all 6,236 quoted values (literal or `\n`-escaped) | 0 |

The separator in `2:1` is a single U+0020 space. Tanzil's own wording is that
applications should "add a newline … on the fly" — the newline is an output the
consumer *produces*, not a delimiter the file provides. A splitter written that
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
`رَّبِّهِمْ`…). It is how Uthmani script marks *idgham*, assimilation into the
preceding word's final consonant. The basmala case is that rule applied across a
surah boundary:

| preceding surah ends | final ب | next surah's ayah 1 |
|---|---|---|
| 13 → `ٱلْكِتَـٰبِ` | kasra | `بِسْمِ` |
| 14 → `ٱلْأَلْبَـٰبِ` | kasra | `بِسْمِ` |
| 34 → `مُّرِيبٍۭ` | tanwin | `بِسْمِ` |
| **94 → `فَٱرْغَب`** | **no diacritic** | **`بِّسْمِ`** |
| **96 → `وَٱقْتَرِب`** | **no diacritic** | **`بِّسْمِ`** |

Surahs 94 and 96 are the only two in the Quran ending in a *majzoom* (jussive) ب.
Their final ب carries no diacritic at all — verified, U+0628 with nothing
following it — precisely because it merges into the ب of the basmala under the
Arabic idgham rule. The three surahs ending in a *voweled* ب correctly take no
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
*after* the ب. Any "what letter does this surah end with" heuristic has to skip
annotation signs, or it reads 96 as ending in ۩ and misses the idgham.

Requirements that follow:

1. `opener(95)` and `opener(97)` return the **shadda'd** spelling, not a shared
   constant. A single hardcoded `BISMILLAH` string renders the wrong orthography
   for these two.
2. Prefix matching must accept both spellings. The skeleton walk (§4.2) already
   does — shadda is a combining mark and is skeletonized away — which is
   precisely why offsets there come out as 41 rather than 40.
3. A constant-string prefix match would silently skip exactly these two surahs.
   Do not introduce one.

## 7. Effects on search

Search indexes `body` **plus the openers as their own units** — not `body`
alone. The distinction matters, and Tanzil's documentation is what settles it:
the basmala of suras 2–114 is *Quran text*, merely unnumbered.

> While Bismillahs are not individually numbered in suras 2 to 114, they are
> considered as part of the Quran text in Medina Mushaf. […] Tanzil Quran text
> closely follows the same standards […] In particular, it considers Bismillahs
> as part of the Quran text.

So dropping them from the index would be under-reporting real content. The fix
is attribution, not exclusion. Measured on simple-clean, `بسم الله الرحمن الرحيم`
returns **114** rows today:

| | now (`raw`) | `body` only | `body` + openers |
|---|---:|---:|---:|
| surahs 2–114 ayah 1 | 112, as ayah hits | 0 | 112, as **opener** hits |
| genuine 1:1 | 1 | 1 | 1 |
| 27:30 (mid-ayah, in Sulayman's letter) | 1 | 1 | 1 |

All three columns find the same text. The difference is what a hit *claims*:
today the API says "surah 2 ayah 1 matches", which misattributes an unnumbered
opener to a numbered ayah and makes ayah 1 of 112 surahs look like it begins
with words it does not begin with. As opener hits they carry no ayah number, the
way the Mushaf sets them.

27:30 is byte-identical to 1:1 in the Uthmani text and is the only non-first
ayah containing the basmala in simple-clean. It is numbered verse content and
must keep matching as an ordinary ayah hit.

Search hits carry `raw` text in the response even though matching runs over
`body`; only the match *offsets* shift. Any highlight range computed against
`body` must be rebased by `bodyOffset[s]` before it is applied to `raw`.

A hit type therefore needs a discriminator (`ayah` vs `opener`) in both the API
response and the worker's `SearchHit`. This is an additive contract change.

Both search implementations — `quran.worker.ts:152` and the Rust in-memory
corpus — build their index from the same view, or the "same search behavior
online and offline" requirement (`quran-web-delivery.md` §1.4) breaks.

### 7.1 Separate defect found during audit: Uthmani queries never match

This is not caused by the basmala and is not fixed by anything above, but it
lives in the same code path and should be fixed in the same pass.

`normalizeArabic` (`web/src/lib/quran/search/normalize.ts:39`) removes
`\p{Mn}\p{Me}\p{Cf}` and folds `آأإ→ا`, `ى→ي`, `ة→ه`. It does **not** handle two
characters that are pervasive in the Uthmani corpus:

| character | category | why it survives normalization | count in Uthmani | count in simple-clean |
|---|---|---|---:|---:|
| `ٱ` U+0671 ALEF WASLA | `Lo` (letter) | not a combining mark; no fold rule | 13,819 | 0 |
| `ـ` U+0640 TATWEEL | `Lm` (modifier letter) | **not** `Mn`/`Me`/`Cf`, so the class misses it | 6,848 | 0 |

The search corpus is simple-clean, which contains neither, so corpus-side
indexing is unaffected. The **query** side is not:

```text
normalizeArabic(uthmani 1:1)       →  "بسم ٱلله ٱلرحمـن ٱلرحيم"
normalizeArabic(simple-clean 1:1)  →  "بسم الله الرحمن الرحيم"
equal? false
```

The reader displays Uthmani. So a user who copies a phrase out of the reader —
or off any Uthmani mushaf site — and pastes it into search gets **zero results**,
for text that is demonstrably in the corpus. It fails silently and looks like
missing content rather than a normalization gap.

Fix: fold `ٱ → ا` and strip `ـ` in `normalizeArabic`. Both are safe — alef wasla
and bare alef are the same letter for matching purposes, and tatweel is a purely
typographic elongation carrying no phonetic content.

Two constraints on doing it, from the module's own header: the rule set is
mirrored byte-for-byte by the Rust backend, and **any rule change bumps
`SEARCH_VERSION`** (`arabic-search-v1`) and requires regenerated fixtures in
`__fixtures__/queries.json`. So this is a coordinated change across
`normalize.ts`, the Rust implementation, and the fixture suite — not a one-line
edit.

## 8. Where the layer lives

Three runtimes consume the corpus and all three need the same answers:

| Runtime | Entry point | Change |
|---|---|---|
| Rust backend | `rust/.../quran/loader.rs` | compute `bodyOffset` at load; profile registry replaces the inline `1/112/1` assertion; `body`/`opener` accessors on `QuranStore` |
| SSG | `web/src/lib/server/quran-sqlite.ts` | compute at build; `opener` + `bodyOffset` into the CATALOG virtual module |
| Browser worker | `web/src/lib/workers/quran.worker.ts` | compute after deserialize; search indexes `body` |

The shared TypeScript stripper is promoted out of `routes/design/_variants/` into
`$lib/data/`, where the shipping reader and the worker can both reach it. The
design variants then import the real thing instead of keeping a private copy.

Rust and TypeScript necessarily hold two implementations of the skeleton walk.
They are kept honest by a **shared fixture file** — surah number, expected
`bodyOffset`, expected first 20 chars of `body` — checked by both test suites.
Divergence between the two is the most likely way this layer breaks, and the
fixture is what catches it.

## 9. Invariants to assert

At load, per source:

1. Canonical row count is 6,236 after any row-shape adaptation.
2. `(sura, aya)` keys and per-surah ayah counts match `quran-data.xml`.
3. The detected shape split matches the registry entry for that source.
4. Golden digest over `raw` matches the registry literal (unchanged; the digest
   is a hardcoded constant, never recomputed from the source being checked).
5. `body(s, a) == raw(s, a)` for every ayah where `a > 1`, and for surahs 1 and 9.
6. `raw(s, 1).endsWith(body(s, 1))` for all 114 surahs — proves the view only
   ever removes a prefix.
7. `bodyOffset[s] > 0` exactly when `shape(s) == embedded-prefix`.
8. `opener(1).kind == "verse"`, `opener(9).kind == "none"`.
9. `body(s, 1)` is non-empty and has no leading or trailing whitespace, for all
   114 surahs. This is what catches the surah-1-blanked defect (§3.1) directly
   rather than relying on a caller-side guard.
10. `bodyOffset` is computed **per corpus**, and each corpus's reference basmala
    equals its own `raw(1,1)`. A corpus whose stripped count is 0 while its
    profile says `embedded-prefix` is a hard error — that is the exact signature
    of the simple-clean inertness in §3.1, which currently fails silently.
11. The Rust and TypeScript implementations agree on every `bodyOffset` for both
    corpora (§8 fixture).

Invariants 5 and 6 are the ones that make "we never alter text" checkable rather
than a claim, and they hold for any source, not just Tanzil. Invariants 9 and 10
exist because both defects found in the audit were *silent* — one masked by a
caller-side guard, the other a no-op that looked like success.

### 9.1 What the corpus audit cleared

A full sweep of both Arabic corpora, the metadata XML and all 115 translation
packs found **no structural or data defects**. Recorded so this is not re-run:

| check | result |
|---|---|
| row count, contiguous `index` 1..6236, unique `(sura,aya)`, sequential `aya` within each surah | pass, both corpora |
| ayah counts and `start` offsets vs `quran-data.xml` | pass, 114/114 |
| juz 30 · page 604 · ruku 556 · hizb-quarter 240 · manzil 7 · sajda 15 | all present, monotonic, unique, every marker resolves to a real verse |
| empty / untrimmed / double-space / control char / newline / tab / BOM / NBSP | none, both corpora |
| codepoint inventory | 70 distinct (Uthmani), 37 (simple-clean); every one in the Arabic blocks — no stray Latin or punctuation |
| golden corpus digests vs `quran-api.md` §3.3 | **match exactly**, both; joined byte lengths match too |
| NFC canary | NFC-normalizing Uthmani yields `6ee54875c37e4d88…`, exactly as documented — the digest check does detect it |
| file digests vs `web/src/lib/config/site.ts` | match, both |
| 115 translation packs: 6,236 rows, contiguous index | pass, 115/115 |
| translation empty verses | 4, in 3 files — exactly as `docs/translation-empty-verses.md` records |

Uthmani is NFC-unstable in 5,782 of 6,236 rows, matching the documented figure.
That is a property of the source, and the reason the digests exist.

All defects found by the audit are in *our code*, not the data: §3.1 and §7.1.

### 9.2 Validation: the proposed rule executed against all 228 first ayahs

§4.1/§4.2 were implemented exactly as specified — reference basmala from the
corpus's own `raw(1,1)`, `bodyOffset[1] = 0` by definition, skeleton walk mapped
back onto the original — and run over ayah 1 of all 114 surahs in **both**
corpora. **0 failures.**

| | Uthmani | simple-clean |
|---|---|---|
| stripped | 112 | 112 |
| zero-offset | 1, 9 | 1, 9 |
| distinct offsets | 40, 41 | 23 |
| distinct opener texts | **2** — 110 plain, 2 shadda'd (95, 97) | **1** |
| `opener + " " + body == raw`, byte-for-byte | 112/112 | 112/112 |
| separator consumed | exactly one U+0020 | exactly one U+0020 |
| body non-empty and trimmed | 114/114 | 114/114 |
| removed prefix skeleton-equals the basmala | 112/112 | 112/112 |
| corpora disagree on basmala presence | none | none |

Reconstruction is lossless in both corpora: the only character the view ever
consumes beyond the basmala is the single separating space. That is invariant 6
demonstrated rather than asserted.

**Stress cases** — ayah 1 texts that would fool a looser matcher, all correct:

| surah | body after cut | why it is a trap |
|---|---|---|
| 55 | `ٱلرَّحْمَـٰنُ` | ayah 1 *is* a word that occurs inside the basmala; a greedy or last-occurrence matcher eats it |
| 96 | `ٱقْرَأْ بِٱسْمِ رَبِّكَ ٱلَّذِى خَلَقَ` | contains `بِٱسْمِ` — the basmala's own opening letters — *after* the cut |
| 87 | `سَبِّحِ ٱسْمَ رَبِّكَ ٱلْأَعْلَى` | contains `ٱسْمَ` |
| 95, 97 | `وَٱلتِّينِ…` / `إِنَّآ أَنزَلْنَـٰهُ…` | shadda'd basmala, offset 41 not 40 |
| 9 | `بَرَآءَةٌ مِّنَ ٱللَّهِ…` | opens with `بَ` and names Allah, but has no basmala — correctly untouched |
| 1 | `بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ` | ayah 1 is the basmala; offset forced to 0, verse preserved whole |

The 30 muqatta'at surahs are the most sensitive to an off-by-one, since their
entire body is two to five characters (`الٓمٓ`, `الٓمٓصٓ`, `كٓهيعٓصٓ`, `حمٓ`, `قٓ`,
`نٓ`, `طه`, `يسٓ`, `صٓ`, `الٓر`, `الٓمٓر`, `طسٓ`, `طسٓمٓ`). All came through intact —
a cut one position early or late would corrupt them visibly.

## 10. Open items

- **The quran.com source is not in the repo and has not been profiled.** §5.1
  describes the Quran Foundation *API* model — 6,236 numbered ayahs, opener as a
  chapter flag, no basmala in `2:1` — which is inferred from the public API and
  schema, not measured from a corpus. A downloaded export may not match it; QUL
  and mushaf-layout exports are packaged differently from the API model. Profile
  the actual file before writing its registry entry: row count, ayah-number base,
  where the basmala sits per surah, whether `verse_index` matches ours (§5.1),
  and whether its Uthmani text preserves the 95/97 shadda. Until then it has no
  profile and will not load — the intended behaviour under fixed decision 6.

  Note that an earlier concern that this project's Tanzil database itself carried
  the basmala "as ayah 1 in some places" was checked and did not hold; see §1.1.
  Do not carry that assumption into the quran.com profiling — measure it.
- Whether the second source becomes a published artifact or stays a
  build/verification input. This decides whether `/quran/v1/scripts`,
  `contentVersion`, and OPFS storage grow a third file.
- Whether `Bismillah` in the public API gains the new shape variants or keeps its
  current three values. It is serialized in OpenAPI, so widening it is a
  breaking change for clients.

---

## 11. References

- [A Note on Bismillah](https://tanzil.net/docs/a_note_on_bismillah) — Tanzil.
  Normative for §2, §5, §6 and §7: the numbering convention, why suras 2–114 are
  encoded with the basmala on the first ayah's line, the 95/97 shadda and its
  idgham rationale, and the instruction to split at read time rather than in the
  file.
- [Quran Foundation content API](https://api-docs.quran.foundation/docs/content_apis_versioned/4.0.0/get-chapter/)
  — `bismillah_pre` on the chapter resource, and `verse_index` / `verse_key` on
  the verse resource. Source for §5.1. Note the project states its production
  database is private, so the shapes there are inferred from the API surface and
  schema, not from the corpus itself.
- `docs/quran-api.md` §3.3 — verbatim-text guarantee and the golden digests.
- `docs/quran-web-delivery.md` §1.2, §1.4 — files unaltered; identical search
  behaviour online and offline.
- `db/quran/tanzil/quran-data.xml` — ayah counts and navigation markers; the
  arbiter of canonical `(sura, aya)` for every source.
