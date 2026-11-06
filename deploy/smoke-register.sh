#!/usr/bin/env bash
# Registration admission smoke probe (A9). Run against the deployed API before
# promoting a release (see deploy/README.md, "Registration admission smoke test").
#
# What it proves without creating an account:
#   - CSRF bootstrap works (anonymous session + token).
#   - The password register door reaches the Turnstile gate. Turnstile runs
#     before any DB write, so a request with an empty token must fail with
#     400 "Captcha verification required" when TURNSTILE_SECRET_KEY is set.
#   - A 503 means the secret is missing or siteverify is unreachable: the
#     deploy must stop. A 2xx means the gate is off: the deploy must stop.
#
# It does NOT prove end-to-end registration with a real Turnstile token. That
# step is manual (deploy/README.md).
#
# Usage: deploy/smoke-register.sh https://api.example.com
# Exit 0 = gate configured and reachable. Non-zero = abort the release.

set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <api-base-url>" >&2
  exit 2
fi

base="${1%/}"
jar="$(mktemp)"
body="$(mktemp)"
trap 'rm -f "$jar" "$body"' EXIT

# 1. CSRF bootstrap: creates the anonymous session and returns the token.
csrf_status="$(curl -sS -o "$body" -w '%{http_code}' -c "$jar" -b "$jar" \
  -X POST "$base/csrf/v1/generate")"
if [ "$csrf_status" != "200" ]; then
  echo "FAIL csrf bootstrap returned HTTP $csrf_status" >&2
  exit 1
fi
token="$(sed -n 's/.*"token"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$body")"
if [ -z "$token" ]; then
  echo "FAIL csrf bootstrap returned no token" >&2
  exit 1
fi

# 2. Register probe with an empty Turnstile token. Nothing is stored: the
#    captcha check runs before the user insert.
probe_status="$(curl -sS -o "$body" -w '%{http_code}' -c "$jar" -b "$jar" \
  -H 'content-type: application/json' -H "csrf-token: $token" \
  -X POST "$base/auth/v1/register" \
  --data '{"name":"Smoke Probe","email":"smoke-probe@example.com","password":"smoke-probe-password-not-used","turnstile_token":""}')"

case "$probe_status" in
  400)
    if grep -q "Captcha verification required" "$body"; then
      echo "PASS register gate configured: Turnstile required and reachable path (HTTP 400, no account created)"
      exit 0
    fi
    echo "FAIL register returned 400 without the captcha message: $(head -c 300 "$body")" >&2
    exit 1
    ;;
  503)
    echo "FAIL register returned 503: TURNSTILE_SECRET_KEY missing in production, or siteverify unreachable" >&2
    exit 1
    ;;
  2[0-9][0-9])
    echo "FAIL register accepted an empty Turnstile token (HTTP $probe_status): the gate is off" >&2
    exit 1
    ;;
  *)
    echo "FAIL register returned unexpected HTTP $probe_status: $(head -c 300 "$body")" >&2
    exit 1
    ;;
esac
