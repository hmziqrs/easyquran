#!/usr/bin/env bash
# One-shot Quran corpus provisioner — the `quran-init` compose service.
#
# Same contract as deploy/fetch-quran-db.sh (CI / dev machines), minus the repo
# checkout: the deploy host has no node/python3 and no web/src tree, so the
# translation file list comes from the manifest COPYed into the api image at
# build time and is read with jq. Downloads are idempotent per file — present
# and non-empty is never refetched — so every redeploy re-runs this in seconds
# and picks up newly published translations only. No hashing anywhere; a
# truncated or error-page download dies on the SQLite magic-header assert.
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

echo "$BASE → $DEST"
get "tanzil/arabic/quran-uthmani.sqlite" "$DEST/arabic/quran-uthmani.sqlite"
get "tanzil/arabic/quran-simple-clean.sqlite" "$DEST/arabic/quran-simple-clean.sqlite"
get "tanzil/quran-data.xml" "$DEST/quran-data.xml"
assert_sqlite "$DEST/arabic/quran-uthmani.sqlite"
assert_sqlite "$DEST/arabic/quran-simple-clean.sqlite"

get "tanzil/translations/index.min.json" "$DEST/translations/index.min.json"
# Translation file names out of the baked catalogue COPYed into the image —
# never from a remote listing (the tracked map is the authority).
while read -r file; do
  get "tanzil/translations/$file" "$DEST/translations/$file"
  assert_sqlite "$DEST/translations/$file"
done < <(jq -r '.[][6]' "$MANIFEST")

echo "done"
