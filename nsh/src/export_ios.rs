//! Hands an authorized profile ZIP to the system's export sheet (`UIDocumentPickerViewController` in export mode:
//! Save to Files, iCloud Drive or another provider). iOS has no save panel, so the save-panel path cannot run here.

use std::cell::RefCell;
use std::path::Path;
use std::sync::mpsc;

use dispatch2::DispatchQueue;
use nsh_core::prelude::blocking;
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject, NSObject, NSObjectProtocol};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_foundation::{NSArray, NSString, NSURL};

use crate::export_archive::Archive;

/// Copies the staged archive to `name` in the app's temporary folder (the sheet copies it from there, so the user
/// sees the real file name), shows the export sheet and resolves once the user saved or dismissed it.
pub(crate) async fn save(archive: Archive, name: String) -> Result<String, String> {
    if name.is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." {
        return Err("The export has an invalid archive name.".into());
    }
    let directory = bir_core::platform::temp_dir().join("profile-exports");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let target = directory.join(&name);
    std::fs::copy(archive.path(), &target).map_err(|error| format!("Could not prepare the export: {error}"))?;
    drop(archive);
    let (opened_tx, opened_rx) = mpsc::channel::<Result<(), String>>();
    let (done_tx, done_rx) = mpsc::channel::<bool>();
    let shown = target.clone();
    DispatchQueue::main().exec_async(move || {
        // SAFETY: runs on the main thread.
        let _ = opened_tx.send(unsafe { present(&shown, done_tx) });
    });
    let saved = blocking(move || -> Result<bool, String> {
        opened_rx
            .recv()
            .map_err(|_| "The export sheet could not open.".to_owned())??;
        done_rx
            .recv()
            .map_err(|_| "The export sheet closed without an answer.".to_owned())
    })
    .await
    .map_err(|error| error.to_string())?;
    // The sheet copies the file before it answers (export as a copy), so the staged copy can go now.
    let _ = std::fs::remove_file(&target);
    match saved? {
        true => Ok(format!("Files: {name}")),
        false => Err("Export cancelled: nothing was saved.".into()),
    }
}

pub(crate) struct ExportIvars {
    done: RefCell<Option<mpsc::Sender<bool>>>,
}

define_class!(
    /// The delegate of one export sheet: reports whether the user saved the file.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "BuwizProfileExportDelegate"]
    #[ivars = ExportIvars]
    pub(crate) struct ExportDelegate;

    unsafe impl NSObjectProtocol for ExportDelegate {}

    impl ExportDelegate {
        #[unsafe(method(documentPicker:didPickDocumentsAtURLs:))]
        fn did_pick(&self, _picker: &AnyObject, _urls: &AnyObject) {
            self.finish(true);
        }

        #[unsafe(method(documentPickerWasCancelled:))]
        fn was_cancelled(&self, _picker: &AnyObject) {
            self.finish(false);
        }

        /// A sheet swiped down: the picker's own callbacks may not come then; whichever comes first answers.
        #[unsafe(method(presentationControllerDidDismiss:))]
        fn did_dismiss(&self, _controller: &AnyObject) {
            self.finish(false);
        }
    }
);

thread_local! {
    /// UIKit holds delegates weakly: each shown sheet's delegate lives here until it answers.
    static SHOWN: RefCell<Vec<Retained<ExportDelegate>>> = const { RefCell::new(Vec::new()) };
}

impl ExportDelegate {
    fn new(mtm: MainThreadMarker, done: mpsc::Sender<bool>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ExportIvars { done: RefCell::new(Some(done)) });
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }

    fn finish(&self, saved: bool) {
        if let Some(done) = self.ivars().done.borrow_mut().take() {
            let _ = done.send(saved);
        }
        let this: *const Self = self;
        SHOWN.with_borrow_mut(|shown| shown.retain(|delegate| !std::ptr::eq(&**delegate, this)));
    }
}

/// Presents the export sheet for the file at `path` over the frontmost view controller; `done` gets whether the
/// user saved it.
///
/// # Safety
/// Main thread only.
unsafe fn present(path: &Path, done: mpsc::Sender<bool>) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("The export sheet must open on the main thread.")?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    let urls = NSArray::from_retained_slice(&[url]);
    let class = AnyClass::get(c"UIDocumentPickerViewController").ok_or("UIKit's document picker is missing.")?;
    // SAFETY: documented UIKit initializer (iOS 14+), on the main thread (the caller's contract).
    let picker: Option<Retained<AnyObject>> = unsafe {
        let allocated: Allocated<AnyObject> = msg_send![class, alloc];
        msg_send![allocated, initForExportingURLs: &*urls, asCopy: true]
    };
    let picker = picker.ok_or("The export sheet could not be made.")?;
    // SAFETY: as above.
    let presenter = unsafe { top_view_controller() }.ok_or("There is no window to show the export sheet in.")?;
    let delegate = ExportDelegate::new(mtm, done);
    let none: Option<&block2::Block<dyn Fn()>> = None;
    // SAFETY: documented UIKit setters and presentation, on the main thread; the delegate is kept in `SHOWN`.
    unsafe {
        let _: () = msg_send![&picker, setDelegate: &*delegate];
        let sheet: Option<Retained<AnyObject>> = msg_send![&picker, presentationController];
        if let Some(sheet) = sheet {
            let _: () = msg_send![&sheet, setDelegate: &*delegate];
        }
        let _: () = msg_send![&presenter, presentViewController: &*picker, animated: true, completion: none];
    }
    SHOWN.with_borrow_mut(|shown| shown.push(delegate));
    Ok(())
}

/// The view controller on top of the key window of the first connected scene.
///
/// # Safety
/// Main thread only.
unsafe fn top_view_controller() -> Option<Retained<AnyObject>> {
    // SAFETY: documented UIKit getters, on the main thread (the caller's contract).
    unsafe {
        let application: Option<Retained<AnyObject>> = msg_send![AnyClass::get(c"UIApplication")?, sharedApplication];
        let application = application?;
        let scenes: Option<Retained<AnyObject>> = msg_send![&application, connectedScenes];
        let scenes = scenes?;
        let scene: Option<Retained<AnyObject>> = msg_send![&scenes, anyObject];
        let scene = scene?;
        let window: Option<Retained<AnyObject>> = msg_send![&scene, keyWindow];
        let window = window?;
        let mut current: Retained<AnyObject> = msg_send![&window, rootViewController];
        loop {
            let next: Option<Retained<AnyObject>> = msg_send![&current, presentedViewController];
            match next {
                Some(next) => current = next,
                None => return Some(current),
            }
        }
    }
}
