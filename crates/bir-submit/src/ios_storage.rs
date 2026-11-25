//! How the app keeps its secrets and its folder on iOS: the Keychain items
//! with their protection class set explicitly, the class of an item stored
//! the old way changed in place, the clean-up after a fresh install, and the
//! app's data folder kept out of device backups.
//!
//! The rules are written against [`Backend`], so they are tested with a fake
//! on every system; the real Keychain ([`OsKeychain`]) and the backup
//! exclusion exist on iOS only. bir-core uses this module too (the saved
//! sign-in, the launch step): it lives in this leaf crate so both crates
//! reach it without a dependency cycle.
//!
//! Every item is `AfterFirstUnlockThisDeviceOnly`:
//! - readable and writable while the phone is locked, once it has been
//!   unlocked after a restart, so a send that outlives the screen can still
//!   store the refresh token the server rotated;
//! - never in a backup and never in iCloud Keychain, so a second phone cannot
//!   sign as this device.

use std::path::Path;

/// When a Keychain item can be read and whether it may leave the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protection {
    /// `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`.
    AfterFirstUnlockThisDeviceOnly,
}

/// One generic-password item: its service, its account and its class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Item {
    pub service: &'static str,
    pub account: &'static str,
    pub protection: Protection,
}

/// The device's Ed25519 signing key (`secrets` name `device_key`).
pub const DEVICE_KEY: Item = Item {
    service: crate::secrets::SERVICE,
    account: "device-key",
    protection: Protection::AfterFirstUnlockThisDeviceOnly,
};

/// The id the server registered the device key under (`device_id`).
pub const DEVICE_ID: Item = Item {
    service: crate::secrets::SERVICE,
    account: "device-id",
    protection: Protection::AfterFirstUnlockThisDeviceOnly,
};

/// The saved Buwiz sign-in, with its refresh token
/// (bir-core `cloud_account::KEYRING_SERVICE` / `KEYRING_ACCOUNT`).
pub const SIGN_IN: Item = Item {
    service: "com.buwiz.bir",
    account: "desktop-refresh",
    protection: Protection::AfterFirstUnlockThisDeviceOnly,
};

/// Why a Keychain call did not do what was asked.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeychainError {
    #[error("no such Keychain item (OSStatus -25300)")]
    NotFound,
    #[error("the Keychain item already exists (OSStatus -25299)")]
    Duplicate,
    /// `errSecInteractionNotAllowed`: the item's class does not allow it now
    /// (locked, or not unlocked since the restart). The item may well exist.
    #[error("the Keychain cannot be used while the device is locked (OSStatus -25308)")]
    Locked,
    /// `errSecNotAvailable`, `errSecReadOnly`: there is no usable Keychain.
    #[error("the Keychain is not available (OSStatus {0})")]
    Unavailable(i32),
    #[error("the Keychain refused (OSStatus {0})")]
    Failed(i32),
    #[error("the Keychain item is not UTF-8")]
    NotUtf8,
}

impl KeychainError {
    /// The error of a Security framework status that is not `errSecSuccess`.
    pub fn from_status(status: i32) -> Self {
        match status {
            -25300 => Self::NotFound,
            -25299 => Self::Duplicate,
            -25308 => Self::Locked,
            -25291 | -25292 => Self::Unavailable(status),
            other => Self::Failed(other),
        }
    }
}

/// An item as the Keychain holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    pub value: Vec<u8>,
    /// The item's class when it is the one [`Item::protection`] asks for;
    /// `None` for any other (the old default "when unlocked").
    pub protection: Option<Protection>,
}

/// The four Keychain calls the rules below are made of.
pub trait Backend {
    /// The item, or [`KeychainError::NotFound`].
    fn read(&self, item: &Item) -> Result<Stored, KeychainError>;
    /// Add the item with its class; [`KeychainError::Duplicate`] when it
    /// exists, which is never replaced by this call.
    fn add(&self, item: &Item, value: &[u8]) -> Result<(), KeychainError>;
    /// Set the existing item's class, and its value when one is given. In
    /// place: a failure leaves the item as it was.
    fn update(&self, item: &Item, value: Option<&[u8]>) -> Result<(), KeychainError>;
    fn delete(&self, item: &Item) -> Result<(), KeychainError>;
}

/// Read an item. One stored with another class (an older build stored
/// every item "when unlocked") is given its class in place, its value
/// untouched; when that fails the item stays exactly as it was, still
/// readable, and the next read tries again. Nothing here deletes.
pub fn get(backend: &dyn Backend, item: &Item) -> Result<Option<String>, KeychainError> {
    let stored = match backend.read(item) {
        Ok(stored) => stored,
        Err(KeychainError::NotFound) => return Ok(None),
        Err(error) => return Err(error),
    };
    let value = String::from_utf8(stored.value).map_err(|_| KeychainError::NotUtf8)?;
    if stored.protection != Some(item.protection) {
        match backend.update(item, None) {
            Ok(()) => tracing::info!(
                "Keychain item {}/{} now has its class",
                item.service,
                item.account
            ),
            Err(error) => tracing::warn!(
                "Keychain item {}/{} keeps its old class for now: {error}",
                item.service,
                item.account
            ),
        }
    }
    Ok(Some(value))
}

/// Create or replace an item, with its class either way.
pub fn set(backend: &dyn Backend, item: &Item, value: &str) -> Result<(), KeychainError> {
    match backend.add(item, value.as_bytes()) {
        Err(KeychainError::Duplicate) => backend.update(item, Some(value.as_bytes())),
        other => other,
    }
}

/// Delete an item; one that is not there counts as deleted.
pub fn delete(backend: &dyn Backend, item: &Item) -> Result<(), KeychainError> {
    match backend.delete(item) {
        Ok(()) | Err(KeychainError::NotFound) => Ok(()),
        Err(error) => Err(error),
    }
}

/// The name of the marker file, in the app's data folder, that tells a
/// relaunch from the first launch after an install.
pub const INSTALL_MARKER: &str = "keychain-install-marker";

/// What [`clear_on_fresh_install`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FreshInstall {
    /// The OS Keychain is switched off: nothing was called or written.
    KeychainOff,
    /// The marker is there: not a fresh install, nothing was touched.
    Relaunch,
    /// The items of an earlier install were deleted and the marker written.
    Cleared,
    /// Something could not be done; the marker was not written, so the next
    /// launch tries again.
    Incomplete(Vec<String>),
}

/// The items a fresh install deletes: exactly these three.
pub const CLEARED_ON_FRESH_INSTALL: [Item; 3] = [DEVICE_KEY, DEVICE_ID, SIGN_IN];

/// The first launch after an install. Keychain items outlive the app, so
/// an install would come back signed in with the old device key: with the
/// marker missing, delete [`CLEARED_ON_FRESH_INSTALL`] (the device key, the
/// device id, the saved sign-in) and nothing else, then write the marker.
/// With the Keychain off (`BUWIZ_NO_KEYCHAIN=1`) nothing is called and no
/// marker is written, so the first launch that uses the Keychain still
/// clears it.
pub fn clear_on_fresh_install(
    backend: &dyn Backend,
    keychain_on: bool,
    marker: &Path,
) -> FreshInstall {
    if !keychain_on {
        return FreshInstall::KeychainOff;
    }
    if marker.exists() {
        return FreshInstall::Relaunch;
    }
    let mut failures = Vec::new();
    for item in &CLEARED_ON_FRESH_INSTALL {
        if let Err(error) = delete(backend, item) {
            failures.push(format!("{}/{}: {error}", item.service, item.account));
        }
    }
    if failures.is_empty() {
        let written = marker
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(marker, b"1\n"));
        if let Err(error) = written {
            failures.push(format!("marker {}: {error}", marker.display()));
        }
    }
    if failures.is_empty() {
        FreshInstall::Cleared
    } else {
        FreshInstall::Incomplete(failures)
    }
}

#[cfg(target_os = "ios")]
pub use os::{OsKeychain, exclude_from_backup};

/// The real Keychain (`SecItem*`) and the backup exclusion.
#[cfg(target_os = "ios")]
mod os {
    use super::{Backend, Item, KeychainError, Protection, Stored};
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
        kSecClassGenericPassword, kSecReturnAttributes, kSecReturnData, kSecValueData,
    };
    use security_framework_sys::keychain_item::{
        SecItemAdd, SecItemCopyMatching, SecItemDelete, SecItemUpdate,
    };
    use std::ffi::c_void;

    // Not declared by security-framework-sys.
    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        static kSecAttrAccessible: CFStringRef;
    }

    // Declared by core-foundation-sys only behind a macOS feature.
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFURLIsExcludedFromBackupKey: CFStringRef;
    }

    fn constant(name: CFStringRef) -> CFString {
        // SAFETY: a Security framework constant, valid for the process.
        unsafe { CFString::wrap_under_get_rule(name) }
    }

    fn class_value(protection: Protection) -> CFString {
        match protection {
            Protection::AfterFirstUnlockThisDeviceOnly => {
                constant(unsafe { kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly })
            }
        }
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

    /// The app's Keychain through `SecItemCopyMatching`, `SecItemAdd`,
    /// `SecItemUpdate` and `SecItemDelete`.
    pub struct OsKeychain;

    impl Backend for OsKeychain {
        fn read(&self, item: &Item) -> Result<Stored, KeychainError> {
            let mut query = identity(item);
            query.push((
                constant(unsafe { kSecReturnData }),
                CFBoolean::true_value().into_CFType(),
            ));
            query.push((
                constant(unsafe { kSecReturnAttributes }),
                CFBoolean::true_value().into_CFType(),
            ));
            let query = CFDictionary::from_CFType_pairs(&query);
            let mut result: CFTypeRef = std::ptr::null();
            check(unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), &mut result) })?;
            if result.is_null() {
                return Err(KeychainError::Failed(-50));
            }
            // SAFETY: a Copy call's result, owned by us.
            let result = unsafe { CFType::wrap_under_create_rule(result) };
            let attributes = result
                .downcast_into::<CFDictionary>()
                .ok_or(KeychainError::Failed(-50))?;
            let field = |key: CFStringRef| {
                attributes
                    .find(key as *const c_void)
                    // SAFETY: a value the dictionary holds; retained here.
                    .map(|value| unsafe { CFType::wrap_under_get_rule(*value) })
            };
            let value = field(unsafe { kSecValueData })
                .and_then(|value| value.downcast::<CFData>())
                .map(|data| data.bytes().to_vec())
                .ok_or(KeychainError::Failed(-50))?;
            let class =
                field(unsafe { kSecAttrAccessible }).and_then(|class| class.downcast::<CFString>());
            let protection =
                (class == Some(class_value(item.protection))).then_some(item.protection);
            Ok(Stored { value, protection })
        }

        fn add(&self, item: &Item, value: &[u8]) -> Result<(), KeychainError> {
            let mut attributes = identity(item);
            attributes.push((
                constant(unsafe { kSecAttrAccessible }),
                class_value(item.protection).into_CFType(),
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

        fn update(&self, item: &Item, value: Option<&[u8]>) -> Result<(), KeychainError> {
            let query = CFDictionary::from_CFType_pairs(&identity(item));
            let mut changes = vec![(
                constant(unsafe { kSecAttrAccessible }),
                class_value(item.protection).into_CFType(),
            )];
            if let Some(value) = value {
                changes.push((
                    constant(unsafe { kSecValueData }),
                    CFData::from_buffer(value).into_CFType(),
                ));
            }
            let changes = CFDictionary::from_CFType_pairs(&changes);
            check(unsafe {
                SecItemUpdate(query.as_concrete_TypeRef(), changes.as_concrete_TypeRef())
            })
        }

        fn delete(&self, item: &Item) -> Result<(), KeychainError> {
            let query = CFDictionary::from_CFType_pairs(&identity(item));
            check(unsafe { SecItemDelete(query.as_concrete_TypeRef()) })
        }
    }

    /// Mark a directory "excluded from backup" (`NSURLIsExcludedFromBackupKey`):
    /// neither iCloud nor a computer backup copies it or what it holds.
    pub fn exclude_from_backup(directory: &std::path::Path) -> Result<(), String> {
        let url = CFURL::from_path(directory, true)
            .ok_or_else(|| format!("{} is not a file URL", directory.display()))?;
        let mut error = std::ptr::null_mut();
        let done = unsafe {
            core_foundation_sys::url::CFURLSetResourcePropertyForKey(
                url.as_concrete_TypeRef(),
                kCFURLIsExcludedFromBackupKey,
                CFBoolean::true_value().as_CFTypeRef(),
                &mut error,
            )
        };
        if done != 0 {
            return Ok(());
        }
        if error.is_null() {
            return Err("the backup exclusion was not set".into());
        }
        // SAFETY: an error the call created and handed to us.
        Err(unsafe { CFError::wrap_under_create_rule(error) }
            .description()
            .to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    type Key = (&'static str, &'static str);

    /// A Keychain in memory: each item's value and whether it has its class.
    /// Records every call; a call named in `fail` fails with its error.
    #[derive(Default)]
    struct Fake {
        items: RefCell<HashMap<Key, (Vec<u8>, bool)>>,
        calls: RefCell<Vec<String>>,
        fail: HashMap<&'static str, KeychainError>,
    }

    impl Fake {
        fn holding(entries: &[(Item, &str, bool)]) -> Self {
            let fake = Self::default();
            for (item, value, has_class) in entries {
                fake.items.borrow_mut().insert(
                    (item.service, item.account),
                    (value.as_bytes().to_vec(), *has_class),
                );
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
            match self.fail.get(call) {
                Some(error) => Err(error.clone()),
                None => Ok(()),
            }
        }
        fn entry(&self, item: &Item) -> Option<(String, bool)> {
            self.items
                .borrow()
                .get(&(item.service, item.account))
                .map(|(value, class)| (String::from_utf8(value.clone()).unwrap(), *class))
        }
        fn calls(&self) -> Vec<String> {
            self.calls.borrow().clone()
        }
    }

    impl Backend for Fake {
        fn read(&self, item: &Item) -> Result<Stored, KeychainError> {
            self.record("read", item)?;
            let items = self.items.borrow();
            let (value, has_class) = items
                .get(&(item.service, item.account))
                .ok_or(KeychainError::NotFound)?;
            Ok(Stored {
                value: value.clone(),
                protection: has_class.then_some(item.protection),
            })
        }
        fn add(&self, item: &Item, value: &[u8]) -> Result<(), KeychainError> {
            self.record("add", item)?;
            let mut items = self.items.borrow_mut();
            let key = (item.service, item.account);
            if items.contains_key(&key) {
                return Err(KeychainError::Duplicate);
            }
            items.insert(key, (value.to_vec(), true));
            Ok(())
        }
        fn update(&self, item: &Item, value: Option<&[u8]>) -> Result<(), KeychainError> {
            self.record("update", item)?;
            let mut items = self.items.borrow_mut();
            let entry = items
                .get_mut(&(item.service, item.account))
                .ok_or(KeychainError::NotFound)?;
            if let Some(value) = value {
                entry.0 = value.to_vec();
            }
            entry.1 = true;
            Ok(())
        }
        fn delete(&self, item: &Item) -> Result<(), KeychainError> {
            self.record("delete", item)?;
            self.items
                .borrow_mut()
                .remove(&(item.service, item.account))
                .map(drop)
                .ok_or(KeychainError::NotFound)
        }
    }

    /// Another app item (the calendar tokens, say), which no rule here
    /// may touch.
    const OTHER_ITEM: Item = Item {
        service: "dev.goldcoders.ebirforms.google-calendar",
        account: "global-oauth",
        protection: Protection::AfterFirstUnlockThisDeviceOnly,
    };

    #[test]
    fn every_item_stays_on_this_device_and_opens_after_the_first_unlock() {
        for item in [DEVICE_KEY, DEVICE_ID, SIGN_IN] {
            assert_eq!(
                item.protection,
                Protection::AfterFirstUnlockThisDeviceOnly,
                "{item:?}"
            );
        }
        // The names every earlier build stored them under.
        assert_eq!(
            (DEVICE_KEY.service, DEVICE_KEY.account),
            ("com.buwiz.bir", "device-key")
        );
        assert_eq!(
            (DEVICE_ID.service, DEVICE_ID.account),
            ("com.buwiz.bir", "device-id")
        );
        assert_eq!(
            (SIGN_IN.service, SIGN_IN.account),
            ("com.buwiz.bir", "desktop-refresh")
        );
        assert_eq!(CLEARED_ON_FRESH_INSTALL, [DEVICE_KEY, DEVICE_ID, SIGN_IN]);
    }

    #[test]
    fn the_security_statuses_map_to_their_meaning() {
        assert_eq!(KeychainError::from_status(-25300), KeychainError::NotFound);
        assert_eq!(KeychainError::from_status(-25299), KeychainError::Duplicate);
        assert_eq!(KeychainError::from_status(-25308), KeychainError::Locked);
        assert_eq!(
            KeychainError::from_status(-25291),
            KeychainError::Unavailable(-25291)
        );
        assert_eq!(
            KeychainError::from_status(-25292),
            KeychainError::Unavailable(-25292)
        );
        assert_eq!(
            KeychainError::from_status(-34018),
            KeychainError::Failed(-34018)
        );
    }

    #[test]
    fn a_new_item_is_added_with_its_class() {
        let fake = Fake::default();
        set(&fake, &SIGN_IN, "token").unwrap();
        assert_eq!(fake.entry(&SIGN_IN), Some(("token".into(), true)));
        assert_eq!(fake.calls(), vec!["add desktop-refresh"]);
    }

    /// A rotated refresh token replaces the old one in place, and an item
    /// still of the old class gets its class in the same call.
    #[test]
    fn writing_an_existing_item_replaces_value_and_class_in_place() {
        let fake = Fake::holding(&[(SIGN_IN, "old token", false)]);
        set(&fake, &SIGN_IN, "rotated").unwrap();
        assert_eq!(fake.entry(&SIGN_IN), Some(("rotated".into(), true)));
        assert_eq!(
            fake.calls(),
            vec!["add desktop-refresh", "update desktop-refresh"]
        );
    }

    #[test]
    fn an_old_class_item_gets_its_class_once_and_keeps_its_value() {
        for item in CLEARED_ON_FRESH_INSTALL {
            let fake = Fake::holding(&[(item, "value", false)]);
            assert_eq!(get(&fake, &item).unwrap().as_deref(), Some("value"));
            assert_eq!(fake.entry(&item), Some(("value".into(), true)));
            assert_eq!(get(&fake, &item).unwrap().as_deref(), Some("value"));
            let updates = fake
                .calls()
                .iter()
                .filter(|call| call.starts_with("update"))
                .count();
            assert_eq!(updates, 1, "{item:?}: once");
            assert!(!fake.calls().iter().any(|call| call.starts_with("delete")));
        }
    }

    #[test]
    fn an_item_of_its_class_is_read_without_a_write() {
        let fake = Fake::holding(&[(DEVICE_ID, "d1", true)]);
        assert_eq!(get(&fake, &DEVICE_ID).unwrap().as_deref(), Some("d1"));
        assert_eq!(fake.calls(), vec!["read device-id"]);
    }

    /// The class change fails (the phone is locked, say): the value is still
    /// returned, the item is still there as it was, nothing was deleted, and
    /// the next read tries the change again.
    #[test]
    fn a_failed_class_change_never_loses_the_value() {
        for error in [KeychainError::Locked, KeychainError::Failed(-34018)] {
            let fake =
                Fake::holding(&[(DEVICE_KEY, "seed", false)]).failing("update", error.clone());
            assert_eq!(get(&fake, &DEVICE_KEY).unwrap().as_deref(), Some("seed"));
            assert_eq!(fake.entry(&DEVICE_KEY), Some(("seed".into(), false)));
            assert_eq!(get(&fake, &DEVICE_KEY).unwrap().as_deref(), Some("seed"));
            assert_eq!(
                fake.calls(),
                vec![
                    "read device-key",
                    "update device-key",
                    "read device-key",
                    "update device-key",
                ],
                "{error:?}"
            );
        }
    }

    /// A read the Keychain refuses is an error, never "missing".
    #[test]
    fn a_locked_read_is_an_error_not_a_missing_item() {
        let fake =
            Fake::holding(&[(DEVICE_KEY, "seed", true)]).failing("read", KeychainError::Locked);
        assert_eq!(get(&fake, &DEVICE_KEY), Err(KeychainError::Locked));
        assert_eq!(get(&Fake::default(), &DEVICE_KEY), Ok(None));
    }

    fn stored() -> Fake {
        Fake::holding(&[
            (DEVICE_KEY, "seed", true),
            (DEVICE_ID, "d1", true),
            (SIGN_IN, "token", false),
            (OTHER_ITEM, "calendar", false),
        ])
    }

    /// Marker missing or there, a database file there or not, Keychain on
    /// or off: the table of the fresh-install rule.
    #[test]
    fn the_fresh_install_rule() {
        for marker_there in [false, true] {
            for database_there in [false, true] {
                for keychain_on in [false, true] {
                    let case = format!(
                        "marker {marker_there}, database {database_there}, keychain {keychain_on}"
                    );
                    let dir = tempfile::tempdir().unwrap();
                    let marker = dir.path().join(INSTALL_MARKER);
                    let database = dir.path().join("bir_data.db");
                    if marker_there {
                        std::fs::write(&marker, "1\n").unwrap();
                    }
                    if database_there {
                        std::fs::write(&database, "the returns").unwrap();
                    }
                    let fake = stored();
                    let outcome = clear_on_fresh_install(&fake, keychain_on, &marker);
                    let cleared = keychain_on && !marker_there;
                    if !keychain_on {
                        assert_eq!(outcome, FreshInstall::KeychainOff, "{case}");
                        assert!(fake.calls().is_empty(), "{case}: no Keychain call");
                        assert_eq!(marker.exists(), marker_there, "{case}: no marker written");
                    } else if marker_there {
                        assert_eq!(outcome, FreshInstall::Relaunch, "{case}");
                        assert!(fake.calls().is_empty(), "{case}");
                    } else {
                        assert_eq!(outcome, FreshInstall::Cleared, "{case}");
                        assert!(marker.exists(), "{case}");
                        assert_eq!(
                            fake.calls(),
                            vec![
                                "delete device-key",
                                "delete device-id",
                                "delete desktop-refresh"
                            ],
                            "{case}: the three and nothing else"
                        );
                    }
                    for item in CLEARED_ON_FRESH_INSTALL {
                        assert_eq!(fake.entry(&item).is_some(), !cleared, "{case}: {item:?}");
                    }
                    assert!(
                        fake.entry(&OTHER_ITEM).is_some(),
                        "{case}: other items stay"
                    );
                    assert_eq!(database.exists(), database_there, "{case}");
                    if database_there {
                        assert_eq!(std::fs::read_to_string(&database).unwrap(), "the returns");
                    }
                }
            }
        }
    }

    #[test]
    fn a_fresh_install_with_nothing_stored_still_writes_the_marker() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("folder").join(INSTALL_MARKER);
        let fake = Fake::default();
        assert_eq!(
            clear_on_fresh_install(&fake, true, &marker),
            FreshInstall::Cleared
        );
        assert!(marker.exists());
        assert_eq!(
            clear_on_fresh_install(&fake, true, &marker),
            FreshInstall::Relaunch
        );
    }

    /// A delete that fails leaves the marker unwritten, so the next launch
    /// clears again.
    #[test]
    fn a_failed_delete_is_retried_on_the_next_launch() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join(INSTALL_MARKER);
        let fake = stored().failing("delete", KeychainError::Locked);
        assert!(matches!(
            clear_on_fresh_install(&fake, true, &marker),
            FreshInstall::Incomplete(failures) if failures.len() == 3
        ));
        assert!(!marker.exists());
    }
}
