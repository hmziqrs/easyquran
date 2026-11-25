#!/bin/sh
# Builds reminders.jar (nsh.json `android.libraries`; `nsh package` runs d8 on
# it) from java/. Needs a JDK (JAVA_HOME) and an Android SDK platform.
set -eu
cd "$(dirname "$0")"
sdk="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
platform="${ANDROID_PLATFORM:-android-36}"
jdk="${JAVA_HOME:-/Applications/Android Studio.app/Contents/jbr/Contents/Home}"
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
"$jdk/bin/javac" -Xlint:all,-options -source 8 -target 8 \
  -bootclasspath "$sdk/platforms/$platform/android.jar" -d "$out" \
  java/fyi/easyquran/reminders/*.java
# One date for every entry: the same sources make the same jar.
"$jdk/bin/jar" --create --no-manifest --date=2020-01-01T00:00:00Z \
  --file reminders.jar -C "$out" fyi
