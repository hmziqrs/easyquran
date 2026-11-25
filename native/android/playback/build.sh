#!/bin/sh
# Builds playback.jar (`[package.metadata.nsh.android]` in native/Cargo.toml)
# after a change to Playback.java. Needs an Android SDK and a JDK:
#   JAVA_HOME=... ANDROID_HOME=... sh build.sh
set -eu
cd "$(dirname "$0")"
out=$(mktemp -d)
"$JAVA_HOME/bin/javac" -Xlint:-options --release 8 \
  -classpath "$ANDROID_HOME/platforms/android-36/android.jar" \
  -d "$out" java/fyi/easyquran/media/Playback.java
"$JAVA_HOME/bin/jar" --create --date=2026-01-01T00:00:00Z --file playback.jar --no-manifest -C "$out" fyi
rm -r "$out"
