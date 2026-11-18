#!/usr/bin/env bash
# The remote screen package's signed delivery: throwaway keys generated into
# .nsh/keys (git-ignored, never committed), a real publish and verify, then a
# revocation that `nsh verify` must refuse. Writes the delivery facts the
# Rust status page reads to .nsh/delivery/state.json and logs everything to
# docs/evidence/remote-screen/delivery.log.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
cd "$here/.."

keys=".nsh/keys"
delivery=".nsh/delivery"
out=".nsh/out"
evidence="../../docs/evidence/remote-screen"
mkdir -p "$keys" "$delivery" "$out" "$evidence"
log="$evidence/delivery.log"
: > "$log"

step() { printf '\n== %s\n' "$1" | tee -a "$log"; }
run() { echo "\$ $*" | tee -a "$log"; "$@" 2>&1 | tee -a "$log"; }

step "$(nsh --version)"

step "Generate throwaway keys (kept out of git under .nsh/keys)"
if [ -f "$keys/root.secret" ]; then
  echo "root key exists; keys are never overwritten" | tee -a "$log"
else
  run nsh keygen "$keys/root"
fi
if [ -f "$keys/release.secret" ]; then
  echo "release key exists; keys are never overwritten" | tee -a "$log"
else
  run nsh keygen "$keys/release"
fi

step "Certify the release key for this app family, 90 days"
run nsh certify --root "$keys/root.secret" --key "$keys/release.pub" \
  --apps 'com.example.nshstarter.*' --days 90 -o "$keys/release.cert"
release_id="$(sed -n 's/.*: key \([0-9a-f]*\) certified by root [0-9a-f]* for .* until .*/\1/p' "$log" | tail -1)"
root_id="$(sed -n 's/.*: key [0-9a-f]* certified by root \([0-9a-f]*\) for .* until .*/\1/p' "$log" | tail -1)"
cert_apps="$(sed -n 's/.* certified by root [0-9a-f]* for \(.*\) until .*/\1/p' "$log" | tail -1)"

step "Publish the package (signed with the release key)"
pkg="$out/com.example.nshstarter.remote-1.nshpkg"
run nsh publish App.tsx --key "$keys/release.secret" --cert "$keys/release.cert" -o "$pkg"

step "Verify against the root (as a host would)"
verify_out="$(run nsh verify "$pkg" --root "$keys/root.pub")"
sha="$(printf '%s' "$verify_out" | sed -n 's/.*sha256 \([0-9a-f]\{64\}\).*/\1/p' | tail -1)"
[ -n "$sha" ] || { echo "could not read the package sha256" | tee -a "$log"; exit 1; }

step "Revoke this package: sequence 1, or the next one over the last list"
if [ -f "$delivery/revocations.json" ]; then
  seq="$(($(sed -n 's/.*"sequence": \([0-9]*\).*/\1/p' "$delivery/state.json" | tail -1) + 1))"
  run nsh revoke --root "$keys/root.secret" --extend "$delivery/revocations.json" \
    --sequence "$seq" --package "$sha" -o "$delivery/revocations.json"
else
  seq=1
  run nsh revoke --root "$keys/root.secret" --sequence "$seq" --package "$sha" \
    -o "$delivery/revocations.json"
fi

step "Verify again with the revocation list applied: it must refuse"
if nsh verify "$pkg" --root "$keys/root.pub" --revocations "$delivery/revocations.json" \
  2>&1 | tee -a "$log"; then
  echo "the revocation did not refuse the package" | tee -a "$log"
  exit 1
else
  echo "(refused, as it must be)" | tee -a "$log"
fi

step "Write the delivery record the Rust status page reads"
bytes="$(wc -c < "$pkg" | tr -d ' ')"
signed_by="$(sed -n 's/.*signed by \([0-9a-f]*\).*/\1/p' "$log" | tail -1)"
min_runtime="$(sed -n 's/.*for runtime \([^ ]*\) or newer.*/\1/p' "$log" | tail -1)"
verify_error="$(sed -n 's/^error .*/&/p' "$log" | tail -1)"
cat > "$delivery/state.json" <<JSON
{
  "package": {
    "id": "com.example.nshstarter.remote",
    "name": "Remote screen",
    "version": "1.0.0",
    "build": 1,
    "sha256": "$sha",
    "bytes": $bytes,
    "path": "packages/remote-screen/.nsh/out/com.example.nshstarter.remote-1.nshpkg"
  },
  "signature": {
    "release_key_id": "${release_id:-$signed_by}",
    "root_key_id": "$root_id",
    "certified_apps": "$cert_apps",
    "cert_days": 90,
    "min_runtime": "$min_runtime"
  },
  "revocation": {
    "sequence": $seq,
    "package_revoked": true,
    "verify_error": "$verify_error"
  }
}
JSON
cat "$delivery/state.json" | tee -a "$log"
echo "delivery complete" | tee -a "$log"
