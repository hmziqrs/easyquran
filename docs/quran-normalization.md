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
- **Row identity** — a source that stores the basmala as its own row has 6,348
  rows, not 6,236. Depending on whether it numbers those rows 0 or 1, ayah
  numbers for suras 2–114 either stay put or shift by one; either way global
  indices and juz/page/ruku boundaries drift against `quran-data.xml`.

Adding a second source today does not produce subtly wrong output. It produces
a **hard boot failure**: `rust/backend/api/src/quran/loader.rs:346` asserts the
basmala split is exactly `1 / 112 / 1` and returns `Invariant` otherwise.

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
| `withoutBasmalaPrefix()` — skeleton-based stripper | `web/src/routes/design/_variants/verses.ts` | **prototype only**, not in the shipping reader |
| Search corpus construction | `web/src/lib/workers/quran.worker.ts:152` | ships, indexes raw text |

The `withoutBasmalaPrefix` prototype already solved the hard part correctly and
its approach is promoted wholesale in §4. The gaps are that it is unreachable
from shipping code, the Rust side has no equivalent, the classification is
hardcoded to one source, and search does not use any of it.

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

### 4.1 bodyOffset — normalization as 114 integers

For each surah, `bodyOffset[s]` is the index into ayah 1 at which the verse body
begins; `0` means nothing is embedded. `body(s, 1) = raw(s, 1)[bodyOffset[s]..]`
and every other ayah is a straight passthrough.

This is the whole mechanism. Consequences that matter:

- **Non-destructive by construction.** A slice cannot rewrite a character. There
  is no code path where a mark is reordered, a tatweel dropped, or NFC applied —
  the failure mode `quran-api.md` §3.3 spends a section warning about is not
  merely tested against, it is unrepresentable.
- **Free.** 114 integers computed once at load. No per-ayah allocation, no
  second copy of a 1.36 MB corpus.
- **Digest-compatible.** Digests run over `raw`, so they are unaffected and keep
  their full detective power.

Measured against the current Tanzil Uthmani source:

| | value |
|---|---|
| surahs with `bodyOffset > 0` | 112 |
| surahs with `bodyOffset = 0` | 1, 9 |
| distinct offsets | 40, and 41 for surahs 95 and 97 |

The 41 is not an anomaly to be smoothed out — see §6.

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

## 5. Source profiles

A profile describes how one corpus packages the basmala and is asserted at load.

| shape | meaning | seen in |
|---|---|---|
| `embedded-prefix` | basmala prepended to ayah 1's text | Tanzil, suras 2–114 |
| `first-ayah` | basmala is ayah 1 and is counted | Tanzil 1:1 |
| `separate-row` | basmala is its own row (often aya 0); ayah numbers may shift | quran.com-style dumps (6,348 rows) |
| `separate-field` | basmala in a sibling column | quran.com API `bismillah_pre` |
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
- Row-shape adapters run **before** canonicalization. A `separate-row` source is
  remapped to canonical `(sura, aya)` over 1..6236 — basmala rows are consumed
  into `opener(s)` rather than dropped, and global indices are recomputed so
  juz/page/ruku ranges from `quran-data.xml` stay valid. `quran-data.xml` is the
  arbiter of ayah counts for every source; it already validates against the
  Tanzil DB (114/114 `start` offsets, 15 sajdas, 604 pages, 30 juz, 240 hizb
  quarters, 556 rukus).

`separate-row` is the shape Tanzil names and declines to use — "giving number 0
to Bismillahs" — so a source in that shape is a legitimate alternative encoding
of the same content, not a broken file. Both encodings are downstream of the
same Medina Mushaf convention, which is why they can be reconciled onto one
canonical view at all.

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

Invariants 5 and 6 are the ones that make "we never alter text" checkable rather
than a claim, and they hold for any source, not just Tanzil.

## 10. Open items

- **The quran.com source is not in the repo.** The `separate-row` and
  `separate-field` shapes in §5 are written from the documented quran.com/QUL
  formats, not from a profiled file. Before that adapter is implemented the
  actual database needs profiling the way Tanzil was — row count, ayah-number
  base, where the basmala sits, and whether its Uthmani text preserves the 95/97
  distinction.
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
- `docs/quran-api.md` §3.3 — verbatim-text guarantee and the golden digests.
- `docs/quran-web-delivery.md` §1.2, §1.4 — files unaltered; identical search
  behaviour online and offline.
- `db/quran/tanzil/quran-data.xml` — ayah counts and navigation markers; the
  arbiter of canonical `(sura, aya)` for every source.
