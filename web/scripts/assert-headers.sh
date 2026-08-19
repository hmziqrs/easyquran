#!/usr/bin/env bash
set -euo pipefail

# Asserts (1) the §6.1 delivery contract against the web origin and (2) the W2
# ingress identity contract against the api origin. Run before each release:
#
#   web/scripts/assert-headers.sh http://localhost:8080 http://localhost:8888
#
# W2 ingress notes:
#   - The api origin must run in production (RUST_ENV=production,
#     IP_SOURCE=CfConnectingIp). External requests are simulated by sending
#     CF-Connecting-IP directly — the SAME header Cloudflare sets. Proving a
#     public caller CANNOT forge it requires the live CF→Traefik path (restrict
#     origin ingress to Cloudflare ranges; see deploy/README.md). That live
#     negative test is owner-run and documented there.
#   - INTERNAL_QURAN_API_TOKEN may be exported to exercise the internal-SSR
#     bucket; if unset, the token checks warn instead of failing hard.

ORIGIN="${1:-http://localhost:8080}"
API_ORIGIN="${2:-}"
INTERNAL_TOKEN="${INTERNAL_QURAN_API_TOKEN:-}"
# A simulated verified client IP (TEST-NET-3 / documentation range). Behind real
# Cloudflare this header is set by CF, not by the caller.
CF_IP="${CF_CONNECTING_IP:-203.0.113.10}"
fail=0

ok()   { printf '  ok   %s\n' "$*"; }
fail_() { printf '  FAIL %s\n' "$*"; fail=1; }
warn() { printf '  WARN %s\n' "$*"; }

contains() {
	local label="$1" haystack="$2" needle="$3"
	if grep -qi -- "$needle" <<<"$haystack"; then ok "$label"; else fail_ "$label (missing '$needle')"; fi
}

absent() {
	local label="$1" haystack="$2" needle="$3"
	if grep -qi -- "$needle" <<<"$haystack"; then fail_ "$label (unexpected '$needle')"; else ok "$label"; fi
}

code_is() {
	local label="$1" got="$2" want="$3"
	if [[ "$got" == "$want" ]]; then ok "$label ($got)"; else fail_ "$label (got $got, want $want)"; fi
}

printf 'Asserting §6.1 delivery contract against %s\n' "$ORIGIN"

# /app itself is a 92-byte locale-redirect stub (no theme script, no asset refs);
# /en/app is the real prerendered page both probes below need.
html=$(curl -sS -H 'Accept: text/html' "$ORIGIN/en/app")

immutable=$(grep -oE '/_app/immutable/[^"]+\.js' <<<"$html" | head -n1 || true)
if [[ -n "$immutable" ]]; then
	immutable_headers=$(curl -sSI "$ORIGIN$immutable")
	contains "immutable asset ($immutable) is immutable" "$immutable_headers" 'immutable'
	contains "immutable asset carries nosniff" "$immutable_headers" 'x-content-type-options: nosniff'
	contains "immutable asset carries HSTS" "$immutable_headers" 'strict-transport-security:'
else
	fail_ "no /_app/immutable/* reference on /app — cannot verify immutable"
fi

for p in /app /app/al-kahf; do
	code=$(curl -sS -o /dev/null -w '%{http_code}' "$ORIGIN$p" || true)
	[[ "$code" == "200" ]] || fail_ "$p is HTTP $code, expected 200 (clean-URL HTML not served — try_files?)"
done
ok "clean-URL HTML routes resolve (200)"

# Localized canonical reader path — the bare /app/** prefix 307s to /{en,ar}/app/**.
translation_path=/en/app/al-fatihah/t/en/sahih
translation_html=$(curl -sS "$ORIGIN$translation_path")
contains "$translation_path renders translated ayahs in SSR HTML" "$translation_html" 'data-verse-key="1:1"'
translation_headers=$(curl -sS -D - -o /dev/null "$ORIGIN$translation_path")
contains "$translation_path warm request is served by disk cache" "$translation_headers" 'x-easyquran-quran-cache: hit'
translation_data_headers=$(curl -sS -D - -o /dev/null "$ORIGIN$translation_path/__data.json")
contains "translation __data.json remains data, never cached HTML" "$translation_data_headers" 'content-type: application/json'
authed_headers=$(curl -sS -D - -o /dev/null -H 'Cookie: session=private' "$ORIGIN$translation_path")
contains "cookie-bearing SSR keeps its private tier (server never clobbers it)" "$authed_headers" 'cache-control: private, no-store'
anonymous_csp=$(curl -sS -D - -o /dev/null "$ORIGIN$translation_path" | grep -i '^content-security-policy:' || true)
contains "SSR (hooks) CSP carries a per-request nonce" "$anonymous_csp" "'nonce-"
web_quran_health=$(curl -sS "$ORIGIN/health/quran")
contains "web Quran health reports readiness only" "$web_quran_health" '"ready":true'
absent "web Quran health leaks no cache metrics" "$web_quran_health" 'translatedPageCache'

contains "/_app/version.json is no-cache"    "$(curl -sSI "$ORIGIN/_app/version.json")" 'no-cache'
contains "/service-worker.js is no-cache"    "$(curl -sSI "$ORIGIN/service-worker.js")" 'no-cache'
contains "/manifest.webmanifest is no-cache" "$(curl -sSI "$ORIGIN/manifest.webmanifest")" 'no-cache'
contains "/quran-meta/quran-data.json is no-cache" "$(curl -sSI "$ORIGIN/quran-meta/quran-data.json")" 'no-cache'
artifact_path=/_quran/tanzil/arabic/quran-uthmani.sqlite
artifact_headers=$(curl -sSI "$ORIGIN$artifact_path")
contains "same-origin Quran artifact gateway is immutable" "$artifact_headers" 'immutable'
artifact_etag=$(printf '%s' "$artifact_headers" | sed -n 's/^[Ee][Tt][Aa][Gg]:[[:space:]]*//p' | tr -d '\r')
artifact_conditional_status=$(curl -sS -o /dev/null -w '%{http_code}' -H "If-None-Match: $artifact_etag" "$ORIGIN$artifact_path")
contains "artifact ETag is id-based and conditionally returns 304" "$artifact_etag $artifact_conditional_status" 'W/"quran-artifact:tanzil/arabic/quran-uthmani.sqlite" 304'
contains "HTML page (/app) is no-cache"      "$(curl -sSI "$ORIGIN/app")" 'no-cache'
contains "/en/app/al-kahf/__data.json is no-cache" "$(curl -sSI "$ORIGIN/en/app/al-kahf/__data.json")" 'no-cache'

app_headers=$(curl -sSI "$ORIGIN/en/app")
contains "prerendered /en/app carries CSP (server.ts outer pass)" "$app_headers" 'content-security-policy: default-src'
contains "prerendered /en/app CSP authorizes inline scripts by hash" "$app_headers" 'sha256-'
contains "prerendered /en/app carries HSTS" "$app_headers" 'strict-transport-security:'
contains "prerendered /en/app carries nosniff" "$app_headers" 'x-content-type-options: nosniff'
headers_file=$(curl -sS "$ORIGIN/_headers" || true)
static_hash=$(grep -oE "sha256-[A-Za-z0-9+/=]+" <<<"$headers_file" | head -n1 || true)
if [[ -n "$static_hash" ]]; then
  contains "web/static/_headers theme-script hash matches the served /app CSP" "$app_headers" "$static_hash"
else
  fail_ "live /_headers file has no sha256 script hash — regenerate it (see web/static/_headers comment)"
fi

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
	pack=$(curl -sS "$ORIGIN/offline/manifest.json" | grep -oE '/offline/pack\.[A-Za-z0-9_-]+\.json' | head -n1 || true)
	if [[ -n "$pack" ]]; then
		contains "pack $pack is immutable" "$(curl -sSI "$ORIGIN$pack")" 'immutable'
	else
		warn "manifest served but no pack path found"
	fi
else
	warn "/offline/manifest.json not served ($code) — skipping pack checks"
fi

if [[ -n "$API_ORIGIN" ]]; then
	printf '\nAsserting W2 ingress identity contract against %s\n' "$API_ORIGIN"

	# A verified external caller (CF-Connecting-IP set, as Cloudflare does).
	cf=(-H "cf-connecting-ip: $CF_IP")

	# External Quran reads require the verified header (content bucket).
	quran_range=$(curl -sS "${cf[@]}" "$API_ORIGIN/quran/sources/en.sahih/range?from=1&to=7")
	contains "external Quran read succeeds with CF-Connecting-IP" "$quran_range" '"ayahs"'
	# Second read exercises translation-pool hit path before sampling health.
	curl -sS "${cf[@]}" -o /dev/null "$API_ORIGIN/quran/sources/en.sahih/range?from=1&to=7"
	quran_health=$(curl -sS "$API_ORIGIN/quran/health/ready")
	contains "Quran readiness exports Arabic resident bytes" "$quran_health" '"arabicResidentBytes"'
	contains "Quran readiness exports translation-pool metrics" "$quran_health" '"translationPool"'
	contains "Quran readiness exports eviction rate" "$quran_health" '"evictionsPerMinute"'

	printf '\n  -- negative identity checks --\n'
	# An external request WITHOUT CF-Connecting-IP is rejected at origin (never
	# collapsed into the 'unknown' bucket).
	no_cf_code=$(curl -sS -o /dev/null -w '%{http_code}' "$API_ORIGIN/quran/sources/en.sahih/range?from=1&to=7" || true)
	code_is "external request without CF-Connecting-IP rejected" "$no_cf_code" "400"

	# A public caller cannot buy internal treatment by sending a forged token
	# without a valid CF identity either.
	forged_code=$(curl -sS -o /dev/null -w '%{http_code}' \
		-H 'x-easyquran-internal-token: attacker-forged' \
		"$API_ORIGIN/quran/sources/en.sahih/range?from=1&to=7" || true)
	code_is "forged internal token without CF identity rejected" "$forged_code" "400"

	printf '\n  -- health isolation checks --\n'
	# Docker/localhost healthchecks carry no CF header and no token; /healthz and
	# /quran/health/ready stay 200 (exempt from identity + route blocker).
	healthz_code=$(curl -sS -o /dev/null -w '%{http_code}' "$API_ORIGIN/healthz" || true)
	code_is "/healthz exempt from identity gate (Docker health)" "$healthz_code" "200"
	ready_code=$(curl -sS -o /dev/null -w '%{http_code}' "$API_ORIGIN/quran/health/ready" || true)
	code_is "/quran/health/ready exempt from identity gate (isolated bucket)" "$ready_code" "200"

	if [[ -n "$INTERNAL_TOKEN" ]]; then
		printf '\n  -- internal SSR bucket checks --\n'
		# Valid token → trusted internal SSR (service bucket), no CF header needed.
		internal_code=$(curl -sS -o /dev/null -w '%{http_code}' \
			-H "x-easyquran-internal-token: $INTERNAL_TOKEN" \
			"$API_ORIGIN/quran/sources/en.sahih/range?from=1&to=7" || true)
		code_is "valid internal token enters service bucket (200)" "$internal_code" "200"
		# Invalid token (wrong value) never grants internal treatment → rejected
		# without a CF identity.
		invalid_code=$(curl -sS -o /dev/null -w '%{http_code}' \
			-H "x-easyquran-internal-token: not-the-real-token" \
			"$API_ORIGIN/quran/sources/en.sahih/range?from=1&to=7" || true)
		code_is "invalid internal token gets no privilege" "$invalid_code" "400"
	else
		warn "INTERNAL_QURAN_API_TOKEN unset — skipping internal-SSR bucket checks"
	fi

	printf '\n  -- CF-range reminder --\n'
	warn "CfConnectingIp trusts the header; owner MUST restrict origin ingress to Cloudflare ranges (deploy/README.md) and run the live negative test from a non-CF origin."
fi

if [[ "$fail" -ne 0 ]]; then
	printf '\nassert-headers: one or more hard checks FAILED\n' >&2
	exit 1
fi
printf '\nassert-headers: all hard checks passed\n'
