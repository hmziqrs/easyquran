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
# QURAN_DB_DIR is the same knob docker-compose.yml reads: on a managed host the databases
# belong outside the deploy tool's checkout. Default is the in-repo gitignored path.
DEST="${QURAN_DB_DIR:-$REPO_ROOT/db/quran}"
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

# Translation file names, one per line, straight out of the tracked catalogue (field 6) —
# never from a bucket listing. node or python3, whichever the host has: a VPS running this in
# `all` mode carries no toolchain, only the images it pulls.
# Emits "filePath sizeBytes" per row (fields 6/7) out of the tracked catalogue.
translation_files() {
  local json="$REPO_ROOT/web/src/lib/data/translations.json"
  if command -v node >/dev/null; then
    node --input-type=module -e "
      import { readFileSync } from 'node:fs';
      for (const row of JSON.parse(readFileSync('$json', 'utf8'))) console.log(row[6] + ' ' + row[7]);
    "
    return
  fi
  if command -v python3 >/dev/null; then
    python3 -c "
import json
for row in json.load(open('$json')): print(row[6], row[7])
"
    return
  fi
  echo "need node or python3 to read translations.json" >&2
  exit 127
}

# A truncated or error-page download must not survive as a "database".
assert_sqlite() {
  local file="$1"
  if [ "$(head -c 15 "$file")" != "SQLite format 3" ]; then
    echo "✗ $file is not a SQLite database" >&2
    exit 1
  fi
}

# Expected byte size per object (identity = id + size, never a hash). Arabic
# sizes mirror web/src/lib/quran/view/source-profiles.ts sizeBytes, the xml size
# mirrors the R2 object — the DBs are immutable, so a mismatch means a
# truncated/corrupted download, and the fetch refuses to bless it.
assert_size() {
  local file="$1" want="$2" got
  got=$(wc -c < "$file" | tr -d ' ')
  if [ "$got" != "$want" ]; then
    echo "✗ $file is $got bytes, expected $want (catalogue mismatch — remove the file and re-run)" >&2
    exit 1
  fi
}

echo "$BASE → db/quran (mode: $MODE)"
get "tanzil/arabic/quran-uthmani.sqlite" "$DEST/arabic/quran-uthmani.sqlite"
get "tanzil/arabic/quran-simple-clean.sqlite" "$DEST/arabic/quran-simple-clean.sqlite"
get "tanzil/quran-data.xml" "$DEST/quran-data.xml"
assert_sqlite "$DEST/arabic/quran-uthmani.sqlite"
assert_size "$DEST/arabic/quran-uthmani.sqlite" 1593344
assert_sqlite "$DEST/arabic/quran-simple-clean.sqlite"
assert_size "$DEST/arabic/quran-simple-clean.sqlite" 929792
assert_size "$DEST/quran-data.xml" 77234

if [ "$MODE" = "all" ]; then
  get "tanzil/translations/index.min.json" "$DEST/translations/index.min.json"
  # File + expected size come from the tracked baked catalogue (fields 6/7:
  # filePath, sizeBytes), never from a remote listing.
  while read -r file want; do
    [ -n "$file" ] || continue
    get "tanzil/translations/$file" "$DEST/translations/$file"
    assert_sqlite "$DEST/translations/$file"
    assert_size "$DEST/translations/$file" "$want"
  done < <(translation_files)
fi

echo "done"
