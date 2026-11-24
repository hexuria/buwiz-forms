//! Where the device key and the device id live: the OS credential
//! store (Keychain, Credential Manager, Secret Service) through `keyring`,
//! or, where there is none (Android, a headless Linux box), a file in the
//! app's private data dir, as bir-core does for its database key.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Service name of the credential-store entries. Their names there are the
/// store's names with `-` for `_`: `device-key`, `device-id`.
pub const SERVICE: &str = "com.buwiz.bir";

#[derive(Debug, thiserror::Error)]
#[error("secret store: {0}")]
pub struct StoreError(pub String);

pub trait SecretStore: Send + Sync {
    fn get(&self, name: &str) -> Result<Option<String>, StoreError>;
    fn set(&self, name: &str, value: &str) -> Result<(), StoreError>;
    fn delete(&self, name: &str) -> Result<(), StoreError>;
}

/// The store for this platform: with `use_keychain`, the OS credential
/// store, falling back to files in `data_dir` when the OS store cannot be
/// used; without it (the host's `BUWIZ_NO_KEYCHAIN=1`, Android, tests),
/// files in `data_dir` only.
pub fn platform_store(data_dir: PathBuf, use_keychain: bool) -> Box<dyn SecretStore> {
    if !use_keychain {
        return Box::new(FileStore::new(data_dir));
    }
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux"
    ))]
    {
        Box::new(KeyringStore {
            keychain: Box::new(OsKeychain),
            fallback: FileStore::new(data_dir),
        })
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux"
    )))]
    {
        Box::new(FileStore::new(data_dir))
    }
}

/// The OS credential store's three calls, so a test can stand in for it.
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "windows",
    target_os = "linux"
))]
trait Keychain: Send + Sync {
    fn get(&self, name: &str) -> Result<String, keyring::Error>;
    fn set(&self, name: &str, value: &str) -> Result<(), keyring::Error>;
    fn delete(&self, name: &str) -> Result<(), keyring::Error>;
}

/// The real one: entries of [`SERVICE`] named `device-key`, `device-id`.
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "windows",
    target_os = "linux"
))]
struct OsKeychain;

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "windows",
    target_os = "linux"
))]
impl OsKeychain {
    fn entry(name: &str) -> Result<keyring::Entry, keyring::Error> {
        keyring::Entry::new(SERVICE, &name.replace('_', "-"))
    }
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "windows",
    target_os = "linux"
))]
impl Keychain for OsKeychain {
    fn get(&self, name: &str) -> Result<String, keyring::Error> {
        Self::entry(name).and_then(|entry| entry.get_password())
    }

    fn set(&self, name: &str, value: &str) -> Result<(), keyring::Error> {
        Self::entry(name).and_then(|entry| entry.set_password(value))
    }

    fn delete(&self, name: &str) -> Result<(), keyring::Error> {
        Self::entry(name).and_then(|entry| entry.delete_credential())
    }
}

/// The OS credential store. When it is unavailable (no Secret Service on
/// D-Bus, for one), secrets go to the fallback file store, and a secret
/// already in that file wins, as bir-core does with `bir_key.txt`.
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "windows",
    target_os = "linux"
))]
pub struct KeyringStore {
    keychain: Box<dyn Keychain>,
    fallback: FileStore,
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "windows",
    target_os = "linux"
))]
impl KeyringStore {
    fn unavailable(error: &keyring::Error) -> bool {
        matches!(
            error,
            keyring::Error::PlatformFailure(_) | keyring::Error::NoStorageAccess(_)
        )
    }
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "windows",
    target_os = "linux"
))]
impl SecretStore for KeyringStore {
    fn get(&self, name: &str) -> Result<Option<String>, StoreError> {
        if let Some(value) = self.fallback.get(name)? {
            return Ok(Some(value));
        }
        match self.keychain.get(name) {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) if Self::unavailable(&error) => Ok(None),
            Err(error) => Err(StoreError(error.to_string())),
        }
    }

    fn set(&self, name: &str, value: &str) -> Result<(), StoreError> {
        match self.keychain.set(name, value) {
            // A keyring built without a backend reports success and
            // forgets: only a value read back is kept there.
            Ok(()) if self.keychain.get(name).ok().as_deref() == Some(value) => {
                self.fallback.delete(name)
            }
            Ok(()) => {
                tracing::warn!(
                    "credential store did not keep {name}; keeping it in a private file"
                );
                self.fallback.set(name, value)
            }
            Err(error) if Self::unavailable(&error) => {
                tracing::warn!(
                    "credential store unavailable ({error}); keeping {name} in a private file"
                );
                self.fallback.set(name, value)
            }
            Err(error) => Err(StoreError(error.to_string())),
        }
    }

    fn delete(&self, name: &str) -> Result<(), StoreError> {
        self.fallback.delete(name)?;
        match self.keychain.delete(name) {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) if Self::unavailable(&error) => Ok(()),
            Err(error) => Err(StoreError(error.to_string())),
        }
    }
}

/// One file per secret, readable by this user only.
pub struct FileStore {
    dir: PathBuf,
}

impl FileStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("buwiz_submit_{name}.txt"))
    }
}

impl SecretStore for FileStore {
    fn get(&self, name: &str) -> Result<Option<String>, StoreError> {
        match std::fs::read_to_string(self.path(name)) {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(StoreError(error.to_string())),
        }
    }

    fn set(&self, name: &str, value: &str) -> Result<(), StoreError> {
        std::fs::create_dir_all(&self.dir).map_err(|error| StoreError(error.to_string()))?;
        let path = self.path(name);
        let temporary = path.with_extension("tmp");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = options
            .open(&temporary)
            .map_err(|error| StoreError(error.to_string()))?;
        std::io::Write::write_all(&mut file, value.as_bytes())
            .map_err(|error| StoreError(error.to_string()))?;
        drop(file);
        std::fs::rename(&temporary, &path).map_err(|error| StoreError(error.to_string()))
    }

    fn delete(&self, name: &str) -> Result<(), StoreError> {
        match std::fs::remove_file(self.path(name)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(StoreError(error.to_string())),
        }
    }
}

/// In memory, for tests and tools.
#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, name: &str) -> Result<Option<String>, StoreError> {
        Ok(self.0.lock().expect("memory store").get(name).cloned())
    }

    fn set(&self, name: &str, value: &str) -> Result<(), StoreError> {
        self.0
            .lock()
            .expect("memory store")
            .insert(name.to_owned(), value.to_owned());
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<(), StoreError> {
        self.0.lock().expect("memory store").remove(name);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_store_round_trips_privately() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileStore::new(dir.path().to_owned());
        assert_eq!(store.get("session").unwrap(), None);
        store.set("session", "{\"a\":1}").unwrap();
        assert_eq!(store.get("session").unwrap().as_deref(), Some("{\"a\":1}"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(store.path("session"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0, "only the owner may read it");
        }
        store.delete("session").unwrap();
        store.delete("session").unwrap();
        assert_eq!(store.get("session").unwrap(), None);
    }

    /// Without the keychain, the platform store is the file store in the
    /// data dir alone.
    #[test]
    fn without_the_keychain_secrets_are_files_in_the_data_dir() {
        let dir = tempfile::tempdir().unwrap();
        let store = platform_store(dir.path().to_owned(), false);
        store.set("device_key", "seed").unwrap();
        assert_eq!(store.get("device_key").unwrap().as_deref(), Some("seed"));
        let file = dir.path().join("buwiz_submit_device_key.txt");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "seed");
        store.delete("device_key").unwrap();
        assert!(!file.exists());
        assert_eq!(store.get("device_key").unwrap(), None);
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux"
    ))]
    mod keychain {
        use super::super::*;

        /// A keychain that keeps what it is told, forgets it (a build with
        /// no backend), or is not there at all.
        #[derive(Clone, Copy)]
        enum Kind {
            Keeps,
            Forgets,
            Unavailable,
        }

        struct Fake {
            kind: Kind,
            entries: std::sync::Arc<Mutex<HashMap<String, String>>>,
        }

        impl Keychain for Fake {
            fn get(&self, name: &str) -> Result<String, keyring::Error> {
                match self.kind {
                    Kind::Unavailable => Err(keyring::Error::PlatformFailure("no keychain".into())),
                    _ => self
                        .entries
                        .lock()
                        .unwrap()
                        .get(name)
                        .cloned()
                        .ok_or(keyring::Error::NoEntry),
                }
            }

            fn set(&self, name: &str, value: &str) -> Result<(), keyring::Error> {
                match self.kind {
                    Kind::Keeps => {
                        self.entries
                            .lock()
                            .unwrap()
                            .insert(name.to_owned(), value.to_owned());
                        Ok(())
                    }
                    Kind::Forgets => Ok(()),
                    Kind::Unavailable => Err(keyring::Error::PlatformFailure("no keychain".into())),
                }
            }

            fn delete(&self, name: &str) -> Result<(), keyring::Error> {
                match self.kind {
                    Kind::Unavailable => Err(keyring::Error::PlatformFailure("no keychain".into())),
                    _ => self
                        .entries
                        .lock()
                        .unwrap()
                        .remove(name)
                        .map(drop)
                        .ok_or(keyring::Error::NoEntry),
                }
            }
        }

        fn store(
            kind: Kind,
            dir: &std::path::Path,
        ) -> (KeyringStore, std::sync::Arc<Mutex<HashMap<String, String>>>) {
            let entries = std::sync::Arc::default();
            let store = KeyringStore {
                keychain: Box::new(Fake {
                    kind,
                    entries: std::sync::Arc::clone(&entries),
                }),
                fallback: FileStore::new(dir.to_owned()),
            };
            (store, entries)
        }

        #[test]
        fn a_keychain_that_keeps_the_value_holds_it_and_no_file_does() {
            let dir = tempfile::tempdir().unwrap();
            let (store, entries) = store(Kind::Keeps, dir.path());
            store.set("device_id", "d1").unwrap();
            assert_eq!(
                entries.lock().unwrap().get("device_id").map(String::as_str),
                Some("d1")
            );
            assert!(!dir.path().join("buwiz_submit_device_id.txt").exists());
            assert_eq!(store.get("device_id").unwrap().as_deref(), Some("d1"));
            store.delete("device_id").unwrap();
            assert_eq!(store.get("device_id").unwrap(), None);
        }

        /// A keychain that reports success and forgets: the value read back
        /// is not there, so it goes to the file store.
        #[test]
        fn a_keychain_that_forgets_falls_back_to_the_file() {
            let dir = tempfile::tempdir().unwrap();
            let (store, _) = store(Kind::Forgets, dir.path());
            store.set("device_key", "seed").unwrap();
            assert_eq!(
                std::fs::read_to_string(dir.path().join("buwiz_submit_device_key.txt")).unwrap(),
                "seed"
            );
            assert_eq!(store.get("device_key").unwrap().as_deref(), Some("seed"));
            store.delete("device_key").unwrap();
            assert_eq!(store.get("device_key").unwrap(), None);
        }

        #[test]
        fn an_unavailable_keychain_falls_back_to_the_file() {
            let dir = tempfile::tempdir().unwrap();
            let (store, _) = store(Kind::Unavailable, dir.path());
            store.set("device_id", "d1").unwrap();
            assert_eq!(store.get("device_id").unwrap().as_deref(), Some("d1"));
            assert!(dir.path().join("buwiz_submit_device_id.txt").exists());
            store.delete("device_id").unwrap();
            assert_eq!(store.get("device_id").unwrap(), None);
        }
    }
}
