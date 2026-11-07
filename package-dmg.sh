#!/bin/bash
# Packages open-typeless as "Open Dictate.app" inside a distributable .dmg.
#
# The Info.plist below is copied verbatim from the macos_bundle module in
# src/main.rs, which is the project's own authored bundle recipe. The
# microphone usage string is required: macOS only grants mic access to a
# signed app bundle, and a bare binary just records silence.
set -euo pipefail

cd "$(dirname "$0")"

APP_NAME="Open Dictate"
BIN_IN="open-typeless"          # cargo package name
BIN_OUT="open-dictate"          # must match CFBundleExecutable
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
ARCH="$(uname -m)"

STAGE="target/dmg-stage"
BUNDLE="$STAGE/$APP_NAME.app"
DIST="../dist"
DMG="$DIST/Open-Dictate-$VERSION-$ARCH.dmg"

echo "==> Building release binary"
cargo build --release

echo "==> Assembling $APP_NAME.app"
rm -rf "$STAGE"
mkdir -p "$BUNDLE/Contents/MacOS" "$BUNDLE/Contents/Resources"
cp "target/release/$BIN_IN" "$BUNDLE/Contents/MacOS/$BIN_OUT"
chmod +x "$BUNDLE/Contents/MacOS/$BIN_OUT"

printf 'APPL????' > "$BUNDLE/Contents/PkgInfo"

cat > "$BUNDLE/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key><string>$BIN_OUT</string>
    <key>CFBundleIdentifier</key><string>app.opendictate.desktop</string>
    <key>CFBundleName</key><string>$APP_NAME</string>
    <key>CFBundleDisplayName</key><string>$APP_NAME</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSMicrophoneUsageDescription</key><string>Open Dictate listens while you hold or press your dictation shortcut, and sends the recording only to the speech model you chose.</string>
    <key>NSAppleEventsUsageDescription</key><string>Open Dictate pastes your dictated text into the app you are using.</string>
</dict>
</plist>
PLIST

echo "==> Ad-hoc signing (ties mic permission to the bundle)"
codesign --force --deep --sign - "$BUNDLE"
codesign --verify --verbose=1 "$BUNDLE"

echo "==> Building DMG"
ln -s /Applications "$STAGE/Applications"
mkdir -p "$DIST"
rm -f "$DMG"
hdiutil create \
    -volname "$APP_NAME" \
    -srcfolder "$STAGE" \
    -ov -format UDZO \
    "$DMG" >/dev/null

echo
echo "DMG:  $(cd "$DIST" && pwd)/$(basename "$DMG")"
echo "Size: $(du -h "$DMG" | cut -f1)"
