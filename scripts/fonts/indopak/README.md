# IndoPak compatibility font

Version 4 is integrated into both IndoPak reader modes. Default Uthmani stays unchanged.
It is built on SIL Lateef SemiBold (OFL, South Asian letterforms) to match Quran.com's IndoPak
look; see [Quran.com study](../../../docs/indopak-qurancom-rendering-study.md). Versions 2–3
(Noto Naskh base) stay packaged for cached clients.
See [font/data evidence](../../../docs/indopak-font-compatibility.md) and
[Quran.com comparison](../../../docs/indopak-qurancom-comparison.md),
[deep audit/source discrepancies](../../../docs/indopak-deep-audit.md).

Run from repository root with Python 3.9+, Node 24+, and pnpm dependencies:

```sh
python3 -m venv .cache/indopak-venv
.cache/indopak-venv/bin/pip install -r scripts/fonts/indopak/requirements.txt
.cache/indopak-venv/bin/python scripts/fonts/indopak/build.py
.cache/indopak-venv/bin/python scripts/fonts/indopak/audit.py \
  --font web/static/fonts/indopak-reader-compat-v4.ttf \
  --font web/static/fonts/indopak-reader-compat-v4.woff2
.cache/indopak-venv/bin/python -m unittest discover -s scripts/fonts/indopak -p 'test_*.py'
.cache/indopak-venv/bin/python scripts/fonts/indopak/validate.py \
  --upstream .cache/fonts/indopak/Lateef-SemiBold.ttf
```

Equivalent commands when pinned Python dependencies are installed globally:
`pnpm font:indopak:preview` (the retained command accepts `--preview`),
`pnpm font:indopak:audit`, `pnpm font:indopak:test`.

The build verifies font/license checksums and pinned dependency versions. It rejects
unresolved/conflicting mappings, absent glyphs, altered original cmaps and inventory/context
mismatches. An unapproved manifest is rejected before downloading/writing production output.
It has no Quran DB input and never hashes Quran data.

Inventory/mapping files are tracked; immutable Quran DBs are separately provisioned under
ignored `db/`. Audit/validation open SQLite `mode=ro&immutable=1`. All three PUA ranges are
inventoried. No DB, restricted font, outlines or reference layout enter this package.

For actual browser checks, start the development server and run:

```sh
pnpm --dir web dev --port 5391
pnpm font:indopak:browser
```

Override host with `INDOPAK_SPECIMEN_BASE`, output with `INDOPAK_SPECIMEN_OUTPUT`; default
screenshots/report go to ignored `.cache/indopak-browser`. The `/design/indopak` specimen
returns 404 in production. It uses the real reader component and original DB strings.
A separate labelled diagnostic substitutes only E004/U+0657, never reader data.

Harness covers every recorded context/repertoire character, original DOM text, fetched font
bytes, sizes/wrapping/mobile, ornament enclosure and composing fallback. Actual canvas paint
checks detect inverted-damma drift and optional-ayah/pause collisions. End-sign checks reject
ornament collisions and orphaned final-word markers. Unavailable engines are reported, while
remaining engines continue; any engine failure exits nonzero. On this host automated Firefox
cannot launch. Focused native Firefox/Safari evidence is reported separately.

`production_approved` denotes technical browser integration, not printed-edition provenance
or recitation approval. Lateef's letterforms resemble, but are not, Quran.com's restricted font.
E004 intrinsic positioning is scoped to its one immutable corpus context and actual browser
bidi runs; forced whole-verse shaping is not a substitute for browser position verification.

Files: `upstream.json` input pins/provenance, `inventory.json` complete repertoire/context
counts, `mapping.json` symbol identities/constructions, `outputs.json` packaged font metadata,
`shaping-report.json` and `browser-report.json` validation, `qurancom-comparison.json` reference
observations, `requirements.txt` pinned build/test dependencies. Font assets include complete
OFL and modification log. Existing Quran text license/attribution is unchanged.

## Full occurrence and live-reference audits

Install additional audit dependencies with `pip install -r scripts/fonts/indopak/audit-requirements.txt`.
Use the same Python environment as the pinned build. These are explicit audits, not boot/build paths.

```sh
python3 scripts/fonts/indopak/deep_audit.py
node scripts/fonts/indopak/deep-browser-check.mjs
python3 scripts/fonts/indopak/dom_ink_analysis.py --engine chromium
python3 scripts/fonts/indopak/dom_ink_analysis.py --engine webkit
INDOPAK_DEEP_FLOW=1 INDOPAK_DEEP_ENGINES=chromium,webkit node scripts/fonts/indopak/deep-browser-check.mjs
python3 scripts/fonts/indopak/dom_ink_analysis.py --engine chromium-flow
python3 scripts/fonts/indopak/dom_ink_analysis.py --engine webkit-flow
node scripts/fonts/indopak/live-reference-check.mjs
python3 scripts/fonts/indopak/compare_reference.py
```

Full output defaults to ignored `.cache/indopak-deep`; override with `INDOPAK_DEEP_OUTPUT` and
`deep_audit.py --output`. `audit=all` adds every private-containing verse to the real reader
specimen. `audit=flow` checks every optional-ayah verse with original same-surah neighbors.
`INDOPAK_DEEP_ENGINES` selects engines; default includes Firefox, whose unavailable state
produces nonzero exit while other engines continue. Never report that as a Firefox pass.

`INDOPAK_DEEP_DIAGNOSTICS_ONLY=1` skips matrices for a targeted ink rerun;
`INDOPAK_DEEP_KEYS` optionally narrows DOM captures. Reports distinguish diagnostic-only from
full-matrix passes. Mask fonts preserve all original layout; transparent COLR layers isolate
ink. DOM reconstruction, negative controls and outside-mask checks reject invalid diagnostics.
Ink/boundary candidates stop approval until reviewed. Production-font/diagnostic raster
comparisons use a recorded two-pixel edge tolerance; masks include shared cmap aliases. No restricted reference font is used
to construct diagnostic fonts.

## Native macOS Safari

Playwright WebKit is not branded Safari. `safari-native-check.mjs` drives installed Safari
through bundled `/usr/bin/safaridriver` (W3C WebDriver), reusing `deep_audit.py` inputs and
the browser-side assertions in `deep-browser-shared.mjs`. Enable Safari → Settings →
Advanced → "Show features for web developers", then Developer → "Allow remote automation"
(first use may also need `safaridriver --enable`). One session at a time; an occluded
automation window is fine (frame waits are timer-bounded, screenshots paint on demand).

```sh
export INDOPAK_DEEP_OUTPUT=.cache/indopak-safari/<run>
python3 scripts/fonts/indopak/deep_audit.py --output "$INDOPAK_DEEP_OUTPUT"
node scripts/fonts/indopak/safari-native-check.mjs
python3 scripts/fonts/indopak/dom_ink_analysis.py --output "$INDOPAK_DEEP_OUTPUT" --engine safari
INDOPAK_DEEP_FLOW=1 node scripts/fonts/indopak/safari-native-check.mjs
python3 scripts/fonts/indopak/dom_ink_analysis.py --output "$INDOPAK_DEEP_OUTPUT" --engine safari-flow
```

Beyond the Playwright matrix it records environment metadata, requested versus measured
viewport, `devicePixelRatio`, served-versus-packaged font bytes, page/run horizontal overflow,
critical-verse captures (both modes, light/dark, 33/56px, 320/640px) and wrap-boundary sweeps.
Element screenshots must match element box × DPR or the run is invalid. Ink analysis keeps
the two-pixel edge tolerance in CSS pixels and scales it by capture DPR; overlap thresholds
stay in raw device pixels (stricter at higher DPR). `INDOPAK_SAFARI_SKIP_CRITICAL=1` skips
critical captures. Reports: `safari-report.json`, `safari-flow-report.json`.

Live reference tool fetches all representative context pages with two concurrent requests,
verifies deployed font family/resource, and stores unchanged reference assets outside repository
(default `/tmp/easyquran-deep-reference`, override `INDOPAK_REFERENCE_OUTPUT`). Comparison
script checks ordered legacy private-code sequences, records source differences, and does not
mutate or normalize DB/reader text. Full API access and qualified editorial review remain open.
