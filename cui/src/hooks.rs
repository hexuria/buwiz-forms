//! What the host functions ask of a UI, answered for the CUI app.
//!
//! The file panels are real: `cui::file_dialog` is a blocking native panel (zenity/kdialog on Linux),
//! called on the thread that asked — the same thread that owns the answer callback, so the panel's
//! answer lands exactly where the host function left it. The updater, the menu, the print preview, the
//! agent control and the OS sign-in prompt delegate to the app's own modules (`desktop.rs`,
//! `updater.rs`, `print.rs`, `agent.rs`, `auth.rs`).

use std::path::{Path, PathBuf};
use std::time::Instant;

use bir_app_api::hooks::{Hooks, PathAnswer, PathChoice, PreviewProducer, UpdateStatus};
use bir_core::db::Database;
use bir_core::forms::agent::AgentFormKind;
use serde_json::Value;

pub(crate) struct CuiHooks;

impl Hooks for CuiHooks {
    // The app menu, the window chrome and the updater (D4): the menu, the tray and the update check are the
    // app's own UI; these calls land in the mailbox in `desktop.rs`, which the app reads each frame.
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

    // The print preview (D3): `print.rs` keeps the document and the live update; the preview window itself
    // is a CUI screen (`screens/print`).
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
        // Only a running app can show a panel; a headless caller (a test, the agent) gets the honest "no UI".
        if !crate::app::ui_running() {
            return false;
        }
        // A native panel on this thread: the answer callback runs before this returns, on the thread that
        // owns the database slot, as the trait asks.
        let chosen = cui::file_dialog(&cui::FileDialog {
            kind: cui::FileDialogKind::Open,
            title: Some(prompt),
            filter_name: None,
            filter: None,
            default_name: None,
        });
        let choice = match chosen {
            Some(path) => PathChoice::Picked(PathBuf::from(path)),
            None => PathChoice::Cancelled,
        };
        answer(choice);
        true
    }

    fn prompt_for_save_path(&self, directory: &Path, file_name: &str, answer: PathAnswer) -> bool {
        if !crate::app::ui_running() {
            return false;
        }
        // CUI's native save panel starts in the default folder and proposes `default_name`; the directory
        // is honored by the app's own file picker where the panel cannot take it.
        let _ = directory;
        let chosen = cui::file_dialog(&cui::FileDialog {
            kind: cui::FileDialogKind::Save,
            title: Some("Save"),
            filter_name: None,
            filter: None,
            default_name: Some(file_name),
        });
        let choice = match chosen {
            Some(path) => PathChoice::Picked(PathBuf::from(path)),
            None => PathChoice::Cancelled,
        };
        answer(choice);
        true
    }

    // Touch ID and the account password (admin recovery): `auth.rs` (false in a test environment).
    fn os_authenticate(&self, reason: &str) -> bool {
        crate::auth::os_authenticate(reason)
    }
}
