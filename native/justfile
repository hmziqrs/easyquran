# Development commands. `just` with no argument lists them.
#
# Gates (no check may be weakened to get a pass):
#   Rust  just check && just test
#   TSX   just test-tsx            (packages/remote-screen)
#   All   just smoke

default:
    @just --list

# Rust gate: formatting check and clippy with warnings denied.
check:
    cargo fmt --all -- --check
    cargo clippy --all-targets -- -D warnings

# Rust unit and integration tests (also run by `nsh test` before its TSX tests).
test:
    cargo test

# TSX package gate in packages/remote-screen: type check, a build with no
# warnings, headless tests. Needs the node_modules/{nsh-query,nsh-form}
# symlinks that point at the sibling library checkouts.
test-tsx:
    cd packages/remote-screen && nsh check && nsh build && nsh test

# Headless smoke gate: the root's App test plus the per-page smoke suite,
# and the remote-screen package gates. vision/ is excluded on purpose: its
# tests drive screenshots, which need a real target (`--target`) and a
# reviewing model, so they fail a headless run by design.
smoke:
    nsh test App.test.tsx
    nsh test smoke/pages.test.tsx
    just test-tsx

# Desktop run in the AppKit development host, with hot reload.
run-macos:
    nsh run --target macos

# iOS simulator run. Uses a booted simulator, else the newest iPhone.
run-ios:
    nsh run --target ios

# Android run on the first device `adb` lists. Uses Android Studio's JDK when
# JAVA_HOME is unset: /usr/bin/java is a stub with no runtime behind it.
run-android:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -z "${JAVA_HOME:-}" ] && [ -d "/Applications/Android Studio.app/Contents/jbr/Contents/Home" ]; then
        export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
    fi
    nsh run --target android

# Web run in the browser (the dev server serves the page).
run-web:
    nsh run --target web

# Package all four shipped targets into .nsh/out: macOS .app, iOS simulator
# .app, Android .apk, and the web static site.
package:
    nsh package --target macos
    nsh package --target ios-simulator
    nsh package --target android
    nsh package --target web

# Rename the app everywhere it lives, or prove the rename with a roundtrip:
#   just rename <new-name> <new-id>
#   just rename --roundtrip-test
rename *args='':
    scripts/rename.sh {{args}}

# Print docs/STATUS.md: what is done, how it was verified, what is unverified.
status:
    cat docs/STATUS.md
