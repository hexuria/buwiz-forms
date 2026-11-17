//! The global show/hide hotkey (port of `nsh/shell/desktop/hotkey.ts`): followed from the stored
//! setting, registered again when the setting changes, off when it is empty.
//!
//! Synchronous deps: `register` blocks until the system answers. The TS original serializes
//! register/unregister because a portal ask can take a while and the setting can change meanwhile;
//! with blocking calls that race cannot happen — there is never a register in flight while `sync`
//! runs. A real OS binding that answers late would need the pending-answer machinery from the TS
//! version; that is platform code CUI does not have yet, so the settled-state machine is what is
//! ported and tested here.

use std::collections::HashMap;

/// Where the hotkey stands: off (none stored), pending (the system has not answered yet),
/// registered (the system took it), refused (another app holds the chord), invalid (not a combo
/// this computer can register).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyStatus {
    Off,
    Pending,
    Registered,
    Refused,
    Invalid,
}

impl HotkeyStatus {
    pub fn word(self) -> &'static str {
        match self {
            HotkeyStatus::Off => "off",
            HotkeyStatus::Pending => "pending",
            HotkeyStatus::Registered => "registered",
            HotkeyStatus::Refused => "refused",
            HotkeyStatus::Invalid => "invalid",
        }
    }
}

/// What the hotkey needs of the world.
pub trait HotkeyDeps {
    /// The keystroke for a stored combo, or `None` (`desktop::hotkey_shortcut`: the host's rules).
    fn shortcut_for(&mut self, combo: &str) -> Option<String>;
    /// `GlobalShortcut.register`: `Ok(true)` when the system took the chord.
    fn register(&mut self, shortcut: &str) -> Result<bool, String>;
    /// `GlobalShortcut.unregister`.
    fn unregister(&mut self, shortcut: &str);
    /// The hotkey was pressed: show the window, or hide it.
    fn on_press(&mut self);
    /// Where a note about a refusal goes.
    fn log(&mut self, _level: &str, _message: &str) {}
    /// Told each time the status changes.
    fn on_status(&mut self, _status: HotkeyStatus) {}
}

/// The hotkey of one app.
pub struct Hotkey<D: HotkeyDeps> {
    deps: D,
    /// The combo wanted now ("" for none); the last one `sync` was given.
    wanted: String,
    /// The chord the system holds for us (registered and not given back).
    held: Option<String>,
    status: HotkeyStatus,
    stopped: bool,
}

impl<D: HotkeyDeps> Hotkey<D> {
    pub fn new(deps: D) -> Self {
        Hotkey {
            deps,
            wanted: String::new(),
            held: None,
            status: HotkeyStatus::Off,
            stopped: false,
        }
    }

    pub fn status(&self) -> HotkeyStatus {
        self.status
    }

    fn change(&mut self, next: HotkeyStatus) {
        self.status = next;
        self.deps.on_status(next);
    }

    /// Brings the system to `wanted`, re-reading it after every step.
    fn settle(&mut self) {
        loop {
            let combo = self.wanted.clone();
            let shortcut = if combo.is_empty() {
                None
            } else {
                self.deps.shortcut_for(&combo)
            };
            // Give back a chord that is not the wanted one.
            if let Some(held) = self.held.clone() {
                if Some(&held) != shortcut.as_ref() {
                    self.deps.unregister(&held);
                    self.held = None;
                }
            }
            if combo.is_empty() {
                return self.change(HotkeyStatus::Off);
            }
            let Some(shortcut) = shortcut else {
                let combo = combo.clone();
                self.deps
                    .log("warn", &format!("invalid global hotkey combo '{combo}'"));
                return self.change(HotkeyStatus::Invalid);
            };
            if self.held.as_deref() == Some(shortcut.as_str()) {
                return self.change(HotkeyStatus::Registered);
            }
            self.change(HotkeyStatus::Pending);
            let took = match self.deps.register(&shortcut) {
                Ok(took) => took,
                Err(error) => {
                    self.deps.log(
                        "warn",
                        &format!("could not register global hotkey '{combo}': {error}"),
                    );
                    false
                }
            };
            // Whatever the system took is ours to give back.
            if took {
                self.held = Some(shortcut);
            }
            if !took {
                self.deps.log(
                    "warn",
                    &format!("could not register global hotkey '{combo}'"),
                );
                return self.change(HotkeyStatus::Refused);
            }
            self.deps
                .log("info", &format!("global show hotkey registered: {combo}"));
            return self.change(HotkeyStatus::Registered);
        }
    }

    /// Follows the stored combo: "control+alt+E", or empty for none. The answer is the status once
    /// the system has settled on it.
    pub fn sync(&mut self, combo: Option<&str>) -> HotkeyStatus {
        let next = combo.unwrap_or("").trim().to_owned();
        // The same combo again changes nothing (a refused or invalid one is not asked again until
        // the setting changes).
        if next == self.wanted {
            return self.status;
        }
        self.wanted = next;
        self.settle();
        self.status
    }

    /// Gives the keystroke back (the app is closing).
    pub fn stop(&mut self) {
        self.stopped = true;
        self.wanted = String::new();
        if let Some(held) = self.held.take() {
            self.deps.unregister(&held);
        }
        self.change(HotkeyStatus::Off);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A fake GlobalShortcut: the system takes every chord but the ones the test says another app holds.
    struct World {
        held_chords: Vec<String>,
        log: Rc<RefCell<Vec<String>>>,
        registered: Rc<RefCell<HashMap<String, ()>>>,
    }
    impl World {
        fn new(held: &[&str]) -> Self {
            World {
                held_chords: held.iter().map(|s| s.to_string()).collect(),
                log: Rc::new(RefCell::new(vec![])),
                registered: Rc::new(RefCell::new(HashMap::new())),
            }
        }
        fn deps(&self) -> FakeDeps {
            FakeDeps {
                world: self,
                notes: Rc::new(RefCell::new(vec![])),
                statuses: Rc::new(RefCell::new(vec![])),
            }
        }
    }

    struct FakeDeps<'a> {
        world: &'a World,
        notes: Rc<RefCell<Vec<String>>>,
        statuses: Rc<RefCell<Vec<HotkeyStatus>>>,
    }
    impl HotkeyDeps for FakeDeps<'_> {
        fn shortcut_for(&mut self, combo: &str) -> Option<String> {
            if combo == "control+notakey" {
                return None;
            }
            Some(combo.to_lowercase().replace("control", "ctrl"))
        }
        fn register(&mut self, shortcut: &str) -> Result<bool, String> {
            self.world
                .log
                .borrow_mut()
                .push(format!("register {shortcut}"));
            if self.world.held_chords.iter().any(|c| c == shortcut) {
                return Ok(false);
            }
            self.world
                .registered
                .borrow_mut()
                .insert(shortcut.to_owned(), ());
            Ok(true)
        }
        fn unregister(&mut self, shortcut: &str) {
            self.world
                .log
                .borrow_mut()
                .push(format!("unregister {shortcut}"));
            self.world.registered.borrow_mut().remove(shortcut);
        }
        fn on_press(&mut self) {
            self.world.log.borrow_mut().push("pressed".to_owned());
        }
        fn log(&mut self, level: &str, message: &str) {
            self.notes.borrow_mut().push(format!("{level}: {message}"));
        }
        fn on_status(&mut self, status: HotkeyStatus) {
            self.statuses.borrow_mut().push(status);
        }
    }

    #[test]
    fn a_stored_combo_is_registered() {
        let w = World::new(&[]);
        let log = w.log.clone();
        let mut hotkey = Hotkey::new(w.deps());
        assert_eq!(hotkey.sync(Some("control+alt+E")), HotkeyStatus::Registered);
        assert_eq!(log.borrow().as_slice(), ["register ctrl+alt+e"]);
        assert_eq!(hotkey.status(), HotkeyStatus::Registered);
        // Pressing the chord runs the toggle: the platform binding calls `HotkeyDeps::on_press`.
        hotkey.deps.on_press();
        assert_eq!(log.borrow().last().map(String::as_str), Some("pressed"));
    }

    #[test]
    fn the_same_combo_again_changes_nothing_and_empty_turns_it_off() {
        let w = World::new(&[]);
        let log = w.log.clone();
        let mut hotkey = Hotkey::new(w.deps());
        assert_eq!(hotkey.sync(Some("control+alt+E")), HotkeyStatus::Registered);
        assert_eq!(hotkey.sync(Some("control+alt+E")), HotkeyStatus::Registered);
        assert_eq!(log.borrow().as_slice(), ["register ctrl+alt+e"]);
        assert_eq!(hotkey.sync(Some("control+alt+B")), HotkeyStatus::Registered);
        assert_eq!(
            &log.borrow()[1..],
            ["unregister ctrl+alt+e", "register ctrl+alt+b"]
        );
        assert_eq!(hotkey.sync(Some("")), HotkeyStatus::Off);
        assert_eq!(hotkey.sync(None), HotkeyStatus::Off);
        assert_eq!(hotkey.sync(Some("  ")), HotkeyStatus::Off);
        assert_eq!(&log.borrow()[3..], ["unregister ctrl+alt+b"]);
        assert!(w.registered.borrow().is_empty());
    }

    #[test]
    fn a_chord_another_app_holds_is_refused_and_not_asked_again() {
        let w = World::new(&["ctrl+alt+e"]);
        let deps = w.deps();
        let notes = deps.notes.clone();
        let mut hotkey = Hotkey::new(deps);
        assert_eq!(hotkey.sync(Some("control+alt+E")), HotkeyStatus::Refused);
        assert_eq!(hotkey.sync(Some("control+alt+E")), HotkeyStatus::Refused);
        assert_eq!(w.log.borrow().as_slice(), ["register ctrl+alt+e"]);
        assert_eq!(
            notes.borrow().as_slice(),
            ["warn: could not register global hotkey 'control+alt+E'"]
        );
        assert_eq!(hotkey.sync(Some("control+alt+H")), HotkeyStatus::Registered);
    }

    #[test]
    fn an_unregistrable_combo_is_invalid() {
        let w = World::new(&[]);
        let mut hotkey = Hotkey::new(w.deps());
        assert_eq!(hotkey.sync(Some("control+notakey")), HotkeyStatus::Invalid);
        assert!(w.log.borrow().is_empty());
    }

    #[test]
    fn statuses_are_reported_as_they_change() {
        let w = World::new(&[]);
        let deps = w.deps();
        let statuses = deps.statuses.clone();
        let mut hotkey = Hotkey::new(deps);
        hotkey.sync(Some("a"));
        hotkey.sync(Some(""));
        assert_eq!(
            statuses.borrow().as_slice(),
            [
                HotkeyStatus::Pending,
                HotkeyStatus::Registered,
                HotkeyStatus::Off
            ]
        );
    }
}
