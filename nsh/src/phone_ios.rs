//! What bir-core needs from UIKit on iOS: the browser for the Buwiz sign-in, and the phone kept awake (with
//! background time asked for) while a send to the BIR is in flight. Both go to the main thread with
//! `DispatchQueue::main()`, from whichever thread bir-core calls them on: UIKit is main-thread only.
//!
//! The `open` crate cannot do the first here: on iOS it spawns a `uiopen` program, which an iOS app has none of and
//! may not start.

use std::cell::Cell;
use std::sync::mpsc;
use std::time::Duration;

use block2::RcBlock;
use dispatch2::DispatchQueue;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool};
use objc2::{MainThreadMarker, msg_send};
use objc2_foundation::{NSString, NSURL};

/// `UIBackgroundTaskInvalid`.
const NO_TASK: usize = 0;

/// How long the sign-in waits for UIKit to say whether the browser opened.
const OPEN_WAIT: Duration = Duration::from_secs(10);

thread_local! {
    /// The background task of the sends in flight (main thread only), or [`NO_TASK`].
    static TASK: Cell<usize> = const { Cell::new(NO_TASK) };
}

/// Registers both hooks with bir-core. Called once, at process set-up.
pub(crate) fn install() {
    bir_core::platform::set_url_opener(open_url);
    bir_core::send_in_flight::set_hook(send_in_flight);
}

/// Opens `url` (already checked by bir-core) in the system browser and answers whether UIKit opened it.
fn open_url(url: &str) -> Result<(), String> {
    let url = url.to_owned();
    if MainThreadMarker::new().is_some() {
        // SAFETY: on the main thread. Nothing can wait for the answer here: it comes on this thread, later.
        return unsafe { open_on_main(&url, None) };
    }
    let (tx, rx) = mpsc::channel::<bool>();
    DispatchQueue::main().exec_async(move || {
        // SAFETY: on the main thread.
        if let Err(error) = unsafe { open_on_main(&url, Some(tx)) } {
            tracing::warn!(%error, "sign-in: the browser did not open");
        }
    });
    match rx.recv_timeout(OPEN_WAIT) {
        Ok(true) => Ok(()),
        Ok(false) => Err("iOS did not open the browser.".into()),
        Err(_) => Err("iOS did not say whether the browser opened.".into()),
    }
}

/// `UIApplication.shared.open(url, options: [:], completionHandler:)`. A sender dropped unanswered (an error here)
/// tells the waiting thread `false` at once.
///
/// # Safety
/// Main thread only.
unsafe fn open_on_main(url: &str, answer: Option<mpsc::Sender<bool>>) -> Result<(), String> {
    let address = NSURL::URLWithString(&NSString::from_str(url)).ok_or("Not a URL iOS can open.")?;
    // SAFETY: documented UIKit and Foundation API, on the main thread (the caller's contract). The completion block
    // runs once, on the main thread.
    unsafe {
        let application = shared_application().ok_or("UIKit has no application.")?;
        let options: Retained<AnyObject> =
            msg_send![AnyClass::get(c"NSDictionary").ok_or("Foundation is missing.")?, dictionary];
        let done = RcBlock::new(move |opened: Bool| {
            if let Some(answer) = &answer {
                let _ = answer.send(opened.as_bool());
            }
        });
        let _: () = msg_send![&application, openURL: &*address, options: &*options, completionHandler: &*done];
    }
    Ok(())
}

/// bir-core's "send in flight" hook: hands the change to the main thread, in order (the main queue is serial).
fn send_in_flight(on: bool) {
    DispatchQueue::main().exec_async(move || {
        // SAFETY: on the main thread.
        unsafe { keep_awake(on) }
    });
}

/// On: the screen does not lock while the app is in front (`isIdleTimerDisabled`), and the app asks for background
/// time (`beginBackgroundTask`) so a send keeps going for a while if the person leaves the app. Off: both are given
/// back. When iOS ends the background time first, its expiration handler gives the task back (iOS ends an app that
/// keeps it); the send then stops with the app, and the queue decides what its outcome is.
///
/// # Safety
/// Main thread only.
unsafe fn keep_awake(on: bool) {
    // SAFETY: documented UIKit API, on the main thread (the caller's contract).
    unsafe {
        let Some(application) = shared_application() else {
            tracing::warn!("send in flight: UIKit has no application");
            return;
        };
        let _: () = msg_send![&application, setIdleTimerDisabled: on];
        if on {
            if TASK.get() != NO_TASK {
                return;
            }
            let name = NSString::from_str("Sending a return to the BIR");
            let expired = RcBlock::new(background_time_expired);
            let task: usize = msg_send![&application, beginBackgroundTaskWithName: &*name, expirationHandler: &*expired];
            TASK.set(task);
        } else {
            end_task();
        }
    }
}

/// The background task's expiration handler: iOS calls it on the main thread.
fn background_time_expired() {
    tracing::warn!("send in flight: iOS ended the background time before the send finished");
    // SAFETY: iOS calls the expiration handler on the main thread.
    unsafe { end_task() };
}

/// Gives the background task back, if there is one.
///
/// # Safety
/// Main thread only.
unsafe fn end_task() {
    let task = TASK.replace(NO_TASK);
    if task == NO_TASK {
        return;
    }
    // SAFETY: documented UIKit API, on the main thread (the caller's contract).
    unsafe {
        if let Some(application) = shared_application() {
            let _: () = msg_send![&application, endBackgroundTask: task];
        }
    }
}

/// `UIApplication.shared`.
///
/// # Safety
/// Main thread only.
unsafe fn shared_application() -> Option<Retained<AnyObject>> {
    // SAFETY: documented UIKit getter, on the main thread (the caller's contract).
    unsafe { msg_send![AnyClass::get(c"UIApplication")?, sharedApplication] }
}
