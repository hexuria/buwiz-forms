//! Desktop glue the TSX shell cannot do for itself (track D4).
//!
//! * **The mailbox.** The host functions reach the app menu, the window chrome and the updater through the
//!   [`Hooks`](bir_app_api::hooks::Hooks) trait; the answers belong to the TSX side (the menu and the
//!   tray are NSH's, the update check is the `nsh/updater` module). So this file keeps a small mailbox: the
//!   hooks write into it (a menu refresh counter, the "chrome is open" flag, the updater commands) and the
//!   TSX desktop host (`nsh/shell/desktop`) reads it with `Desktop.poll()` and writes back what the updater is
//!   doing with `Desktop.publishUpdate()`. `update_status` answers the last thing it published.
//! * **Updates are off until the owner configures them.** Without `configured` in the report the status is
//!   always "idle" with the sentence [`NOT_CONFIGURED`]: the banner never shows and nothing is ever fetched.
//! * **One copy running.** A lock file beside the database (`<database>.instance`, an exclusive file lock the
//!   system drops when the process ends, however it ends) says which process is the app for that database. The
//!   process that holds it listens on a loopback port written to `<database>.instance.port`; a second launch
//!   finds the lock taken, sends "show" to that port and exits, and the first copy's host brings its window to
//!   the front (`Desktop.poll().activate`).
//! * **The hotkey combo language.** [`hotkey_shortcut`] turns a stored combo (`control+alt+E`) into the keystroke
//!   `GlobalShortcut.register` takes (`ctrl+alt+e`), by the rules of `bir_app_api::hotkey_combo` (its `KEYS` and the
//!   NSH app's own `NSH_KEYS`: space, arrows, punctuation ...), which the Settings page and `set_global_hotkey` use too
//!   (through the hooks: [`NshHooks`](crate::hooks) asks this file), so the page and the listener agree.
//!
//! Nothing here changes the process environment, sends anything, or opens a database.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

use bir_app_api::hooks::UpdateStatus;
use bir_app_api::hotkey_combo;
use nsh_core::prelude::*;

use crate::guards;

// ------------------------------------------------------------------------------------------ the mailbox

/// The sentence the updater answers while no update source is configured.
pub const NOT_CONFIGURED: &str =
    "Automatic updates are not configured in this build, so nothing is checked.";

/// The most commands kept for a host that never polls (a headless caller): the oldest go first.
const COMMAND_CAP: usize = 32;

/// The phases the page knows (`bir.update_status().phase`).
const PHASES: [&str; 8] = [
    "idle",
    "checking",
    "downloading",
    "ready",
    "failed",
    "up_to_date",
    "available",
    "installing",
];

/// What the updater driver on the TSX side says it is doing.
#[derive(Debug, Clone, PartialEq, JsObject)]
pub struct UpdateReport {
    /// Whether this build has an update source (nsh.json "updater"). False: the status stays idle and nothing is checked.
    pub configured: bool,
    /// `idle`, `checking`, `downloading`, `ready`, `failed`, `up_to_date`, `available` or `installing`.
    pub phase: String,
    /// The release that was found, when there is one.
    pub version: Option<String>,
    /// A sentence the page can show.
    pub message: Option<String>,
}

impl UpdateReport {
    /// What is true before the TSX driver has said anything: not configured, idle.
    fn unconfigured() -> Self {
        UpdateReport {
            configured: false,
            phase: "idle".to_owned(),
            version: None,
            message: Some(NOT_CONFIGURED.to_owned()),
        }
    }
}

/// A request the host functions made of the updater, in the words the TSX driver reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpdateCommand {
    /// The session opened: check soon, then every few hours (`start_updater`).
    Start,
    /// Check now and download what is found (`check_for_updates`, the menu item).
    Check,
    /// Install the downloaded release and restart (`install_update`).
    Install,
    /// "Later": hide the prompt for this release (`dismiss_update`).
    Dismiss,
    /// "Download updates automatically" was switched on.
    AutoOn,
    /// "Download updates automatically" was switched off.
    AutoOff,
}

impl UpdateCommand {
    fn word(self) -> &'static str {
        match self {
            UpdateCommand::Start => "start",
            UpdateCommand::Check => "check",
            UpdateCommand::Install => "install",
            UpdateCommand::Dismiss => "dismiss",
            UpdateCommand::AutoOn => "auto-on",
            UpdateCommand::AutoOff => "auto-off",
        }
    }
}

/// Everything the hooks and the TSX host hand each other.
struct Mailbox {
    chrome_open: bool,
    menu_revision: u32,
    update: UpdateReport,
    /// "Download updates automatically": what the database says (read once, `auto_known`), or what the switch last set.
    auto_download: bool,
    auto_known: bool,
    commands: VecDeque<UpdateCommand>,
}

impl Mailbox {
    fn new() -> Self {
        Mailbox {
            chrome_open: false,
            menu_revision: 0,
            update: UpdateReport::unconfigured(),
            auto_download: true,
            auto_known: false,
            commands: VecDeque::new(),
        }
    }

    fn refresh_menu(&mut self) {
        self.menu_revision = self.menu_revision.wrapping_add(1);
    }

    fn set_chrome_open(&mut self, open: bool) {
        if self.chrome_open != open {
            self.chrome_open = open;
            self.refresh_menu();
        }
    }

    fn queue(&mut self, command: UpdateCommand) {
        // A second "start" before the first was read says nothing more.
        if command == UpdateCommand::Start && self.commands.contains(&command) {
            return;
        }
        if self.commands.len() == COMMAND_CAP {
            self.commands.pop_front();
        }
        self.commands.push_back(command);
    }

    /// What `bir.update_status` answers. Without an update source it is always the same honest "idle".
    fn status(&self) -> UpdateStatus {
        let report = &self.update;
        let current_version = env!("CARGO_PKG_VERSION");
        if !report.configured {
            return UpdateStatus {
                phase: "idle".to_owned(),
                version: None,
                message: Some(NOT_CONFIGURED.to_owned()),
                auto_download: self.auto_download,
                current_version,
            };
        }
        UpdateStatus {
            phase: report.phase.clone(),
            version: report.version.clone(),
            message: report.message.clone(),
            auto_download: self.auto_download,
            current_version,
        }
    }

    /// The driver's word on what it is doing. An unknown phase is refused: the page would not know it.
    fn publish(&mut self, report: UpdateReport) -> Result<(), String> {
        if !PHASES.contains(&report.phase.as_str()) {
            return Err(format!("{:?} is not an update phase.", report.phase));
        }
        self.update = report;
        Ok(())
    }

    /// "Later": the prompt goes at once (the driver is told too, to remember which release was waved off).
    fn dismiss(&mut self) {
        if matches!(
            self.update.phase.as_str(),
            "ready" | "failed" | "up_to_date" | "available"
        ) {
            self.update.phase = "idle".to_owned();
            self.update.message = None;
        }
        self.queue(UpdateCommand::Dismiss);
    }

    fn set_auto_download(&mut self, enabled: bool) {
        self.auto_download = enabled;
        self.auto_known = true;
        self.queue(if enabled {
            UpdateCommand::AutoOn
        } else {
            UpdateCommand::AutoOff
        });
    }

    fn take_commands(&mut self) -> Vec<String> {
        self.commands.drain(..).map(|c| c.word().to_owned()).collect()
    }
}

thread_local! {
    /// The engine thread's mailbox. The hooks run inside host calls and `Desktop` is called by the TSX, both on the engine thread, so
    /// one engine has one mailbox: the app has one engine, and each engine a test starts has its own (they never share a queue).
    static MAILBOX: RefCell<Mailbox> = RefCell::new(Mailbox::new());
}

/// Runs `work` on this thread's mailbox. Never call it from inside `work`.
fn mailbox<R>(work: impl FnOnce(&mut Mailbox) -> R) -> R {
    MAILBOX.with(|mailbox| work(&mut mailbox.borrow_mut()))
}

// The bodies of the hooks (`hooks.rs` delegates here). They run on the engine thread, inside a host call.

/// The app menu is rebuilt: the session, the gate or the chrome changed.
pub(crate) fn refresh_menu() {
    mailbox(Mailbox::refresh_menu);
}

/// The script is on the main window (`true`) or back on the gate.
pub(crate) fn set_shell_chrome_open(open: bool) {
    mailbox(|mailbox| mailbox.set_chrome_open(open));
}

/// The session opened: ask the updater to start.
pub(crate) fn start_updater() {
    mailbox(|mailbox| mailbox.queue(UpdateCommand::Start));
}

/// The updater's state as the page reads it.
pub(crate) fn update_status() -> UpdateStatus {
    auto_download();
    mailbox(|mailbox| mailbox.status())
}

/// `0`, `false` and `off` turn the automatic download off; anything else, a missing or blank value included, leaves it on
/// (the old updater's rule, `auto_download_enabled`).
pub(crate) fn auto_download_enabled(value: Option<&str>) -> bool {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    !matches!(value.to_ascii_lowercase().as_str(), "0" | "false" | "off")
}

/// What the database says about "Download updates automatically", on the thread that owns the database slot. The page cannot read
/// this setting itself (`get_setting` answers only the theme), so the host reports it through `update_status`.
fn stored_auto_download() -> Option<bool> {
    let handle = bir_app_api::bir_module::database_handle().ok()?;
    let db = bir_core::lock_timing::lock(&handle).ok()?;
    let value = db.get_setting(bir_app_api::hooks::AUTO_DOWNLOAD_SETTING).ok().flatten();
    Some(auto_download_enabled(value.as_deref()))
}

/// Whether newer releases are downloaded without asking: the stored setting (read once), or what the switch set last. On until the
/// database can say otherwise.
pub(crate) fn auto_download() -> bool {
    if let Some(known) = mailbox(|mailbox| mailbox.auto_known.then_some(mailbox.auto_download)) {
        return known;
    }
    // Not inside the mailbox: reading the database is a host call of its own.
    let stored = stored_auto_download();
    mailbox(|mailbox| {
        if !mailbox.auto_known
            && let Some(stored) = stored
        {
            mailbox.auto_download = stored;
            mailbox.auto_known = true;
        }
        mailbox.auto_download
    })
}

/// "Install now".
pub(crate) fn install_update() {
    mailbox(|mailbox| mailbox.queue(UpdateCommand::Install));
}

/// "Later".
pub(crate) fn dismiss_update() {
    mailbox(Mailbox::dismiss);
}

/// "Check for Updates…".
pub(crate) fn check_for_updates() {
    mailbox(|mailbox| mailbox.queue(UpdateCommand::Check));
}

/// "Download updates automatically" changed (the host already stored the setting).
pub(crate) fn set_auto_update_download(enabled: bool) {
    mailbox(|mailbox| mailbox.set_auto_download(enabled));
}

// ------------------------------------------------------------------------------- the hotkey combo language

/// The keystroke `GlobalShortcut.register` takes for a stored combo, or `None` when the combo cannot be registered.
///
/// The rules are `bir_app_api::hotkey_combo`'s (the key lists, the platform's default modifiers for a bare key),
/// and `set_global_hotkey` accepts exactly what this maps (`NshHooks::global_hotkey_registrable`). The keys are
/// `KEYS` (the old listener's: a to z, 0 to 9, F1 to F12) and `NSH_KEYS`, which only this app registers (space, the
/// arrows, punctuation, F13 to F20: [`key_name`] says how `nsh/keymap` spells each). Modifier words are the old
/// listener's: `control`/`ctrl`, `alt`/`option`/`opt`, `shift`, and `platform`/`super`/`cmd`/`meta`/`win` for Command, the
/// Windows key or Super; any other word is ignored. A combo with no Control, Alt or Command left is refused: the old
/// listener would have registered the bare key (or Shift and a letter) for the whole computer.
pub fn hotkey_shortcut(combo: &str) -> Option<String> {
    let (words, key) = hotkey_combo::split_combo(combo)?;
    let key = key_name(&key)?;
    let (mut ctrl, mut alt, mut shift, mut cmd) = (false, false, false, false);
    for word in &words {
        match word.to_ascii_lowercase().as_str() {
            "control" | "ctrl" => ctrl = true,
            "alt" | "option" | "opt" => alt = true,
            "shift" => shift = true,
            "platform" | "super" | "cmd" | "meta" | "win" => cmd = true,
            _ => {}
        }
    }
    if !(ctrl || alt || cmd) {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    for (on, name) in [(ctrl, "ctrl"), (alt, "alt"), (shift, "shift"), (cmd, "cmd")] {
        if on {
            parts.push(name.to_owned());
        }
    }
    parts.push(key);
    Some(parts.join("+"))
}

/// How `nsh/keymap` spells the key a stored word (upper case) names: the lower-case word for a letter, digit or
/// function key (`E` is `e`, `F12` is `f12`, `F13` is `f13`), a name or the character for the others. `None` for a word
/// no listener of ours registers. The punctuation keys are written as the character they type without Shift on a
/// US keyboard (`[`, `;`, `,`), `-` as `minus` and `=` as `plus`, which is what `keymap.bind` calls them.
fn key_name(word: &str) -> Option<String> {
    let name = match word {
        "SPACE" => "space",
        "UP" => "up",
        "DOWN" => "down",
        "LEFT" => "left",
        "RIGHT" => "right",
        "HOME" => "home",
        "END" => "end",
        "PAGEUP" => "pageup",
        "PAGEDOWN" => "pagedown",
        "MINUS" => "minus",
        "EQUAL" => "plus",
        "LEFTBRACKET" => "[",
        "RIGHTBRACKET" => "]",
        "BACKSLASH" => "\\",
        "SEMICOLON" => ";",
        "QUOTE" => "'",
        "COMMA" => ",",
        "PERIOD" => ".",
        "SLASH" => "/",
        "BACKTICK" => "`",
        other if hotkey_combo::KEYS.contains(&other) || hotkey_combo::NSH_KEYS.contains(&other) => {
            return Some(other.to_ascii_lowercase());
        }
        _ => return None,
    };
    Some(name.to_owned())
}

// ------------------------------------------------------------------------------------- one copy running

/// Set by the listener of the copy that is running when a second launch asks it to come forward.
static ACTIVATE: AtomicBool = AtomicBool::new(false);

/// The lock file of a database: `<database>.instance`.
pub(crate) fn lock_path(database: &Path) -> PathBuf {
    sibling(database, ".instance")
}

/// The file the running copy writes its loopback port to: `<database>.instance.port`. It is a file of its own
/// because Windows does not let another process read a file that is locked.
pub(crate) fn port_path(database: &Path) -> PathBuf {
    sibling(database, ".instance.port")
}

fn sibling(database: &Path, suffix: &str) -> PathBuf {
    let mut raw = database.as_os_str().to_owned();
    raw.push(suffix);
    PathBuf::from(raw)
}

/// This process is the app for one database: it holds the lock, and listens for a second launch.
pub(crate) struct InstanceLock {
    _lock: File,
    port_file: PathBuf,
    port: u16,
    stop: Arc<AtomicBool>,
    listener: Option<JoinHandle<()>>,
}

impl InstanceLock {
    /// The loopback port a second launch writes "show" to.
    #[cfg(test)]
    pub(crate) fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // The listener sits in accept: one connection of our own lets it see the flag.
        let _ = TcpStream::connect_timeout(
            &SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)),
            Duration::from_millis(500),
        );
        if let Some(listener) = self.listener.take() {
            let _ = listener.join();
        }
        let _ = fs::remove_file(&self.port_file);
    }
}

/// What claiming a database found.
pub(crate) enum Claim {
    /// Nobody else runs on it: this process is the app, for as long as the value lives.
    Primary(InstanceLock),
    /// Another process is the app for it. `notified`: it was asked to come to the front.
    Running { notified: bool },
    /// The lock could not be made (a read-only folder, ...): the app starts as before, with no guard.
    Unavailable(String),
}

/// Take the right to be the app for `database`, or find who has it.
pub(crate) fn claim(database: &Path) -> Claim {
    let lock = lock_path(database);
    if let Some(parent) = lock.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        return Claim::Unavailable(format!("cannot make {}: {error}", parent.display()));
    }
    let file = match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock)
    {
        Ok(file) => file,
        Err(error) => return Claim::Unavailable(format!("cannot open {}: {error}", lock.display())),
    };
    match file.try_lock() {
        Ok(()) => match start_listening(file, port_path(database)) {
            Ok(held) => Claim::Primary(held),
            Err(error) => Claim::Unavailable(error),
        },
        Err(TryLockError::WouldBlock) => Claim::Running {
            notified: ask_to_show(&port_path(database)),
        },
        Err(TryLockError::Error(error)) => {
            Claim::Unavailable(format!("cannot lock {}: {error}", lock.display()))
        }
    }
}

fn start_listening(lock: File, port_file: PathBuf) -> Result<InstanceLock, String> {
    // A port left by an earlier run is not ours to hand out.
    let _ = fs::remove_file(&port_file);
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|error| format!("cannot listen on the loopback: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("cannot read the loopback port: {error}"))?
        .port();
    fs::write(&port_file, format!("{port}\n{}\n", std::process::id()))
        .map_err(|error| format!("cannot write {}: {error}", port_file.display()))?;
    let stop = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&stop);
    let thread = std::thread::Builder::new()
        .name("ebirforms-instance".to_owned())
        .spawn(move || listen(listener, &seen))
        .map_err(|error| format!("cannot start the listener: {error}"))?;
    Ok(InstanceLock {
        _lock: lock,
        port_file,
        port,
        stop,
        listener: Some(thread),
    })
}

/// Answer second launches until told to stop: "show" asks the window to come forward.
fn listen(listener: TcpListener, stop: &AtomicBool) {
    for stream in listener.incoming() {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else { continue };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut line = String::new();
        // A line of at most 16 bytes: nothing else is ever sent.
        if BufReader::new(stream.take(16)).read_line(&mut line).is_ok() && line.trim() == "show" {
            ACTIVATE.store(true, Ordering::SeqCst);
        }
    }
}

/// Send "show" to the copy that is running. It may still be starting (it holds the lock a moment before it
/// writes its port), so this tries for about three seconds.
fn ask_to_show(port_file: &Path) -> bool {
    for _ in 0..30 {
        if let Some(port) = read_port(port_file)
            && let Ok(mut stream) = TcpStream::connect_timeout(
                &SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
                Duration::from_millis(500),
            )
            && stream.write_all(b"show\n").is_ok()
            && stream.flush().is_ok()
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

fn read_port(port_file: &Path) -> Option<u16> {
    fs::read_to_string(port_file)
        .ok()?
        .lines()
        .next()?
        .trim()
        .parse()
        .ok()
}

/// How a launch decides whether the guard applies: `BIR_SINGLE_INSTANCE` says (`1`/`on`/`true`/`yes` forces it, even in a
/// test environment; `0`/`off`/`false`/`no` turns it off); without it the guard applies to every run except a
/// test environment (`EBIR_TEST_ENV`), where every test names a scratch database of its own.
pub(crate) fn guard_wanted(setting: Option<&str>, test_environment: bool) -> bool {
    match setting.map(|value| value.trim().to_ascii_lowercase()).as_deref() {
        Some("1" | "on" | "true" | "yes") => true,
        Some("0" | "off" | "false" | "no") => false,
        _ => !test_environment,
    }
}

/// What this process knows about the guard, for `Desktop.instance()`.
static INSTANCE: OnceLock<(InstanceState, Option<String>)> = OnceLock::new();
static HELD: OnceLock<InstanceLock> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstanceState {
    Held,
    NotNeeded,
    Unavailable,
}

impl InstanceState {
    fn word(self) -> &'static str {
        match self {
            InstanceState::Held => "held",
            InstanceState::NotNeeded => "not_needed",
            InstanceState::Unavailable => "unavailable",
        }
    }
}

/// Claim `database`. True: this process is the app (or no guard applies) and goes on. False: another copy
/// has the database and was asked to come forward, so this process must end.
pub(crate) fn prepare_process_at(database: &Path) -> bool {
    match claim(database) {
        Claim::Primary(held) => {
            let _ = INSTANCE.set((InstanceState::Held, None));
            let _ = HELD.set(held);
            true
        }
        Claim::Running { notified } => {
            if !notified {
                tracing::warn!("another eBIRForms holds this database and did not answer");
            }
            false
        }
        Claim::Unavailable(why) => {
            tracing::warn!("single-instance guard unavailable: {why}");
            let _ = INSTANCE.set((InstanceState::Unavailable, Some(why)));
            true
        }
    }
}

/// The database the one-copy guard locks for this launch: the one this process opens, decided as the door decides it.
/// `None` when no guard applies: turned off or a test environment by default (`guard_wanted`); a door that makes a
/// scratch database of its own (a test environment with no `BIR_DATABASE_PATH`, even under `BIR_SINGLE_INSTANCE=1`: that
/// database is made per engine thread, later, and two copies never share one; locking `app_database_path()` here was
/// locking a file the process never opens); or the owner's data (never written beside, as the door refuses it).
fn guarded_database() -> Option<PathBuf> {
    let setting = std::env::var("BIR_SINGLE_INSTANCE").ok();
    if !guard_wanted(setting.as_deref(), bir_core::db::test_environment()) || crate::door::scratch_database_wanted() {
        return None;
    }
    let database = bir_core::db::app_database_path();
    guards::database_refusal_here(&database).is_none().then_some(database)
}

/// The launch's guard, from the registration of the native module (before any window opens): a second copy on
/// the same database tells the first to come forward and ends here.
pub(crate) fn prepare_process() {
    let Some(database) = guarded_database() else {
        let _ = INSTANCE.set((InstanceState::NotNeeded, None));
        return;
    };
    if !prepare_process_at(&database) {
        eprintln!(
            "eBIRForms is already running on {}. It was asked to come to the front.",
            database.display()
        );
        std::process::exit(0);
    }
}

// ------------------------------------------------------------------------------------- the TSX face

/// What the desktop host (TSX) reads on its tick.
#[derive(Debug, Clone, PartialEq, JsObject)]
pub struct DesktopPoll {
    /// A second launch asked this window to come to the front (true once per ask).
    pub activate: bool,
    /// The updater requests the host functions made since the last poll, oldest first: `start`, `check`, `install`, `dismiss`, `auto-on`, `auto-off`.
    pub update_commands: Vec<String>,
    /// Counts the menu refreshes the host functions asked for (sign-in, sign-out, the chrome switch).
    pub menu_revision: u32,
    /// The script last said the main window is up (`shell_set_chrome`).
    pub chrome_open: bool,
}

/// How the one-copy guard stands in this process.
#[derive(Debug, Clone, PartialEq, JsObject)]
pub struct InstanceStatus {
    /// `held` (this process is the one copy for its database), `not_needed` (a test environment, or turned off) or
    /// `unavailable` (the lock could not be made; the note says why).
    pub state: String,
    /// Why, when `state` is `unavailable`.
    pub note: Option<String>,
}

/// The desktop shell's door to Rust: what the menu, tray and updater of `nsh/shell/desktop` need and TSX cannot know.
/// Only that folder imports it.
#[module]
pub struct Desktop;

#[module]
impl Desktop {
    /// What changed since the last call: a second launch asking for the window, updater requests, menu refreshes.
    pub fn poll() -> DesktopPoll {
        mailbox(|mailbox| DesktopPoll {
            activate: ACTIVATE.swap(false, Ordering::SeqCst),
            update_commands: mailbox.take_commands(),
            menu_revision: mailbox.menu_revision,
            chrome_open: mailbox.chrome_open,
        })
    }

    /// Says what the updater is doing: `bir.update_status()` answers it. Throws for a phase the page does not know.
    pub fn publish_update(report: UpdateReport) -> Result<(), String> {
        mailbox(|mailbox| mailbox.publish(report))
    }

    /// Whether newer releases are downloaded without asking: the "Download updates automatically" setting (on when it was never set).
    pub fn auto_download() -> bool {
        auto_download()
    }

    /// The keystroke `GlobalShortcut.register` takes for a stored combo (`control+alt+E` becomes `ctrl+alt+e`), by the rules
    /// the Settings page uses; `null` when the combo cannot be registered.
    pub fn hotkey_shortcut(combo: String) -> Option<String> {
        hotkey_shortcut(&combo)
    }

    /// How the one-copy guard stands in this process.
    pub fn instance() -> InstanceStatus {
        let (state, note) = INSTANCE
            .get()
            .cloned()
            .unwrap_or((InstanceState::NotNeeded, None));
        InstanceStatus {
            state: state.word().to_owned(),
            note,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};
    use std::sync::mpsc::{self, Receiver};
    use std::sync::{Mutex, MutexGuard};
    use std::time::Instant;

    use super::*;

    /// The tests that use the process-wide "show" flag take turns.
    static SERIAL: Mutex<()> = Mutex::new(());

    fn serial() -> MutexGuard<'static, ()> {
        SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn scratch() -> (tempfile::TempDir, PathBuf) {
        let folder = tempfile::tempdir().expect("temp folder");
        let path = folder.path().join("data").join("bir_data.db");
        (folder, path)
    }

    fn report(phase: &str) -> UpdateReport {
        UpdateReport {
            configured: true,
            phase: phase.to_owned(),
            version: Some("0.3.0".to_owned()),
            message: Some("a sentence".to_owned()),
        }
    }

    // ---- the updater's mailbox

    #[test]
    fn without_an_update_source_the_status_is_one_honest_idle_sentence() {
        let mut mailbox = Mailbox::new();
        let status = mailbox.status();
        assert_eq!(status.phase, "idle");
        assert_eq!(status.version, None);
        assert_eq!(status.message.as_deref(), Some(NOT_CONFIGURED));
        assert_eq!(status.current_version, env!("CARGO_PKG_VERSION"));
        // Whatever a driver publishes while no source is configured, the page still hears "idle": no banner.
        for phase in PHASES {
            let mut said = report(phase);
            said.configured = false;
            mailbox.publish(said).expect("a known phase");
            let status = mailbox.status();
            assert_eq!(status.phase, "idle", "{phase}");
            assert_eq!(status.version, None, "{phase}");
            assert_eq!(status.message.as_deref(), Some(NOT_CONFIGURED), "{phase}");
        }
    }

    #[test]
    fn with_a_source_the_status_is_what_the_driver_published() {
        let mut mailbox = Mailbox::new();
        mailbox.publish(report("ready")).expect("published");
        let status = mailbox.status();
        assert_eq!(status.phase, "ready");
        assert_eq!(status.version.as_deref(), Some("0.3.0"));
        assert_eq!(status.message.as_deref(), Some("a sentence"));
        assert!(status.auto_download);
    }

    #[test]
    fn a_phase_the_page_does_not_know_is_refused_and_changes_nothing() {
        let mut mailbox = Mailbox::new();
        mailbox.publish(report("ready")).expect("published");
        let error = mailbox.publish(report("flying")).unwrap_err();
        assert!(error.contains("flying"), "{error}");
        assert_eq!(mailbox.status().phase, "ready");
    }

    #[test]
    fn later_hides_the_prompt_at_once_and_tells_the_driver() {
        let mut mailbox = Mailbox::new();
        for phase in ["ready", "failed", "up_to_date", "available"] {
            mailbox.publish(report(phase)).expect("published");
            mailbox.dismiss();
            let status = mailbox.status();
            assert_eq!(status.phase, "idle", "{phase}");
            assert_eq!(status.message, None, "{phase}");
            assert_eq!(mailbox.take_commands(), ["dismiss"], "{phase}");
        }
        // A download in flight is not a prompt: "Later" leaves it alone.
        mailbox.publish(report("downloading")).expect("published");
        mailbox.dismiss();
        assert_eq!(mailbox.status().phase, "downloading");
    }

    #[test]
    fn the_hooks_queue_their_commands_once_and_in_order() {
        let mut mailbox = Mailbox::new();
        mailbox.queue(UpdateCommand::Start);
        mailbox.queue(UpdateCommand::Start);
        mailbox.queue(UpdateCommand::Check);
        mailbox.set_auto_download(false);
        mailbox.set_auto_download(true);
        mailbox.queue(UpdateCommand::Install);
        assert!(mailbox.status().auto_download);
        assert_eq!(
            mailbox.take_commands(),
            ["start", "check", "auto-off", "auto-on", "install"]
        );
        assert!(mailbox.take_commands().is_empty(), "each command comes once");
    }

    #[test]
    fn a_host_that_never_polls_keeps_only_the_newest_commands() {
        let mut mailbox = Mailbox::new();
        for _ in 0..COMMAND_CAP * 2 {
            mailbox.queue(UpdateCommand::Check);
        }
        mailbox.queue(UpdateCommand::Install);
        let commands = mailbox.take_commands();
        assert_eq!(commands.len(), COMMAND_CAP);
        assert_eq!(commands.last().map(String::as_str), Some("install"));
    }

    #[test]
    fn the_menu_revision_moves_on_a_refresh_and_on_a_real_chrome_change_only() {
        let mut mailbox = Mailbox::new();
        assert_eq!((mailbox.menu_revision, mailbox.chrome_open), (0, false));
        mailbox.refresh_menu();
        assert_eq!(mailbox.menu_revision, 1);
        mailbox.set_chrome_open(true);
        assert_eq!((mailbox.menu_revision, mailbox.chrome_open), (2, true));
        mailbox.set_chrome_open(true);
        assert_eq!(mailbox.menu_revision, 2, "the same answer is no change");
        mailbox.set_chrome_open(false);
        assert_eq!((mailbox.menu_revision, mailbox.chrome_open), (3, false));
    }

    #[test]
    fn the_automatic_download_setting_is_on_unless_it_says_off() {
        for on in [None, Some(""), Some("  "), Some("1"), Some("true"), Some("on"), Some("yes")] {
            assert!(auto_download_enabled(on), "{on:?}");
        }
        for off in ["0", "false", "off", " OFF ", "False"] {
            assert!(!auto_download_enabled(Some(off)), "{off:?}");
        }
    }

    /// The host functions the page calls reach the mailbox, through the door and the installed hooks, and the status they answer
    /// is the mailbox's. (The mailbox is the test thread's own.)
    #[test]
    fn the_host_functions_reach_the_mailbox_through_the_door() {
        use serde_json::json;

        // Desktop.poll() takes the "come forward" flag, which the single-copy tests wait for: one at a time.
        let _turn = serial();
        let (folder, database) = scratch();
        let named = database.display().to_string();
        temp_env::with_vars(
            [
                ("BUWIZ_NO_KEYCHAIN", Some("1")),
                ("EBIR_TEST_ENV", Some("1")),
                ("BIR_AUTH_BYPASS", Some("1")),
                ("BIR_SFTP_DRY_RUN", Some("1")),
                ("BIR_SFTP_LIVE", None),
                ("BIR_SFTP_HOST", None),
                ("BIR_DATABASE_PATH", Some(named.as_str())),
            ],
            || {
                bir_core::db::set_database_path_override(None);
                guards::with_home_for_test(folder.path(), || {
                    mailbox(|mailbox| *mailbox = Mailbox::new());
                    // A build with no update source: idle, with the sentence, and the setting reads as on (never set).
                    let status = crate::door::call("update_status", vec![]).expect("update_status");
                    assert_eq!(status["phase"], "idle");
                    assert_eq!(status["version"], serde_json::Value::Null);
                    assert_eq!(status["message"], NOT_CONFIGURED);
                    assert_eq!(status["auto_download"], true);
                    assert_eq!(status["current_version"], env!("CARGO_PKG_VERSION"));

                    // The switch: stored by the host function, passed to the hook, answered by the status, queued for the driver.
                    crate::door::call("set_auto_update_download", vec![json!(false)]).expect("off");
                    let status = crate::door::call("update_status", vec![]).expect("update_status");
                    assert_eq!(status["auto_download"], false);
                    // A restart forgets the mailbox; the database remembers.
                    mailbox(|mailbox| *mailbox = Mailbox::new());
                    let status = crate::door::call("update_status", vec![]).expect("update_status");
                    assert_eq!(status["auto_download"], false, "read back from the database");
                    assert!(!Desktop::auto_download());

                    crate::door::call("check_for_updates", vec![]).expect("check");
                    crate::door::call("install_update", vec![]).expect("install");
                    crate::door::call("dismiss_update", vec![]).expect("dismiss");
                    crate::door::call("shell_set_chrome", vec![json!(true)]).expect("chrome");

                    let seen = Desktop::poll();
                    assert_eq!(seen.update_commands, ["check", "install", "dismiss"]);
                    assert!(seen.chrome_open);
                    assert!(seen.menu_revision >= 1, "the chrome switch and the refresh were counted");
                    let again = Desktop::poll();
                    assert!(again.update_commands.is_empty(), "each command comes once");
                    assert_eq!(again.menu_revision, seen.menu_revision);

                    // The driver says a release is downloaded: with a source configured the page hears it.
                    Desktop::publish_update(report("ready")).expect("published");
                    let status = crate::door::call("update_status", vec![]).expect("update_status");
                    assert_eq!(status["phase"], "ready");
                    assert_eq!(status["version"], "0.3.0");
                    mailbox(|mailbox| *mailbox = Mailbox::new());
                });
                bir_core::db::set_database_path_override(None);
            },
        );
    }

    // ---- the hotkey combo language

    #[test]
    fn a_stored_combo_becomes_the_keystroke_nsh_registers() {
        for (combo, shortcut) in [
            ("control+alt+E", "ctrl+alt+e"),
            ("Control+Option+e", "ctrl+alt+e"),
            ("ctrl+opt+F12", "ctrl+alt+f12"),
            ("super+shift+F12", "shift+cmd+f12"),
            ("platform+shift+B", "shift+cmd+b"),
            ("win+alt+9", "alt+cmd+9"),
            ("control+alt+shift+cmd+H", "ctrl+alt+shift+cmd+h"),
            (" control + alt + E ", "ctrl+alt+e"),
        ] {
            assert_eq!(hotkey_shortcut(combo).as_deref(), Some(shortcut), "{combo}");
        }
        // A bare key means the platform's default modifiers, as for the page and the old listener.
        let bare = hotkey_shortcut("e");
        if cfg!(target_os = "macos") {
            assert_eq!(bare.as_deref(), Some("ctrl+alt+e"));
        } else {
            assert_eq!(bare.as_deref(), Some("shift+cmd+e"));
        }
    }

    #[test]
    fn a_combo_nothing_can_register_is_refused() {
        for combo in [
            "",
            "  ",
            " + ",
            "control+notakey",
            // The character is never a stored key (+ is the separator); a word of the old list's neighbours is no key.
            "control+-",
            "control+=",
            "control+[",
            "control+F21",
            "control+F24",
            "control+TAB",
            "control+ENTER",
            "control+ESCAPE",
            "control+BACKSPACE",
            "control+DELETE",
            "control+INSERT",
            "control+PLUS",
            "control+SPACEBAR",
            "control+ARROWLEFT",
            "control+BRACKETLEFT",
            // No Control, Alt or Command: the bare key, or Shift and a key, would be taken from every app.
            "shift+E",
            "banana+E",
        ] {
            assert_eq!(hotkey_shortcut(combo), None, "{combo:?}");
        }
    }

    /// The page stores what `set_global_hotkey` accepts (`is_registrable`); this file must map all of it to a
    /// keystroke NSH reads, on every platform's reading of `mod`.
    #[test]
    fn every_combo_the_page_can_store_is_a_keystroke_nsh_reads() {
        let sets = [
            "control+alt",
            "control+shift",
            "alt+shift",
            "super+shift",
            "control+alt+shift+super",
            "cmd",
        ];
        for key in hotkey_combo::KEYS {
            for mods in sets {
                let combo = format!("{mods}+{key}");
                assert!(hotkey_combo::is_registrable(&combo), "{combo}");
                let shortcut = hotkey_shortcut(&combo).unwrap_or_else(|| panic!("{combo} maps"));
                for mod_is_meta in [true, false] {
                    nsh_core::desktop::Shortcut::parse(&shortcut, mod_is_meta)
                        .unwrap_or_else(|error| panic!("{combo} -> {shortcut}: {error:?}"));
                }
            }
        }
    }

    /// The keys only this app registers (`NSH_KEYS`) map with every modifier set, to the spelling `nsh/keymap` and
    /// `GlobalShortcut.register` read, and to one keystroke each. The old shell's rule (`is_registrable`) still refuses
    /// every one of them: it is the old listener's, and that listener cannot register them.
    #[test]
    fn every_nsh_only_key_is_a_keystroke_nsh_reads_and_the_old_rule_still_refuses_it() {
        let sets = ["control+alt", "alt", "cmd", "super+shift", "control+alt+shift+super", "shift+control"];
        let mut seen = std::collections::BTreeSet::new();
        for key in hotkey_combo::NSH_KEYS {
            for mods in sets {
                let combo = format!("{mods}+{key}");
                assert!(!hotkey_combo::is_registrable(&combo), "{combo}: the old listener's rule");
                let shortcut = hotkey_shortcut(&combo).unwrap_or_else(|| panic!("{combo} maps"));
                for mod_is_meta in [true, false] {
                    let read = nsh_core::desktop::Shortcut::parse(&shortcut, mod_is_meta)
                        .unwrap_or_else(|error| panic!("{combo} -> {shortcut}: {error:?}"));
                    assert!(read.ctrl || read.alt || read.meta || read.shift, "{combo} -> {shortcut}");
                }
                // The keymap reads the same text as the keystroke the recorder binds.
                nsh_core::keymap::Keystroke::parse(&shortcut, nsh_core::platform::Os::MacOs)
                    .unwrap_or_else(|error| panic!("{combo} -> {shortcut}: {error:?}"));
            }
            let alone = key_name(key).expect("a name");
            assert!(seen.insert(alone.clone()), "{key} -> {alone}: two words, one key");
        }
        assert_eq!(seen.len(), hotkey_combo::NSH_KEYS.len());
    }

    /// The spelling of each word of `NSH_KEYS`: what the recorder binds and what the page's `comboToKeystroke` writes
    /// (`lib/settings_model.ts`, the same table; its test lists the same pairs).
    #[test]
    fn the_nsh_only_keys_have_the_names_the_page_writes() {
        for (word, name) in [
            ("SPACE", "space"),
            ("UP", "up"),
            ("DOWN", "down"),
            ("LEFT", "left"),
            ("RIGHT", "right"),
            ("HOME", "home"),
            ("END", "end"),
            ("PAGEUP", "pageup"),
            ("PAGEDOWN", "pagedown"),
            ("MINUS", "minus"),
            ("EQUAL", "plus"),
            ("LEFTBRACKET", "["),
            ("RIGHTBRACKET", "]"),
            ("BACKSLASH", "\\"),
            ("SEMICOLON", ";"),
            ("QUOTE", "'"),
            ("COMMA", ","),
            ("PERIOD", "."),
            ("SLASH", "/"),
            ("BACKTICK", "`"),
            ("F13", "f13"),
            ("F20", "f20"),
        ] {
            assert!(hotkey_combo::NSH_KEYS.contains(&word), "{word}");
            assert_eq!(key_name(word).as_deref(), Some(name), "{word}");
            assert_eq!(
                hotkey_shortcut(&format!("control+alt+{word}")),
                Some(format!("ctrl+alt+{name}")),
                "{word}"
            );
        }
        // Lower case in a stored combo, as for every key.
        assert_eq!(hotkey_shortcut("Control+Alt+space").as_deref(), Some("ctrl+alt+space"));
        assert_eq!(hotkey_shortcut("alt+pagedown").as_deref(), Some("alt+pagedown"));
        // Shift and a key is still no hotkey: it would be taken from every app (a bare key is the platform's default modifiers).
        for combo in ["space", "SPACE", "shift+SPACE", "shift+LEFT", "F13", "shift+F20"] {
            // A bare key means the platform's default modifiers (control+alt, or super+shift: both have a Control, Alt or
            // Command), so it is a hotkey; Shift and a key, with nothing else, is not.
            assert_eq!(hotkey_shortcut(combo).is_some(), !combo.starts_with("shift+"), "{combo}");
        }
    }

    /// The four chords the Settings page offers (`hotkey_presets` in bir-app-api) round-trip: stored, mapped, read.
    #[test]
    fn the_settings_presets_round_trip() {
        let prefix = if cfg!(target_os = "macos") { "control+alt" } else { "super+shift" };
        for key in ["E", "B", "H", "F12"] {
            let stored = format!("{prefix}+{key}");
            let shortcut = hotkey_shortcut(&stored).expect("a preset maps");
            assert!(shortcut.ends_with(&key.to_ascii_lowercase()), "{stored} -> {shortcut}");
            let read = nsh_core::desktop::Shortcut::parse(&shortcut, cfg!(target_os = "macos"))
                .expect("NSH reads it");
            // The modifiers survived the trip: the keystroke has as many as the combo has words.
            let count = [read.ctrl, read.alt, read.shift, read.meta]
                .into_iter()
                .filter(|on| *on)
                .count();
            assert_eq!(count, 2, "{stored} -> {shortcut}");
        }
    }

    // ---- one copy running, in one process

    #[test]
    fn when_the_guard_applies() {
        // Not set: every run but a test environment.
        assert!(guard_wanted(None, false));
        assert!(!guard_wanted(None, true));
        // Set: the setting decides, in either environment.
        for on in ["1", "on", "true", "YES", " 1 "] {
            assert!(guard_wanted(Some(on), true), "{on:?}");
            assert!(guard_wanted(Some(on), false), "{on:?}");
        }
        for off in ["0", "off", "false", "No"] {
            assert!(!guard_wanted(Some(off), false), "{off:?}");
            assert!(!guard_wanted(Some(off), true), "{off:?}");
        }
        // Anything else is not an answer.
        assert!(guard_wanted(Some("maybe"), false));
        assert!(!guard_wanted(Some(""), true));
    }

    /// The guard locks the database this process opens. A test environment with no `BIR_DATABASE_PATH` opens a scratch
    /// database the door makes per engine thread, so even `BIR_SINGLE_INSTANCE=1` locks nothing there (it used to lock
    /// `app_database_path()`, a file the process never opens). Named, the named file is locked. Nothing is claimed here:
    /// only the decision is asked.
    #[test]
    fn the_guard_locks_the_database_the_door_opens() {
        let _turn = serial();
        let (folder, database) = scratch();
        let named = database.display().to_string();
        let decide = |single: Option<&str>, test_env: Option<&str>, path: Option<&str>| {
            let vars = [("BIR_SINGLE_INSTANCE", single), ("EBIR_TEST_ENV", test_env), ("BIR_DATABASE_PATH", path)];
            temp_env::with_vars(vars, || {
                bir_core::db::set_database_path_override(None);
                guards::with_home_for_test(folder.path(), guarded_database)
            })
        };
        for test_env in ["1", "true"] {
            let on = Some(test_env);
            assert_eq!(decide(Some("1"), on, None), None, "{test_env}: the door makes a scratch database");
            assert_eq!(decide(Some("1"), on, Some("")), None, "{test_env}: an empty path names nothing");
            assert_eq!(decide(Some("1"), on, Some(&named)), Some(database.clone()), "{test_env}: the named database");
            assert_eq!(decide(None, on, Some(&named)), None, "{test_env}: not guarded by default");
        }
        // Not a test environment: the database it opens.
        assert_eq!(decide(None, Some("0"), Some(&named)), Some(database.clone()));
        assert!(!database.parent().unwrap().exists(), "deciding wrote nothing");
    }

    #[test]
    fn the_first_claim_is_the_app_and_a_second_asks_it_to_show() {
        let _turn = serial();
        let (_folder, database) = scratch();
        ACTIVATE.store(false, Ordering::SeqCst);
        let Claim::Primary(first) = claim(&database) else {
            panic!("an empty folder: the first claim is the app");
        };
        assert!(lock_path(&database).exists());
        assert_eq!(read_port(&port_path(&database)), Some(first.port()));

        match claim(&database) {
            Claim::Running { notified } => assert!(notified, "the first copy answered"),
            _ => panic!("a second claim on the same database finds the first"),
        }
        let started = Instant::now();
        while !ACTIVATE.swap(false, Ordering::SeqCst) {
            assert!(started.elapsed() < Duration::from_secs(5), "the first copy heard it");
            std::thread::sleep(Duration::from_millis(10));
        }

        // Another database is another app.
        let (_other_folder, other) = scratch();
        assert!(matches!(claim(&other), Claim::Primary(_)));

        // The first copy ends: its lock and its port go, and the next claim is the app again.
        drop(first);
        assert!(!port_path(&database).exists());
        assert!(matches!(claim(&database), Claim::Primary(_)));
    }

    #[test]
    fn a_lock_file_nobody_holds_is_taken_over() {
        let (_folder, database) = scratch();
        fs::create_dir_all(database.parent().unwrap()).unwrap();
        // What a crashed copy leaves: the files, and a port nobody listens on.
        fs::write(lock_path(&database), "").unwrap();
        fs::write(port_path(&database), "9\n1\n").unwrap();
        let Claim::Primary(held) = claim(&database) else {
            panic!("the system dropped the dead copy's lock");
        };
        assert_eq!(read_port(&port_path(&database)), Some(held.port()));
    }

    #[test]
    fn a_folder_that_cannot_be_made_leaves_the_app_unguarded_not_stopped() {
        let (folder, _) = scratch();
        let blocker = folder.path().join("a-file");
        fs::write(&blocker, "x").unwrap();
        match claim(&blocker.join("inside").join("bir_data.db")) {
            Claim::Unavailable(why) => assert!(why.contains("cannot"), "{why}"),
            _ => panic!("no folder, no lock"),
        }
    }

    #[test]
    fn a_launch_on_the_owners_folders_writes_nothing_there() {
        let _turn = serial();
        let home = tempfile::tempdir().expect("home");
        for relative in [
            "Library/Group Containers/group.dev.goldcoders.bir",
            "BuwizNsh/data",
            "BuwizNsh/migration",
        ] {
            let database = home.path().join(relative).join("bir_data.db");
            let named = database.display().to_string();
            temp_env::with_vars(
                [
                    ("BIR_SINGLE_INSTANCE", Some("1")),
                    ("BIR_DATABASE_PATH", Some(named.as_str())),
                ],
                || guards::with_home_for_test(home.path(), prepare_process),
            );
            assert!(!database.parent().unwrap().exists(), "{relative}: no folder, no lock file");
        }
    }

    // ---- one copy running, in two processes

    /// Not a test: the tests below start this binary again, with these variables, to play a second process.
    #[test]
    fn child_process() {
        let (Some(database), Ok(mode)) = (
            std::env::var_os("NSH_DESKTOP_TEST_DB"),
            std::env::var("NSH_DESKTOP_TEST_MODE"),
        ) else {
            return;
        };
        let database = PathBuf::from(database);
        let say = |text: &str| {
            println!("CHILD {text}");
            let _ = std::io::stdout().flush();
        };
        match mode.as_str() {
            "hold" => {
                let Claim::Primary(held) = claim(&database) else {
                    say("not-the-app");
                    return;
                };
                say("held");
                std::thread::spawn(move || {
                    loop {
                        if ACTIVATE.swap(false, Ordering::SeqCst) {
                            say("activated");
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                });
                // The parent ends this copy by closing its input.
                let mut line = String::new();
                let _ = std::io::stdin().read_line(&mut line);
                drop(held);
                say("released");
            }
            "launch" => {
                say("began");
                prepare_process();
                say("survived");
            }
            _ => {}
        }
    }

    struct Child2 {
        child: Child,
        lines: Receiver<String>,
    }

    impl Child2 {
        fn start(mode: &str, database: &Path, env: &[(&str, &str)]) -> Child2 {
            let mut command = Command::new(std::env::current_exe().expect("the test binary"));
            command
                .args(["--exact", "desktop::tests::child_process", "--nocapture", "--test-threads=1"])
                .env("NSH_DESKTOP_TEST_DB", database)
                .env("NSH_DESKTOP_TEST_MODE", mode)
                .env("BUWIZ_NO_KEYCHAIN", "1")
                .env_remove("BIR_SINGLE_INSTANCE")
                .env_remove("EBIR_TEST_ENV")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            for (name, value) in env {
                command.env(name, value);
            }
            let mut child = command.spawn().expect("starts the test binary again");
            let stdout = child.stdout.take().expect("piped");
            let (sender, lines) = mpsc::channel();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    // libtest starts a line with the test's name, so the first word can follow it.
                    if let Some(at) = line.find("CHILD ") {
                        let _ = sender.send(line[at + "CHILD ".len()..].to_owned());
                    }
                }
            });
            Child2 { child, lines }
        }

        /// The next thing the child said, or a failure after ten seconds.
        fn said(&self) -> String {
            self.lines
                .recv_timeout(Duration::from_secs(10))
                .expect("the other process said something in time")
        }

        /// Closes its input, and waits for it to end.
        fn finish(mut self) -> (bool, Vec<String>) {
            drop(self.child.stdin.take());
            let status = self.child.wait().expect("the other process ends");
            (status.success(), self.lines.try_iter().collect())
        }
    }

    #[test]
    fn a_second_process_finds_the_first_asks_it_to_show_and_takes_over_when_it_ends() {
        let (_folder, database) = scratch();
        let first = Child2::start("hold", &database, &[]);
        assert_eq!(first.said(), "held");

        match claim(&database) {
            Claim::Running { notified } => assert!(notified),
            _ => panic!("the other process is the app"),
        }
        assert_eq!(first.said(), "activated", "the first process heard the second");

        let (ended_well, rest) = first.finish();
        assert!(ended_well);
        assert_eq!(rest, ["released"]);
        assert!(
            matches!(claim(&database), Claim::Primary(_)),
            "its lock went with it"
        );
    }

    #[test]
    fn a_second_launch_ends_without_starting_and_the_first_comes_forward() {
        let _turn = serial();
        let (_folder, database) = scratch();
        ACTIVATE.store(false, Ordering::SeqCst);
        let Claim::Primary(_first) = claim(&database) else {
            panic!("the first is the app");
        };
        let path = database.display().to_string();
        let second = Child2::start(
            "launch",
            &database,
            &[("BIR_SINGLE_INSTANCE", "1"), ("BIR_DATABASE_PATH", &path)],
        );
        let (ended_well, said) = second.finish();
        assert!(ended_well, "a second launch ends quietly, with success");
        assert_eq!(said, ["began"], "it started, and never went on to start the app");
        let started = Instant::now();
        while !ACTIVATE.swap(false, Ordering::SeqCst) {
            assert!(started.elapsed() < Duration::from_secs(5), "the first heard the second launch");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn a_launch_with_nobody_running_goes_on_and_a_test_environment_is_never_stopped() {
        let _turn = serial();
        let (_folder, database) = scratch();
        let path = database.display().to_string();
        // Nobody holds the database: the launch is the app.
        let alone = Child2::start(
            "launch",
            &database,
            &[("BIR_SINGLE_INSTANCE", "1"), ("BIR_DATABASE_PATH", &path)],
        );
        assert_eq!(alone.said(), "began");
        assert_eq!(alone.said(), "survived");
        let (ended_well, _) = alone.finish();
        assert!(ended_well);

        // Somebody does, but this launch is a test environment: no guard, no stop.
        let Claim::Primary(_first) = claim(&database) else {
            panic!("the first is the app");
        };
        let test_run = Child2::start(
            "launch",
            &database,
            &[("EBIR_TEST_ENV", "1"), ("BIR_DATABASE_PATH", &path)],
        );
        assert_eq!(test_run.said(), "began");
        assert_eq!(test_run.said(), "survived");
        let (ended_well, _) = test_run.finish();
        assert!(ended_well);
    }
}
