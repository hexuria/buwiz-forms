//! The welcome line's "first time on this device" (accounts-and-lock W3).
//!
//! The welcome after an account's first sign-in here is the host's answer, never a page setting: the page asks
//! `welcome_first_time()` and hears true or false, nothing more. What this device keeps is host-only, in
//! `device_settings` of `device.db`, and names no account:
//! - [`WELCOME_KEY_SETTING`]: a random 32-byte key made once per device. Not the install id (it is sent to the
//!   server in every sign-in, so it is no secret) and not the relay device key (a new sign-in makes a new one, so
//!   every account would be welcomed again).
//! - [`WELCOMED_HASHES_SETTING`]: a JSON array of HMAC-SHA256 hashes, under that key, of each welcomed account's
//!   key ([`account_key`]: its user id, else its address trimmed and in lower case).
//!
//! The list from before W3 (`welcomed_accounts`, the ids or addresses themselves) is dropped
//! ([`OLD_WELCOMED_SETTING`]): at most one account is welcomed once more.

use pbkdf2::hmac::{Hmac, Mac};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use sha2::Sha256;

use crate::rooms::{DeviceStore, DeviceStoreError, DeviceStoreRef};

/// The per-device key of the hashes: host-only.
pub const WELCOME_KEY_SETTING: &str = "welcome_key";
/// The hashes of the accounts welcomed on this device: host-only.
pub const WELCOMED_HASHES_SETTING: &str = "welcomed_hashes";
/// The list of #417, which held the user ids and addresses themselves: dropped from both stores.
pub const OLD_WELCOMED_SETTING: &str = "welcomed_accounts";
/// The `device_settings` keys this module keeps, none of them the page's.
pub const HOST_ONLY_SETTINGS: [&str; 2] = [WELCOME_KEY_SETTING, WELCOMED_HASHES_SETTING];

/// The account as this device knows it: its user id, else its address trimmed and in lower case; `None` when
/// it has neither.
pub fn account_key(user_id: Option<&str>, email: Option<&str>) -> Option<String> {
    if let Some(user_id) = user_id.filter(|id| !id.is_empty()) {
        return Some(user_id.to_owned());
    }
    let email = email?.trim().to_lowercase();
    (!email.is_empty()).then_some(email)
}

/// HMAC-SHA256 of `account` under `key`, in hex.
pub fn keyed_hash(key: &[u8], account: &str) -> String {
    // HMAC takes a key of any length.
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC takes any key length");
    mac.update(account.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Whether the account `account` ([`account_key`]) signs in on this device for the first time; a first time is
/// recorded, so the next answer for it is false. One write transaction: the key is made on first use, and two
/// callers at once record one hash. The old list is dropped from `device_settings` here too.
pub fn first_time(store: &DeviceStoreRef, account: &str) -> Result<bool, DeviceStoreError> {
    store
        .with(true, |store| first_time_in(store, account))
        .map(|answer| answer.unwrap_or(false))
}

fn first_time_in(store: &DeviceStore, account: &str) -> Result<bool, DeviceStoreError> {
    let transaction =
        rusqlite::Transaction::new_unchecked(&store.conn, TransactionBehavior::Immediate)?;
    let read = |key: &str| -> Result<Option<String>, rusqlite::Error> {
        transaction
            .query_row(
                "SELECT value FROM device_settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
    };
    let write = |key: &str, value: &str| {
        transaction.execute(
            "INSERT INTO device_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )
    };
    transaction.execute(
        "DELETE FROM device_settings WHERE key = ?1",
        params![OLD_WELCOMED_SETTING],
    )?;
    // A key that does not read as 32 hex bytes is made again: the hashes under it no longer match, so at most
    // every account is welcomed once more.
    let key = match read(WELCOME_KEY_SETTING)?
        .and_then(|stored| hex::decode(stored).ok())
        .filter(|key| key.len() == 32)
    {
        Some(key) => key,
        None => {
            let key = rand::random::<[u8; 32]>().to_vec();
            write(WELCOME_KEY_SETTING, &hex::encode(&key))?;
            write(WELCOMED_HASHES_SETTING, "[]")?;
            key
        }
    };
    let hash = keyed_hash(&key, account);
    let mut hashes: Vec<String> = read(WELCOMED_HASHES_SETTING)?
        .and_then(|stored| serde_json::from_str(&stored).ok())
        .unwrap_or_default();
    if hashes.contains(&hash) {
        transaction.commit()?;
        return Ok(false);
    }
    hashes.push(hash);
    write(
        WELCOMED_HASHES_SETTING,
        &serde_json::Value::from(hashes).to_string(),
    )?;
    transaction.commit()?;
    Ok(true)
}

/// Drop the old list (#417's `welcomed_accounts`) from `device_settings` of `store`. No `device.db` has none, and
/// none is made for it.
pub fn drop_old_list(store: &DeviceStoreRef) -> Result<(), DeviceStoreError> {
    store
        .with(false, |store| store.delete_setting(OLD_WELCOMED_SETTING))
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn memory() -> DeviceStoreRef {
        DeviceStoreRef::Ephemeral(Arc::new(Mutex::new(DeviceStore::open_in_memory().unwrap())))
    }

    fn setting(store: &DeviceStoreRef, key: &str) -> Option<String> {
        store
            .with(false, |store| store.setting(key))
            .unwrap()
            .flatten()
    }

    #[test]
    fn the_account_key_is_the_user_id_else_the_trimmed_lowercased_address() {
        assert_eq!(
            account_key(Some("u-1"), Some("Juana@Example.com")).as_deref(),
            Some("u-1")
        );
        assert_eq!(
            account_key(Some(""), Some(" Juana@Example.com ")).as_deref(),
            Some("juana@example.com")
        );
        assert_eq!(account_key(None, Some("  ")), None);
        assert_eq!(account_key(None, None), None);
    }

    /// RFC 4231, test case 2: the hash is HMAC-SHA256.
    #[test]
    fn the_keyed_hash_is_hmac_sha256() {
        assert_eq!(
            keyed_hash(b"Jefe", "what do ya want for nothing?"),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn first_time_answers_once_per_account_and_keeps_one_key() {
        let store = memory();
        assert!(first_time(&store, "user-a").unwrap());
        let key = setting(&store, WELCOME_KEY_SETTING).unwrap();
        assert_eq!(key.len(), 64);
        assert!(!first_time(&store, "user-a").unwrap());
        assert!(first_time(&store, "user-b").unwrap());
        assert!(!first_time(&store, "user-b").unwrap());
        assert_eq!(setting(&store, WELCOME_KEY_SETTING).unwrap(), key);
    }

    #[test]
    fn the_hash_is_keyed_by_the_device() {
        let (one, other) = (memory(), memory());
        first_time(&one, "user-a").unwrap();
        first_time(&other, "user-a").unwrap();
        assert_ne!(
            setting(&one, WELCOMED_HASHES_SETTING),
            setting(&other, WELCOMED_HASHES_SETTING),
            "two devices store different hashes of one account"
        );
        let key = hex::decode(setting(&one, WELCOME_KEY_SETTING).unwrap()).unwrap();
        let hashes: Vec<String> =
            serde_json::from_str(&setting(&one, WELCOMED_HASHES_SETTING).unwrap()).unwrap();
        assert_eq!(hashes, [keyed_hash(&key, "user-a")]);
    }

    #[test]
    fn a_damaged_key_is_made_again_and_the_old_list_is_dropped() {
        let store = memory();
        store
            .with(true, |store| {
                store.set_setting(WELCOME_KEY_SETTING, "not hex")?;
                store.set_setting(OLD_WELCOMED_SETTING, "[\"user-a\"]")
            })
            .unwrap();
        assert!(first_time(&store, "user-a").unwrap());
        assert_eq!(setting(&store, OLD_WELCOMED_SETTING), None);
        assert_eq!(setting(&store, WELCOME_KEY_SETTING).unwrap().len(), 64);
        assert!(!first_time(&store, "user-a").unwrap());
    }
}
