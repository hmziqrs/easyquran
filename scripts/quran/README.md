# Quran artifact publisher

`upload.ts` is the single Cloudflare R2 publisher for every production Quran artifact:

- Arabic SQLite databases from `db/quran/arabic/`
- Tanzil translation SQLite databases from `db/quran/translations/sqlite/`
- QuranEnc translation SQLite databases from `db/quran/translations/quranenc/sqlite/`
- `db/quran/quran-data.xml`
- merged catalogue generated deterministically from tracked translation map

Paths, expected byte sizes, and catalogue come from tracked baked maps/constants. Publisher
validates each local file and checks SQLite headers before contacting R2. It never hashes or
modifies Quran data.

```bash
pnpm install --frozen-lockfile
just quran-fetch all
pnpm quran:check
just upload-sqlite --dry-run
just upload-sqlite
```

Immutable objects already present at expected size are skipped and can never be overwritten
by this script. New immutable writes use a create-only condition, closing concurrent-publisher
races. Remote size mismatch fails before any write. Catalogue is mutable and uploads last,
only after every immutable artifact succeeds.

`just quran-fetch all` creates the canonical local layout under ignored
`db/quran/translations/`, including QuranEnc. The publisher generates the full runtime
catalogue from `web/src/lib/data/translations.json`; a downloaded catalogue is never its input.

Required environment variables: `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, plus
`R2_ACCOUNT_ID` or `R2_ENDPOINT`. Optional: `R2_BUCKET` (default `easyquran`) and
`R2_PUBLIC_BASE`.
