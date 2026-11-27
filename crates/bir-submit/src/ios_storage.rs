//! What the app keeps where on iPhone and iPad.
//!
//! - The saved sign-in, the device key and the server's device id are files
//!   in the app's own data folder (the [`crate::secrets::FileStore`] and
//!   bir-core's `buwiz-cloud-account.json`), never Keychain items: deleting
//!   the app deletes them, and no sign-in or send depends on a Keychain call.
//!   Each is written with the protection "complete until first user
//!   authentication" ([`protect_file`]), so a send can run while the phone is
//!   locked once it has been unlocked after a restart. The data folder is
//!   excluded from device backups (`exclude_from_backup`).
//! - The Keychain holds one item: the install id ([`install_id`]), an
//!   identifier (not a secret) whose only job there is to survive deleting
//!   and reinstalling the app. "After first unlock, this device only", never
//!   synchronized. Any Keychain failure falls back to an id kept in a file.
//! - One build stored the sign-in and the device key in the Keychain; the
//!   launch step deletes those three items once ([`remove_old_items`]).
//!
//! The rules are written against [`Backend`] and tested with a fake on every
//! system; the real Keychain (`OsKeychain`) and the file attributes exist on
//! iOS only. bir-core calls this module too: it lives in this leaf crate so
//! both crates reach it without a dependency cycle.

use std::path::Path;

/// One generic-password item: its service and account, and the name the
/// log calls it by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Item {
    pub name: &'static str,
    pub service: &'static str,
    pub account: &'static str,
}

/// The install id: the only item the app stores in the Keychain.
pub const INSTALL_ID: Item = Item {
    name: "install-id",
    service: crate::secrets::SERVICE,
    account: "install-id",
};

/// The items one earlier build stored in the Keychain, under these names.
pub const OLD_ITEMS: [Item; 3] = [
    Item {
        name: "device-key",
        service: crate::secrets::SERVICE,
        account: "device-key",
    },
    Item {
        name: "device-id",
        service: crate::secrets::SERVICE,
        account: "device-id",
    },
    Item {
        name: "sign-in",
        service: crate::secrets::SERVICE,
        account: "desktop-refresh",
    },
];

/// The symbolic name of a Security framework status the app meets.
pub fn status_name(status: i32) -> Option<&'static str> {
    Some(match status {
        -34018 => "errSecMissingEntitlement",
        -25308 => "errSecInteractionNotAllowed",
        -25300 => "errSecItemNotFound",
        -25299 => "errSecDuplicateItem",
        -25291 => "errSecNotAvailable",
        -25292 => "errSecReadOnly",
        -50 => "errSecParam",
        _ => return None,
    })
}

/// `OSStatus -34018 (errSecMissingEntitlement)`.
fn status_text(status: i32) -> String {
    match status_name(status) {
        Some(name) => format!("OSStatus {status} ({name})"),
        None => format!("OSStatus {status}"),
    }
}

/// A Keychain call that did not succeed, with its status.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeychainError {
    #[error("not found, {}", status_text(-25300))]
    NotFound,
    #[error("already there, {}", status_text(-25299))]
    Duplicate,
    #[error("{}", status_text(*.0))]
    Failed(i32),
}

impl KeychainError {
    /// The error of a Security framework status that is not `errSecSuccess`.
    pub fn from_status(status: i32) -> Self {
        match status {
            -25300 => Self::NotFound,
            -25299 => Self::Duplicate,
            other => Self::Failed(other),
        }
    }

    pub fn status(&self) -> i32 {
        match self {
            Self::NotFound => -25300,
            Self::Duplicate => -25299,
            Self::Failed(status) => *status,
        }
    }
}

/// The three Keychain calls the rules below are made of.
pub trait Backend {
    /// The item's value, or [`KeychainError::NotFound`].
    fn read(&self, item: &Item) -> Result<Vec<u8>, KeychainError>;
    /// Add the item, "after first unlock, this device only" and not
    /// synchronized; [`KeychainError::Duplicate`] when it exists, which this
    /// call never replaces.
    fn add(&self, item: &Item, value: &[u8]) -> Result<(), KeychainError>;
    fn delete(&self, item: &Item) -> Result<(), KeychainError>;
}

// ------------------------------------------------------------- install id

/// The file beside the database that keeps the install id when the
/// Keychain cannot (and mirrors the Keychain's, so a later failure answers
/// the same id).
pub const INSTALL_ID_FILE: &str = "buwiz_install_id.txt";

/// Where the install id came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallIdSource {
    /// It was in the Keychain (it survived a reinstall, or was stored earlier).
    Keychain,
    /// The Keychain had none; this one was just stored there.
    Stored,
    /// The file: the Keychain is off, or it could not be read or written.
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallId {
    pub value: String,
    pub source: InstallIdSource,
}

/// An install id the server takes: 16 to 64 of `[A-Za-z0-9_-]`.
pub fn is_valid_install_id(value: &str) -> bool {
    (16..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// A new install id: 32 random bytes, base64url without padding (43 characters).
pub fn new_install_id() -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>())
}

/// The install id of this installation, from the folder `dir` (beside the
/// database) and, with `use_keychain` on iOS, the Keychain. `offered` is
/// the id to store when there is none yet (a new one is made when it is
/// `None` or not a valid id). Never fails and never waits on a person:
/// any Keychain failure is one logged line and the file's id. Off iOS, and
/// with `use_keychain` false (`BUWIZ_NO_KEYCHAIN=1`), the file only.
pub fn install_id(dir: &Path, use_keychain: bool, offered: Option<&str>) -> InstallId {
    #[cfg(target_os = "ios")]
    let keychain: Option<&dyn Backend> = use_keychain.then_some(&OsKeychain as &dyn Backend);
    #[cfg(not(target_os = "ios"))]
    let keychain: Option<&dyn Backend> = {
        let _ = use_keychain;
        None
    };
    resolve_install_id(keychain, &dir.join(INSTALL_ID_FILE), offered)
}

/// [`install_id`] with the Keychain given (`None`: no call is made) and the
/// file named.
///
/// - The Keychain has a valid id: that one (and the file is made to match).
/// - It has none: the file's id, else `offered`, else a new one, is added;
///   when the add fails the same id is kept in the file instead.
/// - The read fails for any reason, or the stored value is not a valid id:
///   the file's id (made if missing).
pub fn resolve_install_id(
    keychain: Option<&dyn Backend>,
    file: &Path,
    offered: Option<&str>,
) -> InstallId {
    let in_file = read_id_file(file);
    let candidate = || {
        in_file
            .clone()
            .or_else(|| {
                offered
                    .filter(|id| is_valid_install_id(id))
                    .map(str::to_owned)
            })
            .unwrap_or_else(new_install_id)
    };
    let Some(keychain) = keychain else {
        return from_file(file, in_file.clone(), candidate);
    };
    let name = INSTALL_ID.name;
    match keychain.read(&INSTALL_ID) {
        Ok(bytes) => match String::from_utf8(bytes)
            .ok()
            .filter(|id| is_valid_install_id(id))
        {
            Some(value) => {
                tracing::info!("Keychain {name}: read, stored");
                mirror(file, in_file.as_deref(), &value);
                InstallId {
                    value,
                    source: InstallIdSource::Keychain,
                }
            }
            None => {
                tracing::warn!(
                    "Keychain {name}: read, not a valid install id; using the install id in its file"
                );
                from_file(file, in_file.clone(), candidate)
            }
        },
        Err(KeychainError::NotFound) => {
            tracing::info!("Keychain {name}: read, absent");
            let value = candidate();
            match keychain.add(&INSTALL_ID, value.as_bytes()) {
                Ok(()) => {
                    tracing::info!("Keychain {name}: saved");
                    mirror(file, in_file.as_deref(), &value);
                    InstallId {
                        value,
                        source: InstallIdSource::Stored,
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        "Keychain {name}: save failed, {error}; using the install id in its file"
                    );
                    from_file(file, in_file.clone(), || value)
                }
            }
        }
        Err(error) => {
            tracing::warn!(
                "Keychain {name}: read failed, {error}; using the install id in its file"
            );
            from_file(file, in_file.clone(), candidate)
        }
    }
}

fn read_id_file(file: &Path) -> Option<String> {
    std::fs::read_to_string(file)
        .ok()
        .map(|text| text.trim().to_owned())
        .filter(|id| is_valid_install_id(id))
}

/// The file's id, or `candidate()` written there.
fn from_file(
    file: &Path,
    in_file: Option<String>,
    candidate: impl FnOnce() -> String,
) -> InstallId {
    let value = in_file.unwrap_or_else(|| {
        let value = candidate();
        write_id_file(file, &value);
        value
    });
    InstallId {
        value,
        source: InstallIdSource::File,
    }
}

fn mirror(file: &Path, in_file: Option<&str>, value: &str) {
    if in_file != Some(value) {
        write_id_file(file, value);
    }
}

fn write_id_file(file: &Path, value: &str) {
    let written = file
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(file, value));
    match written {
        Ok(()) => protect_file(file),
        Err(error) => {
            tracing::warn!("install id file not written ({error}); kept for this run only")
        }
    }
}

// ------------------------------------------------------ the old items, once

/// The marker file, in the app's data folder, that says the old items are gone.
pub const OLD_ITEMS_MARKER: &str = "keychain-old-items-removed";

/// What [`remove_old_items`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OldItems {
    /// The Keychain is off: no call, no marker.
    KeychainOff,
    /// An earlier launch did it.
    AlreadyDone,
    /// Each old item's delete, as said in the log (`device-key deleted`,
    /// `device-id absent`, `sign-in not deleted, OSStatus …`).
    Removed(Vec<String>),
}

impl std::fmt::Display for OldItems {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeychainOff => write!(f, "skipped, the Keychain is off"),
            Self::AlreadyDone => write!(f, "removed at an earlier launch"),
            Self::Removed(outcomes) => write!(f, "{}", outcomes.join(", ")),
        }
    }
}

/// Delete [`OLD_ITEMS`] once, best effort: every failure is noted and
/// ignored, the marker is written whatever happened, and the install id is
/// never touched. Whether the person is signed in is decided by the files,
/// not by this. With the Keychain off (`None`) nothing is called and no
/// marker is written.
pub fn remove_old_items(keychain: Option<&dyn Backend>, marker: &Path) -> OldItems {
    let Some(keychain) = keychain else {
        return OldItems::KeychainOff;
    };
    if marker.exists() {
        return OldItems::AlreadyDone;
    }
    let mut outcomes: Vec<String> = OLD_ITEMS
        .iter()
        .map(|item| match keychain.delete(item) {
            Ok(()) => format!("{} deleted", item.name),
            Err(KeychainError::NotFound) => format!("{} absent", item.name),
            Err(error) => format!("{} not deleted, {error}", item.name),
        })
        .collect();
    let written = marker
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(marker, b"1\n"));
    if let Err(error) = written {
        outcomes.push(format!("marker not written ({error})"));
    }
    OldItems::Removed(outcomes)
}

// ------------------------------------------------------------ file attributes

/// Give `file` the protection "complete until first user authentication"
/// (iOS): readable and writable while the phone is locked, once it has been
/// unlocked after a restart. A failure is a logged warning, never a failure
/// of the write. Nothing on other systems.
pub fn protect_file(file: &Path) {
    #[cfg(target_os = "ios")]
    if let Err(error) = set_file_protection(file, false) {
        let name = file.file_name().unwrap_or_default().to_string_lossy();
        tracing::warn!("file protection not set on {name}: {error}");
    }
    #[cfg(not(target_os = "ios"))]
    let _ = file;
}

#[cfg(target_os = "ios")]
pub use os::{OsKeychain, exclude_from_backup, set_file_protection};

/// The real Keychain (`SecItem*`) and the file attributes.
#[cfg(target_os = "ios")]
mod os {
    use super::{Backend, Item, KeychainError};
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::data::CFData;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::error::CFError;
    use core_foundation::string::{CFString, CFStringRef};
    use core_foundation::url::CFURL;
    use core_foundation_sys::base::CFTypeRef;
    use security_framework_sys::access_control::kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly;
    use security_framework_sys::item::{
        kSecAttrAccount, kSecAttrService, kSecAttrSynchronizable, kSecClass,
        kSecClassGenericPassword, kSecReturnData, kSecValueData,
    };
    use security_framework_sys::keychain_item::{SecItemAdd, SecItemCopyMatching, SecItemDelete};

    // Not declared by security-framework-sys.
    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        static kSecAttrAccessible: CFStringRef;
    }

    // Declared by core-foundation-sys only behind a macOS feature, or not at all.
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFURLIsExcludedFromBackupKey: CFStringRef;
        static kCFURLFileProtectionKey: CFStringRef;
        static kCFURLFileProtectionCompleteUntilFirstUserAuthentication: CFStringRef;
    }

    fn constant(name: CFStringRef) -> CFString {
        // SAFETY: a framework constant, valid for the process.
        unsafe { CFString::wrap_under_get_rule(name) }
    }

    /// Class, service, account: what names one item. No synchronizable
    /// attribute, so only the item that stays on this device matches.
    fn identity(item: &Item) -> Vec<(CFString, CFType)> {
        vec![
            (
                constant(unsafe { kSecClass }),
                constant(unsafe { kSecClassGenericPassword }).into_CFType(),
            ),
            (
                constant(unsafe { kSecAttrService }),
                CFString::new(item.service).into_CFType(),
            ),
            (
                constant(unsafe { kSecAttrAccount }),
                CFString::new(item.account).into_CFType(),
            ),
        ]
    }

    fn check(status: i32) -> Result<(), KeychainError> {
        if status == 0 {
            Ok(())
        } else {
            Err(KeychainError::from_status(status))
        }
    }

    /// The app's Keychain through `SecItemCopyMatching`, `SecItemAdd` and
    /// `SecItemDelete`.
    pub struct OsKeychain;

    impl Backend for OsKeychain {
        fn read(&self, item: &Item) -> Result<Vec<u8>, KeychainError> {
            let mut query = identity(item);
            query.push((
                constant(unsafe { kSecReturnData }),
                CFBoolean::true_value().into_CFType(),
            ));
            let query = CFDictionary::from_CFType_pairs(&query);
            let mut result: CFTypeRef = std::ptr::null();
            check(unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), &mut result) })?;
            if result.is_null() {
                return Err(KeychainError::Failed(-50));
            }
            // SAFETY: a Copy call's result, owned by us.
            unsafe { CFType::wrap_under_create_rule(result) }
                .downcast_into::<CFData>()
                .map(|data| data.bytes().to_vec())
                .ok_or(KeychainError::Failed(-50))
        }

        fn add(&self, item: &Item, value: &[u8]) -> Result<(), KeychainError> {
            let mut attributes = identity(item);
            attributes.push((
                constant(unsafe { kSecAttrAccessible }),
                constant(unsafe { kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly }).into_CFType(),
            ));
            attributes.push((
                constant(unsafe { kSecAttrSynchronizable }),
                CFBoolean::false_value().into_CFType(),
            ));
            attributes.push((
                constant(unsafe { kSecValueData }),
                CFData::from_buffer(value).into_CFType(),
            ));
            let attributes = CFDictionary::from_CFType_pairs(&attributes);
            check(unsafe { SecItemAdd(attributes.as_concrete_TypeRef(), std::ptr::null_mut()) })
        }

        fn delete(&self, item: &Item) -> Result<(), KeychainError> {
            let query = CFDictionary::from_CFType_pairs(&identity(item));
            check(unsafe { SecItemDelete(query.as_concrete_TypeRef()) })
        }
    }

    /// `CFURLSetResourcePropertyForKey(path, key, value)`.
    fn set_property(
        path: &std::path::Path,
        directory: bool,
        key: CFStringRef,
        value: CFTypeRef,
    ) -> Result<(), String> {
        let url = CFURL::from_path(path, directory)
            .ok_or_else(|| format!("{} is not a file URL", path.display()))?;
        let mut error = std::ptr::null_mut();
        let done = unsafe {
            core_foundation_sys::url::CFURLSetResourcePropertyForKey(
                url.as_concrete_TypeRef(),
                key,
                value,
                &mut error,
            )
        };
        if done != 0 {
            return Ok(());
        }
        if error.is_null() {
            return Err("the attribute was not set".into());
        }
        // SAFETY: an error the call created and handed to us.
        Err(unsafe { CFError::wrap_under_create_rule(error) }
            .description()
            .to_string())
    }

    /// Mark a directory "excluded from backup" (`NSURLIsExcludedFromBackupKey`):
    /// neither iCloud nor a computer backup copies it or what it holds.
    pub fn exclude_from_backup(directory: &std::path::Path) -> Result<(), String> {
        set_property(
            directory,
            true,
            unsafe { kCFURLIsExcludedFromBackupKey },
            CFBoolean::true_value().as_CFTypeRef(),
        )
    }

    /// Set `NSURLFileProtectionKey` to
    /// `NSURLFileProtectionCompleteUntilFirstUserAuthentication` on a file or
    /// a folder (files made in a folder later take the folder's).
    pub fn set_file_protection(path: &std::path::Path, directory: bool) -> Result<(), String> {
        let class = constant(unsafe { kCFURLFileProtectionCompleteUntilFirstUserAuthentication });
        set_property(
            path,
            directory,
            unsafe { kCFURLFileProtectionKey },
            class.as_CFTypeRef(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// A Keychain in memory. Records every call; a call named in `fail`
    /// fails with its error.
    #[derive(Default)]
    struct Fake {
        items: RefCell<HashMap<&'static str, Vec<u8>>>,
        calls: RefCell<Vec<String>>,
        fail: HashMap<&'static str, KeychainError>,
    }

    impl Fake {
        fn holding(entries: &[(Item, &str)]) -> Self {
            let fake = Self::default();
            for (item, value) in entries {
                fake.items
                    .borrow_mut()
                    .insert(item.account, value.as_bytes().to_vec());
            }
            fake
        }
        fn failing(mut self, call: &'static str, error: KeychainError) -> Self {
            self.fail.insert(call, error);
            self
        }
        fn record(&self, call: &'static str, item: &Item) -> Result<(), KeychainError> {
            self.calls
                .borrow_mut()
                .push(format!("{call} {}", item.account));
            self.fail.get(call).cloned().map_or(Ok(()), Err)
        }
        fn value(&self, item: &Item) -> Option<String> {
            self.items
                .borrow()
                .get(item.account)
                .map(|value| String::from_utf8(value.clone()).unwrap())
        }
        fn calls(&self) -> Vec<String> {
            self.calls.borrow().clone()
        }
    }

    impl Backend for Fake {
        fn read(&self, item: &Item) -> Result<Vec<u8>, KeychainError> {
            self.record("read", item)?;
            self.items
                .borrow()
                .get(item.account)
                .cloned()
                .ok_or(KeychainError::NotFound)
        }
        fn add(&self, item: &Item, value: &[u8]) -> Result<(), KeychainError> {
            self.record("add", item)?;
            let mut items = self.items.borrow_mut();
            if items.contains_key(item.account) {
                return Err(KeychainError::Duplicate);
            }
            items.insert(item.account, value.to_vec());
            Ok(())
        }
        fn delete(&self, item: &Item) -> Result<(), KeychainError> {
            self.record("delete", item)?;
            self.items
                .borrow_mut()
                .remove(item.account)
                .map(drop)
                .ok_or(KeychainError::NotFound)
        }
    }

    const KEPT: &str = "kept-install-id-0123456789";
    const OFFERED: &str = "offered-install-id-abcdef";

    /// Every status a Keychain call can fail with in the field.
    fn failures() -> Vec<KeychainError> {
        [-34018, -25308, -25291, -25292, -1]
            .into_iter()
            .map(KeychainError::from_status)
            .chain([KeychainError::Duplicate])
            .collect()
    }

    /// Log lines of `body`, as the app's subscriber writes them.
    fn logged(body: impl FnOnce()) -> String {
        crate::test_logs::capture_logs(tracing::Level::INFO, body)
    }

    #[test]
    fn the_install_id_has_the_servers_shape() {
        let id = new_install_id();
        assert_eq!(id.len(), 43);
        assert!(is_valid_install_id(&id), "{id}");
        assert_ne!(id, new_install_id());
        assert!(!is_valid_install_id("short"));
        assert!(!is_valid_install_id(&"a".repeat(65)));
        assert!(!is_valid_install_id("has spaces in it here"));
        assert!(!is_valid_install_id("padded-base64-value=="));
        assert!(is_valid_install_id(&"a".repeat(16)));
        assert!(is_valid_install_id(&"Z_-9".repeat(16)));
        assert_eq!(INSTALL_ID.service, "com.buwiz.bir");
        assert_eq!(INSTALL_ID.account, "install-id");
    }

    #[test]
    fn a_keychain_hit_is_the_install_id_and_the_file_follows_it() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(INSTALL_ID_FILE);
        let fake = Fake::holding(&[(INSTALL_ID, KEPT)]);
        let text = logged(|| {
            let id = resolve_install_id(Some(&fake), &file, Some(OFFERED));
            assert_eq!(id.value, KEPT);
            assert_eq!(id.source, InstallIdSource::Keychain);
        });
        assert_eq!(fake.calls(), ["read install-id"]);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), KEPT);
        assert!(
            text.contains("INFO bir_submit::ios_storage: Keychain install-id: read, stored"),
            "{text}"
        );
        assert!(!text.contains(KEPT), "never the value: {text}");
    }

    #[test]
    fn a_miss_stores_the_offered_id_or_a_new_one() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(INSTALL_ID_FILE);
        let fake = Fake::default();
        let text = logged(|| {
            let id = resolve_install_id(Some(&fake), &file, Some(OFFERED));
            assert_eq!(id.value, OFFERED);
            assert_eq!(id.source, InstallIdSource::Stored);
        });
        assert_eq!(fake.value(&INSTALL_ID).as_deref(), Some(OFFERED));
        assert_eq!(fake.calls(), ["read install-id", "add install-id"]);
        assert!(
            text.contains("INFO bir_submit::ios_storage: Keychain install-id: read, absent"),
            "{text}"
        );
        assert!(
            text.contains("INFO bir_submit::ios_storage: Keychain install-id: saved"),
            "{text}"
        );
        assert!(!text.contains(OFFERED), "never the value: {text}");
        // The next launch finds it.
        let again = resolve_install_id(Some(&fake), &file, None);
        assert_eq!(
            (again.value.as_str(), again.source),
            (OFFERED, InstallIdSource::Keychain)
        );

        // Nothing offered (or nothing valid): a new id.
        for offered in [None, Some("not valid")] {
            let dir = tempfile::tempdir().unwrap();
            let fake = Fake::default();
            let id = resolve_install_id(Some(&fake), &dir.path().join(INSTALL_ID_FILE), offered);
            assert!(is_valid_install_id(&id.value));
            assert_eq!(fake.value(&INSTALL_ID), Some(id.value));
        }
    }

    /// A file id from a launch whose Keychain failed is the one stored once
    /// the Keychain works, so the install id does not change.
    #[test]
    fn a_miss_stores_the_id_the_file_already_has() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(INSTALL_ID_FILE);
        std::fs::write(&file, KEPT).unwrap();
        let fake = Fake::default();
        let id = resolve_install_id(Some(&fake), &file, Some(OFFERED));
        assert_eq!(id.value, KEPT);
        assert_eq!(fake.value(&INSTALL_ID).as_deref(), Some(KEPT));
    }

    /// Any failure of the read or the write: the file's id (made if
    /// missing), one warning with the name and the status, never an error.
    #[test]
    fn every_keychain_failure_falls_back_to_the_file_with_one_line() {
        for error in failures() {
            for call in ["read", "add"] {
                let dir = tempfile::tempdir().unwrap();
                let file = dir.path().join(INSTALL_ID_FILE);
                let fake = Fake::default().failing(call, error.clone());
                let mut first = None;
                let text = logged(|| {
                    let id = resolve_install_id(Some(&fake), &file, Some(OFFERED));
                    assert_eq!(id.source, InstallIdSource::File, "{call} {error:?}");
                    assert_eq!(id.value, OFFERED);
                    first = Some(id.value);
                });
                assert_eq!(read_id_file(&file), first, "kept in the file");
                let warnings: Vec<&str> =
                    text.lines().filter(|line| line.contains("WARN")).collect();
                assert_eq!(warnings.len(), 1, "{call} {error:?}: {text}");
                let action = if call == "read" {
                    "read failed"
                } else {
                    "save failed"
                };
                assert!(
                    warnings[0].contains(&format!(
                        "Keychain install-id: {action}, {error}; using the install id in its file"
                    )),
                    "{text}"
                );
                assert!(warnings[0].contains(&format!("OSStatus {}", error.status())));
                assert!(!text.contains(OFFERED), "never the value: {text}");
                // The next launch, failing the same way, answers the same id.
                let again = resolve_install_id(Some(&fake), &file, None);
                assert_eq!(Some(again.value), first);
            }
        }
        let text = logged(|| {
            let dir = tempfile::tempdir().unwrap();
            let fake = Fake::default().failing("read", KeychainError::from_status(-34018));
            resolve_install_id(Some(&fake), &dir.path().join(INSTALL_ID_FILE), None);
        });
        assert!(
            text.contains(
                "WARN bir_submit::ios_storage: Keychain install-id: read failed, OSStatus -34018 (errSecMissingEntitlement); using the install id in its file"
            ),
            "{text}"
        );
    }

    #[test]
    fn a_stored_value_that_is_not_an_install_id_is_not_used() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(INSTALL_ID_FILE);
        let fake = Fake::holding(&[(INSTALL_ID, "bad")]);
        let id = resolve_install_id(Some(&fake), &file, Some(OFFERED));
        assert_eq!(
            (id.value.as_str(), id.source),
            (OFFERED, InstallIdSource::File)
        );
        assert_eq!(
            fake.value(&INSTALL_ID).as_deref(),
            Some("bad"),
            "not overwritten"
        );
    }

    /// `BUWIZ_NO_KEYCHAIN=1` (no Keychain given): the file only.
    #[test]
    fn with_the_keychain_off_the_install_id_is_the_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(INSTALL_ID_FILE);
        let text = logged(|| {
            let id = resolve_install_id(None, &file, Some(OFFERED));
            assert_eq!(
                (id.value.as_str(), id.source),
                (OFFERED, InstallIdSource::File)
            );
            let again = resolve_install_id(None, &file, None);
            assert_eq!(again.value, OFFERED);
        });
        assert!(text.is_empty(), "no Keychain, nothing to log: {text}");
        // The real entry point makes no Keychain call with the switch off (and
        // none off iOS at all).
        let id = install_id(dir.path(), false, None);
        assert_eq!(
            (id.value.as_str(), id.source),
            (OFFERED, InstallIdSource::File)
        );
    }

    /// An unwritable folder still answers an id for this run.
    #[test]
    fn an_unwritable_file_never_stops_the_install_id() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("not-a-folder");
        std::fs::write(&blocker, "x").unwrap();
        let file = blocker.join(INSTALL_ID_FILE);
        let fake = Fake::default().failing("read", KeychainError::from_status(-25308));
        let id = resolve_install_id(Some(&fake), &file, None);
        assert!(is_valid_install_id(&id.value));
        assert_eq!(id.source, InstallIdSource::File);
    }

    #[test]
    fn the_old_items_go_once_and_failures_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join(OLD_ITEMS_MARKER);
        let fake = Fake::holding(&[
            (OLD_ITEMS[0], "seed"),
            (OLD_ITEMS[2], "token"),
            (INSTALL_ID, KEPT),
        ]);
        let outcome = remove_old_items(Some(&fake), &marker);
        assert_eq!(
            outcome.to_string(),
            "device-key deleted, device-id absent, sign-in deleted"
        );
        assert!(marker.exists());
        assert_eq!(
            fake.value(&INSTALL_ID).as_deref(),
            Some(KEPT),
            "never the install id"
        );
        assert_eq!(
            fake.calls(),
            [
                "delete device-key",
                "delete device-id",
                "delete desktop-refresh"
            ]
        );
        assert_eq!(
            remove_old_items(Some(&fake), &marker),
            OldItems::AlreadyDone
        );
        assert_eq!(fake.calls().len(), 3, "once");

        // Every delete failing: noted, ignored, and done all the same.
        for error in failures() {
            let dir = tempfile::tempdir().unwrap();
            let marker = dir.path().join(OLD_ITEMS_MARKER);
            let fake = Fake::holding(&[(INSTALL_ID, KEPT)]).failing("delete", error.clone());
            let outcome = remove_old_items(Some(&fake), &marker);
            assert!(
                outcome
                    .to_string()
                    .starts_with(&format!("device-key not deleted, {error}")),
                "{outcome}"
            );
            assert!(marker.exists(), "{error:?}");
            assert_eq!(
                remove_old_items(Some(&fake), &marker),
                OldItems::AlreadyDone
            );
            assert_eq!(fake.value(&INSTALL_ID).as_deref(), Some(KEPT));
        }
    }

    #[test]
    fn with_the_keychain_off_the_old_items_are_left_and_no_marker_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join(OLD_ITEMS_MARKER);
        assert_eq!(remove_old_items(None, &marker), OldItems::KeychainOff);
        assert!(!marker.exists());
        assert_eq!(
            OldItems::KeychainOff.to_string(),
            "skipped, the Keychain is off"
        );
        assert_eq!(
            OldItems::AlreadyDone.to_string(),
            "removed at an earlier launch"
        );
    }

    /// The old names, exactly as the build that used them stored them.
    #[test]
    fn the_old_items_are_the_three_the_old_build_stored() {
        let names: Vec<_> = OLD_ITEMS
            .iter()
            .map(|item| (item.service, item.account))
            .collect();
        assert_eq!(
            names,
            [
                ("com.buwiz.bir", "device-key"),
                ("com.buwiz.bir", "device-id"),
                ("com.buwiz.bir", "desktop-refresh"),
            ]
        );
    }

    #[test]
    fn the_status_and_its_name_travel_in_the_message() {
        for (status, text) in [
            (-34018, "OSStatus -34018 (errSecMissingEntitlement)"),
            (-25308, "OSStatus -25308 (errSecInteractionNotAllowed)"),
            (-25300, "not found, OSStatus -25300 (errSecItemNotFound)"),
            (
                -25299,
                "already there, OSStatus -25299 (errSecDuplicateItem)",
            ),
            (-25291, "OSStatus -25291 (errSecNotAvailable)"),
            (-25292, "OSStatus -25292 (errSecReadOnly)"),
            (-1, "OSStatus -1"),
        ] {
            let error = KeychainError::from_status(status);
            assert_eq!(error.to_string(), text);
            assert_eq!(error.status(), status);
        }
    }

    /// Off iOS a file gets no attribute and nothing is logged.
    #[cfg(not(target_os = "ios"))]
    #[test]
    fn file_protection_is_ios_only() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        std::fs::write(&file, "x").unwrap();
        assert!(logged(|| protect_file(&file)).is_empty());
    }
}
