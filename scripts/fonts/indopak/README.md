# IndoPak compatibility preview

Production blocked. This font demonstrates complete nominal coverage; it does not make the
existing PUA text render correctly in browser bidi/shaping. See
[evidence and remaining work](../../../docs/indopak-font-compatibility.md).

[Live Quran.com comparison](../../../docs/indopak-qurancom-comparison.md) corroborates symbol
identities and records the additional E021/no-stop stacking defect. Observed font/deployment
pins and sampled results are in `qurancom-comparison.json`; reference assets remain outside
the repository.

Run from repository root with Python 3.9 or newer, Node 24 or newer, and pnpm workspace deps.
Font dependencies are build/test tools only; none enter browser runtime.

```sh
python3 -m venv .cache/indopak-venv
.cache/indopak-venv/bin/pip install -r scripts/fonts/indopak/requirements.txt
.cache/indopak-venv/bin/python scripts/fonts/indopak/build.py --preview
.cache/indopak-venv/bin/python scripts/fonts/indopak/audit.py \
  --font web/static/fonts/indopak-reader-compat-preview-v1.ttf \
  --font web/static/fonts/indopak-reader-compat-preview-v1.woff2
.cache/indopak-venv/bin/python -m unittest discover -s scripts/fonts/indopak -p 'test_*.py'
.cache/indopak-venv/bin/python scripts/fonts/indopak/validate.py \
  --upstream .cache/fonts/indopak/NotoNaskhArabic-wght.ttf
```

When these pinned Python packages are available to `python3`, equivalent pnpm commands are
`font:indopak:preview`, `font:indopak:audit`, and `font:indopak:test`.

`build.py` without `--preview` rejects production activation before downloading or writing
anything. Preview mode still rejects unresolved mappings, duplicate assignments, missing
glyphs, inventory/context mismatches, altered original cmap entries and invalid packaged
layout. It verifies checksums of font/license inputs on every use, including cached inputs,
and verifies pinned dependency versions. It never reads or hashes Quran data.

No DBs are build inputs. `inventory.json` and `mapping.json` survive a fresh clone. Corpus and
browser validation require separately provisioned immutable Quran DBs; `audit.py` opens SQLite
with `mode=ro&immutable=1`. Its inventory includes all three PUA ranges. Nothing in this
directory computes a Quran hash.

Start existing local web development server, then run browser checks:

```sh
pnpm exec playwright install chromium firefox webkit
pnpm --dir web dev --port 5391
```

In another terminal:

```sh
pnpm font:indopak:browser
```

The specimen is `/design/indopak`, returns 404 in production, and passes original DB strings
through the existing `ReadingAyah` component. Font controls wait for hydration. One separately
labelled diagnostic row substitutes E004 with U+0657; it is development-only and never reader
data. No production script/font selection changes.

Override host with `INDOPAK_SPECIMEN_BASE`; screenshots/report default to ignored
`.cache/indopak-browser`, overridable with `INDOPAK_SPECIMEN_OUTPUT`. Browser assertions check
original strings, every inventory character, fetched packaged font bytes, font-family usage,
multiple sizes/widths and mobile viewport. They also preserve a visual reproduction of the
known E004 failure. A passing harness means those checks ran, not that positioning passed.
End-of-ayah checks cover one-, two- and three-digit medallions in every specimen/control,
including the Scheherazade fallback when the ornament font cannot load. Escaped digits widen
the run beyond the composing face's one-em medallion; the harness rejects that regression.
Engine failures are recorded while remaining engines continue; any failure exits nonzero.
Recorded full automated Firefox run is unavailable on audited host; installed Firefox GUI
inspection is reported separately alongside native Safari.

Files:

- `upstream.json`: exact revisions, font input checksums, reference provenance.
- `inventory.json`: counts, locations, Unicode properties and context classes.
- `mapping.json`: meanings, evidence, source behavior, constructions, approval blocker.
- `outputs.json`: packaged font metadata/checksums from reviewed build.
- `shaping-report.json`, `browser-report.json`: recorded validation results.
- `requirements.txt`: pinned FontTools, Brotli, uharfbuzz versions.

Keep reference fonts outside repository. They are never downloaded by build, copied,
outlined, traced, embedded, or used as layout sources. All derivative outlines/layout come
from the pinned Noto OFL input. Original Quran text licensing remains unchanged.
