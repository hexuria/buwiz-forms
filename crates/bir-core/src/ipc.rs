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
