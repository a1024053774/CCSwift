#!/usr/bin/env bash
# Build "CCSwift.app", a DMG and release archives for this Mac's architecture.
#
#   scripts/bundle-macos.sh                                        # ad-hoc signed, in dist/
#   CODESIGN_IDENTITY="Developer ID Application: …" scripts/bundle-macos.sh
#   CODESIGN_IDENTITY="Developer ID Application: …" NOTARIZE=1 \
#     APPLE_API_KEY=/path/AuthKey_<id>.p8 APPLE_API_KEY_ID=<id> APPLE_API_ISSUER=<issuer> \
#     scripts/bundle-macos.sh                                      # signed, notarized, stapled
set -euo pipefail
cd "$(dirname "$0")/.."

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
case "$(uname -m)" in
  x86_64 | amd64) arch=x64 ;;
  arm64 | aarch64) arch=arm64 ;;
  *) arch="$(uname -m)" ;;
esac
out="${CC_SAME_OUTPUT:-dist}"
identity="${CODESIGN_IDENTITY:--}"
bundle_id="io.github.a1024053774.ccswift"
mkdir -p "$out"

if [[ "${NOTARIZE:-0}" == "1" ]]; then
  [[ "$identity" != "-" ]] || { echo "NOTARIZE=1 needs CODESIGN_IDENTITY (a Developer ID Application)" >&2; exit 2; }
  for variable in APPLE_API_KEY APPLE_API_KEY_ID APPLE_API_ISSUER; do
    [[ -n "${!variable:-}" ]] || { echo "$variable is required when NOTARIZE=1" >&2; exit 2; }
  done
fi

cargo build --release -p cc-same
cargo build --release --manifest-path crates/app/Cargo.toml

app="$out/CCSwift.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/cc-same-app "$app/Contents/MacOS/CCSwift"
# AppIcon.icon (Icon Composer) has light, dark and tinted art; actool (Xcode 26+) compiles it into
# Assets.car for macOS 26+ and AppIcon.icns for older systems.
icon_plist="$(mktemp "${TMPDIR:-/tmp}/ccswift-icon.XXXXXX")"
xcrun actool crates/app/resources/macos/AppIcon.icon --compile "$app/Contents/Resources" --platform macosx \
  --minimum-deployment-target 11.0 --app-icon AppIcon --output-partial-info-plist "$icon_plist" >/dev/null
rm -f -- "$icon_plist"
test -f "$app/Contents/Resources/Assets.car" && test -f "$app/Contents/Resources/AppIcon.icns"
sed "s/__VERSION__/$version/g" crates/app/resources/macos/Info.plist > "$app/Contents/Info.plist"
xattr -cr "$app"

# sign <path> <identifier> [entitlements]
sign() {
  if [[ "$identity" == "-" ]]; then
    codesign --force --identifier "$2" --sign - "$1"
  else
    codesign --force --identifier "$2" --options runtime --timestamp ${3:+--entitlements "$3"} --sign "$identity" "$1"
  fi
}

notarize() {
  xcrun notarytool submit "$1" --key "$APPLE_API_KEY" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER" --wait
}

sign "$app/Contents/MacOS/CCSwift" "$bundle_id"
sign "$app" "$bundle_id" crates/app/resources/macos/entitlements.plist
codesign --verify --deep --strict --verbose=2 "$app"

cli="$(mktemp -d "${TMPDIR:-/tmp}/cc-same-cli.XXXXXX")"
stage="$(mktemp -d "${TMPDIR:-/tmp}/cc-same-dmg.XXXXXX")"
trap 'rm -rf -- "$cli" "$stage"' EXIT
cp target/release/cc-same "$cli/cc-same"
sign "$cli/cc-same" "$bundle_id.cli"

zip="$out/CCSwift-$version-macos-$arch.zip"
make_zip() {
  rm -f "$zip"
  COPYFILE_DISABLE=1 ditto -c -k --norsrc --noextattr --keepParent "$app" "$zip"
}
make_zip

if [[ "${NOTARIZE:-0}" == "1" ]]; then
  notarize "$zip"
  xcrun stapler staple "$app"
  xcrun stapler validate "$app"
  spctl --assess --type execute --verbose=2 "$app"
  make_zip
  # A bare executable cannot carry a ticket; notarizing it registers it with Gatekeeper.
  ditto -c -k --keepParent "$cli/cc-same" "$cli/notarize.zip"
  notarize "$cli/notarize.zip"
fi

dmg="$out/CCSwift-$version-macos-$arch.dmg"
rm -f "$dmg"
COPYFILE_DISABLE=1 ditto --norsrc --noextattr "$app" "$stage/CCSwift.app"
ln -s /Applications "$stage/Applications"
hdiutil create -quiet -ov -format ULFO -fs HFS+ -volname "CCSwift $version" -srcfolder "$stage" "$dmg"
if [[ "$identity" != "-" ]]; then
  codesign --force --timestamp --sign "$identity" "$dmg"
fi
if [[ "${NOTARIZE:-0}" == "1" ]]; then
  notarize "$dmg"
  xcrun stapler staple "$dmg"
  xcrun stapler validate "$dmg"
fi

cli_archive="$out/CCSwift-$version-cli-macos-$arch.tar.gz"
tar -C "$cli" -czf "$cli_archive" cc-same

printf '%s\n' "$zip" "$dmg" "$cli_archive"
