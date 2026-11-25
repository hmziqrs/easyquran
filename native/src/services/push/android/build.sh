#!/bin/sh
# Builds fcm.jar (`[package.metadata.nsh.features.fcm.android]` in
# native/Cargo.toml) after a change to FcmService.java. Needs an Android SDK, a
# JDK and the libraries nsh-maven vendored into native/android/maven:
#   JAVA_HOME=... ANDROID_HOME=... sh build.sh
# MAVEN=<dir> takes the .aar/.jar files from another directory.
set -eu
cd "$(dirname "$0")"
maven=${MAVEN:-../../../../android/maven}
out=$(mktemp -d)
classpath="$ANDROID_HOME/platforms/android-36/android.jar"
for aar in $(find "$maven" -name '*.aar'); do
  jar="$out/$(basename "$aar" .aar).jar"
  unzip -qp "$aar" classes.jar > "$jar" 2>/dev/null && classpath="$classpath:$jar"
done
for jar in $(find "$maven" -name '*.jar'); do
  classpath="$classpath:$jar"
done
mkdir "$out/classes"
"$JAVA_HOME/bin/javac" -Xlint:-options --release 8 -classpath "$classpath" \
  -d "$out/classes" fyi/easyquran/push/FcmService.java
"$JAVA_HOME/bin/jar" --create --date=2026-01-01T00:00:00Z --file fcm.jar --no-manifest -C "$out/classes" fyi
rm -r "$out"
