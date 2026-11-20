//! Private staging and display paths for Android profile downloads.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) struct Archive {
    directory: PathBuf,
}

impl Archive {
    pub(crate) fn write(
        root: &Path,
        write: &mut bir_app_api::hooks::ArchiveWriter<'_>,
    ) -> Result<Self, String> {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let directory = root.join(format!(
            "profile-export-{}-{nanos}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(&directory)
            .map_err(|error| error.to_string())?;
        let archive = Self { directory };
        write(&archive.path())?;
        Ok(archive)
    }

    pub(crate) fn path(&self) -> PathBuf {
        self.directory.join("profile.zip")
    }
}

impl Drop for Archive {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.directory) {
            tracing::warn!(%error, "Could not remove private profile export staging directory");
        }
    }
}

#[cfg(any(test, target_os = "android"))]
pub(crate) fn download_path(name: &str) -> Result<String, String> {
    if name.is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." {
        return Err("Android Downloads returned an invalid archive name.".into());
    }
    Ok(format!("Downloads/eBIRForms/{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staging_is_private_unique_and_removed_on_drop() {
        let root = tempfile::tempdir().unwrap();
        let mut write = |path: &Path| std::fs::write(path, b"archive").map_err(|e| e.to_string());
        let first = Archive::write(root.path(), &mut write).unwrap();
        let second = Archive::write(root.path(), &mut write).unwrap();
        assert_ne!(first.path(), second.path());
        assert!(first.path().starts_with(root.path()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&first.directory)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        let directory = first.directory.clone();
        drop(first);
        assert!(!directory.exists());
        drop(second);
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn failed_writer_removes_even_a_partial_archive() {
        let root = tempfile::tempdir().unwrap();
        let result = Archive::write(root.path(), &mut |path| {
            std::fs::write(path, b"partial").unwrap();
            Err("ZIP write failed".into())
        });
        assert!(matches!(result, Err(error) if error == "ZIP write failed"));
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn download_path_uses_the_provider_name_including_collision_suffixes() {
        assert_eq!(
            download_path("BIR_Profile (1).zip").unwrap(),
            "Downloads/eBIRForms/BIR_Profile (1).zip"
        );
        for invalid in [
            "",
            ".",
            "..",
            "../profile.zip",
            "other/profile.zip",
            "other\\profile.zip",
        ] {
            assert!(download_path(invalid).is_err());
        }
    }
}
