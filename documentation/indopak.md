# IndoPak rendering

Overview of the IndoPak script font and renderer. Machine-readable evidence is tracked under
`scripts/fonts/indopak/` (`*-report.json`, `mapping.json`, `upstream.json`,
`outputs.json`); build and audit commands are in its [README](../../scripts/fonts/indopak/README.md).

## Status

- **Font v4** (`indopak-reader-compat-v4.{ttf,woff2}`, family `IndoPak Reader Compat`) is the
  IndoPak face in both reader modes. v2/v3 stay packaged for cached clients.
- **The IndoPak script is production-disabled** (`PRODUCTION_DISABLED_SOURCES` in
  `web/src/lib/quran/script-availability.ts`, campaign decision `0f5a8130`, together with
  Tajweed). It works in local/dev; flip only when licence/policy clears.
- Engineering validation is complete within the owner-agreed scope (2026-10-07). **Editorial and
  recitation approval are not claimed**, and owner visual scores are still blank.

## Hard constraints

- The DB (`db/quran/arabic/quran-indopak.sqlite`, id `quran-indopak`, profile
  `indopak-naveed-7d3c21e0`) is immutable; audits open it `mode=ro&immutable=1`. DOM text is the
  exact DB string — no normalization, no substitution, ever.
- No Quran-data hashing in any automated path. Before/after preservation of the five Arabic DBs
  was a manual observation ([`preservation-report.json`](../../scripts/fonts/indopak/preservation-report.json)).
- No restricted font asset (QuranWBW, PDMS, KFGQPC) enters the repo or build; reference fonts
  are inspected outside the repository only.

## How it works

**Corpus.** 6,236 verses, 727,385 characters, 87 distinct code points, nine private-use codes
with 1,383 occurrences, 223 context classes. Source: Naveed Ahmad's `Quran-text` (importer trims
whitespace); exact original download revision and printed edition were **not recovered**.

**Font.** Built by [`build.py`](../../scripts/fonts/indopak/build.py) from SIL Lateef 4.400
SemiBold (OFL; South Asian letterforms) — Quran.com's own face (AlQuran IndoPak by QuranWBW)
forbids modification. The nine private codes map onto Lateef's genuine Quranic glyphs (E003
subscript alef, E004 inverted damma, E01A zain, E01B sad, E01C qaf, E01E qif, E01F waqfa, E021
optional end of ayah, E022 ruku); U+FE8E aliases final alef; U+2003 keeps a one-em advance.
Ayah markers are plain rings with Urdu-form digits; `size-adjust: 125%` matches Quran.com's
letter size. The build pins font/license checksums and FontTools/Brotli/uharfbuzz versions,
rejects unresolved mappings, and never reads Quran data; rebuilds are byte-identical. TTF
242,160 bytes, WOFF2 92,416 bytes.

**Layout** (copied from Quran.com, `IndoPakAyah.svelte`). Every word is an unbreakable box, so
lines break only at source whitespace; pause signs (standalone or ZWSP-joined) stay with the
preceding word; stop-sign words get the wider gap; end signs stack above the ring; the final
word keeps its marker (deliberate deviation — Quran.com can orphan it). Text stays hidden and
`aria-hidden` until the font loads; WOFF2 failure falls back to TTF, both failing shows a
translated error with reload. IndoPak copy events serialize exact DOM ranges.

## Verification

Acceptance requirements and v4 outcome (evidence under the ignored local
`.cache/indopak-v4/2026-10-06-run2/`; summary in
[`v4-verification-report.json`](../../scripts/fonts/indopak/v4-verification-report.json)):

| #   | Requirement         | Outcome                                                                                          |
| --- | ------------------- | ------------------------------------------------------------------------------------------------ |
| R1  | Exact text          | Pass on all desktop engines + Android; iOS/iPadOS specimens only                                 |
| R2  | Coverage            | Zero `.notdef` over 6,236 verses; TTF/WOFF2 traces equal                                         |
| R3  | Sign visibility     | Pass desktop + Android (all 1,383 occurrences); iOS geometry-only                                |
| R4  | No collisions       | Pass desktop + Android; iOS geometry-only                                                        |
| R5  | Line breaking       | Pass; 20 justified segmentation exceptions                                                       |
| R6  | Overflow            | Pass except unbreakable source words at 56px (12:21, 56:23, 18:110)                              |
| R7  | Ayah ring / stacks  | Pass, numbers 1–286 at five sizes                                                                |
| R8  | Quran.com parity    | Full corpus compared; differences classified (below), editorial review open                      |
| R9  | Owner visual scores | **Pending** — 331 three-way crops prepared, no scores                                            |
| R10 | Platform reach      | Desktop Chromium/WebKit/Safari/Firefox pass; mobile coverage accepted by owner 2026-10-07        |
| R11 | Delivery            | Production bytes equal package, immutable headers, offline reload; Chrome 111 + Firefox 128 pass |
| R12 | Licensing           | Pass: OFL + FONTLOG shipped, reserved names absent, no restricted asset in repo or build         |
| R13 | Hard rules          | Pass: read-only DB, no Quran hashing, no tracked DB, gates green                                 |

Engines: Chromium 153, Playwright WebKit 26.6, native Safari 27.0, Firefox 155 (Linux container;
Firefox cannot launch on the dev Mac), Android 17 emulator, iPhone/iPad iOS 27 simulators.
Working support floor: Safari/iOS 17+, Chrome 111+, Firefox 128+ (chosen target; oldest
Safari/iOS untested). Line-break parity with Quran.com at equal width: 57–67% identical lines,
local slightly more compact.

## Open items

- **Editorial review**: a qualified reviewer and a named printed edition (publisher, line count)
  must reconcile the source discrepancies below; outcomes recorded per item.
- **Owner visual scores** (R9) on the prepared review sheets.
- **Deferred to native-app work**: full iOS ink (DOM masks), oldest iOS, assistive-technology
  pronunciation of private codes, OS multiline paste (native paste reorders combining marks even
  outside the app), Android corpus copy rerun.
- **Production enablement**: the script gate above; Quran.com's font would need a Quran
  Foundation Developer Console account + attribution (or QuranWBW written permission) — not
  pursued, the open Lateef derivative ships instead.
- **Mushaf page mode** (fixed 15-line pages) needs a licensed word-to-line layout dataset kept
  outside `db/`; out of scope for the flowing reader.

### Source discrepancies (never repaired silently — DB is immutable)

| Verse                                   | Local vs Quran.com legacy `textIndopak`                                   |
| --------------------------------------- | ------------------------------------------------------------------------- |
| 4:142                                   | Local `خَادِعُوْهُمۡ`, reference `خَادِعُهُمۡ` (extra waw + sukun)        |
| 12:1                                    | Reference adds final kasra: `الۡمُبِيۡنِ`                                 |
| 2:10                                    | Different order/word association of optional-ayah, no-stop and meem marks |
| 7:206                                   | Ruku and sajda markers in different order                                 |
| 2:219, 2:233, 2:243, 3:171, 9:111, 13:5 | Reference additionally has U+06DE rub-el-hizb                             |

Across the full corpus: all 1,383 private occurrences match the legacy field; served encoding
differs (5,778 identical, 191 order-only, 267 sign-pipeline); word boundaries 5,417 identical,
788 reference-joined, 26 source-joined, two letter-pipeline (4:142, 79:28), three mixed (4:171,
5:7, 6:77). Quran.com's current served `word.text` also moves annotations into end words (e.g.
79:27 waqfa → U+06D9), so matching legacy codes does not prove identical editorial text.

## History

| Version | Date       | Base             | What changed                                                                                               |
| ------- | ---------- | ---------------- | ---------------------------------------------------------------------------------------------------------- |
| v1      | 2026-10-05 | Noto Naskh 2.021 | Nine PUA aliases; E004 drifted toward alif, E021 borrowed a wrong ruku anchor                              |
| v2      | 2026-10-05 | Noto Naskh       | E004 intrinsic composite for its standalone bidi run; E021 dedicated glyph; end-cluster grouping           |
| v3      | 2026-10-06 | Noto Naskh       | Full-occurrence audit: E021 lane raised, terminal pause chains, orphaned endings, inline pairs (`104f049`) |
| v4      | 2026-10-06 | SIL Lateef       | Quran.com-style word boxes + rings; full verification run (font `b4b58f7`, candidate `584569c`)            |

Integration fixes found during v4 production checks: page/juz SSR ignored a saved IndoPak
preference (`0fede6b`); selection copy inserted linefeeds (`da69f33`); ring hover hit-testing
(`d570f9b`); reader ResizeObservers moved to animation frames (`7ccce6a`).
