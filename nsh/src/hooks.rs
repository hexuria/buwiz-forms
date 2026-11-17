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

pub(crate) struct NshHooks;

impl Hooks for NshHooks {
    // The app menu, the window chrome and the updater (D4): the menu, the tray and the update check are TSX
    // (`nsh/shell/desktop`); these calls land in the mailbox in `desktop.rs`, which that host reads.
    fn refresh_menu(&self) {
        crate::desktop::refresh_menu();
    }

    fn set_shell_chrome_open(&self, open: bool) {
        crate::desktop::set_shell_chrome_open(open);
    }

    fn start_updater(&self) {
        crate::desktop::start_updater();
    }

    fn update_status(&self) -> UpdateStatus {
        crate::desktop::update_status()
    }

    fn install_update(&self) {
        crate::desktop::install_update();
    }

    fn dismiss_update(&self) {
        crate::desktop::dismiss_update();
    }

    fn check_for_updates(&self) {
        crate::desktop::check_for_updates();
    }

    fn set_auto_update_download(&self, enabled: bool) {
        crate::desktop::set_auto_update_download(enabled);
    }

    // The print preview window (D3) is `print.rs` with the TSX host in `screens/print`: the hooks only delegate. With
    // no host listening, no preview counts as open.
    fn print_preview_request(&self, path: PathBuf, title: String, key: String) {
        crate::print::request(path, title, key);
    }

    fn print_preview_has_live(&self, key: &str) -> bool {
        crate::print::has_live(key)
    }

    fn print_preview_live_update(&self, key: &str, produce: PreviewProducer, started: Instant) {
        crate::print::live_update(key, produce, started);
    }

    // Agent remote control (D6, `agent.rs`): the same validate-and-queue as the UI, and the commands the
    // session queued for the frame. Without the `agent` feature they answer honestly that there is none.
    fn agent_send_draft(
        &self,
        db: &Database,
        kind: AgentFormKind,
        draft: &mut Value,
    ) -> Result<Value, String> {
        crate::agent::send_draft(db, kind, draft)
    }

    fn agent_pop_command(&self) -> Option<Value> {
        crate::agent::pop_command()
    }

    fn prompt_for_open_path(&self, prompt: &str, answer: PathAnswer) -> bool {
        pick_one(OpenDialog::files().button_label(prompt), answer)
    }

    fn prompt_for_folder(&self, prompt: &str, answer: PathAnswer) -> bool {
        // NSH's open panel in folder mode (nsh-core file_dialog.rs `OpenDialog::folders`), one folder.
        pick_one(OpenDialog::folders().button_label(prompt), answer)
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

    // Touch ID and the account password (admin recovery): `auth.rs` (false in a test environment).
    fn os_authenticate(&self, reason: &str) -> bool {
        crate::auth::os_authenticate(reason)
    }

    // The global hotkey: this app's listener registers more keys than the old shell's (space, the arrows, punctuation,
    // F13 to F20) and refuses a combo with no Control, Alt or Command. `set_global_hotkey` stores what this accepts.
    fn global_hotkey_registrable(&self, combo: &str) -> bool {
        crate::desktop::hotkey_shortcut(combo).is_some()
    }
}

/// Shows the open panel `dialog` and answers with the first path picked. False when there is no UI to ask:
/// only a task on the engine thread can show a panel.
fn pick_one(dialog: OpenDialog, answer: PathAnswer) -> bool {
    if try_os().is_none() {
        return false;
    }
    let panel = pick_paths(dialog);
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
