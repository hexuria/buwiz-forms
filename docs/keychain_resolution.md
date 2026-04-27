# Keychain Double-Prompt Resolution

This document summarizes the changes made to resolve the issue where macOS was prompting the user twice for keychain access—once for the main `bir` application and once for the background `bir-daemon`.

## The Root Cause
The Rust `keyring` crate uses the native macOS `security-framework` by default when the `apple-native` feature is enabled. This framework restricts keychain item access specifically to the binary that created it. Because `bir` and `bir-daemon` are two distinct binaries without a shared `Keychain Access Group` (which requires Apple Developer code signing), macOS treated them as separate entities and required manual user approval for each one to access the `sqlcipher_master_key`.

## What Was Changed

### 1. Replaced `keyring` with `security` CLI on macOS
In `crates/bir-core/src/db.rs`, I introduced a conditional compilation split for `get_or_create_master_key()`.
- **For macOS:** The application now bypasses the `keyring` crate entirely. Instead, it interacts directly with the macOS `security` CLI (`security add-generic-password` and `security find-generic-password`).
- **For other platforms:** The application continues to use the reliable `keyring` crate.

### 2. Implemented the "Allow All Applications" Flag (`-A`)
When generating and storing a new master key on macOS, the application now passes the `-A` flag to the `security` command:
```bash
security add-generic-password -a "sqlcipher_master_key" -s "com.ebir.rust" -w <key> -A
```
The `-A` flag explicitly configures the keychain item to allow any application on the system to access it without warning. This elegantly solves the double-prompt issue for local/unsigned development, as both the `bir` GUI and `bir-daemon` can now read the master key silently.

### 3. Added Auto-Recovery for Existing Keys
If the application attempts to save the new unrestricted key but fails (usually because the user still has the old restricted key stored from a previous version), it will now automatically attempt to delete the old restricted key (`security delete-generic-password`) and immediately retry saving the new unrestricted one.

### 4. Compiler Warning Cleanup
I removed the global `use keyring::Entry;` import at the top of `db.rs` and scoped it strictly to the `not(target_os = "macos")` block to ensure a clean compilation without "unused import" warnings on macOS.

## User Action Required
For these changes to take effect perfectly, you might need to manually delete the old restricted `sqlcipher_master_key` from your macOS "Keychain Access" app if the automatic deletion fails due to strict system policies. Once deleted, the app will recreate it silently and unrestricted on its next launch.
