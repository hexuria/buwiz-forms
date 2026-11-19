//! Android services supplied by the embedding app.

use std::path::PathBuf;
use std::sync::OnceLock;

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Sets the app's private data directory before any bir-core service starts.
/// Repeated calls may name the same directory, but cannot move an open database.
pub fn set_data_dir(path: PathBuf) -> std::io::Result<()> {
    if !path.is_absolute() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "The Android data directory must be absolute.",
        ));
    }
    std::fs::create_dir_all(&path)?;
    let path = std::fs::canonicalize(path)?;
    let current = DATA_DIR.get_or_init(|| path.clone());
    if current != &path {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "The Android data directory is already set.",
        ));
    }
    Ok(())
}

/// Returns the private directory set by the embedding app.
pub fn data_dir() -> PathBuf {
    DATA_DIR
        .get()
        .expect("set_data_dir must run before bir-core starts")
        .clone()
}

/// Returns a temporary directory inside the app's private storage.
pub fn temp_dir() -> PathBuf {
    data_dir().join("tmp")
}

/// Android does not support user shell commands.
pub async fn run_shell_command(_cmd: &str) -> Result<std::process::Output, std::io::Error> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Shell commands are not available on Android.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_data_directory_is_absolute_and_cannot_change() {
        assert_eq!(
            set_data_dir(PathBuf::new()).unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
        assert_eq!(
            set_data_dir(PathBuf::from("relative")).unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
        let root = tempfile::tempdir().unwrap().keep();
        let path = root.join("data");
        set_data_dir(path.clone()).unwrap();
        set_data_dir(path.clone()).unwrap();
        assert_eq!(data_dir(), path.canonicalize().unwrap());
        assert_eq!(
            set_data_dir(root.join("other")).unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
    }

    #[tokio::test]
    async fn shell_commands_are_unsupported() {
        assert_eq!(
            run_shell_command("echo hello").await.unwrap_err().kind(),
            std::io::ErrorKind::Unsupported
        );
    }
}
