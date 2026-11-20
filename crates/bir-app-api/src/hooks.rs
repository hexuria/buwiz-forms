//! What the host functions need from a UI, behind one trait.
//!
//! The host functions in this crate run with no window. Where one used to
//! reach into the gpui host (the app menu, the updater, the print preview
//! window, the file panels, the OS sign-in prompt, the agent command queue),
//! it now calls [`hooks`]. A host installs its implementation once per
//! process with [`install_hooks`]; until then a do-nothing default answers,
//! so tests and a UI-free caller run unchanged.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use bir_core::db::Database;
use bir_core::forms::agent::AgentFormKind;
use serde_json::Value;

/// The settings key for "download updates without asking". The updater and
/// `set_auto_update_download` share it.
pub const AUTO_DOWNLOAD_SETTING: &str = "auto_update_download";

/// What `bir.update_status` reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateStatus {
    /// `idle`, `checking`, `downloading`, `ready`, `failed`, `up_to_date`,
    /// `available`, or `installing`.
    pub phase: String,
    /// The release that was found, when there is one.
    pub version: Option<String>,
    /// A sentence the page can show.
    pub message: Option<String>,
    /// Whether a newer release is downloaded without asking first.
    pub auto_download: bool,
    /// The version compiled into this binary.
    pub current_version: &'static str,
}

/// Builds the static print document for a live preview update (may run on a
/// background thread).
pub type PreviewProducer = Box<dyn FnOnce() -> String + Send>;

/// What a file panel answered.
#[derive(Debug)]
pub enum PathChoice {
    Picked(PathBuf),
    /// Dismissed, or the panel went away without answering.
    Cancelled,
    /// The panel could not open; the sentence says why.
    Failed(String),
}

/// Called once with the panel's answer, on the thread that owns the
/// database slot and outside any UI borrow.
pub type PathAnswer = Box<dyn FnOnce(PathChoice) + 'static>;

/// A platform-owned download, answering with its user-visible destination.
pub type DownloadExport =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>>;

/// Writes an authorized archive synchronously on the database's owning thread.
pub type ArchiveWriter<'a> = dyn FnMut(&Path) -> Result<(), String> + 'a;

/// Everything the host functions ask of a UI. Every method has a default
/// that does nothing (or answers "no"), which is what runs without a host.
pub trait Hooks: Send + Sync + 'static {
    /// Rebuild the app menu to match the gate and the chrome.
    fn refresh_menu(&self) {}
    /// The script is on the main window (`true`) or back on the gate.
    fn set_shell_chrome_open(&self, _open: bool) {}

    /// Start the updater after sign-in.
    fn start_updater(&self) {}
    fn update_status(&self) -> UpdateStatus {
        UpdateStatus {
            phase: "idle".to_string(),
            version: None,
            message: None,
            auto_download: true,
            current_version: env!("CARGO_PKG_VERSION"),
        }
    }
    fn install_update(&self) {}
    fn dismiss_update(&self) {}
    fn check_for_updates(&self) {}
    fn set_auto_update_download(&self, _enabled: bool) {}

    /// Open (or refresh) the print preview for `key` on the file at `path`.
    fn print_preview_request(&self, _path: PathBuf, _title: String, key: String) {
        DEFAULT_LIVE_KEYS.with(|keys| keys.borrow_mut().insert(key));
    }
    /// Whether a preview for `key` is open and follows edits.
    fn print_preview_has_live(&self, key: &str) -> bool {
        DEFAULT_LIVE_KEYS.with(|keys| keys.borrow().contains(key))
    }
    /// Hand the open preview for `key` a new document.
    fn print_preview_live_update(&self, _key: &str, _produce: PreviewProducer, _started: Instant) {}

    /// Run the page's Validate on `draft` and queue it (the agent's send).
    fn agent_send_draft(
        &self,
        _db: &Database,
        _kind: AgentFormKind,
        _draft: &mut Value,
    ) -> Result<Value, String> {
        Err("This build has no agent control.".to_string())
    }
    /// The next UI command the agent host queued for the script.
    fn agent_pop_command(&self) -> Option<Value> {
        None
    }

    /// Show an open-file panel titled `prompt`; `answer` gets the result.
    /// False when no UI is running to show it.
    fn prompt_for_open_path(&self, _prompt: &str, _answer: PathAnswer) -> bool {
        false
    }
    /// Show an open panel that picks one folder, its button labelled
    /// `prompt`; `answer` gets the result. False when no UI is running to
    /// show it.
    fn prompt_for_folder(&self, _prompt: &str, _answer: PathAnswer) -> bool {
        false
    }
    /// Show a save panel in `directory` proposing `file_name`.
    /// False when no UI is running to show it.
    fn prompt_for_save_path(
        &self,
        _directory: &Path,
        _file_name: &str,
        _answer: PathAnswer,
    ) -> bool {
        false
    }

    /// Platforms without a save panel may stage the archive using `write`, then
    /// publish it asynchronously. `write` must run before this method returns.
    /// `None` keeps the ordinary save-panel flow.
    fn export_profile_download(
        &self,
        _file_name: &str,
        _write: &mut ArchiveWriter<'_>,
    ) -> Option<DownloadExport> {
        None
    }

    /// Ask the OS to confirm the signed-in user (Touch ID, Windows Hello).
    /// Runs on its own thread; blocking is fine.
    fn os_authenticate(&self, _reason: &str) -> bool {
        false
    }

    /// Whether this host's listener can register `combo` as the global show
    /// hotkey: `set_global_hotkey` stores only a combo this accepts. The
    /// default is the shared rule, [`crate::hotkey_combo::is_registrable`],
    /// which is the old shell's listener's; a host with a listener that takes
    /// more (or fewer) keys says so here.
    fn global_hotkey_registrable(&self, combo: &str) -> bool {
        crate::hotkey_combo::is_registrable(combo)
    }
}

thread_local! {
    /// The default's stand-in for the preview window's open keys: a preview
    /// requested without a UI still counts as open, as it did in the old
    /// host's tests.
    static DEFAULT_LIVE_KEYS: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
}

struct NoHooks;

impl Hooks for NoHooks {}

static HOOKS: OnceLock<Box<dyn Hooks>> = OnceLock::new();

/// Install the host's hooks. Only the first call per process wins; a later
/// one answers false and changes nothing.
pub fn install_hooks(hooks: impl Hooks) -> bool {
    HOOKS.set(Box::new(hooks)).is_ok()
}

/// The installed hooks, or the do-nothing default.
pub fn hooks() -> &'static dyn Hooks {
    match HOOKS.get() {
        Some(hooks) => hooks.as_ref(),
        None => &NoHooks,
    }
}
