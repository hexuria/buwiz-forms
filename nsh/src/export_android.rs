//! Publish an authorized private profile ZIP through scoped-storage Downloads.

use std::io::Read;

use jni::JNIEnv;
use jni::objects::{JObject, JString, JValue};
use nsh_core::prelude::blocking;

use crate::export_archive::{Archive, download_path};
use crate::print_android::{foreground_activity, java, ui, vm};

type Result<T> = std::result::Result<T, String>;

pub(crate) async fn save(archive: Archive, name: String) -> Result<String> {
    let resolver = ui(|env| {
        let activity = foreground_activity(env)?;
        let resolver = java(env.call_method(
            activity,
            "getContentResolver",
            "()Landroid/content/ContentResolver;",
            &[],
        ))?
        .l()
        .map_err(|error| error.to_string())?;
        java(env.new_global_ref(resolver))
    })
    .await
    .map_err(|error| error.replace("Android print:", "Android export:"))?;
    blocking(move || {
        let mut env = java(vm()?.attach_current_thread())?;
        let result = env.with_local_frame(64, |env| {
            Ok::<_, jni::errors::Error>(publish(env, resolver.as_obj(), &archive, &name))
        });
        java(result)?
    })
    .await
    .map_err(|error| error.to_string())?
}

fn text(env: &mut JNIEnv<'_>, values: &JObject<'_>, key: &str, value: &str) -> Result<()> {
    let key = java(env.new_string(key))?;
    let value = java(env.new_string(value))?;
    java(env.call_method(
        values,
        "put",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[JValue::Object(key.as_ref()), JValue::Object(value.as_ref())],
    ))?;
    Ok(())
}

fn pending(env: &mut JNIEnv<'_>, values: &JObject<'_>, value: i32) -> Result<()> {
    let key = java(env.new_string("is_pending"))?;
    let value = java(env.call_static_method(
        "java/lang/Integer",
        "valueOf",
        "(I)Ljava/lang/Integer;",
        &[JValue::Int(value)],
    ))?
    .l()
    .map_err(|error| error.to_string())?;
    java(env.call_method(
        values,
        "put",
        "(Ljava/lang/String;Ljava/lang/Integer;)V",
        &[JValue::Object(key.as_ref()), JValue::Object(&value)],
    ))?;
    Ok(())
}

fn detail(env: &mut JNIEnv<'_>, fallback: String) -> String {
    if !env.exception_check().unwrap_or(false) {
        return fallback;
    }
    let exception = env.exception_occurred();
    let _ = env.exception_clear();
    let message = exception.and_then(|exception| {
        let value = env
            .call_method(exception, "toString", "()Ljava/lang/String;", &[])?
            .l()?;
        let value = JString::from(value);
        let text = env.get_string(&value)?.to_string_lossy().into_owned();
        Ok(text)
    });
    let _ = env.exception_clear();
    message.unwrap_or(fallback)
}

fn publish(
    env: &mut JNIEnv<'_>,
    resolver: &JObject<'_>,
    archive: &Archive,
    name: &str,
) -> Result<String> {
    let result = (|| {
        let sdk = java(env.get_static_field("android/os/Build$VERSION", "SDK_INT", "I"))?
            .i()
            .map_err(|error| error.to_string())?;
        if sdk < 29 {
            return Err("Saving profiles to Downloads requires Android 10 or later.".into());
        }
        download_path(name)?;
        let values = java(env.new_object("android/content/ContentValues", "()V", &[]))?;
        text(env, &values, "_display_name", name)?;
        text(env, &values, "mime_type", "application/zip")?;
        text(env, &values, "relative_path", "Download/eBIRForms")?;
        pending(env, &values, 1)?;
        let collection = java(env.get_static_field(
            "android/provider/MediaStore$Downloads",
            "EXTERNAL_CONTENT_URI",
            "Landroid/net/Uri;",
        ))?
        .l()
        .map_err(|error| error.to_string())?;
        let uri = java(env.call_method(
            resolver,
            "insert",
            "(Landroid/net/Uri;Landroid/content/ContentValues;)Landroid/net/Uri;",
            &[JValue::Object(&collection), JValue::Object(&values)],
        ))?
        .l()
        .map_err(|error| error.to_string())?;
        if uri.is_null() {
            return Err("Android Downloads could not create the archive.".into());
        }
        let result = complete(env, resolver, &uri, archive);
        match result {
            Ok(path) => Ok(path),
            Err(error) => {
                let error = detail(env, error);
                if let Err(cleanup) = env.call_method(
                    resolver,
                    "delete",
                    "(Landroid/net/Uri;Ljava/lang/String;[Ljava/lang/String;)I",
                    &[
                        JValue::Object(&uri),
                        JValue::Object(&JObject::null()),
                        JValue::Object(&JObject::null()),
                    ],
                ) {
                    let cleanup = detail(env, cleanup.to_string());
                    tracing::warn!(%cleanup, "Could not remove incomplete profile download");
                }
                Err(error)
            }
        }
    })();
    result.map_err(|error| detail(env, error))
}

fn complete(
    env: &mut JNIEnv<'_>,
    resolver: &JObject<'_>,
    uri: &JObject<'_>,
    archive: &Archive,
) -> Result<String> {
    let output = java(env.call_method(
        resolver,
        "openOutputStream",
        "(Landroid/net/Uri;)Ljava/io/OutputStream;",
        &[JValue::Object(uri)],
    ))?
    .l()
    .map_err(|error| error.to_string())?;
    if output.is_null() {
        return Err("Android Downloads could not open the archive for writing.".into());
    }
    let copied = (|| {
        let mut source = std::fs::File::open(archive.path()).map_err(|error| error.to_string())?;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = source
                .read(&mut buffer)
                .map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            let bytes = java(env.byte_array_from_slice(&buffer[..count]))?;
            let result =
                java(env.call_method(&output, "write", "([B)V", &[JValue::Object(bytes.as_ref())]));
            java(env.delete_local_ref(bytes))?;
            result?;
        }
        Ok(())
    })()
    .map_err(|error| detail(env, error));
    let closed =
        java(env.call_method(&output, "close", "()V", &[])).map_err(|error| detail(env, error));
    copied?;
    closed?;
    let cursor = java(env.call_method(resolver, "query", "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;", &[
        JValue::Object(uri), JValue::Object(&JObject::null()), JValue::Object(&JObject::null()), JValue::Object(&JObject::null()), JValue::Object(&JObject::null()),
    ]))?.l().map_err(|error| error.to_string())?;
    if cursor.is_null() {
        return Err("Android Downloads did not return the saved name.".into());
    }
    let path = (|| {
        if !java(env.call_method(&cursor, "moveToFirst", "()Z", &[]))?
            .z()
            .map_err(|error| error.to_string())?
        {
            return Err("Android Downloads did not return the saved name.".into());
        }
        let column = java(env.new_string("_display_name"))?;
        let index = java(env.call_method(
            &cursor,
            "getColumnIndexOrThrow",
            "(Ljava/lang/String;)I",
            &[JValue::Object(column.as_ref())],
        ))?
        .i()
        .map_err(|error| error.to_string())?;
        let name = java(env.call_method(
            &cursor,
            "getString",
            "(I)Ljava/lang/String;",
            &[JValue::Int(index)],
        ))?
        .l()
        .map_err(|error| error.to_string())?;
        let name = JString::from(name);
        let name = java(env.get_string(&name))?.to_string_lossy().into_owned();
        download_path(&name)
    })()
    .map_err(|error| detail(env, error));
    let closed =
        java(env.call_method(&cursor, "close", "()V", &[])).map_err(|error| detail(env, error));
    let path = path?;
    closed?;
    let values = java(env.new_object("android/content/ContentValues", "()V", &[]))?;
    pending(env, &values, 0)?;
    let changed = java(env.call_method(
        resolver,
        "update",
        "(Landroid/net/Uri;Landroid/content/ContentValues;Ljava/lang/String;[Ljava/lang/String;)I",
        &[
            JValue::Object(uri),
            JValue::Object(&values),
            JValue::Object(&JObject::null()),
            JValue::Object(&JObject::null()),
        ],
    ))?
    .i()
    .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("Android Downloads could not publish the archive.".into());
    }
    Ok(path)
}
