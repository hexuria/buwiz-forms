//! The `bir` host module for gpui-shell, and the UI half of its hooks.
//!
//! The host functions live in `bir-app-api`, built on `bir-host-shim`'s
//! copy of the gpui-shell host types. [`module`] adapts that module to a
//! real [`gpui_shell::HostModule`] with the same name, functions, kinds and
//! TypeScript, then adds the scroll exports (GPUI elements, so they stay
//! here). [`ShellHooks`] answers what those functions ask of the UI.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

use bir_app_api::hooks::{Hooks, PathAnswer, PathChoice, PreviewProducer, UpdateStatus};
use bir_app_api::host as api;

/// Install [`ShellHooks`] for this process. Later calls change nothing.
pub fn install_hooks() {
    let _ = bir_app_api::install_hooks(ShellHooks);
}

/// The module the app's JavaScript imports as `"bir"`.
pub fn module() -> gpui_shell::HostModule {
    install_hooks();
    let api = Rc::new(bir_app_api::module());
    let functions: Vec<(String, api::FunctionKind)> = api
        .functions()
        .into_iter()
        .map(|(name, kind)| (name.to_owned(), kind))
        .collect();
    let mut module = gpui_shell::HostModule::new(api.name());
    for (name, kind) in functions {
        let api = Rc::clone(&api);
        let function = name.clone();
        module = match kind {
            api::FunctionKind::Sync => module.function(name, move |arguments| {
                api.call(&function, &to_api_arguments(arguments))
                    .map(to_shell_value)
                    .map_err(to_shell_error)
            }),
            api::FunctionKind::Async => module.async_function(name, move |arguments| {
                let future = api
                    .begin(&function, &to_api_arguments(arguments))
                    .map_err(to_shell_error)?;
                Ok(async move { future.await.map(to_shell_value).map_err(to_shell_error) })
            }),
        };
    }
    let module = crate::scroll_module::register(module);
    module.declarations(format!(
        "{}{}",
        api.declared().unwrap_or_default(),
        crate::scroll_module::DECLARATIONS
    ))
}

fn to_api_arguments(arguments: &gpui_shell::HostArguments) -> api::HostArguments {
    api::HostArguments::new(
        (0..arguments.len())
            .filter_map(|index| arguments.get(index))
            .map(to_api_value),
    )
}

fn to_api_value(value: &gpui_shell::HostValue) -> api::HostValue {
    use gpui_shell::HostValue as V;
    match value {
        V::Null => api::HostValue::Null,
        V::Bool(value) => api::HostValue::Bool(*value),
        V::Number(value) => api::HostValue::Number(*value),
        V::Str(value) => api::HostValue::Str(value.clone()),
        V::Array(values) => api::HostValue::Array(values.iter().map(to_api_value).collect()),
        V::Object(fields) => api::HostValue::Object(
            fields
                .iter()
                .map(|(key, value)| (key.clone(), to_api_value(value)))
                .collect(),
        ),
    }
}

fn to_shell_value(value: api::HostValue) -> gpui_shell::HostValue {
    use api::HostValue as V;
    match value {
        V::Null => gpui_shell::HostValue::Null,
        V::Bool(value) => gpui_shell::HostValue::Bool(value),
        V::Number(value) => gpui_shell::HostValue::Number(value),
        V::Str(value) => gpui_shell::HostValue::Str(value),
        V::Array(values) => {
            gpui_shell::HostValue::Array(values.into_iter().map(to_shell_value).collect())
        }
        V::Object(fields) => gpui_shell::HostValue::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key, to_shell_value(value)))
                .collect(),
        ),
    }
}

fn to_shell_error(error: api::HostError) -> gpui_shell::HostError {
    gpui_shell::HostError::new(error.message())
}

/// The gpui host's answers to [`Hooks`]: the code the moved functions called
/// directly before they left this crate.
struct ShellHooks;

impl Hooks for ShellHooks {
    fn refresh_menu(&self) {
        let _ = gpui_shell::with_current_app(crate::app_menu::refresh);
    }

    fn set_shell_chrome_open(&self, open: bool) {
        crate::app_menu::set_shell_chrome_open(open);
    }

    fn start_updater(&self) {
        crate::updater::start();
    }

    fn update_status(&self) -> UpdateStatus {
        let status = crate::updater::current_status();
        UpdateStatus {
            phase: status.phase,
            version: status.version,
            message: status.message,
            auto_download: status.auto_download,
            current_version: status.current_version,
        }
    }

    fn install_update(&self) {
        crate::updater::install_ready_update();
    }

    fn dismiss_update(&self) {
        crate::updater::dismiss();
    }

    fn check_for_updates(&self) {
        crate::updater::check_now();
    }

    fn set_auto_update_download(&self, enabled: bool) {
        crate::updater::set_auto_download(enabled);
    }

    fn print_preview_request(&self, path: PathBuf, title: String, key: String) {
        crate::print_preview_window::request_keyed(path, title, key);
    }

    fn print_preview_has_live(&self, key: &str) -> bool {
        crate::print_preview_window::has_live(key)
    }

    fn print_preview_live_update(&self, key: &str, produce: PreviewProducer, started: Instant) {
        crate::print_preview_window::live_update(key, produce, started);
    }

    #[cfg(feature = "agent")]
    fn agent_send_draft(
        &self,
        db: &bir_core::db::Database,
        kind: bir_core::forms::agent::AgentFormKind,
        draft: &mut serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        crate::agent::send_draft(db, kind, draft)
    }

    #[cfg(feature = "agent")]
    fn agent_pop_command(&self) -> Option<serde_json::Value> {
        crate::agent::pop_command()
    }

    fn prompt_for_open_path(&self, prompt: &str, answer: PathAnswer) -> bool {
        let prompt = prompt.to_owned();
        gpui_shell::with_current_app(move |cx| {
            let paths = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some(prompt.into()),
            });
            answer_after(
                cx,
                paths,
                |paths: Vec<PathBuf>| paths.into_iter().next(),
                answer,
            );
        })
        .is_some()
    }

    fn prompt_for_save_path(&self, directory: &Path, file_name: &str, answer: PathAnswer) -> bool {
        let directory = directory.to_path_buf();
        let file_name = file_name.to_owned();
        gpui_shell::with_current_app(move |cx| {
            let path = cx.prompt_for_new_path(&directory, Some(file_name.as_str()));
            answer_after(cx, path, Some, answer);
        })
        .is_some()
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn os_authenticate(&self, reason: &str) -> bool {
        use robius_authentication::{
            AndroidText, BiometricStrength, Context, PolicyBuilder, Text, WindowsText,
        };
        let Some(policy) = PolicyBuilder::new()
            .biometrics(Some(BiometricStrength::Strong))
            .password(true)
            .watch(true)
            .build()
        else {
            tracing::warn!("OS sign-in policy unavailable");
            return false;
        };
        let text = Text {
            android: AndroidText {
                title: "Admin sign-in",
                subtitle: None,
                description: None,
            },
            apple: reason,
            windows: WindowsText::new_truncated("Admin sign-in", reason),
        };
        futures::executor::block_on(Context::new(()).authenticate(text, &policy)).is_ok()
    }
}

/// GPUI opens the panel from a foreground task after the current update and
/// answers through a channel, so nothing nests inside the `App` borrow; the
/// answer runs in a foreground task too, on the thread that owns the
/// database slot.
fn answer_after<P, E, C>(
    cx: &mut gpui_kit::App,
    panel: impl std::future::Future<Output = Result<Result<Option<P>, E>, C>> + 'static,
    pick: impl FnOnce(P) -> Option<PathBuf> + 'static,
    answer: PathAnswer,
) where
    P: 'static,
    E: std::fmt::Display + 'static,
    C: 'static,
{
    cx.foreground_executor()
        .spawn(async move {
            let choice = match panel.await {
                Ok(Ok(Some(picked))) => {
                    pick(picked).map_or(PathChoice::Cancelled, PathChoice::Picked)
                }
                // Dismissed, or the panel went away without answering.
                Ok(Ok(None)) | Err(_) => PathChoice::Cancelled,
                Ok(Err(error)) => PathChoice::Failed(error.to_string()),
            };
            answer(choice);
        })
        .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The JS sees what it saw before the move: one `bir` module whose
    /// functions, kinds and TypeScript are the app-api module's, plus the
    /// scroll exports appended last.
    #[test]
    fn the_adapted_module_matches_the_app_api_module() {
        let api = bir_app_api::module();
        let shell = module();
        shell.validate().expect("bir module declarations agree");
        assert_eq!(shell.name(), "bir");
        assert_eq!(shell.name(), api.name());
        for (name, kind) in api.functions() {
            assert!(shell.has(name), "{name}");
            assert_eq!(
                shell.is_async(name),
                kind == api::FunctionKind::Async,
                "{name}"
            );
        }
        let mut expected: Vec<&str> = api.function_names();
        expected.extend(["scroll_forget", "scroll_offset", "scroll_reveal"]);
        expected.sort_unstable();
        assert_eq!(shell.function_names(), expected);
        assert_eq!(
            shell.declared().unwrap(),
            format!(
                "{}{}",
                api.declared().unwrap(),
                crate::scroll_module::DECLARATIONS
            )
        );
    }

    #[test]
    fn calls_and_errors_cross_the_adapter_unchanged() {
        let shell = module();
        let refused = shell
            .call(
                "set_global_hotkey",
                &gpui_shell::HostArguments::new([gpui_shell::HostValue::Number(1.0)]),
            )
            .unwrap_err();
        assert_eq!(
            refused.message(),
            "argument 1 must be a string, got a number"
        );
        let refused = shell
            .begin(
                "import_archive",
                &gpui_shell::HostArguments::new([gpui_shell::HostValue::from("x.zip")]),
            )
            .err()
            .expect("an argument is refused before the dialog");
        assert_eq!(
            refused.message(),
            "import_archive takes no path; the host opens the file dialog"
        );
    }
}
