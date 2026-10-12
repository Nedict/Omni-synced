#!/usr/bin/env bash
# Build and package SolidCraft for macOS:
#
#   $DIST/solvecraft-<version>-macos-<arch>.dmg        SolidCraft.app on a drag-to-Applications DMG
#   $DIST/solvecraft-cli-<version>-macos-<arch>.zip    the headless CLI with the licence files
#
# Usage: packaging/macos/package.sh [--arch universal|aarch64|x86_64] [--skip-build]
#
# The app is ad-hoc signed (not notarized): the first launch needs right-click > Open.
# Adapted from PhotoCraft's packaging (storytold/photocraft, Apache-2.0; see ATTRIBUTION.md).
set -euo pipefail
# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
HERE="$ROOT/packaging/macos"

ARCH=universal
SKIP_BUILD=0
while [ $# -gt 0 ]; do
  case "$1" in
    --arch) ARCH="$2"; shift 2 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    -h | --help) sed -n '2,10p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
case "$ARCH" in
  universal) TARGETS=(aarch64-apple-darwin x86_64-apple-darwin) ;;
  aarch64) TARGETS=(aarch64-apple-darwin) ;;
  x86_64) TARGETS=(x86_64-apple-darwin) ;;
  *) echo "unknown --arch $ARCH" >&2; exit 2 ;;
esac

# Keep in sync with LSMinimumSystemVersion in Info.plist.in.
export MACOSX_DEPLOYMENT_TARGET=11.0
SHORT_VERSION="${VERSION%%-*}"
WORK="$CARGO_TARGET_DIR/macos-package"
APP="$WORK/SolidCraft.app"
DMG="$DIST/solvecraft-$VERSION-macos-$ARCH.dmg"
CLI_ZIP="$DIST/solvecraft-cli-$VERSION-macos-$ARCH.zip"

echo "==> SolidCraft $VERSION for macOS ($ARCH)"
if [ "$SKIP_BUILD" = 0 ]; then
  args=()
  for t in "${TARGETS[@]}"; do args+=(--target "$t"); done
  (cd "$ROOT" && cargo build --release --locked -p solvecraft -p solvecraft-cli "${args[@]}")
fi

rm -rf "$WORK"
mkdir -p "$WORK/bin"
for bin in solvecraft solvecraft-cli; do
  inputs=()
  for t in "${TARGETS[@]}"; do inputs+=("$CARGO_TARGET_DIR/$t/release/$bin"); done
  lipo -create -output "$WORK/bin/$bin" "${inputs[@]}"
  lipo -info "$WORK/bin/$bin"
done

# ---- SolidCraft.app ----------------------------------------------------------------------------
echo "==> assembling $APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$WORK/bin/solvecraft" "$APP/Contents/MacOS/SolidCraft"
# The icon, drawn by the CLI (no image files are committed).
SET="$WORK/SolidCraft.iconset"
mkdir -p "$SET"
for s in 16 32 128 256 512; do
  "$WORK/bin/solvecraft-cli" icon --size "$s" --out "$SET/icon_${s}x${s}.png" >/dev/null
  "$WORK/bin/solvecraft-cli" icon --size $((s * 2)) --out "$SET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns -o "$APP/Contents/Resources/SolidCraft.icns" "$SET"
mkdir -p "$APP/Contents/Resources/Licenses"
copy_docs "$APP/Contents/Resources/Licenses"
sed -e "s/@VERSION@/$VERSION/g" -e "s/@SHORT_VERSION@/$SHORT_VERSION/g" "$HERE/Info.plist.in" >"$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist"
printf 'APPL????' >"$APP/Contents/PkgInfo"
# Ad-hoc signature over the whole bundle (Apple silicon refuses unsigned code).
codesign --force --sign - --timestamp=none "$APP"
codesign --verify --strict --deep --verbose=2 "$APP"

# ---- DMG ---------------------------------------------------------------------------------------
echo "==> building $DMG"
STAGE="$WORK/dmg"
mkdir -p "$STAGE"
ditto "$APP" "$STAGE/SolidCraft.app"
ln -s /Applications "$STAGE/Applications"
cp "$HERE/README.txt" "$STAGE/README.txt"
rm -f "$DMG" "$WORK/raw.dmg"
# makehybrid + convert builds the image without attaching a device, unlike `create -srcfolder`,
# which is flaky on CI runners ("Resource busy").
hdiutil makehybrid -hfs -hfs-volume-name "SolidCraft" -hfs-openfolder "$STAGE" -o "$WORK/raw.dmg" "$STAGE"
hdiutil convert "$WORK/raw.dmg" -format UDZO -imagekey zlib-level=9 -o "$DMG"
rm -f "$WORK/raw.dmg"

# Check the image as a user gets it: mount it and look inside.
MNT="$(mktemp -d)"
hdiutil attach -readonly -nobrowse -mountpoint "$MNT" "$DMG"
ls -la "$MNT" "$MNT/SolidCraft.app/Contents" "$MNT/SolidCraft.app/Contents/MacOS" "$MNT/SolidCraft.app/Contents/Resources"
test -L "$MNT/Applications" && test "$(readlink "$MNT/Applications")" = /Applications
plutil -lint "$MNT/SolidCraft.app/Contents/Info.plist"
# Check the names Finder/LaunchServices actually use, as well as compatibility identity.
for key in CFBundleName CFBundleDisplayName CFBundleExecutable CFBundleIconFile; do
  test "$(/usr/libexec/PlistBuddy -c "Print :$key" "$MNT/SolidCraft.app/Contents/Info.plist")" = SolidCraft
done
test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$MNT/SolidCraft.app/Contents/Info.plist")" = "$APP_ID"
test ! -e "$MNT/SolveCraft.app"
test -s "$MNT/SolidCraft.app/Contents/Resources/SolidCraft.icns"
cmp "$HERE/README.txt" "$MNT/README.txt"
case "$ARCH" in universal) WANT=(arm64 x86_64) ;; aarch64) WANT=(arm64) ;; *) WANT=(x86_64) ;; esac
for arch in "${WANT[@]}"; do
  lipo "$MNT/SolidCraft.app/Contents/MacOS/SolidCraft" -verify_arch "$arch"
done
test -s "$MNT/SolidCraft.app/Contents/Resources/Licenses/THIRD-PARTY-LICENSES.txt"
# Not --strict: the mounted HFS volume adds Finder info, which strict verification rejects.
codesign --verify --deep --verbose=2 "$MNT/SolidCraft.app"
"$MNT/SolidCraft.app/Contents/MacOS/SolidCraft" --version
hdiutil detach "$MNT"

# ---- CLI ---------------------------------------------------------------------------------------
echo "==> building $CLI_ZIP"
CLI_DIR="$WORK/solvecraft-cli-$VERSION-macos-$ARCH"
mkdir -p "$CLI_DIR"
cp "$WORK/bin/solvecraft-cli" "$CLI_DIR/"
codesign --force --sign - --timestamp=none --identifier ai.storyteller.solvecraft-cli "$CLI_DIR/solvecraft-cli"
copy_docs "$CLI_DIR"
rm -f "$CLI_ZIP"
ditto -c -k --keepParent "$CLI_DIR" "$CLI_ZIP"

"$WORK/bin/solvecraft-cli" --version
echo "==> done"
ls -lh "$DMG" "$CLI_ZIP"
