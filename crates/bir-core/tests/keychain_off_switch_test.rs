//! `BUWIZ_NO_KEYCHAIN=1` in the SQLCipher build: with no `EBIR_TEST_ENV` there
//! is no database key, and none is ever generated. Nothing here may reach the
//! OS keychain: the switch is checked before any lookup.
#![cfg(feature = "sqlcipher")]

use bir_core::db::Database;

#[test]
fn with_the_keychain_off_and_no_test_env_there_is_no_database_key() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("bir_data.db");

    temp_env::with_vars(
        [
            ("BUWIZ_NO_KEYCHAIN", Some("1")),
            ("EBIR_TEST_ENV", None::<&str>),
        ],
        || {
            let create = Database::get_or_create_master_key().expect_err("no key is created");
            assert!(create.to_string().contains("switched off"), "{create}");

            let open = Database::open(&path);
            assert!(open.is_err(), "no key, no database");
            let read_only = Database::open_existing_read_only(&path);
            assert!(read_only.is_err());
            assert!(!path.exists(), "nothing was created: {}", path.display());
        },
    );
}

#[test]
fn the_test_env_key_still_wins_over_the_switch() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("bir_data.db");

    temp_env::with_vars(
        [
            ("BUWIZ_NO_KEYCHAIN", Some("1")),
            ("EBIR_TEST_ENV", Some("1")),
        ],
        || {
            let db = Database::open(&path).expect("the fixed test key opens it");
            db.set_setting("probe", "keyed").expect("write");
        },
    );
}
