#!/usr/bin/env bash
# EasyQuran auto-deploy — one script. Run it from cron.
#
# Polls the private GitHub repo for a new vX.Y.Z tag, checks it out, rebuilds
# web + api, and redeploys ONLY those two (the external Traefik is untouched). A
# failed build never takes the site down: running containers stay up.
#
# Config (set in the environment — e.g. a root-only file your cron line sources):
#   REPO_DIR       checkout path            (default: parent of this script)
#   GH_REPO        owner/name               (required)
#   GH_TOKEN       fine-grained PAT, Contents:Read (required)
#   DEPLOY_TAG_GLOB tag glob to deploy      (default: v[0-9]*)
#
# Example crontab (every 2 min):
#   */2 * * * *  /opt/easyquran/deploy/deploy.sh >> /var/log/easyquran-deploy.log 2>&1
# with the secrets sourced first, e.g.:
#   */2 * * * *  . /etc/easyquran/secrets.env && /opt/easyquran/deploy/deploy.sh >> /var/log/easyquran-deploy.log 2>&1

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="${REPO_DIR:-"$(dirname "$SCRIPT_DIR")"}"
DEPLOY_TAG_GLOB="${DEPLOY_TAG_GLOB:-v[0-9]*}"
STATE_FILE="$REPO_DIR/.deploy-state/last-tag"
COMPOSE="docker compose -f $REPO_DIR/docker-compose.yml"

: "${GH_REPO:?GH_REPO must be set (owner/repo)}"
: "${GH_TOKEN:?GH_TOKEN must be set (fine-grained PAT, Contents:Read)}"

log() { printf '[%(%F %T)T] %s\n' '-1' "$*"; }

exec 9>/var/lock/easyquran-deploy.lock
flock -n 9 || { log "another run in progress; exiting"; exit 0; }

mkdir -p "$(dirname "$STATE_FILE")"
LAST_TAG="$(cat "$STATE_FILE" 2>/dev/null || true)"
cd "$REPO_DIR"

export GIT_TERMINAL_PROMPT=0
CRED="credential.helper=!f() { echo username=x-access-token; echo password=${GH_TOKEN}; }; f"
git -c "$CRED" fetch --tags --force origin || { log "ERROR: git fetch failed"; exit 1; }

LATEST_TAG="$(git tag --list "$DEPLOY_TAG_GLOB" | sed -E 's/^v//' | sort -V -r | head -n1 | sed -E 's/^/v/')"
[ -z "$LATEST_TAG" ] && { log "no tag matching '$DEPLOY_TAG_GLOB'; nothing to do"; exit 0; }
[ "$LATEST_TAG" = "$LAST_TAG" ] && { log "$LATEST_TAG already deployed"; exit 0; }

log "deploying $LATEST_TAG (was '${LAST_TAG:-<none>}')"
git checkout --force "$LATEST_TAG"

# Stamp the images (OCI label org.opencontainers.image.version) with the tag.
export VERSION="$LATEST_TAG"
export REVISION="$(git rev-parse --short=12 HEAD)"
$COMPOSE build web api || { log "ERROR: build failed for $LATEST_TAG; containers left as-is"; exit 1; }
$COMPOSE up -d web api

sleep 5
curl -fsS --max-time 15 "https://${HEALTH_DOMAIN:-easyquran.fyi}/api/healthz" >/dev/null 2>&1 \
  && log "healthcheck ok" || log "WARN: healthcheck not green — check 'docker compose logs api'"

printf '%s\n' "$LATEST_TAG" >"$STATE_FILE"
log "deployed $LATEST_TAG"
