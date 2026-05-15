//! Cross-process notification for database changes.
//!
//! On macOS: Uses `NSDistributedNotificationCenter` for instant, zero-polling IPC.
//! On other platforms: No-op (desktop falls back to PRAGMA data_version polling).

/// The notification name used for cross-process DB change signals.
#[cfg(target_os = "macos")]
const NOTIFICATION_NAME: &str = "dev.goldcoders.bir.DatabaseChanged";

/// Post a distributed notification that the database was modified.
/// Called by `bir-daemon` after completing a job or updating form status.
#[cfg(target_os = "macos")]
pub fn post_db_changed() {
    use std::os::raw::c_void;

    // SAFETY: These are raw bindings to Core Foundation C APIs that have no
    // safe Rust wrapper. The invariants upheld here are:
    // 1. `CFNotificationCenterGetDistributedCenter()` always returns a non-null
    //    pointer to a process-wide singleton — it is safe to call at any time.
    // 2. `CFStringCreateWithCString` returns a valid CFStringRef for any valid
    //    UTF-8 C string. The CString is kept alive for the duration of the call.
    // 3. `CFNotificationCenterPostNotification` consumes a borrowed reference to
    //    the center and name; no ownership is transferred.
    // 4. `CFRelease(cf_name)` correctly releases the one strong reference
    //    created by `CFStringCreateWithCString` — no double-free is possible.
    unsafe extern "C" {
        fn CFNotificationCenterGetDistributedCenter() -> *const c_void;
        fn CFNotificationCenterPostNotification(
            center: *const c_void,
            name: *const c_void,
            object: *const c_void,
            user_info: *const c_void,
            deliver_immediately: bool,
        );
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            c_str: *const i8,
            encoding: u32,
        ) -> *const c_void;
        fn CFRelease(cf: *const c_void);
    }

    // kCFStringEncodingUTF8 = 0x08000100
    const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;

    // SAFETY: All invariants documented on the `unsafe extern "C"` block above
    // are upheld. The CString outlives all CF calls in this block.
    unsafe {
        let center = CFNotificationCenterGetDistributedCenter();
        let name_cstr = std::ffi::CString::new(NOTIFICATION_NAME).unwrap();
        let cf_name = CFStringCreateWithCString(
            std::ptr::null(),
            name_cstr.as_ptr(),
            K_CF_STRING_ENCODING_UTF8,
        );
        CFNotificationCenterPostNotification(
            center,
            cf_name,
            std::ptr::null(), // object
            std::ptr::null(), // userInfo
            true,             // deliverImmediately
        );
        CFRelease(cf_name);
    }

    tracing::debug!(
        "Posted macOS distributed notification: {}",
        NOTIFICATION_NAME
    );
}

/// No-op on non-macOS platforms. Desktop uses PRAGMA data_version polling instead.
#[cfg(not(target_os = "macos"))]
pub fn post_db_changed() {
    // No-op: Linux/Windows rely on PRAGMA data_version polling.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_db_changed() {
        // Just verify that calling the FFI doesn't cause a segfault or panic.
        // There's no clean way to intercept distributed notifications synchronously in a unit test
        // without standing up an entire NSApplication runloop, but ensuring memory safety is key.
        post_db_changed();
    }
}
