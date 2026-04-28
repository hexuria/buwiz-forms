# macOS Build, Sign & Package — Troubleshooting Guide

> Lessons learned from the first production build cycle (2026-04-28).

---

## 1. Makefile Here-Doc Syntax Error

**Symptom:**
```
/bin/sh: -c: line 0: syntax error near unexpected token `newline'
/bin/sh: -c: line 0: `<?xml version="1.0" encoding="UTF-8"?>'
make: *** [package-mac] Error 2
```

**Cause:** The `Makefile` used a `cat <<'EOF'` here-document to write `Info.plist`. GNU Make invokes each recipe line as a separate `/bin/sh -c` call, which breaks multi-line here-docs.

**Fix:** Replace the here-doc with individual `@echo '...' >> file` statements, or use a `/bin/sh -c` wrapper with proper escaping.

**Rule:** Never use shell here-documents (`<<EOF`) in Makefile recipes. Use `echo` or `printf` instead.

---

## 2. Quoted `.env` Variables Cause Shell Syntax Errors

**Symptom:**
```
/bin/sh: -c: line 0: syntax error near unexpected token `('
/bin/sh: -c: line 0: `if [ -z ""Developer ID Application: Uriah Galang (5KZ8MD34QW)"" ] ...'
```

**Cause:** The `.env` file contains values wrapped in double quotes (e.g., `RELEASE_SIGNING_IDENTITY="Developer ID Application: ..."`). When Make's `-include .env` loads them, the quotes become literal characters. The parentheses in the identity string then break the shell.

**Fix:** Strip quotes at the top of the `Makefile` using `$(subst)`:
```makefile
RELEASE_SIGNING_IDENTITY := $(subst ",,$(RELEASE_SIGNING_IDENTITY))
APPLE_TEAM_ID := $(subst ",,$(APPLE_TEAM_ID))
```

**Rule:** Always strip quotes from `.env` variables in the Makefile header. `$(patsubst "%",%,...)` does NOT work for values containing spaces; use `$(subst ",,...)` instead.

---

## 3. Invalid GPUI Keystroke Identifier → Crash on Launch

**Symptom:**
```
thread 'main' panicked at binding.rs:44:10:
called `Result::unwrap()` on an `Err` value: InvalidKeystrokeError { keystroke: "cmd-opt-h" }
thread caused non-unwinding panic. aborting.
```
The app aborts immediately during `did_finish_launching` — no window ever appears.

**Cause:** GPUI's keystroke parser does not recognize `opt` as a modifier. The correct identifier is `alt`.

**Valid GPUI modifier names:**
| Modifier | GPUI String |
|----------|-------------|
| Command  | `cmd`       |
| Option   | `alt`       |
| Control  | `ctrl`      |
| Shift    | `shift`     |

**Fix:** Change `"cmd-opt-h"` → `"cmd-alt-h"`.

**Rule:** Always use `alt`, never `opt`, in GPUI keybinding strings. There is no runtime graceful fallback — an invalid keystroke panics and kills the app.

---

## 4. Hardcoded `CARGO_MANIFEST_DIR` Asset Path → Crash in `.app` Bundle

**Symptom:** App launches fine via `cargo run` but crashes immediately when launched from the `.app` bundle (either directly or from `/Applications`).

**Cause:** `env!("CARGO_MANIFEST_DIR")` is evaluated at **compile time** and bakes in the absolute path to your source tree (e.g., `/Volumes/goldcoders/reverse-engineer-ebir-forms/bir/crates/bir-desktop`). When the app runs from an `.app` bundle, that path doesn't exist and asset loading fails.

**Fix:** Dynamically resolve the assets directory at runtime:
```rust
let exe = std::env::current_exe().unwrap_or_default();
let is_bundle = exe.to_string_lossy().contains("Contents/MacOS");
let assets_dir = if is_bundle {
    exe.parent().unwrap().parent().unwrap().join("Resources/assets")
} else {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
};
```

**Rule:** Never rely solely on `CARGO_MANIFEST_DIR` for runtime resource paths. Always include a bundle-aware fallback.

---

## 5. Stale DMG from `hdiutil: convert failed - File exists`

**Symptom:** `make package-mac` reports `✅ DMG created` but the DMG contains the **old** binary. Installing and running it reproduces a crash you already fixed.

**Cause:** `create-dmg` calls `hdiutil convert` internally. If a DMG with the same filename already exists, `hdiutil` silently fails and `create-dmg` exits 0 anyway because the file _does_ exist.

**Fix:** Always delete the old DMG before creating a new one:
```makefile
@rm -f "$(RELEASE_DIR)/$(APP_NAME)-macOS-$(VERSION).dmg"
```
This is now included in both the `package-mac` and `sign-mac` targets.

**Rule:** Always `rm -f` the target DMG before running `create-dmg`. Never trust exit code 0 alone — verify the binary UUID changed.

**How to verify:** Compare the `slice_uuid` in the crash report against the output of:
```bash
dwarfdump --uuid target/release-artifacts/eBIRForms.app/Contents/MacOS/bir
```

---

## 6. `create-dmg` "Resource Busy" During Unmount

**Symptom:**
```
hdiutil: couldn't unmount "disk13" - Resource busy
The volume can't be ejected because it's currently in use.
```
This loops several times and can cause `make sign-mac` to fail with exit code 2.

**Cause:** macOS Spotlight (`mds_stores`) indexes the temporary mounted DMG volume, locking it.

**Workaround:** Force-unmount from another terminal:
```bash
diskutil unmount force /dev/diskN
```
Or add `.fseventsd` and `.Spotlight-V100` exclusion files to the DMG volume before unmounting.

**Rule:** If `make sign-mac` fails at the unmount step, force-eject the volume and re-run. This is a macOS Spotlight race condition, not a code bug.

---

## Build Sequence Cheat Sheet

```bash
# 1. Clean previous artifacts
rm -rf target/release-artifacts/eBIRForms.app
rm -f  target/release-artifacts/eBIRForms-macOS-*.dmg

# 2. Build universal binary + .app bundle + unsigned DMG
make package-mac

# 3. Sign inside-out + create SIGNED DMG + notarize + staple
make sign-mac

# 4. Verify the signature
codesign --verify --deep --strict target/release-artifacts/eBIRForms.app
spctl --assess --type exec target/release-artifacts/eBIRForms.app

# 5. Install
open target/release-artifacts/eBIRForms-macOS-0.1.0.dmg
```

---

## Environment Variables Reference

| Variable | Example | Purpose |
|----------|---------|---------|
| `APPLE_ID` | `codeitlikemiley@gmail.com` | Apple ID for notarization |
| `APPLE_TEAM_ID` | `5KZ8MD34QW` | 10-char team identifier |
| `SIGNING_IDENTITY` | `Apple Development: Uriah Galang (SLT9H2M79A)` | Dev signing (local testing) |
| `RELEASE_SIGNING_IDENTITY` | `Developer ID Application: Uriah Galang (5KZ8MD34QW)` | Production signing |
| `APP_PASSWORD` | `iakv-ljxq-mglm-nyaf` | App-specific password for notarization |

All values go in `.env` (not committed). Template structure is in `.env.example`.
