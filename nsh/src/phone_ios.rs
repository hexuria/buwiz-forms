//! What bir-core needs from UIKit on iOS: the browser for the Buwiz sign-in. It goes to the main thread with
//! `DispatchQueue::main()`, from whichever thread bir-core calls it on: UIKit is main-thread only.
//!
//! The `open` crate cannot do it here: on iOS it spawns a `uiopen` program, which an iOS app has none of and
//! may not start.

use std::sync::mpsc;
use std::time::Duration;

use block2::RcBlock;
use dispatch2::DispatchQueue;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool};
use objc2::{MainThreadMarker, msg_send};
use objc2_foundation::{NSString, NSURL};

/// How long the sign-in waits for UIKit to say whether the browser opened.
const OPEN_WAIT: Duration = Duration::from_secs(10);

/// Registers the browser opener with bir-core. Called once, at process set-up.
pub(crate) fn install() {
    bir_core::platform::set_url_opener(open_url);
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

/// `UIApplication.shared`.
///
/// # Safety
/// Main thread only.
unsafe fn shared_application() -> Option<Retained<AnyObject>> {
    // SAFETY: documented UIKit getter, on the main thread (the caller's contract).
    unsafe { msg_send![AnyClass::get(c"UIApplication")?, sharedApplication] }
}
