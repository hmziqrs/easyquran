# Tanzil Quran Translations

Every Quran translation published at <https://tanzil.net/trans/>, mirrored as
MySQL-dump SQL (the same format the Arabic text uses under
`../arabic/sql/`), plus an `index.json` that makes the catalogue easy to
consume directly.

- **115 translations** across **44 languages** (8 right-to-left)
- every file is a complete Quran: **6236 verses**
- retrieved **2026-07-29**

## Layout

```
translations/
├── index.json          # catalogue — one entry per translation (see below)
├── sql/<id>.sql        # the verse data, e.g. sql/en.sahih.sql
├── fetch.py            # regenerates everything from tanzil.net
└── verify.py           # integrity checks (encoding, verse count, checksums)
```

Each `sql/<id>.sql` is a phpMyAdmin dump with one table `<lang>_<translator>`
of `(index, sura, aya, text)` rows, and a header block carrying the
authoritative `Name` / `Translator` / `Language` / `Last Update` for that file.

## `index.json`

```jsonc
{
  "source": "Tanzil.net",
  "count": 115,
  "translations": [
    {
      "id": "en.sahih",                 // tanzil id, also the file stem
      "language": "English",            // display language name
      "languageCode": "en",             // id prefix
      "direction": "ltr",               // "ltr" | "rtl" (script direction)
      "name": "Saheeh International",   // from the SQL header (authoritative)
      "nameNative": null,               // page display name when it differs (often native script)
      "translator": "Saheeh International",
      "lastUpdate": "April 24, 2011",
      "ayaCount": 6236,
      "file": { "sql": "sql/en.sahih.sql", "sizeBytes": 987654, "sha256": "…" },
      "urls": { "tanzil": "…", "download": "…?type=sql", "browse": "…", "changelog": "…" }
    }
    // …
  ]
}
```

The three fields most apps want up front — **language**, **name**,
**translator** — are top-level on each entry.

## Regenerating

```sh
python3 fetch.py            # download anything missing, rebuild index.json
python3 fetch.py --force    # re-download every file
python3 verify.py           # check integrity
```

`fetch.py` discovers the id list and display fields from the `/trans/` page
(source of truth for language/name/translator), downloads each
`/trans/<id>?type=sql` with retries, and takes `lastUpdate` + verse counts
from each dump's header. It is idempotent — already-downloaded files are kept.

## License & attribution

These texts are © the Tanzil Project and their respective translators, used
under the [Tanzil Terms of Use](https://tanzil.net/trans/): **non-commercial
use only**, and redistributing more than three translations **requires
attribution and a backlink to tanzil.net**. Keep this notice and the
per-file copyright headers intact.
