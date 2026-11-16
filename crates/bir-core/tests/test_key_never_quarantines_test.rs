//! The SQLCipher build under `EBIR_TEST_ENV` keys with the public test key.
//! A file that key cannot open was not made in a test run: it is never
//! quarantined, recreated or deleted, and the error says what to do. Built
//! against the library as the app uses it (not bir-core's `cfg(test)` build,
//! which keys every database with the test key), so this is the real wiring.
//! The keychain stays off: nothing here reaches the OS.
#![cfg(feature = "sqlcipher")]

use bir_core::db::{Database, DbError};

/// Bytes that are not a plain SQLite file: what an encrypted database looks
/// like to a key that does not open it.
fn not_ours() -> Vec<u8> {
    (0..8192u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect()
}

fn names(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_file_the_test_key_cannot_open_is_never_quarantined() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("bir_data.db");
    let bytes = not_ours();
    std::fs::write(&path, &bytes).unwrap();
    let named = path.display().to_string();

    for test_env in ["1", "true"] {
        temp_env::with_vars(
            [
                ("BUWIZ_NO_KEYCHAIN", Some("1")),
                ("EBIR_TEST_ENV", Some(test_env)),
                ("BIR_DATABASE_PATH", Some(named.as_str())),
            ],
            || {
                let error = Database::open_or_recreate(&path)
                    .map(|_| ())
                    .expect_err("not opened, not recreated");
                assert!(matches!(error, DbError::NotATestDatabase(_)), "{error}");
                let message = error.to_string();
                assert!(message.contains("not made in a test run"), "{message}");
                assert!(message.contains("BIR_DATABASE_PATH"), "{message}");

                // The app's own open (`open_live_database`) takes the same path.
                let live = bir_core::db::open_live_database()
                    .map(|_| ())
                    .expect_err("the launch open refuses too");
                assert!(matches!(live, DbError::NotATestDatabase(_)), "{live}");

                assert_eq!(
                    std::fs::read(&path).unwrap(),
                    bytes,
                    "{test_env}: the file is as it was"
                );
                assert!(
                    !names(directory.path())
                        .iter()
                        .any(|name| name.contains("corrupt")),
                    "{test_env}: nothing moved aside: {:?}",
                    names(directory.path())
                );
            },
        );
    }
}

/// The key the app opens with, through the public path production uses
/// (`get_or_create_master_key`): the test key only for an "on" value of
/// `EBIR_TEST_ENV`; `0`, `false`, empty or unset is not a test run, so with
/// the keychain off there is no key at all.
#[test]
fn the_test_key_is_used_only_in_a_test_environment() {
    for (value, test_key) in [
        (None, false),
        (Some(""), false),
        (Some("0"), false),
        (Some("false"), false),
        (Some("1"), true),
        (Some("true"), true),
    ] {
        temp_env::with_vars(
            [("BUWIZ_NO_KEYCHAIN", Some("1")), ("EBIR_TEST_ENV", value)],
            || match Database::get_or_create_master_key() {
                Ok(key) => {
                    assert!(test_key, "{value:?}: a key without a test run");
                    assert_eq!(key, "0".repeat(64), "{value:?}");
                }
                Err(error) => {
                    assert!(!test_key, "{value:?}: {error}");
                    assert!(
                        error.to_string().contains("switched off"),
                        "{value:?}: {error}"
                    );
                }
            },
        );
    }
}
