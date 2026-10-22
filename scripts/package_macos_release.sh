#!/usr/bin/env bash
# Package the signed, notarized, stapled universal macOS DMG for a release.
#
# Expects both cargo release builds to already exist. For the shipped
# gpui-shell app (BIN_NAME=bir-shell BIN_TARGET_ROOT=crates/bir-shell/target):
#   crates/bir-shell/target/aarch64-apple-darwin/release/bir-shell
#   crates/bir-shell/target/x86_64-apple-darwin/release/bir-shell
#
# Environment:
#   VERSION               required, must match ^[0-9]+\.[0-9]+\.[0-9]+$
#   BIN_NAME              cargo binary in <triple>/release to package
#                         (default "bir-desktop"; "bir-shell" for the shell)
#   BIN_TARGET_ROOT       directory holding <triple>/release
#                         (default "target"; "crates/bir-shell/target" for the
#                         shell, a standalone workspace with its own target dir)
#   RESOURCE_DIRS         repo-relative directories also staged into
#                         Contents/Resources next to assets/ — "app
#                         print-overrides" for the shell, empty for the
#                         painted app
#   BUILD_NUMBER          default: parsed from the justfile BUILD_NUMBER line
#   SIGNING_IDENTITY      default "-" (ad-hoc, local smoke tests only);
#                         otherwise a codesign identity name or SHA-1
#   KEYCHAIN_PATH         optional keychain passed to every codesign call
#   NOTARY_KEY_PATH       App Store Connect API .p8 key file (all three
#   NOTARY_KEY_ID         NOTARY_* must be set to enable notarization)
#   NOTARY_ISSUER_ID      App Store Connect issuer UUID
#   REQUIRE_NOTARIZATION  "1" fails before any work unless a real identity
#                         and all NOTARY_* inputs are configured
#   OUT_DIR               default target/release-artifacts

set -euo pipefail

fail() {
  echo "package_macos_release: $*" >&2
  exit 1
}

# ── Environment validation (before touching the filesystem) ─────────────────

VERSION="${VERSION:-}"
version_pattern='^[0-9]+\.[0-9]+\.[0-9]+$'
if [[ ! "$VERSION" =~ $version_pattern ]]; then
  fail "VERSION is required and must match $version_pattern (got: '${VERSION:-<unset>}')"
fi

APP_NAME="${APP_NAME:-eBIRForms}"
BUNDLE_ID="${BUNDLE_ID:-dev.goldcoders.bir}"
BIN_NAME="${BIN_NAME:-bir-desktop}"
BIN_TARGET_ROOT="${BIN_TARGET_ROOT:-target}"
RESOURCE_DIRS="${RESOURCE_DIRS:-}"
SIGNING_IDENTITY="${SIGNING_IDENTITY:--}"
NOTARY_KEY_PATH="${NOTARY_KEY_PATH:-}"
NOTARY_KEY_ID="${NOTARY_KEY_ID:-}"
NOTARY_ISSUER_ID="${NOTARY_ISSUER_ID:-}"

if [[ "${REQUIRE_NOTARIZATION:-0}" == "1" ]]; then
  if [[ "$SIGNING_IDENTITY" == "-" ]]; then
    fail "REQUIRE_NOTARIZATION=1 but SIGNING_IDENTITY is '-' (ad-hoc); a Developer ID identity is required"
  fi
  for required_var in NOTARY_KEY_PATH NOTARY_KEY_ID NOTARY_ISSUER_ID; do
    if [[ -z "${!required_var}" ]]; then
      fail "REQUIRE_NOTARIZATION=1 but $required_var is empty"
    fi
  done
fi

notary_vars_set=0
for notary_var in NOTARY_KEY_PATH NOTARY_KEY_ID NOTARY_ISSUER_ID; do
  if [[ -n "${!notary_var}" ]]; then
    notary_vars_set=$((notary_vars_set + 1))
  fi
done
if [[ "$notary_vars_set" -ne 0 && "$notary_vars_set" -ne 3 ]]; then
  fail "partial notarization config: set all of NOTARY_KEY_PATH, NOTARY_KEY_ID, NOTARY_ISSUER_ID or none"
fi
NOTARIZE=0
if [[ "$notary_vars_set" -eq 3 ]]; then
  NOTARIZE=1
fi

AD_HOC=0
if [[ "$SIGNING_IDENTITY" == "-" ]]; then
  AD_HOC=1
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

if [[ -z "${BUILD_NUMBER:-}" ]]; then
  BUILD_NUMBER="$(grep '^BUILD_NUMBER' justfile | grep -o '"[0-9]*"' | tr -d '"' || true)"
fi
if [[ -z "${BUILD_NUMBER:-}" ]]; then
  fail "BUILD_NUMBER is not set and could not be parsed from justfile"
fi

OUT_DIR="${OUT_DIR:-target/release-artifacts}"

# ── Preconditions ────────────────────────────────────────────────────────────

for triple in aarch64-apple-darwin x86_64-apple-darwin; do
  if [[ ! -f "$BIN_TARGET_ROOT/$triple/release/$BIN_NAME" ]]; then
    fail "missing $BIN_TARGET_ROOT/$triple/release/$BIN_NAME — run the release build for $triple"
  fi
done
for resource_dir in $RESOURCE_DIRS; do
  [[ -d "$resource_dir" ]] || fail "resource directory missing: $resource_dir"
done

keychain_args=()
if [[ -n "${KEYCHAIN_PATH:-}" ]]; then
  keychain_args+=(--keychain "$KEYCHAIN_PATH")
fi

mkdir -p "$OUT_DIR"

# notarize FILE — submit to the Notary service with the App Store Connect API
# key, wait up to an hour, and require an Accepted status.
notarize() {
  local file="$1"
  local result_json submission_id
  result_json="$(mktemp "${TMPDIR:-/tmp}/notary-result.XXXXXX")"
  if ! xcrun notarytool submit "$file" \
    --key "$NOTARY_KEY_PATH" \
    --key-id "$NOTARY_KEY_ID" \
    --issuer "$NOTARY_ISSUER_ID" \
    --wait --timeout 1h \
    --output-format json >"$result_json"; then
    submission_id="$(
      python3 -c 'import json, sys; print(json.load(open(sys.argv[1], encoding="utf-8")).get("id") or "")' \
        "$result_json" 2>/dev/null || true
    )"
    if [[ -n "$submission_id" ]]; then
      xcrun notarytool log "$submission_id" \
        --key "$NOTARY_KEY_PATH" \
        --key-id "$NOTARY_KEY_ID" \
        --issuer "$NOTARY_ISSUER_ID" >&2 || true
    fi
    rm -f "$result_json"
    fail "notarytool submit failed for $file"
  fi
  if python3 - "$result_json" <<'PYEOF'; then
import json
import sys

result = json.load(open(sys.argv[1], encoding="utf-8"))
ok = result.get("status") == "Accepted" and bool(result.get("id"))
sys.exit(0 if ok else f"notarytool did not accept submission: {result!r}")
PYEOF
    rm -f "$result_json"
    return 0
  fi
  submission_id="$(
    python3 -c 'import json, sys; print(json.load(open(sys.argv[1], encoding="utf-8")).get("id") or "")' \
      "$result_json" 2>/dev/null || true
  )"
  if [[ -n "$submission_id" ]]; then
    xcrun notarytool log "$submission_id" \
      --key "$NOTARY_KEY_PATH" \
      --key-id "$NOTARY_KEY_ID" \
      --issuer "$NOTARY_ISSUER_ID" >&2 || true
  fi
  rm -f "$result_json"
  fail "notarization rejected $file"
}

# ── a. Universal binary ──────────────────────────────────────────────────────

python3 scripts/alias_desktop_bin.py "$BIN_TARGET_ROOT/aarch64-apple-darwin/release" "$BIN_NAME"
python3 scripts/alias_desktop_bin.py "$BIN_TARGET_ROOT/x86_64-apple-darwin/release" "$BIN_NAME"
lipo -create \
  "$BIN_TARGET_ROOT/aarch64-apple-darwin/release/bir" \
  "$BIN_TARGET_ROOT/x86_64-apple-darwin/release/bir" \
  -output "$OUT_DIR/bir"
chmod +x "$OUT_DIR/bir"

UNIVERSAL_ARCHS="$(lipo -archs "$OUT_DIR/bir")"
for required_arch in arm64 x86_64; do
  case " $UNIVERSAL_ARCHS " in
    *" $required_arch "*) ;;
    *) fail "universal binary is missing $required_arch (lipo -archs: $UNIVERSAL_ARCHS)" ;;
  esac
done

# ── b. .app bundle ───────────────────────────────────────────────────────────

APP="$OUT_DIR/$APP_NAME.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$OUT_DIR/bir" "$APP/Contents/MacOS/"
cp -R assets "$APP/Contents/Resources/"
rm -rf "$APP/Contents/Resources/assets/macos"
cp assets/AppIcon.icns "$APP/Contents/Resources/"
# The shell's runtime resources: `app/` (the JavaScript UI) and
# `print-overrides/` resolve at Contents/Resources/<name> — see
# crates/bir-shell/src/packaged.rs.
for resource_dir in $RESOURCE_DIRS; do
  cp -R "$resource_dir" "$APP/Contents/Resources/"
done

cp assets/macos/Info.plist "$APP/Contents/Info.plist"
sed -i '' "s/VERSION_PLACEHOLDER/${VERSION}/g" "$APP/Contents/Info.plist"
sed -i '' "s/BUILD_NUMBER_PLACEHOLDER/${BUILD_NUMBER}/g" "$APP/Contents/Info.plist"
sed -i '' "s/BUNDLE_ID_PLACEHOLDER/${BUNDLE_ID}/g" "$APP/Contents/Info.plist"
sed -i '' "s/APP_NAME_PLACEHOLDER/${APP_NAME}/g" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c \
  "Add :LSApplicationCategoryType string public.app-category.finance" \
  "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist"
if grep -q "PLACEHOLDER" "$APP/Contents/Info.plist"; then
  fail "unsubstituted PLACEHOLDER remains in $APP/Contents/Info.plist"
fi

python3 scripts/audit_no_legacy.py --package-root "$APP"

if find "$APP/Contents/Resources" -type f -print0 |
  xargs -0 file | grep -q "Mach-O"; then
  find "$APP/Contents/Resources" -type f -print0 |
    xargs -0 file | grep "Mach-O" >&2 || true
  fail "Mach-O file under Contents/Resources — notarization rejects unsigned nested code"
fi

xattr -cr "$APP"

# ── c. Sign ──────────────────────────────────────────────────────────────────

if [[ "$AD_HOC" -eq 1 ]]; then
  codesign --force --options runtime --timestamp=none \
    --sign "$SIGNING_IDENTITY" \
    --entitlements assets/macos/entitlements.mac.plist \
    ${keychain_args[@]+"${keychain_args[@]}"} \
    "$APP"
else
  codesign --force --options runtime --timestamp \
    --sign "$SIGNING_IDENTITY" \
    --entitlements assets/macos/entitlements.mac.plist \
    ${keychain_args[@]+"${keychain_args[@]}"} \
    "$APP"
fi

codesign --verify --strict --deep --verbose=2 "$APP"

# ── d. Notarize and staple the app ───────────────────────────────────────────

if [[ "$NOTARIZE" -eq 1 ]]; then
  NOTARY_DIR="$(mktemp -d "${TMPDIR:-/tmp}/ebirforms-notary.XXXXXX")"
  trap 'rm -rf "$NOTARY_DIR"' EXIT
  NOTARY_ZIP="$NOTARY_DIR/$APP_NAME.zip"
  ditto -c -k --keepParent "$APP" "$NOTARY_ZIP"
  notarize "$NOTARY_ZIP"
  rm -rf "$NOTARY_DIR"
  trap - EXIT
  xcrun stapler staple "$APP"
  xcrun stapler validate "$APP"
  spctl --assess --type execute --verbose=2 "$APP"
fi

# ── e. DMG ───────────────────────────────────────────────────────────────────

DMG_NAME="$APP_NAME-macOS-universal-$VERSION.dmg"
DMG="$OUT_DIR/$DMG_NAME"
rm -f "$DMG"

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/ebirforms-dmg-stage.XXXXXX")"
ditto "$APP" "$STAGE/$APP_NAME.app"
ln -s /Applications "$STAGE/Applications"

dmg_created=0
for attempt in 1 2 3; do
  if hdiutil create -volname "$APP_NAME" \
    -srcfolder "$STAGE" \
    -ov -format UDZO -fs HFS+ \
    "$DMG"; then
    dmg_created=1
    break
  fi
  echo "hdiutil create attempt $attempt failed; retrying" >&2
  sleep 5
done
rm -rf "$STAGE"
if [[ "$dmg_created" -ne 1 ]]; then
  fail "hdiutil create failed after 3 attempts"
fi

if [[ "$AD_HOC" -eq 0 ]]; then
  codesign --force --timestamp \
    --sign "$SIGNING_IDENTITY" \
    ${keychain_args[@]+"${keychain_args[@]}"} \
    "$DMG"
fi

if [[ "$NOTARIZE" -eq 1 ]]; then
  notarize "$DMG"
  xcrun stapler staple "$DMG"
  xcrun stapler validate "$DMG"
  spctl --assess --type open --context context:primary-signature --verbose=2 "$DMG"
fi

# ── f. Verify the mounted DMG ────────────────────────────────────────────────

MOUNT_POINT="$(mktemp -d "${TMPDIR:-/tmp}/ebirforms-dmg-mount.XXXXXX")"
cleanup_mount() {
  hdiutil detach "$MOUNT_POINT" >/dev/null 2>&1 || true
  rm -rf "$MOUNT_POINT"
}
trap cleanup_mount EXIT

hdiutil attach "$DMG" -nobrowse -readonly -mountpoint "$MOUNT_POINT"
MOUNTED_APP="$MOUNT_POINT/$APP_NAME.app"
if [[ ! -d "$MOUNTED_APP" ]]; then
  fail "mounted DMG does not contain $APP_NAME.app"
fi
codesign --verify --strict --deep --verbose=2 "$MOUNTED_APP"
python3 scripts/audit_no_legacy.py --package-root "$MOUNTED_APP"

MOUNTED_ARCHS="$(lipo -archs "$MOUNTED_APP/Contents/MacOS/bir")"
for required_arch in arm64 x86_64; do
  case " $MOUNTED_ARCHS " in
    *" $required_arch "*) ;;
    *) fail "mounted binary is missing $required_arch (lipo -archs: $MOUNTED_ARCHS)" ;;
  esac
done

if [[ ! -L "$MOUNT_POINT/Applications" ]]; then
  fail "mounted DMG is missing the Applications symlink"
fi

if [[ "$NOTARIZE" -eq 1 ]]; then
  xcrun stapler validate "$MOUNTED_APP"
fi

cleanup_mount
trap - EXIT

# ── g. Checksum and summary ──────────────────────────────────────────────────

(
  cd "$OUT_DIR"
  shasum -a 256 "$DMG_NAME" > "$DMG_NAME.sha256"
)

echo ""
echo "=== macOS release packaged ==="
echo "  app:      $APP"
echo "  dmg:      $DMG ($(du -h "$DMG" | awk '{print $1}'))"
echo "  sha256:   $OUT_DIR/$DMG_NAME.sha256"
echo "  archs:    $UNIVERSAL_ARCHS"
echo "  signed:   $(if [[ "$AD_HOC" -eq 1 ]]; then echo "ad-hoc (local smoke only)"; else echo "$SIGNING_IDENTITY"; fi)"
echo "  notarized: $(if [[ "$NOTARIZE" -eq 1 ]]; then echo "yes (stapled)"; else echo "no"; fi)"
