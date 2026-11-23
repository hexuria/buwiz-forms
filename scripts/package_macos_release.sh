#!/usr/bin/env bash
# Package the signed, notarized, stapled universal macOS release of the NSH
# app (nsh/): the .dmg, its checksum, and the updater archives.
#
# 1. `nsh package --target macos --arch universal` builds nsh/.nsh/out/<Name>.app
#    from nsh/nsh.json (id, version, icon, Info.plist keys, entitlements, the
#    updater configuration), signed ad hoc.
# 2. The app is signed again with SIGNING_IDENTITY, hardened runtime and
#    assets/macos/entitlements.mac.plist, then notarized and stapled.
# 3. The updater archives (<Name>-<v>-macos-aarch64.tar.gz and -x86_64, the
#    names `nsh update-manifest` reads) are made again from the signed app, as
#    NSH's app-updates guide says: the archives `nsh package` writes hold the
#    ad hoc signed app.
# 4. The .dmg is made from the signed app, signed, notarized and stapled.
#
# Environment:
#   VERSION               required, ^[0-9]+\.[0-9]+\.[0-9]+$, equal to the
#                         "version" of nsh/nsh.json
#   NSH                   the nsh command line (default: `nsh` on PATH, else
#                         ~/BuwizNsh/tools/bin/nsh)
#   SIGNING_IDENTITY      default "-" (ad hoc, local smoke tests only);
#                         otherwise a codesign identity name or SHA-1
#   KEYCHAIN_PATH         optional keychain passed to every codesign call
#   NOTARY_KEY_PATH       App Store Connect API .p8 key file (all three
#   NOTARY_KEY_ID         NOTARY_* must be set to enable notarization)
#   NOTARY_ISSUER_ID      App Store Connect issuer UUID
#   REQUIRE_NOTARIZATION  "1" fails before any work unless a real identity
#                         and all NOTARY_* inputs are configured
#   SKIP_NSH_PACKAGE      "1" reuses nsh/.nsh/out/<Name>.app as it is
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

NSH_VERSION="$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1], encoding="utf-8"))["version"])' nsh/nsh.json)"
if [[ "$VERSION" != "$NSH_VERSION" ]]; then
  fail "VERSION $VERSION does not match nsh/nsh.json version $NSH_VERSION"
fi

if [[ -z "${NSH:-}" ]]; then
  NSH="$(command -v nsh || true)"
  [[ -n "$NSH" ]] || NSH="$HOME/BuwizNsh/tools/bin/nsh"
fi
if [[ "${SKIP_NSH_PACKAGE:-0}" != "1" && ! -x "$NSH" ]]; then
  fail "nsh command line not found (set NSH, or build it: scripts/setup-macos.sh)"
fi

OUT_DIR="${OUT_DIR:-target/release-artifacts}"
ENTITLEMENTS=assets/macos/entitlements.mac.plist
[[ -f "$ENTITLEMENTS" ]] || fail "missing $ENTITLEMENTS"

keychain_args=()
if [[ -n "${KEYCHAIN_PATH:-}" ]]; then
  keychain_args+=(--keychain "$KEYCHAIN_PATH")
fi

# notarize FILE — submit to the Notary service with the App Store Connect API
# key, wait up to an hour, and require an Accepted status.
notary_log() {
  local result_json="$1" submission_id
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
}
notarize() {
  local file="$1"
  local result_json
  result_json="$(mktemp "${TMPDIR:-/tmp}/notary-result.XXXXXX")"
  if ! xcrun notarytool submit "$file" \
    --key "$NOTARY_KEY_PATH" \
    --key-id "$NOTARY_KEY_ID" \
    --issuer "$NOTARY_ISSUER_ID" \
    --wait --timeout 1h \
    --output-format json >"$result_json"; then
    notary_log "$result_json"
    rm -f "$result_json"
    fail "notarytool submit failed for $file"
  fi
  cat "$result_json"
  echo
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
  notary_log "$result_json"
  rm -f "$result_json"
  fail "notarization rejected $file"
}

require_universal() {
  local archs
  archs="$(lipo -archs "$1")"
  for required_arch in arm64 x86_64; do
    case " $archs " in
      *" $required_arch "*) ;;
      *) fail "$1 is missing $required_arch (lipo -archs: $archs)" ;;
    esac
  done
}

# ── a. The universal app from nsh package ────────────────────────────────────

NSH_APP="nsh/.nsh/out/$APP_NAME.app"
if [[ "${SKIP_NSH_PACKAGE:-0}" != "1" ]]; then
  rm -rf "$NSH_APP"
  (cd nsh && "$NSH" package --target macos --arch universal)
fi
[[ -d "$NSH_APP" ]] || fail "nsh package did not write $NSH_APP"

mkdir -p "$OUT_DIR"
APP="$OUT_DIR/$APP_NAME.app"
rm -rf "$APP"
ditto "$NSH_APP" "$APP"

EXECUTABLE="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$APP/Contents/Info.plist")"
APP_EXE="$APP/Contents/MacOS/$EXECUTABLE"
[[ -x "$APP_EXE" ]] || fail "missing executable $APP_EXE"
require_universal "$APP_EXE"
UNIVERSAL_ARCHS="$(lipo -archs "$APP_EXE")"

plist_value() {
  /usr/libexec/PlistBuddy -c "Print :$1" "$APP/Contents/Info.plist"
}
[[ "$(plist_value CFBundleShortVersionString)" == "$VERSION" ]] \
  || fail "Info.plist CFBundleShortVersionString is not $VERSION"
BUNDLE_ID="$(plist_value CFBundleIdentifier)"
[[ "$BUNDLE_ID" == "dev.goldcoders.bir" ]] || fail "unexpected bundle id $BUNDLE_ID"
# nsh package converts nsh.json's icon to this file.
[[ -f "$APP/Contents/Resources/AppIcon.icns" ]] || fail "missing Contents/Resources/AppIcon.icns"
plutil -lint "$APP/Contents/Info.plist"

python3 scripts/audit_no_legacy.py --package-root "$APP"

if find "$APP/Contents/Resources" -type f -print0 |
  xargs -0 file | grep -q "Mach-O"; then
  find "$APP/Contents/Resources" -type f -print0 |
    xargs -0 file | grep "Mach-O" >&2 || true
  fail "Mach-O file under Contents/Resources — notarization rejects unsigned nested code"
fi

xattr -cr "$APP"

# ── b. Sign ──────────────────────────────────────────────────────────────────

timestamp_arg=--timestamp
if [[ "$AD_HOC" -eq 1 ]]; then
  timestamp_arg=--timestamp=none
fi
# Nested code first (none today: the runner is the only Mach-O), the app last.
while IFS= read -r -d '' nested; do
  if [[ "$nested" != "$APP_EXE" ]] && file "$nested" | grep -q "Mach-O"; then
    codesign --force --options runtime "$timestamp_arg" \
      --sign "$SIGNING_IDENTITY" \
      ${keychain_args[@]+"${keychain_args[@]}"} \
      "$nested"
  fi
done < <(find "$APP/Contents" -type f -perm -u+x -print0)
codesign --force --options runtime "$timestamp_arg" \
  --sign "$SIGNING_IDENTITY" \
  --entitlements assets/macos/entitlements.mac.plist \
  ${keychain_args[@]+"${keychain_args[@]}"} \
  "$APP"

# The updater refuses an update that fails this check (nsh-updater install.rs).
codesign --verify --deep --strict --verbose=2 "$APP"
codesign --display --verbose=2 "$APP" 2>&1 | grep -E '^(Identifier|Authority|TeamIdentifier|Timestamp|CodeDirectory)' || true

# ── c. Notarize and staple the app ───────────────────────────────────────────

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
  codesign --verify --deep --strict --verbose=2 "$APP"
fi

# ── d. Updater archives from the signed app ──────────────────────────────────

ARCHIVE_BASE="$(printf '%s' "$APP_NAME" | tr -c 'A-Za-z0-9' '-')-$VERSION"
UPDATE_ARCHIVES=()
for platform in macos-aarch64 macos-x86_64; do
  archive="$ARCHIVE_BASE-$platform.tar.gz"
  rm -f "$OUT_DIR/$archive"
  COPYFILE_DISABLE=1 tar -czf "$OUT_DIR/$archive" -C "$OUT_DIR" "$APP_NAME.app"
  UPDATE_ARCHIVES+=("$archive")
done
# The archive must unpack to the app as signed.
CHECK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/ebirforms-archive.XXXXXX")"
tar -xzf "$OUT_DIR/${UPDATE_ARCHIVES[0]}" -C "$CHECK_DIR"
[[ "$(ls "$CHECK_DIR")" == "$APP_NAME.app" ]] || fail "update archive must hold only $APP_NAME.app"
codesign --verify --deep --strict --verbose=2 "$CHECK_DIR/$APP_NAME.app"
rm -rf "$CHECK_DIR"

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
require_universal "$MOUNTED_APP/Contents/MacOS/$EXECUTABLE"

if [[ ! -L "$MOUNT_POINT/Applications" ]]; then
  fail "mounted DMG is missing the Applications symlink"
fi

if [[ "$NOTARIZE" -eq 1 ]]; then
  xcrun stapler validate "$MOUNTED_APP"
  spctl --assess --type execute --verbose=2 "$MOUNTED_APP"
fi

cleanup_mount
trap - EXIT

# ── g. Checksums and summary ─────────────────────────────────────────────────

(
  cd "$OUT_DIR"
  for file in "$DMG_NAME" "${UPDATE_ARCHIVES[@]}"; do
    shasum -a 256 "$file" > "$file.sha256"
  done
)

echo ""
echo "=== macOS release packaged ==="
echo "  app:       $APP ($BUNDLE_ID $VERSION)"
echo "  dmg:       $DMG ($(du -h "$DMG" | awk '{print $1}'))"
for archive in "${UPDATE_ARCHIVES[@]}"; do
  echo "  update:    $OUT_DIR/$archive"
done
echo "  archs:     $UNIVERSAL_ARCHS"
echo "  signed:    $(if [[ "$AD_HOC" -eq 1 ]]; then echo "ad-hoc (local smoke only)"; else echo "$SIGNING_IDENTITY"; fi)"
echo "  notarized: $(if [[ "$NOTARIZE" -eq 1 ]]; then echo "yes (stapled)"; else echo "no"; fi)"
