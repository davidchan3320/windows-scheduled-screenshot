#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ARCH="$(uname -m)"
OUTPUT="$ROOT/artifacts"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --arch) ARCH="$2"; shift 2 ;;
    --output) OUTPUT="$2"; shift 2 ;;
    *) echo "Usage: $0 [--arch arm64|x86_64|universal] [--output directory]" >&2; exit 2 ;;
  esac
done
case "$ARCH" in
  arm64|x86_64) ARCHES=("$ARCH") ;;
  universal) ARCHES=(arm64 x86_64) ;;
  *) echo "Unsupported architecture: $ARCH" >&2; exit 2 ;;
esac

SDK="$(xcrun --sdk macosx --show-sdk-path)"
SWIFTC="$(xcrun --find swiftc)"
DEST="$OUTPUT/ScheduledScreenshot-macos-$ARCH"
APP="$DEST/Scheduled Screenshot.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$OUTPUT/swift-cache"
cp "$ROOT/src/ScheduledScreenshot.Mac/Info.plist" "$APP/Contents/Info.plist"
cp "$ROOT/src/ScheduledScreenshot/config-editor.html" "$APP/Contents/Resources/"
cp "$ROOT/src/ScheduledScreenshot.Mac/settings.example.json" "$APP/Contents/Resources/"
BINARIES=()
for TARGET_ARCH in "${ARCHES[@]}"; do
  BINARY="$DEST/ScheduledScreenshot-$TARGET_ARCH"
  "$SWIFTC" -swift-version 5 -O -sdk "$SDK" -target "$TARGET_ARCH-apple-macos14.0" \
    -module-cache-path "$OUTPUT/swift-cache" \
    -framework AppKit -framework ScreenCaptureKit -framework ImageIO \
    "$ROOT"/src/ScheduledScreenshot.Mac/*.swift -o "$BINARY"
  BINARIES+=("$BINARY")
done
if [[ "$ARCH" == universal ]]; then
  lipo -create "${BINARIES[@]}" -output "$APP/Contents/MacOS/ScheduledScreenshot"
else
  cp "${BINARIES[0]}" "$APP/Contents/MacOS/ScheduledScreenshot"
fi
codesign --force --sign - --identifier com.scheduledscreenshot.app "$APP"
plutil -lint "$APP/Contents/Info.plist"
codesign --verify --strict "$APP"
ditto -c -k --keepParent "$APP" "$OUTPUT/ScheduledScreenshot-macos-$ARCH.zip"
(cd "$OUTPUT" && shasum -a 256 "ScheduledScreenshot-macos-$ARCH.zip" > "ScheduledScreenshot-macos-$ARCH.sha256")
echo "Built $APP"
