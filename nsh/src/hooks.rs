//! What the host functions ask of a UI, answered for the NSH app.
//!
//! The file panels are real: they use NSH's own dialogs (`pick_paths`, `pick_save_path`), which only a task
//! on the engine thread can call, and the host functions that need them run there. Everything the NSH app
//! has no UI for yet says so (a log line, a "not available yet" answer), and none of it pretends to work.
//! Wave 4 fills these in: updater and menu (D4), print preview window (D3), sign-in prompt (D2), agent (D6).

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

use bir_app_api::hooks::{Hooks, PathAnswer, PathChoice, PreviewProducer, UpdateStatus};
use bir_core::db::Database;
use bir_core::forms::agent::AgentFormKind;
use nsh_core::file_dialog::{OpenDialog, SaveDialog};
use nsh_core::prelude::{Spawned, pick_paths, pick_save_path, spawn, try_os};
use serde_json::Value;

/// The sentence behind every "not yet" answer of this wave.
const NOT_YET: &str = "is not available in the NSH app yet";

pub(crate) struct NshHooks;

impl Hooks for NshHooks {
    // The app menu, the window chrome and the updater belong to the desktop wave (D4). Their calls come
    // from the host functions as effects with no answer, so there is nothing to say back but a log line.
    fn refresh_menu(&self) {
        tracing::debug!("app menu refresh {NOT_YET}");
    }

    fn set_shell_chrome_open(&self, open: bool) {
        tracing::debug!(open, "shell chrome switch {NOT_YET}");
    }

    fn start_updater(&self) {
        tracing::debug!("updater {NOT_YET}");
    }

    fn update_status(&self) -> UpdateStatus {
        UpdateStatus {
            phase: "idle".to_owned(),
            version: None,
            message: Some(format!("Automatic updates {NOT_YET}.")),
            auto_download: false,
            current_version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn install_update(&self) {
        tracing::warn!("install update {NOT_YET}");
    }

    fn dismiss_update(&self) {}

    fn check_for_updates(&self) {
        tracing::warn!("check for updates {NOT_YET}");
    }

    fn set_auto_update_download(&self, _enabled: bool) {}

    // The print window is wave 4 (D3). The host function still writes the preview file and answers its
    // path; no window opens, and no preview ever counts as open (the default would claim one did).
    fn print_preview_request(&self, path: PathBuf, _title: String, _key: String) {
        tracing::warn!(path = %path.display(), "the print preview window {NOT_YET}");
    }

    fn print_preview_has_live(&self, _key: &str) -> bool {
        false
    }

    fn print_preview_live_update(&self, _key: &str, _produce: PreviewProducer, _started: Instant) {}

    fn agent_send_draft(
        &self,
        _db: &Database,
        _kind: AgentFormKind,
        _draft: &mut Value,
    ) -> Result<Value, String> {
        Err(format!("Agent control {NOT_YET}."))
    }

    fn agent_pop_command(&self) -> Option<Value> {
        None
    }

    fn prompt_for_open_path(&self, prompt: &str, answer: PathAnswer) -> bool {
        // Only a task on the engine thread can show a panel; anywhere else there is no UI to ask.
        if try_os().is_none() {
            return false;
        }
        let panel = pick_paths(OpenDialog::files().button_label(prompt));
        keep(async move {
            let choice = match panel.await {
                Ok(Some(paths)) => paths
                    .into_iter()
                    .next()
                    .map_or(PathChoice::Cancelled, PathChoice::Picked),
                Ok(None) => PathChoice::Cancelled,
                Err(error) => PathChoice::Failed(error),
            };
            answer(choice);
        });
        true
    }

    fn prompt_for_save_path(&self, directory: &Path, file_name: &str, answer: PathAnswer) -> bool {
        if try_os().is_none() {
            return false;
        }
        let panel = pick_save_path(
            SaveDialog::new()
                .file_name(file_name)
                .directory(directory)
                .button_label("Save"),
        );
        keep(async move {
            let choice = match panel.await {
                Ok(Some(path)) => PathChoice::Picked(path),
                Ok(None) => PathChoice::Cancelled,
                Err(error) => PathChoice::Failed(error),
            };
            answer(choice);
        });
        true
    }

    // Touch ID and the account password (admin sign-in, recovery) are wave 4 (D2): refusing is the honest
    // answer of a build with no prompt.
    fn os_authenticate(&self, _reason: &str) -> bool {
        tracing::warn!("the OS sign-in prompt {NOT_YET}");
        false
    }
}

/// A task waiting on a panel, and whether it has answered.
type Panel = (Spawned<()>, Rc<Cell<bool>>);

thread_local! {
    /// The tasks waiting on a panel. A dropped task is cancelled, so each is kept until it has answered.
    static PANELS: RefCell<Vec<Panel>> = const { RefCell::new(Vec::new()) };
}

/// Run `work` as a task of the engine, and keep it alive until it ends.
fn keep(work: impl Future<Output = ()> + 'static) {
    let finished = Rc::new(Cell::new(false));
    let flag = Rc::clone(&finished);
    let task = spawn(async move {
        work.await;
        flag.set(true);
    });
    PANELS.with(|panels| {
        let mut panels = panels.borrow_mut();
        panels.retain(|(_, done)| !done.get());
        panels.push((task, finished));
    });
}
