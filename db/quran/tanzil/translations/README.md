# Tanzil Quran Translations

Every Quran translation published at <https://tanzil.net/trans/>, mirrored as
MySQL-dump SQL (the same format the Arabic text uses under `../arabic/sql/`),
plus JSON catalogues that make the dataset easy to consume.

- **115 translations** across **44 languages** (8 right-to-left)
- every file is a complete Quran: **6236 verses**
- retrieved **2026-07-29**

The pipeline is TypeScript, runnable through pnpm (this folder is a workspace
package). The same codebase mirrors the data, builds a trimmed web catalogue,
verifies integrity, and uploads to Cloudflare R2.

## Layout

```
translations/
├── index.json            # full catalogue — one rich entry per translation
├── index.min.json        # trimmed catalogue for the web (7 fields, bare array)
├── sql/<id>.sql          # mirrored verse data, e.g. sql/en.sahih.sql — never published
├── sqlite/<id>.sqlite    # built artifact, e.g. sqlite/en.sahih.sqlite — published
├── package.json          # workspace package + npm scripts
├── tsconfig.json
├── scripts/
│   ├── lib.ts            # shared logic (parse, download, index, types)
│   ├── fetch.ts          # mirror + build indexes         → pnpm mirror
│   ├── catalog.ts        # rebuild the trimmed catalogue  → pnpm catalog
│   ├── verify.ts         # integrity checks               → pnpm verify
│   ├── sql-to-sqlite.ts  # dumps → SQLite                 → pnpm build:sqlite
│   └── upload-sqlite.ts  # push SQLite + catalogue to R2  → pnpm upload:sqlite
└── README.md
```

Each `sql/<id>.sql` is a phpMyAdmin dump with one table `<lang>_<translator>`
of `(index, sura, aya, text)` rows, plus a header block carrying the
authoritative `Name` / `Translator` / `Language` / `Last Update`.

## Commands

```sh
pnpm install                       # from repo root — installs this package's deps
pnpm mirror                        # download missing dumps + rebuild both indexes
pnpm mirror --force                # re-download every dump
pnpm mirror --limit 5              # operate on the first 5 (smoke test)
pnpm catalog                       # rebuild just index.min.json (+ web copy)
pnpm verify                        # integrity: encoding, verse counts, checksums
pnpm build:sqlite                  # convert the dumps → sqlite/<id>.sqlite
pnpm upload:sqlite --dry-run       # show what would be pushed to R2 (no creds)
pnpm typecheck                     # tsc --noEmit
```

Run these from `db/quran/tanzil/translations/` (or `pnpm --filter ...-translations
run <script>` from the root). `mirror`/`catalog` also copy the trimmed catalogue
to `web/src/lib/data/translations.json` so the static app can import it at build
time; override that path with `WEB_TRANSLATIONS_PATH`.

> `pnpm mirror` is named `mirror` (not `fetch`) because `pnpm fetch` is a
> reserved built-in command.

## Catalogues

### `index.min.json` — web-facing (bare array, ~25 KB raw / ~4.5 KB gzip)
```jsonc
[
  { "id": "en.sahih", "language": "English", "languageCode": "en",
    "direction": "ltr", "name": "Saheeh International", "translator": "Saheeh International",
    "file": "sqlite/en.sahih.sqlite" }
  // …115 entries
]
```
Exactly the fields a translation picker needs: `id` (load key), `name`,
`translator`, `language` + `languageCode` (group/filter), `direction` (render
RTL), `file` (what to download). Everything else — checksums, sizes, URLs,
update dates, verse counts — lives only in the full `index.json`.

`file` is **relative to the catalogue's own location**, so the same catalogue
resolves on disk and in the bucket. It names the SQLite build, never the raw
`.sql` dump — the dumps stay in the repo and are not published. Treat the path
as a placeholder until the derived delivery databases in
`docs/quran-web-delivery.md` land; `index.json` still records `file.sql` as the
mirror's provenance.

### `index.json` — full (source of truth)
```jsonc
{
  "source": "Tanzil.net", "sourceUrl": "…", "format": "sql (MySQL phpMyAdmin dump)",
  "license": "…", "count": 115,
  "translations": [
    {
      "id": "en.sahih", "language": "English", "languageCode": "en", "direction": "ltr",
      "name": "Saheeh International",            // from the SQL header (authoritative)
      "nameNative": null,                         // page display name when it differs (often native script)
      "translator": "Saheeh International", "lastUpdate": "April 24, 2011",
      "ayaCount": 6236,
      "file": { "sql": "sql/en.sahih.sql", "sizeBytes": 987654, "sha256": "…" },
      "urls": { "tanzil": "…", "download": "…?type=sql", "browse": "…", "changelog": "…" }
    }
  ]
}
```

## Upload to Cloudflare R2

`upload-sqlite.ts` is the only publisher for this dataset. It pushes the SQLite
builds and the trimmed catalogue over R2's S3-compatible API, skipping keys that
already exist (use `--force` to overwrite). Keys mirror the tanzil source tree,
so the bucket reads like the repo:

```
tanzil/translations/index.min.json      # catalogue — always re-uploaded
tanzil/translations/sqlite/<id>.sqlite  # 115 translations
tanzil/arabic/<file>.sqlite             # 2 Arabic texts
```

**The raw `sql/*.sql` dumps are not published.** They are the upstream mirror
format, already versioned in git, and nothing downstream can read a phpMyAdmin
dump — only the SQLite artifacts ship.

```sh
pnpm upload:sqlite --dry-run   # plan only, no credentials needed
pnpm upload:sqlite             # upload sqlite/*.sqlite + arabic/*.sqlite + catalogue
pnpm upload:sqlite --force     # re-upload even existing keys
```

Or from the repo root, which loads `.env` for you: `just upload-sqlite`.

Configure via environment variables:

| var | required | default | notes |
|-----|---------|---------|-------|
| `R2_ACCOUNT_ID` | yes* | — | *or set `R2_ENDPOINT` instead |
| `R2_ACCESS_KEY_ID` | yes | — | R2 API token → S3 auth key |
| `R2_SECRET_ACCESS_KEY` | yes | — | R2 API token → S3 secret |
| `R2_BUCKET` | no | `easyquran` | bucket name |
| `R2_ENDPOINT` | no | `https://<account>.r2.cloudflarestorage.com` | |
| `R2_PUBLIC_BASE` | no | — | e.g. `https://cdn.easyquran.app` — prints the catalogue URL |

The key prefix is fixed at `tanzil/` because it mirrors the source tree the
catalogue's relative paths resolve against.

The `.sqlite` objects are uploaded immutable (`Cache-Control: public,
max-age=31536000, immutable`) — one key per translation id, never rewritten in
place. The catalogue is the exception: it changes whenever the translation set
changes, so it ships as `public, max-age=300, must-revalidate` and is
re-uploaded on every run regardless of `--force`.

## License & attribution

These texts are © the Tanzil Project and their respective translators, used
under the [Tanzil Terms of Use](https://tanzil.net/trans/): **non-commercial
use only**, and redistributing more than three translations **requires
attribution and a backlink to tanzil.net**. Keep this notice and the per-file
copyright headers intact.
