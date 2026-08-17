#!/usr/bin/env bash
# Provision the immutable Quran databases into db/ (gitignored, never committed).
#
# `db/` cannot survive a fresh clone, but the web build prerenders every Arabic route from
# db/quran/arabic/*.sqlite through node:sqlite — so CI (and any fresh machine, and the VPS
# checkout that bind-mounts db/quran into the api) must fetch them first.
#
# Source is the PUBLIC R2 base (https://r2.easyquran.fyi) — the same origin the browser
# downloads artifacts from. No credentials, no S3 client, no aws CLI: read-only GETs over
# HTTPS. Publishing still goes exclusively through `just upload-sqlite`.
#
# Keys come from the baked maps, which are the authority (docs/quran-system.md):
#   tanzil/arabic/<file>.sqlite          web/src/lib/quran/view/source-profiles.ts
#   tanzil/quran-data.xml
#   tanzil/translations/sqlite/<id>.sqlite   web/src/lib/data/translations.json (field 6)
#   tanzil/translations/index.min.json
#
# The DBs are immutable and unversioned, so a file already on disk is never refetched
# unless FORCE=1. No hashing anywhere — content asserts happen downstream (the prerender
# opens each DB and validates row counts and packaging).
#
# Usage:
#   deploy/fetch-quran-db.sh          # arabic + metadata: everything `pnpm --filter web build` needs
#   deploy/fetch-quran-db.sh all      # the above + every translation sqlite (~140 files, for the api)
#   FORCE=1 deploy/fetch-quran-db.sh  # refetch even if present
#   QURAN_ARTIFACT_BASE=… deploy/fetch-quran-db.sh   # point at a different origin
set -euo pipefail

MODE="${1:-arabic}"
case "$MODE" in
arabic | all) ;;
*)
  echo "mode must be 'arabic' or 'all' (received: $MODE)" >&2
  exit 2
  ;;
esac

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="$REPO_ROOT/db/quran"
BASE="${QURAN_ARTIFACT_BASE:-https://r2.easyquran.fyi}"
BASE="${BASE%/}"

# get <r2-key> <local-path>
get() {
  local key="$1" out="$2"
  if [ -s "$out" ] && [ "${FORCE:-0}" != "1" ]; then
    return
  fi
  mkdir -p "$(dirname "$out")"
  # --fail: a 404/403 must break the build here, not silently write an HTML error page that
  # only fails later inside the prerender.
  curl -fsSL --retry 3 --retry-connrefused -o "$out.part" "$BASE/$key"
  mv "$out.part" "$out"
  echo "  got $key"
}

# A truncated or error-page download must not survive as a "database".
assert_sqlite() {
  local file="$1"
  if [ "$(head -c 15 "$file")" != "SQLite format 3" ]; then
    echo "✗ $file is not a SQLite database" >&2
    exit 1
  fi
}

echo "$BASE → db/quran (mode: $MODE)"
get "tanzil/arabic/quran-uthmani.sqlite" "$DEST/arabic/quran-uthmani.sqlite"
get "tanzil/arabic/quran-simple-clean.sqlite" "$DEST/arabic/quran-simple-clean.sqlite"
get "tanzil/quran-data.xml" "$DEST/quran-data.xml"
assert_sqlite "$DEST/arabic/quran-uthmani.sqlite"
assert_sqlite "$DEST/arabic/quran-simple-clean.sqlite"

if [ "$MODE" = "all" ]; then
  get "tanzil/translations/index.min.json" "$DEST/translations/index.min.json"
  # Ids come from the tracked baked catalogue, never from a remote listing.
  while read -r file; do
    get "tanzil/translations/$file" "$DEST/translations/$file"
    assert_sqlite "$DEST/translations/$file"
  done < <(node --input-type=module -e "
    import { readFileSync } from 'node:fs';
    const rows = JSON.parse(readFileSync('$REPO_ROOT/web/src/lib/data/translations.json', 'utf8'));
    for (const row of rows) console.log(row[6]);
  ")
fi

echo "done"
