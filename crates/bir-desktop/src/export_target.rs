//! Where an export is written, so an export never fails silently.
//!
//! [`save_with_fallback`] (and [`folder_with_fallback`] for folder exports)
//! shows the native dialog. `rfd` returns `None` both when the user cancels
//! and, on Linux, when no dialog could be shown, so the app checks first
//! whether a dialog backend exists ([`dialog_available`]):
//!
//! - macOS / Windows: always; `None` is a cancel.
//! - Linux: `rfd` (feature `xdg-portal`) asks the
//!   `org.freedesktop.portal.Desktop` D-Bus name and falls back to `zenity`;
//!   a dialog exists when that name has an owner on the session bus or
//!   `zenity` is on `PATH`.
//!
//! With a dialog, `None` means Cancel: nothing is written and no message is
//! shown. Without one, the export goes to a fixed folder:
//!
//! - `~/Documents/eBIRForms/exports/<name>` when `~/Documents` exists,
//! - otherwise `data_dir()/exports/<name>`.
//!
//! The folder is created when missing. Callers show "Saved to <path>" with an
//! "Open folder" button ([`open_folder`]) for the returned [`SavedExport`].

use std::path::{Path, PathBuf};

/// Where an export ended up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedExport {
    /// The written file.
    pub path: PathBuf,
    /// True when the dialog gave no path and the fixed folder was used.
    pub used_fallback: bool,
}

impl SavedExport {
    /// "Saved to <path>".
    pub fn message(&self) -> String {
        format!("Saved to {}", self.path.display())
    }

    /// The folder holding the file.
    pub fn folder(&self) -> PathBuf {
        self.path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.path.clone())
    }
}

/// The fallback folder, chosen from an optional home directory and the app
/// data directory. Pure, so the rule is unit-tested.
pub fn fallback_dir(home: Option<&Path>, data_dir: &Path) -> PathBuf {
    match home.map(|home| home.join("Documents")) {
        Some(documents) if documents.is_dir() => documents.join("eBIRForms").join("exports"),
        _ => data_dir.join("exports"),
    }
}

/// The fallback file path for `file_name` (the name only; any directory part
/// is dropped).
pub fn fallback_path(home: Option<&Path>, data_dir: &Path, file_name: &str) -> PathBuf {
    let name = Path::new(file_name)
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_else(|| "export".into());
    fallback_dir(home, data_dir).join(name)
}

fn home_dir() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// What to do with the dialog's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    /// The user picked this path.
    Picked(PathBuf),
    /// No dialog could be shown: use the fixed folder.
    Fallback,
    /// The user cancelled: write nothing, show nothing.
    Cancelled,
}

/// The Cancel rule. `None` falls back only when no dialog was available.
pub fn decide(chosen: Option<PathBuf>, dialog_available: bool) -> Destination {
    match chosen {
        Some(path) => Destination::Picked(path),
        None if dialog_available => Destination::Cancelled,
        None => Destination::Fallback,
    }
}

/// Linux rule for whether `rfd` can show a dialog: the xdg portal is on the
/// session bus, or rfd's `zenity` fallback is installed.
pub fn linux_dialog_available(portal_owned: bool, zenity_on_path: bool) -> bool {
    portal_owned || zenity_on_path
}

/// Whether a native file dialog can be shown on this machine.
pub fn dialog_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        linux_dialog_available(portal_owned(), program_on_path("zenity"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

/// `NameHasOwner("org.freedesktop.portal.Desktop")` on the session bus, via
/// gio (already a dependency). Short timeout; any error means "no portal".
#[cfg(target_os = "linux")]
fn portal_owned() -> bool {
    use gio::prelude::*;
    let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) else {
        return false;
    };
    bus.call_sync(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "NameHasOwner",
        Some(&("org.freedesktop.portal.Desktop",).to_variant()),
        Some(glib::VariantTy::new("(b)").expect("valid type")),
        gio::DBusCallFlags::NONE,
        300,
        None::<&gio::Cancellable>,
    )
    .ok()
    .and_then(|reply| reply.get::<(bool,)>())
    .is_some_and(|(owned,)| owned)
}

#[cfg(target_os = "linux")]
fn program_on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

fn create_dir(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir)
        .map_err(|error| format!("Could not create {}: {error}", dir.display()))
}

/// Ask for a destination with the save dialog, falling back to the fixed
/// export folder only when no dialog exists (see the module docs), then call
/// `write` with that path. `Ok(None)` means the user cancelled and nothing
/// was written.
///
/// `write` returns an error message; it is passed through unchanged.
pub async fn save_with_fallback(
    title: &str,
    file_name: &str,
    write: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<Option<SavedExport>, String> {
    let available = dialog_available();
    let chosen = if available {
        rfd::AsyncFileDialog::new()
            .set_title(title)
            .set_file_name(file_name)
            .save_file()
            .await
            .map(|handle| handle.path().to_path_buf())
    } else {
        None
    };
    let (path, used_fallback) = match decide(chosen, available) {
        Destination::Picked(path) => (path, false),
        Destination::Cancelled => return Ok(None),
        Destination::Fallback => {
            let path = fallback_path(
                home_dir().as_deref(),
                &bir_core::platform::data_dir(),
                file_name,
            );
            if let Some(dir) = path.parent() {
                create_dir(dir)?;
            }
            (path, true)
        }
    };
    write(&path)?;
    Ok(Some(SavedExport {
        path,
        used_fallback,
    }))
}

/// Folder variant of [`save_with_fallback`]: ask for a folder, then call
/// `write` with it; it returns the file it wrote. `Ok(None)` = cancelled.
pub async fn folder_with_fallback(
    title: &str,
    write: impl FnOnce(&Path) -> Result<PathBuf, String>,
) -> Result<Option<SavedExport>, String> {
    let available = dialog_available();
    let chosen = if available {
        rfd::AsyncFileDialog::new()
            .set_title(title)
            .pick_folder()
            .await
            .map(|handle| handle.path().to_path_buf())
    } else {
        None
    };
    let (dir, used_fallback) = match decide(chosen, available) {
        Destination::Picked(dir) => (dir, false),
        Destination::Cancelled => return Ok(None),
        Destination::Fallback => {
            let dir = fallback_dir(home_dir().as_deref(), &bir_core::platform::data_dir());
            create_dir(&dir)?;
            (dir, true)
        }
    };
    let path = write(&dir)?;
    Ok(Some(SavedExport {
        path,
        used_fallback,
    }))
}

/// Open the folder holding an export in the system file manager.
pub fn open_folder(saved: &SavedExport) -> Result<(), String> {
    let folder = saved.folder();
    open::that_detached(&folder)
        .map_err(|error| format!("Could not open {}: {error}", folder.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_folder_is_used_when_it_exists() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir(home.path().join("Documents")).unwrap();
        let data = Path::new("/data");
        assert_eq!(
            fallback_path(Some(home.path()), data, "a.zip"),
            home.path()
                .join("Documents")
                .join("eBIRForms")
                .join("exports")
                .join("a.zip")
        );
    }

    #[test]
    fn data_dir_is_used_without_documents() {
        let home = tempfile::tempdir().unwrap();
        let data = Path::new("/data");
        assert_eq!(
            fallback_path(Some(home.path()), data, "a.zip"),
            data.join("exports").join("a.zip")
        );
        assert_eq!(
            fallback_path(None, data, "a.zip"),
            data.join("exports").join("a.zip")
        );
    }

    #[test]
    fn fallback_keeps_only_the_file_name() {
        let data = Path::new("/data");
        assert_eq!(
            fallback_path(None, data, "../../etc/x.zip"),
            data.join("exports").join("x.zip")
        );
    }

    #[test]
    fn cancel_in_a_real_dialog_cancels() {
        assert_eq!(decide(None, true), Destination::Cancelled);
    }

    #[test]
    fn no_dialog_falls_back() {
        assert_eq!(decide(None, false), Destination::Fallback);
    }

    #[test]
    fn a_picked_path_is_used() {
        let path = PathBuf::from("/x/a.zip");
        assert_eq!(decide(Some(path.clone()), true), Destination::Picked(path));
    }

    #[test]
    fn linux_dialog_needs_portal_or_zenity() {
        assert!(linux_dialog_available(true, false));
        assert!(linux_dialog_available(false, true));
        assert!(!linux_dialog_available(false, false));
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn macos_and_windows_always_have_a_dialog() {
        assert!(dialog_available());
    }

    #[test]
    fn fallback_dir_prefers_documents_then_data_dir() {
        let home = tempfile::tempdir().unwrap();
        let data = Path::new("/data");
        assert_eq!(fallback_dir(Some(home.path()), data), data.join("exports"));
        std::fs::create_dir(home.path().join("Documents")).unwrap();
        assert_eq!(
            fallback_dir(Some(home.path()), data),
            home.path()
                .join("Documents")
                .join("eBIRForms")
                .join("exports")
        );
        assert_eq!(fallback_dir(None, data), data.join("exports"));
    }

    #[test]
    fn message_and_folder() {
        let saved = SavedExport {
            path: PathBuf::from("/d/exports/a.zip"),
            used_fallback: true,
        };
        assert_eq!(saved.message(), "Saved to /d/exports/a.zip");
        assert_eq!(saved.folder(), PathBuf::from("/d/exports"));
    }
}
