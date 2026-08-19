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
set -euo pipefail

API_KEY="replace-me"
API_URL="http://localhost:3000/api"
COMPOSE_ID="replace-me"

# Image refs from the compose Dokploy actually runs
IMAGES=$(curl -sf "$API_URL/compose.getConvertedCompose?composeId=$COMPOSE_ID" \
  -H "x-api-key: $API_KEY" | node -e '
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

if [ "$CHANGED" -eq 1 ]; then
  curl -sf -X POST "$API_URL/compose.redeploy" \
    -H "x-api-key: $API_KEY" -H "content-type: application/json" \
    -d "{\"composeId\":\"$COMPOSE_ID\"}"
  echo "redeploy triggered"
else
  echo "all images current"
fi
