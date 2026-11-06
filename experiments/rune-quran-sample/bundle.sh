#!/bin/sh
# Builds target/RuneQuran.app (macOS). Rune has no packager of its own yet.
set -e
cd "$(dirname "$0")"
cargo build --release
app=target/RuneQuran.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp target/release/rune-quran-sample "$app/Contents/MacOS/"
cat > "$app/Contents/Info.plist" <<'P'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>rune-quran-sample</string>
<key>CFBundleIdentifier</key><string>com.example.runequransample</string>
<key>CFBundleExecutable</key><string>rune-quran-sample</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
P
codesign --force --sign - "$app" >/dev/null 2>&1
echo "$app"
