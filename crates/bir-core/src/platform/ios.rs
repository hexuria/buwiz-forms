//! iOS implementations for bir-core platform services.

use std::path::PathBuf;

/// Returns the app's data directory inside its sandbox:
/// `$HOME/Library/Application Support/dev.goldcoders.bir`.
pub fn data_dir() -> PathBuf {
    let dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Library")
        .join("Application Support")
        .join(super::NSH_APP_ID);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Returns the temporary directory.
pub fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

/// iOS apps cannot spawn processes, so scheduled shell commands always fail.
pub async fn run_shell_command(_cmd: &str) -> Result<std::process::Output, std::io::Error> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "shell commands are not available on iOS",
    ))
}

/// See `super::prepare_app_storage`. Answers the line to log once logging
/// exists, and whether it is a warning. The marker sits in [`data_dir`],
/// which goes when the app is deleted.
pub(super) fn prepare_ios_storage() -> (bool, String) {
    use bir_submit::ios_storage::{
        OLD_ITEMS_MARKER, OsKeychain, exclude_from_backup, remove_old_items, set_file_protection,
    };
    let dir = data_dir();
    let backup = exclude_from_backup(&dir);
    let protection = set_file_protection(&dir, true);
    let keychain = (!super::keychain_disabled())
        .then_some(&OsKeychain as &dyn bir_submit::ios_storage::Backend);
    let old_items = remove_old_items(keychain, &dir.join(OLD_ITEMS_MARKER));
    let warn = backup.is_err() || protection.is_err();
    let backup = match backup {
        Ok(()) => "set".to_owned(),
        Err(error) => format!("NOT set ({error})"),
    };
    let protection = match protection {
        Ok(()) => "set".to_owned(),
        Err(error) => format!("NOT set ({error})"),
    };
    (
        warn,
        format!(
            "App storage at launch: backup exclusion {backup}; file protection {protection}; old Keychain items {old_items}"
        ),
    )
}
