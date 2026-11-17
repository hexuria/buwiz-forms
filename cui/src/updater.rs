//! The updater driver (port of `nsh/shell/desktop/updater.ts`): the state machine, pure.
//!
//! The network, the download and the install come in through [`UpdaterApi`]. This build has no
//! update source configured (as the NSH build: nsh.json names none), so the real API answers
//! `configured() == false` and the phase stays `idle` with `NOT_CONFIGURED` — but the machine is
//! ported whole so a configured build only swaps the API.
//!
//! Commands arrive as the words `desktop::poll().update_commands` carries; each change is
//! published back with `bir::publish_update` so `update_status` answers it.

use std::time::{Duration, Instant};

/// What an update check found (nsh/updater's AppUpdate, the part used here).
pub struct FoundUpdate {
    pub version: String,
}

/// What the driver needs of the world. Blocking: the driver runs on the UI thread and a real API
/// would hop to the engine thread (the door's calls already do).
pub trait UpdaterApi {
    /// Whether the build has an update source: false means nothing is ever checked.
    fn configured(&self) -> bool;
    /// Asks the source. `Ok(None)` when the app is current; `Err` when it cannot be reached or is refused.
    fn check(&self) -> Result<Option<FoundUpdate>, String>;
    /// Downloads and verifies what the last check found.
    fn download(&self) -> Result<(), String>;
    /// Flushes unsaved work, then replaces the app and restarts it. `Ok` only on failure-to-restart paths;
    /// a refusal is `Err`.
    fn install(&self) -> Result<(), String>;
}

/// Said while no update source is configured (the same sentence Rust answers).
pub const NOT_CONFIGURED: &str =
    "Automatic updates are not configured in this build, so nothing is checked.";
/// Said when an update could not be checked or verified.
pub const VERIFY_REJECTED: &str = "The update could not be verified and was not installed.";

/// The first check after launch, and the gap between checks (the old updater's 2 s and 4 hours).
pub const LAUNCH_DELAY: Duration = Duration::from_millis(2000);
pub const CHECK_INTERVAL: Duration = Duration::from_secs(4 * 60 * 60);

/// The phases the page knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Checking,
    Downloading,
    Ready,
    Failed,
    UpToDate,
    Available,
    Installing,
}

impl Phase {
    pub fn word(self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::Checking => "checking",
            Phase::Downloading => "downloading",
            Phase::Ready => "ready",
            Phase::Failed => "failed",
            Phase::UpToDate => "up_to_date",
            Phase::Available => "available",
            Phase::Installing => "installing",
        }
    }
}

/// What the driver publishes to Rust (`desktop::UpdateReport`).
#[derive(Debug, Clone)]
pub struct Report {
    pub configured: bool,
    pub phase: Phase,
    pub version: Option<String>,
    pub message: Option<String>,
}

/// The commands the host functions queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Start,
    Check,
    Install,
    Dismiss,
    AutoOn,
    AutoOff,
}

impl Command {
    pub fn parse(word: &str) -> Option<Command> {
        Some(match word {
            "start" => Command::Start,
            "check" => Command::Check,
            "install" => Command::Install,
            "dismiss" => Command::Dismiss,
            "auto-on" => Command::AutoOn,
            "auto-off" => Command::AutoOff,
            _ => return None,
        })
    }
}

/// The error text of a failure.
fn said(error: &str) -> String {
    let text = error.trim();
    if text.is_empty() || text == "[object Object]" {
        VERIFY_REJECTED.to_owned()
    } else {
        text.to_owned()
    }
}

/// The driver. It does nothing until it is handed `Start` (the session opened) or `Check`.
pub struct Updater {
    api: Box<dyn UpdaterApi>,
    phase: Phase,
    version: Option<String>,
    message: Option<String>,
    /// The release the prompt was dismissed for.
    dismissed: Option<String>,
    /// The release that is downloaded and verified, waiting for Install now.
    downloaded: Option<String>,
    auto: bool,
    /// The next periodic check's deadline.
    next_check: Option<Instant>,
    busy: bool,
    started: bool,
    stopped: bool,
    /// The version this app is, for "is up to date".
    current_version: String,
    /// Whether the last report was published (publish on change only).
    dirty: bool,
}

impl Updater {
    pub fn new(api: Box<dyn UpdaterApi>, auto_download: bool, current_version: String) -> Self {
        Updater {
            api,
            phase: Phase::Idle,
            version: None,
            message: None,
            dismissed: None,
            downloaded: None,
            auto: auto_download,
            next_check: None,
            busy: false,
            started: false,
            stopped: false,
            current_version,
            dirty: true,
        }
    }

    pub fn report(&self) -> Report {
        if !self.api.configured() {
            return Report {
                configured: false,
                phase: Phase::Idle,
                version: None,
                message: Some(NOT_CONFIGURED.to_owned()),
            };
        }
        Report {
            configured: true,
            phase: self.phase,
            version: self.version.clone(),
            message: self.message.clone(),
        }
    }

    /// Whether a publish is owed (the first report, or a change since the last publish).
    pub fn take_dirty(&mut self) -> bool {
        std::mem::replace(&mut self.dirty, false)
    }

    fn set(&mut self, next: Phase, next_version: Option<String>, next_message: Option<String>) {
        self.phase = next;
        self.version = next_version;
        self.message = next_message;
        self.dirty = true;
    }

    /// Runs one command. Blocking: a check or a download takes a while (the real API hops engines).
    pub fn handle(&mut self, command: Command) {
        match command {
            Command::Start => {
                if self.started {
                    return;
                }
                self.started = true;
                if !self.api.configured() {
                    self.dirty = true;
                    return;
                }
                self.next_check = Some(Instant::now() + LAUNCH_DELAY);
            }
            Command::Check => self.check(true),
            Command::Install => self.install(),
            Command::Dismiss => self.dismiss(),
            Command::AutoOn | Command::AutoOff => {
                self.auto = command == Command::AutoOn;
                self.dirty = true;
                // Switching it on while a release waits downloads it, as the old updater did.
                if self.auto && self.phase == Phase::Available {
                    self.check(true);
                }
            }
        }
    }

    /// The periodic check, when its deadline passed. Call once per frame.
    pub fn tick(&mut self) {
        if self.stopped || self.busy {
            return;
        }
        if let Some(at) = self.next_check {
            if Instant::now() >= at {
                self.next_check = None;
                self.check(false);
                self.next_check = Some(Instant::now() + CHECK_INTERVAL);
            }
        }
    }

    /// Cancels the periodic check (the app is closing).
    pub fn stop(&mut self) {
        self.stopped = true;
        self.next_check = None;
    }

    /// A check, and the download it leads to. `manual`: the menu item or the page's button — it
    /// downloads whatever `auto` says and answers "up to date".
    fn check(&mut self, manual: bool) {
        if !self.api.configured() {
            self.dirty = true;
            return;
        }
        if self.busy || self.phase == Phase::Installing {
            return;
        }
        self.busy = true;
        let result = self.check_inner(manual);
        self.busy = false;
        let _ = result;
    }

    fn check_inner(&mut self, manual: bool) {
        if manual {
            self.dismissed = None;
        }
        // A release already waiting for Install now stays on offer while the periodic check asks again.
        if !(self.phase == Phase::Ready && !manual) {
            self.set(Phase::Checking, self.version.clone(), None);
        }
        let found = match self.api.check() {
            Ok(f) => f,
            Err(e) => {
                if self.phase == Phase::Ready && !manual {
                    return;
                }
                if manual {
                    self.set(Phase::Failed, None, Some(said(&e)));
                } else {
                    self.set(Phase::Idle, None, None);
                }
                return;
            }
        };
        let Some(found) = found else {
            self.downloaded = None;
            if manual {
                self.set(
                    Phase::UpToDate,
                    None,
                    Some(format!("eBIRForms {} is up to date.", self.current_version)),
                );
            } else {
                self.set(Phase::Idle, None, None);
            }
            return;
        };
        if !manual && self.dismissed.as_deref() == Some(found.version.as_str()) {
            self.set(Phase::Idle, Some(found.version), None);
            return;
        }
        // Found again: what was downloaded is still the newest, so there is nothing more to fetch.
        if Some(&found.version) == self.downloaded.as_ref() {
            self.set(Phase::Ready, Some(found.version), None);
            return;
        }
        // A manual check downloads even with the automatic download off (the old force_download).
        if !manual && !self.auto {
            self.set(
                Phase::Available,
                Some(found.version.clone()),
                Some(format!("Version {} is available.", found.version)),
            );
            return;
        }
        self.set(
            Phase::Downloading,
            Some(found.version.clone()),
            Some(format!("Downloading version {}…", found.version)),
        );
        match self.api.download() {
            Ok(()) => {
                self.downloaded = Some(found.version.clone());
                self.set(Phase::Ready, Some(found.version), None);
            }
            Err(e) => {
                self.set(Phase::Failed, Some(found.version), Some(said(&e)));
            }
        }
    }

    fn install(&mut self) {
        if self.phase != Phase::Ready || self.busy {
            return;
        }
        self.busy = true;
        let was = self.version.clone();
        self.set(
            Phase::Installing,
            was.clone(),
            Some(format!(
                "Installing version {}…",
                was.as_deref().unwrap_or("")
            )),
        );
        // install() resolves only when the swap failed; the process is gone on success.
        if let Err(e) = self.api.install() {
            self.set(Phase::Failed, was, Some(said(&e)));
        }
        self.busy = false;
    }

    fn dismiss(&mut self) {
        if matches!(self.phase, Phase::Ready | Phase::Available) {
            self.dismissed = self.version.clone();
        }
        if matches!(
            self.phase,
            Phase::Ready | Phase::Failed | Phase::UpToDate | Phase::Available
        ) {
            self.set(Phase::Idle, self.version.clone(), None);
        }
    }
}

/// This build's API: no update source is configured (the owner sets one in the build config to
/// turn updates on), so nothing is ever checked and the status says so.
pub struct UnconfiguredApi;

impl UpdaterApi for UnconfiguredApi {
    fn configured(&self) -> bool {
        false
    }
    fn check(&self) -> Result<Option<FoundUpdate>, String> {
        Ok(None)
    }
    fn download(&self) -> Result<(), String> {
        Ok(())
    }
    fn install(&self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A fake world: scripted check answers, recorded calls.
    struct Fake {
        configured: bool,
        checks: RefCell<Vec<Result<Option<String>, String>>>,
        download_error: RefCell<Option<String>>,
        install_error: RefCell<Option<String>>,
        calls: RefCell<Vec<&'static str>>,
    }
    impl Fake {
        fn new(configured: bool) -> Self {
            Fake {
                configured,
                checks: RefCell::new(vec![]),
                download_error: RefCell::new(None),
                install_error: RefCell::new(None),
                calls: RefCell::new(vec![]),
            }
        }
        fn found(version: &str) -> Self {
            let fake = Fake::new(true);
            fake.checks.borrow_mut().push(Ok(Some(version.to_owned())));
            fake
        }
        fn calls(&self) -> Vec<&'static str> {
            self.calls.borrow().clone()
        }
    }
    impl UpdaterApi for Fake {
        fn configured(&self) -> bool {
            self.configured
        }
        fn check(&self) -> Result<Option<FoundUpdate>, String> {
            self.calls.borrow_mut().push("check");
            self.checks
                .borrow_mut()
                .remove(0)
                .map(|v| v.map(|s| FoundUpdate { version: s }))
        }
        fn download(&self) -> Result<(), String> {
            self.calls.borrow_mut().push("download");
            match self.download_error.borrow().clone() {
                Some(e) => Err(e),
                None => Ok(()),
            }
        }
        fn install(&self) -> Result<(), String> {
            self.calls.borrow_mut().push("install");
            match self.install_error.borrow().clone() {
                Some(e) => Err(e),
                None => Ok(()),
            }
        }
    }
    /// So a test can keep the fake after handing it to the driver.
    impl UpdaterApi for std::rc::Rc<RefCell<Fake>> {
        fn configured(&self) -> bool {
            self.borrow().configured
        }
        fn check(&self) -> Result<Option<FoundUpdate>, String> {
            self.borrow().check()
        }
        fn download(&self) -> Result<(), String> {
            self.borrow().download()
        }
        fn install(&self) -> Result<(), String> {
            self.borrow().install()
        }
    }

    #[test]
    fn unconfigured_never_checks() {
        let mut u = Updater::new(Box::new(UnconfiguredApi), true, "1.0.0".into());
        u.handle(Command::Start);
        u.handle(Command::Check);
        assert_eq!(u.report().phase, Phase::Idle);
        assert!(!u.report().configured);
        assert_eq!(u.report().message.as_deref(), Some(NOT_CONFIGURED));
    }

    #[test]
    fn check_finds_and_downloads() {
        let fake = Fake::new(true);
        fake.checks.borrow_mut().push(Ok(Some("9.9.9".into())));
        let mut u = Updater::new(Box::new(fake), true, "1.0.0".into());
        u.handle(Command::Check);
        assert_eq!(u.report().phase, Phase::Ready);
        assert_eq!(u.report().version.as_deref(), Some("9.9.9"));
    }

    #[test]
    fn available_when_auto_off() {
        let fake = Fake::new(true);
        fake.checks.borrow_mut().push(Ok(Some("9.9.9".into())));
        let mut u = Updater::new(Box::new(fake), false, "1.0.0".into());
        u.handle(Command::Check); // manual still downloads
        assert_eq!(u.report().phase, Phase::Ready);
        // periodic path with auto off -> available
        let fake2 = Fake::new(true);
        fake2.checks.borrow_mut().push(Ok(Some("8.8.8".into())));
        let mut u2 = Updater::new(Box::new(fake2), false, "1.0.0".into());
        u2.check(false);
        assert_eq!(u2.report().phase, Phase::Available);
    }

    #[test]
    fn up_to_date_and_dismiss() {
        let fake = Fake::new(true);
        fake.checks.borrow_mut().push(Ok(None));
        let mut u = Updater::new(Box::new(fake), true, "1.0.0".into());
        u.handle(Command::Check);
        assert_eq!(u.report().phase, Phase::UpToDate);
        assert!(u.report().message.unwrap().contains("1.0.0"));
    }

    #[test]
    fn a_periodic_check_that_fails_while_a_release_waits_stays_quiet() {
        let fake = std::rc::Rc::new(RefCell::new(Fake::found("0.3.0")));
        fake.borrow_mut()
            .checks
            .borrow_mut()
            .push(Err("offline".into()));
        let mut u = Updater::new(Box::new(fake.clone()), true, "0.2.0".into());
        u.handle(Command::Check);
        assert_eq!(u.report().phase, Phase::Ready);
        // The periodic path asks again; the failure must not disturb the offer.
        u.check(false);
        assert_eq!(u.report().phase, Phase::Ready);
        assert_eq!(u.report().version.as_deref(), Some("0.3.0"));
        // A manual check that fails says why.
        fake.borrow_mut()
            .checks
            .borrow_mut()
            .push(Err("offline".into()));
        u.handle(Command::Check);
        assert_eq!(u.report().phase, Phase::Failed);
        assert_eq!(u.report().message.as_deref(), Some("offline"));
    }

    #[test]
    fn a_ready_release_is_not_downloaded_twice() {
        let fake = std::rc::Rc::new(RefCell::new(Fake::found("0.3.0")));
        fake.borrow_mut()
            .checks
            .borrow_mut()
            .push(Ok(Some("0.3.0".into())));
        fake.borrow_mut()
            .checks
            .borrow_mut()
            .push(Ok(Some("0.3.0".into())));
        let mut u = Updater::new(Box::new(fake.clone()), true, "0.2.0".into());
        u.handle(Command::Check);
        assert_eq!(fake.borrow().calls(), ["check", "download"]);
        u.check(false); // the periodic check finds the same release again
        assert_eq!(fake.borrow().calls(), ["check", "download", "check"]);
        assert_eq!(u.report().phase, Phase::Ready);
    }

    #[test]
    fn a_download_that_does_not_verify_is_failed_and_later_clears_it() {
        let fake = std::rc::Rc::new(RefCell::new(Fake::found("0.3.0")));
        *fake.borrow_mut().download_error.borrow_mut() = Some(VERIFY_REJECTED.into());
        let mut u = Updater::new(Box::new(fake), true, "0.2.0".into());
        u.handle(Command::Check);
        assert_eq!(u.report().phase, Phase::Failed);
        assert_eq!(u.report().message.as_deref(), Some(VERIFY_REJECTED));
        u.handle(Command::Dismiss);
        assert_eq!(u.report().phase, Phase::Idle);
    }

    #[test]
    fn install_only_acts_on_a_ready_release() {
        let idle = std::rc::Rc::new(RefCell::new(Fake::new(true)));
        idle.borrow_mut().checks.borrow_mut().push(Ok(None));
        let mut u = Updater::new(Box::new(idle.clone()), true, "0.2.0".into());
        u.handle(Command::Install); // nothing ready: install is not even asked
        assert!(idle.borrow().calls().is_empty());

        let fake = std::rc::Rc::new(RefCell::new(Fake::found("0.3.0")));
        *fake.borrow_mut().install_error.borrow_mut() = Some("The app cannot be replaced.".into());
        let mut u = Updater::new(Box::new(fake.clone()), true, "0.2.0".into());
        u.handle(Command::Check);
        u.handle(Command::Install);
        assert_eq!(fake.borrow().calls(), ["check", "download", "install"]);
        assert_eq!(u.report().phase, Phase::Failed);
        assert_eq!(
            u.report().message.as_deref(),
            Some("The app cannot be replaced.")
        );
    }

    #[test]
    fn later_on_a_ready_release_is_remembered_by_the_periodic_check() {
        let fake = std::rc::Rc::new(RefCell::new(Fake::found("0.3.0")));
        fake.borrow_mut()
            .checks
            .borrow_mut()
            .push(Ok(Some("0.3.0".into())));
        fake.borrow_mut()
            .checks
            .borrow_mut()
            .push(Ok(Some("0.3.0".into())));
        let mut u = Updater::new(Box::new(fake.clone()), true, "0.2.0".into());
        u.handle(Command::Check);
        u.handle(Command::Dismiss);
        assert_eq!(u.report().phase, Phase::Idle);
        // The periodic check finds it again but does not re-offer it.
        u.check(false);
        assert_eq!(u.report().phase, Phase::Idle);
        assert_eq!(u.report().version.as_deref(), Some("0.3.0"));
        // A manual check offers it again.
        u.handle(Command::Check);
        assert_eq!(u.report().phase, Phase::Ready);
    }

    #[test]
    fn auto_on_while_a_release_waits_downloads_it() {
        let fake = std::rc::Rc::new(RefCell::new(Fake::found("0.3.0")));
        fake.borrow_mut()
            .checks
            .borrow_mut()
            .push(Ok(Some("0.3.0".into())));
        let mut u = Updater::new(Box::new(fake.clone()), false, "0.2.0".into());
        u.check(false); // periodic with auto off: available, not downloaded
        assert_eq!(u.report().phase, Phase::Available);
        assert_eq!(fake.borrow().calls(), ["check"]);
        u.handle(Command::AutoOn);
        assert_eq!(u.report().phase, Phase::Ready);
        assert_eq!(fake.borrow().calls(), ["check", "check", "download"]);
    }
}
