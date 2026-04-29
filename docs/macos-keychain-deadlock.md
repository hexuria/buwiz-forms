# macOS Keychain Deadlock Hazard

## The Problem

When running on macOS, accessing the system Keychain on the Main Thread (or Main RunLoop) during application startup can cause an **unbreakable deadlock**.

This typically manifests as the application "freezing" silently on a blank white screen or splash screen, without crashing, throwing errors, or responding to inputs.

## Root Cause

The deadlock occurs because of how `Security.framework` and macOS GUI applications interact:

1. **The Caller**: The application asks the system keychain for a password using native Apple APIs (e.g., via the `keyring` crate invoking `SecKeychainFindGenericPassword` or `SecItemAdd`).
2. **The Wait**: This API makes a synchronous XPC call to the macOS `securityd` daemon and blocks the calling thread waiting for a response.
3. **The UI Dispatch**: If `securityd` decides it needs to ask the user for permission (e.g., showing a dialog saying "eBIRForms wants to access your keychain"), it attempts to dispatch this UI prompt back onto the application's **Main RunLoop**.
4. **The Deadlock**: If the application made the keychain request *from* the Main RunLoop (which is common during early initialization like `AppState::new()` or `cx.spawn(...)` inside GPUI), the RunLoop is currently blocked waiting for the keychain request to finish. `securityd` is waiting for the RunLoop to become free to show the dialog. The result is a total freeze.

## The Solution

To avoid this entirely, we bypass the FFI bindings to `Security.framework` on macOS and instead delegate keychain operations to the native `security` CLI tool.

Because the CLI tool executes as an entirely separate process (`std::process::Command`), it has its own isolated RunLoop. If a permission dialog needs to be displayed, the CLI process handles it without blocking the GPUI main thread.

### Implementation Pattern

Always use platform-conditional compilation when accessing the master key or other sensitive credentials:

```rust
// CORRECT PATTERN
#[cfg(target_os = "macos")]
fn get_password() -> Result<String, Error> {
    // DO NOT use the `keyring` crate here!
    // Spawn a separate `security` CLI process to avoid deadlocking the RunLoop.
    let output = std::process::Command::new("security")
        .args(["find-generic-password", "-s", "com.ebir.rust", "-a", "sqlcipher_master_key", "-w"])
        .output()?;
    // ...
}

#[cfg(not(target_os = "macos"))]
fn get_password() -> Result<String, Error> {
    // Safe to use `keyring` crate directly on Windows and Linux
    // as they do not have the same RunLoop blocking architectural quirk.
    let entry = keyring::Entry::new("com.ebir.rust", "sqlcipher_master_key")?;
    entry.get_password()
}
```

## How to identify this in the future

If the app freezes on startup on macOS, use the macOS `sample` command:

```bash
sample $(pgrep -x bir | head -1) 1 1
```

If the stack trace contains `Security.framework` and `SecKeychain` or `set_generic_password` right at the bottom of the main thread, you have reintroduced the keychain RunLoop deadlock.
