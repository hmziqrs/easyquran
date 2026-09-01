#!/usr/bin/env bash
# One-shot Quran corpus provisioner — the `quran-init` compose service.
#
# Same contract as deploy/fetch-quran-db.sh (CI / dev machines), minus the repo
# checkout: the deploy host has no node/python3 and no web/src tree, so the
# translation file list and runtime catalogue come from the manifest COPYed
# into the api image at build time and are produced with jq. Downloads are
# idempotent per file — present and non-empty is never refetched — so every
# redeploy re-runs this in seconds and picks up newly published translations.
# No hashing anywhere; a
# truncated or wrong-object download dies on the SQLite magic-header + expected
# byte-size asserts (size comes from the baked catalogue, keyed by id).
#
# Env:
#   QURAN_ARTIFACT_BASE=…  override the public R2 origin
#   FORCE=1               refetch even if present
set -euo pipefail

DEST="${QURAN_DB_DIR:-/app/quran}"
BASE="${QURAN_ARTIFACT_BASE:-https://r2.easyquran.fyi}"
BASE="${BASE%/}"
MANIFEST="${QURAN_MANIFEST:-/app/manifest/translations.json}"

get() {
  local key="$1" out="$2"
  if [ -s "$out" ] && [ "${FORCE:-0}" != "1" ]; then
    return
  fi
  mkdir -p "$(dirname "$out")"
  curl -fsSL --retry 3 --retry-connrefused -o "$out.part" "$BASE/$key"
  mv "$out.part" "$out"
  echo "  got $key"
}

assert_sqlite() {
  if [ "$(head -c 15 "$1")" != "SQLite format 3" ]; then
    echo "✗ $1 is not a SQLite database" >&2
    exit 1
  fi
}

# Expected byte size per object (identity = id + size, never a hash). Arabic
# sizes mirror web/src/lib/quran/view/source-profiles.ts sizeBytes, the xml size
# mirrors the R2 object — the DBs are immutable, so a mismatch means a
# truncated/corrupted download or a stale volume vs a newer baked manifest, and
# the provisioner refuses to bless it.
assert_size() {
  local file="$1" want="$2" got
  got=$(wc -c < "$file" | tr -d ' ')
  if [ "$got" != "$want" ]; then
    echo "✗ $file is $got bytes, expected $want (catalogue mismatch — remove the file and re-run)" >&2
    exit 1
  fi
}

write_catalogue() {
  local out="$DEST/translations/index.min.json"
  mkdir -p "$(dirname "$out")"
  jq '[.[] | {
    id: .[0],
    language: .[1],
    languageCode: .[2],
    direction: .[3],
    name: .[4],
    translator: .[5],
    file: {path: .[6], sizeBytes: .[7]}
  }]' "$MANIFEST" > "$out.part"
  mv "$out.part" "$out"
}

echo "$BASE → $DEST"
get "tanzil/arabic/quran-uthmani.sqlite" "$DEST/arabic/quran-uthmani.sqlite"
get "tanzil/arabic/quran-simple-clean.sqlite" "$DEST/arabic/quran-simple-clean.sqlite"
get "tanzil/arabic/quran-indopak.sqlite" "$DEST/arabic/quran-indopak.sqlite"
get "tanzil/arabic/quran-tajweed.sqlite" "$DEST/arabic/quran-tajweed.sqlite"
get "tanzil/quran-data.xml" "$DEST/quran-data.xml"
assert_sqlite "$DEST/arabic/quran-uthmani.sqlite"
assert_size "$DEST/arabic/quran-uthmani.sqlite" 1593344
assert_sqlite "$DEST/arabic/quran-simple-clean.sqlite"
assert_size "$DEST/arabic/quran-simple-clean.sqlite" 929792
assert_sqlite "$DEST/arabic/quran-indopak.sqlite"
assert_size "$DEST/arabic/quran-indopak.sqlite" 1634304
assert_sqlite "$DEST/arabic/quran-tajweed.sqlite"
assert_size "$DEST/arabic/quran-tajweed.sqlite" 2015232
assert_size "$DEST/quran-data.xml" 77234

# Translation file names + expected byte sizes out of the baked catalogue COPYed
# into the image (fields 6/7: filePath, sizeBytes) — never from a remote listing
# (the tracked map is the authority).
while read -r file want; do
  [ -n "$file" ] || continue
  get "tanzil/translations/$file" "$DEST/translations/$file"
  assert_sqlite "$DEST/translations/$file"
  assert_size "$DEST/translations/$file" "$want"
done < <(jq -r '.[] | "\(.[6]) \(.[7])"' "$MANIFEST")
write_catalogue

echo "done"
