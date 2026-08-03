#!/usr/bin/env bash
set -euo pipefail

ORIGIN="${1:-http://localhost:8080}"
fail=0

ok()   { printf '  ok   %s\n' "$*"; }
fail_() { printf '  FAIL %s\n' "$*"; fail=1; }
warn() { printf '  WARN %s\n' "$*"; }

contains() {
	local label="$1" haystack="$2" needle="$3"
	if grep -qi -- "$needle" <<<"$haystack"; then ok "$label"; else fail_ "$label (missing '$needle')"; fi
}

printf 'Asserting §6.1 delivery contract against %s\n' "$ORIGIN"

html=$(curl -sS -H 'Accept: text/html' "$ORIGIN/app")

immutable=$(grep -oE '/_app/immutable/[^"]+\.js' <<<"$html" | head -n1 || true)
if [[ -n "$immutable" ]]; then
	contains "immutable asset ($immutable) is immutable" "$(curl -sSI "$ORIGIN$immutable")" 'immutable'
else
	fail_ "no /_app/immutable/* reference on /app — cannot verify immutable"
fi

for p in /app /app/al-kahf; do
	code=$(curl -sS -o /dev/null -w '%{http_code}' "$ORIGIN$p" || true)
	[[ "$code" == "200" ]] || fail_ "$p is HTTP $code, expected 200 (clean-URL HTML not served — try_files?)"
done
ok "clean-URL HTML routes resolve (200)"

contains "/_app/version.json is no-cache"    "$(curl -sSI "$ORIGIN/_app/version.json")" 'no-cache'
contains "/service-worker.js is no-cache"    "$(curl -sSI "$ORIGIN/service-worker.js")" 'no-cache'
contains "/manifest.webmanifest is no-cache" "$(curl -sSI "$ORIGIN/manifest.webmanifest")" 'no-cache'
contains "/quran-meta/quran-data.json is no-cache" "$(curl -sSI "$ORIGIN/quran-meta/quran-data.json")" 'no-cache'
contains "HTML page (/app) is no-cache"      "$(curl -sSI "$ORIGIN/app")" 'no-cache'
contains "/app/al-kahf/__data.json is no-cache" "$(curl -sSI "$ORIGIN/app/al-kahf/__data.json")" 'no-cache'

robots_ok=0
for p in /app.txt /app.md /index.txt /index.md; do
	code=$(curl -sS -o /dev/null -w '%{http_code}' "$ORIGIN$p" || true)
	if [[ "$code" == "200" ]]; then
		contains "$p carries X-Robots-Tag noindex" "$(curl -sSI "$ORIGIN$p")" 'x-robots-tag: noindex'
		robots_ok=1
		break
	fi
done
[[ "$robots_ok" -eq 1 ]] || warn "no .md/.txt text-variant found — skipping robots check"

br=$(curl -sSI -H 'Accept-Encoding: br'   "$ORIGIN/app" | grep -i '^content-encoding:' || true)
gz=$(curl -sSI -H 'Accept-Encoding: gzip' "$ORIGIN/app" | grep -i '^content-encoding:' || true)
if grep -qi 'br' <<<"$br" || grep -qi 'gzip' <<<"$gz"; then
	ok "compression offered (br/gzip)"
else
	fail_ "no br/gzip content-encoding on /app"
fi

contains "unknown URL returns branded 404 shell" "$(curl -sS "$ORIGIN/this-route-does-not-exist-easyquran-xyz")" '__sveltekit'

code=$(curl -sS -o /dev/null -w '%{http_code}' "$ORIGIN/offline/manifest.json" || true)
if [[ "$code" == "200" ]]; then
	contains "/offline/manifest.json is no-cache" "$(curl -sSI "$ORIGIN/offline/manifest.json")" 'no-cache'
	pack=$(curl -sS "$ORIGIN/offline/manifest.json" | grep -oE '/offline/pack\.[0-9a-f]+\.json' | head -n1 || true)
	if [[ -n "$pack" ]]; then
		contains "pack $pack is immutable" "$(curl -sSI "$ORIGIN$pack")" 'immutable'
	else
		warn "manifest served but no pack path found"
	fi
else
	warn "/offline/manifest.json not served ($code) — pack is Phase 3, skipping"
fi

if [[ "$fail" -ne 0 ]]; then
	printf '\nassert-headers: one or more hard checks FAILED\n' >&2
	exit 1
fi
printf '\nassert-headers: all hard checks passed\n'
