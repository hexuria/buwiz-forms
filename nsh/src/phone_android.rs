//! What bir-core needs from Android: the browser for the Buwiz sign-in (an `ACTION_VIEW` intent from the
//! foreground Activity), `Settings.Secure.ANDROID_ID` for the install id (hashed by bir-core, never sent as it is),
//! and the screen kept on (`FLAG_KEEP_SCREEN_ON` on its window) while a send to the BIR is in
//! flight. No foreground service: not in this version.
//!
//! The `open` crate cannot do the first here: on Android it runs `xdg-open` and the like, which a phone has none of.
//!
//! bir-core calls both from its own threads, and window flags may only change on the UI thread, which NSH's
//! `run_on_main_thread` reaches from the engine thread only. So the UI thread's own looper is given a pipe once
//! ([`install`], from the engine thread): any thread queues a job and writes a byte, and the looper runs the queued
//! jobs on the UI thread.

use std::collections::VecDeque;
use std::ffi::{c_int, c_void};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc;
use std::sync::{Mutex, Once};
use std::time::Duration;

use jni::JNIEnv;
use jni::objects::{GlobalRef, JObject, JValue};

use crate::print_android::{foreground_activity, java, vm};

type Result<T> = std::result::Result<T, String>;
type Work = Box<dyn FnOnce(&mut JNIEnv<'_>) -> Result<()> + Send>;
type Done = Box<dyn FnOnce(Result<()>) + Send>;

/// `WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON`.
const FLAG_KEEP_SCREEN_ON: i32 = 0x0000_0080;

/// How long the sign-in waits for the UI thread to start the browser.
const OPEN_WAIT: Duration = Duration::from_secs(10);

/// NDK `ALOOPER_POLL_CALLBACK` and `ALOOPER_EVENT_INPUT`.
const ALOOPER_POLL_CALLBACK: c_int = -2;
const ALOOPER_EVENT_INPUT: c_int = 1;

#[repr(C)]
struct ALooper {
    _opaque: [u8; 0],
}

type ALooperCallback = unsafe extern "C" fn(fd: c_int, events: c_int, data: *mut c_void) -> c_int;

#[link(name = "android")]
unsafe extern "C" {
    fn ALooper_forThread() -> *mut ALooper;
    fn ALooper_acquire(looper: *mut ALooper);
    fn ALooper_addFd(
        looper: *mut ALooper,
        fd: c_int,
        ident: c_int,
        events: c_int,
        callback: Option<ALooperCallback>,
        data: *mut c_void,
    ) -> c_int;
}

/// Jobs for the UI thread, run in order.
static JOBS: Mutex<VecDeque<(Work, Done)>> = Mutex::new(VecDeque::new());
/// The pipe's writing end once the UI thread's looper watches it; -1 before.
static WAKE: AtomicI32 = AtomicI32::new(-1);
/// Whether a send is in flight, as bir-core said last. The UI thread applies the latest value.
static AWAKE: AtomicBool = AtomicBool::new(false);
/// The window the flag was set on (UI thread only), so it is cleared there even when another Activity is in front.
static FLAGGED: Mutex<Option<GlobalRef>> = Mutex::new(None);

/// Registers both hooks with bir-core. Called once, at process set-up (any thread).
pub(crate) fn register() {
    bir_core::platform::set_url_opener(open_url);
    bir_core::platform::set_device_id_source(android_id);
    bir_core::send_in_flight::set_hook(send_in_flight);
}

/// Gives the UI thread's looper the pipe, once. Must run on the engine thread (NSH's `run_on_main_thread`); jobs
/// queued earlier run as soon as it is in place.
pub(crate) fn install() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        // Not awaited: the closure runs on the UI thread after this turn's commit.
        drop(nsh_core::prelude::run_on_main_thread(|| {
            // SAFETY: on the UI thread, which has a looper (Android's main thread always does).
            if let Err(error) = unsafe { watch_pipe() } {
                tracing::warn!(%error, "Android: the UI thread's job pipe could not be set up");
            }
        }));
    });
}

/// # Safety
/// UI thread only.
unsafe fn watch_pipe() -> Result<()> {
    // SAFETY: plain NDK and libc calls; the looper is acquired, so it outlives this function, and the read end is
    // never closed.
    unsafe {
        let looper = ALooper_forThread();
        if looper.is_null() {
            return Err("this thread has no looper".into());
        }
        ALooper_acquire(looper);
        let mut fds: [c_int; 2] = [0; 2];
        if libc::pipe2(fds.as_mut_ptr(), libc::O_NONBLOCK | libc::O_CLOEXEC) != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if ALooper_addFd(looper, fds[0], ALOOPER_POLL_CALLBACK, ALOOPER_EVENT_INPUT, Some(on_wake), std::ptr::null_mut()) != 1 {
            libc::close(fds[0]);
            libc::close(fds[1]);
            return Err("ALooper_addFd failed".into());
        }
        WAKE.store(fds[1], Ordering::SeqCst);
    }
    run_jobs();
    Ok(())
}

/// The looper's callback, on the UI thread: empty the pipe, then run the queued jobs. `1` keeps watching.
unsafe extern "C" fn on_wake(fd: c_int, _events: c_int, _data: *mut c_void) -> c_int {
    let mut buffer = [0u8; 64];
    // SAFETY: `fd` is the pipe's non-blocking read end; the buffer is valid for its length.
    while unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) } > 0 {}
    // A panic must not unwind into the looper.
    if std::panic::catch_unwind(run_jobs).is_err() {
        tracing::error!("Android: a UI-thread job panicked");
    }
    1
}

/// Queues `work` for the UI thread; `done` hears how it went (on the UI thread).
fn post(work: impl FnOnce(&mut JNIEnv<'_>) -> Result<()> + Send + 'static, done: impl FnOnce(Result<()>) + Send + 'static) {
    JOBS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push_back((Box::new(work), Box::new(done)));
    let fd = WAKE.load(Ordering::SeqCst);
    if fd >= 0 {
        let byte = 1u8;
        // SAFETY: `fd` is the pipe's writing end, never closed. A full pipe already has a wake-up pending.
        unsafe { libc::write(fd, (&raw const byte).cast(), 1) };
    }
}

fn run_jobs() {
    loop {
        let Some((work, done)) = JOBS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).pop_front() else {
            return;
        };
        done(run_on_this_thread(work));
    }
}

/// Runs `work` with this thread's JNI environment; a Java exception is its error.
fn run_on_this_thread(work: Work) -> Result<()> {
    let mut env = java(vm()?.attach_current_thread())?;
    let result = java(env.with_local_frame(32, |env| Ok::<_, jni::errors::Error>(work(env))))?;
    if java(env.exception_check())? {
        let exception = java(env.exception_occurred())?;
        java(env.exception_clear())?;
        let detail = env
            .call_method(exception, "toString", "()Ljava/lang/String;", &[])
            .and_then(|value| value.l())
            .and_then(|value| env.get_string(&value.into()).map(|text| text.to_string_lossy().into_owned()));
        let _ = env.exception_clear();
        return Err(detail.unwrap_or_else(|_| "a Java call failed".into()));
    }
    result
}

/// Opens `url` (already checked by bir-core) with `Intent(ACTION_VIEW, Uri.parse(url))` from the foreground
/// Activity, and waits for the UI thread to say whether it started.
fn open_url(url: &str) -> Result<()> {
    let url = url.to_owned();
    let (tx, rx) = mpsc::channel::<Result<()>>();
    post(
        move |env| {
            let activity = foreground_activity(env)?;
            let address = java(env.new_string(&url))?;
            let uri = java(env.call_static_method(
                "android/net/Uri",
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&address)],
            ))?
            .l()
            .map_err(|error| error.to_string())?;
            let action = java(env.new_string("android.intent.action.VIEW"))?;
            let intent = java(env.new_object(
                "android/content/Intent",
                "(Ljava/lang/String;Landroid/net/Uri;)V",
                &[JValue::Object(&action), JValue::Object(&uri)],
            ))?;
            let browsable = java(env.new_string("android.intent.category.BROWSABLE"))?;
            java(env.call_method(
                &intent,
                "addCategory",
                "(Ljava/lang/String;)Landroid/content/Intent;",
                &[JValue::Object(&browsable)],
            ))?;
            java(env.call_method(activity, "startActivity", "(Landroid/content/Intent;)V", &[JValue::Object(&intent)]))?;
            Ok(())
        },
        move |result| {
            let _ = tx.send(result);
        },
    );
    match rx.recv_timeout(OPEN_WAIT) {
        Ok(result) => result.map_err(|error| format!("Android did not open the browser: {error}")),
        Err(_) => Err("Android did not open the browser in time.".into()),
    }
}

/// `Settings.Secure.getString(application.getContentResolver(), "android_id")`, on the calling thread (any thread: no
/// UI is touched; the application comes from `ActivityThread.currentApplication()`). `None` when it cannot be read.
fn android_id() -> Option<String> {
    let read = |env: &mut JNIEnv<'_>| -> Result<Option<String>> {
        let application = java(env.call_static_method(
            "android/app/ActivityThread",
            "currentApplication",
            "()Landroid/app/Application;",
            &[],
        ))?
        .l()
        .map_err(|error| error.to_string())?;
        if application.is_null() {
            return Ok(None);
        }
        let resolver = java(env.call_method(&application, "getContentResolver", "()Landroid/content/ContentResolver;", &[]))?
            .l()
            .map_err(|error| error.to_string())?;
        let name = java(env.new_string("android_id"))?;
        let id = java(env.call_static_method(
            "android/provider/Settings$Secure",
            "getString",
            "(Landroid/content/ContentResolver;Ljava/lang/String;)Ljava/lang/String;",
            &[JValue::Object(&resolver), JValue::Object(&name)],
        ))?
        .l()
        .map_err(|error| error.to_string())?;
        if id.is_null() {
            return Ok(None);
        }
        let id = jni::objects::JString::from(id);
        let text: String = java(env.get_string(&id))?.into();
        Ok(Some(text).filter(|id| !id.trim().is_empty()))
    };
    let mut env = vm().ok()?.attach_current_thread().ok()?;
    let found = env.with_local_frame(16, |env| Ok::<_, jni::errors::Error>(read(env))).ok();
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
        tracing::warn!("Android: ANDROID_ID could not be read");
        return None;
    }
    found.and_then(|found| found.map_err(|error| tracing::warn!(%error, "Android: ANDROID_ID could not be read")).ok()).flatten()
}

/// bir-core's "send in flight" hook: records the wish and hands it to the UI thread, which applies the latest one.
fn send_in_flight(on: bool) {
    AWAKE.store(on, Ordering::SeqCst);
    post(keep_screen_on, |result| {
        if let Err(error) = result {
            tracing::warn!(%error, "send in flight: the screen flag did not change");
        }
    });
}

/// On: `activity.getWindow().addFlags(FLAG_KEEP_SCREEN_ON)` on the foreground Activity, and that window is kept. Off:
/// `clearFlags(FLAG_KEEP_SCREEN_ON)` on the kept window. UI thread only.
fn keep_screen_on(env: &mut JNIEnv<'_>) -> Result<()> {
    let on = AWAKE.load(Ordering::SeqCst);
    let mut flagged = FLAGGED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if on {
        if flagged.is_some() {
            return Ok(());
        }
        let activity = foreground_activity(env)?;
        let window = java(env.call_method(activity, "getWindow", "()Landroid/view/Window;", &[]))?
            .l()
            .map_err(|error| error.to_string())?;
        set_flag(env, &window, "addFlags")?;
        *flagged = Some(java(env.new_global_ref(window))?);
    } else if let Some(window) = flagged.take() {
        set_flag(env, window.as_obj(), "clearFlags")?;
    }
    Ok(())
}

fn set_flag(env: &mut JNIEnv<'_>, window: &JObject<'_>, method: &str) -> Result<()> {
    java(env.call_method(window, method, "(I)V", &[JValue::Int(FLAG_KEEP_SCREEN_ON)]))?;
    Ok(())
}
