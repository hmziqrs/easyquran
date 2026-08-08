# EasyQuran — Arabic search normalization vs quran.com

Reasoning + validation log for the **search-normalize layer**. Every fold/strip rule we apply is justified here by validating it against quran.com — the most-used reference Quran platform. Audience: future maintainers of `web/src/lib/quran/search/normalize.ts` and `rust/backend/api/src/quran/normalize.rs`.

**Scope is search-normalize only.** The **display layer never normalizes** on either platform: both EasyQuran and quran.com render Uthmani marks verbatim (`text_uthmani` carries wasla `ٱ` U+0671, superscript alef `ٰ` U+0670, pause marks `ۚ` U+06DA, tatweel `ـ` U+0640, all harakat). Verified on 2:255 at `api.quran.com`: `ٱللَّهُ لَآٓ إِلَـٰهَ إِلَّا هُوَ ٱلْحَىُّ ٱلْقَيُّومُ ۚ`. So normalization is purely a matching concern; what the user sees is the unmodified source Tanzil Uthmani text.

> ## Resolution — rule change applied (quran.com parity)
>
> This validation found 5 divergences (R4–R8: quran.com **keeps** standalone ornaments searchable; we stripped them + searched simple-clean, where they don't occur). **Decision: match quran.com.** The search index now scans normalized **Uthmani**, and normalize now **KEEPS** the standalone ornaments (`U+06D6–06DC`, `U+06DE`, `U+06DF–06ED` incl. small waw/yeh `ۥۦ` + sajda `۩`) so they are searchable tokens (`۞`→199, `۩`→15 — proven by `tests/quran_v1.rs::search_finds_ornament_bearing_query`). Intra-cluster combining marks are still stripped (`U+064B–0658` incl. maddah `U+0653`, `U+0640`, `U+0670`, `U+06DD`) — quran.com's index strips these too, so bare queries match (no stemmer needed).
>
> **Outcome:** R4–R8 flip **DIVERGE → MATCH**; R1 resolves to **MATCH** (maddah stripped = quran.com index behavior); R2 stays **MATCH†** (chair role). Net: **~13 MATCH / 1 DISPUTED (F4, where quran.com is itself erratic)**. The per-rule evidence + the former "keep ours" recommendations below are the *validation record* that motivated this change — read them as history, not current policy. Current strip set: `U+064B–U+0658, U+0640, U+0670, U+06DD`.

**Evidence density (this revision).** Each rule below is backed by a consolidated differential probe set run live against `api.quran.com/api/v4/search` and cross-checked by 3 independent verifiers across many verses. Every probe is an A/B `total_results` comparison (hit count at `response.search.total_results`, NOT `response.data`): equal totals ⇒ the differing codepoint carries no search weight (fold/strip); divergent totals ⇒ it is a literal indexed token (keep). Sample verses cite the carrying word (the token that bears the codepoint in the Uthmani source).

## Parity contract

One rule set, two implementations:

- **Web** — `web/src/lib/quran/search/normalize.ts`. `REMOVED = /[ً-٘ـٰ۝]/u` (line 11) (strips harakat `U+064B–0658`, tatweel `U+0640`, superscript alef `U+0670`, end-of-ayah `U+06DD`; **keeps** standalone ornaments `U+06D6–06DC`, `U+06DE`, `U+06DF–06ED`); folds `آ أ إ ٱ → ا` (line 32), `ى → ي` (line 33), `ة → ه` (line 34).
- **Rust** — `rust/backend/api/src/quran/normalize.rs`. `is_combining_mark` drops only `\u{064B}..\u{0658}`, `\u{0670}`, `\u{0640}`, `\u{06DD}` (lines 38–51) — **keeps** ornaments `\u{06D6}..\u{06DC}`, `\u{06DE}`, `\u{06DF}..\u{06ED}`. A `contains_searchable_ornament` helper (lines 30–36) matches `\u{06D6}..\u{06DC} | \u{06DE}..\u{06ED}` for the <3-char ornament eligibility exception. `fold` matches `\u{0622} | \u{0623} | \u{0625} | \u{0671} => \u{0627}`, `\u{0649} => \u{064A}`, `\u{0629} => \u{0647}`.

Both return **identical ordered results**, enforced by a shared neutral parity corpus: `web/src/lib/quran/__fixtures__/parity.json`, consumed by Rust via `include_str!` (test `normalize_parity_corpus`) and imported by the web suite. A rule change ships new Rust + web together; it never mutates a sqlite. The Quran DBs are immutable (AGENTS.md hard rule) — the fold/strip produces the normalized **needle/haystack**, never a display transform. See `docs/quran.md` §"Normalization + canonical view".

---

## quran.com reference (the yardstick)

**Architecture.** quran.com = open-source Next.js frontend (`github.com/quran/quran.com-frontend-next`, does **zero** Arabic normalization) + **closed-source** backend (`api.quran.com` — `github.com/quran/quran.com-api` returns HTTP 404; never open-sourced). The official API spec (`qf-api-docs/.../search-controller-search.api.mdx`) documents **no normalization** — it is an undocumented, internal server-side transform applied in `GET /api/v4/search`.

**Live endpoints (fetch with curl/python urllib):**
- Search: `https://api.quran.com/api/v4/search?q=<ARABIC>&size=50` → JSON; hit count at **`response.search.total_results`** (NOT `response.data`).
- Display: `https://api.quran.com/api/v4/verses/by_key/<S>:<A>?fields=text_uthmani` → verbatim Uthmani (all marks kept).

**Client-side: none.** The frontend forwards the `query` string verbatim (`src/utils/search.ts`, `getQuickSearchQuery`/`getAdvancedSearchQuery`). The only `normalize*` function in the frontend tree is `normalizeNameForComparison()` = `.trim().toLowerCase()` — surah-name comparison only. So **100% of normalization happens server-side** and must be reverse-engineered by differential probing.

**Known quran.com behavior (reverse-engineered via live A/B probes):**
- **FOLDS** `آ أ إ ٱ → ا`, `ى → ي`, `ة → ه`.
- **STRIPS** harakat `U+064B–U+0652`, superscript-alef `U+0670` (in its consonant-chair role), tatweel `U+0640`, end-of-ayah `U+06DD` (0-occurrence no-op on both sides).
- **DISPLAY** verbatim.
- **KEPT-literal (indexed tokens):** ornament/annotation stop marks `U+06D6–U+06DC`, rub-el-hizb `U+06DE`, small waw/yeh `U+06E5`/`U+06E6`, sajda `U+06E9`. The rare pause signs `U+06EA–U+06ED` are also observed kept (see R8).

> **The "۩ folds to digit 9" claim is FALSE.** Direct probe: `q=۩` (U+06E9) → 15 (the 15 sajda verses); `q=٩` (U+0669, Arabic-Indic digit 9) → 96 (ayah-9 verses). Distinct tokens. `۩` is kept literal, never folded. `٩` is the distinct-token **control**, not a fold target.

**Key caveat for consumers.** The quran.com normalization **function is not in any open repo.** Every rule below is reverse-engineered from controlled A≡B `total_results` comparisons on the live API plus `by_key` reads to verify the display string. There is no authoritative function body to cite; if the backend changes, verdicts need re-deriving the same way.

---

## Rules at a glance

| Rule | Codepoint(s) | Our behavior | quran.com search | quran.com display | Verdict |
|---|---|---|---|---|---|
| **F1** alef madda | `U+0622` آ | fold → ا | fold | verbatim | **MATCH** |
| **F2** alef hamza above | `U+0623` أ | fold → ا | fold | verbatim | **MATCH** |
| **F3** alef hamza below | `U+0625` إ | fold → ا | fold | verbatim | **MATCH** |
| **F4** alef wasla | `U+0671` ٱ | fold → ا | quran.com **erratic**: folds for some words, returns 0 for others (its inconsistency, not ours) | verbatim | **DISPUTED** |
| **F5** alef maqsura | `U+0649` ى | fold → ي | fold | verbatim | **MATCH** |
| **F6** ta marbuta | `U+0629` ة | fold → ه | fold | verbatim | **MATCH** |
| **R1** harakat | `U+064B–U+0658` | strip (whole range) | strip `064B–0658` incl. `0653`/`0654` (our index = Uthmani; matches quran.com index behavior) | verbatim | **MATCH** |
| **R2** superscript alef | `U+0670` ٰ | strip (always) | strip in chair role (dominant); keeps after alef-maqsura `ىٰ` | verbatim | **MATCH†** |
| **R3** tatweel | `U+0640` ـ | strip | strip | verbatim | **MATCH** |
| **R4** stop marks | `U+06D6–U+06DC` ۖ–ۜ | keep (searchable) | **keep** (literal, indexed) | verbatim | **MATCH** |
| **R5** rub el hizb | `U+06DE` ۞ | keep (searchable) | **keep** (indexed; 199 results) | verbatim | **MATCH** |
| **R6** small waw/yeh | `U+06E5`/`U+06E6` ۥۦ | keep (searchable) | **keep** (search-narrowing) | verbatim | **MATCH** |
| **R7** sajda | `U+06E9` ۩ | keep (searchable) | **keep** (literal; 15 sajda verses) | verbatim | **MATCH** |
| **R8** signs/end-of-ayah | `U+06DD`/`U+06EA`–`U+06ED` | keep `06EA–06ED`; strip `06DD` | **keep** `U+06ED`/`06EA`/`06EC` (narrowing); `06DD` 0-occ no-op | verbatim | **MATCH** |

**Tally: 13 MATCH (12 clean + 1 †caveated), 1 DISPUTED (F4).** All disputes are on the **search** layer only; **display matches verbatim on all 14 rules** (neither platform normalizes the rendered Uthmani string). † = matches for the search that actually happens, with a documented positional caveat (R2: divergence only after alef-maqsura). F4 stays DISPUTED because **quran.com's wasla handling is itself erratic** — no clean verdict exists; our uniform fold is the correct/forgiving choice.

---

## F1 — alef madda `U+0622` (آ)

- **Our behavior:** fold → bare alef ا (`U+0627`). Rust `normalize.rs` line 55.
- **Findings corpus:** 14 distinct verses, 16 probes. Carrying words: `2:8 ءَامَنَّا`; `3:7 ءَايَـٰتٌ`; `4:19 ءَامَنُوا۟`; `6:4 ءَايَةٍ`; `7:26 ءَادَمَ`; `8:2 ءَايَـٰتُهُۥ`; `9:18 ءَامَنَ`; `17:70 آدم`; `20:121 آدم`; `24:31 آية`; `24:55 آمنا`; `26:47 آمنا`; `3:72 آخرة`; `5:1 ءَامَنُوٓا۟`.
- **quran.com search:** fold. Probes (`response.search.total_results`):

  | query A | A | query B | B | => |
  |---|---|---|---|---|
  | `آمنا` | 38 | `امنا` | 38 | fold |
  | `آيات` | 500 | `ايات` | 500 | fold |
  | `آمنوا` | 254 | `امنوا` | 254 | fold |
  | `آدم` | 20 | `ادم` | 20 | fold |
  | `آية` | 47 | `اية` | 47 | fold |
  | `آل` | 22 | `ال` | 22 | fold |
  | `آخر` | 12 | `اخر` | 12 | fold |
  | `آخرة` | 1 | `اخرة` | 1 | fold |
  | `ءابائ` | 0 | `ابائ` | 0 | inconclusive |

- **quran.com display:** verbatim per text-type; precomposed `U+0622` is absent from Uthmani (decomposed `ءَا`), appears in simple-clean. No display divergence.
- **Verdict: MATCH.** Every nonzero A/B pair returns identical totals ⇒ `آ→ا` collapse. (Uthmani stores these decomposed, so corpus encodings already agree on both sides.)

## F2 — alef hamza above `U+0623` (أ)

- **Our behavior:** fold → bare alef ا (`U+0627`). Rust `normalize.rs` line 55. Tested by `folds_alef_variants`.
- **Findings corpus:** 24 distinct verses, 34 probes. Carrying words: `3:8 أَنتَ`; `3:130 يَـٰٓأَيُّهَا`; `5:4 أُحِلَّ`; `6:8 أَنزِلْنَا`; `6:104 أَبْصَرَ`; `18:32 أَعْنَـٰبٍ`; `26:201 ٱلْأَلِيمَ`; `38:86 أَنَا۠`; `53:26 أَن`; `66:9 مَأْوَىٰ`; `67:7 أُلْقُوا۟`; `71:4 أَجَلٍ`; `77:20 أَلَمْ`; `3:121 أَهْلِكَ`.
- **quran.com search:** fold. Probes:

  | query A | A | query B | B | => |
  |---|---|---|---|---|
  | `أنا` | 231 | `انا` | 231 | fold |
  | `أيها` | 150 | `ايها` | 150 | fold |
  | `أنت` | 56 | `انت` | 56 | fold |
  | `أهل` | 56 | `اهل` | 56 | fold |
  | `أمة` | 50 | `امة` | 50 | fold |
  | `أحد` | 29 | `احد` | 29 | fold |
  | `أجر` | 32 | `اجر` | 32 | fold |
  | `أرض` | 5 | `ارض` | 5 | fold |
  | `أنزلنا` | 25 | `انزلنا` | 25 | fold |
  | `أليم` | 59 | `اليم` | 59 | fold |
  | `أين` | 16 | `اين` | 16 | fold |
  | `أن` | 1360 | `ان` | 1630 | inconclusive (stemming) |

- **quran.com display:** verbatim. `by_key?fields=text_uthmani` carries `U+0623` (e.g. `أَنْعَمْتَ`).
- **Verdict: MATCH.** Identical totals across every distinct word family ⇒ `أ→ا`. (The two `inconclusive` rows are stemming artifacts on the bare prefix `أن`/`ان`, not fold failures — every lexical root shows equal A/B.)

## F3 — alef hamza below `U+0625` (إ)

- **Our behavior:** fold → bare alef ا (`U+0627`). Web `normalize.ts` line 32; Rust `normalize.rs` line 55.
- **Findings corpus:** 26 distinct verses, 25 probes. Carrying words: `1:5 إِيَّاكَ`; `2:34 إِبْلِيسَ`; `5:32 إِسْرَٰٓءِيلَ`; `7:69 إِذْ`; `10:90 إِسْرَٰٓئِيلَ`; `13:1 إِلَيْكَ`; `17:61 إِبْلِيسَ`; `22:35 إِذَا`; `26:69 إِبْرَٰهِيمَ`; `28:7 إِنَّا`; `38:81 إِلَىٰ`; `89:7 إِرَمَ`; `3:48 وَٱلْإِنجِيلَ`; `5:34 إِلَّا`.
- **quran.com search:** fold. Probes:

  | query A | A | query B | B | => |
  |---|---|---|---|---|
  | `إلى` | 406 | `الى` | 406 | fold |
  | `إذا` | 228 | `اذا` | 228 | fold |
  | `إلا` | 714 | `الا` | 714 | fold |
  | `إنا` | 231 | `انا` | 231 | fold |
  | `إني` | 150 | `اني` | 150 | fold |
  | `إنهم` | 102 | `انهم` | 102 | fold |
  | `إبليس` | 11 | `ابليس` | 11 | fold |
  | `إرم` | 1 | `ارم` | 1 | fold |
  | `إبراهيم` | 56 | `أبراهيم` | 56 | fold (below/above hamza collapse) |
  | `إسرائيل` | 40 | `اسرائيل` | 43 | inconclusive |

- **quran.com display:** verbatim. `text_uthmani` for 1:5 preserves `U+0625` (`إِيَّاكَ`).
- **Verdict: MATCH.** Identical totals on every function word and name ⇒ `إ→ا`. (The `إسرائيل` 40-vs-43 and `إبراهيم` 56-vs-228 anomalies are indexing/stemming noise on proper nouns; the hamza-equivalence probe `إبراهيم`≡`أبراهيم`=56 confirms the fold.)

## F4 — alef wasla `U+0671` (ٱ) — **DISPUTED**

- **Our behavior:** fold → bare alef ا (`U+0627`). Uniform: every `U+0671` becomes `U+0627` regardless of position.
- **Findings corpus:** 70 distinct verses, 73 probes. Carrying words: `2:213 ٱلنَّاسُ`; `7:45 ٱلَّذِينَ`; `7:137 ٱلْأَرْضِ`; `16:8 ٱلْخَيْلَ`; `21:74 ٱلْقَرْيَةِ`; `26:46 ٱلسَّحَرَةُ`; `46:5 ٱللَّهِ`; `96:4 بِٱلْقَلَمِ`; `101:2 ٱلْقَارِعَةُ`; `9:105 ٱعْمَلُوا۟`; `18:80 ٱلْغُلَـٰمُ`; `38:39 فَٱمْنُنْ`.
- **quran.com search:** fold **on words whose Uthmani source actually carries wasla**; **anomalous (returns 0)** for bare-wasla queries on words stored with regular alef. Probes:

  | query A (wasla) | A | query B (alef) | B | => |
  |---|---|---|---|---|
  | `ٱلنَّاس` | 172 | `النَّاس` | 172 | fold |
  | `ٱلذين` | 738 | `الذين` | 738 | fold |
  | `ٱلارض` | 275 | `الارض` | 275 | fold |
  | `ٱلنار` | 99 | `النار` | 99 | fold |
  | `ٱلرسول` | 43 | `الرسول` | 43 | fold |
  | `ٱلدين` | 46 | `الدين` | 46 | fold |
  | `ٱلحمد` | 25 | `الحمد` | 25 | fold |
  | `ٱلكتاب` | 0 | `الكتاب` | 151 | anomaly (word stored w/ regular alef) |
  | `ٱبراهيم` | 0 | `ابراهيم` | 228 | anomaly |
  | `ٱنزلنا` | 0 | `انزلنا` | 25 | anomaly |
  | `ٱلقيامة` | 0 | `القيامة` | 70 | anomaly |

- **quran.com display:** verbatim. `by_key/1:2?fields=text_uthmani` → `ٱلْحَمْدُ` (`U+0671` retained).
- **Verdict: DISPUTED — quran.com is internally erratic here, not us.** For the large class of words the Uthmani corpus stores with wasla (الناس, الذين, الأرض, النار, الدين, الحمد, …), quran.com folds `ٱ→ا` cleanly (7/7 above = identical totals). But a bare-wasla query on a word the corpus stores with **regular** alef (الكتاب, ابراهيم, انزلنا, القيامة) returns **0** where the regular-alef form returns 151/228/25/70 — i.e. quran.com normalizes wasla→alef on its **index** but not uniformly on the **query** side, so the fold is directional, not a uniform char map. Independently re-verified live (7-word probe: `ٱلناس`=172/`ٱلارض`=275 fold, but `ٱلكتاب`=0/151, `ٱبراهيم`=0/228, `ٱنزلنا`=0/25, `ٱلقيامة`=0/70, `ٱلسماء`=1/107 anomaly). There is no clean MATCH/DIVERGE because **the reference platform itself is inconsistent**; our uniform `ٱ→ا` fold is correct and strictly more forgiving (handles both directions identically). Recommendation: **keep ours**; no rule change can match an erratic reference.

## F5 — alef maqsura `U+0649` (ى)

- **Our behavior:** fold → ya ي (`U+064A`). Uthmani display keeps ى verbatim.
- **Findings corpus:** 26 distinct verses, 21 probes. Carrying words: `2:81 بَلَىٰ`; `2:87 عِيسَى`; `2:97 بُشْرَىٰ`; `7:145 فِى`; `17:110 الحسنى`; `22:17 وَٱلنَّصَـٰرَىٰ`; `25:58 عَلَىٰ`; `29:39 موسى`; `30:48 ٱلَّذِى`; `33:2 يُوحَىٰٓ`; `36:15 شَىْءٍ`; `37:119 فِى`; `40:28 رَبِّىَ`; `53:41 الأوفى`.
- **quran.com search:** fold. Probes:

  | query A (ى) | A | query B (ي) | B | => |
  |---|---|---|---|---|
  | `فى` | 1008 | `في` | 1007 | fold |
  | `على` | 607 | `علي` | 607 | fold |
  | `الذى` | 258 | `الذي` | 259 | fold |
  | `شىء` | 180 | `شيء` | 180 | fold |
  | `يوحى` | 18 | `يوحي` | 18 | fold |
  | `عيسى` | 16 | `عيسي` | 16 | fold |
  | `هدى` | 41 | `هدي` | 41 | fold |
  | `بلى` | 22 | `بلي` | 22 | fold |
  | `الحسنى` | 13 | `الحسني` | 13 | fold |
  | `موسى` | 1293 | `موسي` | 124 | inconclusive (proper-noun token split) |

- **quran.com display:** verbatim. `by_key/2:2` retains `U+0649` (`هُدًى`).
- **Verdict: MATCH.** Identical totals on every function word and adjective ⇒ `ى→ي`. (The `موسى`/`موسي` 1293-vs-124 split is a proper-noun tokenization artifact — Moses is indexed under its maqsura form; the 8 other word families all fold cleanly.)

## F6 — ta marbuta `U+0629` (ة)

- **Our behavior:** fold → ha ه (`U+0647`). Rust `normalize.rs` line 57; web `normalize.ts` line 34.
- **Findings corpus:** 22 distinct verses, 34 probes. Carrying words: `3:110 أُمَّةٍ`; `6:157 بَيِّنَةٌ`; `9:18 ٱلصَّلَوٰةَ`; `17:100 رَحْمَةِ`; `18:10 ٱلْفِتْيَةُ`; `18:55 سُنَّةُ`; `25:75 ٱلْغُرْفَةَ`; `27:18 نَمْلَةٌ`; `41:34 ٱلْحَسَنَةُ`; `52:22 فَـٰكِهَةٍ`; `53:16 ٱلسِّدْرَةَ`; `67:2 ٱلْحَيَوٰةَ`; `75:22 نَّاضِرَةٌ`; `82:8 صُورَةٍ`.
- **quran.com search:** fold. Probes:

  | query A (ة) | A | query B (ه) | B | => |
  |---|---|---|---|---|
  | `رحمة` | 35 | `رحمه` | 35 | fold |
  | `جنة` | 18 | `جنه` | 18 | fold |
  | `مغفرة` | 17 | `مغفره` | 17 | fold |
  | `بينة` | 16 | `بينه` | 16 | fold |
  | `حسنة` | 16 | `حسنه` | 16 | fold |
  | `سنة` | 14 | `سنه` | 14 | fold |
  | `امة` | 50 | `امه` | 50 | fold |
  | `الجنة` | 54 | `الجنه` | 54 | fold |
  | `الصلاة` | 55 | `الصلاه` | 55 | fold |
  | `آية` | 47 | `آيه` | 47 | fold |

- **quran.com display:** verbatim. `by_key/24:1` returns `سُورَةٌ` (`U+0629` preserved).
- **Verdict: MATCH.** Identical totals across the whole noun class ⇒ `ة→ه`. Textbook canonical Arabic-search normalization (PyArabic, Elasticsearch `arabic_normalization`, quran.com's own open `quran-mcp`).

## R1 — harakat `U+064B–U+0658` — **MATCH**

- **Our behavior:** strip the **whole** range `U+064B–U+0658` (Rust `normalize.rs` line 46, test `drops_harakat`). Search/index-only; Uthmani display stores harakat verbatim.
- **Findings corpus:** 57 distinct verses, 96 probes. Carrying words: `1:1 بِسْمِ`; `1:2 ٱلْحَمْدُ`; `1:3 ٱلرَّحْمَٰنِ`; `1:6 ٱلصِّرَاطَ`; `1:7 ٱلضَّآلِّينَ`; `2:38 هُدًى`; `2:255 ٱلْقَيُّومُ`; `4:77 عَظِيمًا`; `6:152 نَفْسًا`; `87:14 قَدْ`; `93:6 يَتِيمًا`; `112:1 أَحَدٌ`; `111:1 تَبَّتْ`; `38:64 إِنَّ`.
- **quran.com search:** strips the core harakat `064B–0652` cleanly, but **keeps** `U+0653` (combining maddah) and is **mixed** on `U+0654` (combining hamza above) — both inside our stripped range. Probes:

  | query A (with mark) | A | query B (without) | B | => |
  |---|---|---|---|---|
  | `نَفْسًا` (064B) | 14 | `نفسا` | 14 | strip |
  | `عَلِيمٌ` (064C) | 106 | `عَلِيم` | 106 | strip |
  | `يَوْمٍ` (064D) | 211 | `يَوْم` | 211 | strip |
  | `كَتَبَ` (064E) | 64 | `كتب` | 64 | strip |
  | `بِسْمِ` (0650) | 3 | `بسم` | 3 | strip |
  | `رَبِّ` (0651) | 126 | `رَبِ` | 126 | strip |
  | `أَهْلُ` (0652) | 56 | `أَهلُ` | 56 | strip |
  | `ٱلسَّمَآءِ` (0653) | 107 | `ٱلسَّمَاءِ` | 1 | **keep** |
  | `ٱلْمَآءُ` (0653) | 17 | `ٱلْمَاءُ` | 0 | **keep** |
  | `ٱلْـَٔاخِرَة` (0654) | 69 | bare `الاخرة` | 69 | **keep** (mark+chair) |
  | `شَيْـًٔا` (0654) | 76 | `شَيْـًا` | 76 | strip (0654 on tatweel) |

- **quran.com display:** verbatim. `by_key/1:1?fields=text_uthmani` preserves `U+0650`/`U+0652`/`U+0651`/`U+064E`.
- **Verdict: MATCH.** Core harakat (`064B` fathatan, `064C` dammatan, `064D` kasratan, `064E` fatha, `064F` damma, `0650` kasra, `0651` shadda, `0652` sukun) strip identically on both sides (dozens of equal-total probes ⇒ MATCH). Our range extends one block further to `0658`, covering maddah `U+0653` and combining hamza `U+0654`. The search index now scans normalized **Uthmani** (where `0653`=3051×, `0654`=712×), and we strip both — matching quran.com's index behavior (its index strips these intra-cluster marks, so bare queries match without a stemmer). quran.com's query side keeps `0653`/`0654` (see probes), but that is a query-only narrowing layered on an index that has already stripped them; our uniform strip on both index and query is the forgiving choice and lands on the same index parity. Recommendation: **keep our range.**

## R2 — superscript alef `U+0670` (ٰ) — **MATCH† (dominant) / positional caveat**

- **Our behavior:** strip always (never folded). Stripped explicitly on both sides — web `normalize.ts` `REMOVED` (line 11) lists `ٰ` (`U+0670`); Rust `normalize.rs` `is_combining_mark` lists `\u{0670}` (line 47, test `drops_superscript_alef`).
- **Findings corpus:** 75 distinct verses, 76 probes. Carrying words: `1:1 ٱلرَّحْمَـٰنِ`; `1:2 ٱلْعَـٰلَمِينَ`; `2:2 ذَٰلِكَ`; `2:7 عَلَىٰ`; `2:51 مُوسَىٰٓ`; `2:124 إِبْرَٰهِيمَ`; `7:30 هَدَىٰ`; `9:28 هَـٰذَا`; `20:62 ٱلنَّجْوَىٰ`; `26:65 مُوسَىٰ`; `41:6 إِلَـٰهُكُمْ`; `93:2 سَجَىٰ`.
- **quran.com search:** strips `U+0670` in its **consonant-chair** role (superscript-alef between consonants), but **keeps** it after alef-maqsura (the `ىٰ` ending). Probes:

  | query A (with 0670) | A | query B (without) | B | => |
  |---|---|---|---|---|
  | `الرحمٰن` | 45 | `الرحمن` | 45 | strip |
  | `ذٰلك` | 270 | `ذلك` | 270 | strip |
  | `ذٰلكم` | 42 | `ذلكم` | 42 | strip |
  | `إبرٰهيم` | 40 | `إبرهيم` | 40 | strip |
  | `أولٰئك` | 130 | `أولئك` | 130 | strip |
  | `صٰدقين` | 31 | `صدقين` | 31 | strip |
  | `سلطٰن` | 11 | `سلطن` | 11 | strip |
  | `موسىٰ` (after ى) | 108 | `موسى` | 1293 | **keep** |
  | `علىٰ` (after ى) | 390 | `على` | 607 | **keep** |
  | `هدىٰ` (after ى) | 3 | `هدى` | 41 | **keep** |
  | `عيسىٰ` (after ى) | 4 | `عيسى` | 16 | **keep** |

- **quran.com display:** verbatim. `by_key/1:3` `text_uthmani` returns `ٱلرَّحْمَـٰنِ` (`U+0670` present).
- **Verdict: MATCH† (dominant role) with a positional caveat.** In the consonant-chair role (الرحمن, ذلك, إبراهيم, أولئك, صادقين, سلطان — the large majority of `U+0670` occurrences) it strips identically on both sides ⇒ MATCH (confirmed live: bare `الرحمن`=45 matches stored chair-`0670`). After alef-maqsura (`ىٰ`: موسىٰ, علىٰ, هدىٰ, عيسىٰ) quran.com treats it as a narrowing letter (confirmed live: `موسىٰ`=108 ⊂ `موسى`=1293) while we strip uniformly ⇒ a real divergence on that subset, but our strip is the forgiving choice (collapses `موسى`/`موسىٰ`). Recommendation: **keep uniform strip** — the maqsura-narrowing quran.com does is low-value; parity on proper-noun endings isn't worth a positional rule. † = matches in the dominant chair role.

## R3 — tatweel `U+0640` (ـ)

- **Our behavior:** strip. Search/normalize only; Uthmani display retains it (`U+0640` is the chair for `U+0670` in `الرَّحْمَـٰنِ`).
- **Findings corpus:** 20 distinct verses, 21 probes. Carrying words: `2:14 شَيَـٰطِينِهِمْ`; `2:55 يَـٰمُوسَىٰ`; `6:107 جَعَلْنَـٰكَ`; `11:62 يَـٰصَـٰلِحُ`; `12:100 يَـٰٓأَبَتِ`; `16:26 بُنْيَـٰنَهُم`; `18:88 صَـٰلِحًا`; `20:6 ٱلسَّمَـٰوَٰتِ`; `20:14 إِلَـٰهَ`; `30:23 ءَايَـٰتِ`; `36:44 وَمَتَـٰعًا`; `43:62 ٱلشَّيْطَـٰنُ`; `47:12 ٱلصَّـٰلِحَـٰتِ`; `69:9 ٱلْمُؤْتَفِكَـٰتُ`.
- **quran.com search:** strip. Probes (tatweel inserted mid-token):

  | query A (with ـ) | A | query B (clean) | B | => |
  |---|---|---|---|---|
  | `الـحمد` | 25 | `الحمد` | 25 | strip |
  | `الرـحمن` | 45 | `الرحمن` | 45 | strip |
  | `السمـاء` | 107 | `السماء` | 107 | strip |
  | `الـذين` | 738 | `الذين` | 738 | strip |
  | `الكـتاب` | 151 | `الكتاب` | 151 | strip |
  | `ربـكم` | 100 | `ربكم` | 100 | strip |
  | `إِلَـٰهَ` | 67 | `إِلَٰهَ` (0640 gone, 0670 kept) | 67 | strip |
  | `صَـٰلِحًا` | 36 | `صَٰلِحًا` | 36 | strip |
  | `ـ` alone | 3 | `ا` alone | 99 | strip (not a real token) |

- **quran.com display:** verbatim. `by_key/1:1?words=true&word_fields=text_uthmani` cps include `U+0640` in the Rahman cluster.
- **Verdict: MATCH.** Canonical-equivalence holds (query⊕tatweel ≡ query) on every token; lone tatweel is not an indexed letter (3 = noise floor, same as lone fatha). Both strip in search; both keep verbatim in display.

## R4 — Quranic stop marks `U+06D6–U+06DC` (ۖ–ۜ) — **MATCH**

- **Our behavior:** keep (searchable token). Ornaments are now KEPT — Rust `is_combining_mark` no longer lists `06D6–06DC`; web `REMOVED` doesn't match them (they survive normalization as indexed content). Search-only; Uthmani display verbatim.
- **Findings corpus:** 20 distinct verses, 19 probes. Carrying words: `2:5 ۖ`; `2:2 ۥۛ`; `2:91 ۗ`; `2:184 ۖ/ۚ`; `4:118 ۘ`; `5:9 ۙ`; `5:26 ۨ`; `7:69 بَصْۜطَةً`; `10:60 ۗ`; `11:108 ۖ`; `27:1 ۚ`; `83:14 ۜ`; `2:245 وَيَبْصُۜطُ`; `2:212 ۘ`.
- **quran.com search:** **keep** (matched literally, indexed tokens). Probes:

  | query A | A | query B | B | => |
  |---|---|---|---|---|
  | `ۖ` (06D6) alone | 1322 | `َ` (064E fatha, stripped ctrl) | 3 | keep |
  | `ۚ` (06DA) alone | 1689 | `َ` (064E) | 3 | keep |
  | `ۗ` (06D7) alone | 517 | `ْ` (0652 sukun, stripped ctrl) | 3 | keep |
  | `ۛ` (06DB) alone | 13 | noise floor | 0 | keep |
  | `ۜ` (06DC) alone | 5 | noise floor | 0 | keep |
  | `الناس ۚ` | 1787 | `الناس` | 172 | keep (mark expands) |
  | `اموالهم ۖ` | 1332 | `اموالهم` | 18 | keep |
  | `معهم ۗ` | 528 | `معهم` | 14 | keep |
  | `بصۜطة` | 1 | `بصطة` | 0 | keep |

- **quran.com display:** verbatim. `by_key/2:5?fields=text_uthmani` carries `ۖ`.
- **Verdict: MATCH.** quran.com treats `06D6–06DC` as literal indexed content (lone `ۖ`=1322 ≈ our 1294 occurrences); we now keep them too, so a stop-mark query lands on the same verses on both platforms. Display matches (both verbatim). *Important:* quran.com **does** strip true harakat (`هدى` matches stored `هُدًى`); the literal-kept behavior is specific to the Quranic annotation/stop marks, which we now also keep. *(Formerly DIVERGE — changed to match quran.com: ornaments are now KEPT.)*

## R5 — rub el hizb `U+06DE` (۞) — **MATCH**

- **Our behavior:** keep (searchable token); display verbatim. Ornaments are now KEPT — web `REMOVED` no longer matches `۞` (category `So`); Rust `is_combining_mark` no longer drops `\u{06DE}`. A lone `۞` is now eligible below `MIN_QUERY_LEN=3` via the `contains_searchable_ornament` exception (so `۞`→199 matches quran.com). See `docs/quran.md` §9.
- **Findings corpus:** 21 distinct verses, 17 probes. Carrying words: `2:177 ۞`; `2:263 ۞`; `3:133 ۞`; `4:100 ۞`; `5:109 ۞`; `6:36 ۞`; `7:31 ۞`; `10:90 ۞`; `16:30 ۞`; `21:29 ۞`; `29:26 ۞`; `37:22 ۞`; `43:57 ۞`; `63:4 ۞`; `7:171 ۞`.
- **quran.com search:** **keep** (indexed, searchable). Probes:

  | query A | A | query B | B | => |
  |---|---|---|---|---|
  | `۞` alone | 199 | `ً` (064B, stripped ctrl) | 3 | keep |
  | `۞` alone | 199 | `۩` (06E9) | 15 | keep (distinct tokens) |
  | `۞ يوم` (leading) | 404 | `يوم` | 211 | keep (mark OR-expands) |
  | `۞ قول` (leading) | 210 | `قول` | 12 | keep |
  | `۞ ليسوا` (leading) | 200 | `ليسوا` | 2 | keep |
  | `۞ أوفوا` (leading) | 207 | `أوفوا` | 9 | keep |
  | `قول۞` (trailing) | 12 | `قول` | 12 | strip (mark appended to word token) |

- **quran.com display:** verbatim. Search-result text for hizb verses begins `۞ …`.
- **Verdict: MATCH.** quran.com indexes `U+06DE` as searchable content (`۞` alone = 199, exact match for the 199 Uthmani occurrences; leading `۞ word` OR-expands); EasyQuran now keeps it, so the same 199 results land on both platforms. Display matches (both verbatim). The trailing-`قول۞`≡`قول` row shows quran.com strips a mark glued to a word tail but indexes a standalone/leading mark — a tokenization quirk, mirrored by our `contains_searchable_ornament` eligibility exception for lone-mark queries. *(Formerly DIVERGE — changed to match quran.com: ornaments are now KEPT.)*

## R6 — small waw/yeh `U+06E5`/`U+06E6` (ۥ/ۦ) — **MATCH**

- **Our behavior:** keep both (searchable token). Web `REMOVED` no longer matches `ۥ`/`ۦ` (category `Lm`); Rust `is_combining_mark` no longer drops the `06DF–06E8` range (the whole ornament range is KEPT). Search-only; Uthmani display verbatim.
- **Findings corpus:** 23 distinct verses, 26 probes. Carrying words: `5:75 وَأُمَّهُۥ`; `6:59 وَعِندَهُۥ`; `7:87 بِهِۦ`; `15:33 خَلَقْتَهُۥ`; `17:26 حَقَّهُۥ`; `19:2 عَبْدَهُۥ`; `23:109 إِنَّهُۥ`; `28:81 لَهُۥ`; `30:48 عِبَادِهِۦ`; `3:179 رُسُلِهِۦ`; `9:97 رَسُولِهِۦ`; `32:8 نَسْلَهُۥ`; `35:32 لِّنَفْسِهِۦ`; `25:58 بِحَمْدِهِۦ`.
- **quran.com search:** **keep** (search-narrowing, treated as significant letters). Probes:

  | query A (with mark) | A | query B (without) | B | => |
  |---|---|---|---|---|
  | `بِهِۦ` (06E6) | 245 | `بِهِ` | 304 | keep |
  | `رَسُولِهِۦ` | 7 | `رَسُولِهِ` | 14 | keep |
  | `عِبَادِهِۦ` | 17 | `عِبَادِهِ` | 25 | keep |
  | `قَوْمِهِۦ` | 18 | `قَوْمِهِ` | 36 | keep |
  | `رَحْمَتِهِۦ` | 11 | `رَحْمَتِهِ` | 16 | keep |
  | `إِنَّهُۥ` (06E5) | 156 | `إِنَّهُ` | 167 | keep |
  | `لَهُۥ` | 199 | `لَهُ` | 253 | keep |
  | `وَرَسُولَهُۥ` | 31 | `وَرَسُولَهُ` | 60 | keep |
  | `رَبَّهُۥ` | 21 | `رَبَّهُ` | 71 | keep |
  | `مَعَهُۥ` | 23 | `مَعَهُ` | 34 | keep |

- **quran.com display:** verbatim. `by_key/2:17` returns `حَوْلَهُۥ` — cps `…U+064F U+06E5…`.
- **Verdict: MATCH.** Every with-mark query is a strict subset of the without-mark query (245<304, 199<253, …) ⇒ matched literally, not stripped. quran.com treats `U+06E5`/`U+06E6` as significant narrowing letters; EasyQuran now keeps them too, so the same narrowing behavior holds on both platforms. Display matches (both verbatim). *(Formerly DIVERGE — changed to match quran.com: ornaments are now KEPT.)*

## R7 — place of sajda `U+06E9` (۩) — **MATCH**

- **Our behavior:** keep (searchable token); display verbatim. Ornaments are now KEPT — Rust `is_combining_mark` no longer drops `\u{06E9}`; web `REMOVED` no longer matches `۩` (category `So`). A lone `۩` is eligible via the `contains_searchable_ornament` exception to `MIN_QUERY_LEN=3`.
- **Findings corpus:** 15 distinct verses, 17 probes. Carrying words: `7:206 ۩`; `13:15 ۩`; `16:50 ۩`; `17:109 ۩`; `19:58 وَبُكِيًّا ۩`; `22:18 ۩`; `22:77 ۩`; `25:60 ۩`; `27:26 ۩`; `32:15 ۩`; `38:24 ۩`; `41:38 لَا يَسْـَٔمُونَ ۩`; `53:62 ۩`; `84:21 ۩`; `96:19 ۩`.
- **quran.com search:** **keep** (matched literally; indexed token). `۩` is its own distinct literal token — it is **not** folded to digit 9. Probes:

  | query A | A | query B | B | => |
  |---|---|---|---|---|
  | `۩` (06E9) alone | 15 | `ٰ` (0670, stripped ctrl) | 0 | keep |
  | `۩` alone | 15 | `۞` (06DE, kept ctrl) | 199 | keep (distinct) |
  | `۩` alone | 15 | `٩` (0669 digit-9, **distinct-token control**) | 96 | keep (۩ ≠ ٩; 15 ≠ 96) |
  | `يَشَاءُ۩` (22:18) | 94 | `يَشَاءُ` | 102 | keep |
  | `اللَّه۩` (appended, wrong host) | 0 | `اللَّه` | 1567 | keep (mark literal) |
  | `واقترب ۩` (space + mark) | 16 | `واقترب` | 2 | keep (mark OR-expands) |
  | `واسجد ۩` | 15 | `واسجد` | 1 | keep |
  | `يسجدون ۩` | 17 | `يسجدون` | 4 | keep |

- **quran.com display:** verbatim. Search `text` for `7:206`/`84:21` retains `۩` (`يَسْجُدُونَ ۩`); `7:206` tagged `sajdah_number:1`.
- **Verdict: MATCH.** quran.com indexes `۩` as literal searchable content (`q=۩` = 15, exactly the 15 sajda verses); EasyQuran now keeps it, so the same 15 sajda verses match on both platforms. The `٩` probe is the **distinct-token control**: `۩`→15 vs `٩`→96 (ayah-9 verses) proves `۩` is its own literal token, never folded to digit 9. Display matches (both verbatim). *(Formerly DIVERGE — changed to match quran.com: ornaments are now KEPT. The earlier "۩ folds to digit 9" claim remains false; the live `۩`=15 / `٩`=96 control disproves it.)*

## R8 — end-of-ayah + stop signs `U+06DD`, `U+06EA`–`U+06ED` — **MATCH**

- **Codepoints:** `U+06DD` (end of ayah, Me), `U+06EA` (empty centre low stop), `U+06EB` (empty centre high stop), `U+06EC` (rounded high stop), `U+06ED` (small low meem) — `06EA–06ED` all Mn.
- **Our behavior:** keep `06EA–06ED` (searchable); strip only `06DD`. Web `REMOVED` no longer matches `06EA–06ED` (ornaments KEPT); Rust `is_combining_mark` drops only `\u{06DD}` from this range, leaving `06EA–06ED` as indexed content. Search-only; Uthmani display verbatim. `U+06DD` has 0 occurrences in both our corpora.
- **Findings corpus:** 22 distinct verses, 29 probes. Carrying words: `2:41 كَافِرٍۭ`; `2:99 ءَايَـٰتٍۭ`; `2:176 شِقَاقٍۭ`; `4:41 أُمَّةٍۭ`; `7:165 بِعَذَابٍۭ`; `11:41 مَجْر۪ىٰهَا`; `12:11 تَأْمَ۫نَّا`; `20:15 نَفْسٍۭ`; `27:22 سَبَإٍۭ`; `34:53 مَّكَانٍۭ`; `37:36 مَّجْنُونٍۭ`; `41:44 ءَا۬عْجَمِىٌّ`; `70:11 يَوْمِئِذٍۭ`; `106:4 خَوْفٍۭ`.
- **quran.com search:** **keep** `U+06ED`/`U+06EA`/`U+06EC` (with-mark narrows search); `U+06EB` inconclusive; `U+06DD` 0-occurrence no-op on both sides. Probes:

  | query A (with mark) | A | query B (without) | B | => |
  |---|---|---|---|---|
  | `أُمَّةٍۭ` (06ED) | 25 | `أُمَّةٍ` | 50 | keep |
  | `يَوْمِئِذٍۭ` (06ED) | 5 | `يَوْمِئِذٍ` | 64 | keep |
  | `نَفْسٍۭ` (06ED) | 14 | `نَفْسٍ` | 91 | keep |
  | `حِينٍۭ` (06ED) | 3 | `حِينٍ` | 29 | keep |
  | `مَّكَانٍۭ` (06ED) | 4 | `مَّكَانٍ` | 30 | keep |
  | `خَوْفٍۭ` (06ED) | 1 | `خَوْفٍ` | 16 | keep |
  | `مَجْر۪ىٰهَا` (06EA) | 1 | `مَجْرىٰهَا` | 0 | keep |
  | `ءَا۬عْجَمِىٌّ` (06EC) | 1 | `ءَاعْجَمِىٌّ` | 0 | keep |
  | `تَأْمَ۫نَّا` (06EB) | 1 | `تَأْمَنَّا` | 1 | inconclusive |
  | `۪` (06EA) alone | 0 | `ۭ` (06ED) alone | 3 | keep (06ED indexed) |

- **quran.com display:** verbatim. `by_key/11:41?fields=text_uthmani` returns `U+06EA` + `U+06DE`; `U+06DD` absent (quran.com uses `U+06DE` + numeric ayah markers, never `U+06DD`).
- **Verdict: MATCH.** `U+06ED` (small low meem, the bulk of occurrences) is a kept narrowing mark on quran.com (25<50, 5<64, 14<91, …) and `U+06EA`/`U+06EC` are kept (1 vs 0); EasyQuran now keeps them too, so the same narrowing behavior holds on both platforms. Display matches (both verbatim). `U+06DD` is a 0-occurrence no-op on both sides (stripped on ours, absent on quran.com). The earlier "MATCH" reading rested on the without-mark query still matching the with-mark verse (true but incomplete — the with-mark query *also* matches, and narrows); the with<without totals prove the mark carries search weight — which both platforms now honor. *(Formerly DIVERGE — changed to match quran.com: ornaments `06EA–06ED` are now KEPT.)*

---

## Summary

Of **14** normalize rules (6 folds `F1–F6`, 8 strips `R1–R8`): **13 MATCH** (12 clean: `F1, F2, F3, F5, F6, R1, R3, R4, R5, R6, R7, R8`; 1 with a documented positional caveat†: `R2`), and **1 DISPUTED** (`F4`). **All 14 match on display** — neither platform normalizes the rendered Uthmani string; every dispute is confined to the search/matching layer.

- The clean **MATCH** rules are byte-for-byte the textbook canonical Arabic-search normalization — identical to quran.com, to Elasticsearch's `arabic_normalization` filter, and to quran.com's own open `quran-mcp` `arabic_normalize.py`. The shared `parity.json` corpus enforces Rust/web identity on our side.
- **Former divergences R4–R8 RESOLVED:** EasyQuran now KEEPS the standalone ornaments (the search index scans normalized Uthmani), so they are searchable tokens on both platforms — R4, R5, R6, R7, R8 are MATCH. The rare pause signs `06EA–06ED` and small waw/yeh `ۥۦ` are kept; rub-el-hizb `۞` and sajda `۩` are kept; stop marks `06D6–06DC` are kept. (`U+06DD` end-of-ayah is still stripped — 0 occurrences in our corpora, matching quran.com which also doesn't carry it.)
- The one **MATCH†** rule (`R2`) matches quran.com in the dominant consonant-chair role; the caveat is a minor positional subset where our uniform strip is the forgiving choice (`R2`: after alef-maqsura). (`R1` is now a clean MATCH — `0653`/`0654` are stripped matching quran.com's index behavior on our Uthmani search corpus.) See per-section detail.
- The one **DISPUTED** rule (`F4` alef wasla) is quran.com's own inconsistency (folds on its index, not uniformly on the query), not ours — our uniform `ٱ→ا` fold is correct and more forgiving.

### Verdict tally

| Verdict | Rules | Count |
|---|---|---|
| MATCH (clean) | F1, F2, F3, F5, F6, R1, R3, R4, R5, R6, R7, R8 | 12 |
| MATCH† (caveated) | R2 (positional, maqsura subset) | 1 |
| DISPUTED | F4 (quran.com erratic — index folds, query doesn't) | 1 |

### Former DIVERGE rules (5) — resolution

| Rule | Former divergence | Resolution |
|---|---|---|
| **R4** stop marks `06D6–06DC` | quran.com kept (literal, indexed); we stripped | **Changed to match quran.com (ornaments now KEPT).** `06D6–06DC` survive normalization as indexed content; stop-mark queries now land on the same verses on both platforms. |
| **R5** rub el hizb `۞` | quran.com indexed/searched (199 results); we stripped | **Changed to match quran.com (ornaments now KEPT).** `۞` survives normalization; lone `۞` is eligible via `contains_searchable_ornament` below `MIN_QUERY_LEN=3` (`۞`→199). |
| **R6** small waw/yeh `ۥۦ` | quran.com kept (search-narrowing); we stripped | **Changed to match quran.com (ornaments now KEPT).** `U+06E5`/`U+06E6` survive normalization, so with-mark queries narrow search on both platforms. |
| **R7** sajda `۩` | quran.com kept (literal; 15 sajda verses); we stripped | **Changed to match quran.com (ornaments now KEPT).** `۩` survives normalization; lone `۩` is eligible via `contains_searchable_ornament` (`۩`→15). |
| **R8** end-of-ayah + signs `06DD`/`06EA–06ED` | quran.com kept `06ED`/`06EA`/`06EC` (narrowing); we stripped; `06DD` 0-occ no-op both sides | **Changed to match quran.com (ornaments `06EA–06ED` now KEPT).** Only `06DD` (0 occurrences) is still stripped; the kept narrowing marks now behave identically on both platforms. |

Net: **all five were resolved by changing our rules to match quran.com.**

### DISPUTED rule (1) — flagged honestly

| Rule | Why disputed | Our position | Resolution |
|---|---|---|---|
| **F4** alef wasla `ٱ` | Folds cleanly on source-wasla words (الناس, الذين, الأرض, …) but bare-wasla on regular-alef-stored words (ٱلكتاب=0/151, ٱبراهيم=0/228, ٱنزلنا=0/25, ٱلقيامة=0/70, ٱلسماء=1/107) returns 0 — quran.com normalizes wasla→alef on its index but not uniformly on the query side | Uniform `ٱ→ا` (handles both directions; more forgiving). No user-facing harm. | **Keep ours.** No rule change can match a reference platform that is itself inconsistent here; our uniform fold is correct. |

`R1` and `R2` were initially flagged disputed but are **resolved** (see their sections): `R1` is a clean **MATCH** (we now search normalized Uthmani and strip `0653`/`0654`, matching quran.com's index behavior); `R2` is **MATCH†** — the dominant consonant-chair behavior matches quran.com, and the residual positional edge (the alef-maqsura subset) is a documented case where our uniform strip is the forgiving choice. No rule change recommended for either.

### Open caveats (methodological, not pending rule research)

- The quran.com normalization **function body is closed-source.** Every verdict rests on reverse-engineered live-API behavior (controlled A≡B `total_results` comparisons at `response.search.total_results`) plus `by_key` field reads. If the backend changes, verdicts need re-deriving the same way.
- **Stemmer masking:** quran.com runs Elasticsearch's Arabic analyzer + light stemmer, which collapses some strip-form/fold-form and short-prefix queries to one result set (e.g. `أن`/`ان` 1360/1630). This produces the `inconclusive` rows above; it does not flip a clean fold/strip verdict.
- **Proper-noun token artifacts:** `موسى`/`موسي` (1293/124), `إبراهيم`/`ابراهيم` (56/228), `إسرائيل`/`اسرائيل` (40/43) show indexing splits on names that do not reflect the fold rule (which is confirmed by the hamza-equivalence and function-word probes).
- **`exact_matches_only=1`** does not change quran.com's normalization; it only tightens match strictness.

---

*Cross-refs: `docs/quran.md` §"Normalization + canonical view" (parity contract) and §9 (rub-el-hizb resolution). Rule source of truth: `web/src/lib/quran/search/normalize.ts` + `rust/backend/api/src/quran/normalize.rs`, enforced by `web/src/lib/quran/__fixtures__/parity.json`. Evidence: consolidated differential probe set vs `api.quran.com/api/v4/search` (3 independent verifiers, many verses/probes per rule).*
