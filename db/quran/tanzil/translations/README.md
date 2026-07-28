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
├── index.min.json        # trimmed catalogue for the web (6 fields, bare array)
├── sql/<id>.sql          # the verse data, e.g. sql/en.sahih.sql
├── package.json          # workspace package + npm scripts
├── tsconfig.json
├── scripts/
│   ├── lib.ts            # shared logic (parse, download, index, types)
│   ├── fetch.ts          # mirror + build indexes        → pnpm mirror
│   ├── catalog.ts        # rebuild the trimmed catalogue  → pnpm catalog
│   ├── verify.ts         # integrity checks               → pnpm verify
│   └── upload.ts         # push to Cloudflare R2           → pnpm upload
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
pnpm upload --dry-run              # show what would be pushed to R2 (no creds)
pnpm typecheck                     # tsc --noEmit
```

Run these from `db/quran/tanzil/translations/` (or `pnpm --filter ...-translations
run <script>` from the root). `mirror`/`catalog` also copy the trimmed catalogue
to `web/src/lib/data/translations.json` so the static app can import it at build
time; override that path with `WEB_TRANSLATIONS_PATH`.

> `pnpm mirror` is named `mirror` (not `fetch`) because `pnpm fetch` is a
> reserved built-in command.

## Catalogues

### `index.min.json` — web-facing (bare array, ~15 KB raw / ~4 KB gzip)
```jsonc
[
  { "id": "en.sahih", "language": "English", "languageCode": "en",
    "direction": "ltr", "name": "Saheeh International", "translator": "Saheeh International" }
  // …115 entries
]
```
Exactly the fields a translation picker needs: `id` (load key), `name`,
`translator`, `language` + `languageCode` (group/filter), `direction` (render
RTL). Everything else — checksums, sizes, URLs, update dates, verse counts —
lives only in the full `index.json`.

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

`upload.ts` pushes the SQL dumps and the trimmed catalogue to an R2 bucket over
its S3-compatible API, skipping keys that already exist (use `--force` to
overwrite). Keys become `<prefix>sql/<id>.sql` and `<prefix>index.min.json`.

```sh
pnpm upload --dry-run     # plan only, no credentials needed
pnpm upload               # upload sql/*.sql + index.min.json
pnpm upload --force       # re-upload even existing keys
pnpm upload --full        # also push the fat index.json
```

Configure via environment variables:

| var | required | default | notes |
|-----|---------|---------|-------|
| `R2_ACCOUNT_ID` | yes* | — | *or set `R2_ENDPOINT` instead |
| `R2_ACCESS_KEY_ID` | yes | — | R2 API token → S3 auth key |
| `R2_SECRET_ACCESS_KEY` | yes | — | R2 API token → S3 secret |
| `R2_BUCKET` | no | `easyquran` | bucket name |
| `R2_KEY_PREFIX` | no | `translations/` | key prefix |
| `R2_ENDPOINT` | no | `https://<account>.r2.cloudflarestorage.com` | |
| `R2_PUBLIC_BASE` | no | — | e.g. `https://cdn.easyquran.app` — prints object URLs |

Objects are uploaded immutable (`Cache-Control: public, max-age=31536000,
immutable`). The SQL format ships as-is — if you'd rather serve web-ready JSON
per translation instead of SQL, say the word and I'll add a converter step.

## License & attribution

These texts are © the Tanzil Project and their respective translators, used
under the [Tanzil Terms of Use](https://tanzil.net/trans/): **non-commercial
use only**, and redistributing more than three translations **requires
attribution and a backlink to tanzil.net**. Keep this notice and the per-file
copyright headers intact.
