//! Cross-process event detection for database changes.
//!
//! On macOS: Listens for `NSDistributedNotificationCenter` signals from `bir-daemon`
//!           for instant, zero-polling event-driven updates.
//! On all platforms: Falls back to PRAGMA data_version polling (1s interval)
//!                   to catch any changes the notification might miss.

use bir_core::db::Database;
use gpui::{Entity, EventEmitter};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub enum AppEvent {
    /// Fired when a different process (like bir_daemon) modifies the SQLite database.
    DatabaseChanged,
}

pub struct EventBus {}

impl EventEmitter<AppEvent> for EventBus {}

pub struct GlobalEventBus(pub Entity<EventBus>);

impl gpui::Global for GlobalEventBus {}

/// The notification name matching the one posted by `bir_core::ipc::post_db_changed()`.
#[cfg(target_os = "macos")]
const NOTIFICATION_NAME: &str = "dev.goldcoders.bir.DatabaseChanged";

/// On macOS: Registers a CFNotificationCenter observer that fires `AppEvent::DatabaseChanged`
/// instantly when the daemon posts a distributed notification. Zero polling.
#[cfg(target_os = "macos")]
pub fn start_macos_notification_listener(cx: &mut gpui::Context<crate::app::AppState>) {
    use std::os::raw::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};

    // Shared flag: set to true by the C callback, read by the GPUI polling task.
    // We use a lightweight 100ms check on this flag instead of 1s PRAGMA polling.
    static NOTIFICATION_RECEIVED: AtomicBool = AtomicBool::new(false);

    // SAFETY: These are raw bindings to Core Foundation C APIs. No safe Rust
    // wrapper exists for CFNotificationCenter. Invariants:
    // 1. `CFNotificationCenterGetDistributedCenter()` returns a process-wide
    //    singleton; always non-null and safe to call from any thread.
    // 2. `CFStringCreateWithCString` returns a valid CFStringRef for any valid
    //    null-terminated UTF-8 C string. The CString outlives this block.
    // 3. `CFNotificationCenterAddObserver` borrows center and name without
    //    taking ownership. The `on_notification` fn pointer is 'static.
    // 4. The `cf_name` pointer is intentionally leaked (documented below) —
    //    it must remain valid for the lifetime of the registered observer.
    unsafe extern "C" {
        fn CFNotificationCenterGetDistributedCenter() -> *const c_void;
        fn CFNotificationCenterAddObserver(
            center: *const c_void,
            observer: *const c_void,
            callback: extern "C" fn(
                center: *const c_void,
                observer: *const c_void,
                name: *const c_void,
                object: *const c_void,
                user_info: *const c_void,
            ),
            name: *const c_void,
            object: *const c_void,
            suspension_behavior: isize,
        );
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            c_str: *const i8,
            encoding: u32,
        ) -> *const c_void;
    }

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;
    // CFNotificationSuspensionBehaviorDeliverImmediately = 4
    const DELIVER_IMMEDIATELY: isize = 4;

    extern "C" fn on_notification(
        _center: *const c_void,
        _observer: *const c_void,
        _name: *const c_void,
        _object: *const c_void,
        _user_info: *const c_void,
    ) {
        NOTIFICATION_RECEIVED.store(true, Ordering::Release);
    }

    // SAFETY: All invariants documented on the `unsafe extern "C"` block above
    // are upheld. The CString (`name_cstr`) outlives all CF calls. The
    // `cf_name` pointer is intentionally leaked so it remains valid for the
    // entire process lifetime of the observer registration.
    unsafe {
        let center = CFNotificationCenterGetDistributedCenter();
        let name_cstr = std::ffi::CString::new(NOTIFICATION_NAME).unwrap();
        let cf_name = CFStringCreateWithCString(
            std::ptr::null(),
            name_cstr.as_ptr(),
            K_CF_STRING_ENCODING_UTF8,
        );
        // We intentionally leak cf_name — it must live for the lifetime of the observer.
        CFNotificationCenterAddObserver(
            center,
            std::ptr::null(), // observer (unused, we use a static flag)
            on_notification,
            cf_name,
            std::ptr::null(), // object (observe from any sender)
            DELIVER_IMMEDIATELY,
        );
    }

    tracing::info!(
        "macOS: Registered distributed notification observer for '{}'",
        NOTIFICATION_NAME
    );

    // Lightweight polling task: checks the atomic flag every 100ms.
    // This is much faster than PRAGMA polling and costs essentially zero CPU.
    let bus = cx.global::<GlobalEventBus>().0.clone();
    cx.spawn(async move |_app_state, cx| {
        loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;

            if NOTIFICATION_RECEIVED.swap(false, Ordering::AcqRel) {
                tracing::info!(
                    "macOS: Received distributed notification — broadcasting DatabaseChanged"
                );
                cx.update(|cx| {
                    bus.update(cx, |_, cx| {
                        cx.emit(AppEvent::DatabaseChanged);
                    })
                });
            }
        }
    })
    .detach();
}

/// Spawns a background task that periodically checks the SQLite `data_version`
/// to detect if an external process (like `bir_daemon`) modified the database.
///
/// On macOS: This serves as a fallback safety net alongside the instant notification listener.
///           The interval is increased to 5s since notifications handle the fast path.
/// On other platforms: This is the primary detection mechanism (1s interval).
pub fn start_db_watcher(db: Arc<Mutex<Database>>, cx: &mut gpui::Context<crate::app::AppState>) {
    let bus = cx.global::<GlobalEventBus>().0.clone();

    #[cfg(target_os = "macos")]
    let poll_interval_ms = 5000; // 5s fallback on macOS (notifications handle the fast path)
    #[cfg(not(target_os = "macos"))]
    let poll_interval_ms = 1000; // 1s on Linux/Windows (primary mechanism)

    cx.spawn(async move |_app_state, cx| {
        let mut last_version: Option<i32> = None;
        loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(poll_interval_ms))
                .await;

            let current_version_opt = cx
                .background_executor()
                .spawn({
                    let db = Arc::clone(&db);
                    async move {
                        if let Ok(db_guard) = db.lock() {
                            // data_version() increments when another WAL connection commits.
                            db_guard.data_version()
                        } else {
                            None
                        }
                    }
                })
                .await;

            let current_version = if let Some(v) = current_version_opt {
                v
            } else {
                continue;
            };

            if let Some(lv) = last_version
                && lv != current_version
            {
                tracing::info!(
                    "Database Watcher: External DB change detected (v{} -> v{})",
                    lv,
                    current_version
                );
                cx.update(|cx| {
                    bus.update(cx, |_, cx| {
                        cx.emit(AppEvent::DatabaseChanged);
                    })
                });
            }
            last_version = Some(current_version);
        }
    })
    .detach();
}
