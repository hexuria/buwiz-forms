//! Android system print preview of a static, private sheet snapshot.
//!
//! ActivityThread/mActivities/ActivityClientRecord are hidden Android APIs: OEM or
//! Android changes may deny this lookup. Fail rather than guess an Activity.
//! One WebView, Activity and PrintJob are retained until the preview is closed,
//! because PrintDocumentAdapter can still read the WebView while the spooler is up.
//! UI work runs in async native tasks; revocation happens in the synchronous call.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use jni::objects::{GlobalRef, JObject, JValue};
use jni::{JNIEnv, JavaVM};
use nsh_core::prelude::{blocking, run_on_main_thread};

type Result<T> = std::result::Result<T, String>;

static VM: OnceLock<Result<JavaVM>> = OnceLock::new();
static GENERATION: AtomicU64 = AtomicU64::new(0);
static PREVIEW: Mutex<Option<Preview>> = Mutex::new(None);

struct Preview {
    view: GlobalRef,
    activity: GlobalRef,
    job: Option<GlobalRef>,
}

impl Preview {
    fn dispose(self, env: &mut JNIEnv<'_>, revoked: bool) {
        let calls = [
            (self.view.as_obj(), "stopLoading"),
            (self.view.as_obj(), "destroy"),
        ];
        for (object, method) in calls {
            if let Err(error) = env.call_method(object, method, "()V", &[]) {
                let _ = env.exception_clear();
                tracing::warn!(%error, method, "Android print: could not dispose of preview");
            }
        }
        if let Some(job) = &self.job
            && let Err(error) = self.dismiss_created_job(env, job, revoked)
        {
            let _ = env.exception_clear();
            tracing::warn!(%error, "Android print: could not dismiss preview");
        }
    }

    fn dismiss_created_job(&self, env: &mut JNIEnv<'_>, job: &GlobalRef, revoked: bool) -> Result<()> {
        let state = if revoked {
            let info = java(env.call_method(job.as_obj(), "getInfo", "()Landroid/print/PrintJobInfo;", &[]))?
                .l()
                .map_err(|error| error.to_string())?;
            Some(
                java(env.call_method(info, "getState", "()I", &[]))?
                    .i()
                    .map_err(|error| error.to_string())?,
            )
        } else {
            None
        };
        if !crate::print_android_policy::should_dismiss(revoked, state) {
            return Ok(());
        }
        let class = java(env.call_method(self.activity.as_obj(), "getClass", "()Ljava/lang/Class;", &[]))?
            .l()
            .map_err(|error| error.to_string())?;
        let intent = java(env.new_object(
            "android/content/Intent",
            "(Landroid/content/Context;Ljava/lang/Class;)V",
            &[JValue::Object(self.activity.as_obj()), JValue::Object(&class)],
        ))?;
        const CLEAR_TOP_SINGLE_TOP: i32 = 0x04000000 | 0x20000000;
        java(env.call_method(
            &intent,
            "addFlags",
            "(I)Landroid/content/Intent;",
            &[JValue::Int(CLEAR_TOP_SINGLE_TOP)],
        ))?;
        java(env.call_method(
            self.activity.as_obj(),
            "startActivity",
            "(Landroid/content/Intent;)V",
            &[JValue::Object(&intent)],
        ))?;
        Ok(())
    }
}

pub(crate) fn java<T>(value: jni::errors::Result<T>) -> Result<T> {
    value.map_err(|error| error.to_string())
}

pub(crate) fn vm() -> Result<&'static JavaVM> {
    VM.get_or_init(|| {
        type GetVms = unsafe extern "system" fn(
            *mut *mut jni::sys::JavaVM,
            jni::sys::jsize,
            *mut jni::sys::jsize,
        ) -> jni::sys::jint;
        for library in [c"libnativehelper.so", c"libart.so"] {
            // SAFETY: static C strings; the successful handle is deliberately
            // retained for the lifetime of the cached VM/function provider.
            let handle = unsafe { libc::dlopen(library.as_ptr(), libc::RTLD_NOW) };
            if handle.is_null() {
                continue;
            }
            // SAFETY: handle is open and the symbol has the JNI invocation ABI.
            let symbol = unsafe { libc::dlsym(handle, c"JNI_GetCreatedJavaVMs".as_ptr()) };
            if !symbol.is_null() {
                // SAFETY: JNI specifies this symbol's signature as GetVms.
                let get: GetVms = unsafe { std::mem::transmute(symbol) };
                let mut raw = std::ptr::null_mut();
                let mut count = 0;
                // SAFETY: both output pointers are valid, with room for one VM.
                if unsafe { get(&mut raw, 1, &mut count) } == jni::sys::JNI_OK
                    && count > 0
                    && !raw.is_null()
                {
                    // SAFETY: the invocation API returned the live process VM.
                    return unsafe { JavaVM::from_raw(raw) }.map_err(|error| error.to_string());
                }
            }
            // SAFETY: no pointer from this unsuccessful lookup is retained.
            unsafe { libc::dlclose(handle) };
        }
        Err("Android print: JNI_GetCreatedJavaVMs is unavailable or returned no VM.".into())
    })
    .as_ref()
    .map_err(Clone::clone)
}

pub(crate) async fn ui<R: Send + 'static>(
    work: impl FnOnce(&mut JNIEnv<'_>) -> Result<R> + Send + 'static,
) -> Result<R> {
    run_on_main_thread(move || {
        let mut env = java(vm()?.attach_current_thread())?;
        let result = java(env.with_local_frame(64, |env| Ok::<_, jni::errors::Error>(work(env))))?;
        if java(env.exception_check())? {
            let exception = java(env.exception_occurred())?;
            java(env.exception_clear())?;
            let detail = env
                .call_method(exception, "toString", "()Ljava/lang/String;", &[])
                .and_then(|value| value.l())
                .and_then(|value| {
                    env.get_string(&value.into())
                        .map(|s| s.to_string_lossy().into_owned())
                });
            let _ = env.exception_clear();
            return Err(format!(
                "Android print: {}",
                detail.unwrap_or_else(|_| {
                    "Java API failed (hidden Activity lookup may be blocked).".into()
                })
            ));
        }
        result
    })
    .await
    .map_err(|error| error.to_string())?
}

pub(crate) fn foreground_activity<'a>(env: &mut JNIEnv<'a>) -> Result<JObject<'a>> {
    let thread = java(env.call_static_method(
        "android/app/ActivityThread",
        "currentActivityThread",
        "()Landroid/app/ActivityThread;",
        &[],
    ))?
    .l()
    .map_err(|e| e.to_string())?;
    let activities = java(env.get_field(thread, "mActivities", "Landroid/util/ArrayMap;"))?
        .l()
        .map_err(|e| e.to_string())?;
    let values = java(env.call_method(activities, "values", "()Ljava/util/Collection;", &[]))?
        .l()
        .map_err(|e| e.to_string())?;
    let iterator = java(env.call_method(values, "iterator", "()Ljava/util/Iterator;", &[]))?
        .l()
        .map_err(|e| e.to_string())?;
    while java(env.call_method(&iterator, "hasNext", "()Z", &[]))?
        .z()
        .map_err(|e| e.to_string())?
    {
        let record = java(env.call_method(&iterator, "next", "()Ljava/lang/Object;", &[]))?
            .l()
            .map_err(|e| e.to_string())?;
        let paused = java(env.get_field(&record, "paused", "Z"))?
            .z()
            .map_err(|e| e.to_string())?;
        if !paused {
            let activity = java(env.get_field(&record, "activity", "Landroid/app/Activity;"))?
                .l()
                .map_err(|e| e.to_string())?;
            if !activity.is_null() {
                return Ok(activity);
            }
        }
        java(env.delete_local_ref(record))?;
    }
    Err("Android print: no foreground Activity was found via ActivityThread.mActivities.".into())
}

/// Cancels loading of a superseded sheet without accessing JNI from the sync hook.
pub(crate) fn invalidate() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
}

pub(crate) fn generation() -> u64 {
    GENERATION.load(Ordering::SeqCst)
}

/// Revokes synchronously, before the returned Java cleanup task is started.
pub(crate) fn close() -> impl Future<Output = ()> {
    invalidate();
    let old = {
        let mut kept = PREVIEW.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        kept.take()
    };
    async move {
        let Some(old) = old else {
            return;
        };
        if let Err(error) = ui(move |env| {
            old.dispose(env, true);
            Ok(())
        })
        .await
        {
            tracing::warn!(%error, "Android print: could not close preview");
        }
    }
}

/// Loads a static snapshot, then opens Android's A4 system print preview.
pub(crate) async fn preview(url: String, title: String, generation: u64) -> Result<()> {
    let view = ui(move |env| {
        let mut kept = PREVIEW.lock().map_err(|_| "Android print state is poisoned.")?;
        if GENERATION.load(Ordering::SeqCst) != generation {
            return Err("Android print preview was superseded or closed.".into());
        }
        if let Some(old) = kept.take() {
            old.dispose(env, false);
        }
        let activity = foreground_activity(env)?;
        let view = java(env.new_object(
            "android/webkit/WebView",
            "(Landroid/content/Context;)V",
            &[JValue::Object(&activity)],
        ))?;
        let settings =
            java(env.call_method(&view, "getSettings", "()Landroid/webkit/WebSettings;", &[]))?
                .l()
                .map_err(|e| e.to_string())?;
        java(env.call_method(
            &settings,
            "setJavaScriptEnabled",
            "(Z)V",
            &[JValue::Bool(0)],
        ))?;
        java(env.call_method(&settings, "setAllowFileAccess", "(Z)V", &[JValue::Bool(1)]))?;
        let url = JObject::from(java(env.new_string(url))?);
        java(env.call_method(
            &view,
            "loadUrl",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&url)],
        ))?;
        let view = java(env.new_global_ref(view))?;
        let activity = java(env.new_global_ref(activity))?;
        *kept = Some(Preview {
            view: view.clone(),
            activity,
            job: None,
        });
        Ok(view)
    })
    .await?;
    let started = Instant::now();
    loop {
        blocking(|| std::thread::sleep(Duration::from_millis(100)))
            .await
            .map_err(|e| e.to_string())?;
        if GENERATION.load(Ordering::SeqCst) != generation {
            return Err("Android print preview was superseded or closed.".into());
        }
        let current = view.clone();
        let progress = ui(move |env| {
            if GENERATION.load(Ordering::SeqCst) != generation {
                return Err("Android print preview was superseded or closed.".into());
            }
            java(env.call_method(current.as_obj(), "getProgress", "()I", &[]))?
                .i()
                .map_err(|e| e.to_string())
        })
        .await?;
        if progress == 100 {
            break;
        }
        if started.elapsed() >= Duration::from_secs(10) {
            return Err(
                "Android print: the sheet did not finish loading within 10 seconds.".into(),
            );
        }
    }
    ui(move |env| {
        // A job already starting must be retained before close() can take it.
        let mut kept = PREVIEW.lock().map_err(|_| "Android print state is poisoned.")?;
        if GENERATION.load(Ordering::SeqCst) != generation {
            return Err("Android print preview was superseded or closed.".into());
        }
        let kept = kept.as_mut().ok_or("Android print WebView is no longer available.")?;
        let title = JObject::from(java(env.new_string(title))?);
        let adapter = java(env.call_method(view.as_obj(), "createPrintDocumentAdapter", "(Ljava/lang/String;)Landroid/print/PrintDocumentAdapter;", &[JValue::Object(&title)]))?.l().map_err(|e| e.to_string())?;
        let service = JObject::from(java(env.new_string("print"))?);
        let manager = java(env.call_method(kept.activity.as_obj(), "getSystemService", "(Ljava/lang/String;)Ljava/lang/Object;", &[JValue::Object(&service)]))?.l().map_err(|e| e.to_string())?;
        let builder = java(env.new_object("android/print/PrintAttributes$Builder", "()V", &[]))?;
        let a4 = java(env.get_static_field("android/print/PrintAttributes$MediaSize", "ISO_A4", "Landroid/print/PrintAttributes$MediaSize;"))?.l().map_err(|e| e.to_string())?;
        java(env.call_method(&builder, "setMediaSize", "(Landroid/print/PrintAttributes$MediaSize;)Landroid/print/PrintAttributes$Builder;", &[JValue::Object(&a4)]))?;
        let attributes = java(env.call_method(builder, "build", "()Landroid/print/PrintAttributes;", &[]))?.l().map_err(|e| e.to_string())?;
        if GENERATION.load(Ordering::SeqCst) != generation {
            return Err("Android print preview was superseded or closed.".into());
        }
        let job = java(env.call_method(manager, "print", "(Ljava/lang/String;Landroid/print/PrintDocumentAdapter;Landroid/print/PrintAttributes;)Landroid/print/PrintJob;", &[JValue::Object(&title), JValue::Object(&adapter), JValue::Object(&attributes)]))?.l().map_err(|e| e.to_string())?;
        if job.is_null() {
            return Err("Android print manager returned no print job.".into());
        }
        kept.job = Some(java(env.new_global_ref(job))?);
        Ok(())
    }).await
}
