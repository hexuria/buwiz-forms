//! Where an export is written, so an export never fails silently.
//!
//! [`save_with_fallback`] first shows the native save dialog. On Linux without
//! an xdg file-dialog portal `rfd` returns `None`, exactly as it does when the
//! user cancels; the two cannot be told apart, so a `None` always falls back
//! to a fixed folder:
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

/// Ask for a destination with the save dialog, falling back to the fixed
/// export folder (see the module docs), then call `write` with that path.
///
/// `write` returns an error message; it is passed through unchanged.
pub async fn save_with_fallback(
    title: &str,
    file_name: &str,
    write: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<SavedExport, String> {
    let chosen = rfd::AsyncFileDialog::new()
        .set_title(title)
        .set_file_name(file_name)
        .save_file()
        .await
        .map(|handle| handle.path().to_path_buf());
    let (path, used_fallback) = match chosen {
        Some(path) => (path, false),
        None => {
            let path = fallback_path(
                home_dir().as_deref(),
                &bir_core::platform::data_dir(),
                file_name,
            );
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)
                    .map_err(|error| format!("Could not create {}: {error}", dir.display()))?;
            }
            (path, true)
        }
    };
    write(&path)?;
    Ok(SavedExport {
        path,
        used_fallback,
    })
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
    fn message_and_folder() {
        let saved = SavedExport {
            path: PathBuf::from("/d/exports/a.zip"),
            used_fallback: true,
        };
        assert_eq!(saved.message(), "Saved to /d/exports/a.zip");
        assert_eq!(saved.folder(), PathBuf::from("/d/exports"));
    }
}
