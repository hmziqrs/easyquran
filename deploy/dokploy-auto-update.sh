#!/usr/bin/env bash
#
# Pull-based auto-redeploy for the Dokploy compose stack. This is the deploy-side
# half of the pipeline: .github/workflows/images.yml only builds and pushes — it
# never pokes Dokploy. This job notices the new bytes on its own schedule.
#
# Run as a Dokploy Schedule Job, type "Dokploy Server" (that type runs inside
# the Dokploy container, which has curl, node, and the docker socket), cron e.g.
# `*/10 * * * *`. Paste this file's contents, then run it manually once and
# check the job logs before trusting the schedule.
#
# Config: Settings → Profile → API/CLI → Generate Token → API_KEY below.
# COMPOSE_ID identifies the stack: the id in the compose app's URL
# (/service/compose/<id>), or from the host:
#   curl -s http://localhost:3000/api/project.all -H "x-api-key: …" \
#     | jq '.[].environments[]?.compose[]? | {name, composeId}'
# (project.all returns appName:null on current Dokploy, so the id is the key.)
# The ghcr packages are public, so the plain `docker pull` needs no registry
# credentials. Approach follows Dokploy discussion #3381.
#
# Only moving tags (latest) can ever trigger a redeploy — an immutable `v*` pin
# never changes digest, so the job correctly leaves it alone.
#
# Failure safety: a digest change counts as consumed only AFTER compose.redeploy
# succeeds. The pull below has already made the local digest match, so the diff
# alone can never re-trigger — on failure the script exits non-zero and leaves
# the marker file (MARKER_FILE) so the next run retries the redeploy instead of
# wedging. The API key travels via a curl config piped on stdin (`-K -`), never
# in curl's argv where /proc would show it to other processes on the box.
set -euo pipefail

API_KEY="replace-me"
API_URL="http://localhost:3000/api"
COMPOSE_ID="replace-me"
MARKER_FILE="${MARKER_FILE:-/tmp/easyquran-auto-update.pending}"

# Shared curl wrapper: auth header arrives via stdin config, not argv.
api() {
  curl -sf -K - "$@" <<EOF
header = "x-api-key: $API_KEY"
EOF
}

# Image refs from the compose Dokploy actually runs
IMAGES=$(api "$API_URL/compose.getConvertedCompose?composeId=$COMPOSE_ID" | node -e '
  let d = "";
  process.stdin.on("data", c => (d += c));
  process.stdin.on("end", () => {
    for (const m of d.matchAll(/image:[\s\x22\x27]*([^\s\x22\x27]+)/g)) console.log(m[1]);
  });
')
[ -n "$IMAGES" ] || { echo "ERROR: no images found for compose $COMPOSE_ID" >&2; exit 1; }

CHANGED=0
for IMAGE in $IMAGES; do
  # Only images a running container actually uses
  docker ps -q --filter "ancestor=$IMAGE" | grep -q . || continue
  OLD=$(docker inspect --format '{{index .RepoDigests 0}}' "$IMAGE" 2>/dev/null || true)
  docker pull -q "$IMAGE" >/dev/null
  NEW=$(docker inspect --format '{{index .RepoDigests 0}}' "$IMAGE" 2>/dev/null || true)
  if [ "$OLD" != "$NEW" ]; then
    echo "$IMAGE: digest moved"
    CHANGED=1
  fi
done

# Write the pending marker BEFORE attempting the redeploy — once the redeploy
# call fails, this file is the only remaining retry signal.
if [ "$CHANGED" -eq 1 ]; then
  date -u +"%Y-%m-%dT%H:%M:%SZ redeploy pending (digest moved)" > "$MARKER_FILE"
fi

if [ "$CHANGED" -eq 1 ] || [ -f "$MARKER_FILE" ]; then
  if ! api -X POST "$API_URL/compose.redeploy" \
    -H "content-type: application/json" \
    -d "{\"composeId\":\"$COMPOSE_ID\"}"; then
    echo "ERROR: compose.redeploy failed — marker left at $MARKER_FILE, next run retries" >&2
    exit 1
  fi
  rm -f "$MARKER_FILE"
  echo "redeploy triggered"
else
  echo "all images current"
fi
