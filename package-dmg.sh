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
cp assets/AppIcon.icns "$BUNDLE/Contents/Resources/AppIcon.icns"

cat > "$BUNDLE/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key><string>$BIN_OUT</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
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

# macOS keys Microphone and Accessibility grants to the app's signing identity.
# An ad-hoc signature is keyed to a hash of the binary, so every rebuild looked
# like a brand new app and every permission reset. A real certificate keeps the
# designated requirement stable, and the grants survive.
#
# Override with SIGN_ID=... to use another one. "Developer ID Application" is
# the one to pick if this DMG is ever given to someone else; it also needs
# notarizing for that.
SIGN_ID="${SIGN_ID:-Apple Development: Opeyemi Ajagbe (34598C886Y)}"
if ! security find-identity -v -p codesigning | grep -qF "$SIGN_ID"; then
    echo "!!  signing identity not found: $SIGN_ID"
    echo "!!  falling back to ad-hoc; permissions will reset on every build"
    SIGN_ID="-"
fi

echo "==> Signing as: $SIGN_ID"
codesign --force --deep --sign "$SIGN_ID" "$BUNDLE"
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

# Leaving the staging bundle behind gives Spotlight a second "Open Dictate"
# to offer, which is how you end up launching a stale build by accident.
rm -rf "$STAGE"

echo
echo "DMG:  $(cd "$DIST" && pwd)/$(basename "$DMG")"
echo "Size: $(du -h "$DMG" | cut -f1)"
